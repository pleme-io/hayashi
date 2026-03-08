//! Hayashi (林) — directory-as-buffer file explorer for Neovim.
//!
//! Part of the blnvim-ng distribution — a Rust-native Neovim plugin suite.
//! Built with [`nvim-oxi`](https://github.com/noib3/nvim-oxi) for zero-cost
//! Neovim API bindings.

pub mod actions;
pub mod explorer;
pub mod render;

use nvim_oxi as oxi;
use nvim_oxi::api;
use nvim_oxi::api::opts::OptionOpts;
use nvim_oxi::api::types::CommandArgs;
use std::path::PathBuf;
use tane::prelude::*;

/// Convert a `tane::Error` into an `oxi::Error`.
fn tane_err(e: tane::Error) -> oxi::Error {
    oxi::Error::Api(api::Error::Other(e.to_string()))
}

/// Convert an `io::Error` into an `oxi::Error`.
fn io_err(prefix: &str, e: std::io::Error) -> oxi::Error {
    oxi::Error::Api(api::Error::Other(format!("{prefix}: {e}")))
}

/// Open the explorer buffer for the given directory.
///
/// Creates a new scratch buffer, populates it with the directory listing,
/// and sets up buffer-local keymaps for navigation and file operations.
fn open_explorer(dir: &std::path::Path) -> oxi::Result<()> {
    let canonical = dir
        .canonicalize()
        .map_err(|e| io_err("hayashi: cannot resolve path", e))?;

    let entries = explorer::read_dir(&canonical)
        .map_err(|e| io_err("hayashi: cannot read directory", e))?;

    let show_parent = canonical.parent().is_some() && canonical.as_os_str() != "/";
    let lines = render::render_lines(&entries, show_parent);

    // Create a scratch buffer.
    let mut buf = api::create_buf(true, true)?;

    // Set buffer content.
    let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    buf.set_lines(0..line_refs.len(), true, line_refs.iter().copied())?;

    // Set buffer name to the directory path.
    let buf_name = format!("hayashi://{}", canonical.display());
    buf.set_name(&buf_name)?;

    // Open the buffer in the current window.
    api::set_current_buf(&buf)?;

    // Buffer options.
    let buf_scope = OptionOpts::builder().buffer(buf.clone()).build();
    api::set_option_value("buftype", "nofile", &buf_scope)?;
    api::set_option_value("bufhidden", "wipe", &buf_scope)?;
    api::set_option_value("modifiable", false, &buf_scope)?;
    api::set_option_value("swapfile", false, &buf_scope)?;

    // Store the directory path in a buffer variable for keymap callbacks.
    buf.set_var(
        "hayashi_dir",
        oxi::String::from(canonical.to_string_lossy().as_ref()),
    )?;

    // Buffer-local keymaps via Neovim command (avoids nvim-oxi builder issues).
    api::command(&format!(
        "nnoremap <buffer={}> <silent> <CR> <Cmd>HayashiOpen<CR>",
        buf.handle()
    ))?;
    api::command(&format!(
        "nnoremap <buffer={}> <silent> - <Cmd>HayashiUp<CR>",
        buf.handle()
    ))?;

    Ok(())
}

/// Get the current hayashi directory from the current buffer's variable.
fn current_dir() -> oxi::Result<PathBuf> {
    let buf = api::get_current_buf();
    let dir_str: oxi::String = buf.get_var("hayashi_dir")?;
    Ok(PathBuf::from(dir_str.to_string_lossy().to_string()))
}

/// Get the entry name under the cursor.
fn entry_under_cursor() -> oxi::Result<Option<String>> {
    let win = api::get_current_win();
    let (row, _col) = win.get_cursor()?;
    let buf = api::get_current_buf();
    let lines: Vec<oxi::String> = buf.get_lines(row - 1..row, true)?.collect();
    let Some(line) = lines.first() else {
        return Ok(None);
    };
    let line_str = line.to_string_lossy();
    Ok(explorer::entry_name_from_line(&line_str).map(String::from))
}

#[oxi::plugin]
fn hayashi() -> oxi::Result<()> {
    // :Hayashi [path] — open the explorer at path (default: cwd).
    UserCommand::new("Hayashi")
        .desc("Open Hayashi file explorer")
        .optional_arg()
        .register(|args: CommandArgs| {
            let dir = match &args.args {
                Some(arg) if !arg.is_empty() => PathBuf::from(arg),
                _ => std::env::current_dir()
                    .map_err(|e| tane::Error::Custom(e.to_string()))?,
            };
            open_explorer(&dir).map_err(|e| tane::Error::Custom(e.to_string()))?;
            Ok(())
        })
        .map_err(tane_err)?;

    // :HayashiOpen — open file/dir under cursor (Enter key).
    UserCommand::new("HayashiOpen")
        .desc("Open file or directory under cursor")
        .register(|_args: CommandArgs| {
            let dir = current_dir().map_err(|e| tane::Error::Custom(e.to_string()))?;
            let Some(name) =
                entry_under_cursor().map_err(|e| tane::Error::Custom(e.to_string()))?
            else {
                return Ok(());
            };

            if name == ".." {
                if let Some(parent) = explorer::parent_dir(&dir) {
                    open_explorer(&parent)
                        .map_err(|e| tane::Error::Custom(e.to_string()))?;
                }
                return Ok(());
            }

            let target = dir.join(&name);
            if target.is_dir() {
                open_explorer(&target).map_err(|e| tane::Error::Custom(e.to_string()))?;
            } else {
                // Open the file in the current window.
                let cmd = format!("edit {}", target.display());
                api::command(&cmd).map_err(|e| tane::Error::Oxi(oxi::Error::from(e)))?;
            }
            Ok(())
        })
        .map_err(tane_err)?;

    // :HayashiUp — navigate to parent directory (- key).
    UserCommand::new("HayashiUp")
        .desc("Navigate to parent directory")
        .register(|_args: CommandArgs| {
            let dir = current_dir().map_err(|e| tane::Error::Custom(e.to_string()))?;
            if let Some(parent) = explorer::parent_dir(&dir) {
                open_explorer(&parent).map_err(|e| tane::Error::Custom(e.to_string()))?;
            }
            Ok(())
        })
        .map_err(tane_err)?;

    // :HayashiCreate <name> — create a file (or dir if name ends with /).
    UserCommand::new("HayashiCreate")
        .desc("Create a file or directory (append / for dir)")
        .one_arg()
        .register(|args: CommandArgs| {
            let dir = current_dir().map_err(|e| tane::Error::Custom(e.to_string()))?;
            let name = args.args.unwrap_or_default();
            let target = dir.join(&name);

            let result = if name.ends_with('/') {
                actions::create_dir(&target)
            } else {
                actions::create_file(&target)
            };

            result.map_err(|e| {
                tane::Error::Custom(format!("hayashi: create failed: {e}"))
            })?;

            // Refresh the explorer.
            open_explorer(&dir).map_err(|e| tane::Error::Custom(e.to_string()))?;
            Ok(())
        })
        .map_err(tane_err)?;

    // :HayashiRename <new_name> — rename the entry under cursor.
    UserCommand::new("HayashiRename")
        .desc("Rename the entry under cursor")
        .one_arg()
        .register(|args: CommandArgs| {
            let dir = current_dir().map_err(|e| tane::Error::Custom(e.to_string()))?;
            let Some(old_name) =
                entry_under_cursor().map_err(|e| tane::Error::Custom(e.to_string()))?
            else {
                return Ok(());
            };
            let new_name = args.args.unwrap_or_default();
            let old_path = dir.join(&old_name);
            let new_path = dir.join(&new_name);

            actions::rename(&old_path, &new_path).map_err(|e| {
                tane::Error::Custom(format!("hayashi: rename failed: {e}"))
            })?;

            open_explorer(&dir).map_err(|e| tane::Error::Custom(e.to_string()))?;
            Ok(())
        })
        .map_err(tane_err)?;

    // :HayashiDelete — delete the entry under cursor.
    UserCommand::new("HayashiDelete")
        .desc("Delete the entry under cursor")
        .register(|_args: CommandArgs| {
            let dir = current_dir().map_err(|e| tane::Error::Custom(e.to_string()))?;
            let Some(name) =
                entry_under_cursor().map_err(|e| tane::Error::Custom(e.to_string()))?
            else {
                return Ok(());
            };
            let target = dir.join(&name);

            actions::delete(&target).map_err(|e| {
                tane::Error::Custom(format!("hayashi: delete failed: {e}"))
            })?;

            open_explorer(&dir).map_err(|e| tane::Error::Custom(e.to_string()))?;
            Ok(())
        })
        .map_err(tane_err)?;

    // Global keymap: - opens Hayashi in cwd.
    Keymap::normal("-", "<Cmd>Hayashi<CR>")
        .desc("Open Hayashi file explorer")
        .register()
        .map_err(tane_err)?;

    Ok(())
}
