use crate::utils::{escape_csv, format_permissions, format_size, format_timestamp, is_hidden_file};
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub is_dir: bool,
    pub is_file: bool,
    pub size: u64,
    pub size_human: String,
    pub modified: u64,
    pub modified_human: String,
    pub is_hidden: bool,
    pub extension: String,
    pub permissions: String,
}

impl FileInfo {
    pub fn from_path(path: &Path) -> Result<Self, std::io::Error> {
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

    pub fn to_csv(&self) -> String {
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

    pub fn csv_header() -> &'static str {
        "path,name,is_dir,is_file,size,size_human,modified,modified_human,is_hidden,extension,permissions"
    }
}
