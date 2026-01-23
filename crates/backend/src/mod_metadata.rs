use std::{
    io::{BufRead, Cursor, Read, Write}, path::{Path, PathBuf}, sync::Arc
};

use bridge::{instance::{AtomicContentUpdateStatus, ContentUpdateStatus, ContentType, ContentSummary}, safe_path::SafePath};
use image::imageops::FilterType;
use indexmap::IndexMap;
use parking_lot::{RwLock, RwLockReadGuard};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rc_zip_sync::EntryHandle;
use rustc_hash::{FxHashMap, FxHashSet};
use schema::{content::ContentSource, fabric_mod::{FabricModJson, Icon, Person}, forge_mod::{JarJarMetadata, ModsToml}, modrinth::{ModrinthFile, ModrinthSideRequirement}, mrpack::ModrinthIndexJson, resourcepack::PackMcmeta};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DeserializeAs};
use sha1::{Digest, Sha1};

#[derive(Clone)]
pub enum ModUpdateAction {
    ErrorNotFound,
    ErrorInvalidHash,
    AlreadyUpToDate,
    ManualInstall,
    Modrinth {
        file: ModrinthFile,
        project_id: Arc<str>,
    },
}

impl ModUpdateAction {
    pub fn to_status(&self) -> ContentUpdateStatus {
        match self {
            ModUpdateAction::ErrorNotFound => ContentUpdateStatus::ErrorNotFound,
            ModUpdateAction::ErrorInvalidHash => ContentUpdateStatus::ErrorInvalidHash,
            ModUpdateAction::AlreadyUpToDate => ContentUpdateStatus::AlreadyUpToDate,
            ModUpdateAction::ManualInstall => ContentUpdateStatus::ManualInstall,
            ModUpdateAction::Modrinth { .. } => ContentUpdateStatus::Modrinth,
        }
    }
}

pub struct ModMetadataManager {
    content_library_dir: Arc<Path>,
    sources_dir: PathBuf,
    by_hash: RwLock<FxHashMap<[u8; 20], Option<Arc<ContentSummary>>>>,
    content_sources: RwLock<ContentSources>,
    parents_by_missing_child: RwLock<FxHashMap<[u8; 20], Vec<[u8; 20]>>>,
    pub updates: RwLock<FxHashMap<[u8; 20], ModUpdateAction>>,
}

impl ModMetadataManager {
    pub fn load(content_meta_dir: Arc<Path>, content_library_dir: Arc<Path>) -> Self {
        let legacy_sources_json = content_meta_dir.join("sources.json");
        let sources_dir = content_meta_dir.join("sources");

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

        Self {
            content_library_dir,
            sources_dir,
            by_hash: Default::default(),
            content_sources: RwLock::new(content_sources),
            parents_by_missing_child: Default::default(),
            updates: Default::default(),
        }
    }

    pub fn read_content_sources(&self) -> RwLockReadGuard<'_, ContentSources> {
        self.content_sources.read()
    }

    pub fn set_content_sources(&self, sources: impl Iterator<Item = ([u8; 20], ContentSource)>) {
        let mut content_sources = self.content_sources.write();

        let mut changed = FxHashSet::default();
        for (hash, source) in sources {
            if content_sources.set(&hash, source) {
                changed.insert(hash[0]);
            }
        }

        for changed in changed {
            content_sources.write_to_file(changed, &self.sources_dir);
        }
    }

    pub fn get_path(self: &Arc<Self>, path: &Path) -> Option<Arc<ContentSummary>> {
        let mut file = std::fs::File::open(path).ok()?;
        self.get_file(&mut file)
    }

    pub fn get_file(self: &Arc<Self>, file: &mut std::fs::File) -> Option<Arc<ContentSummary>> {
        let mut hasher = Sha1::new();
        let _ = std::io::copy(file, &mut hasher).ok()?;
        let actual_hash: [u8; 20] = hasher.finalize().into();

        if let Some(summary) = self.by_hash.read().get(&actual_hash) {
            return summary.clone();
        }

        let summary = self.load_mod_summary(actual_hash, file, true);

        self.put(actual_hash, summary.clone());

        summary
    }

    pub fn get_bytes(self: &Arc<Self>, bytes: &[u8]) -> Option<Arc<ContentSummary>> {
        let mut hasher = Sha1::new();
        hasher.write_all(bytes).ok()?;
        let actual_hash: [u8; 20] = hasher.finalize().into();

        if let Some(summary) = self.by_hash.read().get(&actual_hash) {
            return summary.clone();
        }

        let summary = self.load_mod_summary(actual_hash, &bytes, true);

        self.put(actual_hash, summary.clone());

        summary
    }

    fn put(self: &Arc<Self>, hash: [u8; 20], summary: Option<Arc<ContentSummary>>) {
        self.by_hash.write().insert(hash, summary.clone());

        if let Some(parents) = self.parents_by_missing_child.write().remove(&hash) {
            // Remove cached summary of parent, so it can be recalculated next time it is requested
            let mut by_hash = self.by_hash.write();
            for parent in parents {
                by_hash.remove(&parent);
            }
        }
    }

    fn load_mod_summary<R: rc_zip_sync::ReadZip>(self: &Arc<Self>, hash: [u8; 20], file: &R, allow_children: bool) -> Option<Arc<ContentSummary>> {
        let archive = file.read_zip().ok()?;

        if let Some(file) = archive.by_name("fabric.mod.json") {
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
        } else {
            None
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
            eprintln!("Error parsing fabric.mod.json: {e}");
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

        let mut png_icon: Option<Arc<[u8]>> = None;
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
            version_str: format!("v{}", fabric_mod_json.version).into(),
            png_icon,
            update_status: Arc::new(AtomicContentUpdateStatus::new(ContentUpdateStatus::Unknown)),
            extra: ContentType::Fabric
        }))
    }

    fn load_forge_mod<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>, extra: ContentType) -> Option<Arc<ContentSummary>> {
        let bytes = file.bytes().ok()?;

        let mods_toml: ModsToml = toml::from_slice(&bytes).inspect_err(|e| {
            eprintln!("Error parsing mods.toml/neoforge.mods.toml: {e}");
        }).ok()?;

        let Some(first) = mods_toml.mods.first() else {
            return None;
        };

        drop(file);

        let name = first.display_name.clone().unwrap_or_else(|| Arc::clone(&first.mod_id));

        let mut png_icon: Option<Arc<[u8]>> = None;
        if let Some(icon) = &first.logo_file && let Some(icon_file) = archive.by_name(&icon) {
            png_icon = load_icon(icon_file);
        }

        let authors = if let Some(authors) = &first.authors {
            format!("By {authors}").into()
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
            png_icon,
            update_status: Arc::new(AtomicContentUpdateStatus::new(ContentUpdateStatus::Unknown)),
            extra,
        }))
    }

    fn load_modrinth_modpack<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
        let modrinth_index_json: ModrinthIndexJson = serde_json::from_slice(&file.bytes().ok()?).inspect_err(|e| {
            eprintln!("Error parsing modrinth.index.json: {e}");
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
                return cached;
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
                return summary;
            }

            self.parents_by_missing_child.write().entry(file_hash).or_default().push(hash);

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
            version_str: format!("v{}", modrinth_index_json.version_id).into(),
            png_icon,
            update_status: Arc::new(AtomicContentUpdateStatus::new(ContentUpdateStatus::Unknown)),
            extra: ContentType::ModrinthModpack {
                downloads: modrinth_index_json.files,
                summaries: summaries.into(),
                overrides: overrides.into_iter().collect(),
            }
        }))
    }

    fn load_jarjar<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
        let bytes = file.bytes().ok()?;

        let metadata_json: JarJarMetadata = serde_json::from_slice(&bytes).inspect_err(|e| {
            eprintln!("Error parsing jarjar/metadata.json: {e}");
        }).ok()?;

        drop(file);

        for child in &metadata_json.jars {
            let Some(child) = archive.by_name(&child.path) else {
                continue;
            };
            let Ok(child_bytes) = child.bytes() else {
                continue;
            };
            if let Some(child) = self.get_bytes(&child_bytes) {
                return Some(child);
            }
        }

        None
    }

    fn load_from_java_manifest<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
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
            png_icon: None,
            update_status: Arc::new(AtomicContentUpdateStatus::new(ContentUpdateStatus::Unknown)),
            extra: ContentType::JavaModule
        }))
    }

    fn load_from_pack_mcmeta<R: rc_zip_sync::HasCursor>(self: &Arc<Self>, hash: [u8; 20], archive: &rc_zip_sync::ArchiveHandle<R>, file: EntryHandle<'_, R>) -> Option<Arc<ContentSummary>> {
        let bytes = file.bytes().ok()?;

        let pack_mcmeta: PackMcmeta = serde_json::from_slice(&bytes).inspect_err(|e| {
            eprintln!("Error parsing jarjar/metadata.json: {e}");
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
            version_str: pack_mcmeta.pack.description,
            png_icon,
            update_status: Arc::new(AtomicContentUpdateStatus::new(ContentUpdateStatus::Unknown)),
            extra: ContentType::ResourcePack
        }))
    }
}

fn load_icon<R: rc_zip_sync::HasCursor>(icon_file: rc_zip_sync::EntryHandle<R>) -> Option<Arc<[u8]>> {
    let Ok(mut icon_bytes) = icon_file.bytes() else {
        return None;
    };

    let Ok(image) = image::load_from_memory(&icon_bytes) else {
        return None;
    };

    let width = image.width();
    let height = image.height();
    if width != 64 || height != 64 {
        let filter = if width > 64 || height > 64 {
            FilterType::Lanczos3
        } else {
            FilterType::Nearest
        };
        let resized = image.resize_exact(64, 64, filter);

        icon_bytes.clear();
        let mut cursor = Cursor::new(&mut icon_bytes);
        if resized.write_to(&mut cursor, image::ImageFormat::Png).is_err() {
            return None;
        }
    }

    Some(icon_bytes.into())
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
}

impl Default for ContentSources {
    fn default() -> Self {
        Self {
            by_first_byte: Box::new([const { Vec::new() }; 256])
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

    pub fn set(&mut self, hash: &[u8; 20], value: ContentSource) -> bool {
        let first_byte = hash[0];
        let values = &mut self.by_first_byte[first_byte as usize];
        match values.binary_search_by_key(&&hash[1..], |v| &v.0) {
            Ok(existing) => {
                let old_source = &mut values[existing].1;
                let skip = match old_source {
                    ContentSource::Manual => value == ContentSource::Manual,
                    ContentSource::ModrinthUnknown => value == ContentSource::ModrinthUnknown,
                    ContentSource::ModrinthProject { project: _ } => {
                        old_source == &value || value == ContentSource::ModrinthUnknown
                    },
                };
                if skip {
                    return false;
                } else {
                    values[existing].1 = value;
                    return true;
                }
            },
            Err(new) => {
                values.insert(new, (hash[1..].try_into().unwrap(), value));
                return true
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
            ContentSource::ModrinthProject { project } => {
                data.push(2_u8);
                if project.len() > 127 {
                    panic!("modrinth project id was unexpectedly big: {:?}", &project);
                }
                data.push(project.len() as u8);
                data.extend_from_slice(project.as_bytes());
            },
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
            by_first_byte
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

                        ContentSource::ModrinthProject { project: project_id.into() }
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
            by_first_byte
        })
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
