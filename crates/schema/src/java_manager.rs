use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JavaProvider {
    Mojang,
    Adoptium,
    Zulu,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct JavaVariant {
    pub provider: JavaProvider,
    pub major_version: u32,
    pub architecture: String,
    pub os: String,
    pub download_url: String,
    pub is_installed: bool,
}
