//! bluesky-claim-review-app — layer-2 PURE-CORE properties (ADR-007 functional
//! core; Mandate 9: PBT full at layers 1-2).
//!
//! The decisions the review app makes are pure functions in `review-domain`
//! (+ the ADR-071 provenance verdict in `claim-domain`). Their CONTRACTS are
//! stated here as properties over generated inputs, each checked against an
//! independent ORACLE written from the ADR / data-model text — never from the
//! implementation. These pair with the HTTP-level scenarios in the sibling
//! `review_app_*` suites (same AC tags).
//!
//! ## Binding seam (RED scaffold, Mandate 7)
//!
//! `review-domain` does not exist yet, so each property calls a `sut_*`
//! binding below whose body is `todo!()` — a panic, classified RED (not
//! BROKEN) when DELIVER unskips the property. DELIVER replaces each binding
//! body with ONE call into the production function (the view types here are
//! the observable contract; DELIVER maps its ADTs onto them).
//! CORE-11 (`select_person_repos`, BR-3) binds a function that ALREADY ships —
//! it is a GREEN-today regression guard, kept `#[ignore]`d for one-at-a-time
//! discipline.
//!
// SCAFFOLD: true

#[path = "support/review_app/mod.rs"]
mod review_app;

use std::collections::BTreeSet;

use proptest::prelude::*;
use review_app::recomputed_cid;

// =============================================================================
// Observable view types (the contract) + RED binding seam
// =============================================================================

/// ADR-076 §4 ownership verdict (the user-visible subset).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Verdict {
    Verified,
    DidMissing,
    DifferentDid(String),
    NoBio,
}

fn sut_ownership_verdict(_bio: Option<&str>, _session_did: &str) -> Verdict {
    todo!("SCAFFOLD: bind review_domain ownership verdict (ADR-076 §4)")
}

/// ADR-071 provenance verdict over (signature present?, author, repo DID, origin, CID match).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProvenanceVerdict {
    /// Signature present → the EXISTING app-signed verify path decides (unchanged).
    AppSignedPath,
    SelfAttested,
    MalformedProvenance,
    ForeignRepo,
    UnverifiableProvenance,
    IntegrityFailure,
}

fn sut_provenance(
    _signature_present: bool,
    _author: &str,
    _repo_did: &str,
    _origin_is_author_pds: bool,
    _recomputed_cid_matches_rkey: bool,
) -> ProvenanceVerdict {
    todo!("SCAFFOLD: bind claim_domain provenance verdict (ADR-071)")
}

/// A suggestion key `(subject, predicate, object)` (BR-1).
type Key = (String, String, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum State {
    Pending,
    Declined,
    Published,
    Retracted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Reconciled {
    new: BTreeSet<Key>,
    already_published: usize,
    declined_hidden: usize,
}

fn sut_reconcile(_existing: &[(Key, State)], _derived: &[Key]) -> Reconciled {
    todo!("SCAFFOLD: bind review_domain reconcile (BR-1/BR-2)")
}

/// BR-4: the typed confidence → basis points, or the guidance message.
fn sut_parse_confidence(_typed: &str) -> Result<i64, String> {
    todo!("SCAFFOLD: bind review_domain confidence parse (BR-4)")
}

/// The publish plan as observed: the exact record JSON, its record key, and
/// the preview's field values.
struct PlanView {
    record: serde_json::Value,
    rkey: String,
    preview_text: String,
}

fn sut_publish_plan(
    _did: &str,
    _subject: &str,
    _object: &str,
    _evidence: &[String],
    _basis_points: i64,
    _composed_at: &str,
) -> PlanView {
    todo!("SCAFFOLD: bind review_domain PublishPlan (Plan-value, AC-004.4)")
}

/// The share post as observed: text, and the link facet's byte range + URI.
struct ShareView {
    text: String,
    facet_start: usize,
    facet_end: usize,
    facet_uri: String,
}

fn sut_share_post(_profile_url: &str, _published: &[(String, bool)]) -> ShareView {
    todo!("SCAFFOLD: bind review_domain SharePostPlan (I-BRA-6)")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Decline,
    Undo,
    Publish,
    Retract,
}

fn sut_transition(_state: State, _event: Event) -> Option<State> {
    todo!("SCAFFOLD: bind review_domain suggestion lifecycle transition")
}

fn sut_visible_and_approvable(_state: State, _link_verified: bool) -> bool {
    todo!("SCAFFOLD: bind review_domain derived visibility (D-12)")
}

fn sut_budget_allows(_recent_event_ages_secs: &[u64], _limit: usize, _window_secs: u64) -> bool {
    todo!("SCAFFOLD: bind review_domain budget arithmetic (ADR-076)")
}

fn sut_sign_in_pin(_token_sub: &str, _resolved_did: &str) -> bool {
    todo!("SCAFFOLD: bind review_domain sign-in pin (AC-001.6)")
}

// =============================================================================
// Oracles (written from the ADR text, independent of any implementation)
// =============================================================================

/// ADR-076 §4: maximal runs of `[A-Za-z0-9._:%-]`, trailing `.,;:!?)]}>`
/// stripped, that match `did:[a-z]+:<rest>`.
fn oracle_did_tokens(bio: &str) -> Vec<String> {
    bio.split(|c: char| !(c.is_ascii_alphanumeric() || "._:%-".contains(c)))
        .map(|run| run.trim_end_matches(|c: char| ".,;:!?)]}>".contains(c)))
        .filter(|t| {
            let mut parts = t.splitn(3, ':');
            parts.next() == Some("did")
                && parts
                    .next()
                    .map(|m| !m.is_empty() && m.chars().all(|c| c.is_ascii_lowercase()))
                    .unwrap_or(false)
                && parts.next().map(|rest| !rest.is_empty()).unwrap_or(false)
        })
        .map(str::to_string)
        .collect()
}

fn oracle_verdict(bio: Option<&str>, did: &str) -> Verdict {
    match bio {
        None => Verdict::NoBio,
        Some(b) if b.trim().is_empty() => Verdict::NoBio,
        Some(b) => {
            let tokens = oracle_did_tokens(b);
            if tokens.iter().any(|t| t == did) {
                Verdict::Verified
            } else if let Some(first) = tokens.first() {
                Verdict::DifferentDid(first.clone())
            } else {
                Verdict::DidMissing
            }
        }
    }
}

/// data-models.md §1 table + "in every admitted case, CID must equal rkey".
fn oracle_provenance(
    sig: bool,
    author: &str,
    repo: &str,
    author_pds: bool,
    cid_ok: bool,
) -> ProvenanceVerdict {
    let admitted = if sig {
        ProvenanceVerdict::AppSignedPath
    } else if author.contains('#') {
        return ProvenanceVerdict::MalformedProvenance;
    } else if author != repo {
        return ProvenanceVerdict::ForeignRepo;
    } else if !author_pds {
        return ProvenanceVerdict::UnverifiableProvenance;
    } else {
        ProvenanceVerdict::SelfAttested
    };
    if cid_ok {
        admitted
    } else {
        ProvenanceVerdict::IntegrityFailure
    }
}

/// BR-4: 0.00–1.00, at most 2 decimals, stored as round(v × 10000).
fn oracle_confidence(typed: &str) -> Option<i64> {
    let t = typed.trim();
    let (int, frac) = t.split_once('.').unwrap_or((t, ""));
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if !int.chars().all(|c| c.is_ascii_digit())
        || !frac.chars().all(|c| c.is_ascii_digit())
        || frac.len() > 2
    {
        return None;
    }
    if t.contains('.') && frac.is_empty() {
        return None;
    }
    let int: i64 = if int.is_empty() { 0 } else { int.parse().ok()? };
    let hundredths: i64 = format!("{frac:0<2}").parse().ok()?;
    let bp = int * 10_000 + hundredths * 100;
    (0..=10_000).contains(&bp).then_some(bp)
}

fn oracle_transition(state: State, event: Event) -> Option<State> {
    match (state, event) {
        (State::Pending, Event::Decline) => Some(State::Declined),
        (State::Declined, Event::Undo) => Some(State::Pending),
        (State::Pending, Event::Publish) => Some(State::Published),
        (State::Published, Event::Retract) => Some(State::Retracted),
        _ => None,
    }
}

// =============================================================================
// Generators
// =============================================================================

fn arb_did() -> impl Strategy<Value = String> {
    "[a-z2-7]{24}".prop_map(|s| format!("did:plc:{s}"))
}

fn arb_words() -> impl Strategy<Value = String> {
    "[A-Za-z ,!?()]{0,30}"
}

/// A bio built around the signed-in DID in one of several key shapes.
fn arb_bio_around(did: String) -> impl Strategy<Value = (Option<String>, String)> {
    let shapes = prop_oneof![
        Just("exact"),
        Just("trailing-punct"),
        Just("extended"),
        Just("prefixed"),
        Just("uppercased"),
        Just("other-did"),
        Just("absent"),
        Just("empty"),
    ];
    (
        arb_words(),
        arb_words(),
        shapes,
        arb_did(),
        "[.,;:!?)]".prop_map(|s: String| s),
    )
        .prop_map(move |(pre, post, shape, other, punct)| {
            let token = match shape {
                "exact" => did.clone(),
                "trailing-punct" => format!("{did}{punct}"),
                "extended" => format!("{did}x"),
                "prefixed" => format!("x{did}"),
                "uppercased" => did.to_uppercase(),
                "other-did" => other,
                _ => String::new(),
            };
            let bio = match shape {
                "absent" => None,
                "empty" => Some(String::new()),
                _ => Some(format!("{pre} {token} {post}")),
            };
            (bio, did.clone())
        })
}

fn arb_key() -> impl Strategy<Value = Key> {
    (
        "[a-z]{1,6}/[a-z]{1,6}".prop_map(|r| format!("github:{r}")),
        Just("embodiesPhilosophy".to_string()),
        prop::sample::select(vec![
            "dependency-pinning",
            "memory-safety",
            "test-driven",
            "semantic-versioning",
            "documentation-first",
        ])
        .prop_map(|p| format!("org.openlore.philosophy.{p}")),
    )
}

fn arb_state() -> impl Strategy<Value = State> {
    prop_oneof![
        Just(State::Pending),
        Just(State::Declined),
        Just(State::Published),
        Just(State::Retracted)
    ]
}

fn arb_event() -> impl Strategy<Value = Event> {
    prop_oneof![
        Just(Event::Decline),
        Just(Event::Undo),
        Just(Event::Publish),
        Just(Event::Retract)
    ]
}

// =============================================================================
// Properties
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// CORE-1 @property @US-BRA-002 @AC-002.2 @AC-002.6 @ADR-076 @C1b @contract-shape:pure-function
    /// Ownership is Verified iff the signed-in DID appears as an exact,
    /// delimited, byte-equal token; otherwise the verdict names why.
    #[test]
    #[ignore = "DELIVER WS: unskip one-at-a-time (CORE-1 ownership verdict property)"]
    fn ownership_is_verified_exactly_when_the_signed_in_did_is_an_exact_bio_token(
        (bio, did) in arb_did().prop_flat_map(arb_bio_around)
    ) {
        prop_assert_eq!(sut_ownership_verdict(bio.as_deref(), &did), oracle_verdict(bio.as_deref(), &did));
    }

    /// CORE-2 @property @US-BRA-002 @AC-002.5 @adversarial @contract-shape:pure-function
    /// Several DIDs in a bio are allowed; only the signed-in one counts, and
    /// verifying one DID never verifies another.
    #[test]
    #[ignore = "DELIVER WS: unskip one-at-a-time (CORE-2 per-DID verdict)"]
    fn several_dids_in_a_bio_only_the_signed_in_one_counts(
        a in arb_did(), b in arb_did(), words in arb_words()
    ) {
        prop_assume!(a != b);
        let bio = format!("{words} {a}");
        prop_assert_eq!(sut_ownership_verdict(Some(&bio), &a), Verdict::Verified);
        prop_assert_eq!(sut_ownership_verdict(Some(&bio), &b), Verdict::DifferentDid(a.clone()));
    }

    /// CORE-3 @property @US-BRA-009 @AC-009.5 @ADR-071 @C5a @contract-shape:pure-function
    /// The provenance verdict follows the ADR-071 table for every combination
    /// of signature × author shape × origin × CID match, over generated DIDs.
    #[test]
    #[ignore = "DELIVER R2: unskip one-at-a-time (CORE-3 provenance table)"]
    fn the_provenance_verdict_follows_the_adr_071_table(
        repo in arb_did(), other in arb_did(),
        sig in any::<bool>(), author_shape in 0u8..3, author_pds in any::<bool>(), cid_ok in any::<bool>()
    ) {
        prop_assume!(repo != other);
        let author = match author_shape {
            0 => format!("{repo}#org.openlore.application"),
            1 => repo.clone(),
            _ => other,
        };
        prop_assert_eq!(
            sut_provenance(sig, &author, &repo, author_pds, cid_ok),
            oracle_provenance(sig, &author, &repo, author_pds, cid_ok)
        );
    }

    /// CORE-4 @property @US-BRA-010 @AC-010.5 @AC-006.4 @BR-1 @BR-2 @C4a @contract-shape:pure-function
    /// Reconcile offers only keys never seen before (any state), counts the
    /// derived keys already published / declined, and is idempotent: feeding
    /// its new keys back as pending yields nothing new.
    #[test]
    #[ignore = "DELIVER R2: unskip one-at-a-time (CORE-4 reconcile)"]
    fn reconcile_offers_only_unseen_keys_and_is_idempotent(
        existing in prop::collection::btree_map(arb_key(), arb_state(), 0..12),
        derived in prop::collection::vec(arb_key(), 0..12),
    ) {
        let existing_rows: Vec<(Key, State)> = existing.iter().map(|(k, s)| (k.clone(), *s)).collect();
        let out = sut_reconcile(&existing_rows, &derived);
        let derived_set: BTreeSet<Key> = derived.iter().cloned().collect();
        let expected_new: BTreeSet<Key> = derived_set.iter().filter(|k| !existing.contains_key(*k)).cloned().collect();
        prop_assert_eq!(&out.new, &expected_new);
        let published = derived_set.iter().filter(|k| matches!(existing.get(*k), Some(State::Published) | Some(State::Retracted))).count();
        let declined = derived_set.iter().filter(|k| existing.get(*k) == Some(&State::Declined)).count();
        prop_assert_eq!(out.already_published, published);
        prop_assert_eq!(out.declined_hidden, declined);
        let mut after = existing_rows.clone();
        after.extend(out.new.iter().map(|k| (k.clone(), State::Pending)));
        prop_assert!(sut_reconcile(&after, &derived).new.is_empty());
    }

    /// CORE-5 @property @US-BRA-005 @AC-005.1 @AC-005.4 @BR-4 @C1a @C1b @C6a @contract-shape:pure-function
    /// Every hundredth from 0.00 to 1.00 parses to exactly round(v × 10000);
    /// anything else is refused with the field guidance.
    #[test]
    #[ignore = "DELIVER R1: unskip one-at-a-time (CORE-5 confidence parse)"]
    fn confidence_parses_every_hundredth_and_refuses_everything_else(
        hundredths in 0i64..=100,
        junk in prop_oneof![
            "[0-9]{0,2}\\.[0-9]{3,4}",
            "-[0-9]\\.[0-9]{1,2}",
            "[2-9]\\.[0-9]{0,2}",
            "[a-z ,]{0,5}",
            "1\\.[0-9]*[1-9][0-9]*",
        ]
    ) {
        let typed = format!("{}.{:02}", hundredths / 100, hundredths % 100);
        prop_assert_eq!(sut_parse_confidence(&typed), Ok(hundredths * 100));
        match oracle_confidence(&junk) {
            Some(bp) => prop_assert_eq!(sut_parse_confidence(&junk), Ok(bp)),
            None => {
                let err = sut_parse_confidence(&junk).expect_err("refused");
                prop_assert_eq!(err, "Enter a number from 0.00 to 1.00".to_string());
            }
        }
    }

    /// CORE-6 @property @US-BRA-004 @AC-004.4 @AC-004.5 @NFR-BRA-7 @ADR-071 @contract-shape:pure-function
    /// A publish plan's record is self-attested (bare author, no signature,
    /// lexicon keys only, basis points), its key is its recomputed CID, and
    /// the preview shows every written value plus "not as truth".
    #[test]
    #[ignore = "DELIVER WS: unskip one-at-a-time (CORE-6 plan == record)"]
    fn a_publish_plan_record_is_its_preview_and_its_key_is_its_cid(
        did in arb_did(), key in arb_key(), bp in 0i64..=10_000,
        evidence in prop::collection::vec("https://github\\.com/[a-z]{1,8}/[a-z]{1,8}", 1..4)
    ) {
        let plan = sut_publish_plan(&did, &key.0, &key.2, &evidence, bp, "2026-10-04T15:02:11Z");
        prop_assert_eq!(&recomputed_cid(&plan.record), &plan.rkey);
        prop_assert_eq!(&plan.record["author"], &serde_json::json!(did));
        prop_assert!(plan.record.get("signature").is_none());
        prop_assert_eq!(&plan.record["confidence"], &serde_json::json!(bp));
        for k in plan.record.as_object().unwrap().keys() {
            prop_assert!(["$type", "subject", "predicate", "object", "evidence", "confidence", "author", "composedAt", "references"].contains(&k.as_str()));
        }
        for shown in [key.0.as_str(), key.2.as_str(), &bp.to_string(), "not as truth"] {
            prop_assert!(plan.preview_text.contains(shown), "preview shows {}", shown);
        }
    }

    /// CORE-7 @property @US-BRA-008 @AC-008.3 @I-BRA-6 @C1b @contract-shape:pure-function
    /// The share post names only published, non-retracted philosophies, fits
    /// 300 characters, and its link facet covers bytes of the text and points
    /// at the profile.
    #[test]
    #[ignore = "DELIVER R1: unskip one-at-a-time (CORE-7 share text)"]
    fn the_share_post_names_only_live_published_claims_and_links_the_profile(
        claims in prop::collection::vec((arb_key(), any::<bool>()), 1..12)
    ) {
        let published: Vec<(String, bool)> = claims.iter().map(|(k, retracted)| (k.2.clone(), *retracted)).collect();
        let url = "https://app.openlore.jeffbailey.us/@priyaraman.bsky.social";
        let post = sut_share_post(url, &published);
        prop_assert!(post.text.chars().count() <= 300);
        let live: BTreeSet<&str> = published.iter().filter(|(_, r)| !r).map(|(o, _)| o.rsplit('.').next().unwrap()).collect();
        for (object, _) in published.iter().filter(|(o, r)| *r && !live.contains(o.rsplit('.').next().unwrap())) {
            prop_assert!(!post.text.contains(object.rsplit('.').next().unwrap()), "retracted {} not mentioned", object);
        }
        prop_assert!(post.facet_start < post.facet_end && post.facet_end <= post.text.len());
        prop_assert!(post.text.is_char_boundary(post.facet_start) && post.text.is_char_boundary(post.facet_end));
        prop_assert_eq!(post.facet_uri.as_str(), url);
    }

    /// CORE-8 @property @US-BRA-006 @US-BRA-011 @C2a @C2b @state-machine @contract-shape:pure-function
    /// The suggestion lifecycle is the state machine
    /// pending ⇄ declined, pending → published → retracted; every other event
    /// is refused in every state, over arbitrary event sequences.
    #[test]
    #[ignore = "DELIVER R1: unskip one-at-a-time (CORE-8 lifecycle state machine)"]
    fn the_suggestion_lifecycle_allows_only_its_legal_transitions(
        events in prop::collection::vec(arb_event(), 0..20)
    ) {
        let mut model = State::Pending;
        let mut real = State::Pending;
        for event in events {
            let expected = oracle_transition(model, event);
            prop_assert_eq!(sut_transition(real, event), expected);
            if let Some(next) = expected {
                model = next;
                real = next;
            }
        }
    }

    /// CORE-9 @property @US-BRA-010 @AC-010.2 @D-12 @contract-shape:pure-function
    /// A suggestion is visible and approvable iff it is pending AND the
    /// GitHub link is currently verified (hidden, never deleted).
    #[test]
    #[ignore = "DELIVER R2: unskip one-at-a-time (CORE-9 derived visibility)"]
    fn a_suggestion_is_approvable_only_while_pending_and_verified(state in arb_state(), verified in any::<bool>()) {
        prop_assert_eq!(sut_visible_and_approvable(state, verified), state == State::Pending && verified);
    }

    /// CORE-10 @property @ADR-076 @OD-BRA-12 @C1b @contract-shape:pure-function
    /// A budget allows an action iff fewer than `limit` actions happened
    /// inside the window; actions older than the window never count.
    #[test]
    #[ignore = "DELIVER WS: unskip one-at-a-time (CORE-10 budget arithmetic)"]
    fn a_budget_allows_exactly_while_under_its_limit_within_the_window(
        ages in prop::collection::vec(0u64..200_000, 0..15), limit in 1usize..10, window in 1u64..100_000
    ) {
        let inside = ages.iter().filter(|a| **a < window).count();
        prop_assert_eq!(sut_budget_allows(&ages, limit, window), inside < limit);
    }

    /// CORE-11 @property @US-BRA-003 @BR-3 @green-today-regression-guard @contract-shape:pure-function
    /// Repo selection never picks a fork or an archived repo and never more
    /// than asked (the SHIPPED `scraper_domain::select_person_repos`).
    #[test]
    #[ignore = "DELIVER WS: unskip one-at-a-time (CORE-11 BR-3 regression guard, green today)"]
    fn repo_selection_never_picks_a_fork_or_an_archived_repo(
        repos in prop::collection::vec(("[a-z]{1,8}", 0u64..500, any::<bool>(), any::<bool>()), 0..15),
        n in 1usize..10
    ) {
        let owned: Vec<ports::OwnedRepo> = repos
            .iter()
            .map(|(name, stars, fork, archived)| ports::OwnedRepo {
                full_name: format!("priyaraman/{name}"),
                description: None,
                language: None,
                stars: *stars,
                fork: *fork,
                archived: *archived,
                pushed_at: Some("2026-09-01T00:00:00Z".to_string()),
            })
            .collect();
        let selection = scraper_domain::select_person_repos(&owned, n);
        prop_assert!(selection.chosen.len() <= n);
        prop_assert!(selection.chosen.iter().all(|r| !r.fork && !r.archived));
    }

    /// CORE-12 @property @US-BRA-001 @AC-001.6 @contract-shape:pure-function
    /// A sign-in is pinned: accepted iff the token's subject is byte-equal to
    /// the DID the handle resolved to.
    #[test]
    #[ignore = "DELIVER WS: unskip one-at-a-time (CORE-12 sign-in pin)"]
    fn a_sign_in_is_accepted_only_for_the_did_the_handle_resolved_to(a in arb_did(), b in arb_did()) {
        prop_assert!(sut_sign_in_pin(&a, &a));
        prop_assert_eq!(sut_sign_in_pin(&a, &b), a == b);
    }
}

/// CORE-1b — the ADR-076 key examples, pinned (C1 boundary table; examples @contract-shape:pure-function
/// are kept alongside the property so a regression names the exact case).
#[test]
#[ignore = "DELIVER WS: unskip one-at-a-time (CORE-1b ownership key examples)"]
fn ownership_verdict_key_examples() {
    let did = "did:plc:7x3kq2mzv5rj4w6hbn2tqclp";
    let cases: [(Option<&str>, Verdict); 7] = [
        (
            Some("Rust, tide models. did:plc:7x3kq2mzv5rj4w6hbn2tqclp"),
            Verdict::Verified,
        ),
        (
            Some("(did:plc:7x3kq2mzv5rj4w6hbn2tqclp)."),
            Verdict::Verified,
        ),
        (
            Some("did:plc:7x3kq2mzv5rj4w6hbn2tqclpx"),
            Verdict::DifferentDid("did:plc:7x3kq2mzv5rj4w6hbn2tqclpx".into()),
        ),
        (Some("@priyaraman.bsky.social"), Verdict::DidMissing),
        (
            Some("old: did:plc:ab12cd34ef56gh78ij90klmn"),
            Verdict::DifferentDid("did:plc:ab12cd34ef56gh78ij90klmn".into()),
        ),
        (Some(""), Verdict::NoBio),
        (None, Verdict::NoBio),
    ];
    for (bio, expected) in cases {
        assert_eq!(
            oracle_verdict(bio, did),
            expected,
            "oracle self-check for {bio:?}"
        );
        assert_eq!(sut_ownership_verdict(bio, did), expected, "bio {bio:?}");
    }
}
