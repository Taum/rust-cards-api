use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Tracks throughput over the most recent tick interval (1 second).
struct RateTracker {
    total: u64,
    processed: u64,
    at_last_tick: u64,
    last_tick: Instant,
}

impl RateTracker {
    fn new(total: u64) -> Self {
        let now = Instant::now();
        Self {
            total,
            processed: 0,
            at_last_tick: 0,
            last_tick: now,
        }
    }

    fn inc(&mut self) {
        self.processed += 1;
    }

    /// Files processed per second since the previous tick.
    fn tick_rate(&mut self) -> f64 {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_tick).as_secs_f64();
        let rate = if elapsed > 0.0 {
            (self.processed - self.at_last_tick) as f64 / elapsed
        } else {
            0.0
        };
        self.at_last_tick = self.processed;
        self.last_tick = now;
        rate
    }

    fn eta_secs(&self, rate_per_sec: f64) -> Option<u64> {
        if rate_per_sec <= 0.0 {
            return None;
        }
        let remaining = self.total.saturating_sub(self.processed);
        Some((remaining as f64 / rate_per_sec).ceil() as u64)
    }
}

pub struct DiscoveryProgress {
    spinner: ProgressBar,
}

impl DiscoveryProgress {
    pub fn start() -> Self {
        let spinner = ProgressBar::new_spinner();
        spinner.set_draw_target(progress_target());
        spinner.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .expect("discovery spinner template"),
        );
        spinner.set_message("Discovering card files…");
        Self { spinner }
    }

    pub fn set_found(&self, found: usize, limit: Option<usize>, last_file: &str) {
        let count = match limit {
            Some(max) => format!("{found} / {max} files"),
            None => format!("{found} files"),
        };
        let msg = format!("Discovering… {count} | last: {last_file}");
        self.spinner.set_message(msg);
    }

    pub fn finish(self, count: usize, limit: Option<usize>, stopped_early: bool) {
        let msg = match (limit, stopped_early) {
            (Some(max), true) => format!("Found {count} card files (stopped at limit {max})"),
            (Some(max), false) => format!("Found {count} card files (under limit {max})"),
            (None, _) => format!("Found {count} card files"),
        };
        self.spinner.finish_with_message(msg);
    }
}

pub struct BuildProgress {
    bar: ProgressBar,
    tracker: Arc<Mutex<RateTracker>>,
    _tick_thread: Option<std::thread::JoinHandle<()>>,
}

impl BuildProgress {
    pub fn start(total: usize) -> Self {
        let total = total as u64;
        let bar = ProgressBar::new(total);
        bar.set_draw_target(progress_target());
        bar.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} ({percent:>3}%) {msg}",
            )
            .expect("build progress template")
            .progress_chars("█▓▒░  "),
        );
        bar.set_message("starting…");

        let tracker = Arc::new(Mutex::new(RateTracker::new(total)));
        let tick_thread = if progress_enabled() {
            let bar_tick = bar.clone();
            let tracker_tick = Arc::clone(&tracker);
            Some(std::thread::spawn(move || {
                while !bar_tick.is_finished() {
                    std::thread::sleep(Duration::from_secs(1));
                    if bar_tick.is_finished() {
                        break;
                    }
                    let msg = {
                        let mut t = tracker_tick.lock().expect("rate tracker lock");
                        let rate = t.tick_rate();
                        let eta = format_eta(t.eta_secs(rate));
                        format!(
                            "{}/{} total | {rate:.0} files/s (last 1s) | ETA {eta}",
                            t.processed, t.total
                        )
                    };
                    bar_tick.set_message(msg);
                }
            }))
        } else {
            None
        };

        Self {
            bar,
            tracker,
            _tick_thread: tick_thread,
        }
    }

    pub fn inc(&self) {
        let mut t = self.tracker.lock().expect("rate tracker lock");
        t.inc();
        self.bar.inc(1);
    }

    pub fn finish(self, msg: &str) {
        self.bar.finish_with_message(msg.to_string());
    }
}

pub struct WriteProgress {
    spinner: ProgressBar,
}

impl WriteProgress {
    pub fn start() -> Self {
        let spinner = ProgressBar::new_spinner();
        spinner.set_draw_target(progress_target());
        spinner.set_style(
            ProgressStyle::with_template("{spinner:.cyan} {msg}")
                .expect("write spinner template"),
        );
        spinner.set_message("Writing catalog and idGd bitmaps…");
        Self { spinner }
    }

    pub fn finish(self) {
        self.spinner.finish_and_clear();
    }
}

fn progress_target() -> ProgressDrawTarget {
    if progress_enabled() {
        ProgressDrawTarget::stderr()
    } else {
        ProgressDrawTarget::hidden()
    }
}

fn progress_enabled() -> bool {
    std::io::IsTerminal::is_terminal(&std::io::stderr())
}

fn format_eta(secs: Option<u64>) -> String {
    match secs {
        None => "—".to_string(),
        Some(0) => "0s".to_string(),
        Some(s) if s < 60 => format!("{s}s"),
        Some(s) if s < 3600 => format!("{}m {:02}s", s / 60, s % 60),
        Some(s) => format!("{}h {:02}m", s / 3600, (s % 3600) / 60),
    }
}
