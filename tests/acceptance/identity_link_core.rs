//! github-did-identity-link — layer-2 PURE-CORE properties (ADR-007 functional
//! core; Mandate 9: PBT full at layers 1-2).
//!
//! The decisions this feature adds are total pure functions in the NEW module
//! `claim_domain::identity_link` (architecture-design.md "Component
//! boundaries"; brief "Pure seams for property tests") plus the scoring choke
//! point. Their contracts are stated here over generated inputs and checked
//! against ORACLES written from the ADR text — never from an implementation:
//!
//! | ID | Contract | Source |
//! |---|---|---|
//! | CORE-1 | bio proof: exact, case-sensitive DID token; trailing punctuation stripped; several DIDs allowed; no/blank bio = `no_bio` | ADR-076 §4 (hoisted, ADR-085 §1) |
//! | CORE-2 | check decision over ≤ 2 reads: same id → that bio; 404 / other id → the id read's bio or `account_not_found`; rate limit or outage on a consulted read records nothing; closed verdict set (never `identity_changed`) | ADR-085 §4, UC-10 |
//! | CORE-3 | `project`: a link iff provenance passed (app-signed or self-attested from the author's PDS) ∧ predicate `sameAs` ∧ bare author == subject ∧ object `github:<valid login>` ∧ exactly one id URI ∧ profile URL login == object login (case-insensitive) | ADR-084 §3, M-2, N-4 |
//! | CORE-4 | codec: the wire record the production encoder emits (`sut_encode_link`) → real `decode_claim_record` → real canonicalize/compute_cid (deterministic CID) → project returns the same DID, login and id | ADR-084 Earned Trust |
//! | CORE-5 | `link_view` association rule: associated iff Active ∧ this reader's latest non-future check Verified ∧ no other Active DID Verified for the id; Verified pairs ≥ 2 → Contested; NotChecked / Disproved never associate nor contest; retracted / superseded never shown | ADR-085 §5, GDL-D14, H-2 |
//! | CORE-6 | scoring preservation: `score(F ∪ S) == score(F)` for every feed F and every set S of `sameAs` rows (unbounded preservation) | ADR-085 §6, KPI-GDL-2 |
//! | CORE-7 | reserved-predicate guard: reserved iff the predicate is exactly `sameAs` | ADR-084 §4, N-1 |
//!
//! ## Binding seam (Mandate 7)
//!
//! Every property calls a `sut_*` binding that makes ONE call into the
//! production function and maps its ADTs onto the view types here (the view
//! types are the observable contract). CORE-1..5 and CORE-7 bind to
//! `claim_domain::identity_link`, which does not exist yet: their bindings are
//! `todo!()` (RED, not BROKEN — the file compiles today). CORE-6 binds to the
//! REAL `scoring::score`, which today scores a `sameAs` row as a pairing (RED
//! for the right reason). Revised by ADR-086/087 (owner-only-signing): when the
//! drop moves into `scoring::score_attested`, re-point `sut_score` — one line.
//!
//! Tier B (state-machine PBT) is not added: `link_view` is a pure projection of
//! (links, lifecycles, checks, now) — a FORALL property over generated states
//! explores the same space without a command protocol (DD-GDL-6).
//!
//! Mandate 4 (pure function extraction): these are the pure decisions the
//! DESIGN already extracts into `claim-domain`; the layer-3/4 suites only test
//! the thin adapters and wiring around them (review M-12).

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use chrono::{TimeZone, Utc};
use claim_domain::{Cid, Did};
use ports::{AttributedClaim, AuthorRelationship};
use proptest::prelude::*;
use scoring::ScoringConfig;

const SAME_AS: &str = "sameAs";

// =============================================================================
// Observable view types (the contract)
// =============================================================================

/// The five recorded verdicts (ADR-085 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum VerdictView {
    Verified,
    DidMissing,
    DifferentDid,
    NoBio,
    AccountNotFound,
}

/// One GitHub read as the adapter reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ReadView {
    Profile {
        id: u64,
        login: String,
        bio: Option<String>,
    },
    NotFound,
    RateLimited,
    Unavailable,
}

/// The link being checked: whose, which account, which login to read first.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkedView {
    did: String,
    id: u64,
    last_login: String,
}

/// What a check decides: the recorded verdict + login seen (None = nothing
/// recorded). Which reads were consulted is observed at layer 3 through the
/// GitHub request log (IV-2..IV-4), not here (review M-2).
#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckView {
    recorded: Option<(VerdictView, String)>,
}

/// How a claim was proven on the DID side (ADR-071 provenance verdict).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProvenanceView {
    AppSigned,
    /// App-signed, but under another DID's application key.
    AppSignedByAnotherDid,
    SelfAttestedFromAuthorPds,
    SelfAttestedFromElsewhere,
    /// Verification refused (ADR-071 `ProvenanceRejection`): there is no
    /// accepted `Provenance` to hand to `project`, so nothing projects.
    Failed,
}

/// The claim fields `project` reads.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ClaimView {
    author: String,
    subject: String,
    predicate: String,
    object: String,
    evidence: Vec<String>,
}

/// A projected link: (bare DID, login as signed, numeric id).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct LinkView {
    did: String,
    login: String,
    id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LifecycleView {
    Active,
    Retracted,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectedLinkView {
    did: String,
    id: u64,
    login: String,
    lifecycle: LifecycleView,
}

/// One local check; `days_from_now` < 0 is in the past, > 0 in the future.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckInputView {
    did: String,
    id: u64,
    verdict: VerdictView,
    days_from_now: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ShownView {
    Associated,
    NotChecked,
    ProofGone(VerdictView),
    Contested,
}

// =============================================================================
// Binding seam — each binds to ONE production call
// =============================================================================

fn sut_bio_verdict(bio: Option<&str>, did: &str) -> VerdictView {
    match claim_domain::identity_link::bio_verdict(bio, did) {
        claim_domain::identity_link::BioVerdict::Verified => VerdictView::Verified,
        claim_domain::identity_link::BioVerdict::DidMissing => VerdictView::DidMissing,
        claim_domain::identity_link::BioVerdict::DifferentDid(_) => VerdictView::DifferentDid,
        claim_domain::identity_link::BioVerdict::NoBio => VerdictView::NoBio,
    }
}

fn sut_check(link: &LinkedView, login_read: &ReadView, id_read: &ReadView) -> CheckView {
    use claim_domain::identity_link::{recheck_link, CheckVerdict, GithubObservation, GithubRead};
    let answer = |read: &ReadView| match read {
        ReadView::Profile { id, login, bio } => GithubRead::Profile(GithubObservation {
            login: login.clone(),
            user_id: *id,
            bio: bio.clone(),
        }),
        ReadView::NotFound => GithubRead::NotFound,
        ReadView::RateLimited | ReadView::Unavailable => GithubRead::Unreadable(()),
    };
    let linked = claim_domain::identity_link::IdentityLink {
        did: link.did.clone(),
        login: link.last_login.clone(),
        user_id: link.id,
    };
    let decided = recheck_link(&linked, &link.last_login, answer(login_read), |_| {
        answer(id_read)
    });
    CheckView {
        recorded: decided.ok().map(|check| {
            let verdict = match check.verdict() {
                CheckVerdict::Verified => VerdictView::Verified,
                CheckVerdict::DidMissing => VerdictView::DidMissing,
                CheckVerdict::DifferentDid => VerdictView::DifferentDid,
                CheckVerdict::NoBio => VerdictView::NoBio,
                CheckVerdict::AccountNotFound => VerdictView::AccountNotFound,
            };
            (verdict, check.login_seen().to_string())
        }),
    }
}

fn sut_project(claim: &ClaimView, provenance: ProvenanceView) -> Option<LinkView> {
    // Every ACCEPTED provenance — passed for the author or not — reaches
    // `project`, so `project` itself must refuse one that did not pass for the
    // author. Only `Failed` has no accepted `Provenance` value to pass.
    let bare_author = claim_domain::bare_did(&claim.author);
    let other_did = format!("{bare_author}-elsewhere");
    let accepted = match provenance {
        ProvenanceView::AppSigned => Some(claim_domain::Provenance::AppSigned {
            key_id: format!("{}#org.openlore.application", claim.author),
        }),
        ProvenanceView::AppSignedByAnotherDid => Some(claim_domain::Provenance::AppSigned {
            key_id: format!("{other_did}#org.openlore.application"),
        }),
        ProvenanceView::SelfAttestedFromAuthorPds => Some(claim_domain::Provenance::SelfAttested {
            repo_did: Did(claim.author.clone()),
        }),
        ProvenanceView::SelfAttestedFromElsewhere => Some(claim_domain::Provenance::SelfAttested {
            repo_did: Did(other_did),
        }),
        ProvenanceView::Failed => None,
    };
    let fields = claim_domain::identity_link::ClaimFields {
        author: &claim.author,
        subject: &claim.subject,
        predicate: &claim.predicate,
        object: &claim.object,
        evidence: &claim.evidence,
    };
    accepted
        .and_then(|provenance| claim_domain::identity_link::project(fields, &provenance))
        .map(|link| LinkView {
            did: link.did,
            login: link.login,
            id: link.user_id,
        })
}

/// The production wire form of a proven link: the self-attested
/// `org.openlore.claim` record JSON the `LinkDraft` encoder emits for the
/// review app's create-only write (ADR-084 §2). The app-signed CLI wire form
/// is covered at layer 3 (WS-GDL-1 decodes the real record at the PDS).
fn sut_encode_link(did: &str, login: &str, id: u64) -> serde_json::Value {
    let observation = claim_domain::identity_link::GithubObservation {
        login: login.to_string(),
        user_id: id,
        bio: Some(did.to_string()),
    };
    claim_domain::identity_link::self_attested_link_record(
        did,
        &observation,
        "2026-10-01T09:00:00Z",
    )
    .expect("a bio naming the DID proves the link")
}

/// DESIGN signature: `link_view(links, lineages, checks)`. The view type folds
/// each link's ADR-060 lineage into `ProjectedLinkView::lifecycle` (Active |
/// Retracted | Superseded, same-author markers only); the binding unfolds it
/// into the `lineages` argument when it lands (review M-3).
fn sut_link_view(
    links: &[ProjectedLinkView],
    checks: &[CheckInputView],
) -> BTreeMap<(String, u64), ShownView> {
    use claim_domain::identity_link::{
        link_view, CheckVerdict, IdentityLink, LatestCheck, LinkLifecycle, LinkStanding, ReaderLink,
    };
    let now = Utc.with_ymd_and_hms(2026, 10, 15, 12, 0, 0).unwrap();
    let verdict_of = |verdict: VerdictView| match verdict {
        VerdictView::Verified => CheckVerdict::Verified,
        VerdictView::DidMissing => CheckVerdict::DidMissing,
        VerdictView::DifferentDid => CheckVerdict::DifferentDid,
        VerdictView::NoBio => CheckVerdict::NoBio,
        VerdictView::AccountNotFound => CheckVerdict::AccountNotFound,
    };
    let view_of = |verdict: CheckVerdict| match verdict {
        CheckVerdict::Verified => VerdictView::Verified,
        CheckVerdict::DidMissing => VerdictView::DidMissing,
        CheckVerdict::DifferentDid => VerdictView::DifferentDid,
        CheckVerdict::NoBio => VerdictView::NoBio,
        CheckVerdict::AccountNotFound => VerdictView::AccountNotFound,
    };
    let reader_links: Vec<ReaderLink> = links
        .iter()
        .map(|link| ReaderLink {
            link: IdentityLink {
                did: link.did.clone(),
                login: link.login.clone(),
                user_id: link.id,
            },
            lifecycle: match link.lifecycle {
                LifecycleView::Active => LinkLifecycle::Active,
                LifecycleView::Retracted => LinkLifecycle::Retracted,
                LifecycleView::Superseded => LinkLifecycle::Superseded,
            },
        })
        .collect();
    let recorded: Vec<LatestCheck> = checks
        .iter()
        .map(|check| LatestCheck {
            subject_did: check.did.clone(),
            user_id: check.id,
            login_seen: format!("login{}", check.id),
            verdict: verdict_of(check.verdict),
            checked_at: now + chrono::Duration::days(check.days_from_now),
        })
        .collect();
    link_view(&reader_links, &recorded, now)
        .into_iter()
        .map(|shown| {
            let view = match shown.standing {
                LinkStanding::Associated { .. } => ShownView::Associated,
                LinkStanding::NotChecked => ShownView::NotChecked,
                LinkStanding::ProofGone { verdict, .. } => ShownView::ProofGone(view_of(verdict)),
                LinkStanding::Contested => ShownView::Contested,
            };
            ((shown.link.did, shown.link.user_id), view)
        })
        .collect()
}

fn sut_is_reserved_predicate(predicate: &str) -> bool {
    claim_domain::identity_link::is_reserved_predicate(predicate)
}

/// The scoring choke point (REAL today).
fn sut_score(claims: &[AttributedClaim]) -> scoring::WeightedView {
    scoring::score(claims, &ScoringConfig::DEFAULT)
}

// =============================================================================
// Oracles (from the ADR text)
// =============================================================================

fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '%' | '-')
}

fn is_did_shaped(token: &str) -> bool {
    let mut parts = token.splitn(3, ':');
    let (scheme, method, rest) = (
        parts.next(),
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    );
    scheme == Some("did")
        && !method.is_empty()
        && method.chars().all(|c| c.is_ascii_lowercase())
        && !rest.is_empty()
}

/// ADR-076 §4: maximal runs of `[A-Za-z0-9._:%-]`, sentence punctuation
/// stripped from the end, DID-shaped, byte-equal to the DID.
fn oracle_bio_verdict(bio: Option<&str>, did: &str) -> VerdictView {
    let Some(bio) = bio.filter(|b| !b.trim().is_empty()) else {
        return VerdictView::NoBio;
    };
    let tokens: Vec<&str> = bio
        .split(|c: char| !is_token_char(c))
        .map(|t| t.trim_end_matches(|c: char| ".,;:!?)]}>".contains(c)))
        .filter(|t| is_did_shaped(t))
        .collect();
    if tokens.contains(&did) {
        VerdictView::Verified
    } else if tokens.is_empty() {
        VerdictView::DidMissing
    } else {
        VerdictView::DifferentDid
    }
}

/// ADR-085 §4.
fn oracle_check(link: &LinkedView, login_read: &ReadView, id_read: &ReadView) -> CheckView {
    let matched = |bio: &Option<String>, login: &str| {
        Some((
            oracle_bio_verdict(bio.as_deref(), &link.did),
            login.to_string(),
        ))
    };
    match login_read {
        ReadView::RateLimited | ReadView::Unavailable => CheckView { recorded: None },
        ReadView::Profile { id, login, bio } if *id == link.id => CheckView {
            recorded: matched(bio, login),
        },
        ReadView::Profile { .. } | ReadView::NotFound => CheckView {
            recorded: match id_read {
                ReadView::Profile { login, bio, .. } => matched(bio, login),
                ReadView::NotFound => Some((VerdictView::AccountNotFound, link.last_login.clone())),
                ReadView::RateLimited | ReadView::Unavailable => None,
            },
        },
    }
}

fn is_github_login(login: &str) -> bool {
    (1..=39).contains(&login.len())
        && !login.starts_with('-')
        && login.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// ADR-084 §3.
fn oracle_project(claim: &ClaimView, provenance: ProvenanceView) -> Option<LinkView> {
    let provenance_ok = matches!(
        provenance,
        ProvenanceView::AppSigned | ProvenanceView::SelfAttestedFromAuthorPds
    );
    let bare_author = claim.author.split('#').next().unwrap_or_default();
    let login = claim.object.strip_prefix("github:")?;
    let ids: Vec<u64> = claim
        .evidence
        .iter()
        .filter_map(|e| e.strip_prefix("https://api.github.com/user/"))
        .map(|digits| {
            if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                digits.parse::<u64>().ok()
            } else {
                None
            }
        })
        .collect::<Option<Vec<u64>>>()?;
    let profile_matches = claim.evidence.iter().any(|e| {
        e.strip_prefix("https://github.com/")
            .is_some_and(|l| l.eq_ignore_ascii_case(login))
    });
    (provenance_ok
        && claim.predicate == SAME_AS
        && bare_author == claim.subject
        && is_github_login(login)
        && ids.len() == 1
        && profile_matches)
        .then(|| LinkView {
            did: claim.subject.clone(),
            login: login.to_string(),
            id: ids[0],
        })
}

/// ADR-085 §5 "Future-dated checks are ignored" (amended 2026-10-15, DELIVER
/// Q-RM-3; DISTILL Q-GDL-D4).
fn oracle_link_view(
    links: &[ProjectedLinkView],
    checks: &[CheckInputView],
) -> BTreeMap<(String, u64), ShownView> {
    let active: BTreeSet<(String, u64)> = links
        .iter()
        .filter(|l| l.lifecycle == LifecycleView::Active)
        .map(|l| (l.did.clone(), l.id))
        .collect();
    let latest = |key: &(String, u64)| {
        checks
            .iter()
            .enumerate()
            .filter(|(_, c)| c.did == key.0 && c.id == key.1 && c.days_from_now <= 0)
            .max_by_key(|(i, c)| (c.days_from_now, *i))
            .map(|(_, c)| c.verdict)
    };
    let verified_per_id: BTreeMap<u64, usize> = active
        .iter()
        .filter(|k| latest(k) == Some(VerdictView::Verified))
        .fold(BTreeMap::new(), |mut m, (_, id)| {
            *m.entry(*id).or_insert(0) += 1;
            m
        });
    active
        .iter()
        .map(|key| {
            let shown = match latest(key) {
                None => ShownView::NotChecked,
                Some(VerdictView::Verified) if verified_per_id[&key.1] >= 2 => ShownView::Contested,
                Some(VerdictView::Verified) => ShownView::Associated,
                Some(other) => ShownView::ProofGone(other),
            };
            (key.clone(), shown)
        })
        .collect()
}

/// Decode the encoder's own wire record with the REAL claim-domain decoder and
/// re-hash it — no hand-built JSON: the bytes under test are the production
/// encoder's output.
fn wire_round_trip(body: &serde_json::Value) -> (ClaimView, String) {
    let record = claim_domain::decode_claim_record(body, "").expect("the link decodes");
    let unsigned = record.unsigned();
    let canonical = claim_domain::canonicalize(unsigned).expect("canonicalize");
    let cid = claim_domain::compute_cid(&canonical).0;
    assert_eq!(cid, record.cid().0, "decode and re-hash agree on the CID");
    let decoded = ClaimView {
        author: unsigned.author_did.0.clone(),
        subject: unsigned.subject.clone(),
        predicate: unsigned.predicate.clone(),
        object: unsigned.object.clone(),
        evidence: unsigned.evidence.clone(),
    };
    (decoded, cid)
}

// =============================================================================
// Generators
// =============================================================================

fn arb_did() -> impl Strategy<Value = String> {
    "did:plc:[a-z0-9]{6,12}"
}

fn arb_login() -> impl Strategy<Value = String> {
    "[A-Za-z0-9][A-Za-z0-9-]{0,20}"
}

/// A bio assembled from words, other DIDs and near-misses of `did`.
fn arb_bio_for(did: String) -> impl Strategy<Value = Option<String>> {
    let piece = prop_oneof![
        "[A-Za-z]{1,8}",
        Just(did.clone()),
        Just(format!("{did}x")),
        Just(did.to_ascii_uppercase()),
        Just(format!("({did}).")),
        Just(format!("{did},")),
        arb_did(),
    ];
    let sep = prop_oneof![Just(" "), Just(", "), Just("\n"), Just(" / ")];
    prop_oneof![
        1 => Just(None),
        1 => Just(Some("   ".to_string())),
        8 => prop::collection::vec((piece, sep), 1..8).prop_map(|parts| {
            Some(parts.into_iter().map(|(p, s)| format!("{p}{s}")).collect())
        }),
    ]
}

fn arb_read(linked_id: u64) -> impl Strategy<Value = ReadView> {
    prop_oneof![
        (arb_login(), proptest::option::of("[ -~]{0,40}")).prop_map(move |(login, bio)| {
            ReadView::Profile {
                id: linked_id,
                login,
                bio,
            }
        }),
        (arb_login(), 1u64..1_000).prop_map(|(login, id)| ReadView::Profile {
            id: 10_000_000_000 + id,
            login,
            bio: None
        }),
        Just(ReadView::NotFound),
        Just(ReadView::RateLimited),
        Just(ReadView::Unavailable),
    ]
}

/// The id read never answers with another account.
fn arb_id_read(linked_id: u64, did: String) -> impl Strategy<Value = ReadView> {
    prop_oneof![
        (arb_login(), arb_bio_for(did)).prop_map(move |(login, bio)| ReadView::Profile {
            id: linked_id,
            login,
            bio
        }),
        Just(ReadView::NotFound),
        Just(ReadView::RateLimited),
        Just(ReadView::Unavailable),
    ]
}

/// One ADR-084 §3 mutation of a sound link (benign ones keep it a link).
#[derive(Debug, Clone)]
enum Mutation {
    None,
    AuthorAppFragment,
    ObjectLoginCaseChanged,
    SubjectIsAnotherDid(String),
    AuthorIsAnotherDid(String),
    PredicateIs(String),
    ObjectNotALogin,
    ProfileLoginDiffers,
    IdUriMissing,
    IdUriTwice,
    IdUriNotDigits,
}

fn arb_mutation() -> impl Strategy<Value = Mutation> {
    prop_oneof![
        Just(Mutation::None),
        Just(Mutation::AuthorAppFragment),
        Just(Mutation::ObjectLoginCaseChanged),
        arb_did().prop_map(Mutation::SubjectIsAnotherDid),
        arb_did().prop_map(Mutation::AuthorIsAnotherDid),
        "[A-Za-z]{0,10}"
            .prop_filter("not sameAs", |p| p != SAME_AS)
            .prop_map(Mutation::PredicateIs),
        Just(Mutation::ObjectNotALogin),
        Just(Mutation::ProfileLoginDiffers),
        Just(Mutation::IdUriMissing),
        Just(Mutation::IdUriTwice),
        Just(Mutation::IdUriNotDigits),
    ]
}

fn sound_link(did: &str, login: &str, id: u64) -> ClaimView {
    ClaimView {
        author: did.to_string(),
        subject: did.to_string(),
        predicate: SAME_AS.to_string(),
        object: format!("github:{login}"),
        evidence: vec![
            format!("https://github.com/{login}"),
            format!("https://api.github.com/user/{id}"),
        ],
    }
}

fn mutate(claim: ClaimView, m: &Mutation) -> ClaimView {
    match m {
        Mutation::None => claim,
        Mutation::AuthorAppFragment => ClaimView {
            author: format!("{}#org.openlore.application", claim.author),
            ..claim
        },
        Mutation::ObjectLoginCaseChanged => ClaimView {
            object: claim
                .object
                .to_ascii_uppercase()
                .replacen("GITHUB:", "github:", 1),
            ..claim
        },
        Mutation::SubjectIsAnotherDid(d) => ClaimView {
            subject: d.clone(),
            ..claim
        },
        Mutation::AuthorIsAnotherDid(d) => ClaimView {
            author: d.clone(),
            ..claim
        },
        Mutation::PredicateIs(p) => ClaimView {
            predicate: p.clone(),
            ..claim
        },
        Mutation::ObjectNotALogin => ClaimView {
            object: format!("{}/repo", claim.object),
            ..claim
        },
        Mutation::ProfileLoginDiffers => ClaimView {
            evidence: vec![
                "https://github.com/zz-someone-else".to_string(),
                claim.evidence[1].clone(),
            ],
            ..claim
        },
        Mutation::IdUriMissing => ClaimView {
            evidence: vec![claim.evidence[0].clone()],
            ..claim
        },
        Mutation::IdUriTwice => ClaimView {
            evidence: vec![
                claim.evidence[0].clone(),
                claim.evidence[1].clone(),
                "https://api.github.com/user/7".to_string(),
            ],
            ..claim
        },
        Mutation::IdUriNotDigits => ClaimView {
            evidence: vec![
                claim.evidence[0].clone(),
                "https://api.github.com/user/12ab".to_string(),
            ],
            ..claim
        },
    }
}

fn arb_provenance() -> impl Strategy<Value = ProvenanceView> {
    prop_oneof![
        Just(ProvenanceView::AppSigned),
        Just(ProvenanceView::AppSignedByAnotherDid),
        Just(ProvenanceView::SelfAttestedFromAuthorPds),
        Just(ProvenanceView::SelfAttestedFromElsewhere),
        Just(ProvenanceView::Failed),
    ]
}

/// Random printable predicates practically never hit `sameAs`, so the exact
/// literal and its near-misses (case, surrounding space) are mixed in.
fn arb_predicate() -> impl Strategy<Value = String> {
    prop_oneof![
        2 => Just(SAME_AS.to_string()),
        2 => proptest::sample::select(SAME_AS_NEAR_MISSES).prop_map(str::to_string),
        3 => "[ -~]{0,12}",
    ]
}

/// Predicates that look like `sameAs` but are not it (ADR-084 §4: exact match).
const SAME_AS_NEAR_MISSES: &[&str] = &[
    "SameAs", "sameas", "SAMEAS", "sameAS", " sameAs", "sameAs ", "sameAs\t", "same As", "sameA",
    "sameAss",
];

/// A small universe of DIDs × ids so collisions (contest, duplicates) happen.
fn arb_reader_state() -> impl Strategy<Value = (Vec<ProjectedLinkView>, Vec<CheckInputView>)> {
    let did = prop_oneof![
        Just("did:plc:aa".to_string()),
        Just("did:plc:bb".to_string()),
        Just("did:plc:cc".to_string())
    ];
    let id = prop_oneof![Just(1u64), Just(2u64)];
    let lifecycle = prop_oneof![
        6 => Just(LifecycleView::Active),
        1 => Just(LifecycleView::Retracted),
        1 => Just(LifecycleView::Superseded),
    ];
    let verdict = prop_oneof![
        3 => Just(VerdictView::Verified),
        1 => Just(VerdictView::DidMissing),
        1 => Just(VerdictView::DifferentDid),
        1 => Just(VerdictView::NoBio),
        1 => Just(VerdictView::AccountNotFound),
    ];
    let link =
        (did.clone(), id.clone(), lifecycle).prop_map(|(did, id, lifecycle)| ProjectedLinkView {
            did,
            id,
            login: format!("login{id}"),
            lifecycle,
        });
    let check = (did, id, verdict, -40i64..=5).prop_map(|(did, id, verdict, days_from_now)| {
        CheckInputView {
            did,
            id,
            verdict,
            days_from_now,
        }
    });
    (
        prop::collection::vec(link, 0..6),
        prop::collection::vec(check, 0..10),
    )
}

/// A scoring feed over a small universe (shape of `scoring_core.rs`).
fn arb_feed() -> impl Strategy<Value = Vec<AttributedClaim>> {
    let subject = prop_oneof![
        Just("github:deno"),
        Just("github:cargo"),
        Just("github:nixpkgs")
    ];
    let object = prop_oneof![Just("dependency-pinning"), Just("immutability")];
    let author = prop_oneof![
        Just("did:plc:tobias"),
        Just("did:plc:maria"),
        Just("did:plc:rachel")
    ];
    let one = (subject, object, author, 0.0_f64..=1.0).prop_map(|(s, o, a, c)| AttributedClaim {
        author_did: Did(a.to_string()),
        cid: Cid(format!("bafy-{s}-{o}-{a}")),
        subject: s.to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: o.to_string(),
        confidence: c,
        composed_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        relationship: AuthorRelationship::SubscribedPeer,
    });
    prop::collection::vec(one, 0..10)
}

/// `sameAs` rows: author's own DID as subject, `github:<login>` as object.
fn arb_same_as_rows() -> impl Strategy<Value = Vec<AttributedClaim>> {
    let author = prop_oneof![
        Just("did:plc:tobias"),
        Just("did:plc:maria"),
        Just("did:plc:rachel")
    ];
    let row = (author, arb_login()).prop_map(|(a, login)| AttributedClaim {
        author_did: Did(a.to_string()),
        cid: Cid(format!("bafy-link-{a}-{login}")),
        subject: a.to_string(),
        predicate: SAME_AS.to_string(),
        object: format!("github:{login}"),
        confidence: 1.0,
        composed_at: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        relationship: AuthorRelationship::SubscribedPeer,
    });
    prop::collection::vec(row, 1..4)
}

// =============================================================================
// Properties
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// CORE-1 (@property @adr-076 @kpi-gdl-1 @contract-shape:pure-function)
    #[test]
    fn core1_the_bio_proves_a_did_only_by_its_exact_token(
        (did, bio) in arb_did().prop_flat_map(|d| (Just(d.clone()), arb_bio_for(d)))
    ) {
        prop_assert_eq!(sut_bio_verdict(bio.as_deref(), &did), oracle_bio_verdict(bio.as_deref(), &did));
    }

    /// CORE-2 (@property @adr-085-4 @uc-10 @contract-shape:pure-function)
    #[test]
    fn core2_a_check_reads_at_most_twice_and_records_nothing_when_github_fails(
        (link, login_read, id_read) in (arb_did(), arb_login(), 1u64..10_000_000)
            .prop_flat_map(|(did, last_login, id)| {
                let link = LinkedView { did: did.clone(), id, last_login };
                (Just(link), arb_read(id), arb_id_read(id, did))
            })
    ) {
        let decided = sut_check(&link, &login_read, &id_read);
        prop_assert_eq!(decided, oracle_check(&link, &login_read, &id_read));
    }

    /// CORE-3 (@property @adr-084-3 @m-2 @n-4 @kpi-gdl-1 @contract-shape:pure-function)
    #[test]
    fn core3_only_the_owners_well_formed_record_projects_as_a_link(
        did in arb_did(),
        login in arb_login(),
        id in 1u64..10_000_000_000,
        mutation in arb_mutation(),
        provenance in arb_provenance(),
    ) {
        let claim = mutate(sound_link(&did, &login, id), &mutation);
        prop_assert_eq!(sut_project(&claim, provenance), oracle_project(&claim, provenance));
    }

    /// CORE-4 (@property @adr-084-earned-trust @contract-shape:pure-function):
    /// the production encoder's wire record → REAL `decode_claim_record` →
    /// REAL canonicalize / compute_cid (deterministic) → project == the same
    /// DID, login and id.
    #[test]
    fn core4_an_encoded_link_projects_back_to_the_same_person_and_account(
        did in arb_did(),
        login in arb_login(),
        id in 1u64..10_000_000_000,
    ) {
        let (decoded, cid) = wire_round_trip(&sut_encode_link(&did, &login, id));
        let (_, cid_again) = wire_round_trip(&sut_encode_link(&did, &login, id));
        prop_assert_eq!(&cid, &cid_again, "the CID is deterministic");
        prop_assert_eq!(
            sut_project(&decoded, ProvenanceView::SelfAttestedFromAuthorPds),
            Some(LinkView { did, login, id })
        );
    }

    /// CORE-5 (@property @adr-085-5 @gdl-d14 @h-2 @kpi-gdl-1 @contract-shape:pure-function)
    #[test]
    fn core5_only_this_readers_sole_verified_check_associates_an_account(
        (links, checks) in arb_reader_state()
    ) {
        prop_assert_eq!(sut_link_view(&links, &checks), oracle_link_view(&links, &checks));
    }

    /// CORE-5b anti-griefing (metamorphic): adding another DID's link that is
    /// unchecked or disproved never changes any existing row.
    /// (@property @gdl-d14 @h-2 @contract-shape:unbounded-preservation)
    #[test]
    fn core5b_a_squatters_unproven_link_never_changes_anyone_elses_row(
        (links, checks) in arb_reader_state(),
        disproved in proptest::bool::ANY,
    ) {
        let before = sut_link_view(&links, &checks);
        let squatter = ProjectedLinkView {
            did: "did:plc:squatter".to_string(),
            id: 1,
            login: "login1".to_string(),
            lifecycle: LifecycleView::Active,
        };
        let mut more_links = links.clone();
        more_links.push(squatter);
        let mut more_checks = checks.clone();
        if disproved {
            more_checks.push(CheckInputView {
                did: "did:plc:squatter".to_string(),
                id: 1,
                verdict: VerdictView::DifferentDid,
                days_from_now: 0,
            });
        }
        let after = sut_link_view(&more_links, &more_checks);
        for (key, shown) in before {
            prop_assert_eq!(after.get(&key), Some(&shown), "row {:?} changed", key);
        }
    }

    /// CORE-6 (@property @adr-085-6 @gdl-d8 @kpi-gdl-2 @contract-shape:unbounded-preservation)
    #[test]
    #[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
    fn core6_identity_links_never_change_a_weighted_view(
        feed in arb_feed(),
        links in arb_same_as_rows(),
    ) {
        let expected = sut_score(&feed);
        let appended: Vec<AttributedClaim> = feed.iter().chain(links.iter()).cloned().collect();
        let prepended: Vec<AttributedClaim> = links.iter().chain(feed.iter()).cloned().collect();
        prop_assert_eq!(&sut_score(&appended), &expected);
        prop_assert_eq!(&sut_score(&prepended), &expected);
    }

    /// CORE-7 (@property @adr-084-4 @n-1 @kpi-gdl-1 @contract-shape:pure-function)
    #[test]
    fn core7_only_the_exact_same_as_predicate_is_reserved(predicate in arb_predicate()) {
        prop_assert_eq!(sut_is_reserved_predicate(&predicate), predicate == SAME_AS);
    }
}

// =============================================================================
// Example-pinned catalogue (closed-world rows beside the generators)
// =============================================================================

/// CORE-7x: only the exact `sameAs` literal is reserved; every case or
/// spacing variant of it is an ordinary predicate.
/// (@adr-084-4 @n-1 @contract-shape:pure-function)
#[test]
fn core7x_only_exact_same_as_is_reserved_not_its_look_alikes() {
    assert!(sut_is_reserved_predicate(SAME_AS), "`sameAs` is reserved");
    for near_miss in SAME_AS_NEAR_MISSES {
        assert!(
            !sut_is_reserved_predicate(near_miss),
            "{near_miss:?} is not the reserved predicate"
        );
    }
}

/// CORE-1x: the ADR-085 GitHub lie catalogue, by name.
/// (@adr-085-earned-trust @contract-shape:pure-function)
#[test]
fn core1x_the_bio_lie_catalogue() {
    let did = "did:plc:test-jeff";
    let rows: [(Option<&str>, VerdictView); 9] = [
        (None, VerdictView::NoBio),
        (Some(" \n "), VerdictView::NoBio),
        (Some("I like Rust."), VerdictView::DidMissing),
        (Some("did:plc:test-jeffrey"), VerdictView::DifferentDid),
        (Some("DID:PLC:TEST-JEFF"), VerdictView::DidMissing),
        (Some("did:plc:TEST-JEFF"), VerdictView::DifferentDid),
        (Some("(did:plc:test-jeff)."), VerdictView::Verified),
        (
            Some("did:plc:other did:plc:test-jeff"),
            VerdictView::Verified,
        ),
        (Some("did:plc:test-jeff/x"), VerdictView::Verified),
    ];
    for (bio, expected) in rows {
        assert_eq!(oracle_bio_verdict(bio, did), expected, "oracle row {bio:?}");
        assert_eq!(sut_bio_verdict(bio, did), expected, "{bio:?}");
    }
}

/// CORE-6x: the worked example — one feed, one link; the link's pairing never
/// appears and every other pairing is untouched.
/// (@adr-085-6 @kpi-gdl-2 @contract-shape:unbounded-preservation)
#[test]
#[ignore = "DISTILL github-did-identity-link: pending DELIVER (one at a time)"]
fn core6x_a_link_beside_a_claim_adds_no_pairing() {
    let at = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    let claim = AttributedClaim {
        author_did: Did("did:plc:rachel".to_string()),
        cid: Cid("bafy-claim".to_string()),
        subject: "github:rust-lang/regex".to_string(),
        predicate: "embodiesPhilosophy".to_string(),
        object: "memory-safety".to_string(),
        confidence: 0.6,
        composed_at: at,
        relationship: AuthorRelationship::SubscribedPeer,
    };
    let link = AttributedClaim {
        cid: Cid("bafy-link".to_string()),
        subject: "did:plc:rachel".to_string(),
        predicate: SAME_AS.to_string(),
        object: "github:rachelk".to_string(),
        confidence: 1.0,
        ..claim.clone()
    };
    let view = sut_score(&[claim.clone(), link]);
    assert_eq!(view, sut_score(&[claim]));
    assert!(view.ranked.iter().all(|p| p.subject != "did:plc:rachel"));
}
