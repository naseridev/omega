use std::path::PathBuf;

pub struct RootPathProvider;

impl RootPathProvider {
    pub fn get_search_roots(custom_paths: Vec<PathBuf>) -> Result<Vec<PathBuf>, String> {
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
