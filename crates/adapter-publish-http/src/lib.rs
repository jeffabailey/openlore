//! `adapter-publish-http` — the effect shell for the opaque instance transport
//! (ADR-062 §1; serverless-philosophy-federation).
//!
//! Implements the READ-ONLY [`InstanceReadPort`] and the write-capable
//! [`PublishPort`] over plain HTTP against a user's own serverless instance:
//!
//! | Port operation | Wire |
//! |---|---|
//! | `probe_manifest` / `probe` | ONE `GET /manifest` — reachability + the openlore marker ONLY (no canary write; ADR-062 §6 amended 2026-09-25), parsed by the pure `publish-domain`; a pass hands back the manifest |
//! | `get_record` | `GET /records/:cid` — bytes returned VERBATIM |
//! | `put_record` | `PUT /records/:cid` — body = the verbatim record bytes (stage) |
//! | `commit_manifest_entry` | `PUT /records/:cid` + the [`MANIFEST_ENTRY_HEADER`] display projection (commit) |
//!
//! Write auth (DV-4 / Q-SF-D2): an adapter built with [`HttpPublishAdapter::for_owner`]
//! sends `Authorization: Bearer <owner token>` on its `PUT`s ONLY — never on a
//! `GET`; one built with [`HttpPublishAdapter::for_instance`] (the read-only
//! wiring) holds no token at all. A `401`/`403` on a write is the typed
//! [`InstanceError::UnauthorizedWrite`] (`publish.unauthorized_write`).
//!
//! The adapter never parses a record blob and never computes a CID — the Rust
//! `claim-domain` core, called by the composition root, is the sole
//! canonicalizer.

#![forbid(unsafe_code)]

use std::time::Duration;

use claim_domain::Cid;
use ports::{
    InstanceError, InstanceManifest, InstanceReadPort, ManifestEntry, ProbeRefusalReason,
    ProbeRefused, PublishPort, RecordBytes,
};
use publish_domain::ManifestObservation;

/// Request header carrying the manifest v1 display projection (ASCII JSON)
/// on the committing `PUT /records/:cid`.
pub const MANIFEST_ENTRY_HEADER: &str = "x-openlore-manifest-entry";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// The per-instance owner write token (DV-4): a secret. Its `Debug` is
/// redacted and it has no `Display`, so it cannot leak into logs or errors.
#[derive(Clone, PartialEq, Eq)]
pub struct WriteToken(String);

impl WriteToken {
    /// A legal bearer token: non-empty visible ASCII with no whitespace (so
    /// it is always a valid HTTP header value). `None` otherwise.
    pub fn parse(raw: &str) -> Option<Self> {
        let legal = !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_graphic());
        legal.then(|| Self(raw.to_string()))
    }

    /// The `Authorization` header value, marked sensitive for the client.
    fn authorization_header(&self) -> reqwest::header::HeaderValue {
        let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {}", self.0))
            .expect("a parsed WriteToken is visible ASCII, a legal header value");
        value.set_sensitive(true);
        value
    }
}

impl std::fmt::Debug for WriteToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("WriteToken(<redacted>)")
    }
}

/// HTTP adapter bound to ONE instance base URL.
pub struct HttpPublishAdapter {
    base_url: String,
    client: reqwest::blocking::Client,
    /// Sent on writes only; `None` for the read-only wiring.
    write_token: Option<WriteToken>,
}

impl HttpPublishAdapter {
    /// Bind a READ-ONLY-wired adapter to an instance base URL (trailing `/`
    /// tolerated). It holds no write token.
    pub fn for_instance(base_url: &str) -> Self {
        Self::for_owner(base_url, None)
    }

    /// Bind the write-capable adapter: its `PUT`s carry the owner token
    /// (when one is configured); its `GET`s never do.
    pub fn for_owner(base_url: &str, write_token: Option<WriteToken>) -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("reqwest blocking client builds with static configuration");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            write_token,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base_url)
    }

    fn unreachable(&self, err: &reqwest::Error) -> InstanceError {
        InstanceError::Unreachable {
            url: self.base_url.clone(),
            detail: err.to_string(),
        }
    }

    /// One `GET /manifest` attempt, observed as raw facts — the single read
    /// both pure classifiers derive from: the probe
    /// (`classify_manifest_observation`) and the cross-instance
    /// peer-transport selection (`select_peer_transport`). A transport failure
    /// (incl. a body cut off mid-read) is `Unreachable`; any HTTP response is
    /// `Responded` with its bytes. A read: it never carries the write token.
    pub fn observe_manifest(&self) -> ManifestObservation {
        let unreachable = |err: reqwest::Error| ManifestObservation::Unreachable {
            url: self.base_url.clone(),
            detail: err.to_string(),
        };
        let response = match self.client.get(self.url("/manifest")).send() {
            Ok(response) => response,
            Err(err) => return unreachable(err),
        };
        let status = response.status().as_u16();
        match response.bytes() {
            Ok(body) => ManifestObservation::Responded {
                status,
                body: body.to_vec(),
            },
            Err(err) => unreachable(err),
        }
    }

    fn put(
        &self,
        cid: &Cid,
        record: &RecordBytes,
        manifest_entry: Option<String>,
    ) -> Result<(), InstanceError> {
        let request = self
            .client
            .put(self.url(&format!("/records/{}", cid.0)))
            .header("content-type", "application/json")
            .body(record.0.clone());
        let request = match manifest_entry {
            Some(entry) => request.header(MANIFEST_ENTRY_HEADER, entry),
            None => request,
        };
        let request = match &self.write_token {
            Some(token) => {
                request.header(reqwest::header::AUTHORIZATION, token.authorization_header())
            }
            None => request,
        };
        let response = request.send().map_err(|e| self.unreachable(&e))?;
        match response.status().as_u16() {
            200..=299 => Ok(()),
            status @ (401 | 403) => Err(InstanceError::UnauthorizedWrite {
                url: self.base_url.clone(),
                status,
            }),
            status => Err(InstanceError::Rejected {
                status,
                detail: response.text().unwrap_or_default(),
            }),
        }
    }
}

impl InstanceReadPort for HttpPublishAdapter {
    fn probe_manifest(&self) -> Result<InstanceManifest, ProbeRefused> {
        publish_domain::classify_manifest_observation(&self.observe_manifest())
            .map_err(|err| refusal_for(&self.base_url, &err))
    }

    fn get_record(&self, cid: &Cid) -> Result<RecordBytes, InstanceError> {
        let response = self
            .client
            .get(self.url(&format!("/records/{}", cid.0)))
            .send()
            .map_err(|e| self.unreachable(&e))?;
        match response.status().as_u16() {
            200..=299 => response
                .bytes()
                .map(|bytes| RecordBytes(bytes.to_vec()))
                .map_err(|e| self.unreachable(&e)),
            404 => Err(InstanceError::RecordNotFound { cid: cid.0.clone() }),
            status => Err(InstanceError::Rejected {
                status,
                detail: response.text().unwrap_or_default(),
            }),
        }
    }
}

impl PublishPort for HttpPublishAdapter {
    fn put_record(&self, cid: &Cid, record: &RecordBytes) -> Result<(), InstanceError> {
        self.put(cid, record, None)
    }

    fn commit_manifest_entry(
        &self,
        cid: &Cid,
        record: &RecordBytes,
        entry: &ManifestEntry,
    ) -> Result<(), InstanceError> {
        let entry_json =
            serde_json::to_value(entry).expect("a ManifestEntry always serializes to JSON");
        self.put(cid, record, Some(ascii_json(&entry_json)))
    }
}

/// Map a failed probe into the structured `health.startup.refused` payload,
/// carrying the dotted `publish.*` reason code.
fn refusal_for(base_url: &str, err: &InstanceError) -> ProbeRefused {
    let reason = match err {
        InstanceError::Unreachable { .. } => ProbeRefusalReason::PublishInstanceUnreachable,
        _ => ProbeRefusalReason::PublishNotAnOpenloreInstance,
    };
    let reason_code = err
        .reason_code()
        .unwrap_or(ports::REASON_NOT_AN_OPENLORE_INSTANCE);
    ProbeRefused {
        reason,
        detail: err.to_string(),
        structured: serde_json::json!({
            "reason_code": reason_code,
            "instance_url": base_url,
        }),
    }
}

/// Serialize JSON using ONLY visible ASCII, so it is a legal HTTP header
/// value: every non-ASCII char (and DEL) becomes a `\uXXXX` escape (UTF-16
/// surrogate pairs above the BMP). serde_json already escapes quotes and
/// control characters, and non-ASCII can only occur inside JSON strings, so
/// the result is equivalent JSON.
fn ascii_json(value: &serde_json::Value) -> String {
    value
        .to_string()
        .chars()
        .flat_map(|c| {
            if c.is_ascii() && c != '\x7f' {
                vec![c.to_string()]
            } else {
                let mut units = [0u16; 2];
                c.encode_utf16(&mut units)
                    .iter()
                    .map(|unit| format!("\\u{unit:04x}"))
                    .collect()
            }
        })
        .collect()
}
