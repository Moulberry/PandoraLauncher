use std::error::Error;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use url::Url;

use crate::{
    constants,
    models::{FinishedAuthorization, PendingAuthorization},
};

#[derive(thiserror::Error, Debug)]
pub enum ProcessAuthorizationError {
    #[error("Unable to start http server: {0}")]
    StartServer(Box<dyn Error + Send + Sync + 'static>),
    #[error("An I/O error occurred: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Server-side error: {0}")]
    ServersideError(String),
    #[error("Unable to parse HTTP request: {0}")]
    HttpParseError(#[from] httparse::Error),
    #[error("The csrf token in the request didn't match the response")]
    CsrfMismatch,
    #[error("The response didn't include the code")]
    MissingCode,
    #[error("Cancelled by user")]
    CancelledByUser,
}

pub async fn start_server(
    pending_authroization: PendingAuthorization,
) -> Result<FinishedAuthorization, ProcessAuthorizationError> {
    let listener = tokio::net::TcpListener::bind(constants::SERVER_ADDRESS).await?;

    let mut buf = vec![0_u8; 1024];
    let mut read;

    loop {
        let (mut stream, _addr) = listener.accept().await?;

        read = 0;
        loop {
            let n = stream.read(&mut buf[read..]).await?;
            read += n;

            if read == buf.len() {
                buf.resize(buf.len() * 2, 0);
                continue;
            }

            if read == 0 {
                break; // Accept a new connection
            }

            let mut headers = [httparse::EMPTY_HEADER; 32];
            let mut req = httparse::Request::new(&mut headers);
            let parsed = req.parse(&buf[..read])?;

            if parsed.is_partial() {
                if n == 0 {
                    break; // Accept a new connection
                } else {
                    continue;
                }
            }

            const BAD_REQUEST_RESPONSE: &[u8] = b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n";
            const NOT_FOUND_RESPONSE: &[u8] = b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";

            if req.method != Some("GET") || req.path.is_none() {
                stream.write_all(NOT_FOUND_RESPONSE).await?;
                break;
            }
            let path = req.path.unwrap();

            let Ok(url) = Url::parse(&format!("{}{}", constants::REDIRECT_URL_BASE, path)) else {
                stream.write_all(BAD_REQUEST_RESPONSE).await?;
                break;
            };

            if url.path() != "/auth" {
                stream.write_all(NOT_FOUND_RESPONSE).await?;
                break;
            }

            let mut error = None;
            let mut error_description = None;
            let mut code = None;
            let mut state = None;

            for (key, value) in url.query_pairs() {
                match &*key {
                    "error" => error = Some(value),
                    "error_description" => error_description = Some(value),
                    "code" => code = Some(value),
                    "state" => state = Some(value),
                    _ => {
                        eprintln!("Unknown parameter: {:?} => {:?}", key, value);
                    },
                }
            }

            if let Some(error) = error {
                let full_error = if let Some(error_description) = error_description {
                    let response = create_response(&format!("An error occurred: {}", &*error), &error_description, true);
                    stream.write_all(response.as_bytes()).await?;
                    format!("An error occurred: {}\n{}", error, error_description)
                } else {
                    let response = create_response(&format!("An error occurred: {}", &*error), "", true);
                    stream.write_all(response.as_bytes()).await?;
                    format!("An error occurred: {}", error)
                };
                return Err(ProcessAuthorizationError::ServersideError(full_error));
            }

            if let Some(state) = state
                && &*state != pending_authroization.csrf_token.secret()
            {
                let response = create_response(
                    "Error: CSRF Mismatch!",
                    "Did you reload the tab instead of going through the proper authorization flow?",
                    true,
                );
                stream.write_all(response.as_bytes()).await?;
                return Err(ProcessAuthorizationError::CsrfMismatch);
            }

            let Some(code) = code else {
                let response = create_response("Error", "Missing required 'code' parameter", true);
                stream.write_all(response.as_bytes()).await?;
                return Err(ProcessAuthorizationError::MissingCode);
            };

            let response = create_response("Authorization complete", "You may now close this window", false);
            stream.write_all(response.as_bytes()).await?;

            return Ok(FinishedAuthorization {
                pending: pending_authroization,
                code: code.to_string(),
            });
        }
    }
}

fn create_response(main: &str, secondary: &str, error: bool) -> String {
    let status = if error {
        "200 OK"
    } else {
        "400 Bad Request"
    };

    let body = format!(include_str!("auth_page.html"), main, secondary);
    let body_length = body.len();

    format!("HTTP/1.1 {status}\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Length: {}\r\n\r\n{}", body_length, body)
}
