use indicatif::{ProgressBar, ProgressStyle};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub fn log(prefix: &str, message: &str) {
    eprintln!("[{prefix}] {message}");
}

pub fn log_done(prefix: &str, step: &str, elapsed: Duration, detail: Option<&str>) {
    match detail {
        Some(d) => eprintln!("[{prefix}] {step} done in {elapsed:.1?} — {d}"),
        None => eprintln!("[{prefix}] {step} done in {elapsed:.1?}"),
    }
}

pub struct StepGuard {
    prefix: String,
    step: String,
    started: Instant,
}

impl StepGuard {
    pub fn begin(prefix: &str, step: &str) -> Self {
        log(prefix, &format!("{step}…"));
        Self {
            prefix: prefix.to_string(),
            step: step.to_string(),
            started: Instant::now(),
        }
    }

    pub fn finish(self, detail: Option<&str>) {
        log_done(&self.prefix, &self.step, self.started.elapsed(), detail);
    }
}

pub fn bar(prefix: &str, label: &str, len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::with_template(
            "[{prefix}] [{bar:40.cyan/blue}] {pos}/{len} {msg} ({elapsed_precise})",
        )
        .expect("progress template")
        .progress_chars("█▓░"),
    );
    pb.set_prefix(format!("{prefix} {label}"));
    pb.enable_steady_tick(Duration::from_millis(120));
    pb
}

pub fn finish_bar(pb: ProgressBar, message: impl AsRef<str>) {
    pb.finish_with_message(message.as_ref().to_string());
}

/// Rolling event rate over a fixed time window (used for download ETA).
pub struct RollingRate {
    window: Duration,
    events: VecDeque<Instant>,
}

impl RollingRate {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            events: VecDeque::new(),
        }
    }

    pub fn record(&mut self, at: Instant) {
        self.prune(at);
        self.events.push_back(at);
    }

    pub fn rate_per_sec(&mut self, now: Instant) -> f64 {
        self.prune(now);
        if self.events.is_empty() {
            return 0.0;
        }
        self.events.len() as f64 / self.window.as_secs_f64()
    }

    fn prune(&mut self, now: Instant) {
        while self
            .events
            .front()
            .is_some_and(|t| now.duration_since(*t) > self.window)
        {
            self.events.pop_front();
        }
    }
}

pub fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs >= 3600 {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    } else if secs >= 60 {
        format!("{}m{:02}s", secs / 60, secs % 60)
    } else {
        format!("{}s", secs)
    }
}

pub fn download_bar(prefix: &str, label: &str, len: u64) -> ProgressBar {
    let pb = ProgressBar::new(len);
    pb.set_style(
        ProgressStyle::with_template(
            "[{prefix}] [{bar:40.cyan/blue}] {pos}/{len} {msg} ({elapsed_precise})",
        )
        .expect("progress template")
        .progress_chars("█▓░"),
    );
    pb.set_prefix(format!("{prefix} {label}"));
    pb.enable_steady_tick(Duration::from_millis(120));
    pb
}
