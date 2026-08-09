use std::{sync::Arc, time::Duration};

use schema::{minecraft_profile::SkinVariant, unique_bytes::UniqueBytes};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// Keeps the local skin server alive while held. When dropped, the server shuts down.
#[derive(Debug)]
pub struct SkinServerGuard {
    shutdown: Arc<tokio::sync::Notify>,
}

impl Drop for SkinServerGuard {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
    }
}

/// Starts a local HTTP server that serves the given skin texture to the game and
/// acts as a minimal Yggdrasil server for authlib-injector.
///
/// The server handles:
/// - `GET /skins/<uuid>.png` — the skin PNG (for both `userProperties` and Yggdrasil flow)
/// - `GET /` and `GET /api/yggdrasil` — metadata for authlib-injector (both for compatibility)
/// - `GET /sessionserver/session/minecraft/profile/<uuid>` and `GET /api/yggdrasil/sessionserver/session/minecraft/profile/<uuid>` — profile with textures
pub async fn start_skin_server(
    skin: UniqueBytes,
    uuid: Uuid,
    username: Arc<str>,
    variant: SkinVariant,
) -> Option<(SkinServerGuard, String)> {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.ok()?;
    let port = listener.local_addr().ok()?.port();

    let shutdown = Arc::new(tokio::sync::Notify::new());
    let server_shutdown = shutdown.clone();
    let skin_task = skin.clone();
    let uuid_simple = uuid.simple().to_string();
    let skin_url = format!("http://127.0.0.1:{port}/skins/{uuid_simple}.png");

    tokio::task::spawn(async move {
        serve_skin(
            listener,
            server_shutdown,
            skin_task,
            uuid_simple,
            username,
            variant,
            skin_url,
        )
        .await;
    });

    let url = format!("http://127.0.0.1:{port}/skins/{}.png", uuid.simple());
    Some((SkinServerGuard { shutdown }, url))
}

async fn serve_skin(
    listener: tokio::net::TcpListener,
    shutdown: Arc<tokio::sync::Notify>,
    skin: UniqueBytes,
    uuid_simple: String,
    username: Arc<str>,
    variant: SkinVariant,
    skin_url: String,
) {
    loop {
        tokio::select! {
            _ = shutdown.notified() => return,
            accepted = listener.accept() => {
                let Ok((mut socket, _)) = accepted else {
                    continue;
                };
                let skin = skin.clone();
                let uuid_simple = uuid_simple.clone();
                let username = username.clone();
                let skin_url = skin_url.clone();
                tokio::task::spawn(async move {
                    handle_connection(&mut socket, &skin, &uuid_simple, &username, variant, &skin_url).await;
                });
            }
        }
    }
}

async fn handle_connection(
    socket: &mut tokio::net::TcpStream,
    skin: &UniqueBytes,
    uuid_simple: &str,
    username: &str,
    variant: SkinVariant,
    skin_url: &str,
) {
    let mut buf = [0u8; 4096];
    let n = match tokio::time::timeout(Duration::from_secs(5), socket.read(&mut buf)).await {
        Ok(Ok(n)) => n,
        _ => return,
    };
    let request = String::from_utf8_lossy(&buf[..n]);
    let mut parts = request.split_whitespace();
    let _method = parts.next().unwrap_or("");
    let raw_path = parts.next().unwrap_or("");
    // Strip query string for route matching but keep original for query parsing if needed
    let path = raw_path.split('?').next().unwrap_or("");

    let (status, content_type, body): (&str, &str, Vec<u8>) = if path == format!("/skins/{uuid_simple}.png") {
        ("200 OK", "image/png", skin.to_vec())
    } else if path == "/"
        || path == "/api/yggdrasil"
        || path == "/api/yggdrasil/"
    {
        let meta = serde_json::json!({
            "meta": {
                "serverName": "PandoraLauncher",
                "implementationName": "PandoraLauncher"
            },
            "skinDomains": ["127.0.0.1"],
        });
        ("200 OK", "application/json", meta.to_string().into_bytes())
    } else if path == "/api/profiles/minecraft" || path == "/api/yggdrasil/api/profiles/minecraft" {
        // Bulk profile lookup used by authlib-injector; return the local profile
        let resp = serde_json::json!([{
            "id": uuid_simple,
            "name": username,
        }]);
        ("200 OK", "application/json", resp.to_string().into_bytes())
    } else if path.starts_with("/sessionserver/session/minecraft/profile/")
        || path.starts_with("/api/yggdrasil/sessionserver/session/minecraft/profile/")
    {
        // Path is /sessionserver/session/minecraft/profile/<uuid> or with /api/yggdrasil prefix
        let body = build_profile_response(uuid_simple, username, variant, skin_url);
        ("200 OK", "application/json", body.into_bytes())
    } else if path == "/sessionserver/session/minecraft/hasJoined"
        || path.starts_with("/sessionserver/session/minecraft/hasJoined?")
        || path == "/api/yggdrasil/sessionserver/session/minecraft/hasJoined"
        || path.starts_with("/api/yggdrasil/sessionserver/session/minecraft/hasJoined?")
    {
        // For hasJoined checks, return 204 No Content — single-player doesn't need it
        let header = "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let _ = socket.write_all(header.as_bytes()).await;
        let _ = socket.flush().await;
        return;
    } else {
        ("404 Not Found", "text/plain", b"Not Found".to_vec())
    };

    let header = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = socket.write_all(header.as_bytes()).await;
    let _ = socket.write_all(&body).await;
    let _ = socket.flush().await;
}

fn build_profile_response(uuid_simple: &str, username: &str, variant: SkinVariant, skin_url: &str) -> String {
    use base64::Engine;

    let mut skin = serde_json::json!({
        "url": skin_url,
    });
    if variant == SkinVariant::Slim {
        skin["metadata"] = serde_json::json!({ "model": "slim" });
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);

    let textures = serde_json::json!({
        "timestamp": timestamp,
        "profileId": uuid_simple,
        "profileName": username,
        "textures": {
            "SKIN": skin,
        },
    });

    let encoded = base64::engine::general_purpose::STANDARD.encode(textures.to_string());

    let profile = serde_json::json!({
        "id": uuid_simple,
        "name": username,
        "properties": [{
            "name": "textures",
            "value": encoded,
        }],
    });

    profile.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn serves_skin_for_matching_path() {
        let skin = UniqueBytes::new(b"fake-png-bytes");
        let uuid = Uuid::parse_str("5ef06b4d-a14c-3d8b-9b4a-8c0f0a1e2b3c").unwrap();

        let (guard, url) =
            start_skin_server(skin.clone(), uuid, Arc::from("TestPlayer"), SkinVariant::Classic)
                .await
                .expect("server should start");
        let parsed = url::Url::parse(&url).unwrap();
        let path = parsed.path().to_string();

        let mut socket = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", parsed.port().unwrap()))
            .await
            .unwrap();
        socket
            .write_all(format!("GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes())
            .await
            .unwrap();

        let mut response = Vec::new();
        socket.read_to_end(&mut response).await.unwrap();

        let header = String::from_utf8_lossy(&response);
        assert!(header.starts_with("HTTP/1.1 200 OK"), "got: {header}");
        assert!(header.ends_with("fake-png-bytes"), "got: {header}");

        drop(guard);
    }

    #[tokio::test]
    async fn serves_yggdrasil_profile() {
        let skin = UniqueBytes::new(b"fake-png-bytes");
        let uuid = Uuid::parse_str("5ef06b4d-a14c-3d8b-9b4a-8c0f0a1e2b3c").unwrap();

        let (guard, url) =
            start_skin_server(skin, uuid, Arc::from("TestPlayer"), SkinVariant::Slim)
                .await
                .expect("server should start");
        let parsed = url::Url::parse(&url).unwrap();
        let port = parsed.port().unwrap();

        let mut socket = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        let profile_path = format!("/api/yggdrasil/sessionserver/session/minecraft/profile/{}?unsigned=false", uuid.simple());
        socket
            .write_all(format!("GET {profile_path} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").as_bytes())
            .await
            .unwrap();

        let mut response = Vec::new();
        socket.read_to_end(&mut response).await.unwrap();
        let text = String::from_utf8_lossy(&response);
        assert!(text.starts_with("HTTP/1.1 200 OK"), "got: {text}");
        // Body should contain profile JSON with textures
        assert!(text.contains("\"name\":\"textures\"") || text.contains("\"name\": \"textures\"") || text.contains("textures"), "got: {text}");
        assert!(text.contains("TestPlayer"), "got: {text}");

        drop(guard);
    }

    #[tokio::test]
    async fn returns_not_found_for_unknown_path() {
        let skin = UniqueBytes::new(b"fake-png-bytes");
        let uuid = Uuid::parse_str("5ef06b4d-a14c-3d8b-9b4a-8c0f0a1e2b3c").unwrap();

        let (_guard, url) =
            start_skin_server(skin, uuid, Arc::from("TestPlayer"), SkinVariant::Classic)
                .await
                .expect("server should start");
        let parsed = url::Url::parse(&url).unwrap();

        let mut socket = tokio::net::TcpStream::connect(format!("127.0.0.1:{}", parsed.port().unwrap()))
            .await
            .unwrap();
        socket.write_all(b"GET /other.png HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await.unwrap();

        let mut response = Vec::new();
        socket.read_to_end(&mut response).await.unwrap();

        let header = String::from_utf8_lossy(&response);
        assert!(header.starts_with("HTTP/1.1 404 Not Found"), "got: {header}");
    }
}
