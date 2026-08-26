# Trash (rm Redirector)

A robust, per-mountpoint trash system that transparently replaces the `rm` command for interactive shell use.

## Project Structure

```text
.
├── .bashrc         # Bash alias configuration
├── fish.config     # Interactive Fish shell alias configuration
├── README.md       # Project documentation and guide
├── tests/
│   └── test_trash.sh # Namespace-isolated regression and safety tests
└── trash            # Core Bash script that intercepts rm and manages the trash
```

---

## Overview

The `trash` script intercepts `rm` calls and moves files to a local `trash/` directory at the root of the file's respective filesystem mountpoint, rather than deleting them permanently. This ensures that accidental deletions can be recovered while maintaining high performance by avoiding slow cross-device file copies.

### Core Features
- **Per-Mountpoint Trashing**: Automatically detects the mountpoint of each target file (e.g., `/`, `/home`, `/mnt/data`) and moves it to a local `trash/` subdirectory on that same device. The root must be trusted, non-writable by group/other users, and cannot be a symlink.
- **Command-by-Command Grouping**: Groups files from one command into a private random run folder. The folder uses the first two shortened item names, parent application, timestamp, PID, and a random suffix. Its `payload/` directory contains the moved entries.
- **JSON Audit Logs**: Writes `metadata.json` and `items.jsonl` inside each run folder. `metadata.json` contains:
  - `command`: The exact shell-escaped command executed.
  - `cwd`: The directory from which it was run.
  - `invoked_by`: The full process parent spawning chain (e.g. `agy <- fish <- xfce4-terminal <- systemd`).
- **Interactive Clean Confirmation**: `trash --clean`, `trash --clean PATH...`, and `trash --clean-app APP...` require a terminal confirmation unless `-f`/`--force` is explicitly supplied. Noninteractive cleanup without `-f` fails closed.
- **Path Cleanup**: `trash --clean PATH...` validates every path before permanently deleting the exact files, symlinks, or directories from one trusted trash root. It refuses outside paths, the root itself, control files, nested mountpoints, and symlinked-parent escapes; final symlinks are removed without following them.
- **Hindsight App Cleanup**: `trash --clean-app APP...` matches the union of direct run-folder prefixes and exact process names in metadata for every requested app using one trash scan. It never accepts paths as application names and only deletes paths discovered directly below the trusted trash directory.
- **Exceptions Bypass**: Bypasses the trash using `/bin/rm` for exact process applications. `netmgr` is always bypassed. `paru`, `makepkg`, `yay`, and `trigger.sh` remain defaults; `TRASH_EXCEPTIONS` adds more entries. Shell script detection examines the executable or script operand, not arbitrary command arguments.
- **Argument Handling**: Combined short flags such as `-rf` are expanded. `--one-file-system` is accepted because normal trashing uses same-filesystem renames and cleanup already enforces that boundary with `/bin/rm`. Other unsupported options fail with status `2` instead of falling through to permanent `/bin/rm`.
- **Permissions Preservation**: Moves files using `mv` to preserve exact file ownership, permissions, and metadata when the trusted trash root is on the same device.
- **Collision Safety**: Destination collisions are verified and retried; a source is not reported as deleted until it has actually moved.
- **Mountpoint Protection**: Refuses to trash a filesystem mountpoint itself, including under `-f`, before creating any trash run folder.
- **Cleanup Locking**: Normal moves take a shared lock and cleanup takes an exclusive lock, preventing concurrent trash operations from being deleted unexpectedly.
- **Regression Tests**: `tests/test_trash.sh` runs the safety and behavior checks in an isolated user/mount namespace.

---

## Scope of Influence

This script is designed to be "opt-in" for interactive user safety. It is **not** a system-wide replacement for the kernel's `unlink()` system call.

### ✅ WHAT IS AFFECTED
The following will use the `trash` script instead of permanent deletion:

1.  **Interactive Shell Commands**: When you type `rm file.txt` in a terminal running Bash or Fish.
2.  **Privileged Commands**: The provided shell integrations can route the exact `sudo rm ...` form through `trash`; the trash root must be trusted for root use.
3.  **Shell Aliases**: Any user-defined alias that relies on the naked `rm` command within your interactive session.

### ❌ WHAT IS NOT AFFECTED
The following will continue to use the **real** `/bin/rm` (permanent deletion) unless configured in the exceptions bypass:

1.  **Non-Interactive Scripts**: Standard shell scripts (`#!/bin/bash`, `#!/bin/sh`) do not load interactive aliases. A script containing `rm -rf /tmp/foo` will delete it permanently.
2.  **Build Tools (`make clean`, `ninja`, `cargo clean`)**: These tools execute commands in their own subshells or call binaries directly. They do not see your shell's interactive aliases.
3.  **Package Managers (`apt`, `pacman`, `dnf`)**: Installation and uninstallation processes use system-level calls and absolute paths to manage files.
4.  **Language Managers (`pip`, `npm`, `gem`, `cargo`)**: These tools manage their internal state and caches using library calls (`unlink`, `rmdir`) or by calling `/bin/rm` directly.
5.  **CLI Agents (`gemini`, `codex`, `gh`)**: These tools are compiled binaries or run in environments where shell aliases are ignored.
6.  **System Daemons/Services**: Background processes and systemd services operate independently of user shell configurations.

---

## Configuration & Integration

### Bash (`.bashrc`)
```bash
alias rm='/home/lewis/.local/bin/trash'
alias sudo='sudo ' # Allows sudo to expand the rm alias
# TRASH_EXCEPTIONS adds entries to the built-in exception list.
export TRASH_EXCEPTIONS="custom-app"
```

### Fish (`fish.config`)
```fish
alias rm '/home/lewis/.local/bin/trash'
set -gx TRASH_EXCEPTIONS custom-app

function sudo
    if test "$argv[1]" = "rm"
        command sudo /home/lewis/.local/bin/trash $argv[2..-1]
    else
        command sudo $argv
    end
end
```

---

## Operation Notes
- **Directory Trashing**: Directories are moved as whole trees without recursive traversal, so `rm folder` works without `-r`. The `-r`, `-R`, and `-d` forms remain accepted for command compatibility.
- **Interactive Flags**: `-i` prompts per target, `-I` prompts once for recursive or large operations, and `-v` reports successful moves.
- **Force Flag**: `-f` suppresses missing-file diagnostics and bypasses cleanup prompts. Permission, storage, validation, and move failures are still reported.
- **Trash Cleanup**: `trash --clean` clears entries from the current mountpoint's trusted trash directory. `trash --clean PATH...` deletes exact trash paths from one trusted root. Both preserve root control files and refuse to cross nested filesystems.
- **App Cleanup**: `trash --clean-app APP...` removes matching direct run folders for all exact safe process names in one scan; use `trash -f --clean-app APP...` to skip the interactive prompt.
- **Custom Roots**: `TRASH_SUBDIR` accepts one safe directory name. A pre-existing custom root must contain a private `.trash-root` marker; arbitrary paths such as `.`, `/`, `..`, or `tmp` are rejected or fail trust validation.
- **Privileged Use**: Root operations require a root-owned trash root with no group/other write access. A user-owned `/trash` is intentionally rejected when invoked as root.
- **Mountpoint Targets**: Direct attempts to trash `/`, `/media/...` mount roots, or any other filesystem mountpoint fail with exit status `1` and a clear diagnostic. The mountpoint and its contents are not changed.

## Testing

Run the isolated regression suite from the project root:

```bash
env -u TRASH_EXCEPTIONS ./tests/test_trash.sh
```

It requires `unshare`, `mount`, `jq`, and the standard GNU file utilities. The
suite mounts a temporary filesystem in a private namespace and does not modify
the host trash directory.
