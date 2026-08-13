//! External control API.
//!
//! Exposes the launcher's full command surface over a local NDJSON protocol:
//! one JSON object per line, requests `{"id":N,"cmd":"...","params":{...}}`,
//! responses `{"id":N,"ok":true,"data":...}` or `{"id":N,"ok":false,"error":"..."}`,
//! plus event lines `{"event":"...","data":...}` after `events.subscribe`.
//!
//! Transport: Unix domain socket at `<launcher_dir>/api.sock`, file mode 0600
//! (same-user only). Unlike `launcher.sock` (which only forwards argv and
//! focuses the main window on every connection), this listener is silent: it
//! never touches the UI unless a command asks for it.
//!
//! Long-running operations (launch, content install, Microsoft login, update
//! check, export...) return an `op` id immediately; `ops.status` polls the
//! underlying ModalAction (progress trackers, error slot, device-code URL for
//! logins) and `ops.cancel` requests cancellation.
//!
//! Documented in docs/API.md.

#![cfg(unix)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use std::path::Path;

use base64::Engine;
use bridge::import::{ImportFromOtherLauncherJob, OtherLauncher};
use bridge::install::{ContentDownload, ContentInstall, ContentInstallFile, ContentInstallPath, InstallTarget};
use bridge::instance::{ContentFolder, InstanceContentID, InstanceID, InstanceStatus};
use bridge::message::{
    EmbeddedOrRaw, ExportCurseforgeOptions, ExportFormat, ExportModrinthOptions, ExportOptions,
    MessageToBackend, MessageToFrontend, QuickPlayLaunch, UrlOrFile,
};
use bridge::modal_action::ModalAction;
use parking_lot::Mutex;
use schema::backend_config::ProxyConfig;
use schema::content::{ContentInstallReason, ContentSource};
use schema::curseforge::{CurseforgeGetModFilesRequest, CurseforgeSearchRequest};
use schema::instance::{
    InstanceJvmBinaryConfiguration, InstanceJvmFlagsConfiguration, InstanceLinuxWrapperConfiguration,
    InstanceMemoryConfiguration, InstanceSystemLibrariesConfiguration, InstanceWrapperCommandConfiguration,
};
use schema::loader::Loader;
use schema::minecraft_profile::SkinVariant;
use schema::modrinth::{ModrinthProjectVersionsRequest, ModrinthSearchIndex, ModrinthSearchRequest};
use schema::pandora_update::UpdatePrompt;
use schema::unique_bytes::UniqueBytes;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use uuid::Uuid;

use crate::BackendState;
use crate::instance::Instance;
use crate::metadata::items::{
    CurseforgeGetModFilesMetadataItem, CurseforgeSearchMetadataItem,
    ModrinthProjectVersionsMetadataItem, ModrinthSearchMetadataItem,
};

const PROTOCOL_VERSION: u32 = 1;

pub fn version_string() -> &'static str {
    option_env!("PANDORA_RELEASE_VERSION").unwrap_or("dev")
}

struct OpEntry {
    label: &'static str,
    modal: ModalAction,
    created: Instant,
}

struct ApiState {
    backend: Arc<BackendState>,
    started: Instant,
    ops: Mutex<HashMap<u64, OpEntry>>,
    next_op: AtomicU64,
    events: tokio::sync::broadcast::Sender<Arc<str>>,
    // Most recent launcher self-update offer, captured from the frontend
    // stream. InstallUpdate needs the UpdatePrompt (not client-constructible).
    cached_update: Mutex<Option<UpdatePrompt>>,
}

pub fn spawn(backend: Arc<BackendState>) {
    let (events, _) = tokio::sync::broadcast::channel::<Arc<str>>(512);

    let api = Arc::new(ApiState {
        backend: backend.clone(),
        started: Instant::now(),
        ops: Mutex::new(HashMap::new()),
        next_op: AtomicU64::new(1),
        events: events.clone(),
        cached_update: Mutex::new(None),
    });

    // Tap the backend→frontend message stream: re-emit the state-carrying
    // subset as serialized API events, and cache any update offer. Runs on the
    // sender's thread: keep cheap.
    let tap_events = events.clone();
    let tap_api = api.clone();
    backend.send.install_tap(Arc::new(move |message| {
        if let MessageToFrontend::UpdateAvailable { update } = message {
            *tap_api.cached_update.lock() = Some(update.clone());
        }
        if tap_events.receiver_count() == 0 {
            return;
        }
        if let Some(line) = event_json(message) {
            let _ = tap_events.send(line.to_string().into());
        }
    }));

    // Tap the live game console: re-emit stdout/stderr as `game.output` events
    // tagged by instance. Only fires while the game-output capture is active
    // (the "open game output" setting), see docs.
    let game_events = events;
    *backend.game_output_tap.write() = Some(Arc::new(move |id, msg| {
        if game_events.receiver_count() == 0 {
            return;
        }
        let line = json!({
            "event": "game.output",
            "data": {
                "instance": encode_instance_id(id),
                "time": msg.time,
                "level": game_level_str(msg.level),
                "lines": msg.text.iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
            },
        });
        let _ = game_events.send(line.to_string().into());
    }));

    tokio::spawn(run(api));
}

fn game_level_str(level: bridge::game_output::GameOutputLogLevel) -> &'static str {
    use bridge::game_output::GameOutputLogLevel::*;
    match level {
        Fatal => "fatal",
        Error => "error",
        Warn => "warn",
        Info => "info",
        Debug => "debug",
        Trace => "trace",
        Other => "other",
    }
}

fn event_json(message: &MessageToFrontend) -> Option<Value> {
    let (event, data) = match message {
        MessageToFrontend::InstanceAdded { id, name, configuration, .. } => (
            "instance.added",
            json!({
                "id": encode_instance_id(*id),
                "name": name.as_str(),
                "configuration": serde_json::to_value(configuration).ok(),
            }),
        ),
        MessageToFrontend::InstanceRemoved { id } => (
            "instance.removed",
            json!({ "id": encode_instance_id(*id) }),
        ),
        MessageToFrontend::InstanceModified { id, name, status, configuration, .. } => (
            "instance.modified",
            json!({
                "id": encode_instance_id(*id),
                "name": name.as_str(),
                "status": status_str(*status),
                "configuration": serde_json::to_value(configuration).ok(),
            }),
        ),
        MessageToFrontend::InstancePlaytimeUpdated { id, playtime } => (
            "instance.playtime",
            json!({
                "id": encode_instance_id(*id),
                "total_secs": playtime.total_secs,
                "session_secs": playtime.current_session_secs,
            }),
        ),
        MessageToFrontend::AccountsUpdated { accounts, selected_account } => (
            "accounts.updated",
            json!({
                "selected": selected_account.map(|u| u.to_string()),
                "accounts": accounts.iter().map(|a| json!({
                    "uuid": a.uuid.to_string(),
                    "username": a.username.as_ref(),
                    "offline": a.offline,
                })).collect::<Vec<_>>(),
            }),
        ),
        MessageToFrontend::AddNotification { notification_type, message } => (
            "notification",
            json!({
                "level": format!("{notification_type:?}").to_lowercase(),
                "message": message.as_ref(),
            }),
        ),
        MessageToFrontend::UpdateAvailable { update } => (
            "launcher.update_available",
            json!({ "version": update.new_version.as_ref() }),
        ),
        _ => return None,
    };
    Some(json!({ "event": event, "data": data }))
}

async fn run(api: Arc<ApiState>) {
    let socket_path = api.backend.directories.root_launcher_dir.join("api.sock");
    let _ = std::fs::remove_file(&socket_path);

    let listener = match UnixListener::bind(&socket_path) {
        Ok(listener) => listener,
        Err(err) => {
            log::error!("[api] unable to bind {socket_path:?}: {err}");
            return;
        }
    };

    // Same-user only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(err) = std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600)) {
            log::warn!("[api] unable to chmod api socket: {err}");
        }
    }

    log::info!("[api] control API listening on {socket_path:?}");

    let connection_limit = Arc::new(tokio::sync::Semaphore::new(64));
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let Ok(permit) = connection_limit.clone().try_acquire_owned() else {
                    log::warn!("[api] connection limit reached, rejecting client");
                    continue;
                };
                let api = api.clone();
                tokio::spawn(async move {
                    let _permit = permit;
                    if let Err(err) = handle_connection(api, stream).await {
                        log::debug!("[api] connection ended: {err}");
                    }
                });
            }
            Err(err) => {
                log::error!("[api] accept error: {err}");
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
    }
}

async fn handle_connection(api: Arc<ApiState>, stream: UnixStream) -> std::io::Result<()> {
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);

    // All writes (responses + events) funnel through one channel so lines
    // never interleave mid-line.
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<Arc<str>>(256);
    let writer_task = tokio::spawn(async move {
        let mut write_half = write_half;
        while let Some(line) = out_rx.recv().await {
            if write_half.write_all(line.as_bytes()).await.is_err() {
                break;
            }
            if write_half.write_all(b"\n").await.is_err() {
                break;
            }
        }
    });

    let mut events_task: Option<tokio::task::JoinHandle<()>> = None;

    const MAX_LINE: usize = 1 << 20; // 1 MiB per request line

    let mut raw: Vec<u8> = Vec::new();
    loop {
        raw.clear();
        // Read up to and including the next newline, but bound the buffer so a
        // client cannot grow it without limit. read_until stops at MAX_LINE
        // via the AsyncBufRead fill; a line that hits the cap without a
        // newline is rejected and the connection closed.
        let n = read_line_capped(&mut reader, &mut raw, MAX_LINE).await?;
        if n == 0 {
            break;
        }
        if raw.len() >= MAX_LINE && raw.last() != Some(&b'\n') {
            let _ = out_tx.send(json!({"id": null, "ok": false, "error": "request line exceeds 1 MiB"}).to_string().into()).await;
            break;
        }
        let line = String::from_utf8_lossy(&raw);
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let (id, cmd, params) = match serde_json::from_str::<Value>(trimmed) {
            Ok(v) => (
                v.get("id").cloned().unwrap_or(Value::Null),
                v.get("cmd").and_then(|c| c.as_str()).unwrap_or("").to_string(),
                v.get("params").cloned().unwrap_or(Value::Null),
            ),
            Err(err) => {
                let _ = out_tx.send(json!({"id": null, "ok": false, "error": format!("invalid json: {err}")}).to_string().into()).await;
                continue;
            }
        };

        if cmd == "events.subscribe" {
            if events_task.is_none() {
                let mut rx = api.events.subscribe();
                let out = out_tx.clone();
                events_task = Some(tokio::spawn(async move {
                    loop {
                        match rx.recv().await {
                            Ok(line) => {
                                if out.send(line).await.is_err() {
                                    break;
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                let _ = out.send(json!({"event": "events.lagged", "data": {"missed": n}}).to_string().into()).await;
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }));
            }
            let _ = out_tx.send(json!({"id": id, "ok": true, "data": {"subscribed": true}}).to_string().into()).await;
            continue;
        }

        let response = match dispatch(&api, &cmd, &params).await {
            Ok(data) => json!({"id": id, "ok": true, "data": data}),
            Err(error) => json!({"id": id, "ok": false, "error": error}),
        };
        if out_tx.send(response.to_string().into()).await.is_err() {
            break;
        }
    }

    if let Some(task) = events_task {
        task.abort();
    }
    drop(out_tx);
    let _ = writer_task.await;
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Read one newline-terminated line into `out`, refusing to buffer more than
/// `max` bytes. Returns the number of bytes read (0 on EOF). If the cap is hit
/// before a newline, returns with `out` at the cap and no trailing newline so
/// the caller can reject the oversized line.
async fn read_line_capped(
    reader: &mut BufReader<tokio::net::unix::OwnedReadHalf>,
    out: &mut Vec<u8>,
    max: usize,
) -> std::io::Result<usize> {
    let mut byte = [0u8; 1];
    loop {
        let n = reader.read(&mut byte).await?;
        if n == 0 {
            return Ok(out.len());
        }
        out.push(byte[0]);
        if byte[0] == b'\n' || out.len() >= max {
            return Ok(out.len());
        }
    }
}

fn status_str(status: InstanceStatus) -> &'static str {
    match status {
        InstanceStatus::NotRunning => "not_running",
        InstanceStatus::Launching => "launching",
        InstanceStatus::Running => "running",
        InstanceStatus::Stopping => "stopping",
    }
}

fn encode_instance_id(id: InstanceID) -> String {
    format!("{}:{}", id.index, id.generation)
}

fn decode_slab_id(s: &str) -> Option<(usize, usize)> {
    let (index, generation) = s.split_once(':')?;
    Some((index.parse().ok()?, generation.parse().ok()?))
}

fn p_str<'a>(params: &'a Value, key: &str) -> Result<&'a str, String> {
    params.get(key).and_then(|v| v.as_str()).ok_or_else(|| format!("missing string param '{key}'"))
}

/// Strict loader parsing. Loader's serde marks Vanilla as #[serde(other)],
/// which would silently turn any typo ("quilt", "farbic") into Vanilla; the
/// API rejects unknown names instead.
fn parse_loader(value: Option<&Value>) -> Result<Loader, String> {
    let Some(value) = value else { return Ok(Loader::Vanilla) };
    let name = value.as_str().ok_or("loader must be a string")?;
    match name.to_ascii_lowercase().as_str() {
        "vanilla" => Ok(Loader::Vanilla),
        "fabric" => Ok(Loader::Fabric),
        "forge" => Ok(Loader::Forge),
        "neoforge" => Ok(Loader::NeoForge),
        other => Err(format!("unknown loader '{other}' (vanilla|fabric|forge|neoforge)")),
    }
}

/// Interns loader-version strings so repeated instances.set calls with the
/// same value do not grow memory (the bridge message wants &'static str).
fn intern_loader_version(s: &str) -> &'static str {
    static INTERNED: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    let mut interned = INTERNED.lock();
    if let Some(existing) = interned.iter().find(|e| **e == s) {
        return existing;
    }
    let leaked: &'static str = Box::leak(s.to_string().into_boxed_str());
    interned.push(leaked);
    leaked
}

fn p_bool(params: &Value, key: &str) -> Result<bool, String> {
    params.get(key).and_then(|v| v.as_bool()).ok_or_else(|| format!("missing bool param '{key}'"))
}

fn p_usize(params: &Value, key: &str) -> Result<usize, String> {
    params.get(key).and_then(|v| v.as_u64()).map(|v| v as usize).ok_or_else(|| format!("missing unsigned integer param '{key}'"))
}

fn p_i64(params: &Value, key: &str) -> Result<i64, String> {
    params.get(key).and_then(|v| v.as_i64()).ok_or_else(|| format!("missing integer param '{key}'"))
}

fn parse_uuid(params: &Value, key: &str) -> Result<Uuid, String> {
    Uuid::parse_str(p_str(params, key)?).map_err(|e| format!("invalid uuid '{key}': {e}"))
}

fn parse_other_launcher(name: &str) -> Result<OtherLauncher, String> {
    match name {
        "Prism" => Ok(OtherLauncher::Prism),
        "CurseForge" => Ok(OtherLauncher::CurseForge),
        "Modrinth" => Ok(OtherLauncher::Modrinth),
        "MultiMC" => Ok(OtherLauncher::MultiMC),
        "ATLauncher" => Ok(OtherLauncher::ATLauncher),
        other => Err(format!("unknown launcher '{other}' (Prism|CurseForge|Modrinth|MultiMC|ATLauncher)")),
    }
}

fn decode_b64_png(s: &str) -> Result<UniqueBytes, String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(s).map_err(|e| format!("invalid base64: {e}"))?;
    if image::guess_format(&bytes).ok() != Some(image::ImageFormat::Png) {
        return Err("decoded bytes are not a PNG".into());
    }
    Ok(UniqueBytes::from(bytes))
}

fn content_id_from(params: &Value, key: &str) -> Result<InstanceContentID, String> {
    let s = p_str(params, key)?;
    let (index, generation) = decode_slab_id(s).ok_or_else(|| format!("invalid content id '{s}'"))?;
    Ok(InstanceContentID { index, generation })
}

/// Parse an icon value: null clears it, {"embedded": "name"} selects a builtin
/// fallback icon, {"png_base64": "..."} or {"path": "..."} supplies raw PNG.
fn parse_icon(v: &Value) -> Result<Option<EmbeddedOrRaw>, String> {
    if v.is_null() {
        return Ok(None);
    }
    if let Some(name) = v.get("embedded").and_then(|e| e.as_str()) {
        return Ok(Some(EmbeddedOrRaw::Embedded(name.into())));
    }
    if let Some(b64) = v.get("png_base64").and_then(|e| e.as_str()) {
        return Ok(Some(EmbeddedOrRaw::Raw(decode_b64_png(b64)?)));
    }
    if let Some(path) = v.get("path").and_then(|e| e.as_str()) {
        // Reject non-regular files and cap the size before reading, so a
        // client cannot point this at /dev/zero or a huge file.
        let meta = std::fs::metadata(path).map_err(|e| format!("cannot stat icon file: {e}"))?;
        if !meta.is_file() {
            return Err("icon path is not a regular file".into());
        }
        if meta.len() > 8 * 1024 * 1024 {
            return Err("icon file exceeds 8 MiB".into());
        }
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read icon file: {e}"))?;
        if image::guess_format(&bytes).ok() != Some(image::ImageFormat::Png) {
            return Err("icon file is not a PNG".into());
        }
        return Ok(Some(EmbeddedOrRaw::Raw(UniqueBytes::from(bytes))));
    }
    Err("icon must be null, {\"embedded\":name}, {\"png_base64\":...} or {\"path\":...}".into())
}

fn instance_json(instance: &mut Instance) -> Value {
    let configuration = instance.configuration.get().clone();
    let stats = instance.stats.get().clone();
    json!({
        "id": encode_instance_id(instance.id),
        "name": instance.name.as_str(),
        "status": status_str(instance.status()),
        "root_path": instance.root_path.to_string_lossy(),
        "dot_minecraft_path": instance.dot_minecraft_path.to_string_lossy(),
        "configuration": serde_json::to_value(&configuration).unwrap_or(Value::Null),
        "playtime": {
            "total_secs": stats.total_playtime_secs,
            "session_count": stats.session_count,
            "last_played_unix_ms": stats.last_played_unix_ms,
        },
    })
}

impl ApiState {
    fn resolve_instance_id(&self, params: &Value) -> Result<InstanceID, String> {
        let mut guard = self.backend.instance_state.write();
        if let Some(id) = params.get("id").and_then(|v| v.as_str()) {
            let (index, generation) = decode_slab_id(id).ok_or("invalid instance id, expected 'index:generation'")?;
            let id = InstanceID { index, generation };
            if guard.instances.get_mut(id).is_some() {
                return Ok(id);
            }
            return Err(format!("no instance with id {index}:{generation}"));
        }
        if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
            for instance in guard.instances.iter_mut() {
                if instance.name.as_str().eq_ignore_ascii_case(name) {
                    return Ok(instance.id);
                }
            }
            return Err(format!("no instance named '{name}'"));
        }
        Err("provide 'id' (\"index:generation\") or 'name'".into())
    }

    /// Reject unknown or offline accounts before an online-only operation
    /// (skin/cape fetch): offline accounts have no Microsoft credentials, and
    /// the underlying handler would otherwise make a doomed, slow network call.
    fn require_online_account(&self, account: Uuid) -> Result<(), String> {
        let mut guard = self.backend.account_info.write();
        match guard.get().accounts.get(&account) {
            None => Err(format!("no account with uuid {account}")),
            Some(a) if a.offline => Err("offline accounts have no Microsoft skin/cape".into()),
            _ => Ok(()),
        }
    }

    fn register_op(&self, label: &'static str) -> (u64, ModalAction) {
        let modal = ModalAction::default();
        let op = self.next_op.fetch_add(1, Ordering::Relaxed);
        let mut ops = self.ops.lock();
        // Prune finished ops older than an hour so the map cannot grow forever.
        ops.retain(|_, entry| {
            match entry.modal.get_finished_at() {
                Some(at) => at.elapsed() < Duration::from_secs(3600),
                None => true,
            }
        });
        ops.insert(op, OpEntry { label, modal: modal.clone(), created: Instant::now() });
        (op, modal)
    }

    fn op_json(&self, op: u64, entry: &OpEntry) -> Value {
        let trackers: Vec<Value> = entry.modal.trackers.trackers.read().iter().map(|tracker| {
            let (count, total) = tracker.get();
            json!({
                "title": tracker.get_title().as_ref(),
                "count": count,
                "total": total,
                "done": tracker.get_finished_at().is_some(),
            })
        }).collect();
        let visit_url = entry.modal.visit_url.read().as_ref().map(|v| json!({
            "message": v.message.as_ref(),
            "url": v.url.as_ref(),
        }));
        json!({
            "op": op,
            "label": entry.label,
            "finished": entry.modal.get_finished_at().is_some(),
            "error": entry.modal.error.read().as_deref(),
            "visit_url": visit_url,
            "cancel_requested": entry.modal.has_requested_cancel(),
            "age_secs": entry.created.elapsed().as_secs(),
            "progress": trackers,
        })
    }
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

async fn dispatch(api: &Arc<ApiState>, cmd: &str, params: &Value) -> Result<Value, String> {
    let backend = &api.backend;
    match cmd {
        // ── Launcher ────────────────────────────────────────────────────────
        "launcher.version" => Ok(json!({
            "version": version_string(),
            "protocol": PROTOCOL_VERSION,
        })),
        "launcher.status" => {
            let mut counts = HashMap::new();
            let mut guard = backend.instance_state.write();
            let total = guard.instances.iter_mut().map(|i| {
                *counts.entry(status_str(i.status())).or_insert(0u32) += 1;
            }).count();
            Ok(json!({
                "version": version_string(),
                "uptime_secs": api.started.elapsed().as_secs(),
                "instances_total": total,
                "instances_by_status": counts,
                "launcher_dir": backend.directories.root_launcher_dir.to_string_lossy(),
            }))
        }
        "launcher.focus" => {
            backend.send.send(MessageToFrontend::OpenOrFocusMainWindow);
            Ok(json!({"focused": true}))
        }
        "launcher.quit" => {
            backend.self_handle.send(MessageToBackend::Quit);
            Ok(json!({"quitting": true}))
        }

        // ── Instances ───────────────────────────────────────────────────────
        "instances.list" => {
            let mut guard = backend.instance_state.write();
            let list: Vec<Value> = guard.instances.iter_mut().map(instance_json).collect();
            Ok(json!(list))
        }
        "instances.get" => {
            let id = api.resolve_instance_id(params)?;
            let mut guard = backend.instance_state.write();
            let instance = guard.instances.get_mut(id).ok_or("instance disappeared")?;
            Ok(instance_json(instance))
        }
        "instances.create" => {
            let name = p_str(params, "name")?;
            let version = p_str(params, "version")?;
            let loader = parse_loader(params.get("loader"))?;
            backend.self_handle.send(MessageToBackend::CreateInstance {
                name: name.into(),
                version: version.into(),
                loader,
                icon: None,
            });
            Ok(json!({"requested": true, "name": name}))
        }
        "instances.delete" => {
            let id = api.resolve_instance_id(params)?;
            backend.self_handle.send(MessageToBackend::DeleteInstance { id });
            Ok(json!({"requested": true}))
        }
        "instances.rename" => {
            let id = api.resolve_instance_id(params)?;
            let name = p_str(params, "new_name")?;
            backend.self_handle.send(MessageToBackend::RenameInstance { id, name: name.into() });
            Ok(json!({"requested": true}))
        }
        "instances.start" => {
            let id = api.resolve_instance_id(params)?;
            let quick_play = if let Some(world) = params.get("quick_play_singleplayer").and_then(|v| v.as_str()) {
                Some(QuickPlayLaunch::Singleplayer(world.into()))
            } else if let Some(server) = params.get("quick_play_multiplayer").and_then(|v| v.as_str()) {
                Some(QuickPlayLaunch::Multiplayer(server.into()))
            } else {
                None
            };
            let (op, modal) = api.register_op("instances.start");
            backend.self_handle.send(MessageToBackend::StartInstance { id, quick_play, modal_action: modal });
            Ok(json!({"op": op}))
        }
        "instances.kill" => {
            let id = api.resolve_instance_id(params)?;
            backend.self_handle.send(MessageToBackend::KillInstance { id });
            Ok(json!({"requested": true}))
        }
        "instances.set" => {
            let id = api.resolve_instance_id(params)?;

            // Two-phase: parse and validate every provided field first, and
            // only then send the messages, so a bad field never leaves the
            // instance half-updated.
            let mut messages: Vec<MessageToBackend> = Vec::new();
            let mut applied = Vec::new();

            if let Some(v) = params.get("minecraft_version").and_then(|v| v.as_str()) {
                messages.push(MessageToBackend::SetInstanceMinecraftVersion { id, version: v.into() });
                applied.push("minecraft_version");
            }
            if params.get("loader").is_some() {
                let loader = parse_loader(params.get("loader"))?;
                messages.push(MessageToBackend::SetInstanceLoader { id, loader });
                applied.push("loader");
            }
            if let Some(v) = params.get("preferred_account") {
                let account = if v.is_null() {
                    None
                } else {
                    Some(Uuid::parse_str(v.as_str().ok_or("preferred_account must be a uuid string or null")?)
                        .map_err(|e| format!("invalid uuid: {e}"))?)
                };
                messages.push(MessageToBackend::SetInstancePreferredAccount { id, account });
                applied.push("preferred_account");
            }
            if let Some(v) = params.get("disable_file_syncing").and_then(|v| v.as_bool()) {
                messages.push(MessageToBackend::SetInstanceDisableFileSyncing { id, disable_file_syncing: v });
                applied.push("disable_file_syncing");
            }
            if let Some(v) = params.get("sandbox").and_then(|v| v.as_bool()) {
                messages.push(MessageToBackend::SetInstanceSandboxing { id, sandbox: v });
                applied.push("sandbox");
            }
            if let Some(v) = params.get("memory") {
                let memory: InstanceMemoryConfiguration = serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid memory config: {e}"))?;
                if memory.enabled && (memory.min == 0 || memory.min > memory.max) {
                    return Err(format!("invalid memory bounds: min={} max={}", memory.min, memory.max));
                }
                messages.push(MessageToBackend::SetInstanceMemory { id, memory });
                applied.push("memory");
            }
            if let Some(v) = params.get("jvm_flags") {
                let jvm_flags: InstanceJvmFlagsConfiguration = serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid jvm_flags config: {e}"))?;
                messages.push(MessageToBackend::SetInstanceJvmFlags { id, jvm_flags });
                applied.push("jvm_flags");
            }
            if let Some(v) = params.get("jvm_binary") {
                let jvm_binary: InstanceJvmBinaryConfiguration = serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid jvm_binary config: {e}"))?;
                messages.push(MessageToBackend::SetInstanceJvmBinary { id, jvm_binary });
                applied.push("jvm_binary");
            }
            if let Some(v) = params.get("wrapper_command") {
                let wrapper_command: InstanceWrapperCommandConfiguration = serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid wrapper_command config: {e}"))?;
                messages.push(MessageToBackend::SetInstanceWrapperCommand { id, wrapper_command });
                applied.push("wrapper_command");
            }
            if let Some(v) = params.get("preferred_loader_version") {
                let loader_version = if v.is_null() {
                    None
                } else {
                    let s = v.as_str().ok_or("preferred_loader_version must be a string or null")?;
                    Some(intern_loader_version(s))
                };
                messages.push(MessageToBackend::SetInstancePreferredLoaderVersion { id, loader_version });
                applied.push("preferred_loader_version");
            }
            if let Some(v) = params.get("linux_wrapper") {
                let linux_wrapper: InstanceLinuxWrapperConfiguration = serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid linux_wrapper config: {e}"))?;
                messages.push(MessageToBackend::SetInstanceLinuxWrapper { id, linux_wrapper });
                applied.push("linux_wrapper");
            }
            if let Some(v) = params.get("system_libraries") {
                let system_libraries: InstanceSystemLibrariesConfiguration = serde_json::from_value(v.clone())
                    .map_err(|e| format!("invalid system_libraries config: {e}"))?;
                messages.push(MessageToBackend::SetInstanceSystemLibraries { id, system_libraries });
                applied.push("system_libraries");
            }
            if let Some(v) = params.get("icon") {
                let icon = parse_icon(v)?;
                messages.push(MessageToBackend::SetInstanceIcon { id, icon });
                applied.push("icon");
            }
            if applied.is_empty() {
                return Err("no settable field provided (minecraft_version, loader, preferred_account, disable_file_syncing, sandbox, memory, jvm_flags, jvm_binary, wrapper_command, preferred_loader_version, linux_wrapper, system_libraries, icon)".into());
            }
            for message in messages {
                backend.self_handle.send(message);
            }
            Ok(json!({"applied": applied}))
        }
        "instances.update_check" => {
            let id = api.resolve_instance_id(params)?;
            let (op, modal) = api.register_op("instances.update_check");
            backend.self_handle.send(MessageToBackend::UpdateCheck { instance: id, modal_action: modal });
            Ok(json!({"op": op}))
        }
        "instances.logs" => {
            let id = api.resolve_instance_id(params)?;
            let (tx, rx) = tokio::sync::oneshot::channel();
            backend.self_handle.send(MessageToBackend::GetLogFiles { instance: id, channel: tx });
            // The handler drops the channel without replying when the logs
            // directory does not exist yet: treat that as an empty list.
            let logs = match tokio::time::timeout(Duration::from_secs(10), rx).await {
                Ok(Ok(logs)) => logs,
                Ok(Err(_)) => bridge::message::LogFiles::default(),
                Err(_) => return Err("timed out waiting for log list".into()),
            };
            Ok(json!({
                "total_gzipped_size": logs.total_gzipped_size,
                "paths": logs.paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
            }))
        }
        "logs.read" => {
            let path = PathBuf::from(p_str(params, "path")?);
            let max_lines = params.get("max_lines").and_then(|v| v.as_u64()).unwrap_or(500) as usize;
            let max_lines = max_lines.min(100_000);
            let (tx, mut rx) = tokio::sync::mpsc::channel(64);
            backend.self_handle.send(MessageToBackend::ReadLog { path: path.into(), send: tx });
            // The backend tails live logs forever (250ms poll on EOF), so a
            // hard deadline alone would make every read of a short file take
            // the full window. Return as soon as the stream goes idle.
            let mut lines = Vec::new();
            let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
            loop {
                let idle = Duration::from_millis(800);
                let step = tokio::time::timeout(idle, tokio::time::timeout_at(deadline, rx.recv())).await;
                match step {
                    Ok(Ok(Some(chunk))) => {
                        lines.push(chunk.to_string());
                        if lines.len() >= max_lines {
                            break;
                        }
                    }
                    Ok(Ok(None)) => break,
                    // idle or global deadline: return what we have
                    Ok(Err(_)) | Err(_) => break,
                }
            }
            Ok(json!({"lines": lines, "truncated": lines.len() >= max_lines}))
        }
        "instances.worlds" | "instances.servers" | "instances.content" => {
            let id = api.resolve_instance_id(params)?;
            let folder = if cmd == "instances.content" {
                Some(match p_str(params, "folder")? {
                    "mods" => ContentFolder::Mods,
                    "resourcepacks" => ContentFolder::ResourcePacks,
                    "shaders" | "shaderpacks" => ContentFolder::Shaders,
                    other => return Err(format!("unknown content folder '{other}' (mods|resourcepacks|shaders)")),
                })
            } else {
                None
            };

            // Trigger a (re)load, then poll the cache. Loads are asynchronous
            // and watcher-driven; a result is normally available well within
            // the timeout unless the folder is enormous.
            match cmd {
                "instances.worlds" => backend.self_handle.send(MessageToBackend::RequestLoadWorlds { id }),
                "instances.servers" => backend.self_handle.send(MessageToBackend::RequestLoadServers { id }),
                _ => backend.self_handle.send(MessageToBackend::RequestLoadContentFolder { id, content_folder: folder.unwrap() }),
            }

            let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
            loop {
                {
                    let mut guard = backend.instance_state.write();
                    let instance = guard.instances.get_mut(id).ok_or("instance disappeared")?;
                    let data = match cmd {
                        "instances.worlds" => instance.cached_worlds().map(|worlds| json!(worlds.iter().map(|w| json!({
                            "title": w.title.as_ref(),
                            "subtitle": w.subtitle.as_ref(),
                            "level_path": w.level_path.to_string_lossy(),
                            "last_played": w.last_played,
                        })).collect::<Vec<_>>())),
                        "instances.servers" => instance.cached_servers().map(|servers| json!(servers.iter().map(|s| json!({
                            "name": s.name.as_ref(),
                            "ip": s.ip.as_ref(),
                        })).collect::<Vec<_>>())),
                        _ => instance.cached_content(folder.unwrap()).map(|content| json!(content.iter().map(|c| json!({
                            "content_id": format!("{}:{}", c.id.index, c.id.generation),
                            "filename": c.filename.as_ref(),
                            "name": c.content_summary.name.as_deref(),
                            "version": c.content_summary.version_str.as_ref(),
                            "authors": c.content_summary.authors.as_ref(),
                            "enabled": c.enabled,
                            "can_toggle": c.can_toggle,
                            "path": c.path.to_string_lossy(),
                        })).collect::<Vec<_>>())),
                    };
                    if let Some(data) = data {
                        return Ok(data);
                    }
                }
                if tokio::time::Instant::now() >= deadline {
                    return Err("data not loaded yet; retry shortly".into());
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        }
        "content.set_enabled" => {
            let id = api.resolve_instance_id(params)?;
            let enabled = p_bool(params, "enabled")?;
            let content_ids = parse_content_ids(params)?;
            backend.self_handle.send(MessageToBackend::SetContentEnabled { id, content_ids, enabled });
            Ok(json!({"requested": true}))
        }
        "content.delete" => {
            let id = api.resolve_instance_id(params)?;
            let content_ids = parse_content_ids(params)?;
            backend.self_handle.send(MessageToBackend::DeleteContent { id, content_ids });
            Ok(json!({"requested": true}))
        }
        "content.update" => {
            let id = api.resolve_instance_id(params)?;
            let content_ids = parse_content_ids(params)?;
            let [content_id] = content_ids[..] else {
                return Err("content.update takes exactly one content_id".into());
            };
            let (op, modal) = api.register_op("content.update");
            backend.self_handle.send(MessageToBackend::UpdateContent { instance: id, content_id, modal_action: modal });
            Ok(json!({"op": op}))
        }
        "content.install" => {
            let target = if let Some(name) = params.get("new_instance_name").and_then(|v| v.as_str()) {
                InstallTarget::NewInstance { name: Some(name.into()) }
            } else if params.get("id").is_some() || params.get("name").is_some() {
                InstallTarget::Instance(api.resolve_instance_id(params)?)
            } else {
                return Err("provide 'id'/'name' (existing instance) or 'new_instance_name'".into());
            };

            // When installing into an existing instance, default loader and
            // minecraft_version to the instance's own configuration so
            // Modrinth/CurseForge version resolution matches what will run.
            let (instance_loader, instance_version) = if let InstallTarget::Instance(id) = target {
                let mut guard = backend.instance_state.write();
                let instance = guard.instances.get_mut(id).ok_or("instance disappeared")?;
                let configuration = instance.configuration.get();
                (Some(configuration.loader), Some(configuration.minecraft_version))
            } else {
                (None, None)
            };
            let loader = match params.get("loader") {
                Some(v) => parse_loader(Some(v))?,
                None => instance_loader.ok_or("missing 'loader' (vanilla|fabric|forge|neoforge)")?,
            };
            let minecraft_version: ustr::Ustr = match params.get("minecraft_version").and_then(|v| v.as_str()) {
                Some(v) => v.into(),
                None => instance_version.ok_or("missing 'minecraft_version'")?,
            };

            let files_param = params.get("files").and_then(|v| v.as_array()).ok_or("missing 'files' array")?;
            let mut files = Vec::new();
            for file in files_param {
                let download = if let Some(m) = file.get("modrinth") {
                    ContentDownload::Modrinth {
                        project_id: p_str(m, "project_id")?.into(),
                        version_id: m.get("version_id").and_then(|v| v.as_str()).map(Into::into),
                        install_dependencies: m.get("install_dependencies").and_then(|v| v.as_bool()).unwrap_or(true),
                    }
                } else if let Some(c) = file.get("curseforge") {
                    ContentDownload::Curseforge {
                        project_id: c.get("project_id").and_then(|v| v.as_u64()).ok_or("curseforge.project_id must be a number")? as u32,
                        install_dependencies: c.get("install_dependencies").and_then(|v| v.as_bool()).unwrap_or(true),
                    }
                } else if let Some(f) = file.get("file") {
                    let path = PathBuf::from(p_str(f, "path")?);
                    if path.file_name().is_none() {
                        return Err(format!("file path {path:?} has no file name"));
                    }
                    if !path.is_file() {
                        return Err(format!("file path {path:?} does not exist or is not a regular file"));
                    }
                    ContentDownload::File { path }
                } else {
                    return Err("each file needs 'modrinth', 'curseforge' or 'file'".into());
                };
                files.push(ContentInstallFile {
                    replace_old: None,
                    path: ContentInstallPath::Automatic,
                    download,
                    content_source: ContentSource::Manual,
                    reason: ContentInstallReason::Standalone,
                });
            }

            let (op, modal) = api.register_op("content.install");
            backend.self_handle.send(MessageToBackend::InstallContent {
                content: ContentInstall {
                    target,
                    loader,
                    minecraft_version,
                    files: files.into(),
                },
                modal_action: modal,
            });
            Ok(json!({"op": op}))
        }

        // ── Accounts ────────────────────────────────────────────────────────
        "accounts.list" => {
            let mut guard = backend.account_info.write();
            let info = guard.get();
            Ok(json!({
                "selected": info.selected_account.map(|u| u.to_string()),
                "accounts": info.accounts.iter().map(|(uuid, account)| json!({
                    "uuid": uuid.to_string(),
                    "username": account.username.as_ref(),
                    "offline": account.offline,
                })).collect::<Vec<_>>(),
            }))
        }
        "accounts.select" => {
            let uuid = Uuid::parse_str(p_str(params, "uuid")?).map_err(|e| format!("invalid uuid: {e}"))?;
            backend.self_handle.send(MessageToBackend::SelectAccount { uuid });
            Ok(json!({"requested": true}))
        }
        "accounts.delete" => {
            let uuid = Uuid::parse_str(p_str(params, "uuid")?).map_err(|e| format!("invalid uuid: {e}"))?;
            backend.self_handle.send(MessageToBackend::DeleteAccount { uuid });
            Ok(json!({"requested": true}))
        }
        "accounts.add_offline" => {
            let name = p_str(params, "username")?;
            let uuid = match params.get("uuid").and_then(|v| v.as_str()) {
                Some(u) => Uuid::parse_str(u).map_err(|e| format!("invalid uuid: {e}"))?,
                None => offline_uuid(name),
            };
            // The backend handler overwrites an existing entry with the same
            // uuid (which would silently turn a Microsoft account into an
            // offline one and auto-select it); refuse instead.
            if backend.account_info.write().get().accounts.contains_key(&uuid) {
                return Err(format!("an account with uuid {uuid} already exists"));
            }
            backend.self_handle.send(MessageToBackend::AddOfflineAccount { name: name.into(), uuid });
            Ok(json!({"requested": true, "uuid": uuid.to_string()}))
        }
        "accounts.login" => {
            // Microsoft device-code flow. The verification URL appears in
            // ops.status as visit_url once the flow reaches that stage.
            let (op, modal) = api.register_op("accounts.login");
            backend.self_handle.send(MessageToBackend::AddNewAccount { modal_action: modal });
            Ok(json!({"op": op, "hint": "poll ops.status for visit_url, open it in a browser, enter the code"}))
        }

        // ── Sync / settings / metadata ──────────────────────────────────────
        "sync.state" => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            backend.self_handle.send(MessageToBackend::GetSyncState { channel: tx });
            let state = tokio::time::timeout(Duration::from_secs(10), rx).await
                .map_err(|_| "timed out")?
                .map_err(|_| "backend dropped the request")?;
            Ok(json!({
                "sync_folder": state.sync_folder.to_string_lossy(),
                "total_count": state.total_count,
                "targets": state.targets.iter().map(|(name, t)| json!({
                    "target": name.as_ref(),
                    "enabled": t.enabled,
                    "is_file": t.is_file,
                    "sync_count": t.sync_count,
                    "cannot_sync_count": t.cannot_sync_count,
                })).collect::<Vec<_>>(),
            }))
        }
        "sync.set" => {
            let target = p_str(params, "target")?;
            let is_file = p_bool(params, "is_file")?;
            let value = p_bool(params, "value")?;
            backend.self_handle.send(MessageToBackend::SetSyncing { target: target.into(), is_file, value });
            Ok(json!({"requested": true}))
        }
        "settings.get" => {
            let (tx, rx) = tokio::sync::oneshot::channel();
            backend.self_handle.send(MessageToBackend::GetBackendConfiguration { channel: tx });
            let config = tokio::time::timeout(Duration::from_secs(10), rx).await
                .map_err(|_| "timed out")?
                .map_err(|_| "backend dropped the request")?;
            // Deliberately never expose the proxy password over the API.
            Ok(json!({
                "open_game_output_when_launching": !config.config.dont_open_game_output_when_launching,
                "proxy": {
                    "enabled": config.config.proxy.enabled,
                    "protocol": config.config.proxy.protocol.name(),
                    "host": config.config.proxy.host,
                    "port": config.config.proxy.port,
                    "auth_enabled": config.config.proxy.auth_enabled,
                    "username": config.config.proxy.username,
                    "has_password": config.proxy_password.is_some(),
                },
            }))
        }
        "settings.set_open_game_output" => {
            let value = p_bool(params, "value")?;
            backend.self_handle.send(MessageToBackend::SetOpenGameOutputAfterLaunching { value });
            Ok(json!({"requested": true}))
        }
        "settings.set_proxy" => {
            let mut proxy = params.get("proxy").cloned().ok_or("missing 'proxy' object")?;
            // settings.get reports the protocol as "HTTP"/"HTTPS"/"SOCKS5"
            // (display names), while ProxyConfig's serde expects the enum
            // variant casing; normalize so a get -> set round-trip works and
            // typos are rejected instead of silently falling back to HTTP.
            if let Some(protocol) = proxy.get("protocol").and_then(|v| v.as_str()) {
                let normalized = match protocol.to_ascii_uppercase().as_str() {
                    "HTTP" => "Http",
                    "HTTPS" => "Https",
                    "SOCKS5" => "Socks5",
                    other => return Err(format!("unknown proxy protocol '{other}' (HTTP|HTTPS|SOCKS5)")),
                };
                proxy["protocol"] = json!(normalized);
            }
            let config: ProxyConfig = serde_json::from_value(proxy)
                .map_err(|e| format!("invalid proxy config: {e}"))?;
            let password = params.get("password").and_then(|v| v.as_str()).map(String::from);
            backend.self_handle.send(MessageToBackend::SetProxyConfiguration { config, password });
            Ok(json!({"requested": true, "note": "restart the launcher for proxy changes to take effect"}))
        }
        "metadata.minecraft_versions" => {
            // Served from the metadata manager's on-disk cache; triggers a
            // refresh for next time if the cache is missing.
            let cache = backend.directories.metadata_dir.join("version_manifest.json");
            match std::fs::read(&cache) {
                Ok(bytes) => {
                    let manifest: Value = serde_json::from_slice(&bytes).map_err(|e| format!("corrupt cache: {e}"))?;
                    Ok(manifest)
                }
                Err(_) => {
                    backend.self_handle.send(MessageToBackend::RequestMetadata {
                        request: bridge::meta::MetadataRequest::MinecraftVersionManifest,
                        force_reload: true,
                    });
                    Err("version manifest not cached yet; fetch triggered, retry shortly".into())
                }
            }
        }

        // ── Operations ──────────────────────────────────────────────────────
        "ops.list" => {
            let ops = api.ops.lock();
            let mut list: Vec<Value> = ops.iter().map(|(id, entry)| api.op_json(*id, entry)).collect();
            list.sort_by_key(|v| v.get("op").and_then(|o| o.as_u64()));
            Ok(json!(list))
        }
        "ops.status" => {
            let op = params.get("op").and_then(|v| v.as_u64()).ok_or("missing numeric param 'op'")?;
            let ops = api.ops.lock();
            let entry = ops.get(&op).ok_or("unknown op (pruned or never existed)")?;
            Ok(api.op_json(op, entry))
        }
        "ops.cancel" => {
            let op = params.get("op").and_then(|v| v.as_u64()).ok_or("missing numeric param 'op'")?;
            let ops = api.ops.lock();
            let entry = ops.get(&op).ok_or("unknown op")?;
            entry.modal.request_cancel();
            Ok(json!({"cancel_requested": true}))
        }

        // ── Content discovery (metadata search) ─────────────────────────────
        "content.search" => {
            let source = p_str(params, "source")?;
            let query = params.get("query").and_then(|v| v.as_str()).map(Into::into);
            let offset = params.get("offset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let limit = (params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize).min(100);
            match source {
                "modrinth" => {
                    let request = ModrinthSearchRequest {
                        query,
                        facets: params.get("facets").and_then(|v| v.as_str()).map(Into::into),
                        index: ModrinthSearchIndex::default(),
                        offset,
                        limit,
                    };
                    let result = backend.meta.fetch(&ModrinthSearchMetadataItem(&request)).await
                        .map_err(|e| format!("modrinth search failed: {e:?}"))?;
                    Ok(json!({
                        "source": "modrinth",
                        "total": result.total_hits,
                        "offset": result.offset,
                        "hits": result.hits.iter().map(|h| json!({
                            "project_id": h.project_id.as_ref(),
                            "title": h.title.as_deref(),
                            "description": h.description.as_deref(),
                            "author": h.author.as_ref(),
                            "downloads": h.downloads,
                            "icon_url": h.icon_url.as_deref(),
                            "project_type": format!("{:?}", h.project_type).to_lowercase(),
                        })).collect::<Vec<_>>(),
                    }))
                }
                "curseforge" => {
                    let class_id = params.get("class_id").and_then(|v| v.as_u64()).unwrap_or(6) as u32; // 6 = Mod
                    let request = CurseforgeSearchRequest {
                        class_id,
                        category_ids: None,
                        game_version: params.get("game_version").and_then(|v| v.as_str()).map(Into::into),
                        search_filter: params.get("query").and_then(|v| v.as_str()).map(Into::into),
                        mod_loader_types: None,
                        sort_field: 2, // Popularity
                        index: offset as u32,
                        // CurseForge rejects pageSize > 50 (Modrinth allows
                        // 100); clamp per-source so limit 51-100 does not 400.
                        page_size: (limit as u32).clamp(1, 50),
                    };
                    let result = backend.meta.fetch(&CurseforgeSearchMetadataItem(&request)).await
                        .map_err(|e| format!("curseforge search failed: {e:?}"))?;
                    Ok(json!({
                        "source": "curseforge",
                        "hits": result.data.iter().map(|h| json!({
                            "project_id": h.id,
                            "name": h.name.as_ref(),
                            "slug": h.slug.as_ref(),
                            "summary": h.summary.as_ref(),
                            "downloads": h.download_count,
                            "class_id": h.class_id,
                        })).collect::<Vec<_>>(),
                    }))
                }
                other => Err(format!("unknown source '{other}' (modrinth|curseforge)")),
            }
        }
        "content.versions" => {
            let source = p_str(params, "source")?;
            match source {
                "modrinth" => {
                    let request = ModrinthProjectVersionsRequest {
                        project_id: p_str(params, "project_id")?.into(),
                        game_versions: None,
                        loaders: None,
                    };
                    let result = backend.meta.fetch(&ModrinthProjectVersionsMetadataItem(&request)).await
                        .map_err(|e| format!("modrinth versions failed: {e:?}"))?;
                    Ok(json!({
                        "source": "modrinth",
                        "versions": result.0.iter().map(|v| json!({
                            "version_id": v.id.as_ref(),
                            "project_id": v.project_id.as_ref(),
                            "name": v.name.as_deref(),
                            "version_number": v.version_number.as_deref(),
                            "game_versions": v.game_versions.as_ref().map(|g| g.iter().map(|s| s.as_str()).collect::<Vec<_>>()),
                        })).collect::<Vec<_>>(),
                    }))
                }
                "curseforge" => {
                    let request = CurseforgeGetModFilesRequest {
                        mod_id: params.get("project_id").and_then(|v| v.as_u64()).ok_or("curseforge project_id must be a number")? as u32,
                        game_version: params.get("game_version").and_then(|v| v.as_str()).map(Into::into),
                        mod_loader_type: None,
                        page_size: Some(50),
                    };
                    let result = backend.meta.fetch(&CurseforgeGetModFilesMetadataItem(&request)).await
                        .map_err(|e| format!("curseforge files failed: {e:?}"))?;
                    Ok(json!({
                        "source": "curseforge",
                        "files": result.data.iter().map(|f| json!({
                            "file_id": f.id,
                            "mod_id": f.mod_id,
                            "file_name": f.file_name.as_ref(),
                            "file_length": f.file_length,
                            "download_url": f.download_url.as_deref(),
                        })).collect::<Vec<_>>(),
                    }))
                }
                other => Err(format!("unknown source '{other}' (modrinth|curseforge)")),
            }
        }

        // ── Instance export / import ────────────────────────────────────────
        "instances.export" => {
            let id = api.resolve_instance_id(params)?;
            let format = match p_str(params, "format")? {
                "zip" => ExportFormat::Zip,
                "modrinth" => ExportFormat::Modrinth,
                "curseforge" => ExportFormat::Curseforge,
                other => return Err(format!("unknown export format '{other}' (zip|modrinth|curseforge)")),
            };
            let output = std::path::PathBuf::from(p_str(params, "output")?);
            let inc = |k: &str| params.get(k).and_then(|v| v.as_bool()).unwrap_or(false);
            let pack_name: Arc<str> = params.get("name").and_then(|v| v.as_str()).unwrap_or("export").into();
            let pack_version: Arc<str> = params.get("version").and_then(|v| v.as_str()).unwrap_or("1.0.0").into();
            let options = ExportOptions {
                include_saves: inc("include_saves"),
                include_mods: params.get("include_mods").and_then(|v| v.as_bool()).unwrap_or(true),
                include_resourcepacks: params.get("include_resourcepacks").and_then(|v| v.as_bool()).unwrap_or(true),
                include_shaders: params.get("include_shaders").and_then(|v| v.as_bool()).unwrap_or(true),
                include_configs: params.get("include_configs").and_then(|v| v.as_bool()).unwrap_or(true),
                include_screenshots: inc("include_screenshots"),
                include_backups: inc("include_backups"),
                include_logs: inc("include_logs"),
                include_cache: inc("include_cache"),
                include_synced: inc("include_synced"),
                modrinth: ExportModrinthOptions { name: pack_name.clone(), version: pack_version.clone(), summary: None },
                curseforge: ExportCurseforgeOptions { name: pack_name, version: pack_version, author: None, recommended_ram: None },
            };
            let (op, modal) = api.register_op("instances.export");
            backend.self_handle.send(MessageToBackend::ExportInstance { id, format, options, output, modal_action: modal });
            Ok(json!({"op": op}))
        }
        "instances.import_file" => {
            let path = std::path::PathBuf::from(p_str(params, "file")?);
            if !path.is_file() {
                return Err(format!("file {path:?} does not exist or is not a regular file"));
            }
            let (op, modal) = api.register_op("instances.import_file");
            backend.self_handle.send(MessageToBackend::CreateInstanceFromFile { file: path, modal_action: modal });
            Ok(json!({"op": op}))
        }
        "import.scan" => {
            let launcher = parse_other_launcher(p_str(params, "launcher")?)?;
            let path: Arc<Path> = Arc::from(Path::new(p_str(params, "path")?));
            let (tx, rx) = tokio::sync::oneshot::channel();
            backend.self_handle.send(MessageToBackend::GetImportFromOtherLauncherJob { channel: tx, launcher, path });
            let job = tokio::time::timeout(Duration::from_secs(15), rx).await
                .map_err(|_| "timed out scanning launcher")?
                .map_err(|_| "backend dropped the request")?;
            match job {
                Some(job) => Ok(json!({
                    "found": true,
                    "import_accounts": job.import_accounts,
                    "root": job.root.to_string_lossy(),
                    "paths": job.paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
                })),
                None => Ok(json!({"found": false})),
            }
        }
        "import.run" => {
            let launcher = parse_other_launcher(p_str(params, "launcher")?)?;
            let root: Arc<Path> = Arc::from(Path::new(p_str(params, "root")?));
            let paths_raw = params.get("paths").and_then(|v| v.as_array()).ok_or("missing 'paths' array")?;
            let mut paths = Vec::with_capacity(paths_raw.len());
            for p in paths_raw {
                paths.push(Arc::<Path>::from(Path::new(p.as_str().ok_or("paths entries must be strings")?)));
            }
            let import_accounts = params.get("import_accounts").and_then(|v| v.as_bool()).unwrap_or(false);
            let import_job = ImportFromOtherLauncherJob { import_accounts, root, paths };
            let (op, modal) = api.register_op("import.run");
            backend.self_handle.send(MessageToBackend::ImportFromOtherLauncher { launcher, import_job, modal_action: modal });
            Ok(json!({"op": op}))
        }

        // ── Instance management ─────────────────────────────────────────────
        "instances.relocate" => {
            let id = api.resolve_instance_id(params)?;
            let path = std::path::PathBuf::from(p_str(params, "path")?);
            backend.self_handle.send(MessageToBackend::RelocateInstance { id, path });
            Ok(json!({"requested": true}))
        }
        "instances.create_shortcut" => {
            let id = api.resolve_instance_id(params)?;
            let path = std::path::PathBuf::from(p_str(params, "path")?);
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() && !parent.is_dir() {
                    return Err(format!("shortcut parent directory {parent:?} does not exist"));
                }
            }
            backend.self_handle.send(MessageToBackend::CreateInstanceShortcut { id, path });
            Ok(json!({"requested": true}))
        }

        // ── Content: modpack children ───────────────────────────────────────
        "content.set_child_enabled" => {
            let id = api.resolve_instance_id(params)?;
            let content_id = content_id_from(params, "content_id")?;
            backend.self_handle.send(MessageToBackend::SetContentChildEnabled {
                id,
                content_id,
                child_id: params.get("child_id").and_then(|v| v.as_str()).map(Into::into),
                child_name: params.get("child_name").and_then(|v| v.as_str()).map(Into::into),
                child_filename: p_str(params, "child_filename")?.into(),
                disabled_default: params.get("disabled_default").and_then(|v| v.as_bool()).unwrap_or(false),
                enabled: p_bool(params, "enabled")?,
            });
            Ok(json!({"requested": true}))
        }
        "content.download_children" => {
            let id = api.resolve_instance_id(params)?;
            let content_id = content_id_from(params, "content_id")?;
            let (op, modal) = api.register_op("content.download_children");
            backend.self_handle.send(MessageToBackend::DownloadContentChildren { id, content_id, modal_action: modal });
            Ok(json!({"op": op}))
        }

        // ── Reordering ──────────────────────────────────────────────────────
        "servers.reorder" => {
            let id = api.resolve_instance_id(params)?;
            let from_index = p_usize(params, "from_index")?;
            let to_index = p_usize(params, "to_index")?;
            backend.self_handle.send(MessageToBackend::ReorderServers { id, from_index, to_index });
            Ok(json!({"requested": true}))
        }
        "accounts.reorder" => {
            let from_index = p_usize(params, "from_index")?;
            let delta = p_i64(params, "delta")? as isize;
            backend.self_handle.send(MessageToBackend::ReorderAccounts { from_index, delta });
            Ok(json!({"requested": true}))
        }

        // ── Accounts: re-auth, skins, capes ─────────────────────────────────
        "accounts.reauth" => {
            let uuid = parse_uuid(params, "uuid")?;
            {
                let mut guard = backend.account_info.write();
                match guard.get().accounts.get(&uuid) {
                    None => return Err(format!("no account with uuid {uuid}")),
                    Some(account) if account.offline => return Err("offline accounts have no Microsoft credentials to refresh".into()),
                    _ => {}
                }
            }
            let (op, modal) = api.register_op("accounts.reauth");
            backend.self_handle.send(MessageToBackend::Login { account: uuid, modal_action: modal });
            Ok(json!({"op": op, "hint": "poll ops.status for visit_url if interaction is required"}))
        }
        "accounts.skin_get" => {
            let account = parse_uuid(params, "uuid")?;
            api.require_online_account(account)?;
            let (tx, rx) = tokio::sync::oneshot::channel();
            backend.self_handle.send(MessageToBackend::GetAccountSkin { account, result: tx });
            let result = tokio::time::timeout(Duration::from_secs(30), rx).await
                .map_err(|_| "timed out fetching skin")?
                .map_err(|_| "backend dropped the request")?;
            use bridge::message::AccountSkinResult::*;
            Ok(match result {
                Success { skin, variant } => json!({
                    "status": "ok",
                    "variant": format!("{variant:?}").to_lowercase(),
                    "skin_png_base64": skin.map(|b| base64::engine::general_purpose::STANDARD.encode(&*b)),
                }),
                NeedsLogin => json!({"status": "needs_login"}),
                UnableToLoadSkin => json!({"status": "unable_to_load"}),
            })
        }
        "accounts.skin_set" => {
            let account = parse_uuid(params, "uuid")?;
            let skin = decode_b64_png(p_str(params, "skin")?)?;
            let variant = match p_str(params, "variant")?.to_ascii_lowercase().as_str() {
                "slim" => SkinVariant::Slim,
                "classic" => SkinVariant::Classic,
                other => return Err(format!("unknown skin variant '{other}' (classic|slim)")),
            };
            backend.self_handle.send(MessageToBackend::SetAccountSkin { account, skin, variant });
            Ok(json!({"requested": true}))
        }
        "accounts.capes_get" => {
            let account = parse_uuid(params, "uuid")?;
            api.require_online_account(account)?;
            let (tx, rx) = tokio::sync::oneshot::channel();
            backend.self_handle.send(MessageToBackend::GetAccountCapes { account, result: tx });
            let result = tokio::time::timeout(Duration::from_secs(30), rx).await
                .map_err(|_| "timed out fetching capes")?
                .map_err(|_| "backend dropped the request")?;
            use bridge::message::AccountCapesResult::*;
            Ok(match result {
                Success { capes } => json!({
                    "status": "ok",
                    "capes": capes.iter().map(|c| json!({
                        "id": c.id.to_string(),
                        "state": format!("{:?}", c.state).to_lowercase(),
                        "url": c.url.as_ref(),
                        "alias": c.alias.as_ref(),
                    })).collect::<Vec<_>>(),
                }),
                NeedsLogin => json!({"status": "needs_login"}),
            })
        }
        "accounts.cape_set" => {
            let account = parse_uuid(params, "uuid")?;
            let cape = match params.get("cape") {
                None | Some(Value::Null) => None,
                Some(v) => Some(Uuid::parse_str(v.as_str().ok_or("cape must be a uuid string or null")?)
                    .map_err(|e| format!("invalid cape uuid: {e}"))?),
            };
            backend.self_handle.send(MessageToBackend::SetAccountCape { account, cape });
            Ok(json!({"requested": true}))
        }

        // ── Skin library ────────────────────────────────────────────────────
        "skins.add" => {
            let source = if let Some(url) = params.get("url").and_then(|v| v.as_str()) {
                UrlOrFile::Url { url: url.into() }
            } else if let Some(path) = params.get("file").and_then(|v| v.as_str()) {
                let path = std::path::PathBuf::from(path);
                if !path.is_file() {
                    return Err(format!("file {path:?} does not exist or is not a regular file"));
                }
                UrlOrFile::File { path }
            } else {
                return Err("provide 'url' or 'file'".into());
            };
            backend.self_handle.send(MessageToBackend::AddToSkinLibrary { source });
            Ok(json!({"requested": true}))
        }
        "skins.library" => {
            // Trigger a load, then poll the skin manager's cached snapshot.
            backend.self_handle.send(MessageToBackend::RequestSkinLibrary);
            let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
            loop {
                {
                    let manager = backend.skin_manager.read();
                    if manager.skin_library_ready() {
                        let skins = manager.skin_library();
                        return Ok(json!({
                            "count": skins.len(),
                            "skins_png_base64": skins.iter()
                                .map(|b| base64::engine::general_purpose::STANDARD.encode(&**b))
                                .collect::<Vec<_>>(),
                        }));
                    }
                }
                if tokio::time::Instant::now() >= deadline {
                    return Err("skin library not loaded yet; retry shortly".into());
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        }
        "skins.remove" => {
            let skin = decode_b64_png(p_str(params, "skin")?)?;
            backend.self_handle.send(MessageToBackend::RemoveFromSkinLibrary { skin });
            Ok(json!({"requested": true}))
        }
        "skins.copy_player" => {
            let username = p_str(params, "username")?;
            backend.self_handle.send(MessageToBackend::CopyPlayerSkin { username: username.into() });
            Ok(json!({"requested": true}))
        }

        // ── Logs: upload / cleanup ──────────────────────────────────────────
        "logs.upload" => {
            let path: Arc<Path> = Arc::from(Path::new(p_str(params, "path")?));
            let (op, modal) = api.register_op("logs.upload");
            backend.self_handle.send(MessageToBackend::UploadLogFile { path, modal_action: modal });
            Ok(json!({"op": op, "hint": "poll ops.status; the mclo.gs url appears in visit_url when done"}))
        }
        "logs.cleanup" => {
            let id = api.resolve_instance_id(params)?;
            backend.self_handle.send(MessageToBackend::CleanupOldLogFiles { instance: id });
            Ok(json!({"requested": true}))
        }

        // ── Launcher self-update ────────────────────────────────────────────
        "launcher.install_update" => {
            let update = api.cached_update.lock().clone();
            let Some(update) = update else {
                return Err("no update available (none offered this session; check launcher.update_available events)".into());
            };
            if let Some(expected) = params.get("expected_version").and_then(|v| v.as_str()) {
                if expected != update.new_version.as_ref() {
                    return Err(format!("expected_version '{expected}' does not match the available update '{}'", update.new_version));
                }
            }
            let (op, modal) = api.register_op("launcher.install_update");
            backend.self_handle.send(MessageToBackend::InstallUpdate { update, modal_action: modal });
            Ok(json!({"op": op, "note": "the launcher restarts into the new version on success"}))
        }

        // ── Metadata prefetch (heavy, see docs) ─────────────────────────────
        "metadata.download_all" => {
            backend.self_handle.send(MessageToBackend::DownloadAllMetadata);
            Ok(json!({
                "requested": true,
                "warning": "downloads every version+asset index; blocks the backend loop for minutes and cannot report progress",
            }))
        }

        other => Err(format!("unknown command '{other}' (see docs/API.md)")),
    }
}

fn parse_content_ids(params: &Value) -> Result<Vec<InstanceContentID>, String> {
    let raw = params.get("content_ids").and_then(|v| v.as_array()).ok_or("missing 'content_ids' array")?;
    let mut ids = Vec::with_capacity(raw.len());
    for value in raw {
        let s = value.as_str().ok_or("content_ids entries must be 'index:generation' strings")?;
        let (index, generation) = decode_slab_id(s).ok_or_else(|| format!("invalid content id '{s}'"))?;
        ids.push(InstanceContentID { index, generation });
    }
    if ids.is_empty() {
        return Err("content_ids is empty".into());
    }
    Ok(ids)
}

/// Offline-mode UUID, matching vanilla's OfflinePlayer scheme (md5-based v3),
/// so the API-created account gets the same UUID an offline-mode server
/// derives from the username.
fn offline_uuid(username: &str) -> Uuid {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(format!("OfflinePlayer:{username}").as_bytes());
    let mut bytes: [u8; 16] = hasher.finalize().into();
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}
