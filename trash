#!/usr/bin/env bash
set -euo pipefail

# Trash-per-mountpoint:
#   For each file, move it to:  <that file's filesystem mountpoint>/trash/
# Examples:
#   /home/lewis/foo.txt   -> /home/trash/foo.txt
#   /mnt/U1/bar.txt       -> /mnt/U1/trash/bar.txt
#   /etc/hosts            -> /trash/hosts            (because mountpoint is "/")
#
# Collision suffix (timestamp + ns + pid):
#   ts="$(date +%Y%m%d-%H%M%S-%N)-$$"
#
# Notes:
# - Uses a private payload directory and verifies every move.
# - Optimistic move first (fast path), only does extra checks on failure.
# - Broken symlinks are handled (mv moves the symlink itself).

# --- CONFIG ---
TRASH_SUBDIR="${TRASH_SUBDIR:-trash}" # single safe directory name at mountpoint
TRASH_VERSION="0.3.0"

if [[ ! "$TRASH_SUBDIR" =~ ^[A-Za-z0-9._-]+$ || "$TRASH_SUBDIR" == "." || "$TRASH_SUBDIR" == ".." ]]; then
    echo "rm: invalid TRASH_SUBDIR '$TRASH_SUBDIR': use one safe directory name" >&2
    exit 2
fi

# Always bypass trash for these apps, even when TRASH_EXCEPTIONS is overridden.
hardcoded_exceptions_array=("netmgr")

# Defaults are always retained; the environment variable adds entries.
exceptions_array=("paru" "makepkg" "yay" "trigger.sh")
if [ -n "${TRASH_EXCEPTIONS:-}" ]; then
    read -r -a configured_exceptions_array <<<"$TRASH_EXCEPTIONS"
    exceptions_array+=("${configured_exceptions_array[@]}")
fi
exceptions_array+=("${hardcoded_exceptions_array[@]}")

force=false
recursive=false
interactive_mode=""
verbose=false
clean_current_mountpoint=false
clean_app_mode=false
show_help=false
show_version=false
files_to_trash=()
clean_apps=()

orig_argv=("$@")

print_help() {
    local prog
    prog="$(basename -- "$0")"
    cat <<EOF
NAME
    $prog - per-mountpoint rm wrapper with grouped trash folders and cleanup tools

SYNOPSIS
    $prog [OPTIONS] FILE...
    $prog --clean [-f]
    $prog --clean PATH... [-f]
    $prog --clean-app APP... [-f]
    $prog --help
    $prog --version

DESCRIPTION
    $prog intercepts rm-style deletes and moves each target into a trash directory
    on the same filesystem mountpoint instead of unlinking it immediately.

    Files deleted in one command are grouped into a private random run folder
    named from the preferred application, shortened target names, timestamp, and
    PID. The moved entries are stored below that run folder's payload directory.

    Each run folder contains metadata.json and items.jsonl audit records with the
    original command, working directory, parent execution chain, and move map.

OPTIONS
    -f, --force
        Suppress missing-file diagnostics. For cleanup modes, bypass the
        permanent-deletion confirmation prompt. It does not bypass safety,
        permission, storage, or move failures.

    -r, -R, --recursive
        Accepted for rm compatibility. Directories are always moved as whole
        trees without recursive traversal.

    -d, --dir
        Accepted for rm compatibility. Directories do not require this option.

    -i, --interactive
        Prompt before moving each target.

    -I
        Prompt once before recursive or larger operations.

    -v, --verbose
        Report each successful move.

    --interactive=always|once|never
        Select per-target, once-per-operation, or no interactive prompting.

    --preserve-root, --preserve-root=all
        Accepted as no-op compatibility options. Mountpoint and trash-root
        protections always remain active.

    --one-file-system
        Accepted as a compatibility safety option. Normal trashing renames each
        target on its own filesystem instead of traversing it, and permanent
        cleanup always passes --one-file-system to /bin/rm.

    --clean
        With no path, permanently delete all entries in the current mountpoint's
        trash directory. With paths, permanently delete exactly those files,
        symlinks, or directories from one trusted trash root. Requires
        confirmation in an interactive terminal unless -f is supplied.

    --clean-app APP...
        Permanently delete grouped trash runs on the current mountpoint whose
        folder name starts with any (APP), or whose metadata invoked_by chain
        contains any APP as an exact process name. All apps are matched in one
        scan. The scan checks names first and only reads metadata for non-matching
        names. Requires confirmation in an interactive terminal unless -f is
        supplied.

    --help
        Show this help text and exit.

    --version
        Show version information and exit.

OPERATION
    1. The script walks the parent process chain and bypasses trash entirely
       for exception apps such as netmgr, paru, makepkg, yay, and trigger.sh.
    2. For normal trashing, each target is moved into a per-run folder inside
       MOUNTPOINT/TRASH_SUBDIR.
    3. The run folder label prefers codex or agy when either appears in the
       parent execution chain; otherwise it uses the immediate parent process.
    4. Normal trashing uses shared locking; cleanup uses an exclusive lock.
    5. Path cleanup validates every target before deleting any, requires one
       trusted trash root, and protects root control files.
    6. Unknown and unsupported options fail closed with status 2. The real
       /bin/rm is used only for cleanup, exception bypasses, and no-operand
       diagnostics.

EXAMPLES
    $prog file.txt dir
    $prog -rf build-cache
    $prog --clean
    $prog --clean '/trash/(agy)run/payload/one' '/trash/(agy)run/payload/two'
    $prog -f --clean-app agy codex

FILES
    MOUNTPOINT/TRASH_SUBDIR/<run>/metadata.json
        JSON audit record with command, cwd, and invoked_by data.

    MOUNTPOINT/TRASH_SUBDIR/<run>/items.jsonl
        One JSON object per moved source and destination pair.

    MOUNTPOINT/TRASH_SUBDIR/<run>/payload/
        Private directory containing the moved files and folders.

    MOUNTPOINT/TRASH_SUBDIR/.trash.lock
        Lock used to coordinate trashing and cleanup.

    MOUNTPOINT/TRASH_SUBDIR/.trash-root
        Marker required for custom TRASH_SUBDIR roots.

PATHS
    /trash
        Trash root when the deleted file lives on the / mountpoint.

    MOUNTPOINT/trash
        Default trash root for non-root mountpoints.

    TRASH_SUBDIR
        Environment variable that changes the trash directory name from the
        default of "trash".

SECURITY NOTES
    Cleanup modes use permanent deletion via /bin/rm.
    Path cleanup refuses the trash root itself, nested mountpoints, .trash.lock,
    .trash-root, symlinked-parent escapes, mixed trash roots, and paths outside
    the trusted trash root. A final symlink is removed without following its
    target.
    Unsupported options never fall through to permanent /bin/rm.
    Filesystem mountpoints are refused as trash targets before any trash folder
    is created, including when -f or --force is supplied.
    Trash roots must be real, same-filesystem directories without group/other
    write access. Root operations require root-owned roots and markers.
    Cleanup is limited to direct entries below the trusted trash directory and
    does not cross nested filesystems.
    If TRASH_EXCEPTIONS is set, its entries are added to the configurable
    bypass list; netmgr is always hardcoded to bypass trash.

EXIT STATUS
    0  Success.
    1  One or more targets or cleanup actions failed.
    2  Invalid usage, such as a missing --clean-app value or conflicting modes.

AUTHORS
    Terrydaktal
EOF
}

print_version() {
    local prog
    prog="$(basename -- "$0")"
    printf '%s %s\n' "$prog" "$TRASH_VERSION"
}

is_exception_name() {
    local candidate="$1"
    local exception
    for exception in "${exceptions_array[@]}"; do
        if [ "$candidate" = "$exception" ]; then
            return 0
        fi
    done
    return 1
}

is_shell_interpreter() {
    case "$1" in
        bash | sh | dash | zsh | ksh | fish) return 0 ;;
    esac
    return 1
}

# Return the script operand for a shell command, not arbitrary -c source or
# unrelated user arguments.  This prevents exception words in prompts and
# command data from activating permanent deletion.
script_name_from_cmdline() {
    local cmd0="$1"
    local token
    local after_options=false
    shift
    is_shell_interpreter "$cmd0" || return 1
    for token; do
        if [ "$after_options" = false ]; then
            case "$token" in
                -c | -O | -o) return 1 ;;
                --)
                    after_options=true
                    continue
                    ;;
                -*) continue ;;
            esac
        fi
        printf '%s\n' "${token##*/}"
        return 0
    done
    return 1
}

preferred_app_from_cmdline() {
    local cmd0="$1"
    local script_name
    case "$cmd0" in
        codex | agy)
            printf '%s\n' "$cmd0"
            return 0
            ;;
    esac
    if script_name="$(script_name_from_cmdline "$@" 2>/dev/null)"; then
        case "$script_name" in
            codex | agy)
                printf '%s\n' "$script_name"
                return 0
                ;;
        esac
    fi
    return 1
}

exception_from_cmdline() {
    local cmd0="$1"
    local script_name
    if is_exception_name "$cmd0"; then
        return 0
    fi
    if script_name="$(script_name_from_cmdline "$@" 2>/dev/null)" && is_exception_name "$script_name"; then
        return 0
    fi
    return 1
}

# --- 1) UNPACK COMBINED SHORT OPTIONS ---
unpacked_args=()
for arg in "$@"; do
    if [[ "$arg" =~ ^-[a-zA-Z0-9]{2,}$ ]]; then
        for ((i = 1; i < ${#arg}; i++)); do
            unpacked_args+=("-${arg:$i:1}")
        done
    else
        unpacked_args+=("$arg")
    fi
done
set -- "${unpacked_args[@]}"

# --- 2) PROCESS TREE WALK & BYPASS CHECK ---
# Walk up the Linux process tree using pure Bash to find the execution chain
process_chain=()
preferred_parent_app=""
curr_pid="$PPID"
while [[ "$curr_pid" =~ ^[0-9]+$ ]] && [ "$curr_pid" -gt 1 ]; do
    name=""
    if [ -r "/proc/$curr_pid/comm" ]; then
        name=$(<"/proc/$curr_pid/comm") || name=""
        name="${name//$'\n'/}"
        if [ -n "$name" ]; then
            process_chain+=("$name")
            if [ -z "$preferred_parent_app" ]; then
                case "$name" in
                    codex | agy) preferred_parent_app="$name" ;;
                esac
            fi
            if is_exception_name "$name"; then
                exec /bin/rm "${orig_argv[@]}"
            fi
        fi
    fi

    cmdline_args=()
    if [ -r "/proc/$curr_pid/cmdline" ]; then
        mapfile -d '' -t cmdline_args <"/proc/$curr_pid/cmdline" 2>/dev/null || cmdline_args=()
    fi
    if [ ${#cmdline_args[@]} -gt 0 ]; then
        cmd0="${cmdline_args[0]##*/}"
        if [ -z "$preferred_parent_app" ] && preferred="$(preferred_app_from_cmdline "${cmdline_args[@]}" 2>/dev/null)"; then
            preferred_parent_app="$preferred"
        fi
        if exception_from_cmdline "${cmdline_args[@]}"; then
            exec /bin/rm "${orig_argv[@]}"
        fi
    fi

    next_pid=""
    if [ -r "/proc/$curr_pid/status" ]; then
        while IFS=$'\t ' read -r key value; do
            if [ "$key" = "PPid:" ]; then
                next_pid="$value"
                break
            fi
        done <"/proc/$curr_pid/status" || true
    fi

    if [[ "$next_pid" =~ ^[0-9]+$ ]] && [ "$next_pid" -ne "$curr_pid" ]; then
        curr_pid="$next_pid"
    else
        break
    fi
done

# --- 3) PARSE ARGUMENTS (robust) ---
while [ $# -gt 0 ]; do
    case "$1" in
        --help)
            show_help=true
            shift
            ;;
        --version)
            show_version=true
            shift
            ;;
        -f | --force)
            force=true
            shift
            ;;
        -r | -R | --recursive)
            recursive=true
            shift
            ;;
        -d | --dir)
            shift
            ;;
        -i | --interactive)
            interactive_mode="always"
            shift
            ;;
        -I)
            interactive_mode="once"
            shift
            ;;
        -v | --verbose)
            verbose=true
            shift
            ;;
        --interactive=always)
            interactive_mode="always"
            shift
            ;;
        --interactive=once)
            interactive_mode="once"
            shift
            ;;
        --interactive=never)
            interactive_mode=""
            shift
            ;;
        --preserve-root | --preserve-root=all)
            shift
            ;;
        --no-preserve-root)
            echo "rm: refusing unsupported --no-preserve-root because trash safety cannot be bypassed" >&2
            exit 2
            ;;
        --one-file-system)
            # Trashing does not recursively traverse the target, and cleanup
            # independently enforces this boundary with the real rm.
            shift
            ;;
        --clean)
            clean_current_mountpoint=true
            shift
            ;;
        --clean-app)
            clean_app_mode=true
            shift
            ;;
        --clean-app=*)
            clean_app_mode=true
            clean_app_value="${1#--clean-app=}"
            if [ -z "$clean_app_value" ]; then
                echo "rm: option '--clean-app' requires an app name" >&2
                exit 2
            fi
            clean_apps+=("$clean_app_value")
            shift
            ;;
        --)
            shift
            # Everything after -- belongs to the active operand mode.
            while [ $# -gt 0 ]; do
                if [ "$clean_app_mode" = true ]; then
                    clean_apps+=("$1")
                else
                    files_to_trash+=("$1")
                fi
                shift
            done
            ;;
        -*)
            echo "rm: unsupported option '$1'; refusing permanent /bin/rm fallback" >&2
            exit 2
            ;;
        *)
            if [ "$clean_app_mode" = true ]; then
                clean_apps+=("$1")
            else
                files_to_trash+=("$1")
            fi
            shift
            ;;
    esac
done

if [ "$clean_current_mountpoint" = true ] && [ "$clean_app_mode" = true ]; then
    echo "rm: cannot combine --clean and --clean-app" >&2
    exit 2
fi

if [ "$clean_app_mode" = true ] && [ ${#clean_apps[@]} -eq 0 ]; then
    echo "rm: option '--clean-app' requires at least one app name" >&2
    exit 2
fi

if [ "$clean_app_mode" = true ]; then
    declare -A seen_clean_apps=()
    unique_clean_apps=()
    for clean_app_value in "${clean_apps[@]}"; do
        if [[ ! "$clean_app_value" =~ ^[A-Za-z0-9._-]+$ ]]; then
            echo "rm: invalid application name '$clean_app_value'" >&2
            exit 2
        fi
        if [[ -z "${seen_clean_apps[$clean_app_value]:-}" ]]; then
            seen_clean_apps[$clean_app_value]=1
            unique_clean_apps+=("$clean_app_value")
        fi
    done
    clean_apps=("${unique_clean_apps[@]}")
fi

if [ "$show_help" = true ]; then
    print_help
    exit 0
fi

if [ "$show_version" = true ]; then
    print_version
    exit 0
fi

# Preserve normal rm's no-operand diagnostics and -f behavior without creating
# an empty trash run.
if [ "$clean_current_mountpoint" = false ] && [ "$clean_app_mode" = false ] && [ ${#files_to_trash[@]} -eq 0 ]; then
    exec /bin/rm "${orig_argv[@]}"
fi

# --- 4) MASTER FOLDER GENERATION ---

parent_app="unknown"
if [ ${#process_chain[@]} -gt 0 ]; then
    parent_app="${preferred_parent_app:-${process_chain[0]}}"
    parent_app="${parent_app//[^a-zA-Z0-9._-]/_}"
    parent_app="${parent_app:0:32}"
fi

# Generate a unique, timestamped folder name using prefixes of the first few
# targets and a sanitized parent app tag at the front.
short_name() {
    local path="$1"
    local value
    while [[ "$path" == */ ]]; do
        path="${path%/}"
    done
    value="${path##*/}"
    value="${value//[^a-zA-Z0-9._-]/_}"
    [ -n "$value" ] || value="item"
    printf '%s' "${value:0:48}"
}

names=()
for item in "${files_to_trash[@]:0:2}"; do
    names+=("$(short_name "$item")")
done

prefix=""
if [ ${#names[@]} -gt 0 ]; then
    # Join the names with plus signs and add a trailing dash
    prefix="$(
        IFS=+
        printf '%s' "${names[*]}"
    )-"
fi

RUN_DIR_NAME="(${parent_app})${prefix}$(date +%Y-%m-%d_%H-%M-%S)-pid-$$"

# --- PRE-COMPUTE METADATA JSON CONTENT ---
escape_json_string() {
    local LC_ALL=C
    local str="$1"
    local result=""
    local index code char escaped

    # Most paths and process names are printable ASCII. Keep that common case
    # in Bash's bulk substitution path instead of looping over every byte.
    if [[ "$str" =~ ^[[:print:]]*$ ]]; then
        str="${str//\\/\\\\}"
        str="${str//\"/\\\"}"
        printf '%s' "$str"
        return 0
    fi

    for ((index = 0; index < ${#str}; index++)); do
        char="${str:index:1}"
        printf -v code '%d' "'$char"
        if ((code < 0x20 || code > 0x7e)); then
            printf -v escaped '\\u%04x' "$code"
            result+="$escaped"
        else
            case "$char" in
                \\) result+="\\\\" ;;
                \") result+="\\\\\"" ;;
                *) result+="$char" ;;
            esac
        fi
    done
    printf '%s' "$result"
}

cmd_name="rm"

cmd_string="$cmd_name"
if [ ${#orig_argv[@]} -gt 0 ]; then
    escaped_args=$(printf "%q " "${orig_argv[@]}")
    escaped_args="${escaped_args% }"
    cmd_string="$cmd_name $escaped_args"
fi

invoked_by=""
if [ ${#process_chain[@]} -gt 0 ]; then
    invoked_by="${process_chain[0]}"
    for ((i = 1; i < ${#process_chain[@]}; i++)); do
        invoked_by="$invoked_by <- ${process_chain[i]}"
    done
fi

METADATA_JSON_CONTENT="$(
    printf '{\n'
    printf '  "command": "%s",\n' "$(escape_json_string "$cmd_string")"
    printf '  "cwd": "%s",\n' "$(escape_json_string "$PWD")"
    printf '  "invoked_by": "%s"\n' "$(escape_json_string "$invoked_by")"
    printf '}\n'
)"

# --- 5) TRASH IMPLEMENTATION ---
# Cache which mountpoints we've already ensured have a trash dir (speed).
declare -A ensured_trash_dir=()
declare -A ensured_run_dir=()
declare -A trash_lock_fds=()
declare -A trash_manifest_fds=()
declare -A canonical_mountpoints=()
declare -A canonical_trash_roots=()

had_error=0

get_mountpoint() {
    # Uses lstat (does NOT follow symlinks), so broken symlinks still work.
    # Prints mountpoint (e.g. "/", "/home", "/mnt/U1")
    stat -c %m -- "$1"
}

is_mountpoint_target() {
    local file="$1"
    local mp="$2"
    local file_real="${3:-}"
    local mp_real

    [ -L "$file" ] && return 1
    [ -n "$file_real" ] || return 1
    if [ -z "${canonical_mountpoints[$mp]:-}" ]; then
        canonical_mountpoints[$mp]="$(realpath -e -- "$mp" 2>/dev/null)" || return 1
    fi
    mp_real="${canonical_mountpoints[$mp]}"
    [ "$file_real" = "$mp_real" ]
}

is_inside_trash_root() {
    local file="$1"
    local mp="$2"
    local file_real="${3:-}"
    local root root_real

    [ -L "$file" ] && return 1
    [ -n "$file_real" ] || return 1
    root="$(trash_dir_for_mountpoint "$mp")"
    if [ -z "${canonical_trash_roots[$mp]:-}" ]; then
        canonical_trash_roots[$mp]="$(realpath -m -- "$root" 2>/dev/null)" || return 1
    fi
    root_real="${canonical_trash_roots[$mp]}"
    case "$file_real" in
        "$root_real" | "$root_real"/*) return 0 ;;
    esac
    return 1
}

is_android=false
if [ -d "/system/bin" ] && { [ -d "/data/data/com.termux" ] || [ -d "/data/local/tmp" ]; }; then
    is_android=true
fi

trash_root_for_mountpoint() {
    local mp="$1"
    if [ "$is_android" = true ]; then
        if [[ "$mp" == /storage* || "$mp" == /sdcard* ]]; then
            printf '/sdcard/.%s\n' "$TRASH_SUBDIR"
            return 0
        elif [[ "$mp" == /data/local/tmp* || "$EUID" -eq 2000 ]]; then
            if [ -d "/data/local/tmp/termux-sudo" ]; then
                printf '/data/local/tmp/termux-sudo/%s\n' "$TRASH_SUBDIR"
            else
                printf '/data/local/tmp/.%s\n' "$TRASH_SUBDIR"
            fi
            return 0
        elif [ -n "${HOME:-}" ] && [ -d "$HOME" ]; then
            printf '%s/.%s\n' "$HOME" "$TRASH_SUBDIR"
            return 0
        fi
    fi
    if [ "$mp" = "/" ]; then
        printf '/%s\n' "$TRASH_SUBDIR"
    else
        printf '%s/%s\n' "${mp%/}" "$TRASH_SUBDIR"
    fi
}

trash_dir_for_mountpoint() {
    trash_root_for_mountpoint "$1"
}

check_trash_root() {
    local mp="$1"
    local root="$2"
    local owner mode device mp_device marker numeric_mode

    if [ -L "$root" ] || [ ! -d "$root" ]; then
        echo "rm: refusing untrusted trash root '$root': it must be a real directory" >&2
        return 1
    fi
    if ! read -r owner mode device < <(stat -c '%u %a %d' -- "$root"); then
        echo "rm: cannot inspect trash root '$root'" >&2
        return 1
    fi
    mp_device="$(stat -c '%d' -- "$mp")" || {
        echo "rm: cannot inspect mountpoint '$mp'" >&2
        return 1
    }
    if [ "$is_android" = false ] && [ "$device" != "$mp_device" ]; then
        echo "rm: refusing trash root '$root': it is on a different filesystem" >&2
        return 1
    fi
    numeric_mode=$((8#$mode))
    if [ "$is_android" = false ] || [[ "$root" != /sdcard* && "$root" != /storage* ]]; then
        if ((numeric_mode & 0022)); then
            echo "rm: refusing trash root '$root': group/other write permission is unsafe" >&2
            return 1
        fi
    fi
    if [ "$EUID" -eq 0 ] && [ "$owner" -ne 0 ]; then
        echo "rm: refusing root operation: trash root '$root' is not root-owned" >&2
        return 1
    fi
    if [ "$is_android" = false ] && [ "$EUID" -ne 0 ] && [ "$owner" -ne 0 ] && [ "$owner" -ne "$EUID" ]; then
        echo "rm: refusing trash root '$root': it is owned by another user" >&2
        return 1
    fi
    if [ "$TRASH_SUBDIR" != "trash" ]; then
        marker="$root/.trash-root"
        if [ -L "$marker" ] || [ ! -f "$marker" ]; then
            echo "rm: refusing custom trash root '$root': missing .trash-root marker" >&2
            return 1
        fi
        read -r owner mode < <(stat -c '%u %a' -- "$marker") || return 1
        if [ "$EUID" -eq 0 ] && [ "$owner" -ne 0 ]; then
            echo "rm: refusing root operation: trash marker '$marker' is not root-owned" >&2
            return 1
        fi
        numeric_mode=$((8#$mode))
        if ((numeric_mode & 0077)); then
            echo "rm: refusing trash marker '$marker': it must be private" >&2
            return 1
        fi
    fi
}

ensure_trash_root() {
    local mp="$1"
    local root created=false
    root="$(trash_root_for_mountpoint "$mp")"
    if [ ! -e "$root" ]; then
        if ! mkdir -m 0755 -- "$root" 2>/dev/null; then
            echo "rm: cannot create trash root '$root'" >&2
            return 1
        fi
        created=true
    fi
    if [ "$created" = true ] && [ "$TRASH_SUBDIR" != "trash" ]; then
        if ! : >"$root/.trash-root"; then
            echo "rm: cannot create trash marker '$root/.trash-root'" >&2
            return 1
        fi
        chmod 600 -- "$root/.trash-root" 2>/dev/null || true
    fi
    if ! check_trash_root "$mp" "$root"; then
        return 1
    fi
}

validate_trash_dir() {
    local mp="$1"
    local dir owner mode numeric_mode
    dir="$(trash_dir_for_mountpoint "$mp")"
    if [ ! -e "$dir" ]; then
        if ! mkdir -m 0700 -- "$dir" 2>/dev/null; then
            echo "rm: cannot create trash directory '$dir'" >&2
            return 1
        fi
    fi
    if [ -L "$dir" ] || [ ! -d "$dir" ]; then
        echo "rm: refusing trash directory '$dir': it must be a real directory" >&2
        return 1
    fi
    if ! read -r owner mode < <(stat -c '%u %a' -- "$dir"); then
        echo "rm: cannot inspect trash directory '$dir'" >&2
        return 1
    fi
    if [ "$is_android" = false ] && [ "$owner" -ne "$EUID" ]; then
        echo "rm: refusing trash directory '$dir': it is not owned by the current user" >&2
        return 1
    fi
    numeric_mode=$((8#$mode))
    if [ "$is_android" = false ] || [[ "$dir" != /sdcard* && "$dir" != /storage* ]]; then
        if ((numeric_mode & 0022)); then
            echo "rm: refusing trash directory '$dir': group/other write permission is unsafe" >&2
            return 1
        fi
    fi
    if [ "$is_android" = false ] && [ "$(stat -c '%d' -- "$dir")" != "$(stat -c '%d' -- "$mp")" ]; then
        echo "rm: refusing trash directory '$dir': it is on a different filesystem" >&2
        return 1
    fi
}

ensure_trash_dir() {
    local mp="$1"
    local key="$mp"
    local dir run_dir payload_dir metadata_tmp manifest_fd

    if [[ -n "${ensured_trash_dir[$key]:-}" ]]; then
        return 0
    fi

    if ! ensure_trash_root "$mp"; then
        return 1
    fi
    dir="$(trash_dir_for_mountpoint "$mp")"
    if ! validate_trash_dir "$mp"; then
        return 1
    fi

    if ! acquire_trash_lock "$mp"; then
        return 1
    fi

    if ! run_dir="$(mktemp -d -- "$dir/${RUN_DIR_NAME}.XXXXXX" 2>/dev/null)"; then
        echo "rm: cannot create a unique trash run directory in '$dir'" >&2
        return 1
    fi
    if ! mkdir -m 0700 -- "$run_dir/payload" 2>/dev/null; then
        /bin/rm -rf -- "$run_dir"
        echo "rm: cannot create trash payload directory '$run_dir/payload'" >&2
        return 1
    fi
    if ! metadata_tmp="$(mktemp -- "$run_dir/.metadata.json.tmp.XXXXXX" 2>/dev/null)"; then
        /bin/rm -rf -- "$run_dir"
        echo "rm: cannot create metadata temporary file in '$run_dir'" >&2
        return 1
    fi
    if ! printf '%s' "$METADATA_JSON_CONTENT" >"$metadata_tmp" || ! mv -f -- "$metadata_tmp" "$run_dir/metadata.json"; then
        /bin/rm -f -- "$metadata_tmp"
        /bin/rm -rf -- "$run_dir"
        echo "rm: cannot write metadata in '$run_dir'" >&2
        return 1
    fi
    if ! : >"$run_dir/items.jsonl"; then
        /bin/rm -rf -- "$run_dir"
        echo "rm: cannot create item manifest in '$run_dir'" >&2
        return 1
    fi
    if ! exec {manifest_fd}>>"$run_dir/items.jsonl"; then
        /bin/rm -rf -- "$run_dir"
        echo "rm: cannot open item manifest in '$run_dir'" >&2
        return 1
    fi

    payload_dir="$run_dir/payload"
    ensured_run_dir[$key]="$run_dir"
    ensured_trash_dir[$key]="$payload_dir"
    trash_manifest_fds[$key]="$manifest_fd"
}

acquire_trash_lock() {
    local mp="$1"
    local lock_mode="${2:-shared}"
    local dir fd
    if [[ -n "${trash_lock_fds[$mp]:-}" ]]; then
        return 0
    fi
    dir="$(trash_dir_for_mountpoint "$mp")"
    if [ -L "$dir/.trash.lock" ]; then
        echo "rm: refusing trash lock '$dir/.trash.lock': it is a symlink" >&2
        return 1
    fi
    if ! exec {fd}>>"$dir/.trash.lock"; then
        echo "rm: cannot open trash lock '$dir/.trash.lock'" >&2
        return 1
    fi
    if [ "$lock_mode" = "exclusive" ]; then
        if ! flock -x "$fd"; then
            exec {fd}>&-
            echo "rm: cannot acquire exclusive trash lock '$dir/.trash.lock'" >&2
            return 1
        fi
    elif ! flock -s "$fd"; then
        exec {fd}>&-
        echo "rm: cannot acquire trash lock '$dir/.trash.lock'" >&2
        return 1
    fi
    trash_lock_fds[$mp]="$fd"
}

confirm_cleanup_prompt() {
    local prompt="$1"
    local expected="$2"
    local response

    if [ "$force" = true ]; then
        return 0
    fi
    if [ ! -t 0 ] || [ ! -t 2 ]; then
        echo "rm: refusing permanent cleanup without a terminal; use --force explicitly" >&2
        return 1
    fi
    printf '%s' "$prompt" >&2
    if ! IFS= read -r response; then
        echo "rm: failed to read cleanup confirmation" >&2
        return 1
    fi
    if [ "$response" != "$expected" ]; then
        echo "rm: cleanup confirmation failed" >&2
        return 1
    fi
}

clean_trash_paths() {
    local requested path parent base parent_real target mp target_mp root root_real target_real
    local batch_mp=""
    local expected prompt
    local -a targets=()
    local -A seen_targets=()

    for requested; do
        path="$requested"
        while [[ "$path" == */ && "$path" != "/" ]]; do
            path="${path%/}"
        done
        base="${path##*/}"
        case "$base" in
            "" | . | ..)
                echo "rm: refusing invalid cleanup path '$requested'" >&2
                return 1
                ;;
        esac

        parent="$(dirname -- "$path")"
        if ! parent_real="$(realpath -e -- "$parent" 2>/dev/null)"; then
            echo "rm: cannot clean '$requested': parent directory does not exist" >&2
            return 1
        fi
        target="${parent_real%/}/$base"

        if ! mp="$(get_mountpoint "$parent_real" 2>/dev/null)"; then
            echo "rm: cannot clean '$requested': failed to determine mountpoint" >&2
            return 1
        fi
        if [ -n "$batch_mp" ] && [ "$mp" != "$batch_mp" ]; then
            echo "rm: refusing --clean paths from multiple trash roots" >&2
            return 1
        fi
        batch_mp="$mp"

        root="$(trash_root_for_mountpoint "$mp")"
        if ! check_trash_root "$mp" "$root" || ! validate_trash_dir "$mp"; then
            return 1
        fi
        if ! root_real="$(realpath -e -- "$root" 2>/dev/null)"; then
            echo "rm: cannot resolve trash root '$root'" >&2
            return 1
        fi
        case "$target" in
            "$root_real")
                echo "rm: refusing to clean the trash root itself '$target'" >&2
                return 1
                ;;
            "$root_real/.trash.lock" | "$root_real/.trash-root")
                echo "rm: refusing to clean protected trash control file '$target'" >&2
                return 1
                ;;
            "$root_real"/*) ;;
            *)
                echo "rm: refusing cleanup path outside trusted trash root '$target'" >&2
                return 1
                ;;
        esac

        if [ ! -e "$target" ] && [ ! -L "$target" ]; then
            if [ "$force" = false ]; then
                echo "rm: cannot clean '$target': No such file or directory" >&2
                return 1
            fi
        else
            if ! target_mp="$(get_mountpoint "$target" 2>/dev/null)"; then
                echo "rm: cannot clean '$target': failed to determine target mountpoint" >&2
                return 1
            fi
            target_real=""
            if [ ! -L "$target" ]; then
                target_real="$(realpath -e -- "$target" 2>/dev/null || true)"
            fi
            if is_mountpoint_target "$target" "$target_mp" "$target_real" || [ "$target_mp" != "$mp" ]; then
                echo "rm: refusing to clean nested filesystem target '$target'" >&2
                return 1
            fi
        fi

        if [[ -z "${seen_targets[$target]:-}" ]]; then
            seen_targets[$target]=1
            targets+=("$target")
        fi
    done

    if ! acquire_trash_lock "$batch_mp" exclusive; then
        return 1
    fi
    if [ ${#targets[@]} -eq 1 ]; then
        expected="${targets[0]}"
        prompt="WARNING: type '$expected' to permanently delete it: "
    else
        expected="DELETE ${#targets[@]} PATHS"
        prompt="WARNING: type '$expected' to permanently delete ${#targets[@]} paths: "
    fi
    if ! confirm_cleanup_prompt "$prompt" "$expected"; then
        return 1
    fi
    if ! /bin/rm --one-file-system -rf -- "${targets[@]}"; then
        echo "rm: cannot clean one or more trash paths: deletion failed" >&2
        return 1
    fi
}

clean_current_trash() {
    local mp root dir trash_device
    if ! mp="$(get_mountpoint "." 2>/dev/null)"; then
        echo "rm: cannot clean trash: failed to determine mountpoint for '$PWD'" >&2
        return 1
    fi
    root="$(trash_root_for_mountpoint "$mp")"
    if [ ! -e "$root" ]; then
        return 0
    fi
    if ! check_trash_root "$mp" "$root"; then
        return 1
    fi
    dir="$(trash_dir_for_mountpoint "$mp")"
    if [ ! -d "$dir" ]; then
        return 0
    fi
    if ! validate_trash_dir "$mp"; then
        return 1
    fi
    if ! acquire_trash_lock "$mp" exclusive; then
        return 1
    fi
    if ! confirm_cleanup_prompt "WARNING: type '$dir' to permanently empty it: " "$dir"; then
        return 1
    fi
    if ! trash_device="$(stat -c '%d' -- "$dir")"; then
        echo "rm: cannot inspect trash device '$dir'" >&2
        return 1
    fi
    if ! find "$dir" -xdev -mindepth 1 -maxdepth 1 \
        ! -path "$dir/.trash.lock" ! -path "$dir/.trash-root" \
        -printf '%D\0%p\0' |
        bash -c '
            trash_device="$1"
            shift
            status=0
            entries=()

            delete_batch() {
                if [ "${#entries[@]}" -eq 0 ]; then
                    return 0
                fi
                if ! /bin/rm --one-file-system -rf -- "${entries[@]}"; then
                    status=1
                fi
                entries=()
            }

            while IFS= read -r -d "" entry_device && IFS= read -r -d "" entry; do
                if [ "$entry_device" = "$trash_device" ]; then
                    entries+=("$entry")
                    if [ "${#entries[@]}" -ge 256 ]; then
                        delete_batch
                    fi
                else
                    status=1
                fi
            done
            delete_batch
            exit "$status"
        ' bash "$trash_device"; then
        echo "rm: cannot clean trash at '$dir': deletion failed" >&2
        return 1
    fi
}

clean_trash_by_apps() {
    local -a apps=("$@")
    local app_list="${apps[*]}"
    local mp root dir matches_file metadata_paths match_count scan_failed
    if ! mp="$(get_mountpoint "." 2>/dev/null)"; then
        echo "rm: cannot clean trash: failed to determine mountpoint for '$PWD'" >&2
        return 1
    fi

    root="$(trash_root_for_mountpoint "$mp")"
    if [ ! -e "$root" ]; then
        return 0
    fi
    if ! check_trash_root "$mp" "$root"; then
        return 1
    fi
    dir="$(trash_dir_for_mountpoint "$mp")"
    if [ ! -d "$dir" ]; then
        return 0
    fi
    if ! validate_trash_dir "$mp" || ! acquire_trash_lock "$mp" exclusive; then
        return 1
    fi

    if ! matches_file="$(mktemp)" || ! metadata_paths="$(mktemp)"; then
        echo "rm: cannot clean trash entries for apps '$app_list': failed to create temporary files" >&2
        return 1
    fi
    scan_failed=false

    if ! find "$dir" -xdev -mindepth 1 -maxdepth 1 -type d -exec sh -c '
        app_list="$1"
        shift
        set -f
        for run_dir do
            base="${run_dir##*/}"
            matched=false
            for app in $app_list; do
                case "$base" in
                    "($app)"*)
                        printf "%s\0" "$run_dir" >&3
                        matched=true
                        break
                        ;;
                esac
            done
            if [ "$matched" = true ]; then
                continue
            fi
            metadata="$run_dir/metadata.json"
            if [ -f "$metadata" ]; then
                printf "%s\0" "$metadata" >&4
            fi
        done
    ' sh "$app_list" {} + 3>"$matches_file" 4>"$metadata_paths"; then
        scan_failed=true
    fi

    # shellcheck disable=SC2016 # $0 belongs to awk, not the shell.
    if [ "$scan_failed" = false ] && ! xargs -0 -r awk -v app_list="$app_list" '
        BEGIN {
            app_count = split(app_list, apps, / /)
            for (app_index = 1; app_index <= app_count; app_index++) {
                requested_apps[apps[app_index]] = 1
            }
        }
        /^[[:space:]]*"invoked_by"[[:space:]]*:/ {
            line = $0
            sub(/^[^:]*:[[:space:]]*"/, "", line)
            sub(/"[[:space:]]*,?[[:space:]]*$/, "", line)
            count = split(line, chain, /[[:space:]]*<-[[:space:]]*/)
            for (i = 1; i <= count; i++) {
                if (chain[i] in requested_apps) {
                    run = FILENAME
                    sub(/\/metadata\.json$/, "", run)
                    printf "%s%c", run, 0
                    nextfile
                }
            }
        }
    ' <"$metadata_paths" >>"$matches_file"; then
        scan_failed=true
    fi

    /bin/rm -f -- "$metadata_paths"
    if [ "$scan_failed" = true ]; then
        /bin/rm -f -- "$matches_file"
        echo "rm: cannot clean trash entries for apps '$app_list': scan failed" >&2
        return 1
    fi

    if [ "$force" = true ]; then
        if [ ! -s "$matches_file" ]; then
            /bin/rm -f -- "$matches_file"
            return 0
        fi
    else
        if ! match_count="$(tr -cd '\0' <"$matches_file" | wc -c)"; then
            /bin/rm -f -- "$matches_file"
            echo "rm: cannot count trash entries for apps '$app_list'" >&2
            return 1
        fi
        if [ "$match_count" -eq 0 ]; then
            /bin/rm -f -- "$matches_file"
            return 0
        fi
        if ! confirm_cleanup_prompt "WARNING: type '$app_list' to permanently delete $match_count trash run(s): " "$app_list"; then
            /bin/rm -f -- "$matches_file"
            return 1
        fi
    fi

    if ! xargs -0 -r /bin/rm --one-file-system -rf -- <"$matches_file"; then
        /bin/rm -f -- "$matches_file"
        echo "rm: cannot clean trash entries for apps '$app_list': deletion failed" >&2
        return 1
    fi

    /bin/rm -f -- "$matches_file"
    return 0
}

prompt_for_target() {
    local file="$1"
    local response
    printf "rm: move '%s' to trash? [y/N] " "$file" >&2
    if ! IFS= read -r response; then
        echo "rm: failed to read interactive confirmation" >&2
        return 2
    fi
    case "$response" in
        y | Y | yes | YES | Yes) return 0 ;;
        *) return 1 ;;
    esac
}

confirm_once_prompt() {
    local response
    printf "rm: move these targets to trash? [y/N] " >&2
    if ! IFS= read -r response; then
        echo "rm: failed to read interactive confirmation" >&2
        return 1
    fi
    case "$response" in
        y | Y | yes | YES | Yes) return 0 ;;
        *) return 1 ;;
    esac
}

source_basename() {
    local path="$1"
    while [[ "$path" == */ ]]; do
        path="${path%/}"
    done
    printf '%s' "${path##*/}"
}

record_manifest_item() {
    local mp="$1"
    local source="$2"
    local destination="$3"
    local run_dir="${ensured_run_dir[$mp]}"
    local manifest_fd="${trash_manifest_fds[$mp]:-}"
    if [ -z "$manifest_fd" ]; then
        echo "rm: warning: item manifest is not open for '$run_dir'" >&2
        return 1
    fi
    if ! printf '{"source":"%s","destination":"%s"}\n' \
        "$(escape_json_string "$source")" \
        "$(escape_json_string "$destination")" >&"$manifest_fd"; then
        echo "rm: warning: moved '$source' but could not update '$run_dir/items.jsonl'" >&2
        return 1
    fi
}

trash_one() {
    local file="$1"
    local mp root trash_dir base_name dest dest2 ts file_real
    local move_succeeded=false
    local attempt

    if [ ! -e "$file" ] && [ ! -L "$file" ]; then
        if [ "$force" = false ]; then
            echo "rm: cannot remove '$file': No such file or directory" >&2
            return 1
        fi
        return 0
    fi
    base_name="$(source_basename "$file")"
    case "$base_name" in
        . | ..)
            echo "rm: refusing to remove '$file'" >&2
            return 1
            ;;
    esac
    case "$file" in
        . | .. | */. | */..)
            echo "rm: refusing to remove '$file'" >&2
            return 1
            ;;
    esac

    if [ "$interactive_mode" = "always" ] && [ "$force" = false ]; then
        if prompt_for_target "$file"; then
            :
        else
            case "$?" in
                1) return 0 ;;
                *) return 1 ;;
            esac
        fi
    fi

    if ! mp="$(get_mountpoint "$file" 2>/dev/null)"; then
        echo "rm: cannot remove '$file': failed to determine mountpoint" >&2
        return 1
    fi
    file_real=""
    if [ ! -L "$file" ]; then
        file_real="$(realpath -e -- "$file" 2>/dev/null || true)"
    fi
    if is_mountpoint_target "$file" "$mp" "$file_real"; then
        echo "rm: refusing to trash filesystem mountpoint '$file' (mounted at '$mp')" >&2
        return 1
    fi
    if is_inside_trash_root "$file" "$mp" "$file_real"; then
        root="$(trash_root_for_mountpoint "$mp")"
        echo "rm: refusing to trash path inside the trash root '$file' (root '$root')" >&2
        return 1
    fi
    if ! ensure_trash_dir "$mp"; then
        return 1
    fi

    trash_dir="${ensured_trash_dir[$mp]}"
    if [ -z "$base_name" ]; then
        echo "rm: cannot remove '$file': invalid empty basename" >&2
        return 1
    fi
    dest="$trash_dir/$base_name"

    if mv -nT -- "$file" "$dest" 2>/dev/null && [ ! -e "$file" ] && [ ! -L "$file" ]; then
        move_succeeded=true
    fi
    if [ "$move_succeeded" = false ] && [ ! -e "$file" ] && [ ! -L "$file" ]; then
        return 0
    fi

    if [ "$move_succeeded" = false ] && { [ -e "$dest" ] || [ -L "$dest" ]; }; then
        for ((attempt = 1; attempt <= 100; attempt++)); do
            ts="$(date +%Y%m%d-%H%M%S-%N)-$$-$attempt"
            dest2="$trash_dir/${base_name}-${ts}"
            if mv -nT -- "$file" "$dest2" 2>/dev/null && [ ! -e "$file" ] && [ ! -L "$file" ]; then
                dest="$dest2"
                move_succeeded=true
                break
            fi
            if [ ! -e "$file" ] && [ ! -L "$file" ]; then
                return 0
            fi
        done
    fi

    if [ "$move_succeeded" = false ]; then
        echo "rm: cannot move '$file' to trash" >&2
        return 1
    fi
    if ! record_manifest_item "$mp" "$file" "$dest"; then
        return 1
    fi
    if [ "$verbose" = true ]; then
        printf "trashed '%s' -> '%s'\n" "$file" "$dest"
    fi
}

remove_empty_run_directories() {
    local mp run_dir first_entry
    for mp in "${!ensured_run_dir[@]}"; do
        run_dir="${ensured_run_dir[$mp]}"
        if [ ! -d "$run_dir/payload" ]; then
            continue
        fi
        first_entry="$(find "$run_dir/payload" -mindepth 1 -print -quit 2>/dev/null || true)"
        if [ -z "$first_entry" ]; then
            /bin/rm -rf -- "$run_dir"
        fi
    done
}

if [ "$interactive_mode" = "once" ] && [ "$force" = false ] && { [ "$recursive" = true ] || [ ${#files_to_trash[@]} -gt 3 ]; }; then
    if ! confirm_once_prompt; then
        exit 1
    fi
fi

if [ "$clean_current_mountpoint" = true ]; then
    if [ ${#files_to_trash[@]} -gt 0 ]; then
        if clean_trash_paths "${files_to_trash[@]}"; then
            exit 0
        fi
    elif clean_current_trash; then
        exit 0
    fi
    exit 1
fi

if [ "$clean_app_mode" = true ]; then
    if clean_trash_by_apps "${clean_apps[@]}"; then
        exit 0
    fi
    exit 1
fi

for f in "${files_to_trash[@]}"; do
    if ! trash_one "$f"; then
        had_error=1
        # We already match rm -f for missing paths; continue across operands either way.
    fi
done

remove_empty_run_directories
exit "$had_error"
