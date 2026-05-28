//! `github` — the slice-02 GitHub-scraper port surface (WD-61 / ADR-019).
//!
//! `GithubPort` (declared in `lib.rs`) is a NEW driving-shaped port: GitHub
//! is a wholly different external system from ATProto, sharing no method
//! shape, auth model, rate-limit semantic, or failure surface with
//! `PdsPort` (hence a distinct trait, not a `PdsPort` extension). This
//! module owns the value types that flow across that port:
//!
//! - [`TargetKind`] — `owner/repo` (Repo) vs `user` (User) disambiguation.
//! - [`Signal`] + [`SignalKind`] — one harvested public GitHub artifact /
//!   measurable property, carrying enough to name itself in a candidate's
//!   source-signal line and be mapped to a predicate by the SSOT mapping.
//! - [`CandidateClaim`] — a PROPOSAL purely derived from one or more
//!   `Signal`s. In-memory ONLY; never persisted as-is, never signed without
//!   the human's gesture (the human-gate, I-SCR-1).
//! - [`GithubError`] — the railway-oriented failure surface of the port.
//!
//! ## Type placement (Q-DELIVER-3)
//!
//! The DESIGN component-boundaries left `Signal` / `CandidateClaim`
//! placement to DELIVER: `scraper-domain` (default) vs `ports`. DELIVER
//! places them HERE in `ports` because the `GithubPort` trait signatures
//! reference `Signal` and `TargetKind` directly — keeping them in `ports`
//! means `scraper-domain` (the pure derivation crate, step 01-02) depends
//! ON `ports` for the shared shapes rather than `ports` depending on
//! `scraper-domain`. Both directions keep both crates pure; this one adds
//! ZERO new dependency to `ports` (no `serde_yaml`, no `scraper-domain`),
//! so `xtask check-arch`'s pure-`ports` rule stays trivially green. The
//! `scraper-domain::derive_candidates` signature in step 01-02 consumes
//! `ports::Signal` and produces `ports::CandidateClaim`.
//!
//! ## Auditability invariant at the type level (I-SCR-4 / KPI-SCR-3)
//!
//! [`CandidateClaim::source_signals`] is guaranteed NON-EMPTY by the smart
//! constructor [`CandidateClaim::try_new`]: a candidate the user cannot
//! trace back to a public signal is unauditable and must not exist. The
//! field is private; the only construction path validates non-emptiness and
//! returns a [`CandidateClaimError`] on violation. This is the "candidate
//! names its signal" intent (component-boundaries.md) modeled as a
//! validated-construction wrapper (nw-fp-domain-modeling §3) rather than a
//! deeper runtime check.
//
// SCAFFOLD: false  (types are real; the GithubPort trait body is a
// declaration only — implementations land in adapter-github, step 01-03/04)

use thiserror::Error;

// -----------------------------------------------------------------------------
// TargetKind — repo vs user disambiguation (resolve_target output)
// -----------------------------------------------------------------------------

/// Which class of GitHub identifier a `<target>` resolved to.
///
/// `resolve_target` disambiguates `owner/repo` (a public repository) from a
/// bare `user` (a public user / contributor). It REFUSES private /
/// non-existent targets (returning [`GithubError::NotPublic`] /
/// [`GithubError::NotFound`]) — a resolvable `TargetKind` is therefore
/// always a PUBLIC target (WD-51 / I-SCR-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetKind {
    /// `owner/repo` resolved to a public repository.
    Repo { owner: String, repo: String },
    /// `user` resolved to a public user / contributor.
    User { user: String },
}

// -----------------------------------------------------------------------------
// Signal — one harvested public GitHub artifact / measurable property
// -----------------------------------------------------------------------------

/// A public GitHub artifact or measurable property harvested by
/// `adapter-github`.
///
/// Carries exactly enough to (a) name itself in a candidate's source-signal
/// line (auditability; KPI-SCR-3) and (b) be mapped to a predicate by the
/// `signal_predicate_mapping` SSOT (step 01-02). `value` and `source_url`
/// are display + audit metadata — they are NOT canonicalized or signed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    /// Typed kind; matches a `signal_predicate_mapping` entry.
    pub kind: SignalKind,
    /// Human-readable detail rendered verbatim in the candidate's
    /// source-signal line ("Cargo.lock committed", "test ratio 0.61").
    pub value: String,
    /// The public GitHub URL evidencing the signal — flows into the
    /// candidate's `evidence` (and thus the signed claim's `evidence[]` if
    /// the human signs).
    pub source_url: String,
}

/// The bounded set of signal kinds slice-02 recognizes.
///
/// Bounded by the 5-entry `jobs.yaml :: J-004.signal_predicate_mapping`
/// SSOT; `adapter-github` need not harvest signals the mapping cannot use
/// (US-SCR-001 Technical Notes). The mapping each kind resolves to lives in
/// `scraper-domain` (step 01-02), not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalKind {
    /// Cargo.lock committed / `==` version pins.
    DependencyManifestPinned,
    /// `docs/` present + README > 200 lines + high doc-comment density.
    DocsPresentAndSubstantial,
    /// test/source file ratio > 0.5 OR CI runs a test matrix.
    TestRatioOrCiMatrix,
    /// Tags follow semver + CHANGELOG present.
    SemverAndChangelog,
    /// Primary language Rust OR a memory-safe language + no `unsafe`.
    MemorySafetyLanguage,
}

// -----------------------------------------------------------------------------
// CandidateClaim — a PROPOSAL purely derived from one or more Signals
// -----------------------------------------------------------------------------

/// Failure modes for [`CandidateClaim::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CandidateClaimError {
    /// A candidate was constructed with zero source signals. A candidate the
    /// user cannot trace to a public signal is unauditable (I-SCR-4 /
    /// KPI-SCR-3) and must not exist — `try_new` rejects it.
    #[error("candidate has no source signals — every candidate must name at least one public signal (I-SCR-4)")]
    NoSourceSignals,
}

/// A PROPOSAL derived purely from one or more [`Signal`]s.
///
/// In-memory ONLY; never persisted as-is, never signed without the human's
/// gesture (the human-gate, I-SCR-1). A `CandidateClaim` becomes a persisted
/// artifact ONLY by being pre-filled into the slice-01 `UnsignedClaim` and
/// carried through the slice-01 sign pipeline by the human.
///
/// `source_signals` is PRIVATE and guaranteed NON-EMPTY by [`try_new`] (the
/// auditability invariant at the type level, I-SCR-4); read it via
/// [`source_signals`](CandidateClaim::source_signals). All other fields are
/// public display/pre-fill data.
///
/// `confidence` is LOAD-BEARING: `scraper-domain` always stamps the mapping
/// default `0.25`; it is NEVER auto-inflated (WD-52 / I-SCR-3). Only the
/// human may raise it, at sign time, through the slice-01 pipeline.
///
/// [`try_new`]: CandidateClaim::try_new
#[derive(Debug, Clone, PartialEq)]
pub struct CandidateClaim {
    /// `github:<owner>/<repo>` or `github:<user>` (the github_target).
    pub subject: String,
    /// The relation, e.g. `"embodiesPhilosophy"`.
    pub predicate: String,
    /// The philosophy NSID from the mapping (`org.openlore.philosophy.*`).
    pub object: String,
    /// Public GitHub URL(s) from the contributing signal(s).
    pub evidence: Vec<String>,
    /// LOAD-BEARING: always the mapping default (`0.25`); never
    /// auto-inflated (WD-52 / I-SCR-3).
    pub confidence: f64,
    /// LOAD-BEARING: non-empty (I-SCR-4). Private so the only construction
    /// path is the validating [`try_new`](CandidateClaim::try_new).
    source_signals: Vec<Signal>,
}

impl CandidateClaim {
    /// Smart constructor (nw-fp-domain-modeling §3): builds a
    /// `CandidateClaim` only if `source_signals` is non-empty, returning
    /// [`CandidateClaimError::NoSourceSignals`] otherwise. This makes the
    /// auditability invariant (I-SCR-4) a construction-time guarantee rather
    /// than a deeper runtime check.
    pub fn try_new(
        subject: String,
        predicate: String,
        object: String,
        evidence: Vec<String>,
        confidence: f64,
        source_signals: Vec<Signal>,
    ) -> Result<Self, CandidateClaimError> {
        if source_signals.is_empty() {
            return Err(CandidateClaimError::NoSourceSignals);
        }
        Ok(Self {
            subject,
            predicate,
            object,
            evidence,
            confidence,
            source_signals,
        })
    }

    /// The contributing signals — guaranteed non-empty by [`try_new`]
    /// (I-SCR-4). The renderer names each signal's `value` in the
    /// candidate's source-signal line.
    ///
    /// [`try_new`]: CandidateClaim::try_new
    pub fn source_signals(&self) -> &[Signal] {
        &self.source_signals
    }
}

// -----------------------------------------------------------------------------
// GithubError — railway-oriented failure surface of GithubPort
// -----------------------------------------------------------------------------

/// Failure modes for [`GithubPort`](crate::GithubPort).
///
/// Errors are values (railway-oriented; nw-fp-domain-modeling §8). The
/// `NotFound` / `NotPublic` variants name their target so the CLI can
/// explain zero-candidates. [`GithubError::TokenRejected`] carries NO token
/// value — the rejected PAT is NEVER echoed (US-SCR-004 no-token-leak).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GithubError {
    /// HTTP 404 — the target does not exist. Named so zero-candidates is
    /// explainable (US-SCR-001 Ex 3).
    #[error("github target not found: {target}")]
    NotFound { target: String },
    /// Private / inaccessible target. The public-only API refuses it — the
    /// scraper only reads public data (WD-51 / I-SCR-2; US-SCR-001 Ex 4).
    #[error("github target is not public: {target} — the scraper only reads public data")]
    NotPublic { target: String },
    /// HTTP 403 — the rate budget is exhausted. `authenticated`
    /// distinguishes the anon budget from the PAT budget; the CLI surfaces a
    /// "set GITHUB_TOKEN" remediation for the anon case (US-SCR-004 Ex 3).
    #[error("github rate limit exhausted (authenticated: {authenticated}) — set GITHUB_TOKEN for a higher limit")]
    RateLimited { authenticated: bool },
    /// HTTP 401 — a stale / invalid PAT. The token value is NEVER echoed
    /// (US-SCR-004 Ex 4 no-token-leak).
    #[error("github token rejected (401) — the configured GITHUB_TOKEN is stale or invalid")]
    TokenRejected,
    /// Offline / transport failure — scraping requires network access.
    #[error("github network error: {0} — scrape requires network access")]
    Network(String),
    /// Unexpected response shape (contract drift against the public API).
    #[error("github api response shape unexpected: {0}")]
    ApiShape(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_signal() -> impl Strategy<Value = Signal> {
        (
            prop_oneof![
                Just(SignalKind::DependencyManifestPinned),
                Just(SignalKind::DocsPresentAndSubstantial),
                Just(SignalKind::TestRatioOrCiMatrix),
                Just(SignalKind::SemverAndChangelog),
                Just(SignalKind::MemorySafetyLanguage),
            ],
            "[ -~]{0,64}",
            "https://github\\.com/[a-z0-9-]{1,16}/[a-z0-9-]{1,16}",
        )
            .prop_map(|(kind, value, source_url)| Signal {
                kind,
                value,
                source_url,
            })
    }

    proptest! {
        #[test]
        fn candidate_claim_try_new_guarantees_non_empty_source_signals(
            signals in proptest::collection::vec(arb_signal(), 1..6),
        ) {
            let candidate = CandidateClaim::try_new(
                "github:rust-lang/cargo".to_string(),
                "embodiesPhilosophy".to_string(),
                "org.openlore.philosophy.memory-safety".to_string(),
                signals.iter().map(|s| s.source_url.clone()).collect(),
                0.25,
                signals.clone(),
            )
            .expect("non-empty source_signals must construct a CandidateClaim");

            prop_assert!(!candidate.source_signals().is_empty());
            prop_assert_eq!(candidate.source_signals().len(), signals.len());
        }
    }

    #[test]
    fn candidate_claim_try_new_rejects_empty_source_signals() {
        let result = CandidateClaim::try_new(
            "github:rust-lang/cargo".to_string(),
            "embodiesPhilosophy".to_string(),
            "org.openlore.philosophy.memory-safety".to_string(),
            Vec::new(),
            0.25,
            Vec::new(),
        );
        assert!(
            matches!(result, Err(CandidateClaimError::NoSourceSignals)),
            "a candidate with zero source signals is unauditable (I-SCR-4); try_new must reject it"
        );
    }

    #[test]
    fn target_kind_carries_owner_and_repo_or_user() {
        let repo = TargetKind::Repo {
            owner: "rust-lang".to_string(),
            repo: "cargo".to_string(),
        };
        match repo {
            TargetKind::Repo { owner, repo } => {
                assert_eq!(owner, "rust-lang");
                assert_eq!(repo, "cargo");
            }
            TargetKind::User { .. } => panic!("expected Repo"),
        }

        let user = TargetKind::User {
            user: "torvalds".to_string(),
        };
        match user {
            TargetKind::User { user } => assert_eq!(user, "torvalds"),
            TargetKind::Repo { .. } => panic!("expected User"),
        }
    }

    #[test]
    fn github_error_not_found_and_not_public_name_their_target() {
        let not_found = GithubError::NotFound {
            target: "rust-lang/does-not-exist".to_string(),
        };
        assert!(
            not_found.to_string().contains("rust-lang/does-not-exist"),
            "NotFound must name the target so zero-candidates is explainable"
        );

        let not_public = GithubError::NotPublic {
            target: "acme/private".to_string(),
        };
        assert!(
            not_public.to_string().contains("acme/private"),
            "NotPublic must name the target (WD-51 / I-SCR-2)"
        );
        assert!(
            not_public.to_string().contains("public"),
            "NotPublic must mention public-data-only remediation"
        );
    }

    #[test]
    fn github_error_token_rejected_never_echoes_a_token_value() {
        let err = GithubError::TokenRejected;
        let rendered = err.to_string();
        assert!(
            !rendered.contains("ghp_"),
            "TokenRejected must NEVER echo a token value (US-SCR-004 no-token-leak); got: {rendered}"
        );
    }
}
