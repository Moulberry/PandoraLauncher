use std::sync::Arc;

use schema::{curseforge::{CurseforgeGetModFilesRequest, CurseforgeGetModFilesResult, CurseforgeSearchRequest, CurseforgeSearchResult}, fabric_loader_manifest::FabricLoaderManifest, forge::{ForgeMavenManifest, NeoforgeMavenManifest}, modrinth::{ModrinthProjectRequest, ModrinthProjectResult, ModrinthProjectVersionsRequest, ModrinthProjectVersionsResult, ModrinthSearchRequest, ModrinthSearchResult}, quilt_loader_manifest::QuiltLoaderManifest, version_manifest::MinecraftVersionManifest};

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadataRequest {
    MinecraftVersionManifest,
    FabricLoaderManifest,
    QuiltLoaderManifest,
    ForgeMavenManifest,
    NeoforgeMavenManifest,
    ModrinthSearch(ModrinthSearchRequest),
    ModrinthProjectVersions(ModrinthProjectVersionsRequest),
    ModrinthProject(ModrinthProjectRequest),
    CurseforgeSearch(CurseforgeSearchRequest),
    CurseforgeGetModFiles(CurseforgeGetModFilesRequest),
}

#[derive(Debug)]
pub enum MetadataResult {
    MinecraftVersionManifest(Arc<MinecraftVersionManifest>),
    FabricLoaderManifest(Arc<FabricLoaderManifest>),
    QuiltLoaderManifest(Arc<QuiltLoaderManifest>),
    ForgeMavenManifest(Arc<ForgeMavenManifest>),
    NeoforgeMavenManifest(Arc<NeoforgeMavenManifest>),
    ModrinthSearchResult(Arc<ModrinthSearchResult>),
    ModrinthProjectVersionsResult(Arc<ModrinthProjectVersionsResult>),
    ModrinthProjectResult(Arc<ModrinthProjectResult>),
    CurseforgeSearchResult(Arc<CurseforgeSearchResult>),
    CurseforgeGetModFilesResult(Arc<CurseforgeGetModFilesResult>),
}
