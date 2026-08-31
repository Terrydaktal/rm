use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};

use crate::config::is_safe_name;
use crate::error::{AppError, Result};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum InteractiveMode {
    Never,
    Always,
    Once,
}

#[derive(Debug)]
pub struct Cli {
    pub force: bool,
    pub recursive: bool,
    pub interactive: InteractiveMode,
    pub verbose: bool,
    pub clean: bool,
    pub clean_apps: Vec<String>,
    pub show_help: bool,
    pub show_version: bool,
    pub operands: Vec<OsString>,
}

impl Cli {
    pub fn parse(arguments: Vec<OsString>) -> Result<Self> {
        let arguments = unpack_combined_options(arguments);
        let mut cli = Self {
            force: false,
            recursive: false,
            interactive: InteractiveMode::Never,
            verbose: false,
            clean: false,
            clean_apps: Vec::new(),
            show_help: false,
            show_version: false,
            operands: Vec::new(),
        };
        let mut clean_app_mode = false;
        let mut after_options = false;

        for argument in arguments {
            if after_options {
                push_operand(&mut cli, clean_app_mode, argument)?;
                continue;
            }
            let bytes = argument.as_bytes();
            if bytes == b"--" {
                after_options = true;
                continue;
            }
            match bytes {
                b"--help" => cli.show_help = true,
                b"--version" => cli.show_version = true,
                b"-f" | b"--force" => cli.force = true,
                b"-r" | b"-R" | b"--recursive" => cli.recursive = true,
                b"-d" | b"--dir" => {}
                b"-i" | b"--interactive" => cli.interactive = InteractiveMode::Always,
                b"-I" => cli.interactive = InteractiveMode::Once,
                b"-v" | b"--verbose" => cli.verbose = true,
                b"--interactive=always" => cli.interactive = InteractiveMode::Always,
                b"--interactive=once" => cli.interactive = InteractiveMode::Once,
                b"--interactive=never" => cli.interactive = InteractiveMode::Never,
                b"--preserve-root" | b"--preserve-root=all" | b"--one-file-system" => {}
                b"--no-preserve-root" => {
                    return Err(AppError::usage(
                        "rm: refusing unsupported --no-preserve-root because trash safety cannot be bypassed",
                    ));
                }
                b"--clean" => cli.clean = true,
                b"--clean-app" => clean_app_mode = true,
                _ if bytes.starts_with(b"--clean-app=") => {
                    clean_app_mode = true;
                    let value = &bytes[b"--clean-app=".len()..];
                    if value.is_empty() {
                        return Err(AppError::usage(
                            "rm: option '--clean-app' requires an app name",
                        ));
                    }
                    add_clean_app(&mut cli.clean_apps, OsStr::from_bytes(value))?;
                }
                _ if bytes.starts_with(b"-") => {
                    return Err(AppError::usage(format!(
                        "rm: unsupported option '{}'; refusing permanent /bin/rm fallback",
                        argument.to_string_lossy()
                    )));
                }
                _ => push_operand(&mut cli, clean_app_mode, argument)?,
            }
        }

        if cli.clean && clean_app_mode {
            return Err(AppError::usage(
                "rm: cannot combine --clean and --clean-app",
            ));
        }
        if clean_app_mode && cli.clean_apps.is_empty() {
            return Err(AppError::usage(
                "rm: option '--clean-app' requires at least one app name",
            ));
        }
        deduplicate(&mut cli.clean_apps);
        Ok(cli)
    }

    pub fn is_cleanup(&self) -> bool {
        self.clean || !self.clean_apps.is_empty()
    }
}

fn push_operand(cli: &mut Cli, clean_app_mode: bool, argument: OsString) -> Result<()> {
    if clean_app_mode {
        add_clean_app(&mut cli.clean_apps, &argument)
    } else {
        cli.operands.push(argument);
        Ok(())
    }
}

fn add_clean_app(apps: &mut Vec<String>, value: &OsStr) -> Result<()> {
    let value = value.to_str().ok_or_else(|| {
        AppError::usage(format!(
            "rm: invalid application name '{}'",
            value.to_string_lossy()
        ))
    })?;
    if !is_safe_name(value) {
        return Err(AppError::usage(format!(
            "rm: invalid application name '{value}'"
        )));
    }
    apps.push(value.to_owned());
    Ok(())
}

fn deduplicate(values: &mut Vec<String>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn unpack_combined_options(arguments: Vec<OsString>) -> Vec<OsString> {
    let mut unpacked = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let bytes = argument.as_bytes();
        if bytes.len() >= 3 && bytes[0] == b'-' && bytes[1..].iter().all(u8::is_ascii_alphanumeric)
        {
            unpacked.extend(
                bytes[1..]
                    .iter()
                    .map(|byte| OsString::from_vec(vec![b'-', *byte])),
            );
        } else {
            unpacked.push(argument);
        }
    }
    unpacked
}

pub fn help(program: &str) -> String {
    format!(
        r#"NAME
    {program} - per-mountpoint rm wrapper with grouped trash folders and cleanup tools

SYNOPSIS
    {program} [OPTIONS] FILE...
    {program} --clean [-f]
    {program} --clean PATH... [-f]
    {program} --clean-app APP... [-f]
    {program} --help
    {program} --version

DESCRIPTION
    {program} intercepts rm-style deletes and moves each target into a trash directory
    on the same filesystem mountpoint instead of unlinking it immediately.

    Files deleted in one command are grouped into a private random run folder
    named from the preferred application, shortened target names, timestamp, and
    PID. The moved entries are stored below that run folder's payload directory.

    Each run folder contains metadata.json and items.jsonl audit records with the
    original command, working directory, parent execution chain, and move map.

OPTIONS
    -f, --force
        Suppress missing-file diagnostics. For cleanup modes, bypass the
        permanent-deletion confirmation prompt. Safety checks remain active.

    -r, -R, --recursive, -d, --dir
        Accepted for rm compatibility. Directories are moved as whole trees and
        do not require either option.

    -i, --interactive
        Prompt before moving each target.

    -I
        Prompt once before recursive or operations with more than three targets.

    -v, --verbose
        Report each successful move.

    --interactive=always|once|never
        Select per-target, once-per-operation, or no interactive prompting.

    --preserve-root, --preserve-root=all, --one-file-system
        Accepted compatibility safety options. Mountpoint protections always
        remain active, and cleanup passes --one-file-system to /bin/rm.

    --clean [PATH...]
        Permanently empty the current filesystem's trash, or delete exact paths
        from one trusted trash root.

    --clean-app APP...
        Permanently delete direct run folders whose name or metadata process
        chain exactly matches any APP. Names are checked before metadata is read.

    --help
        Show this help text and exit.

    --version
        Show version information and exit.

OPERATION
    1. Parent processes in the exception list bypass trash through /bin/rm.
    2. Each target is moved to a private run under MOUNTPOINT/TRASH_SUBDIR.
    3. codex or agy is preferred as the run label when present in the chain.
    4. Moves hold shared locks; cleanup holds an exclusive lock.
    5. Invalid or unsupported options fail closed with status 2.

EXAMPLES
    {program} file.txt dir
    {program} -rf --one-file-system build-cache
    {program} --clean
    {program} -f --clean '/trash/(agy)run/payload/one'
    {program} -f --clean-app agy codex

FILES
    MOUNTPOINT/TRASH_SUBDIR/<run>/metadata.json
        JSON audit record containing command, cwd, and invoked_by fields.

    MOUNTPOINT/TRASH_SUBDIR/<run>/items.jsonl
        One JSON object per successfully moved source and destination pair.

    MOUNTPOINT/TRASH_SUBDIR/<run>/payload/
        Private directory containing moved files and directories.

    MOUNTPOINT/TRASH_SUBDIR/.trash.lock
        Advisory lock coordinating moves and permanent cleanup.

    MOUNTPOINT/TRASH_SUBDIR/.trash-root
        Private marker required when TRASH_SUBDIR is not "trash".

PATHS
    /trash
        Trash root for the / mountpoint.

    MOUNTPOINT/trash
        Default trash root for other mountpoints.

    TRASH_SUBDIR
        Environment variable selecting one safe trash directory name.

SECURITY NOTES
    Cleanup is the only normal mode that permanently deletes data and explicitly
    invokes /bin/rm. Cleanup refuses roots, control files, paths outside a trusted
    root, symlinked-parent escapes, and nested filesystem targets. Direct attempts
    to trash a mountpoint or an item already inside trash are refused. Trash roots
    must be real, same-filesystem, safely permissioned, and correctly owned.
    TRASH_EXCEPTIONS adds exact process names; netmgr always bypasses trash.

EXIT STATUS
    0  Success.
    1  One or more targets or cleanup actions failed.
    2  Invalid or unsupported command usage.

AUTHORS
    Terrydaktal
"#
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    use super::{Cli, InteractiveMode, help};

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn combined_flags_are_unpacked() {
        let cli = Cli::parse(args(&["-rfv", "item"])).unwrap();
        assert!(cli.force);
        assert!(cli.recursive);
        assert!(cli.verbose);
        assert_eq!(cli.operands, args(&["item"]));
    }

    #[test]
    fn combined_tokens_are_unpacked_before_end_of_options() {
        let cli = Cli::parse(args(&["--", "-rf"])).unwrap();
        assert_eq!(cli.operands, args(&["-r", "-f"]));
    }

    #[test]
    fn app_names_are_exact_and_deduplicated() {
        let cli = Cli::parse(args(&["--clean-app", "par", "par", "paru"])).unwrap();
        assert_eq!(cli.clean_apps, ["par", "paru"]);
    }

    #[test]
    fn later_interactive_option_wins() {
        let cli = Cli::parse(args(&["-i", "--interactive=never", "item"])).unwrap();
        assert_eq!(cli.interactive, InteractiveMode::Never);
    }

    #[test]
    fn compatibility_options_are_all_accepted() {
        let cli = Cli::parse(args(&[
            "-R",
            "-d",
            "--recursive",
            "--dir",
            "--preserve-root",
            "--preserve-root=all",
            "--one-file-system",
            "item",
        ]))
        .unwrap();
        assert!(cli.recursive);
        assert_eq!(cli.operands, args(&["item"]));
    }

    #[test]
    fn clean_modes_cannot_be_combined() {
        let error = Cli::parse(args(&["--clean", "--clean-app", "app"])).unwrap_err();
        assert_eq!(error.code, 2);
    }

    #[test]
    fn clean_app_requires_a_valid_name() {
        for values in [
            args(&["--clean-app"]),
            args(&["--clean-app="]),
            args(&["--clean-app", "bad/name"]),
        ] {
            assert_eq!(Cli::parse(values).unwrap_err().code, 2);
        }
        let invalid_utf8 = OsString::from_vec(vec![b'a', 0xff]);
        assert_eq!(
            Cli::parse(vec![OsString::from("--clean-app"), invalid_utf8])
                .unwrap_err()
                .code,
            2
        );
    }

    #[test]
    fn clean_app_equals_form_is_supported() {
        let cli = Cli::parse(args(&["--clean-app=codex"])).unwrap();
        assert_eq!(cli.clean_apps, ["codex"]);
    }

    #[test]
    fn unsupported_options_always_fail_closed() {
        for option in ["--unknown", "--no-preserve-root", "-x"] {
            assert_eq!(Cli::parse(args(&[option])).unwrap_err().code, 2);
        }
    }

    #[test]
    fn help_contains_every_required_section() {
        let help = help("trash");
        for heading in [
            "NAME",
            "SYNOPSIS",
            "DESCRIPTION",
            "OPTIONS",
            "OPERATION",
            "EXAMPLES",
            "FILES",
            "PATHS",
            "SECURITY NOTES",
            "EXIT STATUS",
            "AUTHORS",
        ] {
            assert!(help.lines().any(|line| line == heading));
        }
    }
}
