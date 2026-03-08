//! Directory listing logic: read dir, sort, format entries.
//!
//! This module is pure Rust with no `nvim-oxi` dependency, making it
//! straightforward to unit test.

use std::fs;
use std::path::{Path, PathBuf};

/// A single entry in a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    /// The file/directory name (not the full path).
    pub name: String,
    /// Whether this entry is a directory.
    pub is_dir: bool,
    /// File size in bytes (0 for directories).
    pub size: u64,
    /// Unix permission bits (e.g., 0o755).
    pub mode: u32,
}

impl DirEntry {
    /// Create a new directory entry.
    #[must_use]
    pub fn new(name: String, is_dir: bool, size: u64, mode: u32) -> Self {
        Self {
            name,
            is_dir,
            size,
            mode,
        }
    }
}

impl Ord for DirEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Directories first, then alphabetical (case-insensitive).
        other
            .is_dir
            .cmp(&self.is_dir)
            .then_with(|| self.name.to_lowercase().cmp(&other.name.to_lowercase()))
    }
}

impl PartialOrd for DirEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// Read a directory and return sorted entries.
///
/// Directories are listed first, then files. Both groups are sorted
/// alphabetically (case-insensitive). Hidden files (starting with `.`)
/// are included.
pub fn read_dir(path: &Path) -> std::io::Result<Vec<DirEntry>> {
    let mut entries = Vec::new();

    for result in fs::read_dir(path)? {
        let entry = result?;
        let metadata = entry.metadata()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = metadata.is_dir();
        let size = if is_dir { 0 } else { metadata.len() };

        #[cfg(unix)]
        let mode = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode()
        };
        #[cfg(not(unix))]
        let mode = if metadata.permissions().readonly() {
            0o444
        } else {
            0o644
        };

        entries.push(DirEntry::new(name, is_dir, size, mode));
    }

    entries.sort();
    Ok(entries)
}

/// Format a file size as a human-readable string.
#[must_use]
pub fn format_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;
    const TIB: u64 = 1024 * GIB;

    if bytes >= TIB {
        format!("{:.1}T", bytes as f64 / TIB as f64)
    } else if bytes >= GIB {
        format!("{:.1}G", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1}M", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1}K", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes}B")
    }
}

/// Format Unix permission bits as an `rwxrwxrwx` string.
#[must_use]
pub fn format_permissions(mode: u32) -> String {
    let mut s = String::with_capacity(9);
    let triplets = [(mode >> 6) & 7, (mode >> 3) & 7, mode & 7];
    for triplet in triplets {
        s.push(if triplet & 4 != 0 { 'r' } else { '-' });
        s.push(if triplet & 2 != 0 { 'w' } else { '-' });
        s.push(if triplet & 1 != 0 { 'x' } else { '-' });
    }
    s
}

/// Resolve the parent directory for "go up" navigation.
///
/// Returns `None` if the path has no parent (i.e., it is the root).
#[must_use]
pub fn parent_dir(path: &Path) -> Option<PathBuf> {
    path.parent().map(Path::to_path_buf)
}

/// Extract the entry name from a formatted display line.
///
/// Display lines have the format: `icon  name  size  perms`
/// We extract the second whitespace-delimited token.
#[must_use]
pub fn entry_name_from_line(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Skip the icon (first token), then grab the name (second token).
    let mut parts = trimmed.splitn(3, "  ");
    let _icon = parts.next()?;
    let name = parts.next()?.trim();
    if name.is_empty() {
        None
    } else {
        // The name might end with '/' for directories — strip it for path ops.
        Some(name.trim_end_matches('/'))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use tempfile::TempDir;

    // ── format_size ──

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(1023), "1023B");
    }

    #[test]
    fn format_size_kib() {
        assert_eq!(format_size(1024), "1.0K");
        assert_eq!(format_size(1536), "1.5K");
        assert_eq!(format_size(10240), "10.0K");
    }

    #[test]
    fn format_size_mib() {
        assert_eq!(format_size(1_048_576), "1.0M");
        assert_eq!(format_size(5_242_880), "5.0M");
    }

    #[test]
    fn format_size_gib() {
        assert_eq!(format_size(1_073_741_824), "1.0G");
    }

    #[test]
    fn format_size_tib() {
        assert_eq!(format_size(1_099_511_627_776), "1.0T");
    }

    // ── format_permissions ──

    #[test]
    fn format_permissions_755() {
        assert_eq!(format_permissions(0o755), "rwxr-xr-x");
    }

    #[test]
    fn format_permissions_644() {
        assert_eq!(format_permissions(0o644), "rw-r--r--");
    }

    #[test]
    fn format_permissions_777() {
        assert_eq!(format_permissions(0o777), "rwxrwxrwx");
    }

    #[test]
    fn format_permissions_000() {
        assert_eq!(format_permissions(0o000), "---------");
    }

    #[test]
    fn format_permissions_600() {
        assert_eq!(format_permissions(0o600), "rw-------");
    }

    #[test]
    fn format_permissions_only_lower_bits() {
        // Mode bits from fs often include file type bits (e.g., 0o100644).
        // Our function only looks at the lower 9 bits.
        assert_eq!(format_permissions(0o100_644), "rw-r--r--");
    }

    // ── DirEntry sorting ──

    #[test]
    fn sort_dirs_before_files() {
        let mut entries = vec![
            DirEntry::new("file.txt".into(), false, 100, 0o644),
            DirEntry::new("src".into(), true, 0, 0o755),
        ];
        entries.sort();
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "src");
        assert_eq!(entries[1].name, "file.txt");
    }

    #[test]
    fn sort_alphabetical_within_dirs() {
        let mut entries = vec![
            DirEntry::new("zebra".into(), true, 0, 0o755),
            DirEntry::new("alpha".into(), true, 0, 0o755),
            DirEntry::new("middle".into(), true, 0, 0o755),
        ];
        entries.sort();
        assert_eq!(entries[0].name, "alpha");
        assert_eq!(entries[1].name, "middle");
        assert_eq!(entries[2].name, "zebra");
    }

    #[test]
    fn sort_alphabetical_within_files() {
        let mut entries = vec![
            DirEntry::new("main.rs".into(), false, 100, 0o644),
            DirEntry::new("Cargo.toml".into(), false, 50, 0o644),
            DirEntry::new("lib.rs".into(), false, 80, 0o644),
        ];
        entries.sort();
        assert_eq!(entries[0].name, "Cargo.toml");
        assert_eq!(entries[1].name, "lib.rs");
        assert_eq!(entries[2].name, "main.rs");
    }

    #[test]
    fn sort_case_insensitive() {
        let mut entries = vec![
            DirEntry::new("Zebra".into(), false, 0, 0o644),
            DirEntry::new("alpha".into(), false, 0, 0o644),
        ];
        entries.sort();
        assert_eq!(entries[0].name, "alpha");
        assert_eq!(entries[1].name, "Zebra");
    }

    #[test]
    fn sort_mixed_dirs_and_files() {
        let mut entries = vec![
            DirEntry::new("README.md".into(), false, 200, 0o644),
            DirEntry::new("src".into(), true, 0, 0o755),
            DirEntry::new("Cargo.toml".into(), false, 50, 0o644),
            DirEntry::new("tests".into(), true, 0, 0o755),
            DirEntry::new(".gitignore".into(), false, 10, 0o644),
        ];
        entries.sort();
        // Dirs first (alphabetical), then files (alphabetical).
        assert_eq!(entries[0].name, "src");
        assert_eq!(entries[1].name, "tests");
        assert_eq!(entries[2].name, ".gitignore");
        assert_eq!(entries[3].name, "Cargo.toml");
        assert_eq!(entries[4].name, "README.md");
    }

    // ── read_dir ──

    #[test]
    fn read_dir_with_files_and_dirs() {
        let tmp = TempDir::new().unwrap();
        File::create(tmp.path().join("file_b.txt")).unwrap();
        File::create(tmp.path().join("file_a.txt")).unwrap();
        fs::create_dir(tmp.path().join("dir_z")).unwrap();
        fs::create_dir(tmp.path().join("dir_a")).unwrap();

        let entries = read_dir(tmp.path()).unwrap();
        assert_eq!(entries.len(), 4);
        // Dirs first.
        assert!(entries[0].is_dir);
        assert_eq!(entries[0].name, "dir_a");
        assert!(entries[1].is_dir);
        assert_eq!(entries[1].name, "dir_z");
        // Files next.
        assert!(!entries[2].is_dir);
        assert_eq!(entries[2].name, "file_a.txt");
        assert!(!entries[3].is_dir);
        assert_eq!(entries[3].name, "file_b.txt");
    }

    #[test]
    fn read_dir_empty() {
        let tmp = TempDir::new().unwrap();
        let entries = read_dir(tmp.path()).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn read_dir_hidden_files_included() {
        let tmp = TempDir::new().unwrap();
        File::create(tmp.path().join(".hidden")).unwrap();
        File::create(tmp.path().join("visible")).unwrap();

        let entries = read_dir(tmp.path()).unwrap();
        assert_eq!(entries.len(), 2);
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&".hidden"));
        assert!(names.contains(&"visible"));
    }

    #[test]
    fn read_dir_nonexistent_returns_error() {
        let result = read_dir(Path::new("/nonexistent/path/xyz"));
        assert!(result.is_err());
    }

    // ── parent_dir ──

    #[test]
    fn parent_of_nested_path() {
        let p = PathBuf::from("/home/user/projects");
        assert_eq!(parent_dir(&p), Some(PathBuf::from("/home/user")));
    }

    #[test]
    fn parent_of_root() {
        let p = PathBuf::from("/");
        // On some platforms Path::parent of "/" is None, on others Some("").
        let parent = parent_dir(&p);
        // Either None or Some("") is acceptable — both mean "no meaningful parent".
        match parent {
            None => {} // expected on macOS
            Some(ref p) if p.as_os_str().is_empty() => {} // expected on some Linux
            other => panic!("unexpected parent of root: {other:?}"),
        }
    }

    // ── entry_name_from_line ──

    #[test]
    fn extract_name_file() {
        let line = "\u{f15b}  README.md  1.2K  rw-r--r--";
        assert_eq!(entry_name_from_line(line), Some("README.md"));
    }

    #[test]
    fn extract_name_directory() {
        let line = "\u{f07b}  src/  0B  rwxr-xr-x";
        assert_eq!(entry_name_from_line(line), Some("src"));
    }

    #[test]
    fn extract_name_empty_line() {
        assert_eq!(entry_name_from_line(""), None);
        assert_eq!(entry_name_from_line("   "), None);
    }

    #[test]
    fn extract_name_header_line() {
        // The "../" parent-nav line uses double-space separators too.
        let line = "\u{f07b}  ../";
        assert_eq!(entry_name_from_line(line), Some(".."));
    }
}
