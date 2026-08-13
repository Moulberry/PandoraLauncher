# Control API

The launcher exposes its full command surface over a local NDJSON protocol on a
Unix domain socket. Anything the UI can do (create, configure, launch and kill
instances, manage accounts, install content, read logs, drive Microsoft login)
can be done programmatically, and a small event stream mirrors the state
changes the UI sees.

The API is served by the main launcher process. It is available on Unix
platforms only (macOS, Linux); on Windows the CLI client prints an error and
exits with code 2.

Protocol version: `1` (returned by `launcher.version`).

## Transport

### Socket

| Property | Value |
|---|---|
| Path | `<launcher_dir>/api.sock` |
| Permissions | `0600` (same user only) |
| Framing | NDJSON: one JSON object per line, `\n` terminated |
| Lifetime | Bound at launcher startup, stale file removed and re-bound |

`<launcher_dir>` is:

| Mode | Path |
|---|---|
| macOS | `~/Library/Application Support/PandoraLauncher` |
| Linux | `~/.local/share/PandoraLauncher` |
| Portable | `<portable_dir>/PandoraLauncher` |

This socket is distinct from `launcher.sock` (the single-instance argv
forwarder, which focuses the main window on every connection). The API
listener is silent: it never touches the UI unless a command explicitly asks
for it (`launcher.focus`).

### Request and response lines

Request:

```json
{"id": 1, "cmd": "instances.get", "params": {"name": "main"}}
```

Success response:

```json
{"id": 1, "ok": true, "data": {"...": "..."}}
```

Error response:

```json
{"id": 1, "ok": false, "error": "no instance named 'main'"}
```

Event line (only after `events.subscribe` on that connection):

```json
{"event": "instance.modified", "data": {"...": "..."}}
```

Rules:

- `id` may be any JSON value and is echoed back verbatim. If omitted it is
  echoed as `null`. Use it to correlate pipelined requests. Event lines carry
  no `id`.
- `params` is optional; commands that take no parameters accept `{}`, `null`
  or an absent field.
- Unparseable lines get `{"id": null, "ok": false, "error": "invalid json: ..."}`.
- Unknown commands get `ok: false` with `unknown command '<name>' (see docs/API.md)`.
- All writes on a connection (responses and events) are funneled through one
  writer, so lines never interleave mid-line.
- Requests on a single connection are processed sequentially: the server reads
  the next line only after the previous command's response has been queued. A
  slow command (for example `instances.worlds` polling its cache for up to
  10 seconds) delays later requests on the same connection. Open multiple
  connections for concurrency; event delivery is not blocked by an in-flight
  command.

## CLI client

The launcher binary doubles as a client. `--api` connects to the running
launcher's `api.sock`, sends one command, prints the raw JSON response line on
stdout and exits. It never takes the lockfile and never opens a window.

```
pandora_launcher --api <cmd> [--api-params '<json object>']
```

Exit codes:

| Code | Meaning |
|---|---|
| 0 | Response received with `"ok": true` (or clean end of an event stream) |
| 1 | Response received with `"ok": false` |
| 2 | Transport error: launcher not running, socket unreachable, invalid `--api-params` JSON, connection dropped |

Examples:

```sh
# No parameters
pandora_launcher --api launcher.status

# With parameters
pandora_launcher --api instances.start --api-params '{"name": "main"}'

# Machine-friendly: pipe through jq
pandora_launcher --api instances.list | jq '.data[].name'
```

`events.subscribe` switches the client to streaming mode: it prints the
acknowledgement, then one line per event, until interrupted (Ctrl-C) or until
the launcher exits:

```sh
pandora_launcher --api events.subscribe
{"id":0,"ok":true,"data":{"subscribed":true}}
{"event":"instance.modified","data":{"id":"0:1","name":"main","status":"launching","configuration":{...}}}
{"event":"instance.modified","data":{"id":"0:1","name":"main","status":"running","configuration":{...}}}
```

### Talking to the socket directly

With `nc`:

```sh
SOCK="$HOME/Library/Application Support/PandoraLauncher/api.sock"
echo '{"id":1,"cmd":"launcher.version"}' | nc -U "$SOCK"
```

With Python:

```python
import json, socket

sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect("/path/to/PandoraLauncher/api.sock")
sock.sendall(json.dumps({"id": 1, "cmd": "instances.list", "params": {}}).encode() + b"\n")
buf = b""
while b"\n" not in buf:
    buf += sock.recv(65536)
print(json.loads(buf.split(b"\n", 1)[0]))
```

When consuming a connection that is also subscribed to events, match responses
by `id` and treat lines with an `event` key as out-of-band.

## Command reference

### Addressing instances

Every command that targets an instance accepts either:

| Param | Type | Meaning |
|---|---|---|
| `id` | string | Slab address `"index:generation"`, e.g. `"0:1"`. Ephemeral, see [Caveats](#caveats-and-guarantees). |
| `name` | string | Instance name, matched case-insensitively. The stable address. |

If neither resolves: `ok: false` with `no instance with id ...`, `no instance
named '...'`, or `provide 'id' ("index:generation") or 'name'`.

### Launcher

#### `launcher.version`

No parameters.

```json
{"id": 1, "ok": true, "data": {"version": "5.4.0", "protocol": 1}}
```

`version` is the release version baked in at build time (`dev` for local
builds).

#### `launcher.status`

No parameters.

```json
{
  "id": 1, "ok": true,
  "data": {
    "version": "5.4.0",
    "uptime_secs": 5133,
    "instances_total": 3,
    "instances_by_status": {"not_running": 2, "running": 1},
    "launcher_dir": "/Users/me/Library/Application Support/PandoraLauncher"
  }
}
```

`uptime_secs` counts from API server start (launcher startup). Status keys are
`not_running`, `launching`, `running`, `stopping`.

#### `launcher.focus`

No parameters. Opens or focuses the main window.

```json
{"id": 1, "ok": true, "data": {"focused": true}}
```

#### `launcher.quit`

No parameters. Asks the launcher to quit. The response is sent before shutdown;
the socket disappears shortly after.

```json
{"id": 1, "ok": true, "data": {"quitting": true}}
```

#### `launcher.install_update`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `expected_version` | string | no | Guard: fail unless the pending update's version matches |

Installs the launcher self-update that was most recently offered this session.
The offer is captured from the backend's `launcher.update_available` stream; the
update payload is not client-constructible, so an update must have been offered
first (watch for the [`launcher.update_available`](#launcherupdate_available)
event). Returns an op:

```json
{"id": 1, "ok": true, "data": {"op": 8, "note": "the launcher restarts into the new version on success"}}
```

Poll the op for progress and errors. On success the launcher restarts into the
new version. Errors: `no update available (none offered this session; check
launcher.update_available events)`; `expected_version '<x>' does not match the
available update '<y>'` when the guard mismatches.

### Instances

#### `instances.list`

No parameters. Returns an array of instance objects:

```json
{
  "id": 1, "ok": true,
  "data": [
    {
      "id": "0:1",
      "name": "main",
      "status": "not_running",
      "root_path": "/Users/me/Library/Application Support/PandoraLauncher/instances/main",
      "dot_minecraft_path": "/Users/me/Library/Application Support/PandoraLauncher/instances/main/.minecraft",
      "configuration": {
        "minecraft_version": "26.2",
        "loader": "vanilla",
        "memory": {"enabled": true, "min": 512, "max": 2048}
      },
      "playtime": {"total_secs": 7421, "session_count": 12, "last_played_unix_ms": 1765400000000}
    }
  ]
}
```

`configuration` is the instance's serialized configuration; fields at their
default values may be omitted.

#### `instances.get`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |

Returns a single instance object (same shape as `instances.list` entries).

Errors: unknown id/name.

#### `instances.create`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `name` | string | yes | New instance name |
| `version` | string | yes | Minecraft version, e.g. `"26.2"` |
| `loader` | string | no | `"vanilla"` (default), `"fabric"`, `"forge"`, `"neoforge"` (case-insensitive). Unrecognized strings are rejected. |

```json
{"id": 1, "ok": true, "data": {"requested": true, "name": "api-test"}}
```

Creation is asynchronous: the instance appears in `instances.list` (and an
`instance.added` event fires) once the backend has created it, typically
within a second or two.

Errors: missing `name`/`version`.

#### `instances.delete`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Deletes the instance folder from disk and deregisters the instance from the
registry immediately (an `instance.removed` event fires). If the folder cannot
be removed, an error notification is emitted and the instance stays
registered.

#### `instances.rename`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `new_name` | string | yes | New name |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

#### `instances.start`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `quick_play_singleplayer` | string | no | World to join directly (world folder name) |
| `quick_play_multiplayer` | string | no | Server address to join directly |

Returns an operation id immediately:

```json
{"id": 1, "ok": true, "data": {"op": 3}}
```

Poll `ops.status` for progress trackers (asset downloads, library
verification) and errors; watch `instances.get` or `instance.modified` events
for the `launching` to `running` transition. Launching an instance that is
already launching finishes the op with error `Can't launch instance, already
launching`.

If the selected account's tokens are expired and cannot be refreshed
non-interactively, the launch op may surface a `visit_url` for re-login (see
[Async operations](#async-operations)).

#### `instances.kill`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Kills the running game process. Verify via `instances.get` reaching
`not_running`.

#### `instances.set`

Applies one or more configuration changes. At least one settable field must be
present.

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `minecraft_version` | string | no | Change the Minecraft version |
| `loader` | string | no | Change the loader (`vanilla`, `fabric`, `forge`, `neoforge`) |
| `preferred_account` | string or null | no | Account UUID to launch with, `null` to clear |
| `disable_file_syncing` | bool | no | Opt the instance out of file syncing |
| `sandbox` | bool | no | Toggle sandboxed launching |
| `memory` | object | no | `{"enabled": bool, "min": u32, "max": u32}` in MiB |
| `jvm_flags` | object | no | `{"enabled": bool, "flags": "..."}` extra JVM flags |
| `jvm_binary` | object | no | `{"enabled": bool, "path": "/path/to/java" or null}` custom JVM |
| `wrapper_command` | object | no | `{"enabled": bool, "flags": "..."}` wrapper command line |
| `preferred_loader_version` | string or null | no | Pin a loader version, `null` to unpin |
| `linux_wrapper` | object | no | Linux launch helpers, fields below |
| `system_libraries` | object | no | Use system GLFW/OpenAL instead of the bundled ones, fields below |
| `icon` | object or null | no | Instance icon, forms below |

`linux_wrapper` fields (all optional booleans, applied only on Linux):

| Field | Type | Default | Meaning |
|---|---|---|---|
| `use_mangohud` | bool | `false` | Launch under MangoHud |
| `use_gamemode` | bool | `false` | Launch under Feral GameMode |
| `use_discrete_gpu` | bool | `true` | Prefer the discrete GPU |
| `disable_gl_threaded_optimizations` | bool | `false` | Disable driver threaded GL optimizations |

`system_libraries` fields:

| Field | Type | Meaning |
|---|---|---|
| `override_glfw` | bool | Use a system GLFW instead of the bundled one |
| `glfw` | LwjglLibraryPath | Which GLFW to use (see below) |
| `override_openal` | bool | Use a system OpenAL instead of the bundled one |
| `openal` | LwjglLibraryPath | Which OpenAL to use (see below) |

A `LwjglLibraryPath` is an externally-tagged enum: the string `"Auto"` (let the
launcher discover it), `{"AutoPreferred": "/path/to/lib"}` (prefer this path,
fall back to auto), or `{"Explicit": "/path/to/lib"}` (use exactly this path).

`icon` is one of: `{"embedded": "name"}` (a built-in fallback icon by name),
`{"png_base64": "..."}` (raw PNG, base64), `{"path": "/path/to/icon.png"}` (a
local PNG file, must be a regular file no larger than 8 MiB), or `null` to clear
the icon.

```json
{"id": 1, "ok": true, "data": {"applied": ["memory", "sandbox"]}}
```

Notes:

- Changes are applied asynchronously; read back with `instances.get` to
  verify (allow around a second for persistence).
- `disable_file_syncing` and `sandbox` are only applied when passed as JSON
  booleans; other types are silently skipped.
- Validation is two-phase: every provided field is parsed and validated before
  any change is sent, so a single bad field causes the whole call to fail with
  nothing applied. `memory` additionally requires `min > 0` and `min <= max`
  when `enabled`.
- If no recognized field is present:
  `no settable field provided (minecraft_version, loader, preferred_account, disable_file_syncing, sandbox, memory, jvm_flags, jvm_binary, wrapper_command, preferred_loader_version, linux_wrapper, system_libraries, icon)`.

Errors: an invalid loader, uuid, memory bounds, `LwjglLibraryPath`/icon shape,
or any other sub-object shape fails the entire call with a descriptive message;
no field is applied. An `icon` with `{"path": ...}` is read and validated
in-handler, so a missing file, a non-regular file, an over-8-MiB file, or
non-PNG bytes fails the whole call.

#### `instances.update_check`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |

Returns `{"op": N}`. Scans all content folders of the instance and checks
Modrinth/CurseForge for available content updates. Results surface in the UI;
over the API, poll the op for completion and errors.

#### `instances.logs`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |

```json
{
  "id": 1, "ok": true,
  "data": {
    "total_gzipped_size": 48213,
    "paths": [
      "/Users/me/.../instances/main/.minecraft/logs/latest.log",
      "/Users/me/.../instances/main/.minecraft/logs/2026-08-12-1.log.gz"
    ]
  }
}
```

An instance that has never launched returns an empty list. Errors: `timed out
waiting for log list` after 10 seconds.

#### `logs.read`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `path` | string | yes | A path returned by `instances.logs` |
| `max_lines` | number | no | Cap on returned lines (default 500) |

```json
{"id": 1, "ok": true, "data": {"lines": ["[12:00:01] [main/INFO]: ..."], "truncated": false}}
```

Gzipped logs are decompressed transparently. Reading stops at `max_lines`, end
of file, or a 15 second deadline, whichever comes first; `truncated` is true
when the line cap was hit.

#### `instances.worlds`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |

```json
{
  "id": 1, "ok": true,
  "data": [
    {
      "title": "New World",
      "subtitle": "New World (2026-08-13 14:03)",
      "level_path": "/Users/me/.../instances/main/.minecraft/saves/New World",
      "last_played": 1765400000000
    }
  ]
}
```

Triggers a (re)load and polls the cache for up to 10 seconds. On a very large
folder that has not finished loading: `data not loaded yet; retry shortly`.

#### `instances.servers`

Same addressing as `instances.worlds`.

```json
{"id": 1, "ok": true, "data": [{"name": "My Server", "ip": "play.example.com"}]}
```

Same load-and-poll semantics as `instances.worlds`.

#### `instances.content`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `folder` | string | yes | `"mods"`, `"resourcepacks"`, or `"shaders"` (alias `"shaderpacks"`) |

```json
{
  "id": 1, "ok": true,
  "data": [
    {
      "content_id": "0:1",
      "filename": "sodium-fabric-0.6.13.jar",
      "name": "Sodium",
      "version": "0.6.13",
      "authors": "JellySquid",
      "enabled": true,
      "can_toggle": true,
      "path": "/Users/me/.../instances/main/.minecraft/mods/sodium-fabric-0.6.13.jar"
    }
  ]
}
```

`content_id` is a slab address with the same ephemerality rules as instance
ids. Same load-and-poll semantics as `instances.worlds`. Unknown folder:
`unknown content folder '<x>' (mods|resourcepacks|shaders)`.

#### `instances.relocate`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `path` | string | yes | New location for the instance folder |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Moves the instance's folder to `path`. Fire-and-forget; verify with
`instances.get` (its `root_path` changes) once the move completes.

#### `instances.create_shortcut`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `path` | string | yes | Destination path for the shortcut |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Creates a shortcut that launches this instance directly. The parent directory
of `path`, when specified, must already exist:
`shortcut parent directory <dir> does not exist` otherwise. Fire-and-forget.

#### `servers.reorder`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `from_index` | number | yes | Current position in the server list (0-based) |
| `to_index` | number | yes | Target position |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Reorders the instance's saved multiplayer server list. Read the current order
with `instances.servers`. Fire-and-forget; re-read to verify.

#### `logs.cleanup`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Deletes the instance's old rotated log files (the launcher's log-retention
pass). Fire-and-forget.

#### `logs.upload`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `path` | string | yes | A log path as returned by `instances.logs` |

Uploads the log file to mclo.gs and returns an op; the resulting URL appears in
the op's `visit_url` when it finishes:

```json
{"id": 1, "ok": true, "data": {"op": 9, "hint": "poll ops.status; the mclo.gs url appears in visit_url when done"}}
```

Poll `ops.status`; on success `visit_url.url` is the mclo.gs paste. This runs
inline on the backend loop and briefly blocks other commands (see
[Caveats](#caveats-and-guarantees)).

### Content

#### `content.set_enabled`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `content_ids` | array of string | yes | Content slab addresses from `instances.content` |
| `enabled` | bool | yes | Target state |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Enabling/disabling renames the file (`.disabled` suffix). Entries with
`can_toggle: false` are not toggleable. Verify by re-listing.

Errors: missing/empty `content_ids`, malformed entries
(`content_ids entries must be 'index:generation' strings`).

#### `content.delete`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `content_ids` | array of string | yes | Content slab addresses |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Deletes the files from disk. Verify by re-listing.

#### `content.update`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `content_ids` | array of string | yes | Exactly one content slab address |

Returns `{"op": N}`. Updates that piece of content to the latest compatible
version. Error if the array does not contain exactly one entry:
`content.update takes exactly one content_id`.

#### `content.install`

Installs one or more files into an existing instance or a brand new one.

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of these three | Install into an existing instance |
| `new_instance_name` | string | | Create a new instance and install into it |
| `loader` | string | see note | Loader context for resolution (`vanilla`/`fabric`/`forge`/`neoforge`). Inherited from the target instance when omitted; required for `new_instance_name`. |
| `minecraft_version` | string | see note | Minecraft version context, e.g. `"26.2"`. Inherited from the target instance when omitted; required for `new_instance_name`. |
| `files` | array | yes | One source object per file, see below |

Each `files[]` entry contains exactly one of:

| Key | Fields | Meaning |
|---|---|---|
| `modrinth` | `project_id` (string, required), `version_id` (string, optional; latest compatible when omitted), `install_dependencies` (bool, default `true`) | Install from Modrinth |
| `curseforge` | `project_id` (number, required), `install_dependencies` (bool, default `true`) | Install from CurseForge |
| `file` | `path` (string, required) | Import a local file |

Example:

```json
{
  "id": 1,
  "cmd": "content.install",
  "params": {
    "name": "main",
    "loader": "fabric",
    "minecraft_version": "26.2",
    "files": [
      {"modrinth": {"project_id": "AANobbMI"}},
      {"file": {"path": "/tmp/custom-mod.jar"}}
    ]
  }
}
```

Returns `{"op": N}`. Download progress appears as op trackers; the op error
slot is set on resolution or download failure.

Errors: no target
(`provide 'id'/'name' (existing instance) or 'new_instance_name'`), missing
`files` array, an entry without a recognized source
(`each file needs 'modrinth', 'curseforge' or 'file'`), missing
`minecraft_version`.

#### `content.search`

Searches Modrinth or CurseForge for installable content. Results are served
through the metadata manager and cached (see [Caveats](#caveats-and-guarantees)).

| Param | Type | Required | Meaning |
|---|---|---|---|
| `source` | string | yes | `"modrinth"` or `"curseforge"` |
| `query` | string | no | Search text |
| `limit` | number | no | Results per page (default 20; capped at 100 for Modrinth, 50 for CurseForge) |
| `offset` | number | no | Result offset for paging (default 0) |
| `facets` | string | no | Modrinth only: raw facets JSON string passed through to the Modrinth API |
| `class_id` | number | no | CurseForge only: content class (default `6` = Mod) |
| `game_version` | string | no | CurseForge only: filter by Minecraft version |

Modrinth response:

```json
{
  "id": 1, "ok": true,
  "data": {
    "source": "modrinth",
    "total": 1234,
    "offset": 0,
    "hits": [
      {"project_id": "AANobbMI", "title": "Sodium", "description": "...", "author": "jellysquid3", "downloads": 40000000, "icon_url": "https://...", "project_type": "mod"}
    ]
  }
}
```

CurseForge response (no total/offset echo; paging is via `offset`):

```json
{
  "id": 1, "ok": true,
  "data": {
    "source": "curseforge",
    "hits": [
      {"project_id": 394468, "name": "Sodium", "slug": "sodium", "summary": "...", "downloads": 5000000, "class_id": 6}
    ]
  }
}
```

Errors: missing `source`, `unknown source '<x>' (modrinth|curseforge)`,
`modrinth search failed: ...` / `curseforge search failed: ...` on upstream
failure.

#### `content.versions`

Lists the available versions/files of a single project. Same metadata caching as
`content.search`.

| Param | Type | Required | Meaning |
|---|---|---|---|
| `source` | string | yes | `"modrinth"` or `"curseforge"` |
| `project_id` | string or number | yes | Modrinth project id (string) or CurseForge mod id (number) |
| `game_version` | string | no | CurseForge only: filter by Minecraft version |

Modrinth response:

```json
{
  "id": 1, "ok": true,
  "data": {
    "source": "modrinth",
    "versions": [
      {"version_id": "abcd1234", "project_id": "AANobbMI", "name": "Sodium 0.6.13", "version_number": "0.6.13", "game_versions": ["26.2"]}
    ]
  }
}
```

CurseForge response:

```json
{
  "id": 1, "ok": true,
  "data": {
    "source": "curseforge",
    "files": [
      {"file_id": 5678, "mod_id": 394468, "file_name": "sodium-fabric-0.6.13.jar", "file_length": 918273, "download_url": "https://..."}
    ]
  }
}
```

The `version_id`/`file_id` values feed `content.install` (Modrinth `version_id`,
CurseForge project id). Errors: missing `source`/`project_id`, `unknown source
'<x>' (modrinth|curseforge)`, `curseforge project_id must be a number`.

#### `content.set_child_enabled`

Enables or disables a single child entry inside a multi-file content item (for
example one mod within an installed modpack).

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `content_id` | string | yes | Parent content slab address from `instances.content` |
| `child_filename` | string | yes | The child's file name |
| `enabled` | bool | yes | Target state |
| `child_id` | string | no | Child identifier, when known |
| `child_name` | string | no | Human-readable child name |
| `disabled_default` | bool | no | Whether the child is disabled by default (default `false`) |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Fire-and-forget; verify by re-listing. Errors: invalid/missing `content_id`
(`invalid content id '<x>'`), missing `child_filename`/`enabled`.

#### `content.download_children`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `content_id` | string | yes | Parent content slab address |

Returns `{"op": N}`. Downloads the not-yet-present child files of a multi-file
content item (for example the mods a modpack references but has not fetched).
This runs inline on the backend loop and briefly blocks other commands (see
[Caveats](#caveats-and-guarantees)); poll the op for progress and errors.

### Import and export

#### `instances.export`

Exports an instance to an archive. Returns `{"op": N}`; the archive is written
to `output` when the op finishes.

| Param | Type | Required | Meaning |
|---|---|---|---|
| `id` or `name` | string | one of | Instance address |
| `format` | string | yes | `"zip"`, `"modrinth"` (.mrpack) or `"curseforge"` |
| `output` | string | yes | Destination file path |
| `name` | string | no | Pack name embedded in the modrinth/curseforge manifest (default `"export"`) |
| `version` | string | no | Pack version embedded in the manifest (default `"1.0.0"`) |
| `include_mods` | bool | no | Include the mods folder (default `true`) |
| `include_resourcepacks` | bool | no | Include resource packs (default `true`) |
| `include_shaders` | bool | no | Include shaders (default `true`) |
| `include_configs` | bool | no | Include config files (default `true`) |
| `include_saves` | bool | no | Include worlds (default `false`) |
| `include_screenshots` | bool | no | Include screenshots (default `false`) |
| `include_backups` | bool | no | Include backups (default `false`) |
| `include_logs` | bool | no | Include logs (default `false`) |
| `include_cache` | bool | no | Include cache (default `false`) |
| `include_synced` | bool | no | Include synced files (default `false`) |

```json
{"id": 1, "ok": true, "data": {"op": 10}}
```

Poll the op for progress and errors. Errors: missing `format`/`output`,
`unknown export format '<x>' (zip|modrinth|curseforge)`.

#### `instances.import_file`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `file` | string | yes | Path to a Modrinth `.mrpack` file |

Returns `{"op": N}`. Creates a new instance from a Modrinth `.mrpack` pack file
(only `.mrpack` is accepted; a non-mrpack finishes the op with error
`Not a .mrpack file`). Poll the op for progress and errors. Up-front errors:
missing `file`, `file <path> does not exist or is not a regular file`.

#### `import.scan`

Scans another launcher's install directory for importable instances.

| Param | Type | Required | Meaning |
|---|---|---|---|
| `launcher` | string | yes | `"Prism"`, `"CurseForge"`, `"Modrinth"`, `"MultiMC"` or `"ATLauncher"` |
| `path` | string | yes | Path to that launcher's data directory |

```json
{
  "id": 1, "ok": true,
  "data": {
    "found": true,
    "import_accounts": true,
    "root": "/path/to/other/launcher",
    "paths": ["/path/to/other/launcher/instances/foo", "..."]
  }
}
```

When nothing importable is found: `{"found": false}`. `paths` is the set of
instance folders you can pass to `import.run`; `import_accounts` reports whether
that launcher's accounts can be imported too. Errors: `unknown launcher '<x>'
(Prism|CurseForge|Modrinth|MultiMC|ATLauncher)`, `timed out scanning launcher`
after 15 seconds.

#### `import.run`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `launcher` | string | yes | Same set as `import.scan` |
| `root` | string | yes | The `root` returned by `import.scan` |
| `paths` | array of string | yes | Instance folders to import (a subset of `import.scan`'s `paths`) |
| `import_accounts` | bool | no | Also import that launcher's accounts (default `false`) |

Returns `{"op": N}`. Imports the selected instances (and optionally accounts).
This runs inline on the backend loop and briefly blocks other commands (see
[Caveats](#caveats-and-guarantees)); poll the op for progress and errors.
Errors: `unknown launcher ...`, missing `root`/`paths`, `paths entries must be
strings`.

### Accounts

#### `accounts.list`

No parameters.

```json
{
  "id": 1, "ok": true,
  "data": {
    "selected": "069a79f4-44e9-4726-a5be-fca90e38aaf5",
    "accounts": [
      {"uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5", "username": "Notch", "offline": false},
      {"uuid": "d979977e-9ba9-3aec-a49a-a4dd6f1c0dc7", "username": "ApiProbe", "offline": true}
    ]
  }
}
```

`selected` is `null` when no account is selected.

#### `accounts.select`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `uuid` | string | yes | Account UUID |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Verify via `accounts.list` (`selected`) or the `accounts.updated` event.

#### `accounts.delete`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `uuid` | string | yes | Account UUID |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

#### `accounts.add_offline`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `username` | string | yes | Offline account name |
| `uuid` | string | no | Explicit UUID; derived when omitted |

```json
{"id": 1, "ok": true, "data": {"requested": true, "uuid": "d979977e-9ba9-3aec-a49a-a4dd6f1c0dc7"}}
```

When `uuid` is omitted, it is derived with vanilla's `OfflinePlayer:` scheme
(md5-based UUIDv3), so the account gets the same UUID an offline-mode server
computes from the username.

#### `accounts.login`

No parameters. Starts an interactive Microsoft login:

```json
{"id": 1, "ok": true, "data": {"op": 5, "hint": "poll ops.status for visit_url, open it in a browser, enter the code"}}
```

See the [walkthrough](#microsoft-login-walkthrough) below.

#### `accounts.reorder`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `from_index` | number | yes | Current position in the account list (0-based) |
| `delta` | number | yes | Signed offset to move by (negative moves up, positive moves down) |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Moves the account at `from_index` by `delta` positions. Fire-and-forget; verify
with `accounts.list`.

#### `accounts.reauth`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `uuid` | string | yes | UUID of an existing non-offline account |

Re-runs the Microsoft login for an already-registered account, for example to
refresh expired tokens. Returns an op:

```json
{"id": 1, "ok": true, "data": {"op": 6, "hint": "poll ops.status for visit_url if interaction is required"}}
```

If the tokens can be refreshed silently the op finishes without a `visit_url`;
otherwise it surfaces a `visit_url` to complete in a browser, exactly like
`accounts.login`. Errors: `no account with uuid <uuid>`, `offline accounts have
no Microsoft credentials to refresh`.

#### `accounts.skin_get`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `uuid` | string | yes | UUID of a non-offline account |

Fetches the account's current Minecraft skin:

```json
{"id": 1, "ok": true, "data": {"status": "ok", "variant": "classic", "skin_png_base64": "iVBORw0KGgo..."}}
```

`status` is one of `ok`, `needs_login` (the account must be re-authenticated,
see `accounts.reauth`), or `unable_to_load`. On `ok`, `variant` is `classic` or
`slim` and `skin_png_base64` is the PNG (may be `null` when the account has no
custom skin). Offline accounts are rejected up front: `offline accounts have no
Microsoft skin/cape`. Other errors: `no account with uuid <uuid>`, `timed out
fetching skin` after 30 seconds.

#### `accounts.skin_set`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `uuid` | string | yes | Account UUID |
| `skin` | string | yes | Base64-encoded PNG skin |
| `variant` | string | yes | `"classic"` or `"slim"` |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Fire-and-forget. Errors: missing `uuid`/`skin`/`variant`, `invalid base64: ...`,
`decoded bytes are not a PNG`, `unknown skin variant '<x>' (classic|slim)`.

#### `accounts.capes_get`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `uuid` | string | yes | UUID of a non-offline account |

Lists the capes owned by the account:

```json
{
  "id": 1, "ok": true,
  "data": {
    "status": "ok",
    "capes": [
      {"id": "...", "state": "active", "url": "https://...", "alias": "Migrator"}
    ]
  }
}
```

`status` is `ok` or `needs_login`. Offline accounts are rejected up front.
Errors: `no account with uuid <uuid>`, `offline accounts have no Microsoft
skin/cape`, `timed out fetching capes` after 30 seconds.

#### `accounts.cape_set`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `uuid` | string | yes | Account UUID |
| `cape` | string or null | no | Cape UUID to equip; `null` (or omitted) unequips |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Fire-and-forget. `cape` is one of the ids from `accounts.capes_get`. Errors:
`invalid cape uuid: ...`.

### Skins

The launcher keeps a personal skin library (a set of saved skin PNGs) that is
separate from any account. These commands manage it.

#### `skins.library`

No parameters. Triggers a load and polls the skin manager's cached snapshot for
up to 10 seconds:

```json
{"id": 1, "ok": true, "data": {"count": 2, "skins_png_base64": ["iVBORw0KGgo...", "iVBORw0KGgo..."]}}
```

If the library has not finished loading: `skin library not loaded yet; retry
shortly` (retrying is correct).

#### `skins.add`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `url` | string | one of | Fetch the skin from a URL |
| `file` | string | one of | Import a local PNG file |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Provide exactly one of `url` or `file`. Fire-and-forget; re-read with
`skins.library`. Errors: `provide 'url' or 'file'`, `file <path> does not exist
or is not a regular file`.

#### `skins.remove`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `skin` | string | yes | Base64-encoded PNG of the skin to remove (as returned by `skins.library`) |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

The skin is matched by its bytes. Errors: `invalid base64: ...`, `decoded bytes
are not a PNG`.

#### `skins.copy_player`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `username` | string | yes | Minecraft username whose current skin to copy into the library |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

Fetches the named player's current skin and adds it to the library.
Fire-and-forget.

### Sync

#### `sync.state`

No parameters.

```json
{
  "id": 1, "ok": true,
  "data": {
    "sync_folder": "/Users/me/.../PandoraLauncher/sync",
    "total_count": 4,
    "targets": [
      {"target": "options.txt", "enabled": true, "is_file": true, "sync_count": 3, "cannot_sync_count": 0},
      {"target": "resourcepacks", "enabled": false, "is_file": false, "sync_count": 0, "cannot_sync_count": 1}
    ]
  }
}
```

Errors: `timed out` after 10 seconds, `backend dropped the request`.

#### `sync.set`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `target` | string | yes | Target name as returned by `sync.state` |
| `is_file` | bool | yes | Whether the target is a file (must match the target's kind) |
| `value` | bool | yes | Enable or disable syncing for the target |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

### Settings

#### `settings.get`

No parameters.

```json
{
  "id": 1, "ok": true,
  "data": {
    "open_game_output_when_launching": true,
    "proxy": {
      "enabled": false,
      "protocol": "HTTP",
      "host": "",
      "port": 0,
      "auth_enabled": false,
      "username": "",
      "has_password": false
    }
  }
}
```

The proxy password is deliberately never returned; `has_password` only reports
whether one is stored in the system keyring.
`open_game_output_when_launching` is exposed in positive form (the on-disk
flag is stored inverted; the API hides that).

#### `settings.set_open_game_output`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `value` | bool | yes | Open the game output window when launching |

```json
{"id": 1, "ok": true, "data": {"requested": true}}
```

#### `settings.set_proxy`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `proxy` | object | yes | Proxy configuration, fields below |
| `password` | string | no | Proxy password; stored in the system keyring. Empty string deletes the stored password. Omitted leaves it unchanged. |

`proxy` fields (all optional, defaulting to disabled/empty):

| Field | Type | Meaning |
|---|---|---|
| `enabled` | bool | Use the proxy |
| `protocol` | string | `"Http"` (default), `"Https"`, `"Socks5"` (exact casing; `settings.get` reports the display form `"HTTP"`/`"HTTPS"`/`"SOCKS5"`) |
| `host` | string | Proxy host |
| `port` | number | Proxy port |
| `auth_enabled` | bool | Send credentials |
| `username` | string | Proxy username |

```json
{"id": 1, "ok": true, "data": {"requested": true, "note": "restart the launcher for proxy changes to take effect"}}
```

Proxy changes only take effect after a launcher restart (the HTTP client is
built at startup).

### Metadata

#### `metadata.minecraft_versions`

No parameters. Returns the cached Minecraft version manifest (Mojang
`version_manifest.json` format):

```json
{
  "id": 1, "ok": true,
  "data": {
    "latest": {"release": "26.2", "snapshot": "26.3-pre1"},
    "versions": [
      {"id": "26.2", "type": "release", "url": "https://piston-meta.mojang.com/..."}
    ]
  }
}
```

If the cache does not exist yet, a fetch is triggered and the call fails with
`version manifest not cached yet; fetch triggered, retry shortly`. Retry after
a few seconds.

#### `metadata.download_all`

No parameters. Fire-and-forget:

```json
{"id": 1, "ok": true, "data": {"requested": true, "warning": "downloads every version+asset index; blocks the backend loop for minutes and cannot report progress"}}
```

Prefetches the entire metadata set (every version manifest and asset index) for
offline use. This is heavy: it issues 1000+ fetches. The backend spawns it onto
its own task (so, despite the wording of the returned `warning`, it does not
actually park the message loop), and there is no op handle and no progress
reporting. Because it saturates the network and metadata caches, avoid issuing
it alongside latency-sensitive commands. See
[Caveats](#caveats-and-guarantees).

### Operations

See [Async operations](#async-operations) for the model.

#### `ops.list`

No parameters. Returns all tracked ops (running, plus finished ops for up to
an hour), sorted by op id:

```json
{"id": 1, "ok": true, "data": [{"op": 3, "label": "instances.start", "finished": true, "error": null, "visit_url": null, "cancel_requested": false, "age_secs": 210, "progress": []}]}
```

#### `ops.status`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `op` | number | yes | Op id returned by an async command |

```json
{
  "id": 1, "ok": true,
  "data": {
    "op": 3,
    "label": "instances.start",
    "finished": false,
    "error": null,
    "visit_url": null,
    "cancel_requested": false,
    "age_secs": 4,
    "progress": [
      {"title": "Downloading assets", "count": 118, "total": 3407, "done": false}
    ]
  }
}
```

Errors: missing/non-numeric `op`, `unknown op (pruned or never existed)`.

#### `ops.cancel`

| Param | Type | Required | Meaning |
|---|---|---|---|
| `op` | number | yes | Op id |

```json
{"id": 1, "ok": true, "data": {"cancel_requested": true}}
```

Cancellation is cooperative: the op observes the request at its next
checkpoint. Poll `ops.status` until `finished` is true. Errors: `unknown op`.

## Async operations

Commands that start long-running work (`instances.start`,
`instances.update_check`, `content.update`, `content.install`,
`content.download_children`, `instances.export`, `instances.import_file`,
`import.run`, `accounts.login`, `accounts.reauth`, `logs.upload`,
`launcher.install_update`) return `{"op": N}` immediately. Each op wraps the
same `ModalAction` structure the UI uses for its progress modals:

| Field | Meaning |
|---|---|
| `finished` | The op has completed (successfully or not) |
| `error` | Error message, or `null`. An op can finish with `error` set: treat that as failure. |
| `visit_url` | `{"message", "url"}` when the op needs the user to open a URL (Microsoft login), else `null` |
| `progress` | Array of trackers `{"title", "count", "total", "done"}`; trackers appear and complete as phases run |
| `cancel_requested` | A cancel has been requested via `ops.cancel` |
| `age_secs` | Seconds since the op was registered |
| `label` | The command that created the op |

Semantics:

- Success: `finished: true` and `error: null`.
- Failure: `finished: true` and `error` set.
- `visit_url` means the op is blocked on user interaction; it clears once the
  interaction completes.
- Op ids are process-local, monotonically increasing, and pruned one hour
  after finishing (pruning runs when new ops are registered). Running ops are
  never pruned.
- Poll interval of about one second is appropriate.

### Microsoft login walkthrough

`accounts.login` drives the same interactive Microsoft OAuth flow as the UI
(authorization code with PKCE, completed via a local redirect server on
`http://localhost:3160/auth`). The browser used to sign in must therefore run
on the same machine as the launcher.

1. Start the flow:

   ```sh
   pandora_launcher --api accounts.login
   {"id":0,"ok":true,"data":{"op":5,"hint":"poll ops.status for visit_url, open it in a browser, enter the code"}}
   ```

2. Poll until `visit_url` appears:

   ```sh
   pandora_launcher --api ops.status --api-params '{"op": 5}'
   ```

   ```json
   {
     "id": 0, "ok": true,
     "data": {
       "op": 5, "label": "accounts.login", "finished": false, "error": null,
       "visit_url": {
         "message": "Login with Microsoft",
         "url": "https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize?..."
       },
       "cancel_requested": false, "age_secs": 2, "progress": []
     }
   }
   ```

3. Open the URL in a browser on the launcher's machine and complete the
   Microsoft sign-in. The browser is redirected to the launcher's local
   redirect server, which resumes the flow (Xbox Live and Minecraft token
   exchange).

4. Keep polling: `visit_url` clears, then `finished` becomes `true`. On
   success the new account appears in `accounts.list` and an
   `accounts.updated` event fires. On failure `error` is set (for example a
   cancelled browser flow).

5. To abort a pending login, call `ops.cancel`; the op finishes with a
   cancellation error.

Note: the login runs inline on the backend message loop; see
[Caveats](#caveats-and-guarantees) for what that blocks while the browser step
is pending.

## Events

`events.subscribe` turns on event delivery for the current connection only:

```json
{"id": 7, "cmd": "events.subscribe"}
{"id": 7, "ok": true, "data": {"subscribed": true}}
```

- The subscription is per-connection and lasts until the connection closes.
  There is no unsubscribe; close the connection instead.
- Calling `events.subscribe` again on the same connection re-acknowledges
  without duplicating delivery.
- Events are interleaved with responses on the same connection, always on
  whole-line boundaries. Distinguish them by the presence of the `event` key
  (responses have `id`/`ok` instead).
- Events reflect backend-to-frontend state broadcasts; only the types below
  are forwarded.

### Lag

Delivery is fan-out from a fixed 512-entry broadcast buffer. A consumer that
reads too slowly loses the oldest events and receives:

```json
{"event": "events.lagged", "data": {"missed": 12}}
```

After a lag notice, resynchronize with reads (`instances.list`,
`accounts.list`) rather than assuming continuity.

### Event types

#### `instance.added`

```json
{"event": "instance.added", "data": {"id": "2:1", "name": "api-test", "configuration": {"minecraft_version": "26.2", "loader": "vanilla"}}}
```

#### `instance.removed`

```json
{"event": "instance.removed", "data": {"id": "2:1"}}
```

#### `instance.modified`

Fires on status transitions and configuration changes:

```json
{"event": "instance.modified", "data": {"id": "0:1", "name": "main", "status": "running", "configuration": {"minecraft_version": "26.2", "loader": "vanilla"}}}
```

#### `instance.playtime`

Periodic while a game session is running:

```json
{"event": "instance.playtime", "data": {"id": "0:1", "total_secs": 7480, "session_secs": 59}}
```

#### `accounts.updated`

Full account list on any account change:

```json
{"event": "accounts.updated", "data": {"selected": "069a79f4-44e9-4726-a5be-fca90e38aaf5", "accounts": [{"uuid": "069a79f4-44e9-4726-a5be-fca90e38aaf5", "username": "Notch", "offline": false}]}}
```

#### `notification`

User-facing notifications (the launcher's toast messages). `level` is one of
`success`, `info`, `error`, `warning`:

```json
{"event": "notification", "data": {"level": "info", "message": "Proxy settings saved. Restart the launcher to apply changes."}}
```

#### `launcher.update_available`

```json
{"event": "launcher.update_available", "data": {"version": "5.5.0"}}
```

#### `game.output`

Streams the running game's stdout/stderr, tagged by instance:

```json
{"event": "game.output", "data": {"instance": "0:1", "time": 1765400000123, "level": "info", "lines": ["[12:00:01] [main/INFO]: Loading Minecraft", "..."]}}
```

- `instance` is the slab address (`index:generation`).
- `time` is the capture time in epoch milliseconds.
- `level` is the parsed log level, one of `fatal`, `error`, `warn`, `info`,
  `debug`, `trace`, `other`.
- `lines` is the batch of console lines for this event.

This event only streams while the game-output capture is active, which is gated
by the "open game output" setting (`open_game_output_when_launching`, see
[`settings.get`](#settingsget) / `settings.set_open_game_output`). With capture
off, no `game.output` events are emitted even for a running game.

## Caveats and guarantees

### Ids are ephemeral, names are stable

Instance ids (`"0:1"`) and content ids are slab addresses
(`index:generation`) valid only for the current launcher process. A restart,
or deletion plus re-creation, changes them (the generation increments so a
stale id is rejected rather than hitting the wrong instance). For scripting,
address instances by `name`; treat `id` as a per-session handle. Content ids
must be re-read from `instances.content` before each batch of operations.

### Fire-and-forget mutations, read-back to verify

Responses of the form `{"requested": true}` mean the command was enqueued to
the backend, not that it was applied. State changes (create, rename, set,
select, sync, settings) persist asynchronously, usually within a second.
Verify with the corresponding read command or by watching events.

### Eventual consistency of worlds, servers and content

`instances.worlds`, `instances.servers` and `instances.content` are backed by
watcher-driven caches. Each call triggers a reload and polls the cache for up
to 10 seconds; on very large folders the call can fail with `data not loaded
yet; retry shortly`, in which case retrying is correct. Listings can also lag
external filesystem changes by the watcher's debounce interval.

### The message loop and long operations

The backend processes its message queue sequentially. The long-running
operations that could otherwise park it are spawned onto their own tasks:
instance launch (`instances.start`, with its asset/Java downloads and login
refresh) and interactive login (`accounts.login` / `accounts.reauth`, which
wait on a browser redirect), plus `content.install` and the heavy
`metadata.download_all` prefetch. They report progress through the ops model
(except `metadata.download_all`, which has no op handle), so an abandoned
`accounts.login` no longer blocks anything; cancel it with `ops.cancel` to free
the login slot. A handful of short handlers still run inline (instance
creation, rename, config setters, sync toggles), but they complete in
milliseconds. A few op-based commands, however, do run inline on the loop and
briefly block other commands while they work: `content.download_children`,
`logs.upload` and `import.run`. They still return an op you poll, but do not
expect other commands on a shared connection to make progress during them; use
a second connection if you need concurrency. Commands that read shared state
directly (`launcher.version/status`, `instances.list/get`, `accounts.list`,
`ops.*`, `metadata.minecraft_versions`) never wait on the loop at all.

### Proxy

- The proxy password is never returned by any command; `settings.get` only
  exposes `has_password`.
- Proxy configuration changes require a launcher restart to take effect.

### Deletion is deterministic

`instances.delete` removes the folder and deregisters the instance in the same
handler, without depending on the filesystem watcher (whose removal events are
intermittently dropped on macOS). After the `instance.removed` event, or once
the instance is gone from `instances.list`, the deletion has fully happened.
Content deletion (`content.delete`) removes files synchronously in the
handler; the listing refreshes via the watcher.

### Content discovery is cached

`content.search` and `content.versions` are served through the metadata
manager, which caches upstream Modrinth/CurseForge responses. Repeating an
identical query can therefore return cached data rather than a fresh network
result, so do not rely on these for up-to-the-second availability. The heavy
`metadata.download_all` prefetch primes the same caches; it is spawned onto its
own task (it does not park the message loop despite its returned `warning`
text) and issues 1000+ fetches with no op handle and no progress reporting.

### Skin and cape reads require online accounts

`accounts.skin_get` and `accounts.capes_get` reject offline accounts up front
(`offline accounts have no Microsoft skin/cape`) and reject unknown UUIDs,
before any network call, since offline accounts have no Microsoft credentials.
`accounts.reauth` similarly refuses offline accounts. A logged-out but online
account returns `status: "needs_login"` instead of failing; refresh it with
`accounts.reauth`.

### Miscellaneous

- Unknown `params` keys are ignored, never an error.
- The socket is `0600`, so any same-user process has full control of the
  launcher; there is no additional authentication layer.
- `instances.set` with `preferred_loader_version` interns the version string;
  distinct values accumulate for the process lifetime, but repeated calls with
  the same value do not.
- Ops are process-local: after a launcher restart, `ops.status` for an old op
  returns `unknown op (pruned or never existed)`.
- The API is available on Unix only; the listener and the CLI client are
  compiled out on Windows.
