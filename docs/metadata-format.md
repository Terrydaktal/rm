# Metadata Format

Each command creates one run directory per affected mountpoint:

```text
(APPLICATION)NAME+NAME-TIMESTAMP-pid-PID.RANDOM/
|-- metadata.json
|-- items.jsonl
`-- payload/
```

## metadata.json

`metadata.json` is written atomically before any source is moved.

```json
{
  "command": "rm -rf example",
  "cwd": "/home/user/project",
  "invoked_by": "fish <- xfce4-terminal <- systemd"
}
```

- `command` is a shell-safe audit rendering of the original argument vector.
- `cwd` is `PWD`, falling back to the process current directory.
- `invoked_by` joins `/proc` parent names from nearest to furthest with ` <- `.

`--clean-app` splits only this `invoked_by` field on the exact delimiter and compares
complete names. Invalid, missing, symlinked, or blank metadata does not match.

## items.jsonl

One compact JSON object is appended after each successful move:

```json
{"source":"build/output","destination":"/trash/(fish)output-.../payload/output"}
```

Each line has:

- `source`: the operand as supplied to the command.
- `destination`: the final collision-resolved payload path.

The payload itself is authoritative. Metadata is for auditing and app cleanup, not a
database required to access or restore files.
