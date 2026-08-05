use crate::cli::Args;
use crate::matcher::PatternMatcher;
use crate::metrics::{SearchLimits, SearchMetrics};
use crate::output::{OutputMode, ResultPrinter, SearchResult};
use crate::scanner::{FileSystemScanner, SearchConfig};
use std::path::PathBuf;
use std::sync::mpsc::sync_channel;

const CHANNEL_DEPTH_PER_THREAD: usize = 4;

pub struct SearchEngine {
    scanner: FileSystemScanner,
    output_mode: OutputMode,
}

impl SearchEngine {
    pub fn new(args: &Args) -> Self {
        let output_mode = OutputMode::from_args(args);

        let scanner = FileSystemScanner::new(
            PatternMatcher::from_args(args),
            SearchLimits::from_args(args),
            SearchMetrics::new(),
            SearchConfig::from_args(args),
            output_mode,
        );

        Self {
            scanner,
            output_mode,
        }
    }

    pub fn search(&self, roots: &[PathBuf]) -> Result<SearchResult, String> {
        let threads = self.scanner.config.threads;

        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .map_err(|error| format!("Failed to create thread pool: {error}"))?;

        let (sender, receiver) = sync_channel(threads * CHANNEL_DEPTH_PER_THREAD);
        let printer = ResultPrinter::new(self.output_mode);
        let metrics = &self.scanner.metrics;

        std::thread::scope(|scope| {
            scope.spawn(move || printer.run(receiver, metrics));
            pool.install(|| self.scanner.run(roots, sender));
        });

        metrics.trigger_shutdown();

        Ok(SearchResult::new(
            metrics.get_found(),
            metrics.get_scanned(),
            metrics.get_errors(),
            self.output_mode,
        ))
    }
}
