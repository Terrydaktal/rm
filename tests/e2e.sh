#!/bin/bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
tool="${TRASH_BIN:-$repo_dir/target/debug/trash}"

if [ ! -x "$tool" ]; then
	echo "e2e: binary is not executable: $tool" >&2
	echo "e2e: run 'cargo build' first or set TRASH_BIN" >&2
	exit 2
fi

for dependency in unshare mount jq flock script setpriv newuidmap newgidmap; do
	if ! command -v "$dependency" >/dev/null 2>&1; then
		echo "e2e: missing dependency: $dependency" >&2
		exit 2
	fi
done

unshare -Urnm "$repo_dir/tests/e2e/namespace.sh" "$tool"
unshare --map-auto --setuid 0 --setgid 0 -nm "$repo_dir/tests/e2e/ownership.sh" "$tool"
