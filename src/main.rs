use clap::Parser;
use omega::{cli::Args, engine::SearchEngine, paths::RootPathProvider};

fn main() {
    let args = Args::parse();

    if args.files_only && args.dirs_only {
        eprintln!("error: cannot use --files-only and --dirs-only together");
        std::process::exit(1);
    }

    let roots = match RootPathProvider::get_search_roots(&args.path) {
        Ok(roots) => roots,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    };

    let hide_errors = args.hide_errors;
    let engine = SearchEngine::new(&args);

    let result = match engine.search(&roots) {
        Ok(result) => result,
        Err(error) => {
            eprintln!("fatal error: {error}");
            std::process::exit(1);
        }
    };

    result.print_summary(hide_errors);
}
