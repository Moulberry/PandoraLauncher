use crate::secret::PlatformSecretStorage;
use crate::{credentials::AccountCredentials, error::AuthError};
use uuid::Uuid;

pub struct CredentialStorage;

impl CredentialStorage {
    pub fn new() -> Self {
        Self
    }

    pub async fn write_and_verify(
        &self,
        storage: &PlatformSecretStorage,
        uuid: Uuid,
        credentials: &AccountCredentials,
    ) -> Result<(), AuthError> {
        storage.write_credentials(uuid, credentials).await?;

        match storage.read_credentials(uuid).await? {
            Some(_) => Ok(()),
            None => Err(AuthError::VerificationFailed),
        }
    }

    pub async fn read(
        &self,
        storage: &PlatformSecretStorage,
        uuid: Uuid,
    ) -> Result<Option<AccountCredentials>, AuthError> {
        let result: Result<Option<AccountCredentials>, _> = storage.read_credentials(uuid).await;
        result.map_err(AuthError::from)
    }

    pub async fn delete(&self, storage: &PlatformSecretStorage, uuid: Uuid) -> Result<(), AuthError> {
        let result: Result<(), _> = storage.delete_credentials(uuid).await;
        result.map_err(AuthError::from)
    }
}
