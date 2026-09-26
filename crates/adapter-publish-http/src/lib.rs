//! `adapter-publish-http` — the effect shell for the opaque instance transport
//! (ADR-062 §1; serverless-philosophy-federation).
//!
//! Implements the READ-ONLY [`InstanceReadPort`] and the write-capable
//! [`PublishPort`] over plain HTTP against a user's own serverless instance:
//!
//! | Port operation | Wire |
//! |---|---|
//! | `probe` | `GET /manifest` — reachability + the openlore marker ONLY (no canary write; ADR-062 §6 amended 2026-09-25) |
//! | `fetch_manifest` | `GET /manifest`, parsed by the pure `publish-domain` |
//! | `get_record` | `GET /records/:cid` — bytes returned VERBATIM |
//! | `put_record` | `PUT /records/:cid` — body = the verbatim record bytes (stage) |
//! | `commit_manifest_entry` | `PUT /records/:cid` + the [`MANIFEST_ENTRY_HEADER`] display projection (commit) |
//!
//! The adapter never parses a record blob and never computes a CID — the Rust
//! `claim-domain` core, called by the composition root, is the sole
//! canonicalizer.

#![forbid(unsafe_code)]

use std::time::Duration;

use claim_domain::Cid;
use ports::{
    InstanceError, InstanceManifest, InstanceReadPort, ManifestEntry, ProbeOutcome,
    ProbeRefusalReason, PublishPort, RecordBytes,
};

/// Request header carrying the manifest v1 display projection (ASCII JSON)
/// on the committing `PUT /records/:cid`.
pub const MANIFEST_ENTRY_HEADER: &str = "x-openlore-manifest-entry";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

/// HTTP adapter bound to ONE instance base URL.
pub struct HttpPublishAdapter {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl HttpPublishAdapter {
    /// Bind the adapter to an instance base URL (trailing `/` tolerated).
    pub fn for_instance(base_url: &str) -> Self {
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("reqwest blocking client builds with static configuration");
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
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
        let response = request.send().map_err(|e| self.unreachable(&e))?;
        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(InstanceError::Rejected {
                status: status.as_u16(),
                detail: response.text().unwrap_or_default(),
            })
        }
    }
}

impl InstanceReadPort for HttpPublishAdapter {
    fn probe(&self) -> ProbeOutcome {
        match self.fetch_manifest() {
            Ok(_) => ProbeOutcome::Ok,
            Err(err) => refusal_for(&self.base_url, &err),
        }
    }

    fn fetch_manifest(&self) -> Result<InstanceManifest, InstanceError> {
        let response = self
            .client
            .get(self.url("/manifest"))
            .send()
            .map_err(|e| self.unreachable(&e))?;
        let status = response.status();
        if !status.is_success() {
            return Err(InstanceError::NotAnOpenloreInstance {
                detail: format!("GET /manifest returned HTTP {}", status.as_u16()),
            });
        }
        let body = response.bytes().map_err(|e| self.unreachable(&e))?;
        let manifest: serde_json::Value =
            serde_json::from_slice(&body).map_err(|_| InstanceError::NotAnOpenloreInstance {
                detail: "GET /manifest did not return JSON".to_string(),
            })?;
        publish_domain::read_manifest(&manifest)
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
fn refusal_for(base_url: &str, err: &InstanceError) -> ProbeOutcome {
    let reason = match err {
        InstanceError::Unreachable { .. } => ProbeRefusalReason::PublishInstanceUnreachable,
        _ => ProbeRefusalReason::PublishNotAnOpenloreInstance,
    };
    let reason_code = err
        .reason_code()
        .unwrap_or(ports::REASON_NOT_AN_OPENLORE_INSTANCE);
    ProbeOutcome::Refused {
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
