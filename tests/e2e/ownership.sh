#!/bin/bash
set -euo pipefail
mount -t tmpfs tmpfs /tmp

tool="$1"
pass=0
fail=0

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

mkdir -m 0700 /tmp/trash
chown 1000:1000 /tmp/trash
printf root-rejects-user-owned >/tmp/root-owned-source
set +e
"$tool" /tmp/root-owned-source >/dev/null 2>&1
status=$?
set -e
check root_rejects_user_owned_trash_root test "$status" -eq 1
check root_rejection_preserves_source test -e /tmp/root-owned-source

chown 0:0 /tmp/trash
"$tool" /tmp/root-owned-source >/dev/null
check root_accepts_root_owned_trash_root test ! -e /tmp/root-owned-source
/bin/rm -rf /tmp/trash

mkdir -m 0700 /tmp/trash
chown 1000:1000 /tmp/trash
printf nonroot-success >/tmp/nonroot-source
chown 1000:1000 /tmp/nonroot-source
setpriv --reuid=1000 --regid=1000 --clear-groups "$tool" /tmp/nonroot-source >/dev/null
check nonroot_accepts_own_trash_root test ! -e /tmp/nonroot-source
check nonroot_payload_remains_user_owned test -n "$(find /tmp/trash -path '*/payload/nonroot-source' -uid 1000 -print -quit)"
/bin/rm -rf /tmp/trash

mkdir -m 0700 /tmp/trash
chown 1001:1001 /tmp/trash
printf owner-mismatch >/tmp/owner-mismatch-source
chown 1000:1000 /tmp/owner-mismatch-source
set +e
setpriv --reuid=1000 --regid=1000 --clear-groups "$tool" /tmp/owner-mismatch-source >/dev/null 2>&1
status=$?
set -e
check nonroot_rejects_another_users_trash_root test "$status" -eq 1
check owner_mismatch_preserves_source test -e /tmp/owner-mismatch-source
/bin/rm -rf /tmp/trash

mkdir -m 0700 /tmp/custom-owned-marker
: >/tmp/custom-owned-marker/.trash-root
chmod 0600 /tmp/custom-owned-marker/.trash-root
chown 1000:1000 /tmp/custom-owned-marker/.trash-root
printf marker-owner >/tmp/marker-owner-source
set +e
TRASH_SUBDIR=custom-owned-marker "$tool" /tmp/marker-owner-source >/dev/null 2>&1
status=$?
set -e
check root_rejects_user_owned_custom_marker test "$status" -eq 1
check marker_owner_rejection_preserves_source test -e /tmp/marker-owner-source
/bin/rm -rf /tmp/custom-owned-marker

mkdir -m 0700 /tmp/trash
chown 1000:1000 /tmp/trash
mkdir -m 0700 /tmp/locked-parent
printf denied >/tmp/locked-parent/permission-source
chown -R 1000:1000 /tmp/locked-parent
chmod 0500 /tmp/locked-parent
set +e
setpriv --reuid=1000 --regid=1000 --clear-groups "$tool" /tmp/locked-parent/permission-source >/dev/null 2>&1
status=$?
set -e
check source_directory_permission_failure_is_reported test "$status" -eq 1
check source_directory_permission_failure_preserves_source test -e /tmp/locked-parent/permission-source
check failed_permission_move_removes_empty_run test "$(find /tmp/trash -mindepth 1 -maxdepth 1 -type d | wc -l)" -eq 0

printf 'OWNERSHIP_RESULT pass=%s fail=%s\n' "$pass" "$fail"
[ "$fail" -eq 0 ]
