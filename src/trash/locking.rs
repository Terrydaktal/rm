use std::fs::{self, File, OpenOptions};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use fs2::FileExt;

use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy)]
pub enum LockMode {
    Shared,
    Exclusive,
}

#[derive(Debug)]
pub struct TrashLock {
    _file: File,
}

impl TrashLock {
    pub fn acquire(root: &Path, mode: LockMode) -> Result<Self> {
        let path = root.join(".trash.lock");
        if fs::symlink_metadata(&path)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false)
        {
            return Err(AppError::operation(format!(
                "rm: refusing trash lock '{}': it is a symlink",
                path.display()
            )));
        }
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(&path)
            .map_err(|_| {
                AppError::operation(format!("rm: cannot open trash lock '{}'", path.display()))
            })?;
        let result = match mode {
            LockMode::Shared => FileExt::lock_shared(&file),
            LockMode::Exclusive => FileExt::lock_exclusive(&file),
        };
        result.map_err(|_| {
            let qualifier = match mode {
                LockMode::Shared => "",
                LockMode::Exclusive => "exclusive ",
            };
            AppError::operation(format!(
                "rm: cannot acquire {qualifier}trash lock '{}'",
                path.display()
            ))
        })?;
        Ok(Self { _file: file })
    }
}
