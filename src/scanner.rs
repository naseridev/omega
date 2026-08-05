use crate::cli::Args;
use crate::matcher::PatternMatcher;
use crate::metrics::{SearchLimits, SearchMetrics};
use crate::output::{OutputMode, write_csv_record, write_path_line};
use rayon::Scope;
use std::fs::{self, DirEntry, Metadata};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::mpsc::SyncSender;

const BATCH_CAPACITY: usize = 32 * 1024;
const STREAM_CAPACITY: usize = 512;
const PROGRESS_INTERVAL: u64 = 256;

pub struct SearchConfig {
    pub threads: usize,
    pub max_depth: usize,
    pub progress_interval: u64,
    pub files_only: bool,
    pub dirs_only: bool,
    pub resolve_symlinks: bool,
    pub stream: bool,
}

impl SearchConfig {
    pub fn from_args(args: &Args) -> Self {
        let threads = args.threads.filter(|count| *count > 0).unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(4, std::num::NonZero::get)
        });

        Self {
            threads,
            max_depth: args.max_depth.unwrap_or(usize::MAX),
            progress_interval: if args.limit_found.is_some() || args.limit_scanned.is_some() {
                1
            } else {
                PROGRESS_INTERVAL
            },
            files_only: args.files_only,
            dirs_only: args.dirs_only,
            resolve_symlinks: args.files_only || args.dirs_only || args.content_search,
            stream: std::io::stdout().is_terminal(),
        }
    }

    #[inline]
    pub fn should_include(&self, is_dir: bool) -> bool {
        if is_dir {
            !self.files_only
        } else {
            !self.dirs_only
        }
    }
}

enum MetadataSource<'entry> {
    Entry(&'entry DirEntry),
    Follow,
}

impl MetadataSource<'_> {
    fn resolve(&self, path: &Path) -> std::io::Result<Metadata> {
        match self {
            Self::Entry(entry) => entry.metadata(),
            Self::Follow => fs::metadata(path).or_else(|_| fs::symlink_metadata(path)),
        }
    }
}

struct Batch {
    buffer: Vec<u8>,
    sender: SyncSender<Vec<u8>>,
    stream: bool,
    connected: bool,
}

impl Batch {
    fn new(sender: SyncSender<Vec<u8>>, stream: bool) -> Self {
        Self {
            buffer: Vec::new(),
            sender,
            stream,
            connected: true,
        }
    }

    #[inline]
    fn writer(&mut self) -> &mut Vec<u8> {
        if self.buffer.capacity() == 0 {
            self.buffer.reserve(if self.stream {
                STREAM_CAPACITY
            } else {
                BATCH_CAPACITY
            });
        }

        &mut self.buffer
    }

    #[inline]
    fn settle(&mut self) {
        if self.stream || self.buffer.len() >= BATCH_CAPACITY {
            self.flush();
        }
    }

    fn flush(&mut self) {
        if !self.connected || self.buffer.is_empty() {
            return;
        }

        self.connected = self.sender.send(std::mem::take(&mut self.buffer)).is_ok();
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        self.flush();
    }
}

pub struct FileSystemScanner {
    pub matcher: PatternMatcher,
    pub limits: SearchLimits,
    pub metrics: SearchMetrics,
    pub config: SearchConfig,
    pub output_mode: OutputMode,
}

impl FileSystemScanner {
    pub fn new(
        matcher: PatternMatcher,
        limits: SearchLimits,
        metrics: SearchMetrics,
        config: SearchConfig,
        output_mode: OutputMode,
    ) -> Self {
        Self {
            matcher,
            limits,
            metrics,
            config,
            output_mode,
        }
    }

    pub fn run(&self, roots: &[PathBuf], sender: SyncSender<Vec<u8>>) {
        rayon::scope(|scope| {
            for root in roots {
                if !self.limits.should_continue(&self.metrics) {
                    break;
                }

                let root = root.clone();
                let sender = sender.clone();
                scope.spawn(move |scope| self.scan_root(root, scope, sender));
            }
        });
    }

    fn scan_root<'scope>(
        &'scope self,
        root: PathBuf,
        scope: &Scope<'scope>,
        sender: SyncSender<Vec<u8>>,
    ) {
        let mut batch = Batch::new(sender, self.config.stream);

        self.metrics.add_scanned(1);

        let Ok(metadata) = fs::metadata(&root) else {
            return;
        };

        let is_dir = metadata.is_dir();

        if let Some(name) = root.file_name().and_then(|name| name.to_str()) {
            self.consider(&root, name, is_dir, MetadataSource::Follow, &mut batch);
        }

        if is_dir && self.config.max_depth > 0 {
            self.walk(root, 1, scope, &mut batch);
        }
    }

    fn walk<'scope>(
        &'scope self,
        start: PathBuf,
        start_depth: usize,
        scope: &Scope<'scope>,
        batch: &mut Batch,
    ) {
        let mut directory = start;
        let mut depth = start_depth;
        let mut subdirectories = Vec::new();

        loop {
            if !self.limits.should_continue(&self.metrics) {
                return;
            }

            subdirectories.clear();
            self.scan_directory(&directory, depth, &mut subdirectories, batch);

            let Some(next) = subdirectories.pop() else {
                return;
            };

            for subdirectory in subdirectories.drain(..) {
                let sender = batch.sender.clone();
                let stream = self.config.stream;
                let child_depth = depth + 1;

                scope.spawn(move |scope| {
                    let mut batch = Batch::new(sender, stream);
                    self.walk(subdirectory, child_depth, scope, &mut batch);
                });
            }

            directory = next;
            depth += 1;
        }
    }

    fn scan_directory(
        &self,
        directory: &Path,
        depth: usize,
        subdirectories: &mut Vec<PathBuf>,
        batch: &mut Batch,
    ) {
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) => {
                self.metrics.record_error(directory, &error);
                return;
            }
        };

        let descend = depth < self.config.max_depth;
        let mut child = directory.to_path_buf();
        let mut extended = false;
        let mut scanned = 0;

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    self.metrics.record_error(directory, &error);
                    continue;
                }
            };

            scanned += 1;
            self.visit(
                &entry,
                &mut child,
                &mut extended,
                descend,
                subdirectories,
                batch,
            );

            if scanned == self.config.progress_interval {
                self.metrics.add_scanned(scanned);
                scanned = 0;

                if !self.limits.should_continue(&self.metrics) {
                    break;
                }
            }
        }

        self.metrics.add_scanned(scanned);
    }

    fn visit(
        &self,
        entry: &DirEntry,
        child: &mut PathBuf,
        extended: &mut bool,
        descend: bool,
        subdirectories: &mut Vec<PathBuf>,
        batch: &mut Batch,
    ) {
        let name = entry.file_name();

        if *extended {
            child.set_file_name(&name);
        } else {
            child.push(&name);
            *extended = true;
        }

        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(error) => {
                self.metrics.record_error(child, &error);
                return;
            }
        };

        let is_directory = file_type.is_dir();

        if descend && is_directory {
            subdirectories.push(child.clone());
        }

        let is_dir = if file_type.is_symlink() && self.config.resolve_symlinks {
            fs::metadata(&*child).is_ok_and(|metadata| metadata.is_dir())
        } else {
            is_directory
        };

        if let Some(name) = name.to_str() {
            let source = if file_type.is_symlink() {
                MetadataSource::Follow
            } else {
                MetadataSource::Entry(entry)
            };

            self.consider(child, name, is_dir, source, batch);
        }
    }

    fn consider(
        &self,
        path: &Path,
        name: &str,
        is_dir: bool,
        source: MetadataSource<'_>,
        batch: &mut Batch,
    ) {
        if !self.config.should_include(is_dir) {
            return;
        }

        let matched = self.matcher.matches(name) || (!is_dir && self.matcher.matches_content(path));

        if !matched {
            return;
        }

        let metadata = match self.output_mode {
            OutputMode::Normal => None,
            OutputMode::Api => match source.resolve(path) {
                Ok(metadata) => Some(metadata),
                Err(_) => return,
            },
        };

        if let Some(limit) = self.limits.found_limit() {
            if !self.metrics.claim_found(limit) {
                self.metrics.trigger_shutdown();
                return;
            }
        } else {
            self.metrics.increment_found();
        }

        match metadata {
            None => write_path_line(batch.writer(), path),
            Some(metadata) => write_csv_record(batch.writer(), path, name, &metadata),
        }

        batch.settle();
    }
}
