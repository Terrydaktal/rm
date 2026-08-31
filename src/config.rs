use std::collections::HashSet;
use std::env;
use std::path::Path;

use crate::error::{AppError, Result};

const DEFAULT_EXCEPTIONS: &[&str] = &["paru", "makepkg", "yay", "trigger.sh"];
const HARDCODED_EXCEPTIONS: &[&str] = &["netmgr"];

#[derive(Debug, Clone)]
pub struct Config {
    pub trash_subdir: String,
    pub exceptions: HashSet<String>,
    pub is_android: bool,
    pub euid: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let trash_subdir = env::var("TRASH_SUBDIR").unwrap_or_else(|_| "trash".to_owned());
        if !is_safe_name(&trash_subdir) || matches!(trash_subdir.as_str(), "." | "..") {
            return Err(AppError::usage(format!(
                "rm: invalid TRASH_SUBDIR '{trash_subdir}': use one safe directory name"
            )));
        }

        let mut exceptions = DEFAULT_EXCEPTIONS
            .iter()
            .chain(HARDCODED_EXCEPTIONS)
            .map(|value| (*value).to_owned())
            .collect::<HashSet<_>>();
        if let Some(configured) = env::var_os("TRASH_EXCEPTIONS") {
            exceptions.extend(
                configured
                    .to_string_lossy()
                    .split_whitespace()
                    .map(str::to_owned),
            );
        }

        Ok(Self {
            trash_subdir,
            exceptions,
            is_android: Path::new("/system/bin").is_dir()
                && (Path::new("/data/data/com.termux").is_dir()
                    || Path::new("/data/local/tmp").is_dir()),
            // SAFETY: geteuid has no preconditions and cannot fail.
            euid: unsafe { libc::geteuid() },
        })
    }
}

pub fn is_safe_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

#[cfg(test)]
mod tests {
    use super::is_safe_name;

    #[test]
    fn safe_names_are_ascii_and_pathless() {
        assert!(is_safe_name("codex-1.0_rc"));
        assert!(!is_safe_name(""));
        assert!(!is_safe_name("foo/bar"));
        assert!(!is_safe_name("has space"));
    }
}
