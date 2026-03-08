//! Format directory entries with icons and metadata for buffer display.

use crate::explorer::{format_permissions, format_size, DirEntry};

/// Nerd Font icon for directories.
const DIR_ICON: &str = "\u{f07b}"; // nf-fa-folder
/// Nerd Font icon for the parent directory (`../`).
const PARENT_ICON: &str = "\u{f07b}"; // same folder icon

/// Format a single directory entry as a display line.
///
/// Format: `icon  name[/]  size  perms`
/// Directories get a trailing `/` on the name.
#[must_use]
pub fn format_entry(entry: &DirEntry) -> String {
    let icon = if entry.is_dir {
        DIR_ICON.to_string()
    } else {
        let (glyph, _color) = kamon::icon_and_color(&entry.name);
        glyph.to_string()
    };

    let display_name = if entry.is_dir {
        format!("{}/", entry.name)
    } else {
        entry.name.clone()
    };

    let size = if entry.is_dir {
        String::new()
    } else {
        format_size(entry.size)
    };

    let perms = format_permissions(entry.mode);

    format!("{icon}  {display_name}  {size}  {perms}")
}

/// Format the parent-directory navigation line (`../`).
#[must_use]
pub fn format_parent_line() -> String {
    format!("{PARENT_ICON}  ../")
}

/// Render an entire directory listing to display lines.
///
/// The first line is always `../` for navigating up (unless at root).
/// Remaining lines are the sorted entries.
#[must_use]
pub fn render_lines(entries: &[DirEntry], show_parent: bool) -> Vec<String> {
    let mut lines = Vec::with_capacity(entries.len() + 1);

    if show_parent {
        lines.push(format_parent_line());
    }

    for entry in entries {
        lines.push(format_entry(entry));
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entries() -> Vec<DirEntry> {
        vec![
            DirEntry::new("src".into(), true, 0, 0o755),
            DirEntry::new("main.rs".into(), false, 1024, 0o644),
            DirEntry::new("Cargo.toml".into(), false, 256, 0o644),
        ]
    }

    #[test]
    fn format_entry_directory() {
        let entry = DirEntry::new("src".into(), true, 0, 0o755);
        let line = format_entry(&entry);
        assert!(line.contains("src/"));
        assert!(line.contains("rwxr-xr-x"));
    }

    #[test]
    fn format_entry_file() {
        let entry = DirEntry::new("main.rs".into(), false, 2048, 0o644);
        let line = format_entry(&entry);
        assert!(line.contains("main.rs"));
        assert!(line.contains("2.0K"));
        assert!(line.contains("rw-r--r--"));
    }

    #[test]
    fn format_parent_line_content() {
        let line = format_parent_line();
        assert!(line.contains("../"));
    }

    #[test]
    fn render_lines_with_parent() {
        let entries = sample_entries();
        let lines = render_lines(&entries, true);
        assert_eq!(lines.len(), 4);
        assert!(lines[0].contains("../"));
    }

    #[test]
    fn render_lines_without_parent() {
        let entries = sample_entries();
        let lines = render_lines(&entries, false);
        assert_eq!(lines.len(), 3);
        assert!(!lines[0].contains("../"));
    }

    #[test]
    fn render_lines_empty() {
        let lines = render_lines(&[], true);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("../"));
    }
}
