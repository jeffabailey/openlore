//! Small shared HTTP plumbing for the bluesky-claim-review-app doubles
//! (`FakeAtprotoNetwork`, `FakeGithubAccounts`).
//!
//! DISTILL (2026-10-04). Kept separate from the slice-01..05 fakes so their
//! (frozen, green) helpers are not touched. No new HTTP client crate: the one
//! outbound call a double makes (the authorization server fetching the app's
//! client metadata, the way a real PDS does) is a minimal HTTP/1.1 GET over a
//! loopback `TcpStream`.

use std::collections::HashMap;

pub(crate) type HttpRequest = hyper::Request<hyper::body::Incoming>;
pub(crate) type HttpResponse = hyper::Response<http_body_util::Full<bytes::Bytes>>;

/// Parse an `application/x-www-form-urlencoded` string (a query string or a
/// form body) into a map. Later keys win.
pub(crate) fn parse_form(raw: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for pair in raw.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        if !key.is_empty() {
            map.insert(url_decode(key), url_decode(value));
        }
    }
    map
}

/// Percent-decode (and `+` → space) a form component.
pub(crate) fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (from_hex(bytes[i + 1]), from_hex(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(if b == b'+' { b' ' } else { b });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encode a form/query component (RFC 3986 unreserved kept).
pub fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn from_hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Build a JSON response with optional extra headers.
pub(crate) fn json_with_headers(
    status: u16,
    body: &serde_json::Value,
    headers: &[(&str, String)],
) -> HttpResponse {
    let mut builder = hyper::Response::builder()
        .status(status)
        .header("content-type", "application/json");
    for (name, value) in headers {
        builder = builder.header(*name, value.as_str());
    }
    builder
        .body(http_body_util::Full::new(bytes::Bytes::from(
            body.to_string(),
        )))
        .expect("build JSON response")
}

pub(crate) fn json(status: u16, body: serde_json::Value) -> HttpResponse {
    json_with_headers(status, &body, &[])
}

/// A redirect (`302`) to `location`.
pub(crate) fn redirect(location: &str) -> HttpResponse {
    hyper::Response::builder()
        .status(302)
        .header("location", location)
        .body(http_body_util::Full::new(bytes::Bytes::new()))
        .expect("build redirect")
}

/// An empty-bodied response (used for RFC 7009 revocation: `200`, empty).
pub(crate) fn empty(status: u16) -> HttpResponse {
    hyper::Response::builder()
        .status(status)
        .body(http_body_util::Full::new(bytes::Bytes::new()))
        .expect("build empty response")
}

/// Read a request body to a UTF-8 string (lossy).
pub(crate) async fn body_string(req: HttpRequest) -> String {
    use http_body_util::BodyExt;
    match req.into_body().collect().await {
        Ok(collected) => String::from_utf8_lossy(&collected.to_bytes()).into_owned(),
        Err(_) => String::new(),
    }
}

/// Header value as a `String`, if present and valid UTF-8.
pub(crate) fn header(req: &HttpRequest, name: &str) -> Option<String> {
    req.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

/// Minimal HTTP/1.1 GET against a loopback `http://` URL. Returns
/// `(status, body)`, or `Err(reason)` on a connect/read failure or timeout.
/// Handles `Content-Length` and `Transfer-Encoding: chunked` bodies.
pub(crate) async fn http_get(raw_url: &str) -> Result<(u16, String), String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let parsed = url::Url::parse(raw_url).map_err(|e| format!("bad url {raw_url}: {e}"))?;
    if parsed.scheme() != "http" {
        return Err(format!(
            "only http:// loopback URLs are fetchable in tests: {raw_url}"
        ));
    }
    let host = parsed.host_str().ok_or("url has no host")?.to_string();
    let port = parsed.port().unwrap_or(80);
    let mut target = parsed.path().to_string();
    if let Some(q) = parsed.query() {
        target.push('?');
        target.push_str(q);
    }
    let fetch = async move {
        let mut stream = tokio::net::TcpStream::connect((host.as_str(), port))
            .await
            .map_err(|e| format!("connect: {e}"))?;
        let request = format!(
            "GET {target} HTTP/1.1\r\nHost: {host}:{port}\r\nAccept: application/json\r\nConnection: close\r\n\r\n"
        );
        stream
            .write_all(request.as_bytes())
            .await
            .map_err(|e| format!("write: {e}"))?;
        let mut raw = Vec::new();
        stream
            .read_to_end(&mut raw)
            .await
            .map_err(|e| format!("read: {e}"))?;
        Ok::<Vec<u8>, String>(raw)
    };
    let raw = tokio::time::timeout(std::time::Duration::from_secs(5), fetch)
        .await
        .map_err(|_| "timeout".to_string())??;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let (head, body) = text.split_once("\r\n\r\n").ok_or("malformed response")?;
    let status: u16 = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse().ok())
        .ok_or("malformed status line")?;
    let chunked = head.lines().any(|l| {
        l.to_ascii_lowercase().starts_with("transfer-encoding:")
            && l.to_ascii_lowercase().contains("chunked")
    });
    let body = if chunked {
        dechunk(body)
    } else {
        body.to_string()
    };
    Ok((status, body))
}

fn dechunk(mut rest: &str) -> String {
    let mut out = String::new();
    while let Some((size_line, after)) = rest.split_once("\r\n") {
        let size = usize::from_str_radix(size_line.trim(), 16).unwrap_or(0);
        if size == 0 || after.len() < size {
            break;
        }
        out.push_str(&after[..size]);
        rest = after[size..].trim_start_matches("\r\n");
    }
    out
}

/// Decode the payload (middle segment) of a compact JWS without verifying it.
/// The doubles check *claims* (DPoP `htm`/`nonce`, client-assertion `iss`),
/// never signatures — signature verification belongs to the real servers and
/// is covered by the nightly live contract smoke (DV-BRA-12).
pub(crate) fn jwt_claims(token: &str) -> Option<serde_json::Value> {
    use base64::Engine;
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// RFC 7636 S256: `BASE64URL(SHA256(verifier))`.
pub(crate) fn pkce_s256(verifier: &str) -> String {
    use base64::Engine;
    use sha2::Digest;
    let digest = sha2::Sha256::digest(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest)
}

/// Short lowercase hex of SHA-256 (deterministic fake CIDs / ids).
pub(crate) fn short_hash(input: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(input.as_bytes());
    digest.iter().take(12).map(|b| format!("{b:02x}")).collect()
}

/// RAII abort guard for a spawned server task.
#[derive(Debug)]
pub(crate) struct AbortOnDrop(pub tokio::task::JoinHandle<()>);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// A dedicated multi-thread runtime for a sync test's doubles, shut down in
/// the background on drop (never blocks the test thread).
pub(crate) fn test_runtime(name: &str) -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_io()
        .enable_time()
        .thread_name(name.to_string())
        .build()
        .expect("build test runtime")
}
