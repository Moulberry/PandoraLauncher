// Code adapted from https://gist.github.com/ThatGravyBoat/fcdab4a3562b082f82e09e6263cc0210
// Licensed as MIT Copyright (c) 2026 ThatGravyBoat

use std::sync::Arc;

use serde::Deserialize;

use crate::text_component::FlatTextComponent;

#[derive(Deserialize, Debug)]
pub struct ServerStatus {
    #[serde(
        default,
        deserialize_with = "crate::text_component::deserialize_flat_text_component_json"
    )]
    pub description: FlatTextComponent,
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub players: Option<StatusPlayers>,
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub version: Option<StatusVersion>,
    #[serde(default, deserialize_with = "crate::try_deserialize")]
    pub favicon: Option<Arc<str>>,
}

#[derive(Deserialize, Debug)]
pub struct StatusVersion {
    pub name: Arc<str>,
    pub protocol: i32,
}

#[derive(Deserialize, Debug, Default)]
pub struct StatusPlayers {
    pub max: i32,
    pub online: i32,
}

#[derive(Deserialize, Debug)]
pub struct StatusPlayer {
    pub name: Arc<str>,
    pub id: Arc<str>,
}
