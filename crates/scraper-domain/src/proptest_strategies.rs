//! Proptest strategies for `scraper-domain` properties.
//!
//! Pure generators (nw-pbt-rust): each returns a fresh immutable value via
//! `prop_map` over small, named builders — never a giant nested tuple. The
//! strategies generate `ports::Signal` values across every `SignalKind`, so
//! the `derive_candidates` properties in `lib.rs` explore the whole bounded
//! signal space (including collapse, where many signals share a predicate).
//!
//! Gated behind `#[cfg(test)]` so a release build of the pure crate does not
//! compile proptest. A future step (02-*) may promote this to a
//! `proptest-strategies` feature if a downstream crate needs the generators.

use ports::{Signal, SignalKind};
use proptest::prelude::*;

/// Any one of the five bounded [`SignalKind`] variants.
pub fn arb_signal_kind() -> impl Strategy<Value = SignalKind> {
    prop_oneof![
        Just(SignalKind::DependencyManifestPinned),
        Just(SignalKind::DocsPresentAndSubstantial),
        Just(SignalKind::TestRatioOrCiMatrix),
        Just(SignalKind::SemverAndChangelog),
        Just(SignalKind::MemorySafetyLanguage),
    ]
}

/// A single [`Signal`] with an arbitrary kind, printable value, and a
/// GitHub-shaped public URL.
pub fn arb_signal() -> impl Strategy<Value = Signal> {
    (
        arb_signal_kind(),
        "[ -~]{0,64}",
        "https://github\\.com/[a-z0-9-]{1,16}/[a-z0-9-]{1,16}",
    )
        .prop_map(|(kind, value, source_url)| Signal {
            kind,
            value,
            source_url,
        })
}

/// A vector of signals whose KINDS are distinct — exercises the "one candidate
/// per predicate" mapping shape without forcing collapse. Length 0..=5 (the
/// bounded mapping has 5 entries). Useful for the confidence / non-empty /
/// determinism properties where collapse is incidental.
pub fn arb_distinct_signals() -> impl Strategy<Value = Vec<Signal>> {
    proptest::collection::vec(arb_signal(), 0..6).prop_map(dedupe_by_kind)
}

/// Drop later signals that repeat an earlier signal's kind, preserving order.
fn dedupe_by_kind(signals: Vec<Signal>) -> Vec<Signal> {
    let mut seen: Vec<SignalKind> = Vec::new();
    let mut out: Vec<Signal> = Vec::new();
    for signal in signals {
        if !seen.contains(&signal.kind) {
            seen.push(signal.kind);
            out.push(signal);
        }
    }
    out
}

// -----------------------------------------------------------------------------
// contributor-philosophy-inference (DDD-3): raw `/contributors` rows
// -----------------------------------------------------------------------------

/// A raw contributors row drawn from a SMALL id / login space so duplicates
/// (the same user id served twice) and contribution ties actually occur, and
/// from all three bot shapes: typed `Bot`, `[bot]` login typed `User` (the
/// API lie), and plain humans.
pub fn arb_raw_contributor() -> impl Strategy<Value = ports::RawContributor> {
    (
        0u64..40,
        prop_oneof![
            3 => Just("User".to_string()),
            1 => Just("Bot".to_string()),
        ],
        any::<bool>(),
        0u64..50,
    )
        .prop_map(|(user_id, account_type, bracket_bot, contributions)| {
            let login = if bracket_bot {
                format!("helper-{user_id}[BOT]")
            } else {
                format!("dev-{user_id:02}")
            };
            ports::RawContributor {
                login,
                user_id,
                account_type,
                contributions,
            }
        })
}

/// A raw contributors list (0..60 rows, duplicates and bots included).
pub fn arb_raw_contributors() -> impl Strategy<Value = Vec<ports::RawContributor>> {
    proptest::collection::vec(arb_raw_contributor(), 0..60)
}

// -----------------------------------------------------------------------------
// Person inference (contributor-philosophy-inference slices 02/03)
// -----------------------------------------------------------------------------

use crate::people::{
    CitedClaim, Hundredths, PersonCandidate, RepoClaim, SupportingClaim, SupportingRepo,
};
use crate::EMBODIES_PHILOSOPHY;
use ports::{AuthorRelationship, ContributionLink};

/// A confidence in whole hundredths across the full `[0, 100]` range.
pub fn arb_hundredths() -> impl Strategy<Value = Hundredths> {
    (0u32..=100).prop_map(Hundredths::new)
}

/// A supporting claim citation; the author DID sometimes carries the
/// application fragment (the codec must cite it bare, Q-CPI-D3).
pub fn arb_supporting_claim() -> impl Strategy<Value = SupportingClaim> {
    (
        "did:(plc|web):[a-z0-9]{4,16}",
        proptest::option::of(Just("#org.openlore.application")),
        "bafy[a-z2-7]{8,24}",
        arb_hundredths(),
    )
        .prop_map(|(did, fragment, cid, confidence)| SupportingClaim {
            cited: CitedClaim::new(&format!("{did}{}", fragment.unwrap_or("")), &cid),
            confidence,
        })
}

/// A repo supporting an inference, citing at least one claim.
pub fn arb_supporting_repo() -> impl Strategy<Value = SupportingRepo> {
    (
        "github:[A-Za-z0-9-]{1,10}/[a-z0-9._-]{1,10}",
        1u32..=30,
        proptest::collection::vec(arb_supporting_claim(), 1..4),
    )
        .prop_map(|(repo_subject, rank, claims)| SupportingRepo {
            repo_subject,
            rank,
            claims,
        })
}

/// A valid person candidate: 1..=4 distinct (case-folded) supporting repos.
pub fn arb_person_candidate() -> impl Strategy<Value = PersonCandidate> {
    (
        "[A-Za-z0-9][A-Za-z0-9-]{0,11}",
        "org\\.openlore\\.philosophy\\.[a-z-]{1,16}",
        proptest::collection::vec(arb_supporting_repo(), 1..5),
    )
        .prop_map(|(login, philosophy, repos)| {
            let mut seen = std::collections::BTreeSet::new();
            let distinct: Vec<SupportingRepo> = repos
                .into_iter()
                .filter(|r| seen.insert(r.repo_subject.to_ascii_lowercase()))
                .collect();
            PersonCandidate::new(&format!("github:{login}"), &philosophy, distinct)
                .expect("non-empty provenance")
        })
}

const REPO_POOL: [&str; 4] = [
    "github:BurntSushi/ripgrep",
    "github:burntsushi/RIPGREP",
    "github:rust-lang/regex",
    "github:dtolnay/serde",
];
pub const PERSON_POOL: [&str; 4] = [
    "github:BurntSushi",
    "github:burntsushi",
    "github:dtolnay",
    "github:alice",
];
const PHILOSOPHY_POOL: [&str; 2] = [
    "org.openlore.philosophy.memory-safety",
    "org.openlore.philosophy.test-driven",
];

fn arb_relationship() -> impl Strategy<Value = AuthorRelationship> {
    prop_oneof![
        Just(AuthorRelationship::You),
        Just(AuthorRelationship::SubscribedPeer),
        Just(AuthorRelationship::UnsubscribedCache),
        Just(AuthorRelationship::NetworkUnfollowed),
    ]
}

/// A recorded contribution link drawn from small, overlapping pools (so links
/// and claims actually join, including case-variant subjects).
pub fn arb_contribution_link() -> impl Strategy<Value = ContributionLink> {
    (
        proptest::sample::select(&REPO_POOL[..]),
        proptest::sample::select(&PERSON_POOL[..]),
        1u32..=30,
    )
        .prop_map(|(repo, person, rank)| ContributionLink {
            repo_subject: repo.to_string(),
            person_subject: person.to_string(),
            github_user_id: u64::from(rank),
            rank,
            contributions: u64::from(100 - rank),
            first_observed_at: Default::default(),
            last_observed_at: Default::default(),
        })
}

/// A signed repo claim over the same pools, across every relationship and
/// both an eligible and an ineligible predicate.
pub fn arb_repo_claim() -> impl Strategy<Value = RepoClaim> {
    (
        proptest::sample::select(&REPO_POOL[..]),
        prop_oneof![Just(EMBODIES_PHILOSOPHY), Just("usesLanguage")],
        proptest::sample::select(&PHILOSOPHY_POOL[..]),
        "did:plc:[a-z]{2,4}",
        arb_relationship(),
        "bafy[a-z2-7]{6,12}",
        arb_hundredths(),
    )
        .prop_map(
            |(repo, predicate, philosophy, author_did, relationship, cid, confidence)| RepoClaim {
                repo_subject: repo.to_string(),
                predicate: predicate.to_string(),
                philosophy: philosophy.to_string(),
                author_did,
                relationship,
                cid,
                confidence,
            },
        )
}

/// Links + repo claims for one inference run.
pub fn arb_inference_inputs() -> impl Strategy<Value = (Vec<ContributionLink>, Vec<RepoClaim>)> {
    (
        proptest::collection::vec(arb_contribution_link(), 0..8),
        proptest::collection::vec(arb_repo_claim(), 0..10),
    )
}
