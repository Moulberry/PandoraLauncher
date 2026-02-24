pub use inner::*;

#[derive(thiserror::Error, Debug)]
pub enum SecretStorageError {
    #[error("Access to the secret storage was denied")]
    AccessDenied,
    #[error("Serialization error")]
    SerializationError,
    #[error("I/O error")]
    IoError,
    #[error("Unknown error")]
    UnknownError,
    #[error("Not unique")]
    NotUnique,
    #[cfg(target_os = "windows")]
    #[error("Windows error: {0}")]
    WindowsError(#[from] windows::core::Error),
    #[cfg(target_os = "macos")]
    #[error("Security.Framework error: {0}")]
    SecurityFrameworkError(#[from] security_framework::base::Error),
}

#[cfg(target_os = "linux")]
mod inner {
    use keyring::Entry;
    use tokio::task;
    use uuid::Uuid;

    use crate::{credentials::AccountCredentials, secret::SecretStorageError};

    impl From<oo7::Error> for SecretStorageError {
        fn from(value: oo7::Error) -> Self {
            Self::from(&value)
        }
    }

    impl From<&oo7::Error> for SecretStorageError {
        fn from(value: &oo7::Error) -> Self {
            match value {
                oo7::Error::File(error) => match error {
                    oo7::file::Error::Io(_) => Self::IoError,
                    _ => Self::UnknownError,
                },
                oo7::Error::DBus(error) => match error {
                    oo7::dbus::Error::Service(service_error) => match service_error {
                        oo7::dbus::ServiceError::IsLocked(_) => Self::AccessDenied,
                        _ => Self::UnknownError,
                    },
                    oo7::dbus::Error::Dismissed => Self::AccessDenied,
                    oo7::dbus::Error::IO(_) => Self::IoError,
                    _ => Self::UnknownError,
                },
            }
        }
    }

    pub struct PlatformSecretStorage {
        keyring: oo7::Result<oo7::Keyring>,
    }

    impl PlatformSecretStorage {
        pub async fn new() -> Result<Self, SecretStorageError> {
            Ok(Self {
                keyring: oo7::Keyring::new().await,
            })
        }

        // Fallback path for platforms where the primary oo7 Secret Service path fails
        // (seen on some KDE/KWallet setups). Uses the keyring crate to talk to
        // org.freedesktop.secrets via the OS keyring provider.
        async fn fallback_read(uuid: Uuid) -> Result<Option<AccountCredentials>, SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            task::spawn_blocking(move || {
                let entry = Entry::new("pandora-launcher", &uuid_str).map_err(|_| SecretStorageError::UnknownError)?;
                match entry.get_password() {
                    Ok(raw) => {
                        Ok(Some(serde_json::from_str(&raw).map_err(|_| SecretStorageError::SerializationError)?))
                    },
                    Err(error) => match error {
                        keyring::Error::NoEntry => Ok(None),
                        keyring::Error::NoStorageAccess(_) => Err(SecretStorageError::AccessDenied),
                        _ => Err(SecretStorageError::UnknownError),
                    },
                }
            })
            .await
            .map_err(|_| SecretStorageError::UnknownError)?
        }

        async fn fallback_write(uuid: Uuid, credentials: &AccountCredentials) -> Result<(), SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            let raw = serde_json::to_string(credentials).map_err(|_| SecretStorageError::SerializationError)?;
            task::spawn_blocking(move || {
                let entry = Entry::new("pandora-launcher", &uuid_str).map_err(|_| SecretStorageError::UnknownError)?;
                entry.set_password(&raw).map_err(|error| match error {
                    keyring::Error::NoStorageAccess(_) => SecretStorageError::AccessDenied,
                    _ => SecretStorageError::UnknownError,
                })
            })
            .await
            .map_err(|_| SecretStorageError::UnknownError)?
        }

        async fn fallback_delete(uuid: Uuid) -> Result<(), SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            task::spawn_blocking(move || {
                let entry = Entry::new("pandora-launcher", &uuid_str).map_err(|_| SecretStorageError::UnknownError)?;
                match entry.delete_credential() {
                    Ok(()) => Ok(()),
                    Err(error) => match error {
                        keyring::Error::NoEntry => Ok(()),
                        keyring::Error::NoStorageAccess(_) => Err(SecretStorageError::AccessDenied),
                        _ => Err(SecretStorageError::UnknownError),
                    },
                }
            })
            .await
            .map_err(|_| SecretStorageError::UnknownError)?
        }

        pub async fn read_credentials(&self, uuid: Uuid) -> Result<Option<AccountCredentials>, SecretStorageError> {
            let primary = async {
                let keyring = self.keyring.as_ref()?;
                keyring.unlock().await?;

                let uuid_str = uuid.as_hyphenated().to_string();
                let attributes = vec![("service", "pandora-launcher"), ("uuid", uuid_str.as_str())];

                let items = keyring.search_items(&attributes).await?;

                if items.is_empty() {
                    Ok(None)
                } else if items.len() > 1 {
                    Err(SecretStorageError::NotUnique)
                } else {
                    let raw = items[0].secret().await?;
                    Ok(Some(serde_json::from_slice(&raw).map_err(|_| SecretStorageError::SerializationError)?))
                }
            }
            .await;

            match primary {
                Ok(value) => Ok(value),
                Err(
                    error @ (SecretStorageError::UnknownError
                    | SecretStorageError::IoError
                    | SecretStorageError::NotUnique),
                ) => {
                    let fallback = Self::fallback_read(uuid).await;
                    if fallback.is_ok() {
                        log::debug!("Secret storage fallback used for read (primary error: {error:?})");
                    }
                    fallback.or(Err(error))
                },
                Err(other) => Err(other),
            }
        }

        pub async fn write_credentials(
            &self,
            uuid: Uuid,
            credentials: &AccountCredentials,
        ) -> Result<(), SecretStorageError> {
            let primary = async {
                let keyring = self.keyring.as_ref()?;
                keyring.unlock().await?;

                let uuid_str = uuid.as_hyphenated().to_string();
                let attributes = vec![("service", "pandora-launcher"), ("uuid", uuid_str.as_str())];

                let bytes = serde_json::to_vec(credentials).map_err(|_| SecretStorageError::SerializationError)?;

                keyring.create_item("Pandora Minecraft Account", &attributes, bytes, true).await?;
                Ok(())
            }
            .await;

            match primary {
                Ok(()) => Ok(()),
                Err(error @ (SecretStorageError::UnknownError | SecretStorageError::IoError)) => {
                    let fallback = Self::fallback_write(uuid, credentials).await;
                    if fallback.is_ok() {
                        log::debug!("Secret storage fallback used for write (primary error: {error:?})");
                    }
                    fallback.or(Err(error))
                },
                Err(other) => Err(other),
            }
        }

        pub async fn delete_credentials(&self, uuid: Uuid) -> Result<(), SecretStorageError> {
            let primary = async {
                let keyring = self.keyring.as_ref()?;
                keyring.unlock().await?;

                let uuid_str = uuid.as_hyphenated().to_string();
                let attributes = vec![("service", "pandora-launcher"), ("uuid", uuid_str.as_str())];

                keyring.delete(&attributes).await?;
                Ok(())
            }
            .await;

            match primary {
                Ok(()) => Ok(()),
                Err(error @ (SecretStorageError::UnknownError | SecretStorageError::IoError)) => {
                    let fallback = Self::fallback_delete(uuid).await;
                    if fallback.is_ok() {
                        log::debug!("Secret storage fallback used for delete (primary error: {error:?})");
                    }
                    fallback.or(Err(error))
                },
                Err(other) => Err(other),
            }
        }
    }
}

#[cfg(target_os = "windows")]
mod inner {
    use uuid::Uuid;

    use crate::{credentials::AccountCredentials, secret::SecretStorageError};

    use windows::Win32::Security::Credentials::*;

    pub struct PlatformSecretStorage;

    impl PlatformSecretStorage {
        pub async fn new() -> Result<Self, SecretStorageError> {
            Ok(Self)
        }

        pub async fn read_credentials(&self, uuid: Uuid) -> Result<Option<AccountCredentials>, SecretStorageError> {
            let target_name = format!("PandoraLauncher_MinecraftAccount_{}", uuid.as_hyphenated());

            fn read<T: for<'a> serde::Deserialize<'a>>(target: String) -> Result<Option<T>, SecretStorageError> {
                let mut target_name: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();

                let mut credentials: *mut CREDENTIALW = std::ptr::null_mut();

                unsafe {
                    let result = CredReadW(
                        windows::core::PWSTR::from_raw(target_name.as_mut_ptr()),
                        CRED_TYPE_GENERIC,
                        None,
                        &mut credentials,
                    );

                    if let Err(error) = result {
                        const ERROR_NOT_FOUND: windows::core::HRESULT =
                            windows::core::HRESULT::from_win32(windows::Win32::Foundation::ERROR_NOT_FOUND.0);
                        if error.code() == ERROR_NOT_FOUND {
                            return Ok(None);
                        }
                        return Err(error.into());
                    }

                    let Some(credentials) = credentials.as_mut() else {
                        return Ok(None);
                    };

                    let raw =
                        std::slice::from_raw_parts(credentials.CredentialBlob, credentials.CredentialBlobSize as usize);
                    Ok(Some(serde_json::from_slice(&raw).map_err(|_| SecretStorageError::SerializationError)?))
                }
            }

            let mut account = AccountCredentials::default();

            let uuid = uuid.as_hyphenated();
            account.msa_refresh = read(format!("PandoraLauncher_MsaRefresh_{}", uuid))?;
            account.msa_refresh_force_client_id = read(format!("PandoraLauncher_MsaRefreshForceClientId_{}", uuid))?;
            account.msa_access = read(format!("PandoraLauncher_MsaAccess_{}", uuid))?;
            account.xbl = read(format!("PandoraLauncher_Xbl_{}", uuid))?;
            account.xsts = read(format!("PandoraLauncher_Xsts_{}", uuid))?;
            account.access_token = read(format!("PandoraLauncher_AccessToken_{}", uuid))?;

            Ok(Some(account))
        }

        pub async fn write_credentials(
            &self,
            uuid: Uuid,
            credentials: &AccountCredentials,
        ) -> Result<(), SecretStorageError> {
            fn write_inner(target: String, bytes: Option<Vec<u8>>) -> Result<(), SecretStorageError> {
                let mut target_name: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();

                if let Some(mut bytes) = bytes {
                    let credentials = CREDENTIALW {
                        Flags: CRED_FLAGS(0),
                        Type: CRED_TYPE_GENERIC,
                        TargetName: windows::core::PWSTR::from_raw(target_name.as_mut_ptr()),
                        CredentialBlobSize: bytes.len() as u32,
                        CredentialBlob: bytes.as_mut_ptr(),
                        Persist: CRED_PERSIST_LOCAL_MACHINE,
                        ..CREDENTIALW::default()
                    };

                    unsafe { Ok(CredWriteW(&credentials, 0)?) }
                } else {
                    unsafe {
                        Ok(CredDeleteW(
                            windows::core::PWSTR::from_raw(target_name.as_mut_ptr()),
                            CRED_TYPE_GENERIC,
                            None,
                        )?)
                    }
                }
            }

            fn write(target: String, data: Option<&impl serde::Serialize>) -> Result<(), SecretStorageError> {
                let bytes = data
                    .map(|v| serde_json::to_vec(v).map_err(|_| SecretStorageError::SerializationError))
                    .transpose()?;
                write_inner(target, bytes)
            }

            let uuid = uuid.as_hyphenated();
            write(format!("PandoraLauncher_MsaRefresh_{}", uuid), credentials.msa_refresh.as_ref())?;
            write(
                format!("PandoraLauncher_MsaRefreshForceClientId_{}", uuid),
                credentials.msa_refresh_force_client_id.as_ref(),
            )?;
            write(format!("PandoraLauncher_MsaAccess_{}", uuid), credentials.msa_access.as_ref())?;
            write(format!("PandoraLauncher_Xbl_{}", uuid), credentials.xbl.as_ref())?;
            write(format!("PandoraLauncher_Xsts_{}", uuid), credentials.xsts.as_ref())?;
            write(format!("PandoraLauncher_AccessToken_{}", uuid), credentials.access_token.as_ref())?;

            Ok(())
        }

        pub async fn delete_credentials(&self, uuid: Uuid) -> Result<(), SecretStorageError> {
            fn delete(target: String) -> windows::core::Result<()> {
                let mut target_name: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();

                unsafe {
                    CredDeleteW(windows::core::PWSTR::from_raw(target_name.as_mut_ptr()), CRED_TYPE_GENERIC, None)
                }
            }

            [
                delete(format!("PandoraLauncher_MsaRefresh_{}", uuid)),
                delete(format!("PandoraLauncher_MsaRefreshForceClientId_{}", uuid)),
                delete(format!("PandoraLauncher_MsaAccess_{}", uuid)),
                delete(format!("PandoraLauncher_Xbl_{}", uuid)),
                delete(format!("PandoraLauncher_Xsts_{}", uuid)),
                delete(format!("PandoraLauncher_AccessToken_{}", uuid)),
            ]
            .into_iter()
            .collect::<Result<(), _>>()?;

            Ok(())
        }
    }
}

#[cfg(target_os = "macos")]
mod inner {
    use security_framework::os::macos::keychain::{SecKeychain, SecPreferencesDomain};
    use uuid::Uuid;

    use crate::{credentials::AccountCredentials, secret::SecretStorageError};

    pub struct PlatformSecretStorage {
        keychain: SecKeychain,
    }

    impl PlatformSecretStorage {
        pub async fn new() -> Result<Self, SecretStorageError> {
            Ok(Self {
                keychain: SecKeychain::default_for_domain(SecPreferencesDomain::User)?,
            })
        }

        pub async fn read_credentials(&self, uuid: Uuid) -> Result<Option<AccountCredentials>, SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            let data = match self.keychain.find_generic_password("com.moulberry.pandoralauncher", uuid_str.as_str()) {
                Ok((data, _)) => data,
                Err(error) if error.code() == security_framework_sys::base::errSecItemNotFound => {
                    return Ok(None);
                },
                Err(error) => {
                    return Err(error.into());
                },
            };
            let data = data.as_ref();
            Ok(Some(serde_json::from_slice(&data).map_err(|_| SecretStorageError::SerializationError)?))
        }

        pub async fn write_credentials(
            &self,
            uuid: Uuid,
            credentials: &AccountCredentials,
        ) -> Result<(), SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            let bytes = serde_json::to_vec(credentials).map_err(|_| SecretStorageError::SerializationError)?;

            self.keychain
                .set_generic_password("com.moulberry.pandoralauncher", uuid_str.as_str(), &bytes)?;
            Ok(())
        }

        pub async fn delete_credentials(&self, uuid: Uuid) -> Result<(), SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();

            let item = match self.keychain.find_generic_password("com.moulberry.pandoralauncher", uuid_str.as_str()) {
                Ok((_, item)) => item,
                Err(error) if error.code() == security_framework_sys::base::errSecItemNotFound => {
                    return Ok(());
                },
                Err(error) => {
                    return Err(error.into());
                },
            };

            item.delete();
            Ok(())
        }
    }
}
