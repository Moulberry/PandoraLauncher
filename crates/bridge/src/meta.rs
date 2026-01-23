use std::sync::Arc;

use schema::{fabric_loader_manifest::FabricLoaderManifest, forge::{ForgeMavenManifest, NeoforgeMavenManifest}, maven::MavenMetadataXml, modrinth::{ModrinthProjectVersionsRequest, ModrinthProjectVersionsResult, ModrinthSearchRequest, ModrinthSearchResult}, version_manifest::MinecraftVersionManifest};

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadataRequest {
    MinecraftVersionManifest,
    FabricLoaderManifest,
    ForgeMavenManifest,
    NeoforgeMavenManifest,
    ModrinthSearch(ModrinthSearchRequest),
    ModrinthProjectVersions(ModrinthProjectVersionsRequest),
}

#[derive(Debug)]
pub enum MetadataResult {
    MinecraftVersionManifest(Arc<MinecraftVersionManifest>),
    FabricLoaderManifest(Arc<FabricLoaderManifest>),
    ForgeMavenManifest(Arc<ForgeMavenManifest>),
    NeoforgeMavenManifest(Arc<NeoforgeMavenManifest>),
    ModrinthSearchResult(Arc<ModrinthSearchResult>),
    ModrinthProjectVersionsResult(Arc<ModrinthProjectVersionsResult>),
}
