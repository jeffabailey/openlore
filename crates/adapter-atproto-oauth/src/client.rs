//! The atrium-oauth 0.1.7 confidential client (ADR-073), wired over the
//! workspace rustls reqwest and persisting its state ONLY through
//! `SecretStorePort` (the composition root supplies the review store).
//!
//! SPIKE finding 3: `OAuthClient::callback` reaches `todo!()` when the token
//! exchange fails. The exchange therefore runs in its own task; a panic there
//! ends only that task and comes back as
//! [`CompleteAuthorizationError::ExchangePanicContained`].

use std::sync::Arc;
use std::time::Duration;

use atrium_api::agent::SessionManager;
use atrium_api::types::string::Did;
use atrium_common::store::Store;
use atrium_identity::did::{CommonDidResolver, CommonDidResolverConfig};
use atrium_identity::handle::{AppViewHandleResolver, AppViewHandleResolverConfig};
use atrium_oauth::store::session::{Session, SessionStore};
use atrium_oauth::store::state::{InternalStateData, StateStore};
use atrium_oauth::{
    AtprotoClientMetadata, AuthMethod, AuthorizeOptions, CallbackParams, GrantType, KnownScope,
    OAuthClient, OAuthClientConfig, OAuthResolverConfig, Scope,
};
use ports::{
    AuthenticatedIdentity, BeginAuthorizationError, CompleteAuthorizationError, PdsCallback,
    ResolvedIdentity, SecretStorePort,
};
use serde_json::{json, Value};

use crate::{ClientKey, RustlsHttpClient, CALLBACK_PATH, CLIENT_METADATA_PATH, JWKS_PATH};

type AtriumClient = OAuthClient<
    StateBridge,
    SessionBridge,
    CommonDidResolver<RustlsHttpClient>,
    AppViewHandleResolver<RustlsHttpClient>,
    RustlsHttpClient,
>;

/// How long any single upstream OAuth / identity request may take.
const UPSTREAM_TIMEOUT: Duration = Duration::from_secs(15);

/// Where the client resolves identities (configuration, DWD-7).
#[derive(Debug, Clone, Copy)]
pub struct Upstreams<'a> {
    pub plc_url: &'a str,
    pub handle_resolver_url: &'a str,
}

/// The client could not be configured (an operator error at startup).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthSetupError(pub String);

impl std::fmt::Display for OAuthSetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the OAuth client cannot be configured: {}", self.0)
    }
}

impl std::error::Error for OAuthSetupError {}

/// A store bridge failure (atrium needs a `std::error::Error`).
#[derive(Debug)]
pub struct BridgeError(String);

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for BridgeError {}

fn bridge_error(e: impl std::fmt::Display) -> BridgeError {
    BridgeError(e.to_string())
}

/// atrium's `StateStore` over `SecretStorePort` (oauth_auth_requests).
pub struct StateBridge(Arc<dyn SecretStorePort>);

impl Store<String, InternalStateData> for StateBridge {
    type Error = BridgeError;

    async fn get(&self, state: &String) -> Result<Option<InternalStateData>, BridgeError> {
        self.0
            .auth_request(state)
            .map_err(bridge_error)?
            .map(|blob| serde_json::from_slice(&blob).map_err(bridge_error))
            .transpose()
    }

    async fn set(&self, state: String, data: InternalStateData) -> Result<(), BridgeError> {
        let expected_did = data
            .app_state
            .as_deref()
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .and_then(|v| v["did"].as_str().map(str::to_string));
        let blob = serde_json::to_vec(&data).map_err(bridge_error)?;
        self.0
            .put_auth_request(&state, &data.iss, expected_did.as_deref(), &blob)
            .map_err(bridge_error)
    }

    async fn del(&self, state: &String) -> Result<(), BridgeError> {
        self.0.remove_auth_request(state).map_err(bridge_error)
    }

    async fn clear(&self) -> Result<(), BridgeError> {
        Ok(())
    }
}

impl StateStore for StateBridge {}

/// atrium's `SessionStore` over `SecretStorePort` (oauth_sessions).
pub struct SessionBridge(Arc<dyn SecretStorePort>);

impl Store<Did, Session> for SessionBridge {
    type Error = BridgeError;

    async fn get(&self, did: &Did) -> Result<Option<Session>, BridgeError> {
        self.0
            .oauth_session(did.as_str())
            .map_err(bridge_error)?
            .map(|blob| serde_json::from_slice(&blob).map_err(bridge_error))
            .transpose()
    }

    async fn set(&self, did: Did, session: Session) -> Result<(), BridgeError> {
        let blob = serde_json::to_vec(&session).map_err(bridge_error)?;
        let scopes = session.token_set.scope.clone().unwrap_or_default();
        self.0
            .put_oauth_session(did.as_str(), &session.token_set.iss, &scopes, &blob)
            .map_err(bridge_error)
    }

    async fn del(&self, did: &Did) -> Result<(), BridgeError> {
        self.0
            .remove_oauth_session(did.as_str())
            .map_err(bridge_error)
    }

    async fn clear(&self) -> Result<(), BridgeError> {
        Ok(())
    }
}

impl SessionStore for SessionBridge {}

/// The configured scope string as atrium scopes (order preserved, so the
/// requested scope is exactly the configured one).
fn scopes_of(scopes: &str) -> Vec<Scope> {
    scopes
        .split_whitespace()
        .map(|scope| match scope {
            "atproto" => Scope::Known(KnownScope::Atproto),
            "transition:generic" => Scope::Known(KnownScope::TransitionGeneric),
            other => Scope::Unknown(other.to_string()),
        })
        .collect()
}

/// The identity the handle named, carried through the authorization as
/// atrium's opaque app state, so the callback can pin the subject.
fn app_state_of(identity: &ResolvedIdentity) -> String {
    json!({
        "did": identity.did,
        "handle": identity.verified_handle,
        "pds": identity.pds_endpoint,
    })
    .to_string()
}

fn identity_of_app_state(app_state: Option<&str>) -> Option<ResolvedIdentity> {
    let value: Value = serde_json::from_str(app_state?).ok()?;
    let field = |name: &str| value[name].as_str().map(str::to_string);
    Some(ResolvedIdentity {
        did: field("did")?,
        verified_handle: field("handle")?,
        pds_endpoint: field("pds")?,
    })
}

/// The confidential client and the store it persists through.
pub(crate) struct Handshake {
    client: Arc<AtriumClient>,
    secrets: Arc<dyn SecretStorePort>,
    scopes: Vec<Scope>,
}

impl Handshake {
    pub(crate) fn new(
        key: &ClientKey,
        origin: &str,
        scopes: &str,
        upstreams: Upstreams<'_>,
        secrets: Arc<dyn SecretStorePort>,
    ) -> Result<Self, OAuthSetupError> {
        let http = || RustlsHttpClient::with_timeout(UPSTREAM_TIMEOUT);
        let config = OAuthClientConfig {
            client_metadata: AtprotoClientMetadata {
                client_id: format!("{origin}{CLIENT_METADATA_PATH}"),
                client_uri: Some(origin.to_string()),
                redirect_uris: vec![format!("{origin}{CALLBACK_PATH}")],
                token_endpoint_auth_method: AuthMethod::PrivateKeyJwt,
                grant_types: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
                scopes: scopes_of(scopes),
                jwks_uri: Some(format!("{origin}{JWKS_PATH}")),
                token_endpoint_auth_signing_alg: Some("ES256".to_string()),
            },
            keys: Some(vec![key.private_jwk().clone()]),
            state_store: StateBridge(Arc::clone(&secrets)),
            session_store: SessionBridge(Arc::clone(&secrets)),
            resolver: OAuthResolverConfig {
                did_resolver: CommonDidResolver::new(CommonDidResolverConfig {
                    plc_directory_url: upstreams.plc_url.to_string(),
                    http_client: Arc::new(http()),
                }),
                handle_resolver: AppViewHandleResolver::new(AppViewHandleResolverConfig {
                    service_url: upstreams.handle_resolver_url.to_string(),
                    http_client: Arc::new(http()),
                }),
                authorization_server_metadata: Default::default(),
                protected_resource_metadata: Default::default(),
            },
            http_client: http(),
        };
        let client = OAuthClient::new(config).map_err(|e| OAuthSetupError(e.to_string()))?;
        Ok(Self {
            client: Arc::new(client),
            secrets,
            scopes: scopes_of(scopes),
        })
    }

    pub(crate) async fn begin(
        &self,
        identity: &ResolvedIdentity,
    ) -> Result<String, BeginAuthorizationError> {
        let options = AuthorizeOptions {
            redirect_uri: None,
            scopes: self.scopes.clone(),
            prompt: None,
            state: Some(app_state_of(identity)),
        };
        self.client
            .authorize(&identity.did, options)
            .await
            .map_err(|e| BeginAuthorizationError::Unavailable {
                detail: e.to_string(),
            })
    }

    pub(crate) async fn complete(
        &self,
        callback: PdsCallback,
    ) -> Result<AuthenticatedIdentity, CompleteAuthorizationError> {
        let client = Arc::clone(&self.client);
        let params = CallbackParams {
            code: callback.code,
            state: Some(callback.state),
            iss: callback.issuer,
        };
        let exchange = tokio::spawn(async move {
            let (session, app_state) = client.callback(params).await?;
            let subject = session.did().await;
            Ok::<_, atrium_oauth::Error>((subject, app_state))
        });
        match exchange.await {
            Err(_contained_panic) => Err(CompleteAuthorizationError::ExchangePanicContained),
            Ok(Err(atrium_oauth::Error::Callback(_))) => {
                Err(CompleteAuthorizationError::NotRecognised)
            }
            Ok(Err(_)) => Err(CompleteAuthorizationError::ExchangeFailed),
            Ok(Ok((subject, app_state))) => {
                let expected = identity_of_app_state(app_state.as_deref());
                match (subject, expected) {
                    (Some(subject), Some(expected)) => Ok(AuthenticatedIdentity {
                        token_subject: subject.as_str().to_string(),
                        expected,
                    }),
                    (subject, _) => {
                        if let Some(subject) = subject {
                            self.forget_session(subject.as_str());
                        }
                        Err(CompleteAuthorizationError::ExchangeFailed)
                    }
                }
            }
        }
    }

    pub(crate) fn abandon(&self, state: &str) {
        let _ = self.secrets.remove_auth_request(state);
    }

    pub(crate) fn forget_session(&self, owner_did: &str) {
        let _ = self.secrets.remove_oauth_session(owner_did);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Universe: scope strings over atproto scope tokens. The scopes the
        /// client asks for join back to exactly the configured string.
        #[test]
        fn the_requested_scope_is_exactly_the_configured_one(
            scopes in prop::collection::vec(prop_oneof![
                Just("atproto"), Just("transition:generic"),
                Just("repo:org.openlore.claim?action=create"),
                Just("repo:app.bsky.feed.post?action=create"),
            ], 1..5)
        ) {
            let configured = scopes.join(" ");
            let scopes = scopes_of(&configured);
            let requested: Vec<&str> = scopes.iter().map(AsRef::as_ref).collect();
            prop_assert_eq!(requested.join(" "), configured);
        }

        /// Universe: resolved identities. The app state carried through the
        /// authorization returns the same identity (round trip).
        #[test]
        fn the_named_identity_survives_the_round_trip(
            did in "did:plc:[a-z2-7]{24}", handle in "[a-z]{1,10}\\.[a-z]{2,5}", pds in "https://[a-z]{1,10}\\.example"
        ) {
            let identity = ResolvedIdentity { did, verified_handle: handle, pds_endpoint: pds };
            prop_assert_eq!(identity_of_app_state(Some(&app_state_of(&identity))), Some(identity));
        }
    }
}
