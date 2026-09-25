use std::{io::ErrorKind, net::{IpAddr, Ipv4Addr, SocketAddr}, path::{Path, PathBuf}, str::FromStr, sync::Arc};

use futures::StreamExt;
use parking_lot::Mutex;
use reqwest::{StatusCode, header::{HeaderMap, HeaderName, HeaderValue}};
use rustc_hash::{FxHashMap, FxHashSet};
use serde::{Deserialize, Serialize};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::TcpStream};
use url::Url;
use uuid::Uuid;

pub enum WindowIdentifier<'a> {
    WindowsHWND(usize),
    WaylandExportHandleString(&'a str),
    X11WindowNumber(u64),
    MacOSNSWindowPtr(usize),
}

pub struct OpenFileDialogFilter<'a> {
    user_friendly_name: &'a str,
    glob_pattern: &'a str,
}

pub struct OpenFileDialogArgs<'a> {
    /// Title of the dialog window
    title: &'a str,
    /// Label for the accept button
    accept_label: Option<&'a str>,
    /// Label for the cancel button
    cancel_label: Option<&'a str>,
    /// Window that the file dialog should be modal for
    modal_for: Option<WindowIdentifier<'a>>,
    /// Whether the dialog should be a save dialog instead of an open dialog
    save: bool,
    /// Whether multiple files can be selected
    multiple: bool,
    /// Whether folders should be selected instead of files
    directory: bool,
    /// List of filters to apply
    filters: &'a [OpenFileDialogFilter<'a>],
    /// Default path (may be folder or file)
    ///
    /// Security note: May be automatically filtered to avoid malware from
    /// opening a file dialog pointing directly at sensitive files
    default_location: Option<&'a str>,
}

pub enum OpenFileDialogResult {
    Forbidden,
    Success {
        /// The path(s) selected by the user, empty if the operation was cancelled
        paths: Vec<PathBuf>,
        /// The filter that was chosen by the user
        filter: usize,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum KnownDragAndDropFile {
    Resourcepack,
    Datapack,
    Shaderpack,
    Mod,
    FlashbackReplay,
    ReplayModReplay,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum IKnowWhatIAmDoingArbitraryFileAccess {
    Yes,
    No,
}

pub trait SandboxApi {
    /// Open the given url in a web browser
    /// Returns true on success
    ///
    /// Implementation notes:
    /// Should not be called on macOS, since sandboxed opens are handled correctly by the OS
    fn open_url(&self, url: Url) -> impl std::future::Future<Output = bool> + std::marker::Send;

    /// Open the given folder in a file browser
    /// Returns true on success
    ///
    /// Implementation notes:
    /// Should not be called on macOS, since sandboxed opens are handled correctly by the OS
    async fn open_folder(&self, folder: PathBuf) -> bool;

    /// Open the given file using the default handler (e.g. an image viewer for pngs, a video player for mp4s)
    /// Returns true on success
    ///
    /// Implementation notes:
    /// Should not be called on macOS, since sandboxed opens are handled correctly by the OS
    /// The service will only call this for files that it is confident are safe (e.g. images, video, etc.)
    async fn open_file(&self, file: PathBuf) -> bool;

    /// Open a file dialog
    ///
    /// Implementation notes:
    /// Should not be called on macOS, since sandboxed NSOpenPanels are handled correctly by the OS
    ///
    /// The service may choose to reject the paths if it believes the files are sensitive
    ///
    /// Any files which are not exposed to the sandbox will be copied into a temp
    /// share folder prior to being sent back to the game.
    /// If 'save' is true, the service will listen for file changes and copy the temp
    /// file back to the source
    ///
    /// For folders, the behaviour is platform-dependent:
    /// - Windows: The user will be granted read-write access to the folder directly
    /// - Linux: The folder will be bind mounted into the temp share folder
    async fn open_file_dialog<'a>(&self, args: OpenFileDialogArgs<'a>) -> OpenFileDialogResult;

    /// Whether to allow a /session/minecraft/join request
    /// May be used to display a confirmation dialog to the user
    /// Returning false will make the request return 403 Forbidden
    ///
    /// Security: While the sandbox may protect the access token directly,
    /// malicious mods can still use the game to authenticate on their behalf.
    /// Servers may enable prevent-proxy-connections in server.properties to make
    /// this more difficult to do in practice (but still not impossible!)
    async fn should_allow_join_server<'a>(&self, uuid: Uuid, server: &'a str) -> bool;

    /// Whether to allow read access to an arbitrary file on the system
    ///
    /// This is used in order to allow file drag-and-drop to work on Windows/Linux.
    /// This is a dangerous function, since malware can send a request for an arbitrary path.
    /// By default, we only allow the access if we can verify that the file format is one which
    /// we know to be used by the game for drag-and-drop (resourcepacks, shaderpacks, etc.)
    ///
    /// Implementation note:
    /// If the sandbox requests access to a file that doesn't exist, it will be blocked
    /// from all future requests
    async fn should_allow_arbitrary_file_access(&self, _path: &Path, known: Option<KnownDragAndDropFile>) -> IKnowWhatIAmDoingArbitraryFileAccess {
        if known.is_some() {
            IKnowWhatIAmDoingArbitraryFileAccess::Yes
        } else {
            IKnowWhatIAmDoingArbitraryFileAccess::No
        }
    }
}

#[derive(Clone)]
pub struct AccessTokenReplacement {
    pub dummy: Arc<str>,
    pub real: Arc<str>,
}

const DEFAULT_SESSION_JOIN: &'static str = "https://sessionserver.mojang.com/session/minecraft/join";

#[derive(Default)]
pub struct ReplaceAccessTokenEndpoints {
    custom_session_join: Option<Url>,
    replace_authorization_header: FxHashMap<Arc<str>, Url>,
}

pub struct SandboxApiService<S: SandboxApi + Sync + Send + 'static> {
    reuse_buffer: Arc<Mutex<Option<Vec<u8>>>>,
    replace_access_token_endpoints: Arc<Mutex<ReplaceAccessTokenEndpoints>>,
    secret: Arc<str>,
    api: Arc<S>,
    port: u16,
    access_token_replacement: Option<AccessTokenReplacement>,
    http_client: reqwest::Client,
}

impl <S: SandboxApi + Sync + Send + 'static> Clone for SandboxApiService<S> {
    fn clone(&self) -> Self {
        Self {
            reuse_buffer: self.reuse_buffer.clone(),
            replace_access_token_endpoints: self.replace_access_token_endpoints.clone(),
            secret: self.secret.clone(),
            api: self.api.clone(),
            port: self.port,
            access_token_replacement: self.access_token_replacement.clone(),
            http_client: self.http_client.clone(),
        }
    }
}

impl <S: SandboxApi + Sync + Send + 'static> SandboxApiService<S> {
    pub fn new(secret: &str, api: S, port: u16, access_token_replacement: Option<AccessTokenReplacement>, http_client: reqwest::Client) -> Self {
        Self {
            reuse_buffer: Default::default(),
            replace_access_token_endpoints: Default::default(),
            secret: secret.trim_ascii().into(),
            api: Arc::new(api),
            port,
            access_token_replacement,
            http_client,
        }
    }

    pub async fn run_localhost_server(self: Self) -> std::io::Result<()> {
        let socket_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), self.port);
        log::info!("Starting sandboxapi service on {}", socket_addr);
        let listener = tokio::net::TcpListener::bind(socket_addr).await?;

        loop {
            let (stream, _addr) = listener.accept().await?;
            tokio::task::spawn(self.clone().handle_incoming(stream));
        }
    }

    async fn handle_incoming(self: Self, stream: TcpStream) -> std::io::Result<()> {
        let mut buf = self.reuse_buffer.lock().take().unwrap_or_else(|| vec![0_u8; 1024]);
        let res = self.clone().handle_incoming_with_buffer(stream, &mut buf).await;
        match &mut *self.reuse_buffer.lock() {
            Some(old) => if old.len() < buf.len() {
                *old = buf;
            },
            v => {
                *v = Some(buf)
            },
        }
        res
    }

    async fn handle_incoming_with_buffer(self: Self, mut stream: TcpStream, buf: &mut Vec<u8>) -> std::io::Result<()> {
        let mut read = 0;
        'read_loop: loop {
            let n =  stream.read(&mut buf[read..]).await?;
            read += n;

            if read == 0 {
                return Ok(());
            }

            if read == buf.len() {
                buf.resize(buf.len() * 2, 0);
                continue;
            }

            let mut headers = [httparse::EMPTY_HEADER; 32];
            let mut request = httparse::Request::new(&mut headers);
            let Ok(parsed) = request.parse(&buf[..read]) else {
                return Err(ErrorKind::InvalidData.into());
            };

            let httparse::Status::Complete(body_offset) = parsed else {
                if n == 0 {
                    return Err(ErrorKind::InvalidData.into());
                } else {
                    continue;
                }
            };

            // Make sure we're read enough content according to Content-Length
            if n > 0 {
                for header in request.headers.iter() {
                    if !header.name.eq_ignore_ascii_case("Content-Length") {
                        continue;
                    }
                    let Ok(content_length_str) = str::from_utf8(header.value) else {
                        break;
                    };
                    let Ok(content_length) = content_length_str.parse::<usize>() else {
                        break;
                    };
                    let received_length = read - body_offset;
                    if received_length < content_length {
                        continue 'read_loop;
                    }
                    break;
                }
            }

            self.handle_request(request, &buf[body_offset..read], &mut stream).await;
            return Ok(());
        }
    }

    async fn handle_request(self: Self, request: httparse::Request<'_, '_>, body: &[u8], stream: &mut TcpStream) {
        log::info!("Got {:?}", &request.path);
        let Some(path) = request.path else {
            RequestStatus::NotFound.write(stream).await;
            return;
        };
        let (path, params) = path.split_once('?').unwrap_or((path, ""));

        // We check for this authorization header in order to prevent other applications
        // on the system from interacting with our API. This isn't strictly necessary, but
        // is good practice
        if path.starts_with("/sandboxapi/") && let Some(error_status) = self.clone().check_authorization_secret(&request) {
            error_status.write(stream).await;
            return;
        }

        match path {
            "/sandboxapi/openUri" => {
                if request.method != Some("POST") {
                    RequestStatus::MethodNotAllowed.write(stream).await;
                    return;
                }
                self.handle_request_open_uri(params).await;
            },
            "/sandboxauth/session/session/minecraft/join" => {
                let Ok(mut join_request) = serde_json::from_slice::<MinecraftJoinRequest>(body) else {
                    RequestStatus::BadRequest.write(stream).await;
                    return;
                };

                if let Some(replacement) = &self.access_token_replacement {
                    if join_request.access_token == replacement.dummy {
                        join_request.access_token = replacement.real.clone();
                    }
                }

                let Ok(new_body) = serde_json::to_vec(&join_request) else {
                    RequestStatus::InternalServerError.write(stream).await;
                    return;
                };


                let to = self.replace_access_token_endpoints.lock().custom_session_join.clone()
                    .or_else(|| Url::parse(DEFAULT_SESSION_JOIN).ok());

                let Some(to) = to else {
                    RequestStatus::InternalServerError.write(stream).await;
                    return;
                };

                self.proxy_with_replacement(&request, new_body, to, stream).await;
                return;
            },
            "/sandboxauth/discovery" => {
                let Ok(discovery) = self.http_client.get("https://discovery.minecraftservices.com/minecraft/client").send().await else {
                    RequestStatus::InternalServerError.write(stream).await;
                    return;
                };

                let mut discovery_result = match discovery.json::<MinecraftDiscoveryResult>().await {
                    Ok(discovery_result) => discovery_result,
                    Err(err) => {
                        log::error!("Failed to get discovery: {:?}", err);
                        RequestStatus::InternalServerError.write(stream).await;
                        return;
                    },
                };

                log::info!("Before: {}", serde_json::to_string(&discovery_result).unwrap());

                discovery_result.environment = Some("sandboxauth".into());
                for (service_name, endpoints) in &mut discovery_result.discovery.services {
                    for (endpoint_name, endpoint) in &mut endpoints.endpoints {
                        let Some(old_uri) = &endpoint.uri else {
                            continue;
                        };
                        if old_uri.contains('{') && old_uri.contains('}') {
                            // Skip URIs with path replacements for now, currently these are only
                            // used for profile/skin lookups which are unauthenticated anyways
                            continue;
                        }
                        let Ok(old_url) = Url::parse(old_uri) else {
                            continue;
                        };
                        if !old_url.has_host() {
                            continue;
                        }

                        // Custom behaviour for session join since it puts the access token in the
                        // body instead of the Authorization header
                        if &**service_name == "session" && &**endpoint_name == "join" {
                            // Update custom_session_join
                            self.replace_access_token_endpoints.lock().custom_session_join = if &**old_uri == DEFAULT_SESSION_JOIN {
                                None
                            } else {
                                Some(old_url)
                            };

                            // Override join with our custom endpoint
                            *endpoint = MinecraftEndpoint {
                                uri: Some(format!("http://127.0.0.1:{}/sandboxauth/session/session/minecraft/join", self.port).into()),
                                valid_uris: None,
                                other_values: Default::default()
                            };
                        } else {
                            // Create new endpoint string
                            let new_endpoint_path: Arc<str> = format!("/sandboxauth/service/{service_name}/{endpoint_name}").into();
                            let new_endpoint: Arc<str> = format!("http://127.0.0.1:{}{}", self.port, new_endpoint_path).into();

                            self.replace_access_token_endpoints.lock().replace_authorization_header.insert(new_endpoint_path.clone(), old_url);

                            *endpoint = MinecraftEndpoint {
                                uri: Some(new_endpoint),
                                valid_uris: None,
                                other_values: Default::default()
                            };
                        }
                    }
                }

                log::info!("After: {}", serde_json::to_string(&discovery_result).unwrap());

                if write_json_response(stream, &discovery_result).await.is_err() {
                    RequestStatus::InternalServerError.write(stream).await;
                }
                return;
            },
            _ => {
                if path.starts_with("/sandboxauth/service") {
                    let to = self.replace_access_token_endpoints.lock().replace_authorization_header.get(path).cloned();
                    if let Some(to) = to {
                        self.proxy_with_replacement(&request, body.to_vec(), to, stream).await;
                        return;
                    }
                }
                RequestStatus::NotFound.write(stream).await;
            }
        }
    }

    fn create_headers_with_replacement(&self, request: &httparse::Request<'_, '_>) -> HeaderMap {
        let mut headers = HeaderMap::new();

        for old_header in request.headers.iter() {
            if old_header.name.eq_ignore_ascii_case("Host")
                || old_header.name.eq_ignore_ascii_case("Content-Length")
                || old_header.name.eq_ignore_ascii_case("Connection")
            {
                // Reqwest will automatically set these headers
                continue;
            }
            if old_header.name.eq_ignore_ascii_case("Authorization") && let Some(replacement) = &self.access_token_replacement {
                if let Some(bearer) = old_header.value.strip_prefix(b"Bearer") {
                    if bearer.trim_ascii_start() == replacement.dummy.as_bytes() {
                        if let Ok(header_value) = HeaderValue::from_str(&format!("Bearer {}", replacement.real)) {
                            headers.insert("Authorization", header_value);
                        }
                        continue;
                    }
                }
            }
            if let Ok(header_name) = HeaderName::from_str(old_header.name) {
                if let Ok(header_value) = HeaderValue::from_bytes(old_header.value) {
                    headers.insert(header_name, header_value);
                }
            }
        }

        headers
    }

    async fn proxy_with_replacement(self: Self, request: &httparse::Request<'_, '_>, body: Vec<u8>, to: Url, stream: &mut TcpStream) {
        let headers = self.create_headers_with_replacement(request);
        let method = request.method
            .and_then(|method| reqwest::Method::from_bytes(method.as_bytes()).ok())
            .unwrap_or(reqwest::Method::GET);
        dbg!(request);
        dbg!(body.len());//reqwest::header::HOST
        let Ok(response) = dbg!(self.http_client.request(method, to).headers(headers).body(body)).send().await else {
            RequestStatus::InternalServerError.write(stream).await;
            return;
        };
        dbg!(response.status());

        let mut builder = Vec::from("HTTP/1.1 ");
        builder.extend_from_slice(response.status().as_str().as_bytes());
        if let Some(canonical_reason) = response.status().canonical_reason() {
            builder.push(b' ');
            builder.extend_from_slice(canonical_reason.as_bytes());
        }
        builder.extend_from_slice(b"\r\n");

        for (name, value) in response.headers() {
            builder.extend_from_slice(name.as_str().as_bytes());
            builder.extend_from_slice(b": ");
            builder.extend_from_slice(value.as_bytes());
            builder.extend_from_slice(b"\r\n");
        }
        builder.extend_from_slice(b"\r\n");

        if stream.write_all(&builder).await.is_err() {
            return;
        }

        // Pipe from input stream to output stream
        let mut input_stream = response.bytes_stream();
        while let Some(Ok(item)) = input_stream.next().await {
            if stream.write_all(&item).await.is_err() {
                return;
            }
        }
    }

    fn check_authorization_secret(self: Self, request: &httparse::Request<'_, '_>) -> Option<RequestStatus> {
        for header in request.headers.iter() {
            if header.name == "Authorization" {
                let Some(token) = header.value.trim_ascii_start().strip_prefix(b"Bearer") else {
                    return Some(RequestStatus::Forbidden);
                };
                if token.trim_ascii_start() == self.secret.as_bytes() {
                    return None;
                } else {
                    return Some(RequestStatus::Forbidden);
                }
            }
        }
        Some(RequestStatus::Unauthorized)
    }

    async fn handle_request_open_uri(self: Self, params: &str) -> RequestStatus {
        let mut uri = None;
        for (key, value) in params.split("&").filter_map(|arg| arg.split_once('=')) {
            if key == "uri" {
                uri = Some(value);
                break;
            }
        }

        let Some(encoded) = uri else {
            return RequestStatus::BadRequest;
        };

        let Ok(decoded) = urlencoding::decode(encoded) else {
            return RequestStatus::BadRequest;
        };

        let Ok(url) = url::Url::parse(&*decoded) else {
            return RequestStatus::BadRequest;
        };

        // Allow http and https
        if url.scheme() == "http" || url.scheme() == "https" {
            if self.api.open_url(url).await {
                return RequestStatus::OK;
            } else {
                return RequestStatus::InternalServerError;
            }
        } else if url.scheme() == "file" && url.host_str().is_none() {
            let Ok(path) = url.to_file_path() else {
                return RequestStatus::BadRequest;
            };
            let Ok(path) = path.canonicalize() else {
                return RequestStatus::BadRequest;
            };

            // We need to copy the file to a directory that the sandbox doesn't have write access to
            // before checking the mime type.
            // todo: finish comment

            todo!();
        } else {
            return RequestStatus::Forbidden;
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct MinecraftDiscoveryResult {
    #[serde(skip_serializing_if = "skip_if_none")]
    environment: Option<Arc<str>>,
    discovery: MinecraftDiscovery,

    #[serde(flatten)]
    other_values: FxHashMap<Arc<str>, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MinecraftDiscovery {
    #[serde(skip_serializing_if = "skip_if_none")]
    product: Option<Arc<str>>,
    #[serde(flatten)]
    services: FxHashMap<Arc<str>, MinecraftEndpoints>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MinecraftEndpoints {
    endpoints: FxHashMap<Arc<str>, MinecraftEndpoint>,

    #[serde(flatten)]
    other_values: FxHashMap<Arc<str>, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftEndpoint {
    #[serde(skip_serializing_if = "skip_if_none")]
    uri: Option<Arc<str>>,
    #[serde(skip_serializing_if = "skip_if_none")]
    valid_uris: Option<Arc<[Arc<str>]>>,

    #[serde(flatten)]
    other_values: FxHashMap<Arc<str>, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MinecraftJoinRequest {
    access_token: Arc<str>,
    selected_profile: Uuid,
    server_id: Arc<str>,
}

pub fn skip_if_none<T>(value: &Option<T>) -> bool {
    value.is_none()
}

#[derive(Clone, Copy)]
enum RequestStatus {
    OK,
    BadRequest,
    Unauthorized,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    InternalServerError,
}

pub async fn write_json_response<T: Serialize>(stream: &mut TcpStream, t: &T) -> serde_json::Result<()> {
    let raw = serde_json::to_string(t)?;
    let response = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        raw.len(), raw);
    _ = stream.write_all(response.as_bytes()).await;
    Ok(())
}

impl RequestStatus {
    pub async fn write(self, stream: &mut TcpStream) {
        let response = match self {
            RequestStatus::OK => "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            RequestStatus::BadRequest => "HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            RequestStatus::Unauthorized => "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            RequestStatus::Forbidden => "HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            RequestStatus::NotFound => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            RequestStatus::MethodNotAllowed => "HTTP/1.1 405 Method Not Allowed\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            RequestStatus::InternalServerError => "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        };
        _ = stream.write_all(response.as_bytes()).await;
    }
}
