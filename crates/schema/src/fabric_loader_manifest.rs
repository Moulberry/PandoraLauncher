use serde::Deserialize;
use ustr::Ustr;

pub const FABRIC_LOADER_MANIFEST_URL: &str = "https://meta.fabricmc.net/v2/versions/loader";

#[derive(Deserialize, Debug)]
#[cfg_attr(debug_assertions, serde(deny_unknown_fields))]
pub struct FabricLoaderManifest(pub Vec<FabricLoaderVersion>);

#[derive(Deserialize, Debug)]
#[cfg_attr(debug_assertions, serde(deny_unknown_fields))]
pub struct FabricLoaderVersion {
    pub separator: Ustr,
    pub build: usize,
    pub maven: Ustr,
    pub version: Ustr,
    pub stable: bool,
}
