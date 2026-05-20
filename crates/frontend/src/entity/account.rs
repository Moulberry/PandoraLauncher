use std::sync::Arc;

use bridge::account::Account;
use gpui::{App, Entity, SharedString};
use uuid::Uuid;

#[derive(Default)]
pub struct AccountEntries {
    pub accounts: Arc<[Account]>,
    pub selected_account_uuid: Option<Uuid>,
    pub selected_account: Option<Account>,
}

impl AccountEntries {
    pub fn set(entity: &Entity<Self>, accounts: Arc<[Account]>, selected_account: Option<Uuid>, cx: &mut App) {
        entity.update(cx, |entries, cx| {
            entries.selected_account =
                selected_account.and_then(|uuid| accounts.iter().find(|acc| acc.uuid == uuid).cloned());
            entries.accounts = accounts;
            entries.selected_account_uuid = selected_account;
            cx.notify();
        });
    }
}

pub trait AccountExt {
    fn username(&self, redact: bool) -> SharedString;
}

static REDACTED: &'static str = "********************************";

impl AccountExt for Account {
    fn username(&self, redacted: bool) -> SharedString {
        if redacted {
            SharedString::new_static(&REDACTED[..self.username.len().min(REDACTED.len())])
        } else {
            self.username.clone().into()
        }
    }
}
