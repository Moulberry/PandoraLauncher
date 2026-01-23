use std::sync::Arc;

use auth::models::{MinecraftAccessToken, MinecraftProfileResponse};
use bridge::{account::Account, message::MessageToFrontend};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub struct MinecraftLoginInfo {
    pub uuid: Uuid,
    pub username: Arc<str>,
    pub access_token: Option<MinecraftAccessToken>,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct BackendAccountInfo {
    pub accounts: FxHashMap<Uuid, BackendAccount>,
    pub selected_account: Option<Uuid>,
}

impl BackendAccountInfo {
    pub fn create_update_message(&self) -> MessageToFrontend {
        let mut accounts = Vec::with_capacity(self.accounts.len());
        for (uuid, account) in &self.accounts {
            accounts.push(Account {
                uuid: *uuid,
                username: account.username.clone(),
                head: account.head.clone(),
            });
        }
        accounts.sort_by(|a, b| lexical_sort::natural_lexical_cmp(&a.username, &b.username));
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
    pub head: Option<Arc<[u8]>>,
}

impl BackendAccount {
    pub fn new_from_profile(profile: &MinecraftProfileResponse) -> Self {
        Self {
            username: profile.name.clone(),
            offline: false,
            head: None,
        }
    }
}
