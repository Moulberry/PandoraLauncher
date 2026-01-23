use std::sync::Arc;

use serde::Deserialize;
use ustr::Ustr;


#[derive(Debug, Deserialize)]
#[serde(rename = "metadata")]
pub struct MavenMetadataXml {
    // #[serde(rename = "groupId")]
    // pub group_id: Arc<str>,
    // #[serde(rename = "artifactId")]
    // pub artifact_id: Arc<str>,
    pub versioning: MavenMetadataVersioning,
}


#[derive(Debug, Deserialize)]
#[serde(rename = "versioning")]
pub struct MavenMetadataVersioning {
    // pub latest: Arc<str>,
    // pub release: Arc<str>,
    pub versions: MavenMetadataVersions,
}

#[derive(Debug, Deserialize)]
pub struct MavenMetadataVersions {
    #[serde(rename = "version")]
    pub version: Arc<[Ustr]>,
}

pub struct MavenCoordinate<'a> {
    pub group_id: &'a str,
    pub artifact_id: &'a str,
    pub version: &'a str,
    pub specifier: Option<&'a str>,
    pub extension: Option<&'a str>,
}

impl<'a> MavenCoordinate<'a> {
    pub fn create(maven: &'a str) -> Self {
        let (main, extension) = if let Some((main, extension)) = maven.split_once('@') {
            (main, Some(extension))
        } else {
            (maven, None)
        };

        let mut split = main.split(":");
        let group_id = split.next().unwrap();
        let artifact_id = split.next().unwrap();
        let version = split.next().unwrap();
        let specifier = split.next();

        Self { group_id, artifact_id, version, specifier, extension }
    }

    pub fn version_id(&self) -> Vec<isize> {
        let without_plus = self.version.split_once("+").map(|s| s.0).unwrap_or(self.version);

        let mut version_numbers = Vec::new();
        for part in without_plus.split(".") {
            if let Ok(number) = part.parse() {
                version_numbers.push(number);
            } else {
                version_numbers.push(0);
            }
        }
        if version_numbers.is_empty() {
            version_numbers.push(0);
        }
        version_numbers
    }

    pub fn artifact_path(&self) -> String {
        let mut name = self.group_id.replace(".", "/");
        name.push('/');
        name.push_str(self.artifact_id);
        name.push('/');
        name.push_str(self.version);
        name.push('/');
        name.push_str(self.artifact_id);
        name.push('-');
        name.push_str(self.version);
        if let Some(specifier) = self.specifier {
            name.push('-');
            name.push_str(specifier);
        }
        name.push('.');
        if let Some(extension) = self.extension {
            name.push_str(extension);
        } else {
            name.push_str("jar");
        }
        name
    }
}
