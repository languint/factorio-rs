//! Write a file only when contents would change (avoids mtime churn / Bacon loops).

use std::path::Path;

use crate::error::{CliError, CliResult};

/// Write `contents` to `path`, creating parent dirs as needed.
///
/// Returns `true` if the file was created or its bytes changed.
pub fn write_if_changed(path: &Path, contents: &str) -> CliResult<bool> {
    if let Ok(existing) = std::fs::read_to_string(path)
        && existing == contents
    {
        return Ok(false);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| CliError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    std::fs::write(path, contents).map_err(|source| CliError::WriteFile {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn skips_identical_rewrite() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("a.txt");
        assert!(write_if_changed(&path, "hello\n").unwrap());
        let mtime1 = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert!(!write_if_changed(&path, "hello\n").unwrap());
        let mtime2 = std::fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(mtime1, mtime2);
        assert!(write_if_changed(&path, "world\n").unwrap());
    }
}
