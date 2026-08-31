use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

#[derive(Debug, Default)]
pub struct ProcessChain {
    pub names: Vec<String>,
    pub preferred_app: Option<String>,
    pub must_bypass: bool,
}

impl ProcessChain {
    pub fn inspect(exceptions: &HashSet<String>) -> Self {
        // SAFETY: getppid has no preconditions and cannot fail.
        let mut pid = unsafe { libc::getppid() } as u32;
        let mut result = Self::default();

        while pid > 1 {
            let proc_dir = Path::new("/proc").join(pid.to_string());
            if let Ok(comm) = fs::read_to_string(proc_dir.join("comm")) {
                let name = comm.trim_end_matches(['\n', '\r']).to_owned();
                if !name.is_empty() {
                    inspect_name(&name, exceptions, &mut result);
                    result.names.push(name);
                }
            }

            if let Ok(cmdline) = fs::read(proc_dir.join("cmdline")) {
                let arguments = cmdline
                    .split(|byte| *byte == 0)
                    .filter(|value| !value.is_empty())
                    .collect::<Vec<_>>();
                if let Some(command) = arguments.first() {
                    let command = basename(command);
                    inspect_name(&command, exceptions, &mut result);
                    if let Some(script) = script_name(command.as_bytes(), &arguments[1..]) {
                        inspect_name(&script, exceptions, &mut result);
                    }
                }
            }

            let Some(parent) = read_parent_pid(&proc_dir.join("status")) else {
                break;
            };
            if parent == pid {
                break;
            }
            pid = parent;
        }
        result
    }
}

fn inspect_name(name: &str, exceptions: &HashSet<String>, result: &mut ProcessChain) {
    if result.preferred_app.is_none() && matches!(name, "codex" | "agy") {
        result.preferred_app = Some(name.to_owned());
    }
    if exceptions.contains(name) {
        result.must_bypass = true;
    }
}

fn basename(value: &[u8]) -> String {
    let value = OsStr::from_bytes(value);
    Path::new(value)
        .file_name()
        .unwrap_or(value)
        .to_string_lossy()
        .into_owned()
}

fn script_name(command: &[u8], arguments: &[&[u8]]) -> Option<String> {
    if !matches!(
        command,
        b"bash" | b"sh" | b"dash" | b"zsh" | b"ksh" | b"fish"
    ) {
        return None;
    }
    let mut after_options = false;
    for argument in arguments {
        if !after_options {
            match *argument {
                b"-c" | b"-O" | b"-o" => return None,
                b"--" => {
                    after_options = true;
                    continue;
                }
                value if value.starts_with(b"-") => continue,
                _ => {}
            }
        }
        return Some(basename(argument));
    }
    None
}

fn read_parent_pid(status_path: &Path) -> Option<u32> {
    let status = fs::read_to_string(status_path).ok()?;
    status.lines().find_map(|line| {
        line.strip_prefix("PPid:")
            .and_then(|value| value.trim().parse().ok())
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{ProcessChain, inspect_name, script_name};

    #[test]
    fn shell_script_operand_is_detected() {
        assert_eq!(
            script_name(b"bash", &[b"-e", b"/tmp/netmgr"]),
            Some("netmgr".to_owned())
        );
    }

    #[test]
    fn shell_command_text_is_not_treated_as_an_app() {
        assert_eq!(script_name(b"bash", &[b"-c", b"rm file", b"netmgr"]), None);
    }

    #[test]
    fn shell_end_of_options_script_is_detected() {
        assert_eq!(
            script_name(b"fish", &[b"--", b"/tmp/agy"]),
            Some("agy".to_owned())
        );
    }

    #[test]
    fn non_shell_commands_do_not_expose_argument_names() {
        assert_eq!(script_name(b"cargo", &[b"netmgr"]), None);
    }

    #[test]
    fn preferred_apps_use_the_first_exact_chain_match() {
        let exceptions = HashSet::new();
        let mut chain = ProcessChain::default();
        inspect_name("bash", &exceptions, &mut chain);
        inspect_name("agy", &exceptions, &mut chain);
        inspect_name("codex", &exceptions, &mut chain);
        assert_eq!(chain.preferred_app.as_deref(), Some("agy"));
    }

    #[test]
    fn exceptions_are_exact_process_names() {
        let exceptions = HashSet::from(["paru".to_owned()]);
        let mut chain = ProcessChain::default();
        inspect_name("par", &exceptions, &mut chain);
        assert!(!chain.must_bypass);
        inspect_name("paru", &exceptions, &mut chain);
        assert!(chain.must_bypass);
    }
}
