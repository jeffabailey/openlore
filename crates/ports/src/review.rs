//! Ports of the hosted review app (bluesky-claim-review-app, ADR-072/073/074).
//!
//! The review app is the THIRD composition root. It reaches the user's PDS
//! only through an OAuth confidential client (`OAuthPort`) and keeps private,
//! owner-scoped state in its own store (`ReviewStorePort`). Both expose an
//! Earned-Trust `probe()` that the root runs before serving (wire, probe, use).

use crate::ProbeOutcome;

/// The OAuth confidential client's public face (ADR-073).
///
/// The authorization server identifies the app by fetching
/// [`OAuthPort::client_metadata`] at the `client_id` URL and verifies the
/// client assertion against [`OAuthPort::public_jwks`].
pub trait OAuthPort: Send + Sync {
    /// Hard arms: the client key signs and its published half verifies;
    /// the JWKS is ES256-only with no private part.
    fn probe(&self) -> ProbeOutcome;

    /// The client metadata document served at the `client_id` URL.
    fn client_metadata(&self) -> serde_json::Value;

    /// The public key set served at `jwks_uri` (never a private part).
    fn public_jwks(&self) -> serde_json::Value;
}

/// The owner-scoped private store (ADR-074).
pub trait ReviewStorePort: Send + Sync {
    /// Hard arms: schema version, AEAD canary, cross-owner canary.
    fn probe(&self) -> ProbeOutcome;
}
