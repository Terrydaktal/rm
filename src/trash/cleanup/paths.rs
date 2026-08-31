use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{AppError, Result};
use crate::permanent;
use crate::platform::MountTable;
use crate::ui;

use super::super::locking::{LockMode, TrashLock};
use super::super::root;

pub fn clean_paths(
    config: &Config,
    mounts: &MountTable,
    force: bool,
    requested_paths: &[OsString],
) -> Result<()> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    let mut batch_mountpoint: Option<PathBuf> = None;
    let mut trusted_root = None;

    for requested in requested_paths {
        let path = trim_trailing_slashes(requested);
        let path = Path::new(&path);
        let basename = raw_basename(path.as_os_str());
        if basename.is_empty() || matches!(basename.as_bytes(), b"." | b"..") {
            return Err(AppError::operation(format!(
                "rm: refusing invalid cleanup path '{}'",
                Path::new(requested).display()
            )));
        }
        let parent = path
            .parent()
            .filter(|value| !value.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = fs::canonicalize(parent).map_err(|_| {
            AppError::operation(format!(
                "rm: cannot clean '{}': parent directory does not exist",
                Path::new(requested).display()
            ))
        })?;
        let target = parent.join(&basename);
        let mountpoint = mounts.mountpoint_for(&parent).map_err(|_| {
            AppError::operation(format!(
                "rm: cannot clean '{}': failed to determine mountpoint",
                Path::new(requested).display()
            ))
        })?;
        if batch_mountpoint
            .as_ref()
            .is_some_and(|existing| *existing != mountpoint)
        {
            return Err(AppError::operation(
                "rm: refusing --clean paths from multiple trash roots",
            ));
        }
        batch_mountpoint = Some(mountpoint.clone());

        let root = root::require_existing(config, &mountpoint)?;
        let canonical_root = fs::canonicalize(&root.path).map_err(|_| {
            AppError::operation(format!(
                "rm: cannot resolve trash root '{}'",
                root.path.display()
            ))
        })?;
        if target == canonical_root {
            return Err(AppError::operation(format!(
                "rm: refusing to clean the trash root itself '{}'",
                target.display()
            )));
        }
        if target == canonical_root.join(".trash.lock")
            || target == canonical_root.join(".trash-root")
        {
            return Err(AppError::operation(format!(
                "rm: refusing to clean protected trash control file '{}'",
                target.display()
            )));
        }
        if !target.starts_with(&canonical_root) {
            return Err(AppError::operation(format!(
                "rm: refusing cleanup path outside trusted trash root '{}'",
                target.display()
            )));
        }

        match fs::symlink_metadata(&target) {
            Ok(_) => {
                let target_mountpoint = mounts.mountpoint_for(&target).map_err(|_| {
                    AppError::operation(format!(
                        "rm: cannot clean '{}': failed to determine target mountpoint",
                        target.display()
                    ))
                })?;
                if target_mountpoint != mountpoint || mounts.is_mountpoint(&target).unwrap_or(false)
                {
                    return Err(AppError::operation(format!(
                        "rm: refusing to clean nested filesystem target '{}'",
                        target.display()
                    )));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound && force => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(AppError::operation(format!(
                    "rm: cannot clean '{}': No such file or directory",
                    target.display()
                )));
            }
            Err(error) => {
                return Err(AppError::operation(format!(
                    "rm: cannot inspect cleanup target '{}': {error}",
                    target.display()
                )));
            }
        }
        if seen.insert(target.clone()) {
            targets.push(target);
        }
        trusted_root = Some(root);
    }

    let root = trusted_root.ok_or_else(|| {
        AppError::operation("rm: internal error: cleanup path root was not retained")
    })?;
    let _lock = TrashLock::acquire(&root.path, LockMode::Exclusive)?;
    let (prompt, expected) = if targets.len() == 1 {
        let expected = targets[0].display().to_string();
        (
            format!("WARNING: type '{expected}' to permanently delete it: "),
            expected,
        )
    } else {
        let expected = format!("DELETE {} PATHS", targets.len());
        (
            format!(
                "WARNING: type '{expected}' to permanently delete {} paths: ",
                targets.len()
            ),
            expected,
        )
    };
    ui::confirm_cleanup(force, &prompt, &expected)?;
    if !permanent::remove_paths(&targets) {
        return Err(AppError::operation(
            "rm: cannot clean one or more trash paths: deletion failed",
        ));
    }
    Ok(())
}

fn trim_trailing_slashes(value: &OsStr) -> OsString {
    let mut bytes = value.as_bytes();
    while bytes.len() > 1 && bytes.ends_with(b"/") {
        bytes = &bytes[..bytes.len() - 1];
    }
    OsString::from_vec(bytes.to_vec())
}

fn raw_basename(path: &OsStr) -> OsString {
    let bytes = path.as_bytes();
    OsString::from_vec(
        bytes
            .rsplit(|byte| *byte == b'/')
            .next()
            .unwrap_or_default()
            .to_vec(),
    )
}
