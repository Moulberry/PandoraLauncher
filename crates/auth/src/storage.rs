use crate::secret::PlatformSecretStorage;
use crate::{credentials::AccountCredentials, error::AuthError};
use std::time::Duration;
use tokio::time::sleep;
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
        self.retry_with_backoff(storage, uuid, credentials, 3).await
    }

    async fn retry_with_backoff(
        &self,
        storage: &PlatformSecretStorage,
        uuid: Uuid,
        credentials: &AccountCredentials,
        max_retries: u32,
    ) -> Result<(), AuthError> {
        let mut attempts = 0;

        loop {
            match self.write_and_verify_once(storage, uuid, credentials).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        return Err(e);
                    }

                    let delay = Duration::from_millis(100 * (1 << (attempts - 1)));
                    sleep(delay).await;
                },
            }
        }
    }

    async fn write_and_verify_once(
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
