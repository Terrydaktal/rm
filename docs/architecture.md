# Architecture

## Goals

The implementation separates syntax handling from filesystem authority. Code that
parses untrusted command input cannot permanently delete data, and code that invokes
`/bin/rm` receives only paths already selected by a cleanup module.

## Modules

- `cli.rs` parses the supported GNU `rm` subset and owns help text.
- `config.rs` validates environment configuration and builds exact process exceptions.
- `process_chain.rs` walks `/proc` without spawning subprocesses.
- `platform/linux.rs` parses mountinfo and wraps `renameat2(RENAME_NOREPLACE)`.
- `trash/root.rs` locates, creates, and validates trusted trash roots.
- `trash/locking.rs` holds shared move locks or exclusive cleanup locks.
- `trash/run.rs` creates one private run for each affected mountpoint.
- `trash/metadata.rs` serializes audit records and parses cleanup metadata.
- `trash/move_item.rs` validates and atomically moves individual operands.
- `trash/cleanup/all.rs` streams direct entries through bounded deletion batches.
- `trash/cleanup/apps.rs` performs one direct scan and exact app matching.
- `trash/cleanup/paths.rs` canonicalizes and validates all requested paths first.
- `permanent.rs` is the single normal cleanup gateway to `/bin/rm`.

## Move Pipeline

`lib.rs` validates configuration before reading process ancestry. An exact exception
replaces the current process with `/bin/rm`; otherwise the CLI parser selects normal
movement or one cleanup mode.

Normal movement creates `Metadata` once and a `Run` lazily for each mountpoint. Every
run owns its shared lock and open manifest. `move_item` rejects unsafe sources before
creating a run, chooses a collision-free payload destination, and uses an atomic
no-replace rename. A successful rename is not reported until the source no longer
exists and its manifest record has been attempted.

## Cleanup Pipeline

All cleanup modes resolve the trash root from the current filesystem and validate it
before acquiring an exclusive lock.

- Whole-root cleanup confirms once and streams direct non-control entries in batches.
- App cleanup scans direct directories once, compares precomputed `(app)` prefixes,
  and parses `metadata.json` only for non-name matches.
- Path cleanup canonicalizes parent directories without following final symlinks,
  validates every target, deduplicates paths, confirms, then deletes.

Permanent removal is always `/bin/rm --one-file-system -rf -- PATH...`. Batches are
limited to 4,096 paths and half of the platform `ARG_MAX`, capped at 1 MiB.

## Concurrency

The `.trash.lock` file uses advisory `flock` semantics. Move invocations hold shared
locks for the lifetime of their runs. Cleanup holds an exclusive lock from validation
through deletion. Lock files are opened with `O_NOFOLLOW | O_CLOEXEC`.

## Platform Boundary

Linux mountpoints come from `/proc/self/mountinfo`, not device IDs alone. This detects
bind mounts that share a device number. Android root placement retains the previous
special paths for emulated storage and may use `mv` only when a rename reports
`EXDEV`.
