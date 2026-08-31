use std::collections::HashSet;
use std::fs;
use std::os::unix::ffi::OsStrExt;

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::permanent;
use crate::platform::MountTable;
use crate::ui;

use super::super::locking::{LockMode, TrashLock};
use super::super::metadata;
use super::super::root;

pub fn clean_apps(
    config: &Config,
    mounts: &MountTable,
    force: bool,
    applications: &[String],
) -> Result<()> {
    let app_list = applications.join(" ");
    let requested = applications.iter().cloned().collect::<HashSet<_>>();
    let name_prefixes = applications
        .iter()
        .map(|application| format!("({application})").into_bytes())
        .collect::<Vec<_>>();
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

    let mut matches = Vec::new();
    let entries = fs::read_dir(&root.path).map_err(|error| {
        AppError::operation(format!(
            "rm: cannot clean trash entries for apps '{app_list}': scan failed: {error}"
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            AppError::operation(format!(
                "rm: cannot clean trash entries for apps '{app_list}': scan failed: {error}"
            ))
        })?;
        let file_type = entry.file_type().map_err(|error| {
            AppError::operation(format!(
                "rm: cannot clean trash entries for apps '{app_list}': scan failed: {error}"
            ))
        })?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        let name = entry.file_name();
        let name_match = name_prefixes
            .iter()
            .any(|prefix| name.as_bytes().starts_with(prefix));
        let metadata_match = if name_match {
            false
        } else {
            let metadata_path = path.join("metadata.json");
            fs::symlink_metadata(&metadata_path)
                .map(|value| value.is_file() && !value.file_type().is_symlink())
                .unwrap_or(false)
                && metadata::metadata_matches(&metadata_path, &requested)
        };
        if name_match || metadata_match {
            if mounts.is_known_mountpoint(&path) {
                return Err(AppError::operation(format!(
                    "rm: refusing to clean nested filesystem target '{}'",
                    path.display()
                )));
            }
            matches.push(path);
        }
    }

    if matches.is_empty() {
        return Ok(());
    }
    ui::confirm_cleanup(
        force,
        &format!(
            "WARNING: type '{app_list}' to permanently delete {} trash run(s): ",
            matches.len()
        ),
        &app_list,
    )?;
    if !permanent::remove_paths(&matches) {
        return Err(AppError::operation(format!(
            "rm: cannot clean trash entries for apps '{app_list}': deletion failed"
        )));
    }
    Ok(())
}
