use serde::Deserialize;
use ustr::Ustr;

pub const QUILT_LOADER_MANIFEST_URL: &str = "https://meta.quiltmc.org/v3/versions/loader";

#[derive(Deserialize, Debug)]
pub struct QuiltLoaderManifest(pub Vec<QuiltLoaderVersion>);

#[derive(Deserialize, Debug)]
pub struct QuiltLoaderVersion {
    pub separator: Ustr,
    pub build: usize,
    pub maven: Ustr,
    pub version: Ustr,
}