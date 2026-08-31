# Trash

`trash` is a Linux and Android `rm` replacement that moves targets into a trusted
trash root on the same filesystem instead of unlinking them. One invocation creates
one private run per affected mountpoint, preserving the original file tree and
recording JSON audit metadata.

The active implementation is a modular Rust binary. The previous Bash implementation
is retained only as a behavioral reference.

## Features

- Moves files, symlinks, and whole directory trees without recursive traversal.
- Refuses filesystem mountpoints and paths already inside a trash root.
- Creates collision-safe, private run directories grouped by command invocation.
- Records `metadata.json` and append-only `items.jsonl` audit data.
- Supports exact-path, whole-root, and exact-application cleanup.
- Scans application cleanup in one native directory pass and reads metadata only
  when the run-folder name does not already match.
- Uses shared locks for moves and exclusive locks for permanent cleanup.
- Validates trash-root ownership, permissions, device, markers, and symlink state.
- Accepts common GNU `rm` flags, including `-rf` and `--one-file-system`, while
  failing closed on unsupported safety-changing options.
- Bypasses trash through explicit `/bin/rm` only for configured process exceptions.

## Project Structure

```text
.
|-- Cargo.toml                  # Rust package, dependencies, and release profile
|-- Cargo.lock                  # Reproducible application dependency versions
|-- README.md                   # User, build, installation, and operation guide
|-- CHANGELOG.md                # User-visible release history
|-- CONTRIBUTING.md             # Development and validation workflow
|-- docs/
|   |-- architecture.md         # Module boundaries and execution pipelines
|   |-- metadata-format.md      # Run directory and JSON record contracts
|   `-- security-model.md       # Trust boundaries and safety invariants
|-- integrations/
|   |-- bash/trash.bash         # Bash aliases for rm and sudo rm
|   `-- fish/trash.fish         # Fish alias and sudo forwarding function
|-- reference/trash.bash        # Frozen pre-Rust implementation; not installed
|-- src/
|   |-- main.rs                 # Minimal process entry point
|   |-- lib.rs                  # Top-level dispatch and /bin/rm exception exec
|   |-- cli.rs                  # rm-compatible parser and complete help text
|   |-- config.rs               # Environment configuration and exception defaults
|   |-- process_chain.rs        # /proc ancestry and exact application detection
|   |-- platform/linux.rs       # Mount table parsing and no-replace rename syscall
|   `-- trash/
|       |-- root.rs             # Trash-root location and trust validation
|       |-- locking.rs          # Shared/exclusive advisory locking
|       |-- metadata.rs         # JSON metadata and manifest serialization
|       |-- run.rs              # Private run creation and lifecycle
|       |-- move_item.rs        # Target validation and atomic movement
|       `-- cleanup/            # Whole-root, app, and exact-path cleanup modes
|-- tests/
|   |-- e2e.sh                  # Namespace test dependency check and runner
|   |-- e2e/
|   |   |-- namespace.sh        # Filesystem, CLI, locking, and failure regressions
|   |   `-- ownership.sh        # Subordinate-ID root/non-root ownership regressions
|   `-- scale.sh                # 100,000/500,000-entry cleanup regressions
`-- .github/workflows/ci.yml    # Formatting, lint, unit, and end-to-end CI
```

## Build

Requirements:

- Rust 1.85 or newer
- Linux or Android
- GNU `/bin/rm` for deliberate permanent cleanup and exception bypasses

```bash
cargo build --release
```

The output is `target/release/trash`.

## Install

Install with symbolic links so rebuilding the release binary updates both commands:

```bash
mkdir -p "$HOME/.local/bin"
ln -sfn "$PWD/target/release/trash" "$HOME/.local/bin/trash"
ln -sfn "$PWD/target/release/trash" "$HOME/.local/bin/rm"
```

Using the direct `rm` symlink affects processes that resolve `rm` through
`~/.local/bin`. For an interactive-shell-only replacement, link only `trash` and
load the relevant integration instead.

### Bash

Source [`integrations/bash/trash.bash`](integrations/bash/trash.bash) from
`~/.bashrc`:

```bash
source /home/lewis/Dev/trash/integrations/bash/trash.bash
```

### Fish

Source [`integrations/fish/trash.fish`](integrations/fish/trash.fish) from
`~/.config/fish/config.fish`:

```fish
source /home/lewis/Dev/trash/integrations/fish/trash.fish
```

Both integrations redirect interactive `rm`. They also route the exact
`sudo rm ...` form to `sudo /home/lewis/.local/bin/trash ...`; the root-owned trash
validation still applies.

## Usage

```bash
rm file.txt directory
rm -rf --one-file-system build-cache
trash --clean
trash -f --clean '/trash/(agy)run/payload/file.txt'
trash -f --clean-app agy codex
trash --help
```

Directories do not require `-r`; they are renamed as complete trees. Shell globbing
still happens before `trash` runs, so `*` includes exactly the paths selected by the
calling shell.

## Configuration

`TRASH_SUBDIR` selects one safe directory name at each mountpoint. The default is
`trash`, producing `/trash` for the root filesystem and `MOUNTPOINT/trash` elsewhere.
A custom root requires a private `.trash-root` marker.

`TRASH_EXCEPTIONS` adds whitespace-separated exact process names to the permanent
bypass list:

```bash
export TRASH_EXCEPTIONS="custom-cleaner another-cleaner"
```

Built-in exceptions are `paru`, `makepkg`, `yay`, and `trigger.sh`. `netmgr` is a
hardcoded exception. An exception executes `/bin/rm` with the original arguments;
substring matches such as `par` versus `paru` are never used.

## Operation Pipelines

Normal movement receives `rm`-style options and path operands. It executes in this
order:

1. Validate configuration and inspect the parent process chain.
2. Execute `/bin/rm` only when an exact exception is found.
3. Parse options and fail closed on unsupported input.
4. Resolve each source against `/proc/self/mountinfo` and reject mountpoints or trash
   contents.
5. Validate or create the mountpoint's trusted trash root and acquire a shared lock.
6. Create a private run, atomically rename each item without replacement, and append
   its source/destination record.
7. Remove empty runs created by skipped or failed operations.

Cleanup receives the current working directory plus either no operands, exact trash
paths, or exact app names. It validates the relevant root, acquires an exclusive
lock, obtains typed confirmation unless `--force` is present, and invokes
`/bin/rm --one-file-system -rf --` in bounded batches. Cleanup never substitutes a
database for the files stored under trash.

## Inputs and Outputs

Inputs are command-line options, path operands, `PWD`, `TRASH_SUBDIR`,
`TRASH_EXCEPTIONS`, `/proc` process data, and `/proc/self/mountinfo`.

Outputs are exit status and diagnostics plus one run directory per touched
mountpoint:

```text
MOUNTPOINT/trash/
|-- .trash.lock
`-- (app)name-2026-08-31_12-00-00-pid-1234.A1b2C3d4/
    |-- metadata.json
    |-- items.jsonl
    `-- payload/
        `-- original-name
```

Raw files and directories remain directly recoverable from `payload/`; no database
is required to read or restore them. See [the metadata contract](docs/metadata-format.md).

## Scope

The tool only affects commands that resolve to this binary or an interactive alias.
Programs using `unlink(2)`, language filesystem APIs, or an absolute `/bin/rm` path
still delete permanently. Package managers and build tools may bypass shell aliases.

## Testing

Run checks in this order:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo build
shellcheck tests/e2e.sh tests/e2e/*.sh tests/scale.sh integrations/bash/trash.bash reference/trash.bash
shfmt -d tests/e2e.sh tests/e2e/*.sh tests/scale.sh integrations/bash/trash.bash
env -u TRASH_EXCEPTIONS ./tests/e2e.sh
cargo build --release
TRASH_BIN="$PWD/target/release/trash" ./tests/scale.sh
```

The end-to-end suite requires `unshare`, subordinate UID/GID mappings, `mount`,
`flock`, `script`, `setpriv`, `jq`, and GNU file utilities. It mounts temporary
filesystems inside private user/mount namespaces and does not modify the host
`/trash` directory. The scale suite checks name-based cleanup at 100,000 and 500,000
entries, metadata-based cleanup at 100,000 entries, and whole-root cleanup at 100,000
entries. `TRASH_SCALE_MAX_MS` changes its default 60-second per-cleanup ceiling.

## Security

Cleanup permanently deletes data. Unsupported options do not fall through to
`/bin/rm`; only cleanup and exact process exceptions cross that boundary. Review
[the security model](docs/security-model.md) before changing root validation,
mountpoint detection, locking, or cleanup behavior.
