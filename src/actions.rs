//! File operations: create file/directory, rename, delete.
//!
//! Each operation works on the filesystem directly and returns an
//! `io::Result`. The caller (the Neovim command handler in `lib.rs`)
//! is responsible for refreshing the buffer after an operation.

use std::fs;
use std::path::Path;

/// Create a new file at the given path. Parent directories are created
/// as needed.
pub fn create_file(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::File::create(path)?;
    Ok(())
}

/// Create a new directory at the given path. Parent directories are
/// created as needed.
pub fn create_dir(path: &Path) -> std::io::Result<()> {
    fs::create_dir_all(path)
}

/// Rename (move) a file or directory from `from` to `to`.
pub fn rename(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::rename(from, to)
}

/// Delete a file.
pub fn delete_file(path: &Path) -> std::io::Result<()> {
    fs::remove_file(path)
}

/// Delete a directory and all of its contents.
pub fn delete_dir(path: &Path) -> std::io::Result<()> {
    fs::remove_dir_all(path)
}

/// Delete whatever is at the path (file or directory).
pub fn delete(path: &Path) -> std::io::Result<()> {
    if path.is_dir() {
        delete_dir(path)
    } else {
        delete_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn create_and_delete_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("new_file.txt");

        create_file(&file_path).unwrap();
        assert!(file_path.exists());

        delete_file(&file_path).unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn create_file_with_nested_parents() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("a/b/c/deep_file.txt");

        create_file(&file_path).unwrap();
        assert!(file_path.exists());
    }

    #[test]
    fn create_and_delete_dir() {
        let tmp = TempDir::new().unwrap();
        let dir_path = tmp.path().join("new_dir");

        create_dir(&dir_path).unwrap();
        assert!(dir_path.is_dir());

        delete_dir(&dir_path).unwrap();
        assert!(!dir_path.exists());
    }

    #[test]
    fn create_nested_dir() {
        let tmp = TempDir::new().unwrap();
        let dir_path = tmp.path().join("a/b/c");

        create_dir(&dir_path).unwrap();
        assert!(dir_path.is_dir());
    }

    #[test]
    fn rename_file() {
        let tmp = TempDir::new().unwrap();
        let old = tmp.path().join("old.txt");
        let new = tmp.path().join("new.txt");

        create_file(&old).unwrap();
        rename(&old, &new).unwrap();

        assert!(!old.exists());
        assert!(new.exists());
    }

    #[test]
    fn rename_dir() {
        let tmp = TempDir::new().unwrap();
        let old = tmp.path().join("old_dir");
        let new = tmp.path().join("new_dir");

        create_dir(&old).unwrap();
        // Put a file inside to verify contents are preserved.
        create_file(&old.join("inner.txt")).unwrap();

        rename(&old, &new).unwrap();

        assert!(!old.exists());
        assert!(new.is_dir());
        assert!(new.join("inner.txt").exists());
    }

    #[test]
    fn delete_dispatches_file() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("target.txt");
        create_file(&file_path).unwrap();

        delete(&file_path).unwrap();
        assert!(!file_path.exists());
    }

    #[test]
    fn delete_dispatches_dir() {
        let tmp = TempDir::new().unwrap();
        let dir_path = tmp.path().join("target_dir");
        create_dir(&dir_path).unwrap();
        create_file(&dir_path.join("child.txt")).unwrap();

        delete(&dir_path).unwrap();
        assert!(!dir_path.exists());
    }

    #[test]
    fn delete_nonexistent_file_errors() {
        let tmp = TempDir::new().unwrap();
        let result = delete_file(&tmp.path().join("ghost.txt"));
        assert!(result.is_err());
    }
}
