# Changelog

## 0.4.0 - 2026-08-31

- Replaced the monolithic Bash implementation with a modular Rust binary.
- Added native mount-table parsing and atomic no-replace renames.
- Replaced `find`/`awk`/`xargs` app cleanup with one native directory scan.
- Added bounded cleanup batches and exact metadata process-chain parsing.
- Strengthened same-device bind-mount, symlink-inside-trash, lock-symlink, marker,
  and unsafe-permission protections.
- Moved Bash and Fish integration snippets under `integrations/`.
- Preserved the previous Bash implementation under `reference/`.
- Expanded validation to 36 Rust unit tests, 151 namespace checks with real
  subordinate-ID ownership cases, and automated cleanup tests up to 500,000 entries.

## 0.3.0

- Added multi-path `--clean` and multi-app `--clean-app` cleanup.
- Accepted `--one-file-system` as a compatibility safety option.
- Allowed whole directories to be trashed without `-r`.
- Added exact process exceptions and grouped JSON audit records.
