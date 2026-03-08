use std::sync::Arc;

use bridge::account::Account;
use gpui::{App, Entity, SharedString};
use gpui_component::{IndexPath, select::{SelectDelegate, SelectItem}};
use uuid::Uuid;

#[derive(Default)]
pub struct AccountEntries {
    pub accounts: Arc<[Account]>,
    pub selected_account_uuid: Option<Uuid>,
    pub selected_account: Option<Account>,
}

#[derive(Default)]
pub struct AccountList {
	pub accounts: Vec<Account>
}

impl From<&AccountEntries> for AccountList {
	fn from(value: &AccountEntries) -> Self {
    	Self { accounts: value.accounts.to_vec() }
	}
}

impl AccountEntries {
    pub fn set(
        entity: &Entity<Self>,
        accounts: Arc<[Account]>,
        selected_account: Option<Uuid>,
        cx: &mut App,
    ) {
        entity.update(cx, |entries, cx| {
            entries.selected_account =
                selected_account.and_then(|uuid| accounts.iter().find(|acc| acc.uuid == uuid).cloned());
            entries.accounts = accounts;
            entries.selected_account_uuid = selected_account;
            cx.notify();
        });
    }
}

impl SelectDelegate for AccountList {
    type Item = Account;

    fn items_count(&self, _section: usize) -> usize {
        self.accounts.len()
    }

    fn item(&self, ix: gpui_component::IndexPath) -> Option<&Self::Item> {
        self.accounts.get(ix.row)
    }

    fn position<V>(&self, value: &V) -> Option<gpui_component::IndexPath>
    where
        Self::Item: gpui_component::select::SelectItem<Value = V>,
        V: PartialEq
    {
        for (ix, item) in self.accounts.iter().enumerate() {
            if item.value() == value {
                return Some(IndexPath::default().row(ix));
            }
        }

        None
    }
}
