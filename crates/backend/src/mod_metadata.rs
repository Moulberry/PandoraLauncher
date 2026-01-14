use std::{
    io::{Cursor, Write},
    path::{Path, PathBuf},
    sync::Arc,
};

use bridge::{
    instance::{AtomicContentUpdateStatus, ContentUpdateStatus, LoaderSpecificModSummary, ModSummary},
    safe_path::SafePath,
};
use image::imageops::FilterType;
use indexmap::IndexMap;
use parking_lot::{RwLock, RwLockReadGuard};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use rc_zip_sync::EntryHandle;
use rustc_hash::FxHashMap;
use schema::{
    content::ContentSource,
    fabric_mod::{FabricModJson, Icon, Person},
    forge_mod::{JarJarMetadata, ModsToml},
    modrinth::{ModrinthFile, ModrinthSideRequirement},
    mrpack::ModrinthIndexJson,
};
use serde::{Deserialize, Serialize};
use serde_with::{DeserializeAs, SerializeAs, serde_as};
use sha1::{Digest, Sha1};

#[derive(Clone)]
pub enum ModUpdateAction {
    ErrorNotFound,
    ErrorInvalidHash,
    AlreadyUpToDate,
    ManualInstall,
    Modrinth(ModrinthFile),
}

impl ModUpdateAction {
    pub fn to_status(&self) -> ContentUpdateStatus {
        match self {
            ModUpdateAction::ErrorNotFound => ContentUpdateStatus::ErrorNotFound,
            ModUpdateAction::ErrorInvalidHash => ContentUpdateStatus::ErrorInvalidHash,
            ModUpdateAction::AlreadyUpToDate => ContentUpdateStatus::AlreadyUpToDate,
            ModUpdateAction::ManualInstall => ContentUpdateStatus::ManualInstall,
            ModUpdateAction::Modrinth(_) => ContentUpdateStatus::Modrinth,
        }
    }
}

pub struct ModMetadataManager {
    content_library_dir: Arc<Path>,
    sources_json: PathBuf,
    by_hash: RwLock<FxHashMap<[u8; 20], Option<Arc<ModSummary>>>>,
    content_sources: RwLock<FxHashMap<[u8; 20], ContentSource>>,
    parents_by_missing_child: RwLock<FxHashMap<[u8; 20], Vec<[u8; 20]>>>,
    pub updates: RwLock<FxHashMap<[u8; 20], ModUpdateAction>>,
}

impl ModMetadataManager {
    pub fn load(content_meta_dir: Arc<Path>, content_library_dir: Arc<Path>) -> Self {
        let sources_json = content_meta_dir.join("sources.json");

        let content_sources = if let Ok(data) = std::fs::read(&sources_json) {
            let content_sources = serde_json::from_slice(&data);
            content_sources.map(|v: DeserializedContentSources| v.0).unwrap_or_default()
        } else {
            Default::default()
        };

        Self {
            content_library_dir,
            sources_json,
            by_hash: Default::default(),
            content_sources: RwLock::new(content_sources),
            parents_by_missing_child: Default::default(),
            updates: Default::default(),
        }
    }

    pub fn read_content_sources(&self) -> RwLockReadGuard<'_, FxHashMap<[u8; 20], ContentSource>> {
        self.content_sources.read()
    }

    pub fn set_content_sources(&self, sources: impl Iterator<Item = ([u8; 20], ContentSource)>) {
        let mut content_sources = self.content_sources.write();

        let mut changed = false;
        for (hash, source) in sources {
            let old = content_sources.insert(hash, source);
            if old != Some(source) {
                changed = true;
            }
        }

        if changed {
            let serialized = SerializedContentSources(&content_sources);
            if let Ok(content) = serde_json::to_vec(&serialized) {
                let _ = crate::write_safe(&self.sources_json, &content);
            }
        }
    }

    pub fn get_path(self: &Arc<Self>, path: &Path) -> Option<Arc<ModSummary>> {
        let mut file = std::fs::File::open(path).ok()?;
        self.get_file(&mut file)
    }

    pub fn get_file(self: &Arc<Self>, file: &mut std::fs::File) -> Option<Arc<ModSummary>> {
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

    pub fn get_bytes(self: &Arc<Self>, bytes: &[u8]) -> Option<Arc<ModSummary>> {
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

    fn put(self: &Arc<Self>, hash: [u8; 20], summary: Option<Arc<ModSummary>>) {
        self.by_hash.write().insert(hash, summary.clone());

        if let Some(parents) = self.parents_by_missing_child.write().remove(&hash) {
            // Remove cached summary of parent, so it can be recalculated next time it is requested
            let mut by_hash = self.by_hash.write();
            for parent in parents {
                by_hash.remove(&parent);
            }
        }
    }

    fn load_mod_summary<R: rc_zip_sync::ReadZip>(
        self: &Arc<Self>,
        hash: [u8; 20],
        file: &R,
        allow_children: bool,
    ) -> Option<Arc<ModSummary>> {
        let archive = file.read_zip().ok()?;

        if let Some(file) = archive.by_name("fabric.mod.json") {
            Self::load_fabric_mod(hash, &archive, file)
        } else if let Some(file) = archive.by_name("META-INF/mods.toml") {
            Self::load_forge_mod(hash, &archive, file)
        } else if let Some(file) = archive.by_name("META-INF/neoforge.mods.toml") {
            Self::load_forge_mod(hash, &archive, file)
        } else if let Some(file) = archive.by_name("META-INF/jarjar/metadata.json") {
            Self::load_jarjar(self, hash, &archive, file)
        } else if let Some(file) = archive.by_name("META-INF/MANIFEST.MF") {
            Self::load_from_java_manifest(self, hash, &archive, file)
        } else if allow_children && let Some(file) = archive.by_name("modrinth.index.json") {
            self.load_modrinth_modpack(hash, &archive, file)
        } else {
            None
        }
    }

    fn load_fabric_mod<R: rc_zip_sync::HasCursor>(
        hash: [u8; 20],
        archive: &rc_zip_sync::ArchiveHandle<R>,
        file: EntryHandle<'_, R>,
    ) -> Option<Arc<ModSummary>> {
        let mut bytes = file.bytes().ok()?;

        // Some mods violate the JSON spec by using raw newline characters inside strings (e.g. BetterGrassify)
        for byte in bytes.iter_mut() {
            if *byte == '\n' as u8 {
                *byte = ' ' as u8;
            }
        }

        let fabric_mod_json: FabricModJson = serde_json::from_slice(&bytes)
            .inspect_err(|e| {
                eprintln!("Error parsing fabric.mod.json: {e}");
            })
            .ok()?;

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
        if let Some(icon) = icon
            && let Some(icon_file) = archive.by_name(&icon)
        {
            png_icon = load_icon(icon_file);
        }

        let authors = if let Some(authors) = fabric_mod_json.authors
            && let Some(authors) = create_authors_string(&authors)
        {
            authors.into()
        } else {
            "".into()
        };

        let mut lowercase_search_key = fabric_mod_json.id.to_lowercase();
        lowercase_search_key.push_str("$$");
        lowercase_search_key.push_str(&name.to_lowercase());

        Some(Arc::new(ModSummary {
            id: fabric_mod_json.id,
            hash,
            name,
            lowercase_search_key: lowercase_search_key.into(),
            authors,
            version_str: format!("v{}", fabric_mod_json.version).into(),
            png_icon,
            update_status: Arc::new(AtomicContentUpdateStatus::new(ContentUpdateStatus::Unknown)),
            extra: LoaderSpecificModSummary::Fabric,
        }))
    }

    fn load_forge_mod<R: rc_zip_sync::HasCursor>(
        hash: [u8; 20],
        archive: &rc_zip_sync::ArchiveHandle<R>,
        file: EntryHandle<'_, R>,
    ) -> Option<Arc<ModSummary>> {
        let bytes = file.bytes().ok()?;

        let mods_toml: ModsToml = toml::from_slice(&bytes)
            .inspect_err(|e| {
                eprintln!("Error parsing mods.toml/neoforge.mods.toml: {e}");
            })
            .ok()?;

        let Some(first) = mods_toml.mods.first() else {
            return None;
        };

        drop(file);

        let name = first.display_name.clone().unwrap_or_else(|| Arc::clone(&first.mod_id));

        let mut png_icon: Option<Arc<[u8]>> = None;
        if let Some(icon) = &first.logo_file
            && let Some(icon_file) = archive.by_name(&icon)
        {
            png_icon = load_icon(icon_file);
        }

        let authors = if let Some(authors) = &first.authors {
            format!("By {authors}").into()
        } else {
            "".into()
        };

        let mut lowercase_search_key = first.mod_id.to_lowercase();
        lowercase_search_key.push_str("$$");
        lowercase_search_key.push_str(&name.to_lowercase());

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

        Some(Arc::new(ModSummary {
            id: first.mod_id.clone(),
            hash,
            name,
            lowercase_search_key: lowercase_search_key.into(),
            authors,
            version_str: version.into(),
            png_icon,
            update_status: Arc::new(AtomicContentUpdateStatus::new(ContentUpdateStatus::Unknown)),
            extra: LoaderSpecificModSummary::Forge,
        }))
    }

    fn load_modrinth_modpack<R: rc_zip_sync::HasCursor>(
        self: &Arc<Self>,
        hash: [u8; 20],
        archive: &rc_zip_sync::ArchiveHandle<R>,
        file: EntryHandle<'_, R>,
    ) -> Option<Arc<ModSummary>> {
        let modrinth_index_json: ModrinthIndexJson = serde_json::from_slice(&file.bytes().ok()?)
            .inspect_err(|e| {
                eprintln!("Error parsing modrinth.index.json: {e}");
            })
            .ok()?;

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

        let lowercase_search_key = modrinth_index_json.name.to_lowercase();

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

        let authors = if let Some(authors) = modrinth_index_json.authors
            && let Some(authors) = create_authors_string(&authors)
        {
            authors.into()
        } else if let Some(author) = modrinth_index_json.author {
            format!("By {}", author.name()).into()
        } else {
            "".into()
        };

        Some(Arc::new(ModSummary {
            id: "".into(),
            hash,
            name: modrinth_index_json.name,
            lowercase_search_key: lowercase_search_key.into(),
            authors,
            version_str: format!("v{}", modrinth_index_json.version_id).into(),
            png_icon,
            update_status: Arc::new(AtomicContentUpdateStatus::new(ContentUpdateStatus::Unknown)),
            extra: LoaderSpecificModSummary::ModrinthModpack {
                downloads: modrinth_index_json.files,
                summaries: summaries.into(),
                overrides: overrides.into_iter().collect(),
            },
        }))
    }

    #[expect(unused_variables)]
    fn load_jarjar<R: rc_zip_sync::HasCursor>(
        self: &Arc<Self>,
        hash: [u8; 20],
        archive: &rc_zip_sync::ArchiveHandle<R>,
        file: EntryHandle<'_, R>,
    ) -> Option<Arc<ModSummary>> {
        let bytes = file.bytes().ok()?;

        let metadata_json: JarJarMetadata = serde_json::from_slice(&bytes)
            .inspect_err(|e| {
                eprintln!("Error parsing jarjar/metadata.json: {e}");
            })
            .ok()?;

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

    #[expect(unused_variables)]
    fn load_from_java_manifest<R: rc_zip_sync::HasCursor>(
        self: &Arc<Self>,
        hash: [u8; 20],
        archive: &rc_zip_sync::ArchiveHandle<R>,
        file: EntryHandle<'_, R>,
    ) -> Option<Arc<ModSummary>> {
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

        Some(Arc::new(ModSummary {
            id: name.clone(),
            hash,
            name: name.clone(),
            lowercase_search_key: name.clone(),
            authors: author.unwrap_or_default(),
            version_str: version.unwrap_or_default(),
            png_icon: None,
            update_status: Arc::new(AtomicContentUpdateStatus::new(ContentUpdateStatus::Unknown)),
            extra: LoaderSpecificModSummary::JavaModule,
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

#[serde_as]
#[derive(Serialize)]
struct SerializedContentSources<'a>(
    #[serde_as(as = "FxHashMap<SerializeAsHex, _>")] &'a FxHashMap<[u8; 20], ContentSource>,
);

struct SerializeAsHex {}

impl SerializeAs<[u8; 20]> for SerializeAsHex {
    fn serialize_as<S>(source: &[u8; 20], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        hex::serde::serialize(source, serializer)
    }
}

#[serde_as]
#[derive(Deserialize)]
struct DeserializedContentSources(
    #[serde_as(as = "FxHashMap<DeserializeAsHex, _>")] FxHashMap<[u8; 20], ContentSource>,
);

struct DeserializeAsHex {}

impl<'de> DeserializeAs<'de, [u8; 20]> for DeserializeAsHex {
    fn deserialize_as<D>(deserializer: D) -> Result<[u8; 20], D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        hex::serde::deserialize(deserializer)
    }
}
