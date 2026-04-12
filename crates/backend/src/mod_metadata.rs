use std::{
    hash::Hash, io::{BufRead, Cursor, Read, Write}, path::{Path, PathBuf}, sync::{Arc, atomic::{AtomicBool, Ordering}}
};

use bridge::{instance::{ContentSummary, ContentType, ContentUpdateStatus, UNKNOWN_CONTENT_SUMMARY}, safe_path::SafePath};
use image::{DynamicImage, GenericImageView, imageops::FilterType};
use indexmap::IndexMap;
use parking_lot::{RwLock, RwLockReadGuard};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rc_zip_sync::EntryHandle;
use rustc_hash::{FxHashMap, FxHashSet};
use schema::{content::ContentSource, curseforge::{CachedCurseforgeFileInfo, CurseforgeFile, CurseforgeModpackManifestJson}, fabric_mod::{FabricModJson, Icon, Person}, forge_mod::{JarJarMetadata, McModInfo, ModsToml}, loader::Loader, modrinth::{ModrinthFile, ModrinthSideRequirement}, mrpack::ModrinthIndexJson, resourcepack::PackMcmeta, unique_bytes::UniqueBytes};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DeserializeAs};
use sha1::{Digest, Sha1};
use ustr::Ustr;

#[derive(Clone)]
pub enum ContentUpdateAction {
    ErrorNotFound,
    ErrorInvalidHash,
    AlreadyUpToDate,
    ManualInstall,
    Modrinth {
        file: ModrinthFile,
        project_id: Arc<str>,
    },
    Curseforge {
        file: CurseforgeFile,
        project_id: u32,
    }
}

impl ContentUpdateAction {
    pub fn to_status(&self) -> ContentUpdateStatus {
        match self {
            ContentUpdateAction::ErrorNotFound => ContentUpdateStatus::ErrorNotFound,
            ContentUpdateAction::ErrorInvalidHash => ContentUpdateStatus::ErrorInvalidHash,
            ContentUpdateAction::AlreadyUpToDate => ContentUpdateStatus::AlreadyUpToDate,
            ContentUpdateAction::ManualInstall => ContentUpdateStatus::ManualInstall,
            ContentUpdateAction::Modrinth { .. } => ContentUpdateStatus::Modrinth,
            ContentUpdateAction::Curseforge { .. } => ContentUpdateStatus::Curseforge,
        }
    }
}

#[derive(Eq, Hash, PartialEq)]
pub struct ContentUpdateKey {
    pub hash: [u8; 20],
    pub loader: Loader,
    pub version: Ustr,
}

pub struct ModMetadataManager {
    content_library_dir: Arc<Path>,
    sources_dir: PathBuf,
    cached_curseforge_info_dat: PathBuf,
    by_hash: RwLock<FxHashMap<[u8; 20], Arc<ContentSummary>>>,
    content_sources: RwLock<ContentSources>,
    parents_by_missing_child: RwLock<FxHashMap<[u8; 20], FxHashSet<[u8; 20]>>>,
    cached_curseforge_info: RwLock<FxHashMap<u32, CachedCurseforgeFileInfo>>,
    parents_by_missing_curseforge_id: RwLock<FxHashMap<u32, FxHashSet<[u8; 20]>>>,
    curseforge_info_dirty: AtomicBool,
    pub updates: RwLock<FxHashMap<ContentUpdateKey, ContentUpdateAction>>,
}

impl ModMetadataManager {
    pub fn load(content_meta_dir: Arc<Path>, content_library_dir: Arc<Path>) -> Self {
        let legacy_sources_json = content_meta_dir.join("sources.json");
        let sources_dir = content_meta_dir.join("sources");
        let cached_curseforge_info_dat = content_meta_dir.join("cached_curseforge_info.dat");

        let content_sources = if sources_dir.is_dir() {
            ContentSources::load_all(&sources_dir).unwrap_or_default()
        } else if let Ok(data) = std::fs::read(&legacy_sources_json) {
            let legacy = serde_json::from_slice(&data);
            if let Ok(legacy) = legacy {
                let content_sources = ContentSources::from_legacy(legacy);
                content_sources.write_all_to_file(&sources_dir);
                _ = std::fs::remove_file(legacy_sources_json);
                content_sources
            } else {
                _ = std::fs::remove_file(legacy_sources_json);
                Default::default()
            }
        } else {
            Default::default()
        };

        let mut cached_curseforge_info = FxHashMap::default();
        if let Ok(data) = std::fs::read(&cached_curseforge_info_dat) {
            let mut cursor = Cursor::new(data);
            let mut buffer = [0_u8; 35];
            loop {
                let data_start = cursor.position() + 8;
                if cursor.read_exact(&mut buffer).is_err() {
                    break;
                }
                let checksum = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
                let data_len = u32::from_le_bytes(buffer[4..8].try_into().unwrap());
                let file_id = u32::from_le_bytes(buffer[8..12].try_into().unwrap());
                let hash: [u8; 20] = buffer[12..32].try_into().unwrap();
                let disabled_third_party_downloads = (buffer[32] & 1) == 1;
                let filename_length = u16::from_le_bytes(buffer[33..35].try_into().unwrap());

                let filename_start = cursor.position() as usize;
                let filename_end = filename_start + filename_length as usize;

                cursor.set_position(data_start + data_len as u64);

                // Calculate and compare checksum
                let all_bytes = &cursor.get_ref()[data_start as usize .. filename_end];
                let calculated_checksum = crc32fast::hash(all_bytes);
                if checksum != calculated_checksum {
                    log::error!("Cached curseforge info checksum failed, expected {:x}, got {:x}", checksum, calculated_checksum);
                    continue;
                }

                let filename_bytes = &cursor.get_ref()[filename_start .. filename_end];
                let Ok(filename_str) = str::from_utf8(filename_bytes) else {
                    continue;
                };

                let info = CachedCurseforgeFileInfo {
                    hash,
                    filename: filename_str.into(),
                    disabled_third_party_downloads,
                };
                cached_curseforge_info.insert(file_id, info);
            }
        }

        Self {
            content_library_dir,
            sources_dir,
            cached_curseforge_info_dat,
            by_hash: Default::default(),
            content_sources: RwLock::new(content_sources),
            parents_by_missing_child: Default::default(),
            cached_curseforge_info: RwLock::new(cached_curseforge_info),
            parents_by_missing_curseforge_id: Default::default(),
            curseforge_info_dirty: AtomicBool::new(false),
            updates: Default::default(),
        }
    }

    pub fn read_content_sources(&self) -> RwLockReadGuard<'_, ContentSources> {
        self.content_sources.read()
    }

    pub fn write_changes(&self) {
        if self.curseforge_info_dirty.swap(false, Ordering::AcqRel) {
            let mut data = Vec::new();
            for (file_id, info) in self.cached_curseforge_info.read().iter() {
                if info.filename.len() >= 1 << 15 {
                    continue;
                }

                let checksum_start = data.len();
                data.extend_from_slice(&[0; 8]);

                let data_start = data.len();
                data.extend_from_slice(&u32::to_le_bytes(*file_id));
                data.extend_from_slice(&info.hash);
                data.push(info.disabled_third_party_downloads as u8);
                data.extend_from_slice(&u16::to_le_bytes(info.filename.len() as u16));
                data.extend_from_slice(info.filename.as_bytes());

                let bytes = &data[data_start..];
                let data_len = bytes.len();
                let checksum = crc32fast::hash(bytes);
                data[checksum_start..checksum_start+4].copy_from_slice(&u32::to_le_bytes(checksum));
                data[checksum_start+4..checksum_start+8].copy_from_slice(&u32::to_le_bytes(data_len as u32));
            }
            _ = crate::write_safe(&self.cached_curseforge_info_dat, &data);
        }
        self.content_sources.write().write_dirty_to_folder(&self.sources_dir);
    }

    pub fn set_content_sources(&self, sources: impl Iterator<Item = ([u8; 20], ContentSource)>) {
        let mut content_sources = self.content_sources.write();

        for (hash, source) in sources {
            content_sources.set(&hash, source);
        }
    }

    pub fn set_cached_curseforge_info(&self, file_id: u32, info: CachedCurseforgeFileInfo) {
        self.cached_curseforge_info.write().insert(file_id, info);
        self.curseforge_info_dirty.store(true, Ordering::Release);

        if let Some(parents) = self.parents_by_missing_curseforge_id.write().remove(&file_id) {
            // Remove cached summary of parent, so it can be recalculated next time it is requested
            let mut by_hash = self.by_hash.write();
            for parent in parents {
                by_hash.remove(&parent);
            }
        }
    }

    pub fn get_path(self: &Arc<Self>, path: &Path) -> Arc<ContentSummary> {
        let Ok(mut file) = std::fs::File::open(path) else {
            return UNKNOWN_CONTENT_SUMMARY.clone();
        };
        self.get_file(&mut file)
    }

    pub fn get_file(self: &Arc<Self>, file: &mut std::fs::File) -> Arc<ContentSummary> {
        let mut hasher = Sha1::new();
        let _ = std::io::copy(file, &mut hasher).ok().unwrap();
        let actual_hash: [u8; 20] = hasher.finalize().into();

        if let Some(summary) = self.by_hash.read().get(&actual_hash) {
            return summary.clone();
        }

        let summary = self.load_mod_summary(actual_hash, file, true);

        self.put(actual_hash, summary.clone());

        summary
    }

    pub fn get_cached_by_sha1(self: &Arc<Self>, sha1: &str) -> Option<Arc<ContentSummary>> {
        let mut hash = [0u8; 20];
        hex::decode_to_slice(sha1, &mut hash).ok()?;
        self.by_hash.read().get(&hash).cloned()
    }

    pub fn get_bytes(self: &Arc<Self>, bytes: &[u8]) -> Arc<ContentSummary> {
        let mut hasher = Sha1::new();
        hasher.write_all(bytes).ok().unwrap();
        let actual_hash: [u8; 20] = hasher.finalize().into();

        if let Some(summary) = self.by_hash.read().get(&actual_hash) {
            return summary.clone();
        }

        let summary = self.load_mod_summary(actual_hash, &bytes, true);

        self.put(actual_hash, summary.clone());

        summary
    }

    fn put(self: &Arc<Self>, hash: [u8; 20], summary: Arc<ContentSummary>) {
        self.by_hash.write().insert(hash, summary.clone());

        if let Some(parents) = self.parents_by_missing_child.write().remove(&hash) {
            // Remove cached summary of parent, so it can be recalculated next time it is requested
            let mut by_hash = self.by_hash.write();
            for parent in parents {
                by_hash.remove(&parent);
            }
        }
    }

    fn load_mod_summary<R: rc_zip_sync::ReadZip>(self: &Arc<Self>, hash: [u8; 20], file: &R, allow_children: bool) -> Arc<ContentSummary> {
        let Ok(archive) = file.read_zip() else {
            return UNKNOWN_CONTENT_SUMMARY.clone();
        };

        let summary = if let Some(file) = archive.by_name("mcmod.info") {
            self.load_legacy_forge_mod(hash, &archive, file)
        } else if let Some(file) = archive.by_name("fabric.mod.json") {
            self.load_fabric_mod(hash, &archive, file)
        } else if let Some(file) = archive.by_name("META-INF/mods.toml") {
            self.load_forge_mod(hash, &archive, file, ContentType::Forge)
        } else if let Some(file) = archive.by_name("META-INF/neoforge.mods.toml") {
            self.load_forge_mod(hash, &archive, file, ContentType::NeoForge)
        } else if let Some(file) = archive.by_name("META-INF/jarjar/metadata.json") {
            self.load_jarjar(hash, &archive, file)
        } else if let Some(file) = archive.by_name("META-INF/MANIFEST.MF") {
            self.load_from_java_manifest(hash, &archive, file)
        } else if let Some(file) = archive.by_name("pack.mcmeta") {
            self.load_from_pack_mcmeta(hash, &archive, file)
        } else if allow_children && let Some(file) = archive.by_name("modrinth.index.json") {
            self.load_modrinth_modpack(hash, &archive, file)
        } else if allow_children && let Some(file) = archive.by_name("manifest.json") {
            self.load_curseforge_modpack(hash, &archive, file)
        } else {
            None
        };
        if let Some(summary) = summary {
            summary
        } else {
            UNKNOWN_CONTENT_SUMMARY.clone()
        }
    }

    fn load_fabric_mod<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
        let mut bytes = file.bytes().ok()?;

        // Some mods violate the JSON spec by using raw newline characters inside strings (e.g. BetterGrassify)
        for byte in bytes.iter_mut() {
            if *byte == '\n' as u8 {
                *byte = ' ' as u8;
            }
        }

        let fabric_mod_json: FabricModJson = serde_json::from_slice(&bytes).inspect_err(|e| {
            log::error!("Error parsing fabric.mod.json: {e}");
        }).ok()?;

        drop(file);

        let name = fabric_mod_json.name.unwrap_or_else(|| Arc::clone(&fabric_mod_json.id));

        let icon = match fabric_mod_json.icon {
            Some(icon) => match icon {
                Icon::Single(icon) => Some(icon),
                Icon::Sizes(hash_map) => {
                    const DESIRED_SIZE: usize = 64;
                    hash_map.iter().min_by_key(|size| size.0.abs_diff(DESIRED_SIZE)).map(|e| Arc::clone(e.1))
                },
            },
            None => None,
        };

        let mut png_icon: Option<UniqueBytes> = None;
        if let Some(icon) = icon && let Some(icon_file) = archive.by_name(&icon) {
            png_icon = load_icon(icon_file);
        }

        let authors = if let Some(authors) = fabric_mod_json.authors && let Some(authors) = create_authors_string(&authors) {
            authors.into()
        } else {
            "".into()
        };

        Some(Arc::new(ContentSummary {
            id: Some(fabric_mod_json.id),
            hash,
            name: Some(name),
            authors,
            version_str: create_version_string(&fabric_mod_json.version),
            rich_description: None,
            png_icon,
            extra: ContentType::Fabric
        }))
    }

    fn load_forge_mod<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>, extra: ContentType) -> Option<Arc<ContentSummary>> {
        let bytes = file.bytes().ok()?;

        let mods_toml: ModsToml = toml::from_slice(&bytes).inspect_err(|e| {
            log::error!("Error parsing mods.toml/neoforge.mods.toml: {e}");
        }).ok()?;

        let Some(first) = mods_toml.mods.first() else {
            return None;
        };

        drop(file);

        let name = first.display_name.clone().unwrap_or_else(|| Arc::clone(&first.mod_id));

        let mut png_icon: Option<UniqueBytes> = None;
        if let Some(icon) = &first.logo_file && let Some(icon_file) = archive.by_name(&icon) {
            png_icon = load_icon(icon_file);
        }

        let authors = if let Some(authors) = create_authors_string(&first.authors) {
            authors.into()
        } else {
            "".into()
        };

        let mut version = format!("v{}", first.version.as_deref().unwrap_or("1"));
        if version.contains("${file.jarVersion}") {
            if let Some(manifest) = archive.by_name("META-INF/MANIFEST.MF") {
                if let Ok(manifest_bytes) = manifest.bytes() {
                    if let Ok(manifest_str) = str::from_utf8(&manifest_bytes) {
                        let manifest_map = crate::java_manifest::parse_java_manifest(manifest_str);
                        if let Some(impl_version) = manifest_map.get("Implementation-Version") {
                            version = version.replace("${file.jarVersion}", impl_version);
                        }
                    }
                }
            }
        }

        Some(Arc::new(ContentSummary {
            id: Some(first.mod_id.clone()),
            hash,
            name: Some(name),
            authors,
            version_str: version.into(),
            rich_description: None,
            png_icon,
            extra,
        }))
    }

    fn load_legacy_forge_mod<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
        let bytes = file.bytes().ok()?;

        let mc_mod_info: McModInfo = serde_json::from_slice(&bytes).inspect_err(|e| {
            log::error!("Error parsing mcmod.info: {e}");
        }).ok()?;

        let Some(first) = mc_mod_info.0.first() else {
            return None;
        };

        drop(file);

        let mut png_icon: Option<UniqueBytes> = None;
        if let Some(icon) = &first.logo_file && let Some(icon_file) = archive.by_name(&icon) {
            png_icon = load_icon(icon_file);
        }

        let authors = if let Some(authors) = &first.author_list {
            create_authors_string(authors.as_slice()).unwrap_or_default()
        } else {
            "".into()
        };

        let mut version = format!("v{}", first.version.as_deref().unwrap_or("1"));
        if version.contains("${file.jarVersion}") {
            if let Some(manifest) = archive.by_name("META-INF/MANIFEST.MF") {
                if let Ok(manifest_bytes) = manifest.bytes() {
                    if let Ok(manifest_str) = str::from_utf8(&manifest_bytes) {
                        let manifest_map = crate::java_manifest::parse_java_manifest(manifest_str);
                        if let Some(impl_version) = manifest_map.get("Implementation-Version") {
                            version = version.replace("${file.jarVersion}", impl_version);
                        }
                    }
                }
            }
        }

        Some(Arc::new(ContentSummary {
            id: Some(first.modid.clone()),
            hash,
            name: Some(first.name.clone()),
            authors: authors.into(),
            version_str: version.into(),
            rich_description: None,
            png_icon,
            extra: ContentType::LegacyForge,
        }))
    }

    fn load_modrinth_modpack<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
        let modrinth_index_json: ModrinthIndexJson = serde_json::from_slice(&file.bytes().ok()?).inspect_err(|e| {
            log::error!("Error parsing modrinth.index.json: {e}");
        }).ok()?;

        let mut overrides: IndexMap<SafePath, Arc<[u8]>> = IndexMap::new();

        for entry in archive.entries() {
            if entry.kind() != rc_zip_sync::rc_zip::EntryKind::File {
                continue;
            }
            let Some(path) = SafePath::new(&entry.name) else {
                continue;
            };

            let (prioritize, path) = if let Some(path) = path.strip_prefix("overrides") {
                (false, path)
            } else if let Some(path) = path.strip_prefix("client-overrides") {
                (true, path)
            } else {
                continue;
            };

            if !prioritize && overrides.contains_key(&path) {
                continue;
            }

            let Ok(data) = entry.bytes() else {
                continue;
            };
            overrides.insert(path, data.into());
        }

        let summaries = modrinth_index_json.files.par_iter().map(|download| {
            if let Some(env) = download.env {
                if env.client == ModrinthSideRequirement::Unsupported {
                    return None;
                }
            }

            let mut file_hash = [0u8; 20];
            let Ok(_) = hex::decode_to_slice(&*download.hashes.sha1, &mut file_hash) else {
                return None;
            };

            if let Some(cached) = self.by_hash.read().get(&file_hash).cloned() {
                return Some(cached);
            }

            let Some(path) = SafePath::new(&download.path) else {
                return None;
            };

            let file_hash_as_str = hex::encode(file_hash);

            let mut file = self.content_library_dir.join(&file_hash_as_str[..2]);
            file.push(&file_hash_as_str);
            if let Some(extension) = path.extension() {
                file.set_extension(extension);
            }

            if let Ok(mut file) = std::fs::File::open(file) {
                let summary = self.load_mod_summary(file_hash, &mut file, false);
                self.put(file_hash, summary.clone());
                return Some(summary);
            }

            self.parents_by_missing_child.write().entry(file_hash).or_default().insert(hash);

            None
        });
        let summaries: Vec<_> = summaries.collect();

        let mut png_icon = None;
        if let Some(icon) = archive.by_name("icon.png") {
            png_icon = load_icon(icon);
        }

        let authors = if let Some(authors) = modrinth_index_json.authors && let Some(authors) = create_authors_string(&authors) {
            authors.into()
        } else if let Some(author) = modrinth_index_json.author {
            format!("By {}", author.name()).into()
        } else {
            "".into()
        };

        Some(Arc::new(ContentSummary {
            id: None,
            hash,
            name: Some(modrinth_index_json.name),
            authors,
            version_str: create_version_string(&modrinth_index_json.version_id),
            rich_description: None,
            png_icon,
            extra: ContentType::ModrinthModpack {
                downloads: modrinth_index_json.files,
                summaries: summaries.into(),
                overrides: overrides.into_iter().collect(),
                dependencies: modrinth_index_json.dependencies,
            }
        }))
    }

    fn load_curseforge_modpack<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
        let manifest_json: CurseforgeModpackManifestJson = serde_json::from_slice(&file.bytes().ok()?).inspect_err(|e| {
            log::error!("Error parsing manifest.json: {e}");
        }).ok()?;

        let mut overrides: IndexMap<SafePath, Arc<[u8]>> = IndexMap::new();

        let overrides_prefix = manifest_json.overrides.as_deref().unwrap_or("overrides");

        for entry in archive.entries() {
            if entry.kind() != rc_zip_sync::rc_zip::EntryKind::File {
                continue;
            }
            let Some(path) = SafePath::new(&entry.name) else {
                continue;
            };

            let Some(path) = path.strip_prefix(overrides_prefix) else {
                continue;
            };

            let Ok(data) = entry.bytes() else {
                continue;
            };
            overrides.insert(path, data.into());
        }

        let summaries = manifest_json.files.par_iter().map(|file| {
            let Some(cached_info) = self.cached_curseforge_info.read().get(&file.file_id).cloned() else {
                self.parents_by_missing_curseforge_id.write().entry(file.file_id).or_default().insert(hash);
                return (None, None);
            };

            if let Some(cached) = self.by_hash.read().get(&cached_info.hash).cloned() {
                return (Some(cached), Some(cached_info));
            }

            let Some(path) = SafePath::new(&cached_info.filename) else {
                return (None, Some(cached_info));
            };

            let file_hash_as_str = hex::encode(cached_info.hash);

            let mut file = self.content_library_dir.join(&file_hash_as_str[..2]);
            file.push(&file_hash_as_str);
            if let Some(extension) = path.extension() {
                file.set_extension(extension);
            }

            if let Ok(mut file) = std::fs::File::open(file) {
                let summary = self.load_mod_summary(cached_info.hash, &mut file, false);
                self.put(cached_info.hash, summary.clone());
                return (Some(summary), Some(cached_info));
            }

            self.parents_by_missing_child.write().entry(cached_info.hash).or_default().insert(hash);

            (None, Some(cached_info))
        });
        let summaries: Vec<_> = summaries.collect();

        let mut png_icon = None;
        if let Some(icon) = archive.by_name("icon.png") {
            png_icon = load_icon(icon);
        }

        let authors = if let Some(author) = manifest_json.author {
            format!("By {}", author).into()
        } else {
            "".into()
        };

        Some(Arc::new(ContentSummary {
            id: None,
            hash,
            name: manifest_json.name,
            authors,
            version_str: create_version_string(&manifest_json.version),
            rich_description: None,
            png_icon,
            extra: ContentType::CurseforgeModpack {
                files: manifest_json.files,
                summaries: summaries.into(),
                overrides: overrides.into_iter().collect(),
                minecraft: manifest_json.minecraft,
            }
        }))
    }

    fn load_jarjar<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, _hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
        let bytes = file.bytes().ok()?;

        let metadata_json: JarJarMetadata = serde_json::from_slice(&bytes).inspect_err(|e| {
            log::error!("Error parsing jarjar/metadata.json: {e}");
        }).ok()?;

        drop(file);

        for child in &metadata_json.jars {
            let Some(child) = archive.by_name(&child.path) else {
                continue;
            };
            let Ok(child_bytes) = child.bytes() else {
                continue;
            };
            let child = self.get_bytes(&child_bytes);
            if !ContentSummary::is_unknown(&child) {
                return Some(child);
            }
        }

        None
    }

    fn load_from_java_manifest<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], _archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
        let bytes = file.bytes().ok()?;

        let manifest_str = str::from_utf8(&bytes).ok()?;

        let manifest_map = crate::java_manifest::parse_java_manifest(manifest_str);

        let name: Arc<str> = if let Some(module_name) = manifest_map.get("Automatic-Module-Name") {
            module_name.as_str().into()
        } else if let Some(impl_title) = manifest_map.get("Implementation-Title") {
            impl_title.as_str().into()
        } else if let Some(spec_title) = manifest_map.get("Specification-Title") {
            spec_title.as_str().into()
        } else {
            return None;
        };

        let author: Option<Arc<str>> = if let Some(impl_author) = manifest_map.get("Implementation-Vendor") {
            Some(impl_author.as_str().into())
        } else if let Some(spec_author) = manifest_map.get("Specification-Vendor") {
            Some(spec_author.as_str().into())
        } else {
            None
        };

        let version: Option<Arc<str>> = if let Some(impl_version) = manifest_map.get("Implementation-Version") {
            Some(Arc::from(format!("v{impl_version}")))
        } else if let Some(spec_version) = manifest_map.get("Specification-Version") {
            Some(Arc::from(format!("v{spec_version}")))
        } else {
            None
        };

        Some(Arc::new(ContentSummary {
            id: None,
            hash,
            name: Some(name.clone()),
            authors: author.unwrap_or_default(),
            version_str: version.unwrap_or_default(),
            rich_description: None,
            png_icon: None,
            extra: ContentType::JavaModule
        }))
    }

    fn load_from_pack_mcmeta<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
        let bytes = file.bytes().ok()?;

        let pack_mcmeta: PackMcmeta = serde_json::from_slice(&bytes).inspect_err(|e| {
            log::error!("Error parsing pack.mcmeta: {e}");
        }).ok()?;

        drop(file);

        let mut png_icon = None;
        if let Some(icon) = archive.by_name("pack.png") {
            png_icon = load_icon(icon);
        }

        Some(Arc::new(ContentSummary {
            id: None,
            hash,
            name: None,
            authors: "".into(),
            version_str: "".into(),
            rich_description: Some(Arc::new(pack_mcmeta.pack.description)),
            png_icon,
            extra: ContentType::ResourcePack
        }))
    }

    pub fn create_resource_pack(pack_mcmeta_bytes: &[u8], pack_png_bytes: Option<&[u8]>) -> Option<Arc<ContentSummary>> {
        let pack_mcmeta: PackMcmeta = serde_json::from_slice(&pack_mcmeta_bytes).inspect_err(|e| {
            log::error!("Error parsing pack.mcmeta: {e}");
        }).ok()?;

        let png_icon = pack_png_bytes.map(load_icon_bytes).flatten();

        Some(Arc::new(ContentSummary {
            id: None,
            hash: [0; 20],
            name: None,
            authors: "".into(),
            version_str: "".into(),
            rich_description: Some(Arc::new(pack_mcmeta.pack.description)),
            png_icon,
            extra: ContentType::ResourcePack
        }))
    }
}


fn load_icon<R: rc_zip_sync::HasCursor>(icon_file: rc_zip_sync::EntryHandle<R>) -> Option<UniqueBytes> {
    let Ok(icon_bytes) = icon_file.bytes() else {
        return None;
    };

    load_icon_bytes(&icon_bytes)
}

fn load_icon_bytes(icon_bytes: &[u8]) -> Option<UniqueBytes> {
    let Ok(mut image) = image::load_from_memory(&icon_bytes) else {
        return None;
    };
    let mut changed = false;

    if let Some(cropped) = crop_to_content(&image) {
        image = cropped;
        changed = true;
    }

    let width = image.width();
    let height = image.height();
    if width != 64 || height != 64 {
        let filter = if width > 64 || height > 64 {
            FilterType::Lanczos3
        } else {
            FilterType::Nearest
        };
        image = image.resize(64, 64, filter);
        changed = true;
    }

    if !changed {
        return Some(icon_bytes.into());
    }

    let mut modified_bytes = Vec::new();
    let mut cursor = Cursor::new(&mut modified_bytes);
    let encoder = image::codecs::png::PngEncoder::new_with_quality(&mut cursor, image::codecs::png::CompressionType::Best, Default::default());
    if image.write_with_encoder(encoder).is_err() {
        return None;
    }
    return Some(modified_bytes.into());
}

fn crop_to_content(image: &DynamicImage) -> Option<DynamicImage> {
    let width = image.width();
    let height = image.height();
    let mut min_x = 0;
    let mut max_x = width;
    let mut min_y = 0;
    let mut max_y = height;

    'crop_min_x: loop {
        if min_x >= max_x {
            return None;
        }
        for y in min_y..max_y {
            if image.get_pixel(min_x, y).0[3] != 0 {
                break 'crop_min_x;
            }
        }
        min_x += 1;
    }
    'crop_max_x: loop {
        if max_x <= min_x {
            return None;
        }
        for y in min_y..max_y {
            if image.get_pixel(max_x-1, y).0[3] != 0 {
                break 'crop_max_x;
            }
        }
        max_x -= 1;
    }
    'crop_min_y: loop {
        if min_y >= max_y {
            return None;
        }
        for x in min_x..max_x {
            if image.get_pixel(x, min_y).0[3] != 0 {
                break 'crop_min_y;
            }
        }
        min_y += 1;
    }
    'crop_max_y: loop {
        if max_y <= min_y {
            return None;
        }
        for x in min_x..max_x {
            if image.get_pixel(x, max_y-1).0[3] != 0 {
                break 'crop_max_y;
            }
        }
        max_y -= 1;
    }

    if min_x != 0 || max_x != width || min_y != 0 || max_y != height {
        Some(image.crop_imm(min_x, min_y, max_x - min_x, max_y - min_y))
    } else {
        None
    }
}

fn create_authors_string(authors: &[Person]) -> Option<String> {
    if !authors.is_empty() {
        let mut authors_string = "By ".to_owned();
        let mut first = true;
        for author in authors {
            if first {
                first = false;
            } else {
                authors_string.push_str(", ");
            }
            authors_string.push_str(author.name());
        }
        Some(authors_string.into())
    } else {
        None
    }
}

#[derive(Debug)]
pub struct ContentSources {
    by_first_byte: Box<[Vec<([u8; 19], ContentSource)>; 256]>,
    dirty: [u32; 8],
}

impl Default for ContentSources {
    fn default() -> Self {
        Self {
            by_first_byte: Box::new([const { Vec::new() }; 256]),
            dirty: [0; 8],
        }
    }
}

impl ContentSources {
    pub fn get(&self, hash: &[u8; 20]) -> Option<ContentSource> {
        let first_byte = hash[0];
        let values = &self.by_first_byte.get(first_byte as usize)?;
        let index = values.binary_search_by_key(&&hash[1..], |v| &v.0).ok()?;
        Some(values[index].1.clone())
    }

    pub fn set(&mut self, hash: &[u8; 20], value: ContentSource) {
        let first_byte = hash[0];
        let values = &mut self.by_first_byte[first_byte as usize];
        match values.binary_search_by_key(&&hash[1..], |v| &v.0) {
            Ok(existing) => {
                let old_source = &mut values[existing].1;
                let skip = match old_source {
                    ContentSource::ModrinthProject { project_id: _ } => {
                        old_source == &value || value == ContentSource::ModrinthUnknown
                    },
                    _ => old_source == &value
                };
                if !skip {
                    values[existing].1 = value;
                    self.dirty[(first_byte >> 5) as usize] |= 1 << (first_byte & 0b11111);
                }
            },
            Err(new) => {
                values.insert(new, (hash[1..].try_into().unwrap(), value));
                self.dirty[(first_byte >> 5) as usize] |= 1 << (first_byte & 0b11111);
            },
        }
    }

    pub fn write_all_to_file(&self, dir: &Path) {
        _ = std::fs::create_dir_all(dir);

        for (first_byte, values) in self.by_first_byte.iter().enumerate() {
            if !values.is_empty() {
                let path = dir.join(hex::encode(&[first_byte as u8]));

                let mut data = Vec::new();
                for (key, source) in values {
                    Self::write(&mut data, key, source);
                }
                _ = crate::write_safe(&path, &data);
            }
        }
    }

    pub fn write_dirty_to_folder(&mut self, dir: &Path) {
        let dirty = std::mem::take(&mut self.dirty);
        for (int_index, mut int) in dirty.into_iter().enumerate() {
            while int != 0 {
                let index = int.trailing_zeros();
                debug_assert!(index as usize + int_index as usize * 32 <= u8::MAX as usize);
                self.write_to_file(index as u8 + int_index as u8 * 32, dir);
                int &= !(1 << index);
            }
        }
    }

    pub fn write_to_file(&self, first_byte: u8, dir: &Path) {
        let path = dir.join(hex::encode(&[first_byte]));

        let mut data = Vec::new();
        let values = &self.by_first_byte[first_byte as usize];
        for (key, source) in values {
            Self::write(&mut data, key, source);
        }

        _ = crate::write_safe(&path, &data);
    }

    fn write(data: &mut Vec<u8>, key: &[u8], source: &ContentSource) {
        data.extend_from_slice(key);
        match source {
            ContentSource::Manual => {
                data.push(0_u8);
                data.push(0_u8);
            },
            ContentSource::ModrinthUnknown => {
                data.push(1_u8);
                data.push(0_u8);
            },
            ContentSource::ModrinthProject { project_id } => {
                data.push(2_u8);
                if project_id.len() > 127 {
                    panic!("modrinth project id was unexpectedly big: {:?}", &project_id);
                }
                data.push(project_id.len() as u8);
                data.extend_from_slice(project_id.as_bytes());
            },
            ContentSource::CurseforgeProject { project_id } => {
                data.push(3_u8);
                data.push(4_u8);
                data.extend_from_slice(&project_id.to_le_bytes());
            }
        }
    }

    fn from_legacy(legacy: LegacyDeserializedContentSources) -> Self {
        let mut by_first_byte = Box::new([const { Vec::new() }; 256]);

        for (key, source) in legacy.0 {
            let first_byte = key[0];
            let source = match source {
                LegacyContentSource::Manual => ContentSource::Manual,
                LegacyContentSource::Modrinth => ContentSource::ModrinthUnknown,
            };
            by_first_byte[first_byte as usize].push((key[1..].try_into().unwrap(), source));
        }

        for vec in &mut *by_first_byte {
            vec.sort_by_key(|(k, _)| *k)
        }

        Self {
            by_first_byte,
            dirty: [0; 8],
        }
    }

    fn load_all(sources_dir: &Path) -> std::io::Result<ContentSources> {
        let read_dir = std::fs::read_dir(sources_dir)?;

        let mut by_first_byte = Box::new([const { Vec::<([u8; 19], ContentSource)>::new() }; 256]);

        for entry in read_dir {
            let Ok(entry) = entry else {
                continue;
            };

            let path = entry.path();
            let filename = entry.file_name();

            let Some(filename) = filename.to_str() else {
                continue;
            };

            if filename.len() != 2 {
                continue;
            }

            let mut first_byte = [0_u8; 1];
            let Ok(_) = hex::decode_to_slice(filename, &mut first_byte) else {
                continue;
            };

            let Ok(data) = std::fs::read(path) else {
                continue;
            };

            let mut cursor = Cursor::new(data);
            let values = &mut by_first_byte[first_byte[0] as usize];

            let mut key_buf = [0_u8; 19];
            let mut type_and_size_buf = [0_u8; 2];
            loop {
                if cursor.read_exact(&mut key_buf).is_err() {
                    break;
                }
                if cursor.read_exact(&mut type_and_size_buf).is_err() {
                    break;
                }

                let source = match type_and_size_buf[0] {
                    0 => {
                        debug_assert_eq!(type_and_size_buf[1], 0);
                        ContentSource::Manual
                    },
                    1 => {
                        debug_assert_eq!(type_and_size_buf[1], 0);
                        ContentSource::ModrinthUnknown
                    },
                    2 => {
                        let mut project_buf = vec![0_u8; type_and_size_buf[1] as usize];

                        if cursor.read_exact(&mut project_buf).is_err() {
                            break;
                        }

                        let Ok(project_id) = str::from_utf8(&project_buf) else {
                            continue;
                        };

                        ContentSource::ModrinthProject { project_id: project_id.into() }
                    },
                    3 => {
                        debug_assert_eq!(type_and_size_buf[1], 4);
                        let mut id_buf = [0_u8; 4];

                        if cursor.read_exact(&mut id_buf).is_err() {
                            break;
                        }

                        ContentSource::CurseforgeProject { project_id: u32::from_le_bytes(id_buf) }
                    },
                    _ => {
                        cursor.consume(type_and_size_buf[1] as usize);
                        continue;
                    }
                };

                match values.binary_search_by_key(&key_buf, |v| v.0) {
                    Ok(existing) => {
                        values[existing] = (key_buf, source);
                    },
                    Err(new) => {
                        values.insert(new, (key_buf, source));
                    },
                }
            }
        }

        Ok(Self {
            by_first_byte,
            dirty: [0; 8]
        })
    }
}

fn create_version_string(ver: &str) -> Arc<str> {
    let ver = ver.trim_ascii();
    if ver.starts_with('v') {
        ver.into()
    } else {
        format!("v{ver}").into()
    }
}

#[serde_as]
#[derive(Deserialize)]
struct LegacyDeserializedContentSources(
    #[serde_as(as = "FxHashMap<DeserializeAsHex, _>")]
    FxHashMap<[u8; 20], LegacyContentSource>
);

struct DeserializeAsHex {}

impl<'de> DeserializeAs<'de, [u8; 20]> for DeserializeAsHex {
    fn deserialize_as<D>(deserializer: D) -> Result<[u8; 20], D::Error>
    where
        D: serde::Deserializer<'de> {
        hex::serde::deserialize(deserializer)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LegacyContentSource {
    Manual,
    Modrinth,
}
