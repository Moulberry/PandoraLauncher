use thiserror::Error;

use crate::secret::SecretStorageError;

#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Secure storage error: {0}")]
    StorageError(#[from] SecretStorageError),
    #[error("Credential verification failed")]
    VerificationFailed,
    #[error("Credential serialization failed")]
    SerializationError,
}
