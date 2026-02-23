use enumset::{EnumSet, EnumSetType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct BackendConfig {
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub sync_targets: EnumSet<SyncTarget>,
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub dont_open_game_output_when_launching: bool,
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub proxy: ProxyConfig,
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ProxyConfig {
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub protocol: ProxyProtocol,
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub host: String,
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub port: u16,
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub auth_enabled: bool,
    #[serde(default, skip_serializing_if = "crate::skip_if_default", deserialize_with = "crate::try_deserialize")]
    pub username: String,
}

impl ProxyConfig {
    pub fn to_url(&self, password: Option<&str>) -> Option<String> {
        if !self.enabled || self.host.is_empty() {
            return None;
        }

        let scheme = self.protocol.scheme();

        if self.auth_enabled && !self.username.is_empty() {
            let password = password.unwrap_or("");
            // URL-encode username and password to handle special characters
            let username = urlencoding::encode(&self.username);
            let password = urlencoding::encode(password);
            Some(format!("{}://{}:{}@{}:{}", scheme, username, password, self.host, self.port))
        } else {
            Some(format!("{}://{}:{}", scheme, self.host, self.port))
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum ProxyProtocol {
    #[default]
    Http,
    Https,
    Socks5,
}

impl ProxyProtocol {
    pub fn scheme(&self) -> &'static str {
        match self {
            ProxyProtocol::Http => "http",
            ProxyProtocol::Https => "https",
            ProxyProtocol::Socks5 => "socks5",
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ProxyProtocol::Http => "HTTP",
            ProxyProtocol::Https => "HTTPS",
            ProxyProtocol::Socks5 => "SOCKS5",
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "HTTP" => ProxyProtocol::Http,
            "HTTPS" => ProxyProtocol::Https,
            "SOCKS5" => ProxyProtocol::Socks5,
            _ => ProxyProtocol::Http,
        }
    }
}

#[derive(Debug, enum_map::Enum, EnumSetType, strum::EnumIter)]
pub enum SyncTarget {
    Options = 0,
    Servers = 1,
    Commands = 2,
    Hotbars = 13,
    Saves = 3,
    Config = 4,
    Screenshots = 5,
    Resourcepacks = 6,
    Shaderpacks = 7,
    Flashback = 8,
    DistantHorizons = 9,
    Voxy = 10,
    XaerosMinimap = 11,
    Bobby = 12,
    Litematic = 14,
}

impl SyncTarget {
    pub fn get_folder(self) -> Option<&'static str> {
        match self {
            SyncTarget::Options => None,
            SyncTarget::Servers => None,
            SyncTarget::Commands => None,
            SyncTarget::Hotbars => None,
            SyncTarget::Saves => Some("saves"),
            SyncTarget::Config => Some("config"),
            SyncTarget::Screenshots => Some("screenshots"),
            SyncTarget::Resourcepacks => Some("resourcepacks"),
            SyncTarget::Shaderpacks => Some("shaderpacks"),
            SyncTarget::Flashback => Some("flashback"),
            SyncTarget::DistantHorizons => Some("Distant_Horizons_server_data"),
            SyncTarget::Voxy => Some(".voxy"),
            SyncTarget::XaerosMinimap => Some("xaero"),
            SyncTarget::Bobby => Some(".bobby"),
            SyncTarget::Litematic => Some("schematics"),
        }
    }
}
