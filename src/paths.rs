use std::path::PathBuf;

pub struct RootPathProvider;

impl RootPathProvider {
    pub fn get_search_roots(custom_paths: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
        if !custom_paths.is_empty() {
            for path in custom_paths {
                if !path.exists() {
                    return Err(format!("path does not exist: {}", path.display()));
                }
            }

            return Ok(custom_paths.to_vec());
        }

        let roots = default_roots();

        if roots.is_empty() {
            return Err("no valid search paths found".to_string());
        }

        Ok(roots)
    }
}

#[cfg(windows)]
fn default_roots() -> Vec<PathBuf> {
    (b'C'..=b'Z')
        .map(|drive| PathBuf::from(format!("{}:\\", drive as char)))
        .filter(|path| path.exists())
        .collect()
}

#[cfg(not(windows))]
fn default_roots() -> Vec<PathBuf> {
    vec![PathBuf::from("/")]
}
