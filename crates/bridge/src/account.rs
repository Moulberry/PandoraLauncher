use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct Account {
    pub uuid: Uuid,
    pub username: Arc<str>,
    pub offline: bool,
    pub head: Option<Arc<[u8]>>,
}
