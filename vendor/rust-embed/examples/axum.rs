use axum::{
  extract::Path,
  http::{HeaderMap, StatusCode},
  response::{Html, IntoResponse, Response},
  routing::{get, Router},
};
use rust_embed::Embed;
use std::net::SocketAddr;

#[derive(Embed)]
#[folder = "examples/public/"]
#[compression = "zstd"]
struct Asset;

#[tokio::main]
async fn main() {
  let app = Router::new()
    .route("/", get(index_handler))
    .route("/index.html", get(index_handler))
    .route("/dist/{*file}", get(static_handler))
    .fallback_service(get(not_found));

  let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
  println!("listening on {}", addr);
  let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
  axum::serve(listener, app.into_make_service()).await.unwrap();
}

async fn index_handler(headers: HeaderMap) -> impl IntoResponse {
  serve_asset("index.html", &headers)
}

async fn static_handler(Path(path): Path<String>, headers: HeaderMap) -> impl IntoResponse {
  serve_asset(&path, &headers)
}

async fn not_found() -> Html<&'static str> {
  Html("<h1>404</h1><p>Not Found</p>")
}

#[cfg_attr(not(feature = "compression"), allow(unused_variables, reason = "headers for compression path only"))]
fn serve_asset(path: &str, headers: &HeaderMap) -> Response {
  #[cfg(feature = "compression")]
  if let Some(compressed) = Asset::compressed(path) {
    let encoding = compressed.content_encoding();
    let accept = headers.get(axum::http::header::ACCEPT_ENCODING).and_then(|v| v.to_str().ok()).unwrap_or("");
    if accept.contains(encoding) {
      return (
        [
          #[cfg(feature = "mime-guess")]
          (axum::http::header::CONTENT_TYPE, compressed.metadata.mimetype()),
          (axum::http::header::CONTENT_ENCODING, encoding),
        ],
        compressed.data.compressed().to_vec(),
      )
        .into_response();
    }
  }

  match Asset::get(path) {
    Some(content) => (
      #[cfg(feature = "mime-guess")]
      [(axum::http::header::CONTENT_TYPE, content.metadata.mimetype())],
      content.data,
    )
      .into_response(),
    None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
  }
}
