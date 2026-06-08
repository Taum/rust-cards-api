use crate::plan::{DownloadErrorRow, DownloadIndexRow, DownloadTask, ResolvedCard};
use crate::progress::{self, StepGuard};
use anyhow::{bail, Context, Result};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use reqwest::blocking::Client;
use indicatif::ProgressBar;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

pub struct DownloadOptions {
    pub plan_resolved: PathBuf,
    pub out_dir: PathBuf,
    pub concurrency: usize,
    pub max_retries: u32,
    pub backoff_ms: u64,
    pub timeout_secs: u64,
    pub spot_check_n: usize,
    pub force: bool,
    pub use_proxy: bool,
    pub proxy_width: u32,
    pub proxy_quality: u32,
    pub user_agent: String,
    pub seed: u64,
    /// Max HTTP image fetches per second across all workers (`0` = unlimited).
    pub images_per_second: f64,
}

const DOWNLOAD_ETA_WINDOW: Duration = Duration::from_secs(15);

struct DownloadProgressTracker {
    already_on_disk: u64,
    pending_fetch: u64,
    skipped: u64,
    downloaded: u64,
    errors: u64,
    fetch_rate: progress::RollingRate,
    pb: Arc<ProgressBar>,
}

impl DownloadProgressTracker {
    fn new(already_on_disk: usize, pending_fetch: usize, pb: ProgressBar) -> Self {
        let mut tracker = Self {
            already_on_disk: already_on_disk as u64,
            pending_fetch: pending_fetch as u64,
            skipped: 0,
            downloaded: 0,
            errors: 0,
            fetch_rate: progress::RollingRate::new(DOWNLOAD_ETA_WINDOW),
            pb: Arc::new(pb),
        };
        tracker.refresh_message();
        tracker
    }

    fn record_skip(&mut self) {
        self.skipped += 1;
        self.pb.inc(1);
        self.refresh_message();
    }

    fn record_download(&mut self) {
        self.downloaded += 1;
        self.fetch_rate.record(Instant::now());
        self.pb.inc(1);
        self.refresh_message();
    }

    fn record_error(&mut self) {
        self.errors += 1;
        self.pb.inc(1);
        self.refresh_message();
    }

    fn refresh_message(&mut self) {
        let msg = self.format_message();
        self.pb.set_message(msg);
    }

    fn format_message(&mut self) -> String {
        let now = Instant::now();
        let rate_per_sec = self.fetch_rate.rate_per_sec(now);
        let remaining_fetch = self.pending_fetch.saturating_sub(self.downloaded);
        let rate_str = if self.downloaded > 0 && remaining_fetch > 0 && rate_per_sec > 0.0 {
            format!(" | {:.1}/s (15s avg)", rate_per_sec)
        } else {
            String::new()
        };
        let eta_str = if remaining_fetch == 0 {
            String::new()
        } else if rate_per_sec > 0.0 {
            let eta = Duration::from_secs_f64(remaining_fetch as f64 / rate_per_sec);
            format!(" | ETA {}", progress::format_duration(eta))
        } else {
            " | ETA —".to_string()
        };
        format!(
            "on disk {}/{} | fetch {}/{}{} | err {}{}",
            self.skipped,
            self.already_on_disk,
            self.downloaded,
            self.pending_fetch,
            rate_str,
            self.errors,
            eta_str
        )
    }

    fn finish_message(&self) -> String {
        format!(
            "on disk {}/{} | fetch {}/{} | err {}",
            self.skipped,
            self.already_on_disk,
            self.downloaded,
            self.pending_fetch,
            self.errors
        )
    }

    fn counts(&self) -> (usize, usize, usize) {
        (
            self.downloaded as usize,
            self.skipped as usize,
            self.errors as usize,
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadSummary {
    pub resolved_cards: usize,
    pub download_tasks: usize,
    pub downloaded: usize,
    pub skipped_existing: usize,
    pub errors: usize,
    pub by_set: BTreeMap<String, usize>,
    pub by_locale: BTreeMap<String, usize>,
}

pub fn run(opts: &DownloadOptions) -> Result<DownloadSummary> {
    const P: &str = "download";
    let (resolved_cards, tasks) = {
        let step = StepGuard::begin(P, "load plan-resolved");
        let loaded = load_tasks(
            &opts.plan_resolved,
            opts.use_proxy,
            opts.proxy_width,
            opts.proxy_quality,
        )?;
        step.finish(Some(&format!(
            "{} cards, {} download tasks",
            loaded.0,
            loaded.1.len()
        )));
        loaded
    };
    anyhow::ensure!(
        !tasks.is_empty(),
        "no download tasks in {}",
        opts.plan_resolved.display()
    );
    anyhow::ensure!(
        opts.images_per_second >= 0.0,
        "images-per-second must be >= 0 (0 = unlimited)"
    );
    std::fs::create_dir_all(&opts.out_dir)
        .with_context(|| format!("create out dir {}", opts.out_dir.display()))?;
    let images_root = opts.out_dir.join("images");
    std::fs::create_dir_all(&images_root)
        .with_context(|| format!("create images dir {}", images_root.display()))?;

    let client = Client::builder()
        .user_agent(&opts.user_agent)
        .timeout(Duration::from_secs(opts.timeout_secs))
        .build()
        .context("build reqwest client")?;

    let pending = tasks_pending(&tasks, &images_root, opts.force);
    let already_on_disk = tasks.len().saturating_sub(pending);
    if already_on_disk > 0 {
        eprintln!(
            "resume: {already_on_disk}/{} image(s) already on disk (will verify, not re-fetch)",
            tasks.len()
        );
    }
    if pending > 0 {
        eprintln!("fetch: {pending}/{} image(s) to download", tasks.len());
    }

    if !opts.force && opts.spot_check_n > 0 {
        spot_check(&client, &tasks, &images_root, opts)?;
    }

    let index_path = opts.out_dir.join("index.jsonl");
    let errors_path = opts.out_dir.join("errors.jsonl");
    let indexed_keys = Arc::new(Mutex::new(if opts.force {
        BTreeSet::new()
    } else {
        load_index_keys(&index_path)?
    }));
    let index_writer = Arc::new(Mutex::new(open_log_writer(&index_path, opts.force)?));
    let errors_writer = Arc::new(Mutex::new(open_log_writer(&errors_path, opts.force)?));

    let total = tasks.len();
    let task_queue = Arc::new(Mutex::new(tasks.into_iter()));
    let by_set = Arc::new(Mutex::new(BTreeMap::<String, usize>::new()));
    let by_locale = Arc::new(Mutex::new(BTreeMap::<String, usize>::new()));
    let progress = Arc::new(Mutex::new(DownloadProgressTracker::new(
        already_on_disk,
        pending,
        progress::download_bar(P, "download", total as u64),
    )));
    let progress_done = Arc::new(AtomicBool::new(false));
    {
        let progress = Arc::clone(&progress);
        let done = Arc::clone(&progress_done);
        thread::spawn(move || {
            while !done.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(500));
                if let Ok(mut tracker) = progress.lock() {
                    tracker.refresh_message();
                }
            }
        });
    }
    let throttle = Arc::new(DownloadThrottle::new(opts.images_per_second));
    if throttle.is_enabled() {
        eprintln!(
            "throttle: {:.2} image fetch(es)/s across {} worker(s)",
            opts.images_per_second,
            opts.concurrency.max(1)
        );
    }

    let workers = opts.concurrency.max(1);
    let mut handles = Vec::with_capacity(workers);
    for _ in 0..workers {
        let client = client.clone();
        let task_queue = Arc::clone(&task_queue);
        let index_writer = Arc::clone(&index_writer);
        let errors_writer = Arc::clone(&errors_writer);
        let progress = Arc::clone(&progress);
        let by_set = Arc::clone(&by_set);
        let by_locale = Arc::clone(&by_locale);
        let images_root = images_root.clone();
        let indexed_keys = Arc::clone(&indexed_keys);
        let max_retries = opts.max_retries;
        let backoff_ms = opts.backoff_ms;
        let force = opts.force;
        let throttle = Arc::clone(&throttle);
        handles.push(thread::spawn(move || {
            loop {
                let task = {
                    let mut q = task_queue.lock().unwrap();
                    q.next()
                };
                let Some(task) = task else { break };
                match process_task(
                    &client,
                    &task,
                    &images_root,
                    max_retries,
                    backoff_ms,
                    force,
                    &throttle,
                ) {
                    Ok(ProcessResult::Downloaded(index_row)) => {
                        record_index_row(
                            &index_writer,
                            &indexed_keys,
                            &index_row,
                            &task,
                            &by_set,
                            &by_locale,
                        );
                        progress.lock().unwrap().record_download();
                    }
                    Ok(ProcessResult::Skipped(index_row)) => {
                        record_index_row(
                            &index_writer,
                            &indexed_keys,
                            &index_row,
                            &task,
                            &by_set,
                            &by_locale,
                        );
                        progress.lock().unwrap().record_skip();
                    }
                    Err(e) => {
                        let mut w = errors_writer.lock().unwrap();
                        let err = DownloadErrorRow {
                            card: task.card.clone(),
                            locale: task.locale.clone(),
                            src_url: task.src_url.clone(),
                            error: format!("{e:#}"),
                        };
                        let _ = serde_json::to_writer(&mut *w, &err);
                        let _ = w.write_all(b"\n");
                        progress.lock().unwrap().record_error();
                    }
                }
            }
        }));
    }
    progress_done.store(true, Ordering::Relaxed);
    for h in handles {
        let _ = h.join();
    }
    let (finish_msg, pb, downloaded, skipped, errors) = {
        let tracker = progress.lock().unwrap();
        let (downloaded, skipped, errors) = tracker.counts();
        (
            tracker.finish_message(),
            Arc::clone(&tracker.pb),
            downloaded,
            skipped,
            errors,
        )
    };
    progress::finish_bar((*pb).clone(), finish_msg);

    {
        let mut w = index_writer.lock().unwrap();
        w.flush()?;
    }
    {
        let mut w = errors_writer.lock().unwrap();
        w.flush()?;
    }

    let summary = DownloadSummary {
        resolved_cards,
        download_tasks: total,
        downloaded,
        skipped_existing: skipped,
        errors,
        by_set: by_set.lock().unwrap().clone(),
        by_locale: by_locale.lock().unwrap().clone(),
    };
    let manifest_path = opts.out_dir.join("manifest.json");
    let text = serde_json::to_string_pretty(&summary)?;
    std::fs::write(&manifest_path, text)
        .with_context(|| format!("write {}", manifest_path.display()))?;
    Ok(summary)
}

enum ProcessResult {
    Downloaded(DownloadIndexRow),
    Skipped(DownloadIndexRow),
}

/// Global rate limiter shared across download worker threads.
struct DownloadThrottle {
    min_interval: Duration,
    next_slot: Mutex<Instant>,
}

impl DownloadThrottle {
    fn new(images_per_second: f64) -> Self {
        let min_interval = if images_per_second > 0.0 {
            Duration::from_secs_f64(1.0 / images_per_second)
        } else {
            Duration::ZERO
        };
        Self {
            min_interval,
            next_slot: Mutex::new(Instant::now()),
        }
    }

    fn is_enabled(&self) -> bool {
        !self.min_interval.is_zero()
    }

    /// Block until this worker may start the next HTTP image fetch.
    fn wait_before_fetch(&self) {
        if !self.is_enabled() {
            return;
        }
        let sleep_for = {
            let mut next = self.next_slot.lock().unwrap();
            let now = Instant::now();
            let slot = if *next > now { *next } else { now };
            *next = slot + self.min_interval;
            slot.saturating_duration_since(now)
        };
        if !sleep_for.is_zero() {
            thread::sleep(sleep_for);
        }
    }
}

fn process_task(
    client: &Client,
    task: &DownloadTask,
    images_root: &Path,
    max_retries: u32,
    backoff_ms: u64,
    force: bool,
    throttle: &DownloadThrottle,
) -> Result<ProcessResult> {
    let dest = image_dest(images_root, task);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create dir {}", parent.display()))?;
    }

    if is_complete_on_disk(&dest, force) {
        let bytes = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
        let sha = hash_file(&dest)?;
        return Ok(ProcessResult::Skipped(DownloadIndexRow {
                card: task.card.clone(),
                locale: task.locale.clone(),
                locale_tier: task.locale_tier,
                shape_floor: task.shape_floor,
                rel_path: task.rel_path.clone(),
                src_url: task.src_url.clone(),
                local_path: dest.display().to_string(),
                sha256: sha,
                bytes,
        }));
    }

    throttle.wait_before_fetch();
    let bytes_buf = fetch_with_retries(client, &task.src_url, max_retries, backoff_ms)?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes_buf);
    let sha = hex_encode(&hasher.finalize());
    write_atomic(&dest, &bytes_buf)?;
    Ok(ProcessResult::Downloaded(DownloadIndexRow {
        card: task.card.clone(),
        locale: task.locale.clone(),
        locale_tier: task.locale_tier,
        shape_floor: task.shape_floor,
        rel_path: task.rel_path.clone(),
        src_url: task.src_url.clone(),
        local_path: dest.display().to_string(),
        sha256: sha,
        bytes: bytes_buf.len() as u64,
    }))
}

fn fetch_with_retries(
    client: &Client,
    url: &str,
    max_retries: u32,
    backoff_ms: u64,
) -> Result<Vec<u8>> {
    let mut attempt: u32 = 0;
    loop {
        let outcome = client
            .get(url)
            .header(reqwest::header::ACCEPT, "image/avif,image/webp,image/*,*/*;q=0.8")
            .send();
        match outcome {
            Ok(resp) if resp.status().is_success() => {
                let bytes = resp.bytes().context("read response body")?;
                return Ok(bytes.to_vec());
            }
            Ok(resp) if resp.status() == reqwest::StatusCode::NOT_FOUND => {
                bail!("HTTP 404 for {url}");
            }
            Ok(resp) if !resp.status().is_server_error() => {
                bail!("HTTP {} for {url}", resp.status());
            }
            Ok(resp) => {
                if attempt >= max_retries {
                    bail!("HTTP {} for {url} after {} retries", resp.status(), attempt);
                }
            }
            Err(e) => {
                if attempt >= max_retries {
                    return Err(anyhow::anyhow!("network error for {url}: {e}"));
                }
            }
        }
        attempt += 1;
        let sleep = backoff_ms.saturating_mul(1u64 << attempt.min(6));
        thread::sleep(Duration::from_millis(sleep));
    }
}

fn image_dest(images_root: &Path, task: &DownloadTask) -> PathBuf {
    images_root
        .join(&task.card.set)
        .join(&task.card.faction)
        .join(&task.card.family_number)
        .join(&task.card.reference)
        .join(format!("{}.jpg", task.locale))
}

fn hash_file(path: &Path) -> Result<String> {
    let mut f = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 32 * 1024];
    loop {
        let n = f
            .read(&mut buf)
            .with_context(|| format!("read {}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex_encode(&hasher.finalize()))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn load_tasks(
    path: &Path,
    use_proxy: bool,
    proxy_width: u32,
    proxy_quality: u32,
) -> Result<(usize, Vec<DownloadTask>)> {
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    let reader = BufReader::new(file);
    let mut cards = 0usize;
    let mut tasks = Vec::new();
    for (line_no, line) in reader.lines().enumerate() {
        let l = line?;
        if l.trim().is_empty() {
            continue;
        }
        let card: ResolvedCard = serde_json::from_str(&l)
            .with_context(|| format!("parse resolved card at line {}", line_no + 1))?;
        tasks.extend(card.download_tasks(use_proxy, proxy_width, proxy_quality));
        cards += 1;
    }
    Ok((cards, tasks))
}

fn is_complete_on_disk(dest: &Path, force: bool) -> bool {
    if force {
        return false;
    }
    dest.exists()
        && std::fs::metadata(dest)
            .map(|m| m.len())
            .unwrap_or(0)
            > 0
}

fn tasks_pending(tasks: &[DownloadTask], images_root: &Path, force: bool) -> usize {
    tasks
        .iter()
        .filter(|t| !is_complete_on_disk(&image_dest(images_root, t), force))
        .count()
}

fn write_atomic(dest: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = dest.with_extension("jpg.part");
    if tmp.exists() {
        let _ = std::fs::remove_file(&tmp);
    }
    std::fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    std::fs::rename(&tmp, dest).with_context(|| format!("rename {} -> {}", tmp.display(), dest.display()))?;
    Ok(())
}

fn index_key(row: &DownloadIndexRow) -> (String, String) {
    (row.card.reference.clone(), row.locale.clone())
}

fn load_index_keys(path: &Path) -> Result<BTreeSet<(String, String)>> {
    let mut keys = BTreeSet::new();
    if !path.exists() {
        return Ok(keys);
    }
    let file = File::open(path).with_context(|| format!("open {}", path.display()))?;
    for line in BufReader::new(file).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(row) = serde_json::from_str::<DownloadIndexRow>(&line) {
            keys.insert(index_key(&row));
        }
    }
    Ok(keys)
}

fn open_log_writer(path: &Path, force: bool) -> Result<BufWriter<File>> {
    let file = if force || !path.exists() {
        File::create(path).with_context(|| format!("create {}", path.display()))?
    } else {
        OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("append {}", path.display()))?
    };
    Ok(BufWriter::new(file))
}

fn record_index_row(
    index_writer: &Arc<Mutex<BufWriter<File>>>,
    indexed_keys: &Arc<Mutex<BTreeSet<(String, String)>>>,
    index_row: &DownloadIndexRow,
    task: &DownloadTask,
    by_set: &Arc<Mutex<BTreeMap<String, usize>>>,
    by_locale: &Arc<Mutex<BTreeMap<String, usize>>>,
) {
    let key = index_key(index_row);
    let write_row = {
        let mut keys = indexed_keys.lock().unwrap();
        if keys.contains(&key) {
            false
        } else {
            keys.insert(key);
            true
        }
    };
    if write_row {
        let mut w = index_writer.lock().unwrap();
        let _ = serde_json::to_writer(&mut *w, index_row);
        let _ = w.write_all(b"\n");
    }
    *by_set
        .lock()
        .unwrap()
        .entry(task.card.set.clone())
        .or_insert(0) += 1;
    *by_locale
        .lock()
        .unwrap()
        .entry(task.locale.clone())
        .or_insert(0) += 1;
}

fn spot_check(
    client: &Client,
    tasks: &[DownloadTask],
    images_root: &Path,
    opts: &DownloadOptions,
) -> Result<()> {
    let pending: Vec<usize> = tasks
        .iter()
        .enumerate()
        .filter(|(_, t)| !is_complete_on_disk(&image_dest(images_root, t), opts.force))
        .map(|(i, _)| i)
        .collect();
    if pending.is_empty() {
        eprintln!("spot-check: all images already on disk, skipping");
        return Ok(());
    }
    let n = opts.spot_check_n.min(pending.len());
    if n == 0 {
        return Ok(());
    }
    let mut rng = ChaCha8Rng::seed_from_u64(opts.seed);
    let mut indices: Vec<usize> = pending;
    for i in (1..indices.len()).rev() {
        let j = rng.gen_range(0..=i);
        indices.swap(i, j);
    }
    indices.truncate(n);
    eprintln!("spot-check: fetching {} URL(s) before bulk download...", n);
    let mut failures: Vec<String> = Vec::new();
    for i in indices {
        let task = &tasks[i];
        let resp = client.head(&task.src_url).send();
        match resp {
            Ok(r) if r.status().is_success() => {
                eprintln!(
                    "  OK {} {} -> {} {}",
                    task.card.reference, task.locale, r.status(), task.src_url
                );
            }
            Ok(r) => {
                eprintln!(
                    "  FAIL {} {} -> {} {}",
                    task.card.reference, task.locale, r.status(), task.src_url
                );
                failures.push(format!("{} {}", r.status(), task.src_url));
            }
            Err(e) => {
                eprintln!(
                    "  ERR  {} {} -> {} ({})",
                    task.card.reference, task.locale, task.src_url, e
                );
                failures.push(format!("network {} ({e})", task.src_url));
            }
        }
    }
    if !failures.is_empty() {
        bail!(
            "spot-check failed for {} URL(s); inspect output above. Re-run with --force to continue anyway.",
            failures.len()
        );
    }
    Ok(())
}

pub fn print_summary(summary: &DownloadSummary) {
    println!("== download ==");
    println!(
        "  resolved_cards={}, download_tasks={}, downloaded={}, skipped_existing={}, errors={}",
        summary.resolved_cards,
        summary.download_tasks,
        summary.downloaded,
        summary.skipped_existing,
        summary.errors
    );
    if summary.skipped_existing > 0 {
        println!("  (skipped_existing = resumed from files already in out/images/)");
    }
    println!();
    println!("  by set:");
    for (set, count) in &summary.by_set {
        println!("    {:<10} {}", set, count);
    }
    println!();
    println!("  by locale:");
    for (locale, count) in &summary.by_locale {
        println!("    {:<8} {}", locale, count);
    }
}
