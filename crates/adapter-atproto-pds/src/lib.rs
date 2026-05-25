//! `adapter-atproto-pds` — `PdsPort` over `atrium-api` XRPC + rustls.
//!
//! Handles auth refresh, retries, idempotency-on-rkey-collision (ADR-004).
//! Probe verifies TLS handshake against configured PDS + DID match +
//! rkey-collision idempotency sentinel per architecture §6.2.
//!
//! RED-baseline scaffold (step 01-01).
//
// SCAFFOLD: true

#![allow(dead_code)]
#![forbid(unsafe_code)]

use async_trait::async_trait;
use ports::{AtUri, PdsError, PdsPort, ProbeOutcome};

pub struct AtProtoPdsAdapter {
    _endpoint: String,
}

impl AtProtoPdsAdapter {
    /// Build the adapter pointed at the given PDS endpoint URL.
    pub fn for_endpoint(endpoint: impl Into<String>) -> Self {
        Self {
            _endpoint: endpoint.into(),
        }
    }
}

#[async_trait]
impl PdsPort for AtProtoPdsAdapter {
    fn probe(&self) -> ProbeOutcome {
        panic!("Not yet implemented -- RED scaffold");
    }

    async fn create_record(
        &self,
        _collection: &str,
        _rkey: &str,
        _body: serde_json::Value,
    ) -> Result<AtUri, PdsError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    async fn get_record(
        &self,
        _collection: &str,
        _rkey: &str,
    ) -> Result<Option<serde_json::Value>, PdsError> {
        panic!("Not yet implemented -- RED scaffold");
    }

    async fn list_records(
        &self,
        _collection: &str,
    ) -> Result<Vec<serde_json::Value>, PdsError> {
        panic!("Not yet implemented -- RED scaffold");
    }
}
