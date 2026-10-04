//! The create-only write to the signed-in user's OWN repo (ADR-073), and the
//! read-back of what was written (DWD-5). Both go through the user's restored
//! atrium-oauth session (DPoP-bound, nonce + refresh handled by atrium), so
//! a write can only ever target the session's own repo on its own PDS.
//!
//! A write the PDS refuses as `invalid_token` is retried ONCE after the
//! session's recorded expiry is dropped, so restore refreshes (SPIKE
//! finding 4): the cached access token is never resent as-is.
//!
//! Only `com.atproto.repo.createRecord` is ever sent: the port has no
//! update, put or delete (I-BRA-8 is non-representable here). An existing
//! record under the same key is success — the key is the content's CID.

use async_trait::async_trait;
use atrium_api::types::string::Did;
use atrium_xrpc::http::{Method, StatusCode};
use atrium_xrpc::types::{InputDataOrBytes, OutputDataOrBytes, XrpcRequest};
use atrium_xrpc::XrpcClient;
use ports::{CreatedRecord, RepoWriteError, UserRepoReadPort, UserRepoWritePort};
use serde_json::{json, Value};

use crate::OAuthClientAdapter;

const CLAIM_COLLECTION: &str = "org.openlore.claim";
const CREATE_RECORD: &str = "com.atproto.repo.createRecord";
const GET_RECORD: &str = "com.atproto.repo.getRecord";

type XrpcFailure = atrium_xrpc::Error<Value>;

fn at_uri(owner_did: &str, rkey: &str) -> String {
    format!("at://{owner_did}/{CLAIM_COLLECTION}/{rkey}")
}

/// Map a transport/XRPC failure onto the port's error.
fn repo_error(failure: XrpcFailure) -> RepoWriteError {
    match failure {
        atrium_xrpc::Error::XrpcResponse(response)
            if response.status == StatusCode::UNAUTHORIZED =>
        {
            RepoWriteError::SessionExpired
        }
        atrium_xrpc::Error::XrpcResponse(response) => RepoWriteError::Refused {
            status: response.status.as_u16(),
            detail: format!("{:?}", response.error),
        },
        atrium_xrpc::Error::Authentication(_) => RepoWriteError::SessionExpired,
        other => RepoWriteError::Unreachable {
            detail: other.to_string(),
        },
    }
}

/// ADR-073 idempotency: a retried create of the same content-addressed key.
fn already_exists(failure: &XrpcFailure) -> bool {
    matches!(
        failure,
        atrium_xrpc::Error::XrpcResponse(response)
            if format!("{:?}", response.error).to_lowercase().contains("already exists")
    )
}

impl OAuthClientAdapter {
    async fn session_for(&self, owner_did: &str) -> Result<impl XrpcClient + Sync, RepoWriteError> {
        let did = Did::new(owner_did.to_string()).map_err(|_| RepoWriteError::NoSession)?;
        self.handshake
            .client
            .restore(&did)
            .await
            .map_err(|_| RepoWriteError::NoSession)
    }

    /// Send `request` on the owner's restored session; on a refused access
    /// token, refresh through restore and send it once more.
    async fn send_refreshing(
        &self,
        owner_did: &str,
        request: &XrpcRequest<Value, Value>,
    ) -> Result<OutputDataOrBytes<Value>, SendFailure> {
        let first = self.send_once(owner_did, request).await;
        match first {
            Err(SendFailure::Xrpc(failure)) if access_refused(&failure) => {
                if let Ok(did) = Did::new(owner_did.to_string()) {
                    self.handshake.mark_access_stale(&did).await;
                }
                self.send_once(owner_did, request).await
            }
            other => other,
        }
    }

    async fn send_once(
        &self,
        owner_did: &str,
        request: &XrpcRequest<Value, Value>,
    ) -> Result<OutputDataOrBytes<Value>, SendFailure> {
        let session = self
            .session_for(owner_did)
            .await
            .map_err(|_| SendFailure::NoSession)?;
        session
            .send_xrpc::<Value, Value, Value, Value>(request)
            .await
            .map_err(SendFailure::Xrpc)
    }
}

/// Why a send did not answer.
enum SendFailure {
    /// No stored session for the owner (they must sign in).
    NoSession,
    Xrpc(XrpcFailure),
}

impl SendFailure {
    fn into_repo_error(self) -> RepoWriteError {
        match self {
            Self::NoSession => RepoWriteError::NoSession,
            Self::Xrpc(failure) => repo_error(failure),
        }
    }
}

/// The PDS refused the access token (expired early or revoked).
fn access_refused(failure: &XrpcFailure) -> bool {
    match failure {
        atrium_xrpc::Error::Authentication(_) => true,
        atrium_xrpc::Error::XrpcResponse(response) => response.status == StatusCode::UNAUTHORIZED,
        _ => false,
    }
}

#[async_trait]
impl UserRepoWritePort for OAuthClientAdapter {
    async fn create_claim_record(
        &self,
        owner_did: &str,
        rkey: &str,
        record: &Value,
    ) -> Result<CreatedRecord, RepoWriteError> {
        let request = XrpcRequest {
            method: Method::POST,
            nsid: CREATE_RECORD.to_string(),
            parameters: None,
            input: Some(InputDataOrBytes::Data(json!({
                "repo": owner_did,
                "collection": CLAIM_COLLECTION,
                "rkey": rkey,
                "record": record,
            }))),
            encoding: Some("application/json".to_string()),
        };
        let fallback_uri = || CreatedRecord {
            uri: at_uri(owner_did, rkey),
        };
        match self.send_refreshing(owner_did, &request).await {
            Ok(OutputDataOrBytes::Data(created)) => Ok(created["uri"]
                .as_str()
                .map(|uri| CreatedRecord {
                    uri: uri.to_string(),
                })
                .unwrap_or_else(fallback_uri)),
            Ok(OutputDataOrBytes::Bytes(_)) => Ok(fallback_uri()),
            Err(SendFailure::Xrpc(failure)) if already_exists(&failure) => Ok(fallback_uri()),
            Err(failure) => Err(failure.into_repo_error()),
        }
    }
}

#[async_trait]
impl UserRepoReadPort for OAuthClientAdapter {
    async fn read_claim_record(
        &self,
        owner_did: &str,
        rkey: &str,
    ) -> Result<Value, RepoWriteError> {
        let request = XrpcRequest {
            method: Method::GET,
            nsid: GET_RECORD.to_string(),
            parameters: Some(json!({
                "repo": owner_did,
                "collection": CLAIM_COLLECTION,
                "rkey": rkey,
            })),
            input: None,
            encoding: None,
        };
        match self.send_refreshing(owner_did, &request).await {
            Ok(OutputDataOrBytes::Data(view)) => Ok(view["value"].clone()),
            Ok(OutputDataOrBytes::Bytes(_)) => Err(RepoWriteError::Refused {
                status: 200,
                detail: "getRecord answered with bytes".to_string(),
            }),
            Err(failure) => Err(failure.into_repo_error()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // bypass: the at:// shape is a fixed format, one example pins it.
    #[test]
    fn the_fallback_address_is_the_claim_collection_under_the_cid() {
        assert_eq!(
            at_uri("did:plc:x", "bafyabc"),
            "at://did:plc:x/org.openlore.claim/bafyabc"
        );
    }
}
