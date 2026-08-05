use crate::cli::Args;
use crate::metrics::SearchMetrics;
use crate::utils::{
    is_hidden, modified_seconds, path_bytes, write_csv_field, write_permissions, write_size,
    write_timestamp,
};
use std::fs::Metadata;
use std::io::{BufWriter, IsTerminal, Write};
use std::path::Path;
use std::sync::mpsc::Receiver;

const OUTPUT_BUFFER: usize = 256 * 1024;

const CSV_HEADER: &[u8] =
    b"path,name,is_dir,is_file,size,size_human,modified,modified_human,is_hidden,extension,permissions\n";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Normal,
    Api,
}

impl OutputMode {
    pub fn from_args(args: &Args) -> Self {
        if args.api { Self::Api } else { Self::Normal }
    }
}

pub fn write_path_line(out: &mut Vec<u8>, path: &Path) {
    out.extend_from_slice(&path_bytes(path));
    out.push(b'\n');
}

pub fn write_csv_record(out: &mut Vec<u8>, path: &Path, name: &str, metadata: &Metadata) {
    let is_file = metadata.is_file();
    let size = if is_file { metadata.len() } else { 0 };
    let modified = modified_seconds(metadata);

    write_csv_field(out, &path_bytes(path));
    out.push(b',');
    write_csv_field(out, name.as_bytes());
    out.push(b',');
    write_bool(out, metadata.is_dir());
    out.push(b',');
    write_bool(out, is_file);
    out.push(b',');
    let _ = write!(out, "{size}");
    out.push(b',');
    write_size(out, size);
    out.push(b',');
    let _ = write!(out, "{modified}");
    out.push(b',');
    write_timestamp(out, modified);
    out.push(b',');
    write_bool(out, is_hidden(name, metadata));
    out.push(b',');
    write_csv_field(
        out,
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .as_bytes(),
    );
    out.push(b',');
    write_permissions(out, metadata);
    out.push(b'\n');
}

fn write_bool(out: &mut Vec<u8>, value: bool) {
    out.extend_from_slice(if value { b"true" } else { b"false" });
}

pub struct ResultPrinter {
    output_mode: OutputMode,
}

impl ResultPrinter {
    pub fn new(output_mode: OutputMode) -> Self {
        Self { output_mode }
    }

    pub fn run(&self, batches: Receiver<Vec<u8>>, metrics: &SearchMetrics) {
        let stdout = std::io::stdout();
        let interactive = stdout.is_terminal();
        let mut writer = BufWriter::with_capacity(OUTPUT_BUFFER, stdout.lock());
        let mut writable = true;

        if self.output_mode == OutputMode::Api {
            writable = writer.write_all(CSV_HEADER).is_ok();
        }

        for batch in batches {
            if !writable {
                continue;
            }

            writable = writer.write_all(&batch).is_ok() && (!interactive || writer.flush().is_ok());

            if !writable {
                metrics.trigger_shutdown();
            }
        }

        if writable {
            let _ = writer.flush();
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
        if self.output_mode == OutputMode::Api {
            return;
        }

        eprintln!("\n{} found | {} scanned", self.found, self.scanned);

        if !hide_errors && self.errors > 0 {
            eprintln!("{} errors occurred", self.errors);
        }
    }
}
