use std::{collections::HashMap, sync::Arc};

use serde::Deserialize;
use ustr::Ustr;

use crate::{maven::{MavenCoordinate, MavenMetadataXml}, version::{GameLibrary, GameLibraryArtifact, GameLibraryDownloads, PartialMinecraftVersion}, version_manifest::MinecraftVersionType};

pub const NEOFORGE_INSTALLER_MAVEN_URL: &str = "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeInstallProfile {
    pub minecraft: Arc<str>,
    pub json: Arc<str>,
    pub mirror_list: Arc<str>,
    pub data: HashMap<String, ForgeSidedData>,
    pub processors: Arc<[ForgeInstallProcessor]>,
    pub libraries: Arc<[GameLibrary]>
}

#[derive(Debug, Deserialize)]
#[cfg_attr(debug_assertions, serde(deny_unknown_fields))]
pub struct ForgeSidedData {
    pub client: Arc<str>,
    pub server: Arc<str>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum ForgeSide {
    Client,
    Server,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(debug_assertions, serde(deny_unknown_fields))]
pub struct ForgeInstallProcessor {
    pub sides: Option<Arc<[ForgeSide]>>,
    pub jar: Arc<str>,
    pub classpath: Arc<[Arc<str>]>,
    pub args: Arc<[Ustr]>,
    pub outputs: Option<HashMap<String, String>>,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum VersionFragment {
    Alpha,
    Beta,
    Snapshot,
    String(String),
    Number(usize),
}

impl VersionFragment {
    pub fn string_to_parts(version: &str) -> Vec<Self> {
        version.split(&['.', '-', '+'])
            .map(|v| {
                if let Ok(number) = v.parse::<usize>() {
                    VersionFragment::Number(number)
                } else if v.eq_ignore_ascii_case("alpha") {
                    VersionFragment::Alpha
                } else if v.eq_ignore_ascii_case("beta") {
                    VersionFragment::Beta
                } else if v.eq_ignore_ascii_case("snapshot") {
                    VersionFragment::Snapshot
                } else {
                    VersionFragment::String(v.into())
                }
            })
            .collect::<Vec<_>>()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeInstallProfileLegacy {
    pub install: LegacyInstallInfo,
    pub version_info: LegacyVersionInfo,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyInstallInfo {
    pub path: Arc<str>,
    pub file_path: Arc<str>,
    pub minecraft: Arc<str>,
    pub mirror_list: Arc<str>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyVersionInfo {
    pub inherits_from: Option<Ustr>,
    pub assets: Option<Ustr>,
    pub id: Option<Ustr>,
    pub libraries: Option<Vec<LegacyLibraryDownload>>,
    pub main_class: Option<Ustr>,
    pub minecraft_arguments: Option<Ustr>,
    pub minimum_launcher_version: Option<u32>,
    pub r#type: Option<MinecraftVersionType>,
}

#[derive(Debug, Deserialize)]
pub struct LegacyLibraryDownload {
    pub name: Ustr,
    pub url: Option<Ustr>,
    pub clientreq: Option<bool>,
    pub serverreq: Option<bool>,
}

impl LegacyVersionInfo {
    pub fn into_partial_version(self, side: ForgeSide) -> PartialMinecraftVersion {
        let libraries = self.libraries.map(|libraries| libraries.into_iter().filter_map(|library| {
            let req = match side {
                ForgeSide::Client => library.clientreq,
                ForgeSide::Server => library.serverreq,
            };
            if req == Some(false) {
                return None;
            }

            let coordinate = MavenCoordinate::create(&library.name);
            let artifact_path = coordinate.artifact_path();
            let url = if let Some(url) = library.url {
                format!("{}{}", url, artifact_path)
            } else {
                format!("https://libraries.minecraft.net/{}", artifact_path)
            };

            Some(GameLibrary {
                downloads: GameLibraryDownloads {
                    artifact: Some(GameLibraryArtifact {
                        url: url.into(),
                        path: artifact_path.into(),
                        sha1: None,
                        size: None,
                    }),
                    classifiers: None,
                },
                name: library.name,
                rules: None,
                natives: None,
                extract: None,
            })
        }).collect());

        PartialMinecraftVersion {
            inherits_from: self.inherits_from,
            arguments: None,
            asset_index: None,
            assets: self.assets,
            compliance_level: None,
            downloads: None,
            id: self.id,
            java_version: None,
            libraries,
            logging: None,
            main_class: self.main_class,
            minecraft_arguments: self.minecraft_arguments,
            minimum_launcher_version: self.minimum_launcher_version,
            r#type: self.r#type,
        }
    }
}

#[derive(Debug)]
pub struct ForgeMavenManifest(pub Vec<Ustr>);

#[derive(Debug)]
pub struct NeoforgeMavenManifest(pub Vec<Ustr>);
