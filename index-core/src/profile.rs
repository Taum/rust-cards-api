use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// True when `--profile` is set or `CLI_INDEXER_PROFILE` is `1` / `true` (case-insensitive).
pub fn profile_enabled(cli_flag: bool) -> bool {
    if cli_flag {
        return true;
    }
    std::env::var("CLI_INDEXER_PROFILE")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
}

#[derive(Debug, Default)]
pub struct BuildProfile {
    pub discovery_ns: u64,
    pub read_ns: u64,
    pub parse_ns: u64,
    pub process_ns: u64,
    pub write_ns: u64,
    pub bytes_read: u64,
    pub cards_indexed: u32,
}

impl BuildProfile {
    pub fn time<F, R>(f: F) -> (R, u64)
    where
        F: FnOnce() -> R,
    {
        let start = Instant::now();
        let result = f();
        (result, start.elapsed().as_nanos() as u64)
    }

    pub fn total_ns(&self) -> u64 {
        self.discovery_ns + self.read_ns + self.parse_ns + self.process_ns + self.write_ns
    }

    pub fn print_report(&self) {
        let total = self.total_ns().max(1);
        let cards = self.cards_indexed;
        println!();
        println!("build profile ({cards} cards):");
        self.print_line("discovery", self.discovery_ns, total);
        self.print_line("read", self.read_ns, total);
        self.print_line("parse", self.parse_ns, total);
        self.print_line("process", self.process_ns, total);
        self.print_line("write", self.write_ns, total);
        self.print_line("total", total, total);
        if self.bytes_read > 0 {
            let mib = self.bytes_read as f64 / (1024.0 * 1024.0);
            println!("  bytes read: {} ({mib:.2} MiB)", self.bytes_read);
        }
    }

    fn print_line(&self, label: &str, ns: u64, total_ns: u64) {
        let secs = ns as f64 / 1_000_000_000.0;
        let pct = (ns as f64 / total_ns as f64) * 100.0;
        println!("  {label:<10} {secs:>7.1}s  ({pct:>5.1}%)");
    }
}

/// Per-card phase samples over a rolling wall-clock window (for live progress).
#[derive(Debug)]
pub struct PhaseRollingWindow {
    samples: VecDeque<PhaseSample>,
    window: Duration,
}

#[derive(Debug)]
struct PhaseSample {
    at: Instant,
    read_ns: u64,
    parse_ns: u64,
    process_ns: u64,
}

impl PhaseRollingWindow {
    pub fn new(window: Duration) -> Self {
        Self {
            samples: VecDeque::new(),
            window,
        }
    }

    pub fn record(&mut self, read_ns: u64, parse_ns: u64, process_ns: u64) {
        let now = Instant::now();
        self.samples.push_back(PhaseSample {
            at: now,
            read_ns,
            parse_ns,
            process_ns,
        });
        self.trim(now);
    }

    fn trim(&mut self, now: Instant) {
        let cutoff = now.checked_sub(self.window).unwrap_or(now);
        while self
            .samples
            .front()
            .is_some_and(|s| s.at < cutoff)
        {
            self.samples.pop_front();
        }
    }

    /// Average milliseconds per card for samples in the window.
    pub fn avg_ms_per_card(&mut self) -> (f64, f64, f64) {
        let now = Instant::now();
        self.trim(now);
        let n = self.samples.len();
        if n == 0 {
            return (0.0, 0.0, 0.0);
        }
        let mut read_ns = 0u64;
        let mut parse_ns = 0u64;
        let mut process_ns = 0u64;
        for s in &self.samples {
            read_ns += s.read_ns;
            parse_ns += s.parse_ns;
            process_ns += s.process_ns;
        }
        let n = n as f64;
        (
            read_ns as f64 / n / 1_000_000.0,
            parse_ns as f64 / n / 1_000_000.0,
            process_ns as f64 / n / 1_000_000.0,
        )
    }

    pub fn sample_count(&mut self) -> usize {
        let now = Instant::now();
        self.trim(now);
        self.samples.len()
    }
}

pub const PHASE_WINDOW: Duration = Duration::from_secs(5);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_enabled_cli_flag() {
        assert!(profile_enabled(true));
    }

    #[test]
    fn phase_window_averages_recent_samples() {
        let mut window = PhaseRollingWindow::new(Duration::from_secs(5));
        window.record(1_000_000, 2_000_000, 3_000_000);
        window.record(3_000_000, 4_000_000, 5_000_000);
        let (read, parse, process) = window.avg_ms_per_card();
        assert!((read - 2.0).abs() < 0.01);
        assert!((parse - 3.0).abs() < 0.01);
        assert!((process - 4.0).abs() < 0.01);
    }

    #[test]
    fn total_ns_sums_buckets() {
        let p = BuildProfile {
            discovery_ns: 1,
            read_ns: 2,
            parse_ns: 3,
            process_ns: 4,
            write_ns: 5,
            ..Default::default()
        };
        assert_eq!(p.total_ns(), 15);
    }
}
