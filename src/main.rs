use clap::Parser;
use crossbeam::channel::{Sender, unbounded};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "omega")]
#[command(version = VERSION)]
#[command(about = "Blazing fast cross-platform file search", long_about = None)]
#[command(author = "Nima Naseri")]
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

    #[arg(short = 'f', long, help = "Search only files")]
    files_only: bool,

    #[arg(short = 'D', long, help = "Search only directories")]
    dirs_only: bool,

    #[arg(short = 'e', long, help = "Hide errors")]
    hide_errors: bool,

    #[arg(short = 'z', long, help = "Fuzzy search using Levenshtein distance")]
    fuzzy: bool,

    #[arg(
        short = 'T',
        long = "fuzzy-threshold",
        default_value = "2",
        help = "Fuzzy search distance threshold"
    )]
    fuzzy_threshold: usize,

    #[arg(long, help = "API mode - output as CSV format")]
    api: bool,
}

#[derive(Debug, Clone)]
struct FileInfo {
    path: String,
    name: String,
    is_dir: bool,
    is_file: bool,
    size: u64,
    size_human: String,
    modified: u64,
    modified_human: String,
    is_hidden: bool,
    extension: String,
    permissions: String,
}

impl FileInfo {
    fn from_path(path: &Path) -> Result<Self, std::io::Error> {
        let metadata = std::fs::metadata(path)?;
        let is_dir = metadata.is_dir();
        let is_file = metadata.is_file();

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string())
            .unwrap_or_default();

        let size = if is_file { metadata.len() } else { 0 };
        let size_human = format_size(size);

        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let modified_human = format_timestamp(modified);

        let is_hidden = is_hidden_file(path, &name);

        let permissions = format_permissions(&metadata);

        Ok(FileInfo {
            path: path.display().to_string(),
            name,
            is_dir,
            is_file,
            size,
            size_human,
            modified,
            modified_human,
            is_hidden,
            extension,
            permissions,
        })
    }

    fn to_csv(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{},{}",
            escape_csv(&self.path),
            escape_csv(&self.name),
            self.is_dir,
            self.is_file,
            self.size,
            escape_csv(&self.size_human),
            self.modified,
            escape_csv(&self.modified_human),
            self.is_hidden,
            escape_csv(&self.extension),
            escape_csv(&self.permissions)
        )
    }

    fn csv_header() -> &'static str {
        "path,name,is_dir,is_file,size,size_human,modified,modified_human,is_hidden,extension,permissions"
    }
}

fn is_hidden_file(path: &Path, _name: &str) -> bool {
    #[cfg(unix)]
    {
        _name.starts_with('.')
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if let Ok(metadata) = std::fs::metadata(path) {
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
            (metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN) != 0
        } else {
            false
        }
    }

    #[cfg(not(any(unix, windows)))]
    {
        _name.starts_with('.')
    }
}

fn escape_csv(s: &str) -> String {
    if s.contains(',')
        || s.contains('"')
        || s.contains('\n')
        || s.contains('\r')
        || s.contains('\t')
    {
        format!("\"{}\"", s.replace("\"", "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(unix)]
fn format_permissions(metadata: &std::fs::Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    let mode = metadata.permissions().mode();

    let user = format!(
        "{}{}{}",
        if mode & 0o400 != 0 { "r" } else { "-" },
        if mode & 0o200 != 0 { "w" } else { "-" },
        if mode & 0o100 != 0 { "x" } else { "-" }
    );

    let group = format!(
        "{}{}{}",
        if mode & 0o040 != 0 { "r" } else { "-" },
        if mode & 0o020 != 0 { "w" } else { "-" },
        if mode & 0o010 != 0 { "x" } else { "-" }
    );

    let other = format!(
        "{}{}{}",
        if mode & 0o004 != 0 { "r" } else { "-" },
        if mode & 0o002 != 0 { "w" } else { "-" },
        if mode & 0o001 != 0 { "x" } else { "-" }
    );

    format!("{}{}{}", user, group, other)
}

#[cfg(windows)]
fn format_permissions(metadata: &std::fs::Metadata) -> String {
    let readonly = metadata.permissions().readonly();
    if readonly {
        "r--r--r--".to_string()
    } else {
        "rw-rw-rw-".to_string()
    }
}

#[cfg(not(any(unix, windows)))]
fn format_permissions(_metadata: &std::fs::Metadata) -> String {
    "rwxrwxrwx".to_string()
}

fn format_timestamp(timestamp: u64) -> String {
    const SECONDS_PER_DAY: u64 = 86400;
    const SECONDS_PER_HOUR: u64 = 3600;
    const SECONDS_PER_MINUTE: u64 = 60;

    let mut remaining = timestamp;

    let days = remaining / SECONDS_PER_DAY;
    remaining %= SECONDS_PER_DAY;

    let hours = remaining / SECONDS_PER_HOUR;
    remaining %= SECONDS_PER_HOUR;

    let minutes = remaining / SECONDS_PER_MINUTE;
    let seconds = remaining % SECONDS_PER_MINUTE;

    let (year, month, day) = days_to_date(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn days_to_date(days_since_epoch: u64) -> (u32, u32, u32) {
    let mut year = 1970u32;
    let mut remaining_days = days_since_epoch;

    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };

        if remaining_days < days_in_year as u64 {
            break;
        }

        remaining_days -= days_in_year as u64;
        year += 1;
    }

    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u32;
    for &days_in_month in &days_in_months {
        if remaining_days < days_in_month as u64 {
            break;
        }
        remaining_days -= days_in_month as u64;
        month += 1;
    }

    let day = (remaining_days + 1) as u32;

    (year, month, day)
}

fn is_leap_year(year: u32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

#[derive(Debug, Clone, Copy)]
enum OutputMode {
    Normal,
    Api,
}

impl OutputMode {
    fn from_args(args: &Args) -> Self {
        if args.api { Self::Api } else { Self::Normal }
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
            threshold: args.fuzzy_threshold,
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
                    if !word.is_empty() {
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

fn can_access_path(path: &Path) -> bool {
    std::fs::metadata(path).is_ok()
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

    fn scan_directory(&self, root: &Path, tx: Sender<FileInfo>) {
        let mut walker = WalkDir::new(root).follow_links(false);

        if let Some(depth) = self.config.max_depth {
            walker = walker.max_depth(depth);
        }

        for entry_result in walker
            .into_iter()
            .filter_entry(|e| can_access_path(e.path()))
        {
            if !self.limits.should_continue(&self.metrics) {
                break;
            }

            let entry = match entry_result {
                Ok(e) => e,
                Err(_) => {
                    self.metrics.increment_errors();
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

                    if let Ok(file_info) = FileInfo::from_path(path) {
                        let _ = tx.send(file_info);
                    }
                }
            }
        }
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
        R: Iterator<Item = FileInfo>,
    {
        if matches!(self.output_mode, OutputMode::Api) {
            println!("{}", FileInfo::csv_header());
        }

        for file_info in rx {
            match self.output_mode {
                OutputMode::Normal => println!("{}", file_info.path),
                OutputMode::Api => println!("{}", file_info.to_csv()),
            }
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

    fn search(&self, roots: Vec<PathBuf>) -> Result<SearchResult, String> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(self.config.threads)
            .build()
            .map_err(|e| format!("Failed to create thread pool: {}", e))?;

        let (tx, rx) = unbounded::<FileInfo>();

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

        Ok(SearchResult {
            found: self.scanner.metrics.get_found(),
            scanned: self.scanner.metrics.get_scanned(),
            errors: self.scanner.metrics.get_errors(),
        })
    }
}

struct SearchResult {
    found: u64,
    scanned: u64,
    errors: u64,
}

impl SearchResult {
    fn print_summary(&self, output_mode: &OutputMode, hide_errors: bool) {
        if matches!(output_mode, OutputMode::Api) {
            return;
        }

        eprintln!("\n{} found | {} scanned", self.found, self.scanned);

        if !hide_errors && self.errors > 0 {
            eprintln!("{} errors occurred", self.errors);
        }
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
        eprintln!("error: cannot use --files-only and --dirs-only together");
        std::process::exit(1);
    }

    let roots = match RootPathProvider::get_search_roots(args.path.clone()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {}", e);
            std::process::exit(1);
        }
    };

    let hide_errors = args.hide_errors;
    let output_mode = OutputMode::from_args(&args);
    let engine = SearchEngine::new(&args);

    let result = match engine.search(roots) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("fatal error: {}", e);
            std::process::exit(1);
        }
    };

    result.print_summary(&output_mode, hide_errors);
}
