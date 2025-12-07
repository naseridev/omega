use clap::Parser;
use std::path::PathBuf;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "omega")]
#[command(version = VERSION)]
#[command(about = "Blazing fast cross-platform file search", long_about = None)]
#[command(author = "Nima Naseri")]
pub struct Args {
    #[arg(required = true, help = "Search patterns")]
    pub patterns: Vec<String>,

    #[arg(short = 'p', long, help = "Search paths (can be used multiple times)")]
    pub path: Vec<PathBuf>,

    #[arg(short = 'l', long, help = "Limit number of results found")]
    pub limit_found: Option<u64>,

    #[arg(short = 's', long, help = "Limit number of items scanned")]
    pub limit_scanned: Option<u64>,

    #[arg(
        short = 't',
        long,
        help = "Number of threads (auto-detected if not specified)"
    )]
    pub threads: Option<usize>,

    #[arg(short = 'd', long, help = "Maximum search depth")]
    pub max_depth: Option<usize>,

    #[arg(short = 'i', long, help = "Case-insensitive search")]
    pub case_insensitive: bool,

    #[arg(short = 'f', long, help = "Search only files")]
    pub files_only: bool,

    #[arg(short = 'D', long, help = "Search only directories")]
    pub dirs_only: bool,

    #[arg(short = 'e', long, help = "Hide errors")]
    pub hide_errors: bool,

    #[arg(short = 'z', long, help = "Fuzzy search using Levenshtein distance")]
    pub fuzzy: bool,

    #[arg(
        short = 'T',
        long = "fuzzy-threshold",
        default_value = "2",
        help = "Fuzzy search distance threshold"
    )]
    pub fuzzy_threshold: usize,

    #[arg(long, help = "API mode - output as CSV format")]
    pub api: bool,

    #[arg(short = 'c', long, help = "Search inside file contents")]
    pub content_search: bool,

    #[arg(
        long,
        default_value = "10485760",
        help = "Maximum file size for content search (bytes, default 10MB)"
    )]
    pub max_content_size: u64,
}
