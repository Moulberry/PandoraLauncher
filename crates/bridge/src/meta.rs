use std::sync::Arc;

use schema::{modrinth::{ModrinthProjectVersionsRequest, ModrinthProjectVersionsResult, ModrinthSearchRequest, ModrinthSearchResult}, version_manifest::MinecraftVersionManifest};

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetadataRequest {
    MinecraftVersionManifest,
    ModrinthSearch(ModrinthSearchRequest),
    ModrinthProjectVersions(ModrinthProjectVersionsRequest),
}

#[derive(Debug)]
pub enum MetadataResult {
    MinecraftVersionManifest(Arc<MinecraftVersionManifest>),
    ModrinthSearchResult(Arc<ModrinthSearchResult>),
    ModrinthProjectVersionsResult(Arc<ModrinthProjectVersionsResult>),
}
