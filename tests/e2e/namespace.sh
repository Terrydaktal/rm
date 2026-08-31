#!/bin/bash
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
first_metadata=$(find /tmp/trash -name metadata.json -type f -print -quit)
first_manifest=$(find /tmp/trash -name items.jsonl -type f -print -quit)
first_payload=$(find /tmp/trash -path '*/payload/one' -type f -print -quit)
check metadata_is_json jq empty "$first_metadata"
check manifest_is_json jq empty "$first_manifest"
# shellcheck disable=SC2016 # $source belongs to jq.
check metadata_records_original_command jq -e --arg source "$work/one" '.command | contains($source)' "$first_metadata"
# shellcheck disable=SC2016 # $cwd belongs to jq.
check metadata_records_working_directory jq -e --arg cwd "$PWD" '.cwd == $cwd' "$first_metadata"
check metadata_records_process_chain jq -e '.invoked_by | length > 0' "$first_metadata"
# shellcheck disable=SC2016 # $source belongs to jq.
check manifest_records_exact_source jq -e --arg source "$work/one" '.source == $source' "$first_manifest"
# shellcheck disable=SC2016 # $destination belongs to jq.
check manifest_records_existing_destination jq -e --arg destination "$first_payload" '.destination == $destination' "$first_manifest"

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
printf protected >/tmp/trash/nested-mount/protected
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
printf hidden >/tmp/custom-root/.hidden
(cd /tmp && TRASH_SUBDIR=custom-root "$tool" -f --clean >/dev/null)
check custom_root_preserves_marker test -f /tmp/custom-root/.trash-root
check clean_removes_hidden_entries test ! -e /tmp/custom-root/.hidden

app=auditvictim
app2=secondvictim
app3=thirdvictim
mkdir -p /tmp/trash/'(other)run' /tmp/trash/'(auditvictim)run' /tmp/trash/'(secondvictim)run' /tmp/trash/'(third-other)run' "$work/$app"
printf '{\n  "invoked_by": "auditvictim"\n}\n' >/tmp/trash/'(other)run'/metadata.json
printf '{\n  "invoked_by": "other"\n}\n' >/tmp/trash/'(auditvictim)run'/metadata.json
printf '{\n  "invoked_by": "other"\n}\n' >/tmp/trash/'(secondvictim)run'/metadata.json
printf '{\n  "invoked_by": "shell <- thirdvictim <- systemd"\n}\n' >/tmp/trash/'(third-other)run'/metadata.json
printf matching >/tmp/trash/'(auditvictim)run'/payload
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
' >/tmp/trash/'(paru)run'/metadata.json
(cd /tmp && "$tool" -f --clean-app par >/dev/null)
check clean_app_requires_exact_app_name test -e /tmp/trash/'(paru)run'

set +e
"$tool" >/dev/null 2>&1
status=$?
set -e
check no_operand_matches_rm_failure test "$status" -eq 1
"$tool" -f >/dev/null
check force_without_operands_matches_rm_success test "$?" -eq 0

printf outside-link-target >"$work/outside-link-target"
ln -s "$work/outside-link-target" /tmp/trash/inside-trash-link
set +e
"$tool" -f /tmp/trash/inside-trash-link >/dev/null 2>&1
status=$?
set -e
check normal_rm_refuses_symlink_inside_trash test "$status" -eq 1
check refused_inside_symlink_remains test -L /tmp/trash/inside-trash-link
/bin/rm -f /tmp/trash/inside-trash-link

mkdir -m 0777 /tmp/unsafe-root
printf unsafe >"$work/unsafe-source"
set +e
TRASH_SUBDIR=unsafe-root "$tool" "$work/unsafe-source" >/dev/null 2>&1
status=$?
set -e
check unsafe_root_mode_is_refused test "$status" -eq 1
check unsafe_root_source_remains test -f "$work/unsafe-source"
/bin/rm -rf /tmp/unsafe-root

mkdir -m 0700 /tmp/marker-root
ln -s "$work/outside-link-target" /tmp/marker-root/.trash-root
printf marker >"$work/marker-source"
set +e
TRASH_SUBDIR=marker-root "$tool" "$work/marker-source" >/dev/null 2>&1
status=$?
set -e
check symlinked_custom_marker_is_refused test "$status" -eq 1
check marker_source_remains test -f "$work/marker-source"
/bin/rm -rf /tmp/marker-root

/bin/rm -f /tmp/trash/.trash.lock
ln -s "$work/outside-link-target" /tmp/trash/.trash.lock
printf lock >"$work/lock-source"
set +e
"$tool" "$work/lock-source" >/dev/null 2>&1
status=$?
set -e
check symlinked_lock_is_refused test "$status" -eq 1
check lock_source_remains test -f "$work/lock-source"
/bin/rm -f /tmp/trash/.trash.lock
: >/tmp/trash/.trash.lock
chmod 600 /tmp/trash/.trash.lock

mkdir -p /tmp/trash/'(blankvictim)run' /tmp/trash/'(blank-other)run'
: >/tmp/trash/'(blankvictim)run'/metadata.json
: >/tmp/trash/'(blank-other)run'/metadata.json
(cd /tmp && "$tool" -f --clean-app blankvictim >/dev/null)
check clean_app_name_match_ignores_blank_metadata test ! -e /tmp/trash/'(blankvictim)run'
check clean_app_blank_metadata_without_name_is_preserved test -d /tmp/trash/'(blank-other)run'

printf exception >"$work/exception-source"
cat >"$work/netmgr" <<EOF
#!/bin/bash
"$tool" "\$1"
EOF
chmod +x "$work/netmgr"
before_count=$(find /tmp/trash -path '*/payload/exception-source' | wc -l)
"$work/netmgr" "$work/exception-source"
after_count=$(find /tmp/trash -path '*/payload/exception-source' | wc -l)
check hardcoded_exception_permanently_removes_source test ! -e "$work/exception-source"
check hardcoded_exception_does_not_create_trash_payload test "$after_count" -eq "$before_count"

mkdir -p "$work/bind-source" /tmp/trash/bind-mount
printf protected >"$work/bind-source/protected"
printf removable >/tmp/trash/safe-sibling
mount --bind "$work/bind-source" /tmp/trash/bind-mount
set +e
(cd /tmp && "$tool" -f --clean >/dev/null 2>&1)
status=$?
set -e
check same_device_bind_mount_blocks_full_clean test "$status" -eq 1
check same_device_bind_mount_contents_survive test -f /tmp/trash/bind-mount/protected
check full_clean_removes_safe_sibling_before_reporting_nested_mount test ! -e /tmp/trash/safe-sibling
umount /tmp/trash/bind-mount
rmdir /tmp/trash/bind-mount

help_output=$("$tool" --help)
check help_reports_name_section grep -q '^NAME$' <<<"$help_output"
check help_reports_security_section grep -q '^SECURITY NOTES$' <<<"$help_output"
version_output=$("$tool" --version)
check version_reports_current_release test "$version_output" = "trash 0.4.0"

printf verbose >"$work/verbose-source"
verbose_output=$("$tool" -v "$work/verbose-source")
check verbose_reports_source grep -Fq "trashed '$work/verbose-source' ->" <<<"$verbose_output"

printf once-positive >"$work/once-positive"
printf 'y\n' | "$tool" -I -r "$work/once-positive" >/dev/null
check interactive_once_accepts test ! -e "$work/once-positive"
printf once-negative >"$work/once-negative"
set +e
printf 'n\n' | "$tool" -I -r "$work/once-negative" >/dev/null 2>&1
status=$?
set -e
check interactive_once_refusal_returns_failure test "$status" -eq 1
check interactive_once_refusal_preserves_source test -e "$work/once-negative"

printf always-negative >"$work/always-negative"
printf 'n\n' | "$tool" --interactive=always "$work/always-negative" >/dev/null
check interactive_always_refusal_preserves_source test -e "$work/always-negative"
printf never-prompts >"$work/never-prompts"
printf 'n\n' | "$tool" --interactive=never "$work/never-prompts" >/dev/null
check interactive_never_ignores_stdin test ! -e "$work/never-prompts"

printf compatible >"$work/compatible"
"$tool" -Rd --preserve-root "$work/compatible" >/dev/null
check recursive_dir_and_preserve_root_are_accepted test ! -e "$work/compatible"

for invalid_command in \
	"--clean --clean-app app" \
	"--clean-app" \
	"--clean-app=" \
	"--clean-app bad/name" \
	"--unknown"; do
	read -r -a invalid_args <<<"$invalid_command"
	set +e
	"$tool" "${invalid_args[@]}" >/dev/null 2>&1
	status=$?
	set -e
	check "invalid_usage_${invalid_command//[^A-Za-z0-9]/_}" test "$status" -eq 2
done

before_runs=$(find /tmp/trash -mindepth 1 -maxdepth 1 -type d | wc -l)
set +e
"$tool" "$work/missing-source" >/dev/null 2>&1
missing_status=$?
set -e
"$tool" -f "$work/missing-source" >/dev/null
after_runs=$(find /tmp/trash -mindepth 1 -maxdepth 1 -type d | wc -l)
check missing_source_without_force_fails test "$missing_status" -eq 1
check missing_source_does_not_create_empty_run test "$after_runs" -eq "$before_runs"

ln -s nowhere "$work/broken-link"
"$tool" "$work/broken-link" >/dev/null
broken_payload=$(find /tmp/trash -path '*/payload/broken-link' -type l -print -quit)
check broken_symlink_is_moved test -n "$broken_payload"
check broken_symlink_target_is_preserved test "$(readlink "$broken_payload")" = nowhere

non_utf8_name=$'non-utf8-\xff'
printf nonutf8 >"$work/$non_utf8_name"
"$tool" "$work/$non_utf8_name" >/dev/null
check non_utf8_source_is_moved test ! -e "$work/$non_utf8_name"
check non_utf8_payload_name_is_preserved test -n "$(find /tmp/trash -path "*/payload/$non_utf8_name" -print -quit)"

printf hardlink >"$work/hardlink-a"
ln "$work/hardlink-a" "$work/hardlink-b"
"$tool" "$work/hardlink-a" "$work/hardlink-b" >/dev/null
hardlink_a=$(find /tmp/trash -path '*/payload/hardlink-a' -print -quit)
hardlink_b=$(find /tmp/trash -path '*/payload/hardlink-b' -print -quit)
check hardlink_identity_is_preserved test "$(stat -c %i "$hardlink_a")" = "$(stat -c %i "$hardlink_b")"

mkfifo "$work/fifo-source"
"$tool" "$work/fifo-source" >/dev/null
check fifo_type_is_preserved test -p "$(find /tmp/trash -path '*/payload/fifo-source' -print -quit)"

printf mode >"$work/mode-source"
chmod 0640 "$work/mode-source"
"$tool" "$work/mode-source" >/dev/null
mode_payload=$(find /tmp/trash -path '*/payload/mode-source' -print -quit)
check file_mode_is_preserved test "$(stat -c %a "$mode_payload")" = 640
check file_owner_is_preserved test "$(stat -c %u "$mode_payload")" = "$(id -u)"

ln -s /tmp "$work/mountpoint-link"
"$tool" "$work/mountpoint-link" >/dev/null
mountpoint_link_payload=$(find /tmp/trash -path '*/payload/mountpoint-link' -type l -print -quit)
check symlink_to_mountpoint_is_moved test -n "$mountpoint_link_payload"
check symlink_to_mountpoint_does_not_affect_target test -d /tmp

ln -s "$work" /tmp/trash/intermediate-link
set +e
(cd /tmp && "$tool" -f --clean /tmp/trash/intermediate-link/outside-clean >/dev/null 2>&1)
status=$?
set -e
check symlinked_intermediate_cleanup_path_is_refused test "$status" -eq 1
check symlinked_intermediate_cleanup_preserves_outside_file test -f "$work/outside-clean"
/bin/rm -f /tmp/trash/intermediate-link

: >/tmp/trash/.trash-root
set +e
(cd /tmp && "$tool" -f --clean /tmp/trash/.trash-root >/dev/null 2>&1)
status=$?
set -e
check exact_path_cleanup_protects_marker test "$status" -eq 1
check exact_path_cleanup_preserves_marker test -f /tmp/trash/.trash-root
/bin/rm -f /tmp/trash/.trash-root

mkdir -m 0700 /tmp/missing-marker-root
printf missing-marker >"$work/missing-marker-source"
set +e
TRASH_SUBDIR=missing-marker-root "$tool" "$work/missing-marker-source" >/dev/null 2>&1
status=$?
set -e
check custom_root_without_marker_is_refused test "$status" -eq 1
check custom_root_without_marker_preserves_source test -e "$work/missing-marker-source"
/bin/rm -rf /tmp/missing-marker-root

mkdir -m 0700 /tmp/public-marker-root
: >/tmp/public-marker-root/.trash-root
chmod 0644 /tmp/public-marker-root/.trash-root
printf public-marker >"$work/public-marker-source"
set +e
TRASH_SUBDIR=public-marker-root "$tool" "$work/public-marker-source" >/dev/null 2>&1
status=$?
set -e
check public_custom_marker_is_refused test "$status" -eq 1
check public_custom_marker_preserves_source test -e "$work/public-marker-source"
/bin/rm -rf /tmp/public-marker-root

ln -s "$work" /tmp/symlink-root
printf symlink-root >"$work/symlink-root-source"
set +e
TRASH_SUBDIR=symlink-root "$tool" "$work/symlink-root-source" >/dev/null 2>&1
status=$?
set -e
check symlinked_trash_root_is_refused test "$status" -eq 1
check symlinked_trash_root_preserves_source test -e "$work/symlink-root-source"
/bin/rm -f /tmp/symlink-root

printf not-a-directory >/tmp/file-root
printf file-root >"$work/file-root-source"
set +e
TRASH_SUBDIR=file-root "$tool" "$work/file-root-source" >/dev/null 2>&1
status=$?
set -e
check non_directory_trash_root_is_refused test "$status" -eq 1
check non_directory_trash_root_preserves_source test -e "$work/file-root-source"
/bin/rm -f /tmp/file-root

mkdir -p /tmp/mount-a /tmp/mount-b
mount -t tmpfs tmpfs /tmp/mount-a
mount -t tmpfs tmpfs /tmp/mount-b
printf mount-a >/tmp/mount-a/source-a
printf mount-b >/tmp/mount-b/source-b
"$tool" /tmp/mount-a/source-a /tmp/mount-b/source-b >/dev/null
check multi_mount_first_source_moves test ! -e /tmp/mount-a/source-a
check multi_mount_second_source_moves test ! -e /tmp/mount-b/source-b
check multi_mount_first_payload_exists test -n "$(find /tmp/mount-a/trash -path '*/payload/source-a' -print -quit)"
check multi_mount_second_payload_exists test -n "$(find /tmp/mount-b/trash -path '*/payload/source-b' -print -quit)"
check multi_mount_creates_one_run_per_mount test "$(find /tmp/mount-a/trash /tmp/mount-b/trash -mindepth 1 -maxdepth 1 -type d | wc -l)" -eq 2
umount /tmp/mount-a /tmp/mount-b
rmdir /tmp/mount-a /tmp/mount-b

mkdir -p '/tmp/mount with space'
mount -t tmpfs tmpfs '/tmp/mount with space'
printf spaced >'/tmp/mount with space/spaced-source'
"$tool" '/tmp/mount with space/spaced-source' >/dev/null
check escaped_mountinfo_path_is_resolved test -n "$(find '/tmp/mount with space/trash' -path '*/payload/spaced-source' -print -quit)"
umount '/tmp/mount with space'
rmdir '/tmp/mount with space'

mkdir -p /tmp/readonly-fs
mount -t tmpfs tmpfs /tmp/readonly-fs
mkdir -m 0700 /tmp/readonly-fs/trash
printf readonly >/tmp/readonly-fs/readonly-source
mount -o remount,ro,uid=0,gid=0 tmpfs /tmp/readonly-fs
set +e
"$tool" /tmp/readonly-fs/readonly-source >/dev/null 2>&1
status=$?
set -e
check readonly_filesystem_move_fails test "$status" -eq 1
check readonly_filesystem_failure_preserves_source test -e /tmp/readonly-fs/readonly-source
mount -o remount,rw,uid=0,gid=0 tmpfs /tmp/readonly-fs
umount /tmp/readonly-fs
rmdir /tmp/readonly-fs

mkdir -p /tmp/full-fs
mount -t tmpfs -o size=1m,nr_inodes=64 tmpfs /tmp/full-fs
mkdir -m 0700 /tmp/full-fs/trash
printf full >/tmp/full-fs/full-source
fill_index=0
while touch "/tmp/full-fs/fill-$fill_index" 2>/dev/null; do
	fill_index=$((fill_index + 1))
done
set +e
"$tool" /tmp/full-fs/full-source >/dev/null 2>&1
status=$?
set -e
check inode_exhaustion_move_fails test "$status" -eq 1
check inode_exhaustion_preserves_source test -e /tmp/full-fs/full-source
umount /tmp/full-fs
rmdir /tmp/full-fs

for app_label in codex agy; do
	printf label >"$work/$app_label-label-source"
	cat >"$work/$app_label" <<EOF
#!/bin/bash
"$tool" "\$1"
EOF
	chmod +x "$work/$app_label"
	"$work/$app_label" "$work/$app_label-label-source"
	check "${app_label}_process_chain_labels_run" test -n "$(find /tmp/trash -mindepth 1 -maxdepth 1 -type d -name "($app_label)*" -print -quit)"
done

printf configured-exception >"$work/configured-exception-source"
cat >"$work/custom-cleaner" <<EOF
#!/bin/bash
"$tool" "\$1"
EOF
chmod +x "$work/custom-cleaner"
before_count=$(find /tmp/trash -path '*/payload/configured-exception-source' | wc -l)
TRASH_EXCEPTIONS=custom-cleaner "$work/custom-cleaner" "$work/configured-exception-source"
after_count=$(find /tmp/trash -path '*/payload/configured-exception-source' | wc -l)
check configured_exception_permanently_removes_source test ! -e "$work/configured-exception-source"
check configured_exception_creates_no_payload test "$after_count" -eq "$before_count"

exec {exclusive_lock_fd}>>/tmp/trash/.trash.lock
flock -x "$exclusive_lock_fd"
printf blocked >"$work/blocked-move"
"$tool" "$work/blocked-move" >/dev/null 2>&1 &
blocked_move_pid=$!
sleep 0.2
check exclusive_cleanup_lock_blocks_move kill -0 "$blocked_move_pid"
check blocked_move_has_not_changed_source test -e "$work/blocked-move"
flock -u "$exclusive_lock_fd"
wait "$blocked_move_pid"
exec {exclusive_lock_fd}>&-
check blocked_move_completes_after_unlock test ! -e "$work/blocked-move"

printf cleanup-lock >"$work/cleanup-lock-source"
"$tool" "$work/cleanup-lock-source" >/dev/null
cleanup_lock_target=$(find /tmp/trash -path '*/payload/cleanup-lock-source' -print -quit)
exec {shared_lock_fd}>>/tmp/trash/.trash.lock
flock -s "$shared_lock_fd"
(cd /tmp && "$tool" -f --clean "$cleanup_lock_target" >/dev/null 2>&1) &
blocked_cleanup_pid=$!
sleep 0.2
check active_move_lock_blocks_cleanup kill -0 "$blocked_cleanup_pid"
check blocked_cleanup_has_not_deleted_target test -e "$cleanup_lock_target"
flock -u "$shared_lock_fd"
wait "$blocked_cleanup_pid"
exec {shared_lock_fd}>&-
check blocked_cleanup_completes_after_unlock test ! -e "$cleanup_lock_target"

exec {interrupt_lock_fd}>>/tmp/trash/.trash.lock
flock -x "$interrupt_lock_fd"
printf interrupted >"$work/interrupted-source"
"$tool" "$work/interrupted-source" >/dev/null 2>&1 &
interrupted_pid=$!
sleep 0.2
kill "$interrupted_pid"
set +e
wait "$interrupted_pid"
status=$?
set -e
flock -u "$interrupt_lock_fd"
exec {interrupt_lock_fd}>&-
check interrupted_waiter_returns_failure test "$status" -ne 0
check interrupted_waiter_preserves_source test -e "$work/interrupted-source"

parallel_pids=()
for parallel_index in $(seq 1 12); do
	printf parallel >"$work/parallel-$parallel_index"
	"$tool" "$work/parallel-$parallel_index" >/dev/null 2>&1 &
	parallel_pids+=("$!")
done
parallel_status=0
for parallel_pid in "${parallel_pids[@]}"; do
	wait "$parallel_pid" || parallel_status=1
done
check parallel_shared_lock_moves_succeed test "$parallel_status" -eq 0
check parallel_shared_lock_moves_all_sources test "$(find "$work" -maxdepth 1 -name 'parallel-*' | wc -l)" -eq 0

mkdir -p /tmp/trash/prompt-preserve
set +e
printf 'WRONG\n' | script -qefc "cd /tmp && '$tool' --clean" /dev/null >/dev/null 2>&1
status=$?
set -e
check cleanup_tty_wrong_confirmation_fails test "$status" -eq 1
check cleanup_tty_wrong_confirmation_preserves_data test -d /tmp/trash/prompt-preserve
printf '/tmp/trash\n' | script -qefc "cd /tmp && '$tool' --clean" /dev/null >/dev/null 2>&1
check cleanup_tty_exact_confirmation_succeeds test ! -e /tmp/trash/prompt-preserve

printf 'RESULT pass=%s fail=%s\n' "$pass" "$fail"
[ "$fail" -eq 0 ]
