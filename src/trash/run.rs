use std::fs::{self, DirBuilder, File, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{AppError, Result};

use super::locking::{LockMode, TrashLock};
use super::metadata::{ManifestItem, Metadata, random_suffix};
use super::root;

#[derive(Debug)]
pub struct Run {
    pub run_dir: PathBuf,
    pub payload_dir: PathBuf,
    manifest: File,
    _lock: TrashLock,
}

impl Run {
    pub fn create(
        config: &Config,
        mountpoint: &Path,
        run_name: &str,
        metadata: &Metadata,
    ) -> Result<Self> {
        let root = root::ensure(config, mountpoint)?;
        let lock = TrashLock::acquire(&root.path, LockMode::Shared)?;
        let run_dir = create_unique_run_dir(&root.path, run_name)?;
        let payload_dir = run_dir.join("payload");
        let setup = (|| {
            let mut builder = DirBuilder::new();
            builder.mode(0o700);
            builder.create(&payload_dir)?;
            metadata
                .write_atomic(&run_dir)
                .map_err(|error| std::io::Error::other(error.message))?;
            OpenOptions::new()
                .append(true)
                .create_new(true)
                .mode(0o600)
                .open(run_dir.join("items.jsonl"))
        })();
        let manifest = match setup {
            Ok(manifest) => manifest,
            Err(error) => {
                let _ = fs::remove_dir_all(&run_dir);
                return Err(AppError::operation(format!(
                    "rm: cannot initialize trash run '{}': {error}",
                    run_dir.display()
                )));
            }
        };
        Ok(Self {
            run_dir,
            payload_dir,
            manifest,
            _lock: lock,
        })
    }

    pub fn record(&mut self, source: &Path, destination: &Path) -> Result<()> {
        ManifestItem::new(source.as_os_str(), destination.as_os_str())
            .append_to(&mut self.manifest)
            .map_err(|_| {
                AppError::operation(format!(
                    "rm: warning: moved '{}' but could not update '{}/items.jsonl'",
                    source.display(),
                    self.run_dir.display()
                ))
            })
    }

    pub fn is_empty(&self) -> bool {
        fs::read_dir(&self.payload_dir)
            .map(|mut entries| entries.next().is_none())
            .unwrap_or(false)
    }
}

fn create_unique_run_dir(root: &Path, run_name: &str) -> Result<PathBuf> {
    for _ in 0..100 {
        let path = root.join(format!("{run_name}.{}", random_suffix()?));
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => break,
        }
    }
    Err(AppError::operation(format!(
        "rm: cannot create a unique trash run directory in '{}'",
        root.display()
    )))
}
