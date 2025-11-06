use clap::Parser;
use crossbeam::channel::{Sender, unbounded};
use rayon::prelude::*;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "omega")]
#[command(about = "Blazing fast cross-platform file search", long_about = None)]
struct Args {
    #[arg(required = true, help = "Search patterns")]
    patterns: Vec<String>,

    #[arg(short = 'p', long, help = "Search paths (can be used multiple times)")]
    path: Vec<PathBuf>,

    #[arg(short = 'l', long, help = "Limit number of results found")]
    limit_found: Option<u64>,

    #[arg(short = 's', long, help = "Limit number of items scanned")]
    limit_scanned: Option<u64>,

    #[arg(
        short = 't',
        long,
        help = "Number of threads (auto-detected if not specified)"
    )]
    threads: Option<usize>,

    #[arg(short = 'd', long, help = "Maximum search depth")]
    max_depth: Option<usize>,

    #[arg(short = 'i', long, help = "Case-sensitive search")]
    case_sensitive: bool,

    #[arg(short = 'q', long, help = "Quiet mode - only print paths")]
    quiet: bool,
}

enum OutputMode {
    Normal,
    Quiet,
}

impl OutputMode {
    fn from_args(args: &Args) -> Self {
        if args.quiet {
            Self::Quiet
        } else {
            Self::Normal
        }
    }

    fn format_result(&self, path: &Path, is_dir: bool) -> String {
        match self {
            Self::Quiet => format!("{}", path.display()),
            Self::Normal => {
                let marker = if is_dir { "[D]" } else { "[F]" };
                format!("{} {}", marker, path.display())
            }
        }
    }

    fn should_show_progress(&self) -> bool {
        matches!(self, Self::Normal)
    }
}

struct SearchMetrics {
    found: Arc<AtomicU64>,
    scanned: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
}

impl SearchMetrics {
    fn new() -> Self {
        Self {
            found: Arc::new(AtomicU64::new(0)),
            scanned: Arc::new(AtomicU64::new(0)),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    fn increment_found(&self) {
        self.found.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_scanned(&self) {
        self.scanned.fetch_add(1, Ordering::Relaxed);
    }

    fn get_found(&self) -> u64 {
        self.found.load(Ordering::Relaxed)
    }

    fn get_scanned(&self) -> u64 {
        self.scanned.load(Ordering::Relaxed)
    }

    fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    fn trigger_shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

struct SearchLimits {
    found: Option<u64>,
    scanned: Option<u64>,
}

impl SearchLimits {
    fn from_args(args: &Args) -> Self {
        Self {
            found: args.limit_found,
            scanned: args.limit_scanned,
        }
    }

    fn should_continue(&self, metrics: &SearchMetrics) -> bool {
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

struct PatternMatcher {
    patterns: Vec<String>,
    case_sensitive: bool,
}

impl PatternMatcher {
    fn from_args(args: &Args) -> Self {
        let patterns = if args.case_sensitive {
            args.patterns.clone()
        } else {
            args.patterns.iter().map(|s| s.to_lowercase()).collect()
        };

        Self {
            patterns,
            case_sensitive: args.case_sensitive,
        }
    }

    fn matches(&self, name: &str) -> bool {
        let target = if self.case_sensitive {
            name.to_string()
        } else {
            name.to_lowercase()
        };

        self.patterns.iter().any(|p| target.contains(p))
    }
}

struct SearchConfig {
    threads: usize,
    max_depth: Option<usize>,
}

impl SearchConfig {
    fn from_args(args: &Args) -> Self {
        let threads = args.threads.unwrap_or_else(|| {
            thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });

        Self {
            threads,
            max_depth: args.max_depth,
        }
    }
}

struct FileSystemScanner {
    matcher: PatternMatcher,
    limits: SearchLimits,
    metrics: SearchMetrics,
}

impl FileSystemScanner {
    fn new(matcher: PatternMatcher, limits: SearchLimits, metrics: SearchMetrics) -> Self {
        Self {
            matcher,
            limits,
            metrics,
        }
    }

    fn scan_directory(&self, root: &Path, max_depth: Option<usize>, tx: Sender<(PathBuf, bool)>) {
        let mut walker = WalkDir::new(root).follow_links(false);

        if let Some(depth) = max_depth {
            walker = walker.max_depth(depth);
        }

        for entry in walker.into_iter().filter_map(Result::ok) {
            if !self.limits.should_continue(&self.metrics) {
                break;
            }

            self.metrics.increment_scanned();

            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if self.matcher.matches(name) {
                    self.metrics.increment_found();
                    let is_dir = path.is_dir();
                    let _ = tx.send((path.to_path_buf(), is_dir));
                }
            }
        }
    }
}

struct ProgressReporter {
    metrics: SearchMetrics,
    show_progress: bool,
}

impl ProgressReporter {
    fn new(metrics: SearchMetrics, show_progress: bool) -> Self {
        Self {
            metrics,
            show_progress,
        }
    }

    fn run(&self) {
        if !self.show_progress {
            return;
        }

        let mut stdout = io::stdout();
        let mut last_scanned = 0u64;

        loop {
            thread::sleep(Duration::from_millis(300));

            if self.metrics.is_shutdown() {
                break;
            }

            let current_scanned = self.metrics.get_scanned();
            let current_found = self.metrics.get_found();

            if current_scanned > last_scanned {
                eprint!(
                    "\r\x1b[Komega: {} scanned | {} found",
                    current_scanned, current_found
                );
                let _ = stdout.flush();
                last_scanned = current_scanned;
            }
        }

        eprint!("\r\x1b[K");
        let _ = stdout.flush();
    }
}

struct ResultPrinter {
    output_mode: OutputMode,
}

impl ResultPrinter {
    fn new(output_mode: OutputMode) -> Self {
        Self { output_mode }
    }

    fn run<R>(&self, rx: R)
    where
        R: Iterator<Item = (PathBuf, bool)>,
    {
        for (path, is_dir) in rx {
            println!("{}", self.output_mode.format_result(&path, is_dir));
        }
    }
}

struct SearchEngine {
    scanner: FileSystemScanner,
    config: SearchConfig,
    output_mode: OutputMode,
}

impl SearchEngine {
    fn new(args: &Args) -> Self {
        let matcher = PatternMatcher::from_args(args);
        let limits = SearchLimits::from_args(args);
        let metrics = SearchMetrics::new();
        let config = SearchConfig::from_args(args);
        let output_mode = OutputMode::from_args(args);

        let scanner = FileSystemScanner::new(matcher, limits, metrics);

        Self {
            scanner,
            config,
            output_mode,
        }
    }

    fn search(&self, roots: Vec<PathBuf>) -> SearchResult {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.config.threads)
            .build()
            .unwrap();

        let (tx, rx) = unbounded::<(PathBuf, bool)>();

        let reporter = ProgressReporter::new(
            SearchMetrics {
                found: Arc::clone(&self.scanner.metrics.found),
                scanned: Arc::clone(&self.scanner.metrics.scanned),
                shutdown: Arc::clone(&self.scanner.metrics.shutdown),
            },
            self.output_mode.should_show_progress(),
        );

        let progress_handle = thread::spawn(move || {
            reporter.run();
        });

        let printer = ResultPrinter::new(if matches!(self.output_mode, OutputMode::Quiet) {
            OutputMode::Quiet
        } else {
            OutputMode::Normal
        });

        let printer_handle = thread::spawn(move || {
            printer.run(rx.into_iter());
        });

        pool.install(|| {
            roots.par_iter().for_each(|root| {
                if !self.scanner.limits.should_continue(&self.scanner.metrics) {
                    return;
                }
                self.scanner
                    .scan_directory(root, self.config.max_depth, tx.clone());
            });
        });

        drop(tx);
        self.scanner.metrics.trigger_shutdown();

        let _ = progress_handle.join();
        let _ = printer_handle.join();

        SearchResult {
            found: self.scanner.metrics.get_found(),
            scanned: self.scanner.metrics.get_scanned(),
        }
    }
}

struct SearchResult {
    found: u64,
    scanned: u64,
}

impl SearchResult {
    fn print_summary(&self, elapsed: f64, quiet: bool) {
        if quiet {
            return;
        }

        eprintln!(
            "omega: {} found in {:.2}s ({:.0}/s)",
            self.found,
            elapsed,
            self.scanned as f64 / elapsed
        );
    }
}

struct RootPathProvider;

impl RootPathProvider {
    fn get_search_roots(custom_paths: Vec<PathBuf>) -> Vec<PathBuf> {
        if !custom_paths.is_empty() {
            return custom_paths;
        }

        let mut roots = Vec::new();

        #[cfg(target_os = "windows")]
        {
            for drive in b'C'..=b'Z' {
                let root = format!("{}:\\", drive as char);
                let path = PathBuf::from(root);
                if path.exists() {
                    roots.push(path);
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            roots.push(PathBuf::from("/"));
        }

        roots
    }
}

fn main() {
    let args = Args::parse();
    let quiet = args.quiet;
    let custom_paths = args.path.clone();

    let engine = SearchEngine::new(&args);
    let roots = RootPathProvider::get_search_roots(custom_paths);

    let start = Instant::now();
    let result = engine.search(roots);
    let elapsed = start.elapsed().as_secs_f64();

    result.print_summary(elapsed, quiet);
}
