use crate::cli::Args;
use crate::utils::{path_bytes, write_timestamp};
use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[repr(align(64))]
struct Padded<T>(T);

pub struct SearchMetrics {
    found: Padded<AtomicU64>,
    scanned: Padded<AtomicU64>,
    errors: Padded<AtomicU64>,
    shutdown: Padded<AtomicBool>,
    log: Mutex<Option<File>>,
}

impl Default for SearchMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchMetrics {
    pub fn new() -> Self {
        Self {
            found: Padded(AtomicU64::new(0)),
            scanned: Padded(AtomicU64::new(0)),
            errors: Padded(AtomicU64::new(0)),
            shutdown: Padded(AtomicBool::new(false)),
            log: Mutex::new(None),
        }
    }

    #[inline]
    pub fn increment_found(&self) {
        self.found.0.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn claim_found(&self, limit: u64) -> bool {
        self.found
            .0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |found| {
                (found < limit).then_some(found + 1)
            })
            .is_ok()
    }

    #[inline]
    pub fn add_scanned(&self, count: u64) {
        if count != 0 {
            self.scanned.0.fetch_add(count, Ordering::Relaxed);
        }
    }

    pub fn record_error(&self, path: &Path, error: &std::io::Error) {
        if error.kind() == ErrorKind::PermissionDenied {
            return;
        }

        self.errors.0.fetch_add(1, Ordering::Relaxed);

        let Ok(mut guard) = self.log.lock() else {
            return;
        };

        if guard.is_none() {
            *guard = OpenOptions::new()
                .create(true)
                .append(true)
                .open("omega.log")
                .ok();
        }

        let Some(file) = guard.as_mut() else {
            return;
        };

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());

        let mut line = Vec::with_capacity(128);
        line.push(b'[');
        write_timestamp(&mut line, timestamp);
        line.extend_from_slice(b"] ");
        line.extend_from_slice(&path_bytes(path));
        line.extend_from_slice(b": ");
        let _ = write!(line, "{error}");
        line.push(b'\n');

        let _ = file.write_all(&line);
    }

    #[inline]
    pub fn get_found(&self) -> u64 {
        self.found.0.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn get_scanned(&self) -> u64 {
        self.scanned.0.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn get_errors(&self) -> u64 {
        self.errors.0.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.0.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn trigger_shutdown(&self) {
        self.shutdown.0.store(true, Ordering::Relaxed);
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

    #[inline]
    pub fn found_limit(&self) -> Option<u64> {
        self.found
    }

    #[inline]
    pub fn should_continue(&self, metrics: &SearchMetrics) -> bool {
        if metrics.is_shutdown() {
            return false;
        }

        if let Some(limit) = self.found
            && metrics.get_found() >= limit
        {
            metrics.trigger_shutdown();
            return false;
        }

        if let Some(limit) = self.scanned
            && metrics.get_scanned() >= limit
        {
            metrics.trigger_shutdown();
            return false;
        }

        true
    }
}
