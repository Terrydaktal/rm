use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use chrono::Local;

use crate::cli::{Cli, InteractiveMode};
use crate::config::Config;
use crate::error::{AppError, Result};
use crate::permanent;
use crate::platform::{MountTable, canonical_location, rename_noreplace};
use crate::ui;

use super::metadata::Metadata;
use super::root;
use super::run::Run;

pub struct TrashSession<'a> {
    config: &'a Config,
    mounts: &'a MountTable,
    cli: &'a Cli,
    metadata: Metadata,
    run_name: String,
    runs: HashMap<PathBuf, Run>,
}

impl<'a> TrashSession<'a> {
    pub fn new(
        config: &'a Config,
        mounts: &'a MountTable,
        cli: &'a Cli,
        metadata: Metadata,
        run_name: String,
    ) -> Self {
        Self {
            config,
            mounts,
            cli,
            metadata,
            run_name,
            runs: HashMap::new(),
        }
    }

    pub fn trash_one(&mut self, source: &OsStr) -> Result<()> {
        let source = Path::new(source);
        let metadata = match fs::symlink_metadata(source) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if self.cli.force {
                    return Ok(());
                }
                return Err(AppError::operation(format!(
                    "rm: cannot remove '{}': No such file or directory",
                    source.display()
                )));
            }
            Err(error) => {
                return Err(AppError::operation(format!(
                    "rm: cannot inspect '{}': {error}",
                    source.display()
                )));
            }
        };

        let basename = source_basename(source.as_os_str());
        if matches!(basename.as_bytes(), b"." | b"..")
            || matches!(source.as_os_str().as_bytes(), b"." | b"..")
        {
            return Err(AppError::operation(format!(
                "rm: refusing to remove '{}'",
                source.display()
            )));
        }

        if self.cli.interactive == InteractiveMode::Always
            && !self.cli.force
            && !ui::confirm_target(&source.display().to_string())?
        {
            return Ok(());
        }

        let mountpoint = self.mounts.mountpoint_for(source).map_err(|_| {
            AppError::operation(format!(
                "rm: cannot remove '{}': failed to determine mountpoint",
                source.display()
            ))
        })?;
        if !metadata.file_type().is_symlink() && self.mounts.is_mountpoint(source).unwrap_or(false)
        {
            return Err(AppError::operation(format!(
                "rm: refusing to trash filesystem mountpoint '{}' (mounted at '{}')",
                source.display(),
                mountpoint.display()
            )));
        }
        if is_inside_root(self.config, source, &mountpoint) {
            let expected_root = root::root_path(self.config, &mountpoint);
            return Err(AppError::operation(format!(
                "rm: refusing to trash path inside the trash root '{}' (root '{}')",
                source.display(),
                expected_root.display()
            )));
        }
        if basename.is_empty() {
            return Err(AppError::operation(format!(
                "rm: cannot remove '{}': invalid empty basename",
                source.display()
            )));
        }

        if !self.runs.contains_key(&mountpoint) {
            let run = Run::create(self.config, &mountpoint, &self.run_name, &self.metadata)?;
            self.runs.insert(mountpoint.clone(), run);
        }
        let run = self
            .runs
            .get_mut(&mountpoint)
            .ok_or_else(|| AppError::operation("rm: internal error: trash run was not retained"))?;
        let mut destination = run.payload_dir.join(&basename);
        let mut moved = try_move(source, &destination)?;

        if !moved && !path_exists(source) {
            return Ok(());
        }
        if !moved && path_exists(&destination) {
            for attempt in 1..=100 {
                let suffix = Local::now().format("%Y%m%d-%H%M%S-%f");
                let mut collision_name = basename.clone();
                collision_name.push(format!("-{suffix}-{}-{attempt}", std::process::id()));
                destination = run.payload_dir.join(collision_name);
                moved = try_move(source, &destination)?;
                if moved {
                    break;
                }
                if !path_exists(source) {
                    return Ok(());
                }
            }
        }
        if !moved {
            return Err(AppError::operation(format!(
                "rm: cannot move '{}' to trash",
                source.display()
            )));
        }

        run.record(source, &destination)?;
        if self.cli.verbose {
            println!(
                "trashed '{}' -> '{}'",
                source.display(),
                destination.display()
            );
        }
        Ok(())
    }

    pub fn remove_empty_runs(&self) {
        let empty = self
            .runs
            .values()
            .filter(|run| run.is_empty())
            .map(|run| run.run_dir.clone())
            .collect::<Vec<_>>();
        if !empty.is_empty() {
            let _ = permanent::remove_paths(&empty);
        }
    }
}

fn is_inside_root(config: &Config, source: &Path, mountpoint: &Path) -> bool {
    let expected = root::root_path(config, mountpoint);
    let source = canonical_location(source).ok();
    let root = if expected.exists() {
        fs::canonicalize(&expected).ok()
    } else {
        expected
            .parent()
            .and_then(|parent| fs::canonicalize(parent).ok())
            .and_then(|parent| expected.file_name().map(|name| parent.join(name)))
    };
    matches!((source, root), (Some(source), Some(root)) if source == root || source.starts_with(&root))
}

fn source_basename(source: &OsStr) -> OsString {
    let mut bytes = source.as_bytes();
    while bytes.ends_with(b"/") {
        bytes = &bytes[..bytes.len() - 1];
    }
    let basename = bytes
        .rsplit(|byte| *byte == b'/')
        .next()
        .unwrap_or_default();
    OsString::from_vec(basename.to_vec())
}

fn path_exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}

fn try_move(source: &Path, destination: &Path) -> Result<bool> {
    try_move_with(source, destination, rename_noreplace)
}

fn try_move_with(
    source: &Path,
    destination: &Path,
    rename: impl FnOnce(&Path, &Path) -> io::Result<()>,
) -> Result<bool> {
    match rename(source, destination) {
        Ok(()) => return Ok(!path_exists(source)),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
        Err(error)
            if matches!(
                error.raw_os_error(),
                Some(libc::ENOSYS | libc::EINVAL | libc::EOPNOTSUPP)
            ) =>
        {
            if path_exists(destination) {
                return Ok(false);
            }
            match fs::rename(source, destination) {
                Ok(()) => return Ok(!path_exists(source)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => return Ok(false),
                Err(error) if error.raw_os_error() != Some(libc::EXDEV) => return Ok(false),
                Err(_) => {}
            }
        }
        Err(error) if error.raw_os_error() == Some(libc::EXDEV) => {}
        Err(_) => return Ok(false),
    }

    // Android's safe trash location may not share a device with emulated
    // storage. Preserve the Bash implementation's mv fallback for that case.
    let status = Command::new("mv")
        .args(["-nT", "--"])
        .arg(source)
        .arg(destination)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(status.is_ok_and(|status| status.success()) && !path_exists(source))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::io;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{source_basename, try_move_with};

    static NEXT_TEST: AtomicUsize = AtomicUsize::new(0);

    fn test_dir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "trash-move-test-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn basename_trims_all_trailing_slashes() {
        assert_eq!(source_basename(OsStr::new("dir///")), "dir");
        assert_eq!(source_basename(OsStr::new("/")), "");
    }

    #[test]
    fn unsupported_renameat2_falls_back_to_standard_rename() {
        let dir = test_dir();
        let source = dir.join("source");
        let destination = dir.join("destination");
        fs::write(&source, b"data").unwrap();
        assert!(
            try_move_with(&source, &destination, |_, _| {
                Err(io::Error::from_raw_os_error(libc::ENOSYS))
            })
            .unwrap()
        );
        assert_eq!(fs::read(&destination).unwrap(), b"data");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cross_device_error_uses_mv_fallback() {
        let dir = test_dir();
        let source = dir.join("source");
        let destination = dir.join("destination");
        fs::write(&source, b"data").unwrap();
        assert!(
            try_move_with(&source, &destination, |_, _| {
                Err(io::Error::from_raw_os_error(libc::EXDEV))
            })
            .unwrap()
        );
        assert!(!source.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"data");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_mv_fallback_leaves_source_in_place() {
        let dir = test_dir();
        let source = dir.join("source");
        let destination = dir.join("missing/destination");
        fs::write(&source, b"data").unwrap();
        assert!(
            !try_move_with(&source, &destination, |_, _| {
                Err(io::Error::from_raw_os_error(libc::EXDEV))
            })
            .unwrap()
        );
        assert!(source.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn collision_and_permission_failures_do_not_move_source() {
        for error_code in [libc::EEXIST, libc::EACCES] {
            let dir = test_dir();
            let source = dir.join("source");
            let destination = dir.join("destination");
            fs::write(&source, b"data").unwrap();
            assert!(
                !try_move_with(&source, &destination, |_, _| {
                    Err(io::Error::from_raw_os_error(error_code))
                })
                .unwrap()
            );
            assert!(source.exists());
            fs::remove_dir_all(dir).unwrap();
        }
    }
}
