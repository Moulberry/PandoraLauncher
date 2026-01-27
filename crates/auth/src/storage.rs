use crate::{credentials::AccountCredentials, error::AuthError, secret::SecretStorage};
use uuid::Uuid;

pub struct CredentialStorage {
    storage: SecretStorage,
}

impl CredentialStorage {
    pub fn new(storage: SecretStorage) -> Self {
        Self { storage }
    }

    pub async fn write_and_verify(&self, uuid: Uuid, credentials: &AccountCredentials) -> Result<(), AuthError> {
        self.storage.write_credentials(uuid, credentials).await?;

        match self.storage.read_credentials(uuid).await? {
            Some(_) => Ok(()),
            None => Err(AuthError::VerificationFailed),
        }
    }

    pub async fn read(&self, uuid: Uuid) -> Result<Option<AccountCredentials>, AuthError> {
        self.storage.read_credentials(uuid).await.map_err(AuthError::from)
    }

    pub async fn delete(&self, uuid: Uuid) -> Result<(), AuthError> {
        self.storage.delete_credentials(uuid).await.map_err(AuthError::from)
    }
}
