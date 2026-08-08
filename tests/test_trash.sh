#!/bin/bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
tool="$repo_dir/trash"

for dependency in unshare mount jq; do
    if ! command -v "$dependency" >/dev/null 2>&1; then
        echo "test_trash: missing dependency: $dependency" >&2
        exit 2
    fi
done

unshare -Urnm bash -s -- "$tool" <<'TEST_SCRIPT'
set -euo pipefail
mount -t tmpfs tmpfs /tmp

tool="$1"
work=/tmp/trash-test-work
mkdir -p "$work"
pass=0
fail=0

cleanup() {
    /bin/rm -rf -- "$work" /tmp/trash /tmp/custom-root
}
trap cleanup EXIT

check() {
    local name="$1"
    shift
    if "$@"; then
        printf 'PASS %s\n' "$name"
        pass=$((pass + 1))
    else
        printf 'FAIL %s\n' "$name" >&2
        fail=$((fail + 1))
    fi
}

printf 'one\n' >"$work/one"
"$tool" "$work/one" >/dev/null
check file_moved test ! -e "$work/one"
check payload_layout test "$(find /tmp/trash -path '*/payload/one' -type f | wc -l)" -eq 1
check metadata_is_json jq empty "$(find /tmp/trash -name metadata.json -type f -print -quit)"
check manifest_is_json jq empty "$(find /tmp/trash -name items.jsonl -type f -print -quit)"

printf 'metadata\n' >"$work/metadata.json"
"$tool" "$work/metadata.json" >/dev/null
check metadata_name_is_payload test "$(find /tmp/trash -path '*/payload/metadata.json' -type f | wc -l)" -eq 1

mkdir -p "$work/a" "$work/b"
printf a >"$work/a/item"
printf b >"$work/b/item"
"$tool" "$work/a/item" "$work/b/item" >/dev/null
check duplicate_a_moved test ! -e "$work/a/item"
check duplicate_b_moved test ! -e "$work/b/item"
check duplicate_payloads test "$(find /tmp/trash -path '*/payload/item*' -type f | wc -l)" -eq 2

mkdir -p "$work/not-recursive"
set +e
"$tool" "$work/not-recursive" >/dev/null 2>&1
status=$?
set -e
check directory_requires_recursive test "$status" -eq 1
check directory_not_moved_without_recursive test -d "$work/not-recursive"

mkdir -p "$work/traversal-form"
set +e
"$tool" "$work/traversal-form/../" >/dev/null 2>&1
status=$?
set -e
check traversal_form_is_refused test "$status" -eq 1
check traversal_form_source_remains test -d "$work"

printf unsupported >"$work/unsupported"
set +e
"$tool" --no-preserve-root "$work/unsupported" >/dev/null 2>&1
status=$?
set -e
check unsupported_option_fails_closed test "$status" -eq 2
check unsupported_source_remains test -e "$work/unsupported"

printf interactive >"$work/interactive"
printf 'y\n' | "$tool" -i "$work/interactive" >/dev/null
check interactive_confirmation_moves test ! -e "$work/interactive"

printf unrelated >"$work/unrelated"
exception_name=$(printf 'net%s' mgr)
bash -c '"$0" "$1"; :' "$tool" "$work/unrelated" "$exception_name"
check unrelated_parent_argument_does_not_bypass test ! -e "$work/unrelated"

long_name=$(printf 'x%.0s' $(seq 1 240))
printf long >"$work/$long_name"
"$tool" "$work/$long_name" >/dev/null
check long_name_moves test ! -e "$work/$long_name"

weird="$work/bad"$'\001'cwd
mkdir -p "$weird"
printf control >"$weird/payload"
(cd "$weird" && "$tool" payload >/dev/null)
check control_character_metadata_is_json jq empty "$(find /tmp/trash -name metadata.json -type f -print | tail -n 1)"

set +e
"$tool" -rf /tmp >/dev/null 2>&1
status=$?
set -e
check mountpoint_is_refused test "$status" -eq 1

printf config >"$work/config"
set +e
TRASH_SUBDIR=. "$tool" -f "$work/config" >/dev/null 2>&1
status=$?
set -e
check_dot_subdir() {
    [ "$status" -eq 2 ] && [ -e "$work/config" ]
}
check dot_subdir_is_rejected check_dot_subdir

set +e
(cd /tmp && "$tool" --clean </dev/null >/dev/null 2>&1)
status=$?
set -e
check noninteractive_clean_requires_force test "$status" -eq 1
check noninteractive_clean_preserves_data test -f "$(find /tmp/trash -path '*/payload/one' -type f -print -quit)"

mkdir -p /tmp/trash/nested-mount
mount -t tmpfs tmpfs /tmp/trash/nested-mount
printf protected > /tmp/trash/nested-mount/protected
set +e
(cd /tmp && "$tool" -f --clean >/dev/null 2>&1)
status=$?
set -e
check clean_reports_nested_mount_failure test "$status" -eq 1
check clean_preserves_nested_mount test -f /tmp/trash/nested-mount/protected
umount /tmp/trash/nested-mount
rmdir /tmp/trash/nested-mount

printf custom >"$work/custom"
TRASH_SUBDIR=custom-root "$tool" "$work/custom" >/dev/null
check custom_root_has_marker test -f /tmp/custom-root/.trash-root
printf hidden > /tmp/custom-root/.hidden
(cd /tmp && TRASH_SUBDIR=custom-root "$tool" -f --clean >/dev/null)
check custom_root_preserves_marker test -f /tmp/custom-root/.trash-root
check clean_removes_hidden_entries test ! -e /tmp/custom-root/.hidden

app=auditvictim
mkdir -p /tmp/trash/'(other)run' /tmp/trash/'(auditvictim)run' "$work/$app"
printf '{\n  "invoked_by": "auditvictim"\n}\n' > /tmp/trash/'(other)run'/metadata.json
printf '{\n  "invoked_by": "other"\n}\n' > /tmp/trash/'(auditvictim)run'/metadata.json
printf matching > /tmp/trash/'(auditvictim)run'/payload
printf outside >"$work/$app/payload"
(cd /tmp && "$tool" -f --clean-app "$app" >/dev/null)
check clean_app_does_not_delete_cwd test -d "$work/$app"
check clean_app_deletes_name_match test ! -e /tmp/trash/'(auditvictim)run'
check clean_app_deletes_metadata_match test ! -e /tmp/trash/'(other)run'

mkdir -p /tmp/trash/'(paru)run'/payload
printf '{
  "invoked_by": "paru"
}
' > /tmp/trash/'(paru)run'/metadata.json
(cd /tmp && "$tool" -f --clean-app par >/dev/null)
check clean_app_requires_exact_app_name test -e /tmp/trash/'(paru)run'

printf 'RESULT pass=%s fail=%s\n' "$pass" "$fail"
[ "$fail" -eq 0 ]
TEST_SCRIPT
