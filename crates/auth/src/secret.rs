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

    async fn read(storage: &PlatformSecretStorage, attributes: &[(&str, &str)]) -> Result<Option<oo7::Secret>, SecretStorageError> {
        let keyring = storage.keyring.as_ref()?;
        keyring.unlock().await?;

        let items = keyring.search_items(&attributes).await?;

        if items.is_empty() {
            Ok(None)
        } else if items.len() > 1 {
            Err(SecretStorageError::NotUnique)
        } else {
            Ok(Some(items[0].secret().await?))
        }
    }

    async fn write(storage: &PlatformSecretStorage, label: &str, attributes: &[(&str, &str)], value: &[u8]) -> Result<(), SecretStorageError> {
        let keyring = storage.keyring.as_ref()?;
        keyring.unlock().await?;

        keyring.create_item(label, &attributes, value, true).await?;
        Ok(())
    }

    async fn delete(storage: &PlatformSecretStorage, attributes: &[(&str, &str)]) -> Result<(), SecretStorageError> {
        let keyring = storage.keyring.as_ref()?;
        keyring.unlock().await?;
        keyring.delete(&attributes).await?;
        Ok(())
    }

    impl PlatformSecretStorage {
        pub async fn new() -> Result<Self, SecretStorageError> {
            Ok(Self {
                keyring: oo7::Keyring::new().await,
            })
        }

        pub async fn read_credentials(&self, uuid: Uuid) -> Result<Option<AccountCredentials>, SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            let attributes = &[("service", "pandora-launcher"), ("uuid", uuid_str.as_str())];
            let Some(secret) = read(self, attributes).await? else {
                return Ok(None);
            };
            Ok(Some(serde_json::from_slice(&secret).map_err(|_| SecretStorageError::SerializationError)?))
        }

        pub async fn write_credentials(
            &self,
            uuid: Uuid,
            credentials: &AccountCredentials,
        ) -> Result<(), SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            let attributes = &[("service", "pandora-launcher"), ("uuid", uuid_str.as_str())];

            let bytes = serde_json::to_vec(credentials).map_err(|_| SecretStorageError::SerializationError)?;

            write(self, "Pandora Minecraft Account", attributes, &bytes).await
        }

        pub async fn delete_credentials(&self, uuid: Uuid) -> Result<(), SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            let attributes = &[("service", "pandora-launcher"), ("uuid", uuid_str.as_str())];

            delete(self, attributes).await?;
            Ok(())
        }

        pub async fn read_proxy_password(&self) -> Result<Option<String>, SecretStorageError> {
            let attributes = &[("service", "pandora-launcher"), ("type", "proxy-password")];
            let Some(secret) = read(self, attributes).await? else {
                return Ok(None);
            };
            Ok(Some(String::from_utf8(secret.to_vec()).map_err(|_| SecretStorageError::SerializationError)?))
        }

        pub async fn write_proxy_password(&self, password: &str) -> Result<(), SecretStorageError> {
            let attributes = &[("service", "pandora-launcher"), ("type", "proxy-password")];
            write(self, "Pandora Proxy Password", attributes, password.as_bytes()).await
        }

        pub async fn delete_proxy_password(&self) -> Result<(), SecretStorageError> {
            let attributes = &[("service", "pandora-launcher"), ("type", "proxy-password")];
            delete(self, attributes).await
        }
    }
}

#[cfg(target_os = "windows")]
mod inner {
    use uuid::Uuid;

    use crate::{credentials::AccountCredentials, secret::SecretStorageError};

    use windows::Win32::Security::Credentials::*;

    pub struct PlatformSecretStorage;

    fn read(target: &str) -> Result<Option<Vec<u8>>, SecretStorageError> {
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

            let Some(creds) = credentials.as_mut() else {
                return Ok(None);
            };

            let raw = std::slice::from_raw_parts(creds.CredentialBlob, creds.CredentialBlobSize as usize);
            let raw = raw.to_vec();

            CredFree(credentials as *mut std::ffi::c_void);

            Ok(Some(raw))
        }
    }

    fn read_deserialize<T: for<'a> serde::Deserialize<'a>>(target: &str) -> Result<Option<T>, SecretStorageError> {
        let Some(bytes) = read(target)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(&bytes).map_err(|_| SecretStorageError::SerializationError)?))
    }

    fn write(target: &str, bytes: Option<Vec<u8>>) -> Result<(), SecretStorageError> {
        let Some(mut bytes) = bytes else {
            return delete(target);
        };

        let mut target_name: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();

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
    }

    fn write_serialize(target: &str, data: Option<&impl serde::Serialize>) -> Result<(), SecretStorageError> {
        let bytes = data
            .map(|v| serde_json::to_vec(v).map_err(|_| SecretStorageError::SerializationError))
            .transpose()?;
        write(target, bytes)
    }

    fn delete(target: &str) -> Result<(), SecretStorageError> {
        let mut target_name: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();

        unsafe {
            let result = CredDeleteW(
                windows::core::PWSTR::from_raw(target_name.as_mut_ptr()),
                CRED_TYPE_GENERIC,
                None,
            );

            if let Err(error) = result {
                const ERROR_NOT_FOUND: windows::core::HRESULT =
                    windows::core::HRESULT::from_win32(windows::Win32::Foundation::ERROR_NOT_FOUND.0);
                if error.code() == ERROR_NOT_FOUND {
                    return Ok(());
                }
                return Err(error.into());
            }

            Ok(())
        }
    }

    impl PlatformSecretStorage {
        pub async fn new() -> Result<Self, SecretStorageError> {
            Ok(Self)
        }

        pub async fn read_credentials(&self, uuid: Uuid) -> Result<Option<AccountCredentials>, SecretStorageError> {
            let target_name = format!("PandoraLauncher_MinecraftAccount_{}", uuid.as_hyphenated());

            let mut account = AccountCredentials::default();

            let uuid = uuid.as_hyphenated();
            account.msa_refresh = read_deserialize(&format!("PandoraLauncher_MsaRefresh_{}", uuid))?;
            account.msa_refresh_force_client_id = read_deserialize(&format!("PandoraLauncher_MsaRefreshForceClientId_{}", uuid))?;
            account.msa_access = read_deserialize(&format!("PandoraLauncher_MsaAccess_{}", uuid))?;
            account.xbl = read_deserialize(&format!("PandoraLauncher_Xbl_{}", uuid))?;
            account.xsts = read_deserialize(&format!("PandoraLauncher_Xsts_{}", uuid))?;
            account.access_token = read_deserialize(&format!("PandoraLauncher_AccessToken_{}", uuid))?;

            Ok(Some(account))
        }

        pub async fn write_credentials(
            &self,
            uuid: Uuid,
            credentials: &AccountCredentials,
        ) -> Result<(), SecretStorageError> {
            let uuid = uuid.as_hyphenated();
            write_serialize(&format!("PandoraLauncher_MsaRefresh_{}", uuid), credentials.msa_refresh.as_ref())?;
            write_serialize(&format!("PandoraLauncher_MsaRefreshForceClientId_{}", uuid), credentials.msa_refresh_force_client_id.as_ref())?;
            write_serialize(&format!("PandoraLauncher_MsaAccess_{}", uuid), credentials.msa_access.as_ref())?;
            write_serialize(&format!("PandoraLauncher_Xbl_{}", uuid), credentials.xbl.as_ref())?;
            write_serialize(&format!("PandoraLauncher_Xsts_{}", uuid), credentials.xsts.as_ref())?;
            write_serialize(&format!("PandoraLauncher_AccessToken_{}", uuid), credentials.access_token.as_ref())?;

            Ok(())
        }

        pub async fn delete_credentials(&self, uuid: Uuid) -> Result<(), SecretStorageError> {
            let uuid = uuid.as_hyphenated();
            [
                delete(&format!("PandoraLauncher_MsaRefresh_{}", uuid)),
                delete(&format!("PandoraLauncher_MsaRefreshForceClientId_{}", uuid)),
                delete(&format!("PandoraLauncher_MsaAccess_{}", uuid)),
                delete(&format!("PandoraLauncher_Xbl_{}", uuid)),
                delete(&format!("PandoraLauncher_Xsts_{}", uuid)),
                delete(&format!("PandoraLauncher_AccessToken_{}", uuid)),
            ].into_iter().collect::<Result<(), _>>()?;

            Ok(())
        }

        pub async fn read_proxy_password(&self) -> Result<Option<String>, SecretStorageError> {
            let Some(bytes) = read("PandoraLauncher_ProxyPassword")? else {
                return Ok(None);
            };
            Ok(Some(String::from_utf8(bytes).map_err(|_| SecretStorageError::SerializationError)?))
        }

        pub async fn write_proxy_password(&self, password: &str) -> Result<(), SecretStorageError> {
            write("PandoraLauncher_ProxyPassword", Some(password.as_bytes().to_vec()))
        }

        pub async fn delete_proxy_password(&self) -> Result<(), SecretStorageError> {
            delete("PandoraLauncher_ProxyPassword")
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

    fn read(storage: &PlatformSecretStorage, target: &str) -> Result<Option<Vec<u8>>, SecretStorageError> {
        let data = match storage.keychain.find_generic_password("com.moulberry.pandoralauncher", target) {
            Ok((data, _)) => data,
            Err(error) if error.code() == security_framework_sys::base::errSecItemNotFound => {
                return Ok(None);
            },
            Err(error) => {
                return Err(error.into());
            }
        };
        Ok(Some(data.to_owned()))
    }

    fn read_deserialize<T: for<'a> serde::Deserialize<'a>>(storage: &PlatformSecretStorage, target: &str) -> Result<Option<T>, SecretStorageError> {
        let Some(bytes) = read(storage, target)? else {
            return Ok(None);
        };
        Ok(Some(serde_json::from_slice(&bytes).map_err(|_| SecretStorageError::SerializationError)?))
    }

    fn write(storage: &PlatformSecretStorage, target: &str, bytes: &[u8]) -> Result<(), SecretStorageError> {
        storage.keychain.set_generic_password("com.moulberry.pandoralauncher", target, bytes)?;
        Ok(())
    }

    fn write_serialize(storage: &PlatformSecretStorage, target: &str, data: &impl serde::Serialize) -> Result<(), SecretStorageError> {
        let bytes = serde_json::to_vec(data).map_err(|_| SecretStorageError::SerializationError)?;
        write(storage, target, &bytes)
    }

    fn delete(storage: &PlatformSecretStorage, target: &str) -> Result<(), SecretStorageError> {
        let item = match storage.keychain.find_generic_password("com.moulberry.pandoralauncher", target) {
            Ok((_, item)) => item,
            Err(error) if error.code() == security_framework_sys::base::errSecItemNotFound => {
                return Ok(());
            },
            Err(error) => {
                return Err(error.into());
            }
        };

        item.delete();
        Ok(())
    }

    impl PlatformSecretStorage {
        pub async fn new() -> Result<Self, SecretStorageError> {
            Ok(Self {
                keychain: SecKeychain::default_for_domain(SecPreferencesDomain::User)?
            })
        }

        pub async fn read_credentials(&self, uuid: Uuid) -> Result<Option<AccountCredentials>, SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            read_deserialize(self, uuid_str.as_str())
        }

        pub async fn write_credentials(
            &self,
            uuid: Uuid,
            credentials: &AccountCredentials,
        ) -> Result<(), SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            write_serialize(self, uuid_str.as_str(), credentials)
        }

        pub async fn delete_credentials(&self, uuid: Uuid) -> Result<(), SecretStorageError> {
            let uuid_str = uuid.as_hyphenated().to_string();
            delete(self, uuid_str.as_str())
        }

        pub async fn read_proxy_password(&self) -> Result<Option<String>, SecretStorageError> {
            let Some(bytes) = read(self, "proxy-password")? else {
                return Ok(None);
            };
            Ok(Some(String::from_utf8(bytes).map_err(|_| SecretStorageError::SerializationError)?))
        }

        pub async fn write_proxy_password(&self, password: &str) -> Result<(), SecretStorageError> {
            write(self, "proxy-password", password.as_bytes())
        }

        pub async fn delete_proxy_password(&self) -> Result<(), SecretStorageError> {
            delete(self, "proxy-password")
        }
    }
}
