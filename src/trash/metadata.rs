use std::borrow::Cow;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

#[derive(Debug, Serialize)]
pub struct Metadata {
    command: String,
    cwd: String,
    invoked_by: String,
}

impl Metadata {
    pub fn new(arguments: &[OsString], process_names: &[String]) -> Self {
        let mut command = String::from("rm");
        for argument in arguments {
            command.push(' ');
            command.push_str(&shell_escape(argument));
        }
        let cwd = env::var_os("PWD")
            .or_else(|| env::current_dir().ok().map(|path| path.into_os_string()))
            .map(|value| audit_string(&value))
            .unwrap_or_default();
        Self {
            command,
            cwd,
            invoked_by: process_names.join(" <- "),
        }
    }

    pub fn write_atomic(&self, run_dir: &Path) -> Result<()> {
        let temporary = run_dir.join(format!(".metadata.json.tmp.{}", random_suffix()?));
        let target = run_dir.join("metadata.json");
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            serde_json::to_writer_pretty(&mut file, self)?;
            file.write_all(b"\n")?;
            file.flush()?;
            fs::rename(&temporary, &target)
        })();
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(AppError::operation(format!(
                "rm: cannot write metadata in '{}': {error}",
                run_dir.display()
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
pub struct ManifestItem<'a> {
    source: Cow<'a, str>,
    destination: Cow<'a, str>,
}

impl<'a> ManifestItem<'a> {
    pub fn new(source: &'a OsStr, destination: &'a OsStr) -> Self {
        Self {
            source: audit_string_cow(source),
            destination: audit_string_cow(destination),
        }
    }

    pub fn append_to(&self, file: &mut File) -> std::io::Result<()> {
        serde_json::to_writer(&mut *file, self)?;
        file.write_all(b"\n")?;
        file.flush()
    }
}

#[derive(Debug, Deserialize)]
struct ScannedMetadata {
    invoked_by: String,
}

pub fn metadata_matches(path: &Path, applications: &std::collections::HashSet<String>) -> bool {
    let Ok(file) = File::open(path) else {
        return false;
    };
    let Ok(metadata) = serde_json::from_reader::<_, ScannedMetadata>(BufReader::new(file)) else {
        return false;
    };
    metadata
        .invoked_by
        .split(" <- ")
        .any(|name| applications.contains(name))
}

pub fn random_suffix() -> Result<String> {
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut random = [0_u8; 8];
    getrandom::fill(&mut random).map_err(|error| {
        AppError::operation(format!(
            "rm: cannot obtain randomness for trash run: {error}"
        ))
    })?;
    Ok(random
        .iter()
        .map(|byte| ALPHABET[*byte as usize % ALPHABET.len()] as char)
        .collect())
}

pub fn audit_string(value: &OsStr) -> String {
    audit_string_cow(value).into_owned()
}

fn audit_string_cow(value: &OsStr) -> Cow<'_, str> {
    match value.to_str() {
        Some(value) => Cow::Borrowed(value),
        None => Cow::Owned(
            value
                .as_bytes()
                .iter()
                .map(|byte| char::from(*byte))
                .collect(),
        ),
    }
}

fn shell_escape(value: &OsStr) -> String {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return "''".to_owned();
    }
    if bytes.iter().all(|byte| {
        byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'_' | b'@' | b'%' | b'+' | b'=' | b':' | b',' | b'.' | b'/' | b'-'
            )
    }) {
        return audit_string(value);
    }
    let text = audit_string(value);
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::ffi::OsStr;
    use std::fs;
    use std::fs::OpenOptions;
    use std::os::unix::ffi::OsStrExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{ManifestItem, Metadata, audit_string, metadata_matches};

    static NEXT_TEST: AtomicUsize = AtomicUsize::new(0);

    fn test_dir() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "trash-metadata-test-{}-{}",
            std::process::id(),
            NEXT_TEST.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn app_metadata_matching_is_exact() {
        let dir = test_dir();
        let path = dir.join("metadata.json");
        fs::write(&path, r#"{"invoked_by":"bash <- paru <- systemd"}"#).unwrap();
        assert!(metadata_matches(&path, &HashSet::from(["paru".to_owned()])));
        assert!(!metadata_matches(&path, &HashSet::from(["par".to_owned()])));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn malformed_and_blank_metadata_never_match() {
        let dir = test_dir();
        let path = dir.join("metadata.json");
        for contents in [b"".as_slice(), b"not-json", br#"{"invoked_by":42}"#] {
            fs::write(&path, contents).unwrap();
            assert!(!metadata_matches(
                &path,
                &HashSet::from(["codex".to_owned()])
            ));
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn metadata_atomic_rename_failure_removes_temporary_file() {
        let dir = test_dir();
        fs::create_dir(dir.join("metadata.json")).unwrap();
        let metadata = Metadata::new(&[], &[]);
        assert!(metadata.write_atomic(&dir).is_err());
        assert!(fs::read_dir(&dir).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".metadata.json.tmp.")
        }));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn manifest_write_failure_is_reported() {
        let mut full = OpenOptions::new().write(true).open("/dev/full").unwrap();
        let item = ManifestItem::new(OsStr::new("source"), OsStr::new("destination"));
        assert!(item.append_to(&mut full).is_err());
    }

    #[test]
    fn manifest_failure_after_move_leaves_moved_item_recoverable() {
        let dir = test_dir();
        let source = dir.join("source");
        let destination = dir.join("destination");
        fs::write(&source, b"recoverable").unwrap();
        fs::rename(&source, &destination).unwrap();

        let mut full = OpenOptions::new().write(true).open("/dev/full").unwrap();
        let item = ManifestItem::new(source.as_os_str(), destination.as_os_str());
        assert!(item.append_to(&mut full).is_err());
        assert!(!source.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"recoverable");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn non_utf8_audit_values_preserve_every_byte() {
        let value = OsStr::from_bytes(b"before\xffafter");
        assert_eq!(audit_string(value).as_bytes(), b"before\xc3\xbfafter");
    }
}
