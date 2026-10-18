//! The create-only write to the signed-in user's OWN repo (ADR-073), and the
//! read-back of what was written (DWD-5). Both go through the user's restored
//! atrium-oauth session (DPoP-bound, nonce + refresh handled by atrium), so
//! a write can only ever target the session's own repo on its own PDS.
//!
//! A write the PDS refuses as `invalid_token` is retried ONCE after the
//! session's recorded expiry is dropped, so restore refreshes (SPIKE
//! finding 4): the cached access token is never resent as-is.
//!
//! Only `com.atproto.repo.createRecord` is ever sent, into the claim
//! collection or (the opt-in share post) `app.bsky.feed.post`: the port has
//! no update, put or delete (I-BRA-8 is non-representable here). An
//! existing claim under the same key is success — the key is the content's
//! CID.

use async_trait::async_trait;
use atrium_api::types::string::Did;
use atrium_xrpc::http::{Method, StatusCode};
use atrium_xrpc::types::{InputDataOrBytes, OutputDataOrBytes, XrpcRequest};
use atrium_xrpc::XrpcClient;
use ports::{CreatedRecord, RepoWriteError, UserRepoReadPort, UserRepoWritePort};
use serde_json::{json, Value};

use crate::OAuthClientAdapter;

const CLAIM_COLLECTION: &str = "org.openlore.claim";
const POST_COLLECTION: &str = "app.bsky.feed.post";
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
        atrium_xrpc::Error::HttpClient(source) if never_connected(source.as_ref()) => {
            RepoWriteError::Unreachable {
                detail: source.to_string(),
            }
        }
        other => RepoWriteError::AnswerLost {
            detail: other.to_string(),
        },
    }
}

/// U-43: the transport never reached the PDS (TCP connect or TLS handshake
/// failed), so the request was not sent. Anything else after the send may
/// have been stored.
fn never_connected(failure: &(dyn std::error::Error + 'static)) -> bool {
    std::iter::successors(Some(failure), |error| error.source())
        .filter_map(|error| error.downcast_ref::<reqwest::Error>())
        .any(reqwest::Error::is_connect)
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

    async fn create_post_record(
        &self,
        owner_did: &str,
        record: &Value,
    ) -> Result<CreatedRecord, RepoWriteError> {
        let request = XrpcRequest {
            method: Method::POST,
            nsid: CREATE_RECORD.to_string(),
            parameters: None,
            input: Some(InputDataOrBytes::Data(json!({
                "repo": owner_did,
                "collection": POST_COLLECTION,
                "record": record,
            }))),
            encoding: Some("application/json".to_string()),
        };
        match self.send_refreshing(owner_did, &request).await {
            Ok(OutputDataOrBytes::Data(created)) => match created["uri"].as_str() {
                Some(uri) => Ok(CreatedRecord {
                    uri: uri.to_string(),
                }),
                None => Err(RepoWriteError::Refused {
                    status: 200,
                    detail: "createRecord answered without a uri".to_string(),
                }),
            },
            Ok(OutputDataOrBytes::Bytes(_)) => Err(RepoWriteError::Refused {
                status: 200,
                detail: "createRecord answered with bytes".to_string(),
            }),
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
    use atrium_xrpc::error::{XrpcError, XrpcErrorKind};
    use proptest::prelude::*;

    /// The PDS's error answer with `status` and `message`.
    fn answered(status: u16, message: &str) -> XrpcFailure {
        atrium_xrpc::Error::XrpcResponse(XrpcError {
            status: StatusCode::from_u16(status).expect("status"),
            error: Some(XrpcErrorKind::Custom(
                json!({"error": "InvalidRequest", "message": message}),
            )),
        })
    }

    /// What reqwest reports for a GET to `url`, through atrium's boxed
    /// HttpClient error, optionally wrapped once more (an error source chain).
    fn transport_failure(url: &str, wrapped: bool) -> XrpcFailure {
        let failure = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(reqwest::Client::new().get(url).send())
            .expect_err("the request fails");
        let boxed: Box<dyn std::error::Error + Send + Sync> = if wrapped {
            Box::new(Wrapped(failure))
        } else {
            Box::new(failure)
        };
        atrium_xrpc::Error::HttpClient(boxed)
    }

    #[derive(Debug)]
    struct Wrapped(reqwest::Error);

    impl std::fmt::Display for Wrapped {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "session send failed")
        }
    }

    impl std::error::Error for Wrapped {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }

    /// A local address nothing listens on, so a connect there is refused.
    /// Its port is the local end of a loopback connection, kept open by the
    /// returned guard: a connected socket takes no new connections, and no
    /// parallel test can bind the port while the guard lives (NF-5).
    fn refusing_address() -> (String, [std::net::TcpStream; 2]) {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .expect("runtime");
        runtime.block_on(async {
            let far = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind");
            let near = tokio::net::TcpSocket::new_v4().expect("socket");
            near.bind("127.0.0.1:0".parse().expect("addr"))
                .expect("bind");
            let address = near.local_addr().expect("addr");
            let far_address = far.local_addr().expect("addr");
            let (near, far) = tokio::join!(near.connect(far_address), far.accept());
            let held = [
                near.expect("connect").into_std().expect("std stream"),
                far.expect("accept").0.into_std().expect("std stream"),
            ];
            (format!("http://{address}/xrpc/{CREATE_RECORD}"), held)
        })
    }

    /// A local server that takes the connection, reads the request and
    /// hangs up without answering: the request was sent, the answer lost.
    fn hangs_up_after_the_request() -> String {
        use std::io::Read;
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let address = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut request = [0u8; 1024];
                let _ = stream.read(&mut request);
            }
        });
        format!("http://{address}/xrpc/{CREATE_RECORD}")
    }

    proptest! {
        /// U-43. Universe: the PDS's literal 4xx and 5xx refusals × their
        /// messages. Each keeps its status for the domain to classify (a
        /// 4xx stored nothing, a 5xx may have), and only "already exists"
        /// is the create's success.
        #[test]
        fn a_refusal_keeps_its_status_and_only_already_exists_is_success(
            status in prop::sample::select(vec![400u16, 403, 404, 500, 502, 503]),
            message in prop::sample::select(vec![
                "Record refused by this PDS",
                "InternalServerError",
                "Record already exists",
            ]),
        ) {
            let failure = answered(status, message);

            prop_assert_eq!(already_exists(&failure), message == "Record already exists");
            let error = repo_error(failure);
            prop_assert!(
                matches!(error, RepoWriteError::Refused { status: kept, .. } if kept == status),
                "{:?}", error
            );
        }

        /// U-43. Universe: a connection never made × a request sent whose
        /// answer was lost, each bare or one error source deeper. Only the
        /// connection never made says nothing was sent.
        #[test]
        fn only_a_connection_never_made_is_unreachable(
            connected in any::<bool>(), wrapped in any::<bool>(),
        ) {
            let (url, _held) = if connected {
                (hangs_up_after_the_request(), None)
            } else {
                let (url, held) = refusing_address();
                (url, Some(held))
            };

            let error = repo_error(transport_failure(&url, wrapped));

            prop_assert_eq!(
                matches!(error, RepoWriteError::Unreachable { .. }),
                !connected,
                "{:?}", error
            );
            prop_assert_eq!(
                matches!(error, RepoWriteError::AnswerLost { .. }),
                connected,
                "{:?}", error
            );
        }
    }

    // bypass: the at:// shape is a fixed format, one example pins it.
    #[test]
    fn the_fallback_address_is_the_claim_collection_under_the_cid() {
        assert_eq!(
            at_uri("did:plc:x", "bafyabc"),
            "at://did:plc:x/org.openlore.claim/bafyabc"
        );
    }
}
