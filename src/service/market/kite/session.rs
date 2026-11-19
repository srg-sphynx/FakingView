//! User-initiated Kite sessions. The API secret and one-use request token are never persisted.

use super::{
    BASE_URL, authorization, encode, map_http_error, request, response_data, valid_secret,
};
use crate::service::ProviderError;
use crate::service::net::{agent::FollowingAgent, http};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;
use std::time::Duration;

static SESSION: LazyLock<FollowingAgent> = LazyLock::new(|| {
    FollowingAgent::new(|config| {
        config
            .timeout_global(Some(Duration::from_secs(15)))
            .http_status_as_error(false)
    })
});

pub fn test_connection(api_key: &str, access_token: &str) -> Result<(), ProviderError> {
    let body = request(
        &format!("{BASE_URL}/user/profile"),
        api_key,
        access_token,
        1024 * 1024,
    )?;
    let data = response_data(&body)?;
    if !data.is_object() {
        return Err(ProviderError::data(
            "Kite Connect profile data is not an object",
        ));
    }
    Ok(())
}

/// Complete the documented login exchange. A request token is single-use, so a POST is never
/// automatically retried, including after a transport failure whose outcome is unknown.
pub fn exchange_request_token(
    api_key: &str,
    api_secret: &str,
    request_token: &str,
) -> Result<String, ProviderError> {
    if ![api_key, api_secret, request_token]
        .into_iter()
        .all(valid_secret)
    {
        return Err(ProviderError::other(
            "Enter the Kite API key, API secret and request token to start a session.",
        ));
    }
    exchange_at(
        &format!("{BASE_URL}/session/token"),
        api_key.trim(),
        api_secret.trim(),
        request_token.trim(),
    )
}

fn exchange_form(api_key: &str, api_secret: &str, request_token: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(api_key.as_bytes());
    digest.update(request_token.as_bytes());
    digest.update(api_secret.as_bytes());
    let checksum = format!("{:x}", digest.finalize());
    format!(
        "api_key={}&request_token={}&checksum={checksum}",
        encode(api_key),
        encode(request_token)
    )
}

fn exchange_at(
    url: &str,
    api_key: &str,
    api_secret: &str,
    request_token: &str,
) -> Result<String, ProviderError> {
    let form = exchange_form(api_key, api_secret, request_token);
    let mut response = SESSION
        .get_for(url)
        .post(url)
        .header("X-Kite-Version", "3")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(form.as_bytes())
        .map_err(|_| {
            ProviderError::Kite(http::Error::Network(
                "Kite session request failed".to_owned(),
            ))
        })?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(map_http_error(http::Error::Status(status)));
    }
    let body = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_to_string()
        .map_err(|_| ProviderError::data("Kite Connect session response could not be read"))?;
    let data = response_data(&body)?;
    let token = data
        .get("access_token")
        .and_then(Value::as_str)
        .filter(|token| valid_secret(token))
        .ok_or_else(|| {
            ProviderError::data("Kite Connect session response has no valid access token")
        })?;
    // Apply the same header validation as every later REST call before accepting a session.
    authorization(api_key, token)?;
    super::clear_catalog();
    Ok(token.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn session_exchange_posts_a_checksum_and_never_the_api_secret() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/session/token", listener.local_addr().unwrap());
        let worker = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut received = Vec::new();
            let mut buffer = [0; 2048];
            loop {
                let count = stream.read(&mut buffer).unwrap();
                received.extend_from_slice(&buffer[..count]);
                if count == 0 {
                    break;
                }
                if let Some(boundary) = received.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    let headers = String::from_utf8_lossy(&received[..boundary]).to_lowercase();
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            line.strip_prefix("content-length:")
                                .and_then(|value| value.trim().parse::<usize>().ok())
                        })
                        .unwrap();
                    if received.len() >= boundary + 4 + length {
                        break;
                    }
                }
            }
            let body = r#"{"status":"success","data":{"access_token":"new-access-token"}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            String::from_utf8(received).unwrap()
        });
        assert_eq!(
            exchange_at(&url, "key", "secret", "request").unwrap(),
            "new-access-token"
        );
        let received = worker.join().unwrap();
        assert!(received.starts_with("POST /session/token HTTP/1.1"));
        assert!(received.to_lowercase().contains("x-kite-version: 3"));
        let body = received.split_once("\r\n\r\n").unwrap().1;
        assert_eq!(body, exchange_form("key", "secret", "request"));
        assert!(!body.contains("secret"));
        assert!(!body.contains("access_token"));
    }

    #[test]
    fn form_encoding_does_not_treat_request_token_punctuation_as_parameters() {
        let form = exchange_form("a&b", "private", "x+y=z");
        assert!(form.starts_with("api_key=a%26b&request_token=x%2By%3Dz&checksum="));
        assert_eq!(form.split("checksum=").nth(1).unwrap().len(), 64);
        assert!(!form.contains("private"));
    }

    #[test]
    fn checksum_matches_an_independent_sha256_reference() {
        assert_eq!(
            exchange_form("key", "secret", "request"),
            "api_key=key&request_token=request&checksum=e4465f58be6159d2a7f087f4292515e9d631b4f91e056b53e6f14456222ff709"
        );
    }
}
