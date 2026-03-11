use std::sync::Arc;

use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize, Clone)]
pub struct MinecraftProfileResponse {
    pub id: Uuid,
    pub name: Arc<str>,
    pub skins: Vec<MinecraftProfileSkin>,
    pub capes: Vec<MinecraftProfileCape>,
}

impl MinecraftProfileResponse {
    pub fn active_skin(&self) -> Option<&MinecraftProfileSkin> {
        self.skins.iter().find(|skin| skin.state == SkinState::Active)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MinecraftProfileSkin {
    pub state: SkinState,
    pub url: Arc<str>,
    pub variant: SkinVariant,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SkinState {
    Active,
    #[serde(other)]
    Inactive,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SkinVariant {
    Classic,
    Slim,
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MinecraftProfileCape {
    pub id: Uuid,
    pub state: SkinState,
    pub url: Arc<str>,
    pub alias: Arc<str>,
}
