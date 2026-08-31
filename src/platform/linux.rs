use std::collections::HashSet;
use std::ffi::CString;
use std::fs;
use std::io;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};

#[derive(Debug)]
pub struct MountTable {
    mountpoints: Vec<PathBuf>,
    mountpoint_set: HashSet<PathBuf>,
}

impl MountTable {
    pub fn load() -> Result<Self> {
        let mountinfo = fs::read("/proc/self/mountinfo").map_err(|error| {
            AppError::operation(format!("rm: cannot read /proc/self/mountinfo: {error}"))
        })?;
        let mut mountpoints = mountinfo
            .split(|byte| *byte == b'\n')
            .filter_map(parse_mountpoint)
            .collect::<Vec<_>>();
        mountpoints.sort_by_key(|path| path.as_os_str().as_bytes().len());
        let mountpoint_set = mountpoints.iter().cloned().collect();
        Ok(Self {
            mountpoints,
            mountpoint_set,
        })
    }

    pub fn mountpoint_for(&self, path: &Path) -> io::Result<PathBuf> {
        let location = canonical_location(path)?;
        self.mountpoints
            .iter()
            .rev()
            .find(|mountpoint| location == **mountpoint || location.starts_with(mountpoint))
            .cloned()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no containing mountpoint"))
    }

    pub fn is_mountpoint(&self, path: &Path) -> io::Result<bool> {
        let metadata = fs::symlink_metadata(path)?;
        if metadata.file_type().is_symlink() {
            return Ok(false);
        }
        let location = fs::canonicalize(path)?;
        Ok(self.mountpoint_set.contains(&location))
    }

    pub fn is_known_mountpoint(&self, absolute_path: &Path) -> bool {
        self.mountpoint_set.contains(absolute_path)
    }
}

pub fn canonical_location(path: &Path) -> io::Result<PathBuf> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        let parent = path
            .parent()
            .filter(|value| !value.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let file_name = path.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "symlink path has no file name")
        })?;
        return Ok(fs::canonicalize(parent)?.join(file_name));
    }
    fs::canonicalize(path)
}

fn parse_mountpoint(line: &[u8]) -> Option<PathBuf> {
    let field = line.split(|byte| *byte == b' ').nth(4)?;
    Some(PathBuf::from(std::ffi::OsString::from_vec(
        decode_mount_field(field),
    )))
}

fn decode_mount_field(field: &[u8]) -> Vec<u8> {
    let mut decoded = Vec::with_capacity(field.len());
    let mut index = 0;
    while index < field.len() {
        if field[index] == b'\\'
            && index + 3 < field.len()
            && field[index + 1..index + 4]
                .iter()
                .all(|byte| matches!(byte, b'0'..=b'7'))
        {
            let value = (field[index + 1] - b'0') * 64
                + (field[index + 2] - b'0') * 8
                + (field[index + 3] - b'0');
            decoded.push(value);
            index += 4;
        } else {
            decoded.push(field[index]);
            index += 1;
        }
    }
    decoded
}

pub fn rename_noreplace(source: &Path, destination: &Path) -> io::Result<()> {
    let source = CString::new(source.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // SAFETY: both paths are valid NUL-terminated strings and the remaining
    // syscall arguments are constants accepted by renameat2.
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            libc::AT_FDCWD,
            source.as_ptr(),
            libc::AT_FDCWD,
            destination.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{decode_mount_field, parse_mountpoint};

    #[test]
    fn mountinfo_escapes_are_decoded() {
        assert_eq!(
            decode_mount_field(br"/media/with\040space"),
            b"/media/with space"
        );
    }

    #[test]
    fn mountpoint_field_is_selected() {
        let line = b"36 25 0:32 / /mnt/test rw - tmpfs tmpfs rw";
        assert_eq!(parse_mountpoint(line), Some(PathBuf::from("/mnt/test")));
    }
}
