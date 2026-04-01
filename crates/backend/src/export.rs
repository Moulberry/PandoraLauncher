use std::{
    collections::{HashMap, HashSet},
    ffi::OsStr,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use bridge::{
    instance::InstanceID,
    message::{ExportFormat, ExportOptions},
    modal_action::{ModalAction, ProgressTracker, ProgressTrackerFinishType},
};
use schema::{
    backend_config::SyncTargets,
    curseforge::{CurseforgeFingerprintRequest, CurseforgeFingerprintResponse},
    instance::InstanceConfiguration,
    loader::Loader,
    modrinth::ModrinthLoader,
};
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::{Digest as Sha2Digest, Sha512};
use walkdir::WalkDir;
use zip::{write::FileOptions, CompressionMethod, ZipWriter};

use crate::{
    BackendState,
    metadata::{
        items::{CurseforgeFingerprintMetadataItem, ModrinthVersionUpdateMetadataItem, VersionUpdateParameters},
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
    rel: PathBuf,
    enabled: bool,
}

struct ExportInstanceData {
    root_path: Arc<Path>,
    dot_minecraft_path: Arc<Path>,
    configuration: InstanceConfiguration,
    sync_targets: SyncTargets,
}

struct ModrinthResolvedFile {
    source_rel: PathBuf,
    rel_path: String,
    sha1: String,
    sha512: String,
    url: String,
    size: u64,
    optional: bool,
}

struct CurseforgeResolvedFile {
    rel_path: PathBuf,
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
    let tracker = ProgressTracker::new("Collecting files".into(), backend.send.clone());
    modal_action.trackers.push(tracker.clone());

    let files = collect_files(
        &instance.root_path,
        options,
        &instance.sync_targets,
        &backend.directories.synced_dir,
        modal_action,
    )?;
    tracker.set_total(files.len());
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
    let collect_tracker = ProgressTracker::new("Collecting files".into(), backend.send.clone());
    modal_action.trackers.push(collect_tracker.clone());

    let files = collect_files(
        &instance.dot_minecraft_path,
        options,
        &instance.sync_targets,
        &backend.directories.synced_dir,
        modal_action,
    )?;
    collect_tracker.set_total(files.len());
    collect_tracker.notify();
    collect_tracker.set_finished(ProgressTrackerFinishType::Normal);

    let hash_tracker = ProgressTracker::new("Hashing mods".into(), backend.send.clone());
    modal_action.trackers.push(hash_tracker.clone());

    let resolved = resolve_modrinth_files(backend, instance, options, &files, modal_action, &hash_tracker).await?;
    hash_tracker.set_finished(ProgressTrackerFinishType::Normal);

    let mut exclude = HashSet::new();
    for resolved_file in &resolved {
        exclude.insert(resolved_file.source_rel.clone());
    }

    let index_json = build_modrinth_index(instance, options, &resolved)?;
    let extra_files = vec![("modrinth.index.json".to_string(), index_json)];

    let write_tracker = ProgressTracker::new("Writing zip".into(), backend.send.clone());
    modal_action.trackers.push(write_tracker.clone());
    write_tracker.set_total(files.len());
    write_tracker.notify();

    write_zip(output, &files, &extra_files, &exclude, Some("overrides/"), modal_action, &write_tracker)?;
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
    let collect_tracker = ProgressTracker::new("Collecting files".into(), backend.send.clone());
    modal_action.trackers.push(collect_tracker.clone());

    let files = collect_files(
        &instance.dot_minecraft_path,
        options,
        &instance.sync_targets,
        &backend.directories.synced_dir,
        modal_action,
    )?;
    collect_tracker.set_total(files.len());
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
        ("modlist.html".to_string(), modlist_html),
    ];

    let write_tracker = ProgressTracker::new("Writing zip".into(), backend.send.clone());
    modal_action.trackers.push(write_tracker.clone());
    write_tracker.set_total(files.len());
    write_tracker.notify();

    write_zip(output, &files, &extra_files, &exclude, Some("overrides/"), modal_action, &write_tracker)?;
    write_tracker.set_finished(ProgressTrackerFinishType::Normal);
    Ok(())
}

fn collect_files(
    root: &Path,
    options: &ExportOptions,
    sync_targets: &SyncTargets,
    synced_dir: &Path,
    modal_action: &ModalAction,
) -> Result<Vec<ExportFile>, ExportError> {
    let mut files = Vec::new();
    let walker = WalkDir::new(root).follow_links(true);

    for entry in walker.into_iter() {
        check_cancel(modal_action)?;
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().is_dir() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_path_buf();

        if rel.as_os_str().is_empty() {
            continue;
        }

        if !options.include_synced {
            if let Ok(real_path) = entry.path().canonicalize() {
                if real_path.starts_with(synced_dir) {
                    continue;
                }
            }
            if matches_sync_target(&rel, sync_targets) {
                continue;
            }
        }

        if is_os_junk(&rel) {
            continue;
        }

        if should_skip(&rel, options) {
            continue;
        }

        let enabled = !rel.to_string_lossy().ends_with(".disabled");
        files.push(ExportFile {
            abs: entry.path().to_path_buf(),
            rel,
            enabled,
        });
    }

    Ok(files)
}

fn matches_sync_target(rel: &Path, sync_targets: &SyncTargets) -> bool {
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    let rel_str = rel_str.trim_start_matches("./");
    let mut candidates = vec![rel_str];

    if let Some(tail) = rel_str.strip_prefix("minecraft/") {
        candidates.push(tail);
    }
    if let Some(tail) = rel_str.strip_prefix(".minecraft/") {
        candidates.push(tail);
    }

    for candidate in candidates {
        for target in sync_targets.folders.iter() {
            let target = target.as_ref().replace('\\', "/");
            if candidate == target || candidate.starts_with(&(target + "/")) {
                return true;
            }
        }
        for target in sync_targets.files.iter() {
            let target = target.as_ref().replace('\\', "/");
            if candidate == target {
                return true;
            }
        }
    }

    false
}

fn should_skip(rel: &Path, options: &ExportOptions) -> bool {
    let Some(component) = rel.components().next() else {
        return false;
    };
    let name = match component {
        std::path::Component::Normal(name) => name.to_string_lossy(),
        _ => return false,
    };

    if !options.include_logs && (name == "logs" || name == "crash-reports") {
        return true;
    }
    if !options.include_cache && name == ".cache" {
        return true;
    }
    if !options.include_saves && name == "saves" {
        return true;
    }
    if !options.include_mods && name == "mods" {
        return true;
    }
    if !options.include_resourcepacks && name == "resourcepacks" {
        return true;
    }
    if !options.include_configs && name == "config" {
        return true;
    }

    false
}

fn is_os_junk(rel: &Path) -> bool {
    let Some(file_name) = rel.file_name().and_then(OsStr::to_str) else {
        return false;
    };
    matches!(file_name, ".DS_Store" | "Thumbs.db" | "thumbs.db")
}

async fn resolve_modrinth_files(
    backend: &BackendState,
    instance: &ExportInstanceData,
    options: &ExportOptions,
    files: &[ExportFile],
    modal_action: &ModalAction,
    tracker: &ProgressTracker,
) -> Result<Vec<ModrinthResolvedFile>, ExportError> {
    if !options.include_mods {
        return Ok(Vec::new());
    }

    let modrinth_loader = instance.configuration.loader.as_modrinth_loader();
    let minecraft_version = instance.configuration.minecraft_version;
    let params = VersionUpdateParameters {
        loaders: if modrinth_loader == ModrinthLoader::Unknown {
            Arc::<[ModrinthLoader]>::from([])
        } else {
            Arc::from([modrinth_loader])
        },
        game_versions: Arc::from([minecraft_version]),
    };

    let mod_files: Vec<&ExportFile> = files
        .iter()
        .filter(|file| is_mod_file(&file.rel))
        .collect();

    tracker.set_total(mod_files.len());
    tracker.notify();

    let mut resolved = Vec::new();
    for file in mod_files {
        check_cancel(modal_action)?;
        tracker.add_count(1);
        tracker.notify();

        let (sha1_hex, sha512_hex, size) = compute_hashes(&file.abs, modal_action)?;

        if modrinth_loader == ModrinthLoader::Unknown {
            continue;
        }

        let response = backend
            .meta
            .fetch(&ModrinthVersionUpdateMetadataItem {
                sha1: sha1_hex.clone().into(),
                params: params.clone(),
            })
            .await
            .map_err(|e| format!("Error resolving Modrinth file: {}", e))?;

        let file_url = response
            .0
            .files
            .iter()
            .find(|f| f.hashes.sha1.as_ref() == sha1_hex.as_str())
            .map(|f| f.url.as_ref().to_string());

        let Some(url) = file_url else {
            continue;
        };

        let mut rel_path = file.rel.to_string_lossy().replace('\\', "/");
        let optional = options.modrinth.optional_files && !file.enabled && rel_path.ends_with(".disabled");
        if optional {
            rel_path = rel_path.trim_end_matches(".disabled").to_string();
        }

        resolved.push(ModrinthResolvedFile {
            source_rel: file.rel.clone(),
            rel_path,
            sha1: sha1_hex,
            sha512: sha512_hex,
            url,
            size,
            optional,
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

    let mut fingerprint_to_candidate: HashMap<u32, (PathBuf, bool, bool)> = HashMap::new();
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
    let mut out = serde_json::Map::new();
    out.insert("formatVersion".into(), serde_json::Value::from(1));
    out.insert("game".into(), serde_json::Value::from("minecraft"));
    out.insert("name".into(), serde_json::Value::from(options.modrinth.name.as_ref()));
    out.insert("versionId".into(), serde_json::Value::from(options.modrinth.version.as_ref()));
    if let Some(summary) = options.modrinth.summary.as_ref() {
        if !summary.is_empty() {
            out.insert("summary".into(), serde_json::Value::from(summary.as_ref()));
        }
    }

    let config = &instance.configuration;
    let mut deps = serde_json::Map::new();
    deps.insert("minecraft".into(), serde_json::Value::from(config.minecraft_version.as_str()));
    if let Some(loader_version) = config.preferred_loader_version {
        match config.loader {
            Loader::Fabric => { deps.insert("fabric-loader".into(), serde_json::Value::from(loader_version.as_str())); },
            Loader::Forge => { deps.insert("forge".into(), serde_json::Value::from(loader_version.as_str())); },
            Loader::NeoForge => { deps.insert("neoforge".into(), serde_json::Value::from(loader_version.as_str())); },
            _ => {}
        }
    }
    out.insert("dependencies".into(), serde_json::Value::Object(deps));

    let mut files_out = Vec::new();
    for file in resolved {
        let mut env = serde_json::Map::new();
        if file.optional {
            env.insert("client".into(), serde_json::Value::from("optional"));
            env.insert("server".into(), serde_json::Value::from("optional"));
        } else {
            env.insert("client".into(), serde_json::Value::from("required"));
            env.insert("server".into(), serde_json::Value::from("required"));
        }

        let mut hashes = serde_json::Map::new();
        hashes.insert("sha1".into(), serde_json::Value::from(file.sha1.as_str()));
        hashes.insert("sha512".into(), serde_json::Value::from(file.sha512.as_str()));

        let mut file_out = serde_json::Map::new();
        file_out.insert("path".into(), serde_json::Value::from(file.rel_path.as_str()));
        file_out.insert("downloads".into(), serde_json::Value::from(vec![file.url.as_str()]));
        file_out.insert("hashes".into(), serde_json::Value::Object(hashes));
        file_out.insert("fileSize".into(), serde_json::Value::from(file.size));
        file_out.insert("env".into(), serde_json::Value::Object(env));
        files_out.push(serde_json::Value::Object(file_out));
    }
    out.insert("files".into(), serde_json::Value::Array(files_out));

    serde_json::to_vec(&serde_json::Value::Object(out)).map_err(|e| e.to_string())
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
        let required = file.enabled || !options.curseforge.optional_files;
        files_out.push(serde_json::json!({
            "projectID": file.project_id,
            "fileID": file.file_id,
            "required": required,
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
    exclude: &HashSet<PathBuf>,
    prefix: Option<&str>,
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
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

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
            let mut rel = file.rel.to_string_lossy().replace('\\', "/");
            if let Some(prefix) = prefix {
                rel = format!("{}{}", prefix, rel);
            }

            let mut input = File::open(&file.abs).map_err(|e| e.to_string())?;
            zip.start_file(rel, options).map_err(|e| e.to_string())?;
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
    let ext = output.extension().and_then(OsStr::to_str).unwrap_or("tmp");
    temp.set_extension(format!("{}.new", ext));
    temp
}

fn compute_hashes(path: &Path, modal_action: &ModalAction) -> Result<(String, String, u64), ExportError> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut sha1 = Sha1::new();
    let mut sha512 = Sha512::new();
    let mut buffer = [0_u8; 8192];
    let mut size = 0_u64;
    loop {
        check_cancel(modal_action)?;
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
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

fn is_mod_file(path: &Path) -> bool {
    let rel = path.to_string_lossy().replace('\\', "/");
    if !rel.starts_with("mods/") {
        return false;
    }
    rel.ends_with(".jar")
        || rel.ends_with(".jar.disabled")
        || rel.ends_with(".zip")
        || rel.ends_with(".zip.disabled")
        || rel.ends_with(".litemod")
        || rel.ends_with(".litemod.disabled")
}

fn is_resourcepack_file(path: &Path) -> bool {
    let rel = path.to_string_lossy().replace('\\', "/");
    if !rel.starts_with("resourcepacks/") {
        return false;
    }
    rel.ends_with(".zip") || rel.ends_with(".zip.disabled")
}
