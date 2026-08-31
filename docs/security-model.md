# Security Model

## Protected Assets

- Source paths that the user intended to recover later.
- Files outside the selected trash root.
- Mounted filesystems nested below a trash target.
- Trash control files and active move runs.
- The distinction between recoverable movement and permanent deletion.

## Trust Boundary

A trash root must be a real directory on the expected filesystem, owned by the
effective user, and not writable by group or other users. Root executions require a
root-owned trash root. Custom roots additionally require a regular, private
`.trash-root` marker. Android emulated-storage permission checks retain their platform
exception.

The root remains raw filesystem storage. Manually adding or deleting run directories
does not corrupt an index because there is no database. Malformed or blank metadata
cannot trigger an app match; a valid exact `(app)` folder prefix can still match.

## Safety Invariants

1. A failed trash move never falls back to permanent deletion.
2. A filesystem mountpoint is never accepted as a normal trash target.
3. A path already inside its expected trash root is never nested into another run.
4. Cleanup never follows a final symlink supplied as an exact path.
5. Exact-path cleanup validates all targets before deleting any of them.
6. Cleanup preserves `.trash.lock`, `.trash-root`, and nested mountpoints.
7. Unsupported options exit with status 2 instead of reaching `/bin/rm`.
8. Process exception and `--clean-app` matching use complete application names.
9. Cleanup and active moves cannot overlap because their locks conflict.
10. Permanent cleanup passes `--one-file-system` to `/bin/rm`.

## Permanent Deletion Boundary

Only two paths deliberately execute `/bin/rm`:

- An exact process exception receives the original command arguments unchanged.
- A validated cleanup operation receives only selected trash paths and fixed safety
  options.

The small `permanent.rs` module centralizes cleanup invocation. Exception execution is
visible in `lib.rs` because it replaces the current process.

## Out of Scope

- Protecting commands that call `/bin/rm`, `unlink(2)`, or language filesystem APIs.
- Defending against the same effective user deliberately replacing files while the
  command runs.
- Recovering data after an explicit cleanup or process-exception bypass.
- Providing cryptographic integrity or tamper evidence for metadata.
