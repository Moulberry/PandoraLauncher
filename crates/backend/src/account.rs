use std::sync::Arc;

use auth::models::MinecraftAccessToken;
use bridge::{account::Account, message::MessageToFrontend};
use rustc_hash::FxHashMap;
use schema::{minecraft_profile::MinecraftProfileResponse, unique_bytes::UniqueBytes};
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
    #[serde(default)]
    pub account_order: Vec<Uuid>,
    pub selected_account: Option<Uuid>,
}

impl BackendAccountInfo {
    pub fn ensure_account_order(&self) -> Vec<Uuid> {
        let mut order = self.account_order.clone();
        order.retain(|uuid| self.accounts.contains_key(uuid));

        for uuid in self.accounts.keys() {
            if !order.contains(uuid) {
                order.push(*uuid);
            }
        }

        if self.account_order.is_empty() {
            order.sort_by(|a, b| {
                let a_name = self.accounts.get(a).map(|a| a.username.as_ref()).unwrap_or("");
                let b_name = self.accounts.get(b).map(|a| a.username.as_ref()).unwrap_or("");
                lexical_sort::natural_lexical_cmp(a_name, b_name)
            });
        }

        order
    }

    pub fn normalize_account_order(&mut self) {
        self.account_order = self.ensure_account_order();
    }

    pub fn create_update_message(&self) -> MessageToFrontend {
        let mut accounts = Vec::with_capacity(self.accounts.len());
        for uuid in self.ensure_account_order() {
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
