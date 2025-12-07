use crate::cli::Args;
use crate::matcher::PatternMatcher;
use crate::metrics::{SearchLimits, SearchMetrics};
use crate::output::{OutputMode, ResultPrinter, SearchResult};
use crate::scanner::{FileSystemScanner, SearchConfig};
use crossbeam::channel::unbounded;
use rayon::prelude::*;
use std::path::PathBuf;
use std::thread;

pub struct SearchEngine {
    scanner: FileSystemScanner,
    config: SearchConfig,
    output_mode: OutputMode,
}

impl SearchEngine {
    pub fn new(args: &Args) -> Self {
        let matcher = PatternMatcher::from_args(args);
        let limits = SearchLimits::from_args(args);
        let metrics = SearchMetrics::new();
        let config = SearchConfig::from_args(args);
        let output_mode = OutputMode::from_args(args);

        let scanner_config = SearchConfig::from_args(args);
        let scanner = FileSystemScanner::new(matcher, limits, metrics, scanner_config);

        Self {
            scanner,
            config,
            output_mode,
        }
    }

    pub fn search(&self, roots: Vec<PathBuf>) -> Result<SearchResult, String> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.config.threads)
            .build()
            .map_err(|e| format!("Failed to create thread pool: {}", e))?;

        let (tx, rx) = unbounded();
        let printer = ResultPrinter::new(self.output_mode);
        let printer_handle = thread::spawn(move || printer.run(rx.into_iter()));

        pool.install(|| {
            roots.par_iter().for_each(|root| {
                if !self.scanner.limits.should_continue(&self.scanner.metrics) {
                    return;
                }
                self.scanner.scan_directory(root, tx.clone());
            });
        });

        drop(tx);
        self.scanner.metrics.trigger_shutdown();

        printer_handle
            .join()
            .map_err(|_| "Printer thread panicked".to_string())?;

        Ok(SearchResult::new(
            self.scanner.metrics.get_found(),
            self.scanner.metrics.get_scanned(),
            self.scanner.metrics.get_errors(),
            self.output_mode,
        ))
    }
}
