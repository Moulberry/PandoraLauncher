// Code adapted from https://gist.github.com/ThatGravyBoat/fcdab4a3562b082f82e09e6263cc0210
// Licensed as MIT Copyright (c) 2026 ThatGravyBoat

use std::{sync::Arc, time::{Duration, Instant}};

use bridge::instance::InstanceID;
use hickory_resolver::name_server::TokioConnectionProvider;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use rc_zip_sync::ReadZip;
use rustc_hash::{FxHashMap, FxHashSet};
use schema::server_status::ServerStatus;
use tokio::net::TcpStream;
use ustr::Ustr;
use std::io::{Cursor, Error, ErrorKind};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::BackendState;

const MINECRAFT_PORT: u16 = 25565;
const TIMEOUT: Duration = Duration::from_secs(5);
const FALLBACK_PROTOCOL_VERSION: i32 = 774;

pub struct ServerListPinger {
    data: Arc<RwLock<FxHashMap<(Arc<str>, i32), PingEntry>>>,
    start: Instant,
    resolver: OnceCell<Option<Box<hickory_resolver::Resolver<TokioConnectionProvider>>>>,
    protocol_versions: Arc<RwLock<FxHashMap<Ustr, i32>>>,
}

enum PingEntry {
    Loading {
        instances: FxHashSet<InstanceID>,
    },
    Loaded {
        status: Arc<ServerStatus>,
        ping: Option<Duration>
    },
    Failed,
}

pub enum PingResult {
    Pinging,
    Loaded {
        status: Arc<ServerStatus>,
        ping: Option<Duration>
    },
    Error,
}

impl ServerListPinger {
    pub fn new() -> Self {
        Self {
            data: Default::default(),
            start: Instant::now(),
            resolver: OnceCell::new(),
            protocol_versions: Default::default(),
        }
    }

    fn load_minecraft_data_version(file: std::fs::File) -> Option<i32> {
        let archive = file.read_zip().ok()?;
        let version_json = archive.by_name("version.json")?;
        let bytes = version_json.bytes().ok()?;

        let Ok(serde_json::Value::Object(json)) = serde_json::from_slice(&bytes) else {
            return None
        };

        let protocol_version = json.get("protocol_version")?;
        protocol_version.as_i64().map(|v| v as i32)
    }

    pub fn load_status(backend: &Arc<BackendState>, server: Arc<str>, version: Ustr, instance: InstanceID) -> PingResult {
        let protocol_version = {
            let mut protocol_versions = backend.server_list_pinger.protocol_versions.upgradable_read();
            let mut protocol_version = protocol_versions.get(&version).copied();

            if protocol_version.is_none() {
                let minecraft_jar_pathname = format!("net/minecraft/{0}/minecraft-client-{0}.jar", version);
                let minecraft_jar_path = backend.directories.libraries_dir.join(minecraft_jar_pathname);
                if let Ok(minecraft_jar) = std::fs::File::open(minecraft_jar_path) {
                    let protocol_version_number = Self::load_minecraft_data_version(minecraft_jar).unwrap_or(FALLBACK_PROTOCOL_VERSION);
                    protocol_versions.with_upgraded(|upgraded| {
                        upgraded.insert(version, protocol_version_number);
                    });
                    protocol_version = Some(protocol_version_number);
                }
            }

            protocol_version.unwrap_or(FALLBACK_PROTOCOL_VERSION)
        };

        let key = (server.clone(), protocol_version);
        {
            let mut data = backend.server_list_pinger.data.write();
            if let Some(existing) = data.get_mut(&key) {
                match existing {
                    PingEntry::Loading { instances } => {
                        instances.insert(instance);
                        return PingResult::Pinging;
                    },
                    PingEntry::Loaded { status, ping } => {
                        return PingResult::Loaded { status: status.clone(), ping: ping.clone() };
                    },
                    PingEntry::Failed => {
                        return PingResult::Error;
                    },
                }
            }
            let mut instances = FxHashSet::default();
            instances.insert(instance);
            data.insert(key.clone(), PingEntry::Loading { instances });
        }

        let backend = backend.clone();
        tokio::spawn(async move {
            let (host, port) = if let Some((host, port)) = server.split_once(':') {
                let Ok(port) = port.parse::<u16>() else {
                    backend.server_list_pinger.data.write().insert(key.clone(), PingEntry::Failed);
                    return;
                };
                (host, Some(port))
            } else {
                (&*server, None)
            };
            let status = backend.server_list_pinger.request_status(host, port, protocol_version).await;
            let Ok((status, ping)) = status else {
                backend.server_list_pinger.data.write().insert(key.clone(), PingEntry::Failed);
                return;
            };

            let entry = PingEntry::Loaded { status: Arc::new(status), ping };
            let old_status = backend.server_list_pinger.data.write().insert(key.clone(), entry);

            if let Some(PingEntry::Loading { instances }) = old_status {
                let mut instance_state = backend.instance_state.write();
                for instance in instances {
                    if let Some(instance) = instance_state.instances.get_mut(instance) {
                        instance.mark_servers_dirty(&backend, true);
                    }
                }
            }
        });

        PingResult::Pinging
    }

    pub async fn request_status_as_string(&self, host: &str, port: Option<u16>, protocol: i32) -> std::io::Result<(String, Option<Duration>)> {
        let port = port.unwrap_or(MINECRAFT_PORT);
        let (host, port) = if port == MINECRAFT_PORT && let Some(result) = self.srv_lookup(host).await {
            result
        } else {
            (host.to_string(), port)
        };

        let address = format!("{host}:{port}");
        let (response, stream) = tokio::time::timeout(TIMEOUT, async {
            let mut stream = TcpStream::connect(address).await?;

            stream.set_nodelay(true)?;

            let handshake = ServerboundPacket::Handshake {
                pvn: protocol,
                host,
                port,
                intention: 1, // Status
            };
            let status_request = ServerboundPacket::StatusRequest;

            handshake.write_to_stream(&mut stream).await?;
            status_request.write_to_stream(&mut stream).await?;
            stream.flush().await?;

            let response = ClientboundPacket::read_from_stream(&mut stream).await?;
            std::io::Result::Ok((response, stream))
        }).await??;

        match response {
            ClientboundPacket::StatusResponse { response } => {
                let ping = tokio::time::timeout(TIMEOUT, self.request_ping(stream))
                    .await.ok().map(Result::ok).flatten();

                Ok((response, ping))
            },
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Expected status response, got other packet"),
            ))
        }
    }

    async fn request_ping(&self, mut stream: TcpStream) -> std::io::Result<Duration> {
        let ping_start = Instant::now();
        let time = ping_start.checked_duration_since(self.start).map(|d| d.as_millis()).unwrap_or(0);
        let ping_request = ServerboundPacket::PingRequest {
            id: time as i64
        };

        ping_request.write_to_stream(&mut stream).await?;
        stream.flush().await?;

        let response = ClientboundPacket::read_from_stream(&mut stream).await?;

        match response {
            ClientboundPacket::PingResponse { _id } => {
                Ok(Instant::now() - ping_start)
            },
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Expected ping response, got other packet"),
            ))
        }
    }

    async fn request_status(&self, address: &str, port: Option<u16>, protocol: i32) -> std::io::Result<(ServerStatus, Option<Duration>)> {
        let (response, ping) = self.request_status_as_string(address, port, protocol).await?;
        let status = serde_json::from_str(&response)?;
        Ok((status, ping))
    }

    async fn srv_lookup(&self, host: &str) -> Option<(String, u16)> {
        let resolver = self.resolver.get_or_init(|| {
            Some(Box::new(hickory_resolver::Resolver::builder_tokio().ok()?.build()))
        }).as_ref()?;
        let query = &format!("_minecraft._tcp.{}", host);
        let lookup = tokio::time::timeout(TIMEOUT, resolver.srv_lookup(query))
            .await
            .ok()?
            .ok()?;

        let record = lookup.iter().next()?;
        let mut host = record.target().to_utf8();
        if host.ends_with('.') {
            host.pop();
        }

        Some((host, record.port()))
    }
}

const VAR_INT_SECTION_BITS: u32 = 0x7F;
const VAR_INT_SECTION_CONTINUE_BIT: u32 = 0x80;

type VarInt = i32;

trait PacketBufferExt {
    async fn write_varint(&mut self, value: impl Into<VarInt>) -> std::io::Result<()>;
    async fn write_string(&mut self, value: impl Into<&str>, max_length: Option<usize>) -> std::io::Result<()>;

    async fn read_varint(&mut self) -> std::io::Result<VarInt>;
    async fn read_string(&mut self, max_length: Option<usize>) -> std::io::Result<String>;
}

impl <Buffer : AsyncReadExt + AsyncWriteExt + Unpin> PacketBufferExt for Buffer {
    async fn write_varint(&mut self, value: impl Into<VarInt>) -> std::io::Result<()> {
        let mut value = value.into() as u32;
        loop {
            if (value & !VAR_INT_SECTION_BITS) == 0 {
                self.write_u8(value as u8).await?;
                return Ok(());
            } else {
                self.write_u8(((value & VAR_INT_SECTION_BITS) | VAR_INT_SECTION_CONTINUE_BIT) as u8).await?;
                value >>= 7;
            }
        }
    }

    async fn write_string(&mut self, value: impl Into<&str>, max_length: Option<usize>) -> std::io::Result<()> {
        let bytes = value.into().as_bytes();
        let length = bytes.len();
        if let Some(max) = max_length && length > max {
            return Err(Error::new(
                ErrorKind::QuotaExceeded,
                format!("String length {} exceeds maximum of {}", length, max),
            ));
        }

        self.write_varint(length as i32).await?;
        self.write_all(bytes).await?;
        Ok(())
    }

    async fn read_varint(&mut self) -> std::io::Result<VarInt> {
        let mut value: i32 = 0;
        let mut position: u32 = 0;
        let mut byte: u8;

        loop {
            byte = self.read_u8().await?;
            value |= ((byte & VAR_INT_SECTION_BITS as u8) as i32) << position;

            if (byte & VAR_INT_SECTION_CONTINUE_BIT as u8) == 0 {
                return Ok(value);
            }

            position += 7;
            if position >= 32 {
                return Err(Error::new(ErrorKind::InvalidData, "VarInt is too big"));
            }
        }
    }

    async fn read_string(&mut self, max_length: Option<usize>) -> std::io::Result<String> {
        let length = self.read_varint().await? as usize;
        if let Some(max) = max_length && length > max {
            return Err(Error::new(
                ErrorKind::QuotaExceeded,
                format!("String length {} exceeds maximum of {}", length, max),
            ));
        }

        let mut buffer = vec![0; length];
        self.read_exact(&mut buffer).await?;
        match String::from_utf8(buffer) {
            Ok(s) => Ok(s),
            Err(_) => Err(Error::new(ErrorKind::InvalidData, "Invalid UTF-8 string")),
        }
    }
}

#[derive(Debug)]
enum ServerboundPacket {
    Handshake {
        pvn: i32,
        host: String,
        port: u16,
        intention: i32,
    },
    StatusRequest,
    PingRequest {
        id: i64,
    }
}

#[derive(Debug)]
enum ClientboundPacket {
    StatusResponse {
        response: String,
    },
    PingResponse {
        _id: i64,
    }
}

impl ServerboundPacket {
    pub async fn write_to_stream(&self, stream: &mut TcpStream) -> std::io::Result<()> {
        let inner = self.write_packet().await?;
        stream.write_varint(inner.len() as i32).await?;
        stream.write_all(&inner).await?;

        Ok(())
    }

    async fn write_packet(&self) -> std::io::Result<Vec<u8>> {
        let mut buffer = Cursor::new(Vec::new());

        match self {
            ServerboundPacket::Handshake { pvn: protocol_version, host: host_name, port, intention } => {
                buffer.write_varint(0x00).await?; // Packet ID for Handshake
                buffer.write_varint(*protocol_version).await?;
                buffer.write_string(host_name.as_str(), Some(255)).await?;
                buffer.write_u16(*port).await?;
                buffer.write_varint(*intention).await?;
            },
            ServerboundPacket::StatusRequest => {
                buffer.write_varint(0x00).await?; // Packet ID for Status Request
            },
            ServerboundPacket::PingRequest { id } => {
                buffer.write_varint(0x01).await?; // Packet ID for Ping Request
                buffer.write_i64(*id).await?;
            },
        }

        Ok(buffer.into_inner())
    }
}

impl ClientboundPacket {
    async fn read_from_stream(stream: &mut TcpStream) -> std::io::Result<ClientboundPacket> {
        let length = stream.read_varint().await?;

        if length <= 0 {
            return Err(Error::new(ErrorKind::UnexpectedEof, "Packet length cannot be zero"));
        } else if length > 100000 {
            return Err(Error::new(ErrorKind::UnexpectedEof, "Packet length cannot be greater than 100kb"));
        }

        let mut buffer = vec![0; length as usize];
        stream.read_exact(&mut buffer).await?;

        Self::read_packet(buffer).await
    }

    async fn read_packet(buffer: Vec<u8>) -> std::io::Result<Self> {
        let mut buffer = Cursor::new(buffer);
        let packet_id = buffer.read_varint().await?;

        match packet_id {
            0x00 => { // Packet ID for Status Response
                let response = buffer.read_string(Some(32767)).await?;
                Ok(ClientboundPacket::StatusResponse { response })
            },
            0x01 => { // Packet ID for Ping Response
                let id = buffer.read_i64().await?;
                Ok(ClientboundPacket::PingResponse { _id: id })
            },
            _ => Err(Error::new(
                ErrorKind::InvalidData,
                format!("Unexpected packet ID: {}", packet_id),
            )),
        }
    }
}
