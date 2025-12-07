use crate::file_info::FileInfo;

#[derive(Debug, Clone, Copy)]
pub enum OutputMode {
    Normal,
    Api,
}

impl OutputMode {
    pub fn from_args(args: &crate::cli::Args) -> Self {
        if args.api { Self::Api } else { Self::Normal }
    }
}

pub struct ResultPrinter {
    output_mode: OutputMode,
}

impl ResultPrinter {
    pub fn new(output_mode: OutputMode) -> Self {
        Self { output_mode }
    }

    pub fn run<R>(&self, rx: R)
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

pub struct SearchResult {
    pub found: u64,
    pub scanned: u64,
    pub errors: u64,
    output_mode: OutputMode,
}

impl SearchResult {
    pub fn new(found: u64, scanned: u64, errors: u64, output_mode: OutputMode) -> Self {
        Self {
            found,
            scanned,
            errors,
            output_mode,
        }
    }

    pub fn print_summary(&self, hide_errors: bool) {
        if matches!(self.output_mode, OutputMode::Api) {
            return;
        }

        eprintln!("\n{} found | {} scanned", self.found, self.scanned);

        if !hide_errors && self.errors > 0 {
            eprintln!("{} errors occurred", self.errors);
        }
    }
}
