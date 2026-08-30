use std::sync::Arc;

use schema::{curseforge::{CurseforgeChangelogRequest, CurseforgeChangelogResult, CurseforgeGetModFilesRequest, CurseforgeGetModFilesResult, CurseforgeSearchRequest, CurseforgeSearchResult}, fabric_loader_manifest::FabricLoaderManifest, forge::{ForgeMavenManifest, NeoforgeMavenManifest}, modrinth::{ModrinthChangelogRequest, ModrinthChangelogResult, ModrinthProjectRequest, ModrinthProjectResult, ModrinthProjectVersionsRequest, ModrinthProjectVersionsResult, ModrinthSearchRequest, ModrinthSearchResult}, version_manifest::MinecraftVersionManifest};

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadataRequest {
    MinecraftVersionManifest,
    FabricLoaderManifest,
    ForgeMavenManifest,
    NeoforgeMavenManifest,
    ModrinthSearch(ModrinthSearchRequest),
    ModrinthProjectVersions(ModrinthProjectVersionsRequest),
    ModrinthProject(ModrinthProjectRequest),
    ModrinthChangelog(ModrinthChangelogRequest),
    CurseforgeSearch(CurseforgeSearchRequest),
    CurseforgeGetModFiles(CurseforgeGetModFilesRequest),
    CurseforgeChangelog(CurseforgeChangelogRequest),
}

#[derive(Debug)]
pub enum MetadataResult {
    MinecraftVersionManifest(Arc<MinecraftVersionManifest>),
    FabricLoaderManifest(Arc<FabricLoaderManifest>),
    ForgeMavenManifest(Arc<ForgeMavenManifest>),
    NeoforgeMavenManifest(Arc<NeoforgeMavenManifest>),
    ModrinthSearchResult(Arc<ModrinthSearchResult>),
    ModrinthProjectVersionsResult(Arc<ModrinthProjectVersionsResult>),
    ModrinthProjectResult(Arc<ModrinthProjectResult>),
    ModrinthChangelogResult(Arc<ModrinthChangelogResult>),
    CurseforgeSearchResult(Arc<CurseforgeSearchResult>),
    CurseforgeGetModFilesResult(Arc<CurseforgeGetModFilesResult>),
    CurseforgeChangelogResult(Arc<CurseforgeChangelogResult>),
}
