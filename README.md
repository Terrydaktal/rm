# Trash (rm Redirector)

A robust, per-mountpoint trash system that transparently replaces the `rm` command for interactive shell use.

## Project Structure

```text
.
├── fish.config     # Interactive Fish shell alias & sudo wrapper configuration
├── README.md       # Project documentation and guide
└── trash           # Core Bash script that intercepts rm and manages the trash
```

---

## Overview

The `trash` script intercepts `rm` calls and moves files to a local `trash/` directory at the root of the file's respective filesystem mountpoint, rather than deleting them permanently. This ensures that accidental deletions can be recovered while maintaining high performance by avoiding slow cross-device file copies.

### Core Features
- **Per-Mountpoint Trashing**: Automatically detects the mountpoint of each target file (e.g., `/`, `/home`, `/mnt/data`) and moves it to a local `trash/` subdirectory on that same device.
- **Command-by-Command Grouping**: Group all files deleted in a single command execution into a unique master folder inside the trash. The folder is named using the first two deleted items as a prefix, a readable timestamp, the parent deleting application, and the process ID (PID):
  `([PARENT_APP])[FILE1]+[FILE2]-YYYY-MM-DD_HH-MM-SS-pid-[PID]/`
- **JSON Audit Logs**: Writes a `metadata.json` file inside each master folder containing:
  - `command`: The exact shell-escaped command executed.
  - `cwd`: The directory from which it was run.
  - `invoked_by`: The full process parent spawning chain (e.g. `agy <- fish <- xfce4-terminal <- systemd`).
- **Interactive Clean Confirmation**: Running `trash --clean` will verify your current mountpoint and prompt you to type the exact path of the trash directory to confirm permanent deletion. (Automatically bypassed if standard input/output is not a terminal, or if `-f`/`--force` is specified).
- **Hindsight App Cleanup**: Running `trash --clean-app APP` permanently deletes trash run folders on the current mountpoint whose folder name starts with `(APP)` or whose `metadata.json` execution chain has `APP` as an exact process name. It uses the same interactive confirmation behavior as `--clean`.
- **Exceptions Bypass**: Bypasses the trash (permanently deletes using `/bin/rm`) for specific applications. `netmgr` is always bypassed. Additional exceptions are read from the `TRASH_EXCEPTIONS` environment variable (e.g., `set -gx TRASH_EXCEPTIONS "paru makepkg yay"`) and fall back to a default list (`paru`, `makepkg`, `yay`, `trigger.sh`) when unset. The script inspects both process names and full command arguments (`/proc/$PID/cmdline`) to match shell-interpreted scripts in the execution chain.
- **Argument Unpacking**: Pre-processes and expands combined short flags (e.g. `-rf` -> `-r -f`) to ensure standard flags do not trigger accidental fallback to real `/bin/rm`.
- **Permissions Preservation**: Moves files using `mv` to preserve exact file ownership, permissions, and metadata.
- **Safety Fallback**: Unknown options are passed through to the real `/bin/rm`; `--help` and `--version` are handled by the wrapper itself.
- **Mountpoint Protection**: Refuses to trash a filesystem mountpoint itself, including under `-f`, before creating any trash run folder.

---

## Scope of Influence

This script is designed to be "opt-in" for interactive user safety. It is **not** a system-wide replacement for the kernel's `unlink()` system call.

### ✅ WHAT IS AFFECTED
The following will use the `trash` script instead of permanent deletion:

1.  **Interactive Shell Commands**: When you type `rm file.txt` in a terminal running Bash or Fish.
2.  **Sudo Commands**: Typing `sudo rm file.txt` is intercepted by a shell wrapper that redirects it to `sudo trash`.
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
export TRASH_EXCEPTIONS="paru makepkg yay trigger.sh"
```

### Fish (`fish.config`)
```fish
alias rm '/home/lewis/.local/bin/trash'
set -gx TRASH_EXCEPTIONS "paru makepkg yay trigger.sh"

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
- **Recursive Deletion**: The `-r` and `-R` flags are accepted but ignored; since `mv` can move directories, the script naturally handles recursive "deletion" by moving the entire tree to the trash.
- **Force Flag**: The `-f` flag suppresses error messages (e.g., if a file doesn't exist) and bypasses clean prompts, mimicking standard `rm` behavior.
- **Trash Cleanup**: `trash --clean` clears all entries from the current mountpoint's trash directory. It requires path confirmation when run interactively.
- **App Cleanup**: `trash --clean-app APP` removes grouped trash runs whose folder name starts with `(APP)` or whose recorded `invoked_by` chain has `APP` as an exact process name; use `trash -f --clean-app APP` to skip the interactive prompt.
- **Mountpoint Targets**: Direct attempts to trash `/`, `/media/...` mount roots, or any other filesystem mountpoint fail with exit status `1` and a clear diagnostic. The mountpoint and its contents are not changed.
