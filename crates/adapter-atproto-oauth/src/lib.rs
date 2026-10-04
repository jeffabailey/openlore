//! `adapter-atproto-oauth` — the review app's atproto OAuth confidential
//! client (ADR-073).
//!
//! The authorization server identifies the app by fetching its client
//! metadata at the `client_id` URL and authenticates it with a
//! `private_key_jwt` assertion signed by the ES256 client key. Only the
//! public half of that key is ever published. Writes to a user's repo are
//! create-only (I-BRA-8): the port has no other write verb.
//!
//! Linked ONLY by `openlore-review-app` (xtask check-arch).

#![forbid(unsafe_code)]

mod probe;

use jose_jwa::{Algorithm, Signing};
use jose_jwk::{Class, Ec, EcCurves, Jwk, JwkSet, Key, Parameters};
use p256::SecretKey;
use ports::{OAuthPort, ProbeOutcome};
use serde_json::{json, Value};

/// The name Bluesky shows on its consent screen.
pub const CLIENT_NAME: &str = "OpenLore review";

/// Where the app publishes its client metadata (the `client_id`).
pub const CLIENT_METADATA_PATH: &str = "/oauth/client-metadata.json";
/// Where the authorization server returns the user after consent.
pub const CALLBACK_PATH: &str = "/oauth/callback";
/// Where the app publishes its public key set.
pub const JWKS_PATH: &str = "/oauth/jwks.json";

/// Why a client key was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientKeyError {
    /// Not a JSON Web Key.
    NotAJwk,
    /// Not a P-256 (ES256) elliptic-curve key.
    NotEs256,
    /// No key id, so the authorization server cannot select it.
    MissingKeyId,
    /// The public half only: the app cannot sign its assertions.
    NotPrivate,
}

impl std::fmt::Display for ClientKeyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NotAJwk => "the client key is not a JSON Web Key",
            Self::NotEs256 => "the client key is not an ES256 (P-256) key",
            Self::MissingKeyId => "the client key has no key id",
            Self::NotPrivate => "the client key has no private part",
        })
    }
}

impl std::error::Error for ClientKeyError {}

/// The app's ES256 client key: the private JWK and its parsed scalar.
#[derive(Clone)]
pub struct ClientKey {
    jwk: Jwk,
    secret: SecretKey,
}

impl ClientKey {
    /// A fresh random key with the given key id (`gen-client-jwk`, self-test).
    pub fn generate(key_id: &str) -> Self {
        let secret = SecretKey::random(&mut rand::rngs::OsRng);
        let jwk = Jwk {
            key: Key::Ec(Ec::from(&secret)),
            prm: Parameters {
                alg: Some(Algorithm::Signing(Signing::Es256)),
                kid: Some(key_id.to_string()),
                cls: Some(Class::Signing),
                ..Default::default()
            },
        };
        Self { jwk, secret }
    }

    /// Parse the `client-jwk` secret.
    pub fn parse(text: &str) -> Result<Self, ClientKeyError> {
        let jwk: Jwk = serde_json::from_str(text.trim()).map_err(|_| ClientKeyError::NotAJwk)?;
        if jwk.prm.kid.is_none() {
            return Err(ClientKeyError::MissingKeyId);
        }
        let Key::Ec(ec) = &jwk.key else {
            return Err(ClientKeyError::NotEs256);
        };
        if ec.crv != EcCurves::P256 {
            return Err(ClientKeyError::NotEs256);
        }
        if ec.d.is_none() {
            return Err(ClientKeyError::NotPrivate);
        }
        let secret = SecretKey::try_from(ec).map_err(|_| ClientKeyError::NotEs256)?;
        Ok(Self { jwk, secret })
    }

    /// The private JWK as JSON (written to the `client-jwk` secret).
    pub fn to_private_json(&self) -> String {
        serde_json::to_string(&self.jwk).expect("a JWK always serializes")
    }

    /// The public half: the same JWK with its private part removed.
    pub fn public_jwk(&self) -> Jwk {
        let mut public = self.jwk.clone();
        if let Key::Ec(ec) = &mut public.key {
            ec.d = None;
        }
        public
    }

    pub(crate) fn secret(&self) -> &SecretKey {
        &self.secret
    }
}

/// The client metadata document (pure). `origin` is the app's public origin.
pub fn client_metadata(origin: &str, scopes: &str) -> Value {
    json!({
        "client_id": format!("{origin}{CLIENT_METADATA_PATH}"),
        "client_name": CLIENT_NAME,
        "client_uri": origin,
        "application_type": "web",
        "redirect_uris": [format!("{origin}{CALLBACK_PATH}")],
        "grant_types": ["authorization_code", "refresh_token"],
        "response_types": ["code"],
        "scope": scopes,
        "token_endpoint_auth_method": "private_key_jwt",
        "token_endpoint_auth_signing_alg": "ES256",
        "dpop_bound_access_tokens": true,
        "jwks_uri": format!("{origin}{JWKS_PATH}"),
    })
}

/// The OAuth confidential client for one deployment (origin + scopes + key).
pub struct OAuthClientAdapter {
    key: ClientKey,
    origin: String,
    scopes: String,
}

impl OAuthClientAdapter {
    pub fn new(key: ClientKey, origin: impl Into<String>, scopes: impl Into<String>) -> Self {
        Self {
            key,
            origin: origin.into(),
            scopes: scopes.into(),
        }
    }
}

impl OAuthPort for OAuthClientAdapter {
    fn probe(&self) -> ProbeOutcome {
        probe::run(&self.key, &self.public_jwks())
    }

    fn client_metadata(&self) -> Value {
        client_metadata(&self.origin, &self.scopes)
    }

    fn public_jwks(&self) -> Value {
        let set = JwkSet {
            keys: vec![self.key.public_jwk()],
        };
        serde_json::to_value(set).expect("a JWK set always serializes")
    }
}

/// `atrium_xrpc::HttpClient` over the workspace rustls `reqwest` (ADR-073):
/// atrium-oauth's own default client pulls native-tls, which is banned.
#[derive(Default)]
pub struct RustlsHttpClient {
    client: reqwest::Client,
}

impl atrium_xrpc::HttpClient for RustlsHttpClient {
    async fn send_http(
        &self,
        request: atrium_xrpc::http::Request<Vec<u8>>,
    ) -> Result<atrium_xrpc::http::Response<Vec<u8>>, Box<dyn std::error::Error + Send + Sync>>
    {
        let response = self.client.execute(request.try_into()?).await?;
        let mut builder = atrium_xrpc::http::Response::builder().status(response.status());
        for (name, value) in response.headers() {
            builder = builder.header(name, value);
        }
        builder
            .body(response.bytes().await?.to_vec())
            .map_err(Into::into)
    }
}
