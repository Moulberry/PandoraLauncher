use std::{
    borrow::Cow, fmt::Debug, path::{Path, PathBuf}, sync::Arc
};

use reqwest::RequestBuilder;
use schema::{
    assets_index::AssetsIndex,
    curseforge::{
        CURSEFORGE_SEARCH_URL, CurseforgeFingerprintRequest, CurseforgeFingerprintResponse,
        CurseforgeGetFilesRequest, CurseforgeGetModFilesRequest, CurseforgeGetModFilesResult,
        CurseforgeSearchRequest, CurseforgeSearchResult, MINECRAFT_GAME_ID
    },
    fabric_launch::FabricLaunch,
    fabric_loader_manifest::{FABRIC_LOADER_MANIFEST_URL, FabricLoaderManifest},
    forge::{ForgeMavenManifest, NeoforgeMavenManifest, VersionFragment},
    java_runtime_component::JavaRuntimeComponentManifest,
    java_runtimes::{JAVA_RUNTIMES_URL, JavaRuntimes},
    maven::MavenMetadataXml,
    modrinth::{
        MODRINTH_PROJECT_URL, MODRINTH_SEARCH_URL, ModrinthLoader, ModrinthProjectRequest,
        ModrinthProjectResult, ModrinthProjectVersion, ModrinthProjectVersionsRequest,
        ModrinthProjectVersionsResult, ModrinthProjectsRequest, ModrinthProjectsResponse,
        ModrinthSearchRequest, ModrinthSearchResult, ModrinthVersionFileUpdateResult,
        ModrinthVersionsFromHashesRequest, ModrinthVersionsFromHashesResponse
    },
    version::MinecraftVersion,
    version_manifest::{MOJANG_VERSION_MANIFEST_URL, MinecraftVersionLink, MinecraftVersionManifest}
};
use serde::Serialize;
use ustr::Ustr;

use crate::metadata::manager::{MetaLoadError, MetaLoadStateWrapper, MetadataManager, MetadataManagerStates};

pub trait MetadataItem: Debug {
    type T: Send + Sync + 'static;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder;
    fn expires(&self) -> bool;
    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T>;
    fn post_process_download(bytes: &[u8]) -> Result<Cow<'_, [u8]>, MetaLoadError> {
        Ok(Cow::Borrowed(bytes))
    }
    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError>;
    fn cache_file(&self, _metadata_manager: &MetadataManager) -> Option<impl AsRef<Path> + Send + Sync + 'static> {
        None::<PathBuf>
    }
    fn data_hash(&self) -> Option<Ustr> {
        None
    }
}

#[derive(Debug)]
pub struct MinecraftVersionManifestMetadataItem;

impl MetadataItem for MinecraftVersionManifestMetadataItem {
    type T = MinecraftVersionManifest;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get(MOJANG_VERSION_MANIFEST_URL)
    }

    fn expires(&self) -> bool {
        true
    }

    fn cache_file(&self, metadata_manager: &MetadataManager) -> Option<impl AsRef<Path> + Send + Sync + 'static> {
        Some(Arc::clone(&metadata_manager.version_manifest_cache))
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.minecraft_version_manifest.clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct MojangJavaRuntimesMetadataItem;

impl MetadataItem for MojangJavaRuntimesMetadataItem {
    type T = JavaRuntimes;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get(JAVA_RUNTIMES_URL)
    }

    fn expires(&self) -> bool {
        true
    }

    fn cache_file(&self, metadata_manager: &MetadataManager) -> Option<impl AsRef<Path> + Send + Sync + 'static> {
        Some(Arc::clone(&metadata_manager.mojang_java_runtimes_cache))
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.mojang_java_runtimes.clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct MinecraftVersionMetadataItem<'v>(pub &'v MinecraftVersionLink);

impl<'v> MetadataItem for MinecraftVersionMetadataItem<'v> {
    type T = MinecraftVersion;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get(self.0.url.as_str())
    }

    fn expires(&self) -> bool {
        false
    }

    fn data_hash(&self) -> Option<Ustr> {
        Some(self.0.sha1)
    }

    fn cache_file(&self, metadata_manager: &MetadataManager) -> Option<impl AsRef<Path> + Send + Sync + 'static> {
        if !crate::is_single_component_path_str(&self.0.sha1) {
            panic!("Invalid sha1 {}, possible directory traversal attack?", self.0.sha1);
        }
        let mut path = metadata_manager.metadata_cache.join("version_info");
        path.push(self.0.sha1.as_str());
        Some(path)
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.version_info.entry(self.0.url).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct AssetsIndexMetadataItem {
    pub url: Ustr,
    pub cache: Arc<Path>,
    pub hash: Ustr,
}

impl MetadataItem for AssetsIndexMetadataItem {
    type T = AssetsIndex;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get(self.url.as_str())
    }

    fn expires(&self) -> bool {
        false
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.assets_index.entry(self.url).or_default().clone()
    }

    fn cache_file(&self, _: &MetadataManager) -> Option<impl AsRef<Path> + Send + Sync + 'static> {
        Some(Arc::clone(&self.cache))
    }

    fn data_hash(&self) -> Option<Ustr> {
        Some(self.hash)
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct MojangJavaRuntimeComponentMetadataItem {
    pub url: Ustr,
    pub cache: Arc<Path>,
    pub hash: Ustr,
}

impl MetadataItem for MojangJavaRuntimeComponentMetadataItem {
    type T = JavaRuntimeComponentManifest;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get(self.url.as_str())
    }

    fn expires(&self) -> bool {
        false
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.java_runtime_manifests.entry(self.url).or_default().clone()
    }

    fn cache_file(&self, _: &MetadataManager) -> Option<impl AsRef<Path> + Send + Sync + 'static> {
        Some(Arc::clone(&self.cache))
    }

    fn data_hash(&self) -> Option<Ustr> {
        Some(self.hash)
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct FabricLoaderManifestMetadataItem;

impl MetadataItem for FabricLoaderManifestMetadataItem {
    type T = FabricLoaderManifest;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get(FABRIC_LOADER_MANIFEST_URL)
    }

    fn expires(&self) -> bool {
        true
    }

    fn cache_file(&self, metadata_manager: &MetadataManager) -> Option<impl AsRef<Path> + Send + Sync + 'static> {
        Some(Arc::clone(&metadata_manager.fabric_loader_manifest_cache))
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.fabric_loader_manifest.clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct FabricLaunchMetadataItem {
    pub minecraft_version: Ustr,
    pub loader_version: Ustr,
}

impl MetadataItem for FabricLaunchMetadataItem {
    type T = FabricLaunch;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get(format!("https://meta.fabricmc.net/v2/versions/loader/{}/{}", self.minecraft_version, self.loader_version))
    }

    fn expires(&self) -> bool {
        false
    }

    fn cache_file(&self, metadata_manager: &MetadataManager) -> Option<impl AsRef<Path> + Send + Sync + 'static> {
        let mut path = metadata_manager.metadata_cache.join("fabric_launch");
        path.push(self.minecraft_version.as_str());
        path.push(self.loader_version.as_str());
        Some(path)
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        let key = (self.minecraft_version, self.loader_version);
        states.fabric_launch.entry(key).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct ModrinthSearchMetadataItem<'a>(pub &'a ModrinthSearchRequest);

impl<'a> MetadataItem for ModrinthSearchMetadataItem<'a> {
    type T = ModrinthSearchResult;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get(MODRINTH_SEARCH_URL).query(self.0)
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.modrinth_search.entry(self.0.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct ModrinthProjectVersionsMetadataItem<'a>(pub &'a ModrinthProjectVersionsRequest);

impl<'a> MetadataItem for ModrinthProjectVersionsMetadataItem<'a> {
    type T = ModrinthProjectVersionsResult;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        let url = format!("https://api.modrinth.com/v2/project/{}/version?include_changelog=false", self.0.project_id);
        let mut request = client.get(url);
        if let Some(loaders) = &self.0.loaders && let Ok(str) = serde_json::to_string(loaders) {
            request = request.query(&[("loaders", str)]);
        }
        if let Some(game_versions) = &self.0.game_versions && let Ok(str) = serde_json::to_string(game_versions) {
            request = request.query(&[("game_versions", str)]);
        }
        request
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.modrinth_project_versions.entry(self.0.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct ModrinthVersionMetadataItem(pub Arc<str>);

impl MetadataItem for ModrinthVersionMetadataItem {
    type T = ModrinthProjectVersion;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        let url = format!("https://api.modrinth.com/v2/version/{}", self.0);
        client.get(url)
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.modrinth_versions.entry(self.0.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Clone, Debug, Serialize, Hash, PartialEq, Eq)]
pub struct VersionUpdateParameters {
    pub loaders: Arc<[ModrinthLoader]>,
    pub game_versions: Arc<[Ustr]>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ModrinthVersionUpdateMetadataItem {
    pub sha1: Arc<str>,
    pub params: VersionUpdateParameters,
}

impl MetadataItem for ModrinthVersionUpdateMetadataItem {
    type T = ModrinthVersionFileUpdateResult;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        let url = format!("https://api.modrinth.com/v2/version_file/{}/update", self.sha1);
        client.post(url).json(&self.params)
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.modrinth_version_v2_updates.entry(self.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Clone, Debug, Serialize, Hash, PartialEq, Eq)]
pub struct VersionV3UpdateParameters {
    pub loaders: Arc<[Arc<str>]>,
    pub loader_fields: VersionV3LoaderFields,
}

#[derive(Clone, Debug, Serialize, Hash, PartialEq, Eq)]
pub struct VersionV3LoaderFields {
    pub mrpack_loaders: Arc<[ModrinthLoader]>,
    pub game_versions: Arc<[Ustr]>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ModrinthV3VersionUpdateMetadataItem {
    pub sha1: Arc<str>,
    pub params: VersionV3UpdateParameters,
}

impl MetadataItem for ModrinthV3VersionUpdateMetadataItem {
    type T = ModrinthVersionFileUpdateResult;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        let url = format!("https://api.modrinth.com/v3/version_file/{}/update", self.sha1);
        client.post(url).json(&self.params)
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.modrinth_version_v3_updates.entry(self.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct ModrinthVersionsFromHashesMetadataItem<'a>(pub &'a ModrinthVersionsFromHashesRequest);

impl<'a> MetadataItem for ModrinthVersionsFromHashesMetadataItem<'a> {
    type T = ModrinthVersionsFromHashesResponse;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.post("https://api.modrinth.com/v2/version_files").json(self.0)
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.modrinth_versions_from_hashes.entry(self.0.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct ModrinthProjectsMetadataItem<'a>(pub &'a ModrinthProjectsRequest);

impl<'a> MetadataItem for ModrinthProjectsMetadataItem<'a> {
    type T = ModrinthProjectsResponse;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        let mut ids = String::from("[");
        for (i, id) in self.0.ids.iter().enumerate() {
            if i > 0 {
                ids.push(',');
            }
            ids.push('"');
            ids.push_str(id.as_ref());
            ids.push('"');
        }
        ids.push(']');

        client.get("https://api.modrinth.com/v2/projects").query(&[("ids", ids)])
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.modrinth_projects.entry(self.0.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(ModrinthProjectsResponse(serde_json::from_slice::<Arc<[ModrinthProjectResult]>>(bytes)?))
    }
}

#[derive(Debug)]
pub struct NeoforgeInstallerMavenMetadataItem;

impl MetadataItem for NeoforgeInstallerMavenMetadataItem {
    type T = NeoforgeMavenManifest;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get("https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml")
    }

    fn expires(&self) -> bool {
        true
    }

    fn cache_file(&self, metadata_manager: &MetadataManager) -> Option<impl AsRef<Path> + Send + Sync + 'static> {
        Some(Arc::clone(&metadata_manager.neoforge_installer_maven_cache))
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.neoforge_installer_maven_manifest.clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        let maven = serde_xml_rs::from_reader::<MavenMetadataXml, _>(bytes)?;

        let mut versions: Vec<Ustr> = maven.versioning.versions.version.iter().cloned().collect();

        versions.sort_by_cached_key(|version| {
            VersionFragment::string_to_parts(version.as_str())
        });

        Ok(NeoforgeMavenManifest(versions.into_iter().rev().collect()))
    }
}

#[derive(Debug)]
pub struct ForgeInstallerMavenMetadataItem;

impl MetadataItem for ForgeInstallerMavenMetadataItem {
    type T = ForgeMavenManifest;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get("https://maven.minecraftforge.net/net/minecraftforge/forge/maven-metadata.xml")
    }

    fn expires(&self) -> bool {
        true
    }

    fn cache_file(&self, metadata_manager: &MetadataManager) -> Option<impl AsRef<Path> + Send + Sync + 'static> {
        Some(Arc::clone(&metadata_manager.forge_installer_maven_cache))
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.forge_installer_maven_manifest.clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        let maven = serde_xml_rs::from_reader::<MavenMetadataXml, _>(bytes)?;

        let mut versions: Vec<Ustr> = maven.versioning.versions.version.iter().cloned().collect();

        versions.sort_by_cached_key(|version| {
            if let Some((minecraft, forge)) = version.split_once('-') {
                let mut minecraft_parts = VersionFragment::string_to_parts(minecraft);
                let forge_parts = VersionFragment::string_to_parts(forge);
                while minecraft_parts.len() < 3 {
                    minecraft_parts.push(VersionFragment::Number(0));
                }
                minecraft_parts.extend(forge_parts.into_iter());
                minecraft_parts
            } else {
                VersionFragment::string_to_parts(version.as_str())
            }
        });

        Ok(ForgeMavenManifest(versions.into_iter().rev().collect()))
    }
}

#[derive(Debug)]
pub struct ModrinthProjectMetadataItem<'a>(pub &'a ModrinthProjectRequest);

impl<'a> MetadataItem for ModrinthProjectMetadataItem<'a> {
    type T = ModrinthProjectResult;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        let url = format!("{}/{}", MODRINTH_PROJECT_URL, self.0.project_id);
        client.get(url)
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.modrinth_project.entry(self.0.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct CurseforgeSearchMetadataItem<'a>(pub &'a CurseforgeSearchRequest);

impl<'a> MetadataItem for CurseforgeSearchMetadataItem<'a> {
    type T = CurseforgeSearchResult;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.get(CURSEFORGE_SEARCH_URL)
            .query(self.0)
            .query(&[("gameId", MINECRAFT_GAME_ID)])
            .query(&[("sortOrder", "desc")])
            .header("x-api-key", "$2a$10$YXf6dyJfJZM4zeChdr.RDOvWN.L48AN0dQShQO8/cVc5ho1wA8ZbS")
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.curseforge_search.entry(self.0.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct CurseforgeFingerprintMetadataItem<'a>(pub &'a CurseforgeFingerprintRequest);

impl<'a> MetadataItem for CurseforgeFingerprintMetadataItem<'a> {
    type T = CurseforgeFingerprintResponse;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.post("https://api.curseforge.com/v1/fingerprints")
            .json(self.0)
            .header("x-api-key", "$2a$10$YXf6dyJfJZM4zeChdr.RDOvWN.L48AN0dQShQO8/cVc5ho1wA8ZbS")
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.curseforge_fingerprints.entry(self.0.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct CurseforgeGetModFilesMetadataItem<'a>(pub &'a CurseforgeGetModFilesRequest);

impl<'a> MetadataItem for CurseforgeGetModFilesMetadataItem<'a> {
    type T = CurseforgeGetModFilesResult;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        let mut req = client.get(format!("https://api.curseforge.com/v1/mods/{}/files", self.0.mod_id))
            .query(&[("gameId", MINECRAFT_GAME_ID)])
            .header("x-api-key", "$2a$10$YXf6dyJfJZM4zeChdr.RDOvWN.L48AN0dQShQO8/cVc5ho1wA8ZbS");

        if let Some(mod_loader_type) = self.0.mod_loader_type {
            req = req.query(&[("modLoaderType", mod_loader_type)]);
        }
        if let Some(page_size) = self.0.page_size {
            req = req.query(&[("pageSize", page_size)]);
        }
        if let Some(game_version) = self.0.game_version {
            req = req.query(&[("gameVersion", &game_version)]);
        }

        req
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.curseforge_get_mod_files.entry(self.0.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}

#[derive(Debug)]
pub struct CurseforgeGetFilesMetadataItem<'a>(pub &'a CurseforgeGetFilesRequest);

impl<'a> MetadataItem for CurseforgeGetFilesMetadataItem<'a> {
    type T = CurseforgeGetModFilesResult;

    fn request(&self, client: &reqwest::Client) -> RequestBuilder {
        client.post("https://api.curseforge.com/v1/mods/files")
            .json(self.0)
            .header("x-api-key", "$2a$10$YXf6dyJfJZM4zeChdr.RDOvWN.L48AN0dQShQO8/cVc5ho1wA8ZbS")
    }

    fn expires(&self) -> bool {
        true
    }

    fn state(&self, states: &mut MetadataManagerStates) -> MetaLoadStateWrapper<Self::T> {
        states.curseforge_get_files.entry(self.0.clone()).or_default().clone()
    }

    fn deserialize(bytes: &[u8]) -> Result<Self::T, MetaLoadError> {
        Ok(serde_json::from_slice(bytes)?)
    }
}
