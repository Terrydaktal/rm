# Contributing

## Development Workflow

1. Build with Rust 1.85 or newer using `cargo build`.
2. Keep parsing, platform, trust, movement, and cleanup responsibilities in their
   existing modules; do not add deletion logic to CLI or metadata code.
3. Add unit tests for pure parsing or serialization changes.
4. Add namespace end-to-end checks for filesystem, lock, mount, or cleanup changes.
5. Run the validation pipeline below before submitting changes.

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

## Safety Requirements

- Never silently fall back to permanent deletion after a failed trash move.
- Keep `/bin/rm` calls centralized and explicit.
- Validate every exact cleanup path before deleting any path in its batch.
- Do not follow a final symlink during movement or exact-path cleanup.
- Treat mountpoint, ownership, permission, marker, and lock regressions as release
  blockers.
- Preserve non-UTF-8 path operands even when diagnostics must display them lossily.

The reference Bash script is frozen. Do not format or edit it unless intentionally
documenting a compatibility correction.
