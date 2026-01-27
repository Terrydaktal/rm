# Trash (rm Redirector)

A robust, per-mountpoint trash system that transparently replaces the `rm` command for interactive shell use.

## Overview

The `trash` script intercepts `rm` calls and moves files to a `trash/` directory at the root of the file's respective filesystem mountpoint, rather than deleting them permanently. This ensures that accidental deletions can be recovered while maintaining performance by avoiding cross-device file moves.

### Core Features
- **Per-Mountpoint Trashing**: Automatically detects the mountpoint of each file (e.g., `/`, `/home`, `/mnt/data`) and moves it to a local `trash/` subdirectory.
- **Collision Prevention**: If a file with the same name already exists in the trash, it appends a unique suffix (timestamp + nanoseconds + PID) to prevent overwrites.
- **Permissions Preservation**: Uses `mv` to preserve file ownership and permissions.
- **Safety Fallback**: If flags like `--help` or `--version` are used, or if the script encounters unknown options, it safely passes the call through to the real `/bin/rm`.

---

## Scope of Influence

This script is designed to be "opt-in" for interactive user safety. It is **not** a system-wide replacement for the `unlink()` system call.

### ✅ WHAT IS AFFECTED
The following will use the `trash` script instead of permanent deletion:

1.  **Interactive Shell Commands**: When you type `rm file.txt` in a terminal running Bash or Fish.
2.  **Sudo Commands**: Typing `sudo rm file.txt` is intercepted by a shell wrapper that redirects it to `sudo trash`.
3.  **Shell Aliases**: Any user-defined alias that relies on the naked `rm` command within your interactive session.

### ❌ WHAT IS NOT AFFECTED
The following will continue to use the **real** `/bin/rm` (permanent deletion):

1.  **Non-Interactive Scripts**: Standard shell scripts (`#!/bin/bash`, `#!/bin/sh`) do not load interactive aliases. A script containing `rm -rf /tmp/foo` will delete it permanently.
2.  **Build Tools (`make clean`, `ninja`, `cargo clean`)**: These tools execute commands in their own subshells or call binaries directly. They do not see your shell's interactive aliases.
3.  **Package Managers (`apt`, `pacman`, `dnf`)**: Installation and uninstallation processes use system-level calls and absolute paths to manage files.
4.  **Language Managers (`pip`, `npm`, `gem`, `cargo`)**: These tools manage their internal state and caches using library calls (`unlink`, `rmdir`) or by calling `/bin/rm` directly.
5.  **CLI Agents (`gemini`, `codex`, `gh`)**: These tools are compiled binaries or run in environments where shell aliases are ignored.
6.  **System Daemons/Services**: Background processes and systemd services operate independently of user shell configurations.

---

## Why are some things not affected?

1.  **Shell Aliases vs. Binaries**: The redirection is implemented as a **shell alias** (Bash) or **shell function** (Fish). These features only exist within the context of an active, interactive terminal session.
2.  **Path Resolution**: Many professional tools and system scripts use the absolute path `/bin/rm` to ensure predictable behavior, bypassing any user-defined `rm` in the `$PATH`.
3.  **System Calls**: Most high-level languages (Python, Rust, Go, C++) delete files using the `unlink()` or `remove()` system calls provided by the Linux kernel. These calls do not involve the `rm` command at all.

## Configuration & Integration

### Bash (`.bashrc`)
```bash
alias rm='/path/to/trash'
alias sudo='sudo ' # Allows sudo to expand the rm alias
```

### Fish (`fish.config`)
```fish
alias rm '/path/to/trash'
function sudo
    if test "$argv[1]" = "rm"
        command sudo /path/to/trash $argv[2..-1]
    else
        command sudo $argv
    end
end
```

## Operation Notes
- **Recursive Deletion**: The `-r` and `-R` flags are accepted but ignored; since `mv` can move directories, the script naturally handles recursive "deletion" by moving the entire tree to the trash.
- **Force Flag**: The `-f` flag suppresses error messages (e.g., if a file doesn't exist), mimicking standard `rm` behavior.
