use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::MetadataExt;

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::permanent;
use crate::platform::MountTable;
use crate::ui;

use super::super::locking::{LockMode, TrashLock};
use super::super::root;

pub fn clean_all(config: &Config, mounts: &MountTable, force: bool) -> Result<()> {
    let cwd = std::env::current_dir().map_err(|_| {
        AppError::operation("rm: cannot clean trash: failed to determine current directory")
    })?;
    let mountpoint = mounts.mountpoint_for(&cwd).map_err(|_| {
        AppError::operation(format!(
            "rm: cannot clean trash: failed to determine mountpoint for '{}'",
            cwd.display()
        ))
    })?;
    let Some(root) = root::existing(config, &mountpoint)? else {
        return Ok(());
    };
    let _lock = TrashLock::acquire(&root.path, LockMode::Exclusive)?;
    let root_text = root.path.display().to_string();
    ui::confirm_cleanup(
        force,
        &format!("WARNING: type '{root_text}' to permanently empty it: "),
        &root_text,
    )?;

    let mut deletion_failed = false;
    loop {
        let mut targets = Vec::with_capacity(permanent::DELETE_BATCH_SIZE);
        let entries = fs::read_dir(&root.path).map_err(|error| {
            AppError::operation(format!(
                "rm: cannot scan trash at '{}': {error}",
                root.path.display()
            ))
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                AppError::operation(format!(
                    "rm: cannot scan trash at '{}': {error}",
                    root.path.display()
                ))
            })?;
            if matches!(entry.file_name().as_os_str(), value if value == OsStr::new(".trash.lock") || value == OsStr::new(".trash-root"))
            {
                continue;
            }
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                AppError::operation(format!("rm: cannot inspect '{}': {error}", path.display()))
            })?;
            if metadata.dev() != root.device || mounts.is_known_mountpoint(&path) {
                deletion_failed = true;
                continue;
            }
            targets.push(path);
            if targets.len() == permanent::DELETE_BATCH_SIZE {
                break;
            }
        }
        if targets.is_empty() {
            break;
        }
        if !permanent::remove_paths(&targets) {
            deletion_failed = true;
            break;
        }
    }

    if deletion_failed {
        return Err(AppError::operation(format!(
            "rm: cannot clean trash at '{}': deletion failed",
            root.path.display()
        )));
    }
    Ok(())
}
