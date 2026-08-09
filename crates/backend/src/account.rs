use std::sync::Arc;

use auth::models::MinecraftAccessToken;
use bridge::{account::Account, message::MessageToFrontend};
use indexmap::IndexMap;
use schema::{
    minecraft_profile::{MinecraftProfileResponse, SkinVariant},
    unique_bytes::UniqueBytes,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct MinecraftLoginInfo {
    pub uuid: Uuid,
    pub username: Arc<str>,
    pub access_token: Option<MinecraftAccessToken>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct BackendAccountInfo {
    pub accounts: IndexMap<Uuid, BackendAccount>,
    pub selected_account: Option<Uuid>,
}

impl BackendAccountInfo {
    pub fn create_update_message(&self) -> MessageToFrontend {
        let mut accounts = Vec::with_capacity(self.accounts.len());
        for uuid in self.accounts.keys().copied() {
            let Some(account) = self.accounts.get(&uuid) else {
                continue;
            };
            accounts.push(Account {
                uuid,
                username: account.username.clone(),
                offline: account.offline,
                head: account.head.clone(),
            });
        }
        MessageToFrontend::AccountsUpdated {
            accounts: accounts.into(),
            selected_account: self.selected_account,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BackendAccount {
    pub username: Arc<str>,
    #[serde(default)]
    pub offline: bool,
    pub head: Option<UniqueBytes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offline_skin: Option<UniqueBytes>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offline_skin_variant: Option<SkinVariant>,
}

impl BackendAccount {
    pub fn new_from_profile(profile: &MinecraftProfileResponse) -> Self {
        Self {
            username: profile.name.clone(),
            offline: false,
            head: None,
            offline_skin: None,
            offline_skin_variant: None,
        }
    }

    pub fn new_offline(username: Arc<str>) -> Self {
        Self {
            username,
            offline: true,
            head: None,
            offline_skin: None,
            offline_skin_variant: None,
        }
    }

    pub fn offline_skin_variant(&self) -> SkinVariant {
        self.offline_skin_variant.unwrap_or(SkinVariant::Classic)
    }
}
