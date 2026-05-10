use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use bridge::{
    instance::InstanceID,
    message::{ExportFormat, ExportOptions},
    modal_action::{ModalAction, ProgressTracker, ProgressTrackerFinishType}, safe_path::SafePath,
};
use once_cell::sync::Lazy;
use rustc_hash::FxHashSet;
use schema::{
    backend_config::SyncTargets,
    curseforge::{CurseforgeFingerprintRequest, CurseforgeFingerprintResponse},
    instance::InstanceConfiguration,
    loader::Loader,
    modification::{ModrinthEnv, ModrinthModpackFileDownload},
    modrinth::{
        ModrinthProjectsRequest, ModrinthSideRequirement, ModrinthVersionsFromHashesRequest,
        ModrinthVersionsFromHashesResponse,
    },
    mrpack::ModrinthIndexJson,
};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha512;
use walkdir::WalkDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

use crate::{
    BackendState,
    metadata::{
        items::{CurseforgeFingerprintMetadataItem, ModrinthProjectsMetadataItem, ModrinthVersionsFromHashesMetadataItem},
        manager::MetaLoadError,
    },
};

#[derive(Debug)]
enum ExportError {
    Cancelled,
    Other(String),
}

impl From<String> for ExportError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

impl From<&str> for ExportError {
    fn from(value: &str) -> Self {
        Self::Other(value.to_string())
    }
}

fn check_cancel(modal_action: &ModalAction) -> Result<(), ExportError> {
    if modal_action.has_requested_cancel() {
        Err(ExportError::Cancelled)
    } else {
        Ok(())
    }
}

#[derive(Clone)]
struct ExportFile {
    abs: PathBuf,
    rel: SafePath,
    enabled: bool,
}

struct ExportInstanceData {
    root_path: Arc<Path>,
    dot_minecraft_path: Arc<Path>,
    configuration: InstanceConfiguration,
    sync_targets: SyncTargets,
}

struct ModrinthResolvedFile {
    source: SafePath,
    sha1: String,
    sha512: String,
    url: String,
    size: u64,
    env: Option<ModrinthEnv>,
}

struct CurseforgeResolvedFile {
    rel_path: SafePath,
    project_id: u32,
    file_id: u32,
    enabled: bool,
    is_mod: bool,
}

pub async fn export_instance(
    backend: Arc<BackendState>,
    id: InstanceID,
    format: ExportFormat,
    options: ExportOptions,
    output: PathBuf,
    modal_action: ModalAction,
) {
    let instance_data = {
        let mut instance_state = backend.instance_state.write();
        if let Some(instance) = instance_state.instances.get_mut(id) {
            Some(ExportInstanceData {
                root_path: Arc::clone(&instance.root_path),
                dot_minecraft_path: Arc::clone(&instance.dot_minecraft_path),
                configuration: instance.configuration.get().clone(),
                sync_targets: backend.config.write().get().sync_targets.clone(),
            })
        } else {
            None
        }
    };

    let Some(instance) = instance_data else {
        modal_action.set_error_message("Unable to export instance, unknown id".into());
        modal_action.set_finished();
        return;
    };

    let result: Result<(), ExportError> = match format {
        ExportFormat::Zip => export_instance_zip(&backend, &instance, &options, &output, &modal_action).await,
        ExportFormat::Modrinth => export_modrinth_pack(&backend, &instance, &options, &output, &modal_action).await,
        ExportFormat::Curseforge => export_curseforge_pack(&backend, &instance, &options, &output, &modal_action).await,
    };

    if let Err(error) = result {
        match error {
            ExportError::Cancelled => {
                for tracker in modal_action.trackers.trackers.read().iter() {
                    if tracker.get_finished_at().is_none() {
                        tracker.set_finished(ProgressTrackerFinishType::Fast);
                        tracker.notify();
                    }
                }
            }
            ExportError::Other(error) => modal_action.set_error_message(error.into()),
        }
    }
    modal_action.set_finished();
}

async fn export_instance_zip(
    backend: &BackendState,
    instance: &ExportInstanceData,
    options: &ExportOptions,
    output: &Path,
    modal_action: &ModalAction,
) -> Result<(), ExportError> {
    check_cancel(modal_action)?;
    let tracker = ProgressTracker::new("Collecting files...".into(), backend.send.clone());
    modal_action.trackers.push(tracker.clone());

    let files = collect_files(
        &instance.root_path,
        &instance.dot_minecraft_path,
        options,
        &instance.sync_targets,
        &backend.directories.synced_dir,
        modal_action,
    )?;
    tracker.notify();
    tracker.set_finished(ProgressTrackerFinishType::Normal);

    let write_tracker = ProgressTracker::new("Writing zip".into(), backend.send.clone());
    modal_action.trackers.push(write_tracker.clone());
    write_tracker.set_total(files.len());
    write_tracker.notify();

    write_zip(output, &files, &[], &HashSet::new(), None, modal_action, &write_tracker)?;
    write_tracker.set_finished(ProgressTrackerFinishType::Normal);
    Ok(())
}

async fn export_modrinth_pack(
    backend: &BackendState,
    instance: &ExportInstanceData,
    options: &ExportOptions,
    output: &Path,
    modal_action: &ModalAction,
) -> Result<(), ExportError> {
    check_cancel(modal_action)?;
    let collect_tracker = ProgressTracker::new("Collecting files...".into(), backend.send.clone());
    modal_action.trackers.push(collect_tracker.clone());

    let files = collect_files(
        &instance.dot_minecraft_path,
        &instance.dot_minecraft_path,
        options,
        &instance.sync_targets,
        &backend.directories.synced_dir,
        modal_action,
    )?;
    collect_tracker.notify();
    collect_tracker.set_finished(ProgressTrackerFinishType::Normal);

    let hash_tracker = ProgressTracker::new("Hashing mods".into(), backend.send.clone());
    modal_action.trackers.push(hash_tracker.clone());

    let resolved = resolve_modrinth_files(backend, instance, options, &files, modal_action, &hash_tracker).await?;
    hash_tracker.set_finished(ProgressTrackerFinishType::Normal);

    let mut exclude = HashSet::new();
    for resolved_file in &resolved {
        exclude.insert(resolved_file.source.clone());
    }

    let index_json = build_modrinth_index(instance, options, &resolved)?;
    let extra_files = vec![("modrinth.index.json".to_string(), index_json)];

    let write_tracker = ProgressTracker::new("Writing zip".into(), backend.send.clone());
    modal_action.trackers.push(write_tracker.clone());
    write_tracker.set_total(files.len());
    write_tracker.notify();

    write_zip(output, &files, &extra_files, &exclude, Some(SafePath::new("overrides").unwrap()), modal_action, &write_tracker)?;
    write_tracker.set_finished(ProgressTrackerFinishType::Normal);
    Ok(())
}

async fn export_curseforge_pack(
    backend: &BackendState,
    instance: &ExportInstanceData,
    options: &ExportOptions,
    output: &Path,
    modal_action: &ModalAction,
) -> Result<(), ExportError> {
    check_cancel(modal_action)?;
    let collect_tracker = ProgressTracker::new("Collecting files...".into(), backend.send.clone());
    modal_action.trackers.push(collect_tracker.clone());

    let files = collect_files(
        &instance.dot_minecraft_path,
        &instance.dot_minecraft_path,
        options,
        &instance.sync_targets,
        &backend.directories.synced_dir,
        modal_action,
    )?;
    collect_tracker.notify();
    collect_tracker.set_finished(ProgressTrackerFinishType::Normal);

    let hash_tracker = ProgressTracker::new("Hashing mods".into(), backend.send.clone());
    modal_action.trackers.push(hash_tracker.clone());

    let resolved = resolve_curseforge_files(backend, instance, options, &files, modal_action, &hash_tracker).await?;
    hash_tracker.set_finished(ProgressTrackerFinishType::Normal);

    let mut exclude = HashSet::new();
    for resolved_file in &resolved {
        exclude.insert(resolved_file.rel_path.clone());
    }

    let manifest_json = build_curseforge_manifest(instance, options, &resolved)?;
    let modlist_html = build_curseforge_modlist(&resolved);
    let extra_files = vec![
        ("manifest.json".to_string(), manifest_json),
        // This is a legacy/optional artifact included by some exporters (e.g. PrismLauncher).
        ("modlist.html".to_string(), modlist_html),
    ];

    let write_tracker = ProgressTracker::new("Writing zip".into(), backend.send.clone());
    modal_action.trackers.push(write_tracker.clone());
    write_tracker.set_total(files.len());
    write_tracker.notify();

    write_zip(output, &files, &extra_files, &exclude, Some(SafePath::new("overrides").unwrap()), modal_action, &write_tracker)?;
    write_tracker.set_finished(ProgressTrackerFinishType::Normal);
    Ok(())
}

fn collect_files(
    root: &Path,
    dot_minecraft_path: &Path,
    options: &ExportOptions,
    sync_targets: &SyncTargets,
    synced_dir: &Path,
    modal_action: &ModalAction,
) -> Result<Vec<ExportFile>, ExportError> {
    let mut files = Vec::new();
    let sync_target_paths = SyncTargetPaths::new(sync_targets);
    let walker = WalkDir::new(root).follow_links(true);

    for entry in walker.into_iter() {
        check_cancel(modal_action)?;
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_dir() {
            continue;
        }

        let Ok(rel) = entry.path().strip_prefix(root) else {
            continue;
        };
        let Some(rel) = SafePath::from_std_path(rel) else {
            continue;
        };
        if rel.as_ref().components().next().is_none() {
            continue;
        }

        let rel_to_dot_minecraft = entry
            .path()
            .strip_prefix(dot_minecraft_path)
            .ok()
            .and_then(SafePath::from_std_path);

        if !options.include_synced {
            if let Ok(real_path) = entry.path().canonicalize() {
                if real_path.starts_with(synced_dir) {
                    continue;
                }
            }
            if let Some(rel_to_dot_minecraft) = rel_to_dot_minecraft.as_ref() {
                if matches_sync_target(rel_to_dot_minecraft, &sync_target_paths) {
                    continue;
                }
            }
        }

        if is_export_junk(&rel) {
            continue;
        }

        if should_skip(&rel, rel_to_dot_minecraft.as_ref(), options) {
            continue;
        }

        let enabled = if let Some(filename) = rel.file_name() {
            !filename.ends_with(".disabled")
        } else {
            true
        };
        files.push(ExportFile {
            abs: entry.path().to_path_buf(),
            rel,
            enabled,
        });
    }

    Ok(files)
}

struct SyncTargetPaths {
    files: Vec<SafePath>,
    folders: Vec<SafePath>,
}

impl SyncTargetPaths {
    fn new(sync_targets: &SyncTargets) -> Self {
        let mut files = Vec::new();
        let mut folders = Vec::new();

        for target in sync_targets.files.iter() {
            if let Some(path) = SafePath::new(target) {
                files.push(path);
            }
        }
        for target in sync_targets.folders.iter() {
            if let Some(path) = SafePath::new(target) {
                folders.push(path);
            }
        }

        Self { files, folders }
    }
}

fn matches_sync_target(rel_to_dot_minecraft: &SafePath, sync_targets: &SyncTargetPaths) -> bool {
    for folder in &sync_targets.folders {
        if rel_to_dot_minecraft == folder || rel_to_dot_minecraft.starts_with(folder) {
            return true;
        }
    }
    for file in &sync_targets.files {
        if rel_to_dot_minecraft == file {
            return true;
        }
    }
    false
}

static KNOWN_CACHE_FILES: Lazy<FxHashSet<&'static str>> = Lazy::new(|| {
    let mut set = FxHashSet::default();
    set.insert("usercache.json");
    set.insert("usernamecache.json");
    set.insert("realms_persistence.json");
    set
});

static IGNORED_FILES: Lazy<FxHashSet<&'static str>> = Lazy::new(|| {
    let mut set = FxHashSet::default();
    set.insert("config/sodium-fingerprint.json");
    set.insert("config/flashback/.flashback.json.backup");
    set.insert("config/axiom/.axiom.json.backup");
    set.insert("config/axiom/.license");
    set.insert("servers.dat_old");
    set
});

fn should_skip(rel: &SafePath, rel_to_dot_minecraft: Option<&SafePath>, options: &ExportOptions) -> bool {
    // Exclude log artifacts regardless of where they are in the instance.
    if !options.include_logs {
        if let Some(file_name) = rel.file_name() {
            if ends_with_ignore_ascii_case(file_name, ".log") || ends_with_ignore_ascii_case(file_name, ".log.gz") {
                return true;
            }
        }
    }

    if rel.starts_with(".fabric")
        || rel.starts_with("mods/.connector")
        || rel.starts_with("config/axiom/history")
        || IGNORED_FILES.contains(rel.as_str())
    {
        return true;
    }

    if !options.include_cache && KNOWN_CACHE_FILES.contains(rel.as_str()) {
        return true;
    }

    // The common include/exclude toggles are defined relative to the .minecraft folder.
    let rel = rel_to_dot_minecraft.unwrap_or(rel);
    let Some(first_component) = rel.as_ref().components().next() else {
        return true;
    };
    let relative_path::Component::Normal(name) = first_component else {
        return true;
    };

    match name {
        "logs" | "crash-reports" => !options.include_logs,
        ".cache" | "downloads" => !options.include_cache,
        "saves" => !options.include_saves,
        "mods" => !options.include_mods,
        "resourcepacks" => !options.include_resourcepacks,
        "config" => !options.include_configs,
        _ => false,
    }
}

fn is_export_junk(rel: &SafePath) -> bool {
    let Some(file_name) = rel.file_name() else {
        return false;
    };

    // OS junk
    if file_name == ".DS_Store" || file_name.eq_ignore_ascii_case("thumbs.db") {
        return true;
    }

    // Pandora internal metadata/temp files.
    if file_name.starts_with(".pandora.") {
        return true;
    }
    if file_name.starts_with(".") && file_name.ends_with(".aux.json") {
        return true;
    }

    false
}

fn ends_with_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if haystack.len() < needle.len() {
        return false;
    }
    let start = haystack.len() - needle.len();
    haystack[start..].eq_ignore_ascii_case(needle)
}

async fn resolve_modrinth_files(
    backend: &BackendState,
    _instance: &ExportInstanceData,
    options: &ExportOptions,
    files: &[ExportFile],
    modal_action: &ModalAction,
    tracker: &ProgressTracker,
) -> Result<Vec<ModrinthResolvedFile>, ExportError> {
    let mut candidates = Vec::new();
    for file in files {
        if is_mod_file(&file.rel) && options.include_mods {
            candidates.push(file);
            continue;
        }
        if is_resourcepack_file(&file.rel) && options.include_resourcepacks {
            candidates.push(file);
        }
    }

    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    tracker.set_total(candidates.len());
    tracker.notify();

    let mut buf = vec![0_u8; 128 * 1024];

    struct CandidateInfo {
        source: SafePath,
        sha512: Arc<str>,
    }

    let mut candidate_infos: Vec<CandidateInfo> = Vec::with_capacity(candidates.len());
    let mut hashes: Vec<Arc<str>> = Vec::with_capacity(candidates.len());
    for file in candidates {
        check_cancel(modal_action)?;
        tracker.add_count(1);
        tracker.notify();

        let (_sha1_hex, sha512_hex, _size) = compute_hashes(&file.abs, modal_action, &mut buf)?;

        let sha512: Arc<str> = sha512_hex.into();
        hashes.push(Arc::clone(&sha512));

        candidate_infos.push(CandidateInfo {
            source: file.rel.clone(),
            sha512,
        });
    }

    let request = ModrinthVersionsFromHashesRequest {
        hashes: hashes.into(),
        algorithm: "sha512".into(),
    };

    let response: Arc<ModrinthVersionsFromHashesResponse> = backend
        .meta
        .fetch(&ModrinthVersionsFromHashesMetadataItem(&request))
        .await
        .map_err(|e| format!("Error resolving Modrinth versions: {}", e))?;

    let mut versions_by_hash: HashMap<&str, &schema::modrinth::ModrinthProjectVersion> = HashMap::new();
    for (hash, version) in response.0.iter() {
        if let Some(version) = version.as_ref() {
            versions_by_hash.insert(hash.as_ref(), version);
        }
    }

    // Collect project ids for env side-support lookup.
    let mut project_ids = HashSet::<Arc<str>>::new();
    for info in &candidate_infos {
        if let Some(version) = versions_by_hash.get(info.sha512.as_ref()) {
            project_ids.insert(Arc::clone(&version.project_id));
        }
    }

    let projects_map: HashMap<Arc<str>, schema::modrinth::ModrinthProjectResult> = if project_ids.is_empty() {
        HashMap::new()
    } else {
        let ids_vec: Vec<Arc<str>> = project_ids.into_iter().collect();
        let req = ModrinthProjectsRequest { ids: ids_vec.into() };
        let projects: Arc<schema::modrinth::ModrinthProjectsResponse> = backend
            .meta
            .fetch(&ModrinthProjectsMetadataItem(&req))
            .await
            .map_err(|e| format!("Error resolving Modrinth projects: {}", e))?;

        projects
            .0
            .iter()
            .cloned()
            .map(|p| (Arc::clone(&p.id), p))
            .collect()
    };

    let mut resolved = Vec::new();
    for info in candidate_infos {
        check_cancel(modal_action)?;

        let Some(version) = versions_by_hash.get(info.sha512.as_ref()) else {
            continue;
        };

        // Match the exact file for this sha512.
        let Some(file_entry) = version
            .files
            .iter()
            .find(|f| f.hashes.sha512.as_deref() == Some(info.sha512.as_ref()))
        else {
            continue;
        };

        let mut env: Option<ModrinthEnv> = None;
        if let Some(project) = projects_map.get(version.project_id.as_ref()) {
            let client = project.client_side.unwrap_or(ModrinthSideRequirement::Required);
            let server = project.server_side.unwrap_or(ModrinthSideRequirement::Required);

            env = Some(ModrinthEnv { client, server });
        }

        resolved.push(ModrinthResolvedFile {
            source: info.source,
            sha1: file_entry.hashes.sha1.as_ref().to_string(),
            sha512: info.sha512.as_ref().to_string(),
            url: file_entry.url.as_ref().to_string(),
            size: file_entry.size as u64,
            env,
        });
    }

    Ok(resolved)
}

async fn resolve_curseforge_files(
    backend: &BackendState,
    _instance: &ExportInstanceData,
    options: &ExportOptions,
    files: &[ExportFile],
    modal_action: &ModalAction,
    tracker: &ProgressTracker,
) -> Result<Vec<CurseforgeResolvedFile>, ExportError> {
    let mut candidates = Vec::new();

    for file in files {
        check_cancel(modal_action)?;
        if is_mod_file(&file.rel) && options.include_mods {
            candidates.push((file.rel.clone(), file.abs.clone(), file.enabled, true));
            continue;
        }
        if is_resourcepack_file(&file.rel) && options.include_resourcepacks {
            candidates.push((file.rel.clone(), file.abs.clone(), file.enabled, false));
        }
    }

    tracker.set_total(candidates.len());
    tracker.notify();

    let mut fingerprint_to_candidate: HashMap<u32, (SafePath, bool, bool)> = HashMap::new();
    let mut fingerprints = Vec::new();

    for (rel, abs, enabled, is_mod) in candidates {
        check_cancel(modal_action)?;
        tracker.add_count(1);
        tracker.notify();
        let fingerprint = compute_murmur2(&abs)?;
        fingerprint_to_candidate.insert(fingerprint, (rel, enabled, is_mod));
        fingerprints.push(fingerprint);
    }

    if fingerprints.is_empty() {
        return Ok(Vec::new());
    }

    let request = CurseforgeFingerprintRequest { fingerprints };
    let response: Arc<CurseforgeFingerprintResponse> = backend
        .meta
        .fetch(&CurseforgeFingerprintMetadataItem(&request))
        .await
        .map_err(|e| match e {
            MetaLoadError::NonOK(code) => format!("CurseForge API error: {code}"),
            _ => format!("CurseForge API error: {}", e),
        })?;

    let mut resolved = Vec::new();
    for match_item in response.data.exact_matches.iter() {
        check_cancel(modal_action)?;
        let fingerprint = match_item.file.file_fingerprint;
        let Some((rel, enabled, is_mod)) = fingerprint_to_candidate.get(&fingerprint) else {
            continue;
        };
        resolved.push(CurseforgeResolvedFile {
            rel_path: rel.clone(),
            project_id: match_item.file.mod_id,
            file_id: match_item.file.id,
            enabled: *enabled,
            is_mod: *is_mod,
        });
    }

    Ok(resolved)
}

fn build_modrinth_index(
    instance: &ExportInstanceData,
    options: &ExportOptions,
    resolved: &[ModrinthResolvedFile],
) -> Result<Vec<u8>, String> {
    let config = &instance.configuration;

    let mut dependencies = indexmap::IndexMap::new();
    dependencies.insert("minecraft".into(), config.minecraft_version.as_str().into());
    if let Some(loader_version) = config.preferred_loader_version {
        match config.loader {
            Loader::Fabric => { dependencies.insert("fabric-loader".into(), loader_version.as_str().into()); },
            Loader::Forge => { dependencies.insert("forge".into(), loader_version.as_str().into()); },
            Loader::NeoForge => { dependencies.insert("neoforge".into(), loader_version.as_str().into()); },
            _ => {}
        }
    }

    let summary = options.modrinth.summary.as_ref().and_then(|s| {
        if s.is_empty() { None } else { Some(Arc::<str>::clone(s)) }
    });

    let files_out: Vec<ModrinthModpackFileDownload> = resolved
        .iter()
        .map(|file| ModrinthModpackFileDownload {
            path: file.source.as_str().into(),
            hashes: schema::modrinth::ModrinthHashes {
                sha1: file.sha1.as_str().into(),
                sha512: Some(file.sha512.as_str().into()),
            },
            env: file.env,
            downloads: Arc::from([file.url.as_str().into()]),
            file_size: file.size as usize,
        })
        .collect();

    let index = ModrinthIndexJson {
        format_version: 1,
        game: "minecraft".into(),
        version_id: Arc::<str>::clone(&options.modrinth.version),
        name: Arc::<str>::clone(&options.modrinth.name),
        summary,
        files: files_out.into(),
        dependencies,
        authors: None,
        author: None,
    };

    serde_json::to_vec(&index).map_err(|e| e.to_string())
}

fn build_curseforge_manifest(
    instance: &ExportInstanceData,
    options: &ExportOptions,
    resolved: &[CurseforgeResolvedFile],
) -> Result<Vec<u8>, String> {
    let config = &instance.configuration;

    let mut obj = serde_json::Map::new();
    obj.insert("manifestType".into(), serde_json::Value::from("minecraftModpack"));
    obj.insert("manifestVersion".into(), serde_json::Value::from(1));
    obj.insert("name".into(), serde_json::Value::from(options.curseforge.name.as_ref()));
    obj.insert("version".into(), serde_json::Value::from(options.curseforge.version.as_ref()));
    if let Some(author) = options.curseforge.author.as_ref() {
        if !author.is_empty() {
            obj.insert("author".into(), serde_json::Value::from(author.as_ref()));
        }
    }
    obj.insert("overrides".into(), serde_json::Value::from("overrides"));

    let mut minecraft = serde_json::Map::new();
    minecraft.insert("version".into(), serde_json::Value::from(config.minecraft_version.as_str()));

    let mut mod_loaders = Vec::new();
    if let Some(loader_version) = config.preferred_loader_version {
        let loader_id = match config.loader {
            Loader::Fabric => format!("fabric-{}", loader_version),
            Loader::Forge => format!("forge-{}", loader_version),
            Loader::NeoForge => {
                if config.minecraft_version.as_str() == "1.20.1" {
                    format!("neoforge-1.20.1-{}", loader_version)
                } else {
                    format!("neoforge-{}", loader_version)
                }
            }
            _ => String::new(),
        };
        if !loader_id.is_empty() {
            mod_loaders.push(serde_json::json!({ "id": loader_id, "primary": true }));
        }
    }
    minecraft.insert("modLoaders".into(), serde_json::Value::Array(mod_loaders));

    if let Some(ram) = options.curseforge.recommended_ram {
        minecraft.insert("recommendedRam".into(), serde_json::Value::from(ram));
    }
    obj.insert("minecraft".into(), serde_json::Value::Object(minecraft));

    let mut files_out = Vec::new();
    for file in resolved {
        files_out.push(serde_json::json!({
            "projectID": file.project_id,
            "fileID": file.file_id,
            "required": file.enabled,
        }));
    }
    obj.insert("files".into(), serde_json::Value::Array(files_out));

    serde_json::to_vec(&serde_json::Value::Object(obj)).map_err(|e| e.to_string())
}

fn build_curseforge_modlist(resolved: &[CurseforgeResolvedFile]) -> Vec<u8> {
    let mut items = String::new();
    for file in resolved.iter().filter(|f| f.is_mod) {
        items.push_str(&format!(
            "<li><a href=\"https://www.curseforge.com/minecraft/mc-mods/{}\">{}</a></li>\n",
            file.project_id, file.project_id
        ));
    }
    let html = format!("<ul>{}</ul>", items);
    html.into_bytes()
}

fn write_zip(
    output: &Path,
    files: &[ExportFile],
    extra_files: &[(String, Vec<u8>)],
    exclude: &HashSet<SafePath>,
    prefix: Option<SafePath>,
    modal_action: &ModalAction,
    tracker: &ProgressTracker,
) -> Result<(), ExportError> {
    let temp_path = temp_output_path(output);
    if let Some(parent) = temp_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let temp_file = File::create(&temp_path).map_err(|e| e.to_string())?;
    let result: Result<(), ExportError> = (|| {
        let mut zip = ZipWriter::new(temp_file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        for (name, data) in extra_files {
            check_cancel(modal_action)?;
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            zip.write_all(data).map_err(|e| e.to_string())?;
        }

        let mut buffer = vec![0_u8; 1024 * 128];
        for file in files {
            check_cancel(modal_action)?;
            if exclude.contains(&file.rel) {
                continue;
            }
            let mut rel = file.rel.clone();
            if let Some(prefix) = &prefix {
                rel = prefix.join(&rel);
            }

            let mut input = File::open(&file.abs).map_err(|e| e.to_string())?;
            zip.start_file(rel.as_str(), options).map_err(|e| e.to_string())?;
            loop {
                check_cancel(modal_action)?;
                let read = input.read(&mut buffer).map_err(|e| e.to_string())?;
                if read == 0 {
                    break;
                }
                zip.write_all(&buffer[..read]).map_err(|e| e.to_string())?;
            }
            tracker.add_count(1);
            tracker.notify();
        }

        check_cancel(modal_action)?;
        zip.finish().map_err(|e| e.to_string())?;
        check_cancel(modal_action)?;
        fs::rename(&temp_path, output).map_err(|e| e.to_string())?;
        Ok(())
    })();

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
}

fn temp_output_path(output: &Path) -> PathBuf {
    let mut temp = output.to_path_buf();
    temp.add_extension("new");
    temp
}

fn compute_hashes(path: &Path, modal_action: &ModalAction, buffer: &mut [u8]) -> Result<(String, String, u64), ExportError> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut sha1 = Sha1::new();
    let mut sha512 = Sha512::new();
    let mut size = 0_u64;
    loop {
        check_cancel(modal_action)?;
        let read = file.read(buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        size += read as u64;
        sha1.update(&buffer[..read]);
        sha512.update(&buffer[..read]);
    }
    let sha1_hex = hex::encode(sha1.finalize());
    let sha512_hex = hex::encode(sha512.finalize());
    Ok((sha1_hex, sha512_hex, size))
}

fn compute_murmur2(path: &Path) -> Result<u32, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|e| e.to_string())?;
    Ok(murmur2_32(&data))
}

fn murmur2_32(data: &[u8]) -> u32 {
    const M: u32 = 0x5bd1_e995;
    const R: u32 = 24;

    let len = data.len() as u32;
    let mut h = len;

    let mut i = 0;
    while i + 4 <= data.len() {
        let mut k = u32::from_le_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
        k = k.wrapping_mul(M);
        k ^= k >> R;
        k = k.wrapping_mul(M);

        h = h.wrapping_mul(M);
        h ^= k;

        i += 4;
    }

    match data.len() & 3 {
        3 => {
            h ^= (data[i + 2] as u32) << 16;
            h ^= (data[i + 1] as u32) << 8;
            h ^= data[i] as u32;
            h = h.wrapping_mul(M);
        }
        2 => {
            h ^= (data[i + 1] as u32) << 8;
            h ^= data[i] as u32;
            h = h.wrapping_mul(M);
        }
        1 => {
            h ^= data[i] as u32;
            h = h.wrapping_mul(M);
        }
        _ => {}
    }

    h ^= h >> 13;
    h = h.wrapping_mul(M);
    h ^= h >> 15;

    h
}

fn is_mod_file(path: &SafePath) -> bool {
    if !path.starts_with("mods") {
        return false;
    }

    let Some(filename) = path.file_name() else {
        return false;
    };

    filename.ends_with(".jar")
        || filename.ends_with(".jar.disabled")
        || filename.ends_with(".zip")
        || filename.ends_with(".zip.disabled")
        || filename.ends_with(".litemod")
        || filename.ends_with(".litemod.disabled")
}

fn is_resourcepack_file(path: &SafePath) -> bool {
    if !path.starts_with("resourcepacks") {
        return false;
    }

    let Some(filename) = path.file_name() else {
        return false;
    };

    filename.ends_with(".zip") || filename.ends_with(".zip.disabled")
}
