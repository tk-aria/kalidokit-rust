//! Plugin discovery.
//!
//! Finds plugin binaries by glob pattern in a directory.
//! Mirrors Go's `discover.go`.

use crate::error::PluginError;
use std::path::{Path, PathBuf};

/// Discover plugin binaries matching a glob pattern in a directory.
///
/// Returns sorted list of matching file paths.
///
/// # Example
/// ```no_run
/// let plugins = go_plugin::discover::discover("terraform-provider-*", "/usr/local/bin").unwrap();
/// ```
pub fn discover(glob_pattern: &str, dir: &str) -> Result<Vec<PathBuf>, PluginError> {
    let full_pattern = Path::new(dir).join(glob_pattern);
    let pattern_str = full_pattern.to_string_lossy();

    let mut results = Vec::new();
    for entry in glob::glob(&pattern_str)
        .map_err(|e| PluginError::Other(format!("invalid glob pattern: {e}")))?
    {
        match entry {
            Ok(path) => {
                // Only include files (not directories)
                if path.is_file() {
                    results.push(path);
                }
            }
            Err(e) => {
                log::warn!("Glob error for {:?}: {e}", pattern_str);
            }
        }
    }

    results.sort();
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn discover_matching_files() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("my-plugin-foo"), "").unwrap();
        fs::write(dir.path().join("my-plugin-bar"), "").unwrap();
        fs::write(dir.path().join("other-thing"), "").unwrap();

        let results = discover("my-plugin-*", dir.path().to_str().unwrap()).unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().any(|p| p.file_name().unwrap() == "my-plugin-bar"));
        assert!(results.iter().any(|p| p.file_name().unwrap() == "my-plugin-foo"));
    }

    #[test]
    fn discover_no_matches() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("unrelated"), "").unwrap();

        let results = discover("my-plugin-*", dir.path().to_str().unwrap()).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn discover_excludes_directories() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("my-plugin-dir")).unwrap();
        fs::write(dir.path().join("my-plugin-file"), "").unwrap();

        let results = discover("my-plugin-*", dir.path().to_str().unwrap()).unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].file_name().unwrap() == "my-plugin-file");
    }
}
