use crate::cli::Args;
use crate::utils::format_timestamp;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

pub struct SearchMetrics {
    found: Arc<AtomicU64>,
    scanned: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
    log_file: Arc<Mutex<Option<std::fs::File>>>,
}

impl Default for SearchMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchMetrics {
    pub fn new() -> Self {
        Self {
            found: Arc::new(AtomicU64::new(0)),
            scanned: Arc::new(AtomicU64::new(0)),
            errors: Arc::new(AtomicU64::new(0)),
            shutdown: Arc::new(AtomicBool::new(false)),
            log_file: Arc::new(Mutex::new(None)),
        }
    }

    pub fn increment_found(&self) {
        self.found.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_scanned(&self) {
        self.scanned.fetch_add(1, Ordering::Relaxed);
    }

    pub fn increment_errors(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    pub fn log_error(&self, error: &str) {
        if error.contains("Access is denied") {
            return;
        }

        self.increment_errors();

        let mut guard = match self.log_file.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        if guard.is_none() {
            *guard = OpenOptions::new()
                .create(true)
                .append(true)
                .open("omega.log")
                .ok();
        }

        if let Some(file) = guard.as_mut() {
            let timestamp = std::time::SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let formatted_time = format_timestamp(timestamp);
            let _ = writeln!(file, "[{}] {}", formatted_time, error);
            let _ = file.flush();
        }
    }

    pub fn get_found(&self) -> u64 {
        self.found.load(Ordering::Relaxed)
    }

    pub fn get_scanned(&self) -> u64 {
        self.scanned.load(Ordering::Relaxed)
    }

    pub fn get_errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
    }

    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    pub fn trigger_shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

pub struct SearchLimits {
    found: Option<u64>,
    scanned: Option<u64>,
}

impl SearchLimits {
    pub fn from_args(args: &Args) -> Self {
        Self {
            found: args.limit_found,
            scanned: args.limit_scanned,
        }
    }

    pub fn should_continue(&self, metrics: &SearchMetrics) -> bool {
        if metrics.is_shutdown() {
            return false;
        }

        if let Some(limit) = self.found {
            if metrics.get_found() >= limit {
                metrics.trigger_shutdown();
                return false;
            }
        }

        if let Some(limit) = self.scanned {
            if metrics.get_scanned() >= limit {
                metrics.trigger_shutdown();
                return false;
            }
        }

        true
    }
}
