use crate::file_info::FileInfo;
use crate::matcher::PatternMatcher;
use crate::metrics::{SearchLimits, SearchMetrics};
use crossbeam::channel::Sender;
use std::path::Path;
use walkdir::WalkDir;

pub struct SearchConfig {
    pub threads: usize,
    pub max_depth: Option<usize>,
    pub files_only: bool,
    pub dirs_only: bool,
}

impl SearchConfig {
    pub fn from_args(args: &crate::cli::Args) -> Self {
        use std::thread;

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

    pub fn should_include(&self, is_dir: bool) -> bool {
        if self.files_only && is_dir {
            return false;
        }
        if self.dirs_only && !is_dir {
            return false;
        }
        true
    }
}

pub struct FileSystemScanner {
    pub matcher: PatternMatcher,
    pub limits: SearchLimits,
    pub metrics: SearchMetrics,
    pub config: SearchConfig,
}

impl FileSystemScanner {
    pub fn new(
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

    pub fn scan_directory(&self, root: &Path, tx: Sender<FileInfo>) {
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
                Err(e) => {
                    self.metrics
                        .log_error(&format!("Failed to read entry: {}", e));
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
                let name_matches = self.matcher.matches(name);
                let content_matches = if !name_matches && !is_dir {
                    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
                    self.matcher.matches_content(path, size)
                } else {
                    false
                };

                if name_matches || content_matches {
                    self.metrics.increment_found();

                    if let Ok(file_info) = FileInfo::from_path(path) {
                        let _ = tx.send(file_info);
                    }
                }
            }
        }
    }
}

fn can_access_path(path: &Path) -> bool {
    std::fs::metadata(path).is_ok()
}
