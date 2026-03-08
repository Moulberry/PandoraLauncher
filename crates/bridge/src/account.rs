use std::sync::Arc;

use gpui::SharedString;
use gpui_component::select::SelectItem;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Account {
    pub uuid: Uuid,
    pub username: Arc<str>,
    pub head: Option<Arc<[u8]>>,
}


impl SelectItem for Account {
    type Value = Arc<str>;

    fn title(&self) -> SharedString {
   		SharedString::from(self.username.clone())
    }

    fn value(&self) -> &Self::Value {
   		&self.username
    }
}
