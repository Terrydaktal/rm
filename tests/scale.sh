#!/bin/bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
tool="${TRASH_BIN:-$repo_dir/target/release/trash}"
max_cleanup_ms="${TRASH_SCALE_MAX_MS:-60000}"

if [ ! -x "$tool" ]; then
	echo "scale: binary is not executable: $tool" >&2
	echo "scale: run 'cargo build --release' first or set TRASH_BIN" >&2
	exit 2
fi

for dependency in unshare mount xargs; do
	if ! command -v "$dependency" >/dev/null 2>&1; then
		echo "scale: missing dependency: $dependency" >&2
		exit 2
	fi
done

unshare -Urnm bash -s -- "$tool" "$max_cleanup_ms" <<'SCALE_TEST_SCRIPT'
set -euo pipefail
mount -t tmpfs -o size=2g,nr_inodes=1500000 tmpfs /tmp

tool="$1"
max_cleanup_ms="$2"
jobs=$(nproc)
if [ "$jobs" -gt 8 ]; then
    jobs=8
fi

run_scale_case() {
    local count="$1"
    local app="scale$count"
    local before after start end elapsed_ms

    mkdir -p -m 0700 /tmp/trash
    seq 1 "$count" | xargs -P "$jobs" -n 1000 bash -c '
        app="$1"
        shift
        paths=()
        for index; do
            paths+=("/tmp/trash/($app)run-$index")
        done
        mkdir -- "${paths[@]}"
    ' bash "$app"

    before=$(find /tmp/trash -mindepth 1 -maxdepth 1 -type d | wc -l)
    start=$(date +%s%N)
    (cd /tmp && "$tool" -f --clean-app "$app")
    end=$(date +%s%N)
    after=$(find /tmp/trash -mindepth 1 -maxdepth 1 -type d | wc -l)
    elapsed_ms=$(((end - start) / 1000000))

    if [ "$before" -ne "$count" ] || [ "$after" -ne 0 ]; then
        printf 'FAIL scale_%s_count before=%s after=%s\n' "$count" "$before" "$after" >&2
        return 1
    fi
    if [ "$elapsed_ms" -gt "$max_cleanup_ms" ]; then
        printf 'FAIL scale_%s_time elapsed_ms=%s limit_ms=%s\n' "$count" "$elapsed_ms" "$max_cleanup_ms" >&2
        return 1
    fi
    printf 'PASS scale_%s entries=%s elapsed_ms=%s limit_ms=%s\n' \
        "$count" "$count" "$elapsed_ms" "$max_cleanup_ms"
}

run_metadata_scale_case() {
    local count="$1"
    local app="metascale$count"
    local before after start end elapsed_ms

    mkdir -p -m 0700 /tmp/trash
    seq 1 "$count" | xargs -P "$jobs" -n 1000 bash -c '
        app="$1"
        shift
        paths=()
        for index; do
            paths+=("/tmp/trash/(other)metadata-$index")
        done
        mkdir -- "${paths[@]}"
        for index; do
            run="/tmp/trash/(other)metadata-$index"
            printf "{\"invoked_by\":\"bash <- %s <- systemd\"}\n" "$app" >"$run/metadata.json"
        done
    ' bash "$app"

    before=$(find /tmp/trash -mindepth 1 -maxdepth 1 -type d | wc -l)
    start=$(date +%s%N)
    (cd /tmp && "$tool" -f --clean-app "$app")
    end=$(date +%s%N)
    after=$(find /tmp/trash -mindepth 1 -maxdepth 1 -type d | wc -l)
    elapsed_ms=$(((end - start) / 1000000))

    if [ "$before" -ne "$count" ] || [ "$after" -ne 0 ]; then
        printf 'FAIL metadata_scale_%s_count before=%s after=%s\n' "$count" "$before" "$after" >&2
        return 1
    fi
    if [ "$elapsed_ms" -gt "$max_cleanup_ms" ]; then
        printf 'FAIL metadata_scale_%s_time elapsed_ms=%s limit_ms=%s\n' "$count" "$elapsed_ms" "$max_cleanup_ms" >&2
        return 1
    fi
    printf 'PASS metadata_scale_%s entries=%s elapsed_ms=%s limit_ms=%s\n' \
        "$count" "$count" "$elapsed_ms" "$max_cleanup_ms"
}

run_full_clean_scale_case() {
    local count="$1"
    local before after start end elapsed_ms

    mkdir -p -m 0700 /tmp/trash
    seq 1 "$count" | xargs -P "$jobs" -n 1000 bash -c '
        paths=()
        for index; do
            paths+=("/tmp/trash/full-run-$index")
        done
        mkdir -- "${paths[@]}"
    ' bash

    before=$(find /tmp/trash -mindepth 1 -maxdepth 1 -type d | wc -l)
    start=$(date +%s%N)
    (cd /tmp && "$tool" -f --clean)
    end=$(date +%s%N)
    after=$(find /tmp/trash -mindepth 1 -maxdepth 1 -type d | wc -l)
    elapsed_ms=$(((end - start) / 1000000))

    if [ "$before" -ne "$count" ] || [ "$after" -ne 0 ]; then
        printf 'FAIL full_clean_scale_%s_count before=%s after=%s\n' "$count" "$before" "$after" >&2
        return 1
    fi
    if [ "$elapsed_ms" -gt "$max_cleanup_ms" ]; then
        printf 'FAIL full_clean_scale_%s_time elapsed_ms=%s limit_ms=%s\n' "$count" "$elapsed_ms" "$max_cleanup_ms" >&2
        return 1
    fi
    printf 'PASS full_clean_scale_%s entries=%s elapsed_ms=%s limit_ms=%s\n' \
        "$count" "$count" "$elapsed_ms" "$max_cleanup_ms"
}

run_scale_case 100000
run_scale_case 500000
run_metadata_scale_case 100000
run_full_clean_scale_case 100000
SCALE_TEST_SCRIPT
