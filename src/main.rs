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

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "omega")]
#[command(version = VERSION)]
#[command(about = "Blazing fast cross-platform file search", long_about = None)]
#[command(author = "Omega Contributors")]
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

    #[arg(short = 'i', long, help = "Case-insensitive search")]
    case_insensitive: bool,

    #[arg(short = 'q', long, help = "Quiet mode - only print paths")]
    quiet: bool,

    #[arg(short = 'f', long, help = "Search only files")]
    files_only: bool,

    #[arg(short = 'D', long, help = "Search only directories")]
    dirs_only: bool,

    #[arg(short = 'v', long, help = "Verbose output")]
    verbose: bool,

    #[arg(long, help = "Follow symbolic links")]
    follow_links: bool,

    #[arg(short = 'e', long, help = "Show errors")]
    show_errors: bool,

    #[arg(short = 'z', long, help = "Fuzzy search using Levenshtein distance")]
    fuzzy: bool,

    #[arg(
        short = 'T',
        long,
        default_value = "2",
        help = "Fuzzy search distance threshold"
    )]
    threshold: usize,
}

enum OutputMode {
    Normal,
    Quiet,
    Verbose,
}

impl OutputMode {
    fn from_args(args: &Args) -> Self {
        if args.quiet {
            Self::Quiet
        } else if args.verbose {
            Self::Verbose
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
            Self::Verbose => {
                let type_str = if is_dir { "DIR " } else { "FILE" };
                let metadata = std::fs::metadata(path);
                let size = metadata
                    .map(|m| m.len())
                    .map(|s| format_size(s))
                    .unwrap_or_else(|_| "unknown".to_string());
                format!("[{}] {:>10} {}", type_str, size, path.display())
            }
        }
    }

    fn should_show_progress(&self) -> bool {
        !matches!(self, Self::Quiet)
    }
}

fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_idx = 0;

    while size >= 1024.0 && unit_idx < UNITS.len() - 1 {
        size /= 1024.0;
        unit_idx += 1;
    }

    if unit_idx == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.2} {}", size, UNITS[unit_idx])
    }
}

struct SearchMetrics {
    found: Arc<AtomicU64>,
    scanned: Arc<AtomicU64>,
    errors: Arc<AtomicU64>,
    shutdown: Arc<AtomicBool>,
}

impl SearchMetrics {
    fn new() -> Self {
        Self {
            found: Arc::new(AtomicU64::new(0)),
            scanned: Arc::new(AtomicU64::new(0)),
            errors: Arc::new(AtomicU64::new(0)),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    fn increment_found(&self) {
        self.found.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_scanned(&self) {
        self.scanned.fetch_add(1, Ordering::Relaxed);
    }

    fn increment_errors(&self) {
        self.errors.fetch_add(1, Ordering::Relaxed);
    }

    fn get_found(&self) -> u64 {
        self.found.load(Ordering::Relaxed)
    }

    fn get_scanned(&self) -> u64 {
        self.scanned.load(Ordering::Relaxed)
    }

    fn get_errors(&self) -> u64 {
        self.errors.load(Ordering::Relaxed)
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

fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.chars().count();
    let len2 = s2.chars().count();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut prev_row: Vec<usize> = (0..=len2).collect();
    let mut curr_row = vec![0; len2 + 1];

    for (i, c1) in s1.chars().enumerate() {
        curr_row[0] = i + 1;

        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            curr_row[j + 1] = (curr_row[j] + 1)
                .min(prev_row[j + 1] + 1)
                .min(prev_row[j] + cost);
        }

        std::mem::swap(&mut prev_row, &mut curr_row);
    }

    prev_row[len2]
}

struct PatternMatcher {
    patterns: Vec<String>,
    case_sensitive: bool,
    fuzzy: bool,
    threshold: usize,
}

impl PatternMatcher {
    fn from_args(args: &Args) -> Self {
        let case_sensitive = !args.case_insensitive;
        let patterns = if case_sensitive {
            args.patterns.clone()
        } else {
            args.patterns.iter().map(|s| s.to_lowercase()).collect()
        };

        Self {
            patterns,
            case_sensitive,
            fuzzy: args.fuzzy,
            threshold: args.threshold,
        }
    }

    fn matches(&self, name: &str) -> bool {
        let target = if self.case_sensitive {
            name.to_string()
        } else {
            name.to_lowercase()
        };

        if self.fuzzy {
            self.patterns.iter().any(|p| {
                let exact_match = target.contains(p);
                if exact_match {
                    return true;
                }

                let words: Vec<&str> = target.split(|c: char| !c.is_alphanumeric()).collect();
                words.iter().any(|word| {
                    if word.len() > 0 {
                        levenshtein_distance(p, word) <= self.threshold
                    } else {
                        false
                    }
                })
            })
        } else {
            self.patterns.iter().any(|p| target.contains(p))
        }
    }
}

struct SearchConfig {
    threads: usize,
    max_depth: Option<usize>,
    files_only: bool,
    dirs_only: bool,
    follow_links: bool,
    show_errors: bool,
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
            files_only: args.files_only,
            dirs_only: args.dirs_only,
            follow_links: args.follow_links,
            show_errors: args.show_errors,
        }
    }

    fn should_include(&self, is_dir: bool) -> bool {
        if self.files_only && is_dir {
            return false;
        }
        if self.dirs_only && !is_dir {
            return false;
        }
        true
    }
}

struct FileSystemScanner {
    matcher: PatternMatcher,
    limits: SearchLimits,
    metrics: SearchMetrics,
    config: SearchConfig,
}

impl FileSystemScanner {
    fn new(
        matcher: PatternMatcher,
        limits: SearchLimits,
        metrics: SearchMetrics,
        config: SearchConfig,
    ) -> Self {
        Self {
            matcher,
            limits,
            metrics,
            config,
        }
    }

    fn scan_directory(&self, root: &Path, tx: Sender<(PathBuf, bool)>) {
        let mut walker = WalkDir::new(root).follow_links(self.config.follow_links);

        if let Some(depth) = self.config.max_depth {
            walker = walker.max_depth(depth);
        }

        for entry_result in walker.into_iter() {
            if !self.limits.should_continue(&self.metrics) {
                break;
            }

            let entry = match entry_result {
                Ok(e) => e,
                Err(e) => {
                    self.metrics.increment_errors();
                    if self.config.show_errors {
                        eprintln!("omega: error: {}", e);
                    }
                    continue;
                }
            };

            self.metrics.increment_scanned();

            let path = entry.path();
            let is_dir = path.is_dir();

            if !self.config.should_include(is_dir) {
                continue;
            }

            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if self.matcher.matches(name) {
                    self.metrics.increment_found();
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

        let scanner_config = SearchConfig::from_args(args);
        let scanner = FileSystemScanner::new(matcher, limits, metrics, scanner_config);

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
                errors: Arc::clone(&self.scanner.metrics.errors),
                shutdown: Arc::clone(&self.scanner.metrics.shutdown),
            },
            self.output_mode.should_show_progress(),
        );

        let progress_handle = thread::spawn(move || {
            reporter.run();
        });

        let printer = ResultPrinter::new(match self.output_mode {
            OutputMode::Quiet => OutputMode::Quiet,
            OutputMode::Verbose => OutputMode::Verbose,
            OutputMode::Normal => OutputMode::Normal,
        });

        let printer_handle = thread::spawn(move || {
            printer.run(rx.into_iter());
        });

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

        let _ = progress_handle.join();
        let _ = printer_handle.join();

        SearchResult {
            found: self.scanner.metrics.get_found(),
            scanned: self.scanner.metrics.get_scanned(),
            errors: self.scanner.metrics.get_errors(),
        }
    }
}

struct SearchResult {
    found: u64,
    scanned: u64,
    errors: u64,
}

impl SearchResult {
    fn print_summary(&self, elapsed: f64, quiet: bool, show_errors: bool) {
        if quiet {
            return;
        }

        let rate = self.scanned as f64 / elapsed;
        eprint!(
            "omega: {} found in {:.2}s ({:.0}/s)",
            self.found, elapsed, rate
        );

        if show_errors && self.errors > 0 {
            eprint!(" | {} errors", self.errors);
        }

        eprintln!();
    }
}

struct RootPathProvider;

impl RootPathProvider {
    fn get_search_roots(custom_paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
        if !custom_paths.is_empty() {
            for path in &custom_paths {
                if !path.exists() {
                    return Err(format!("path does not exist: {}", path.display()));
                }
            }
            return Ok(custom_paths);
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

        if roots.is_empty() {
            return Err("no valid search paths found".to_string());
        }

        Ok(roots)
    }
}

fn main() {
    let args = Args::parse();

    if args.files_only && args.dirs_only {
        eprintln!("omega: error: cannot use --files-only and --dirs-only together");
        std::process::exit(1);
    }

    let roots = match RootPathProvider::get_search_roots(args.path.clone()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("omega: error: {}", e);
            std::process::exit(1);
        }
    };

    let quiet = args.quiet;
    let show_errors = args.show_errors;
    let engine = SearchEngine::new(&args);

    let start = Instant::now();
    let result = engine.search(roots);
    let elapsed = start.elapsed().as_secs_f64();

    result.print_summary(elapsed, quiet, show_errors);
}
