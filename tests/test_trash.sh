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
printf nested >"$work/not-recursive/payload"
"$tool" "$work/not-recursive" >/dev/null
check directory_without_recursive_is_moved test ! -e "$work/not-recursive"
check directory_without_recursive_keeps_contents test -f "$(find /tmp/trash -path '*/payload/not-recursive/payload' -type f -print -quit)"

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

mkdir -p "$work/one-file-system"
printf guarded >"$work/one-file-system/payload"
"$tool" -rf --one-file-system "$work/one-file-system" >/dev/null
check one_file_system_is_accepted test ! -e "$work/one-file-system"
check one_file_system_moves_directory test -f "$(find /tmp/trash -path '*/payload/one-file-system/payload' -type f -print -quit)"

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

mkdir -p "/tmp/trash/single run/payload"
printf delete >"/tmp/trash/single run/payload/delete-me"
printf delete >"/tmp/trash/single run/payload/delete-me-too"
printf keep >"/tmp/trash/single run/payload/keep-me"
set +e
(cd /tmp && "$tool" --clean "/tmp/trash/single run/payload/delete-me" "/tmp/trash/single run/payload/delete-me-too" </dev/null >/dev/null 2>&1)
status=$?
set -e
check noninteractive_single_clean_requires_force test "$status" -eq 1
check noninteractive_single_clean_preserves_target test -f "/tmp/trash/single run/payload/delete-me"
(cd /tmp && "$tool" -f --clean "/tmp/trash/single run/payload/delete-me" "/tmp/trash/single run/payload/delete-me-too" "/tmp/trash/single run/payload/delete-me" >/dev/null)
check path_clean_deletes_first_exact_path test ! -e "/tmp/trash/single run/payload/delete-me"
check path_clean_deletes_second_exact_path test ! -e "/tmp/trash/single run/payload/delete-me-too"
check path_clean_preserves_sibling test -f "/tmp/trash/single run/payload/keep-me"

printf outside >"$work/outside-clean"
printf validate-first >"/tmp/trash/single run/payload/validate-first"
set +e
(cd /tmp && "$tool" -f --clean "/tmp/trash/single run/payload/validate-first" "$work/outside-clean" >/dev/null 2>&1)
status=$?
set -e
check single_clean_rejects_outside_path test "$status" -eq 1
check single_clean_preserves_outside_path test -f "$work/outside-clean"
check path_clean_validates_all_before_deleting test -f "/tmp/trash/single run/payload/validate-first"

ln -s "$work/outside-clean" "/tmp/trash/single link"
(cd /tmp && "$tool" -f --clean "/tmp/trash/single link" >/dev/null)
check single_clean_removes_final_symlink test ! -L "/tmp/trash/single link"
check single_clean_does_not_follow_final_symlink test -f "$work/outside-clean"

set +e
(cd /tmp && "$tool" -f --clean /tmp/trash >/dev/null 2>&1)
status=$?
set -e
check single_clean_rejects_trash_root test "$status" -eq 1
check single_clean_preserves_trash_root test -d /tmp/trash

set +e
(cd /tmp && "$tool" -f --clean /tmp/trash/.trash.lock >/dev/null 2>&1)
status=$?
set -e
check single_clean_rejects_lock test "$status" -eq 1
check single_clean_preserves_lock test -f /tmp/trash/.trash.lock

mkdir -p /tmp/trash/nested-mount
mount -t tmpfs tmpfs /tmp/trash/nested-mount
printf protected > /tmp/trash/nested-mount/protected
set +e
(cd /tmp && "$tool" -f --clean /tmp/trash/nested-mount >/dev/null 2>&1)
status=$?
set -e
check single_clean_rejects_nested_mount test "$status" -eq 1
check single_clean_preserves_nested_mount test -f /tmp/trash/nested-mount/protected
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
app2=secondvictim
app3=thirdvictim
mkdir -p /tmp/trash/'(other)run' /tmp/trash/'(auditvictim)run' /tmp/trash/'(secondvictim)run' /tmp/trash/'(third-other)run' "$work/$app"
printf '{\n  "invoked_by": "auditvictim"\n}\n' > /tmp/trash/'(other)run'/metadata.json
printf '{\n  "invoked_by": "other"\n}\n' > /tmp/trash/'(auditvictim)run'/metadata.json
printf '{\n  "invoked_by": "other"\n}\n' > /tmp/trash/'(secondvictim)run'/metadata.json
printf '{\n  "invoked_by": "shell <- thirdvictim <- systemd"\n}\n' > /tmp/trash/'(third-other)run'/metadata.json
printf matching > /tmp/trash/'(auditvictim)run'/payload
printf outside >"$work/$app/payload"
(cd /tmp && "$tool" --clean-app "$app" "$app2" "$app3" -f >/dev/null)
check clean_app_does_not_delete_cwd test -d "$work/$app"
check clean_app_deletes_name_match test ! -e /tmp/trash/'(auditvictim)run'
check clean_app_deletes_metadata_match test ! -e /tmp/trash/'(other)run'
check clean_apps_delete_second_name_match test ! -e /tmp/trash/'(secondvictim)run'
check clean_apps_delete_third_metadata_match test ! -e /tmp/trash/'(third-other)run'

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
