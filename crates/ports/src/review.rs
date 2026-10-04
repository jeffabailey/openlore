//! Ports of the hosted review app (bluesky-claim-review-app, ADR-072/073/074).
//!
//! The review app is the THIRD composition root. It reaches the user's PDS
//! only through an OAuth confidential client (`OAuthPort`) and keeps private,
//! owner-scoped state in its own store (`ReviewStorePort`, `SessionPort`,
//! `SecretStorePort`). Each adapter exposes an Earned-Trust `probe()` that the
//! root runs before serving (wire, probe, use).

use async_trait::async_trait;

use crate::{ProbeOutcome, ResolvedIdentity};

/// What the user's PDS sent to the sign-in return address after approval.
/// Codes are secrets: this type has no `Debug`, so it cannot be logged.
pub struct PdsCallback {
    pub code: String,
    pub state: String,
    pub issuer: Option<String>,
}

/// A completed code exchange: whom the PDS signed in, next to whom the
/// handle named when authorization began. The sign-in pin decides between
/// them; the adapter decides nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedIdentity {
    /// The token's subject (`sub`).
    pub token_subject: String,
    /// The identity the handle resolved to before authorization began.
    pub expected: ResolvedIdentity,
}

/// Why authorization could not begin.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BeginAuthorizationError {
    /// The PDS / authorization server is unreachable or could not identify
    /// the app (e.g. it could not fetch the client metadata).
    #[error("the authorization server is unavailable: {detail}")]
    Unavailable { detail: String },
}

/// Why a return from the PDS signed nobody in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CompleteAuthorizationError {
    /// The `state` is unknown, expired or already used (replay).
    #[error("the sign-in return is not recognised")]
    NotRecognised,
    /// The authorization server refused or failed the code exchange.
    #[error("the code exchange failed")]
    ExchangeFailed,
    /// The OAuth library panicked during the exchange (SPIKE finding 3);
    /// the panic was contained to the exchange.
    #[error("the code exchange failed inside the OAuth client (contained)")]
    ExchangePanicContained,
}

/// The OAuth confidential client (ADR-073): its public face plus driving
/// the handshake. It writes nothing to any repo here.
#[async_trait]
pub trait OAuthPort: Send + Sync {
    /// Hard arms: the client key signs and its published half verifies;
    /// the JWKS is ES256-only with no private part.
    fn probe(&self) -> ProbeOutcome;

    /// The client metadata document served at the `client_id` URL.
    fn client_metadata(&self) -> serde_json::Value;

    /// The public key set served at `jwks_uri` (never a private part).
    fn public_jwks(&self) -> serde_json::Value;

    /// Push the authorization request for `identity` to its PDS and return
    /// the URL of the consent screen.
    async fn begin_authorization(
        &self,
        identity: &ResolvedIdentity,
    ) -> Result<String, BeginAuthorizationError>;

    /// Exchange an approved return's code. Single-use: the `state` is
    /// consumed whatever the outcome.
    async fn complete_authorization(
        &self,
        callback: PdsCallback,
    ) -> Result<AuthenticatedIdentity, CompleteAuthorizationError>;

    /// Forget a pending authorization the person declined.
    fn abandon_authorization(&self, state: &str);

    /// Forget the OAuth session held for `owner_did` (a refused sign-in).
    fn forget_session(&self, owner_did: &str);
}

/// A store operation failed (the detail is operator-facing, never a secret).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("review store failure: {0}")]
pub struct ReviewStoreError(pub String);

/// The owner-scoped private store (ADR-074).
pub trait ReviewStorePort: Send + Sync {
    /// Hard arms: schema version, AEAD canary, cross-owner canary.
    fn probe(&self) -> ProbeOutcome;
}

/// AEAD-sealed OAuth state, keyed by `state` id or owner DID (ADR-074).
/// Callers hand over and receive plaintext; sealing is the store's job.
pub trait SecretStorePort: Send + Sync {
    /// Keep a pending authorization (expires on its own after 10 minutes).
    fn put_auth_request(
        &self,
        state: &str,
        issuer: &str,
        expected_did: Option<&str>,
        blob: &[u8],
    ) -> Result<(), ReviewStoreError>;

    /// A live pending authorization, if any.
    fn auth_request(&self, state: &str) -> Result<Option<Vec<u8>>, ReviewStoreError>;

    fn remove_auth_request(&self, state: &str) -> Result<(), ReviewStoreError>;

    /// Keep (or replace) the OAuth session of `owner_did`.
    fn put_oauth_session(
        &self,
        owner_did: &str,
        issuer: &str,
        granted_scopes: &str,
        blob: &[u8],
    ) -> Result<(), ReviewStoreError>;

    fn oauth_session(&self, owner_did: &str) -> Result<Option<Vec<u8>>, ReviewStoreError>;

    fn remove_oauth_session(&self, owner_did: &str) -> Result<(), ReviewStoreError>;
}

/// A browser session to start: only hashes of the cookie and CSRF token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWebSession {
    pub session_hash: String,
    pub csrf_hash: String,
    pub owner_did: String,
    pub handle: String,
    pub pds_endpoint: String,
}

/// A live browser session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSession {
    pub owner_did: String,
    pub handle: String,
    pub csrf_hash: String,
}

/// Browser sessions (create, resolve, end).
pub trait SessionPort: Send + Sync {
    fn start_session(&self, session: &NewWebSession) -> Result<(), ReviewStoreError>;

    /// The live session whose cookie hashes to `session_hash`.
    fn resolve_session(&self, session_hash: &str) -> Result<Option<WebSession>, ReviewStoreError>;

    fn end_session(&self, session_hash: &str, owner_did: &str) -> Result<(), ReviewStoreError>;
}

/// A person's GitHub link (ADR-076): the numeric id is what was proven; the
/// login is how it was named at the time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GithubLink {
    pub github_login: String,
    pub github_user_id: u64,
    /// `false` once a re-check failed: verify again before any scan.
    pub verified: bool,
}

/// What recording a verified link did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRecorded {
    Linked,
    /// Another DID already holds a verified link to this GitHub account
    /// (one owner per GitHub account); nothing changed.
    HeldByAnotherOwner,
}

/// Owner-scoped GitHub links (`github_links`, one per DID).
pub trait GithubLinkPort: Send + Sync {
    fn github_link(&self, owner_did: &str) -> Result<Option<GithubLink>, ReviewStoreError>;

    /// Record (or replace) the verified link of `owner_did`, unless another
    /// DID holds a verified link to the same GitHub id.
    fn record_verified_link(
        &self,
        owner_did: &str,
        github_login: &str,
        github_user_id: u64,
    ) -> Result<LinkRecorded, ReviewStoreError>;

    /// A re-check failed: the link stays, marked unverified, with why.
    fn mark_link_unverified(&self, owner_did: &str, verdict: &str) -> Result<(), ReviewStoreError>;
}

/// How a scan run ended (or that it is still running), as `scan_runs.status`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanStatus {
    Running,
    Completed,
    RateLimited,
    Interrupted,
    OwnershipFailed,
}

impl ScanStatus {
    pub const ALL: [ScanStatus; 5] = [
        Self::Running,
        Self::Completed,
        Self::RateLimited,
        Self::Interrupted,
        Self::OwnershipFailed,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::RateLimited => "rate_limited",
            Self::Interrupted => "interrupted",
            Self::OwnershipFailed => "ownership_failed",
        }
    }

    pub fn parse(stored: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == stored)
    }
}

/// A scan run as the owner sees it: how it stands, and when a paused run
/// may resume (`resume_after`, Unix seconds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanRun {
    pub status: ScanStatus,
    pub resume_after: Option<i64>,
}

/// Owner-scoped scan runs (`scan_runs`).
pub trait ScanRunPort: Send + Sync {
    /// Record that a run started (`running`).
    fn start_scan(&self, owner_did: &str, run_id: &str) -> Result<(), ReviewStoreError>;

    /// Record how a started run ended.
    fn finish_scan(
        &self,
        owner_did: &str,
        run_id: &str,
        status: ScanStatus,
        resume_after: Option<i64>,
    ) -> Result<(), ReviewStoreError>;

    /// The owner's most recent run, if any.
    fn latest_scan(&self, owner_did: &str) -> Result<Option<ScanRun>, ReviewStoreError>;
}

/// A suggestion's identity (BR-1): one per (subject, predicate, object).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SuggestionKey {
    pub subject: String,
    pub predicate: String,
    pub object: String,
}

/// Where a suggestion stands (`suggestions.state`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SuggestionState {
    Pending,
    Declined,
    Published,
    Retracted,
}

impl SuggestionState {
    pub const ALL: [SuggestionState; 4] = [
        Self::Pending,
        Self::Declined,
        Self::Published,
        Self::Retracted,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Declined => "declined",
            Self::Published => "published",
            Self::Retracted => "retracted",
        }
    }

    pub fn parse(stored: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == stored)
    }
}

/// A private suggestion: the key, its confidence in basis points, the
/// evidence URLs and the "why" of each producing signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub key: SuggestionKey,
    pub confidence_bp: u16,
    pub evidence: Vec<String>,
    pub why: Vec<String>,
    /// `owner/repo` the suggestion was derived from.
    pub source_repo: String,
}

/// Owner-scoped READS of the suggestion queue (page renders).
pub trait ReviewStateRead: Send + Sync {
    /// The owner's pending suggestions, in a stable order.
    fn pending_suggestions(&self, owner_did: &str) -> Result<Vec<Suggestion>, ReviewStoreError>;

    /// Every key the owner has ever been offered, with its state.
    fn suggestion_states(
        &self,
        owner_did: &str,
    ) -> Result<Vec<(SuggestionKey, SuggestionState)>, ReviewStoreError>;
}

/// Owner-scoped WRITES to the suggestion queue (transitions).
pub trait ReviewStateWrite: Send + Sync {
    /// Add new pending suggestions (a key already held is left untouched).
    fn add_pending(
        &self,
        owner_did: &str,
        suggestions: &[Suggestion],
    ) -> Result<(), ReviewStoreError>;

    /// Move `key` from `from` to `to`; `false` when it was not in `from`.
    fn change_state(
        &self,
        owner_did: &str,
        key: &SuggestionKey,
        from: SuggestionState,
        to: SuggestionState,
    ) -> Result<bool, ReviewStoreError>;
}
