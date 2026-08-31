use std::env;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{AppError, Result};

#[derive(Debug, Clone)]
pub struct TrustedRoot {
    pub path: PathBuf,
    pub device: u64,
}

pub fn root_path(config: &Config, mountpoint: &Path) -> PathBuf {
    if config.is_android {
        if mountpoint.starts_with("/storage") || mountpoint.starts_with("/sdcard") {
            return PathBuf::from(format!("/sdcard/.{}", config.trash_subdir));
        }
        if mountpoint.starts_with("/data/local/tmp") || config.euid == 2000 {
            if Path::new("/data/local/tmp/termux-sudo").is_dir() {
                return Path::new("/data/local/tmp/termux-sudo").join(&config.trash_subdir);
            }
            return PathBuf::from(format!("/data/local/tmp/.{}", config.trash_subdir));
        }
        if let Some(home) = env::var_os("HOME").filter(|home| Path::new(home).is_dir()) {
            return Path::new(&home).join(format!(".{}", config.trash_subdir));
        }
    }

    if mountpoint == Path::new("/") {
        Path::new("/").join(&config.trash_subdir)
    } else {
        mountpoint.join(&config.trash_subdir)
    }
}

pub fn ensure(config: &Config, mountpoint: &Path) -> Result<TrustedRoot> {
    let path = root_path(config, mountpoint);
    let mut created = false;
    match fs::symlink_metadata(&path) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let mut builder = DirBuilder::new();
            builder.mode(0o755);
            builder.create(&path).map_err(|_| {
                AppError::operation(format!("rm: cannot create trash root '{}'", path.display()))
            })?;
            created = true;
        }
        Err(error) => {
            return Err(AppError::operation(format!(
                "rm: cannot inspect trash root '{}': {error}",
                path.display()
            )));
        }
    }

    if created && config.trash_subdir != "trash" {
        let marker = path.join(".trash-root");
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&marker)
            .map_err(|_| {
                AppError::operation(format!(
                    "rm: cannot create trash marker '{}'",
                    marker.display()
                ))
            })?;
        let _ = fs::set_permissions(&marker, fs::Permissions::from_mode(0o600));
    }
    validate(config, mountpoint, &path)
}

pub fn existing(config: &Config, mountpoint: &Path) -> Result<Option<TrustedRoot>> {
    let path = root_path(config, mountpoint);
    match fs::symlink_metadata(&path) {
        Ok(_) => validate(config, mountpoint, &path).map(Some),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(AppError::operation(format!(
            "rm: cannot inspect trash root '{}': {error}",
            path.display()
        ))),
    }
}

pub fn require_existing(config: &Config, mountpoint: &Path) -> Result<TrustedRoot> {
    let path = root_path(config, mountpoint);
    existing(config, mountpoint)?.ok_or_else(|| {
        AppError::operation(format!(
            "rm: refusing untrusted trash root '{}': it must be a real directory",
            path.display()
        ))
    })
}

fn validate(config: &Config, mountpoint: &Path, path: &Path) -> Result<TrustedRoot> {
    let metadata = fs::symlink_metadata(path).map_err(|_| {
        AppError::operation(format!(
            "rm: cannot inspect trash root '{}'",
            path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(AppError::operation(format!(
            "rm: refusing untrusted trash root '{}': it must be a real directory",
            path.display()
        )));
    }

    let mount_metadata = fs::metadata(mountpoint).map_err(|_| {
        AppError::operation(format!(
            "rm: cannot inspect mountpoint '{}'",
            mountpoint.display()
        ))
    })?;
    if !config.is_android && metadata.dev() != mount_metadata.dev() {
        return Err(AppError::operation(format!(
            "rm: refusing trash root '{}': it is on a different filesystem",
            path.display()
        )));
    }
    if should_enforce_modes(config, path) && metadata.mode() & 0o022 != 0 {
        return Err(AppError::operation(format!(
            "rm: refusing trash root '{}': group/other write permission is unsafe",
            path.display()
        )));
    }
    if config.euid == 0 && metadata.uid() != 0 {
        return Err(AppError::operation(format!(
            "rm: refusing root operation: trash root '{}' is not root-owned",
            path.display()
        )));
    }
    if !config.is_android && metadata.uid() != config.euid {
        return Err(AppError::operation(format!(
            "rm: refusing trash directory '{}': it is not owned by the current user",
            path.display()
        )));
    }

    if config.trash_subdir != "trash" {
        validate_marker(config, &path.join(".trash-root"))?;
    }

    Ok(TrustedRoot {
        path: path.to_owned(),
        device: metadata.dev(),
    })
}

fn validate_marker(config: &Config, marker: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(marker).map_err(|_| {
        AppError::operation(format!(
            "rm: refusing custom trash root '{}': missing .trash-root marker",
            marker.parent().unwrap_or(Path::new("/")).display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AppError::operation(format!(
            "rm: refusing custom trash root '{}': missing .trash-root marker",
            marker.parent().unwrap_or(Path::new("/")).display()
        )));
    }
    if config.euid == 0 && metadata.uid() != 0 {
        return Err(AppError::operation(format!(
            "rm: refusing root operation: trash marker '{}' is not root-owned",
            marker.display()
        )));
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(AppError::operation(format!(
            "rm: refusing trash marker '{}': it must be private",
            marker.display()
        )));
    }
    Ok(())
}

fn should_enforce_modes(config: &Config, path: &Path) -> bool {
    !config.is_android || !(path.starts_with("/sdcard") || path.starts_with("/storage"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    use crate::config::Config;

    use super::{root_path, should_enforce_modes};

    fn config(is_android: bool, euid: u32) -> Config {
        Config {
            trash_subdir: "trash".to_owned(),
            exceptions: HashSet::new(),
            is_android,
            euid,
        }
    }

    #[test]
    fn native_root_paths_are_per_mountpoint() {
        let config = config(false, 1000);
        assert_eq!(root_path(&config, Path::new("/")), PathBuf::from("/trash"));
        assert_eq!(
            root_path(&config, Path::new("/media/disk")),
            PathBuf::from("/media/disk/trash")
        );
    }

    #[test]
    fn android_storage_uses_hidden_sdcard_root() {
        let config = config(true, 1000);
        assert_eq!(
            root_path(&config, Path::new("/storage/emulated/0")),
            PathBuf::from("/sdcard/.trash")
        );
        assert!(!should_enforce_modes(&config, Path::new("/sdcard/.trash")));
    }

    #[test]
    fn android_shell_uid_uses_local_tmp_root() {
        let config = config(true, 2000);
        let expected = if Path::new("/data/local/tmp/termux-sudo").is_dir() {
            PathBuf::from("/data/local/tmp/termux-sudo/trash")
        } else {
            PathBuf::from("/data/local/tmp/.trash")
        };
        assert_eq!(root_path(&config, Path::new("/data")), expected);
    }
}
