//! Proptest strategies for `UnsignedClaim` and its component types.
//!
//! Step 02-04: bootstraps the one `@property`-tagged scenario in
//! slice-01 — LC-3 (CID byte-stability across N re-canonicalizations).
//! Per DD-12, proptest is the canonical Rust PBT crate (nw-distill
//! polyglot matrix); the strategy below generates valid
//! `UnsignedClaim` values along EVERY field, biased toward the wire
//! shapes the Lexicon will accept in production.
//!
//! ## Functional discipline
//!
//! Pure. No I/O. No mutation. Each generator returns a fresh
//! immutable value. The strategies compose via `prop_map` and `Just`
//! — small, named, single-purpose builders, NEVER a 200-line nested
//! `(_,_,_,_,_,_,_,_)` tuple.
//!
//! ## Why diverse generators matter
//!
//! ADR-006 §Earned Trust pins CID byte-stability as a load-bearing
//! invariant: if `canonicalize(c)` ever produces a different byte
//! sequence on a second call, every federation peer sees the SAME
//! claim under a DIFFERENT CID and the round-trip identity contract
//! (KPI-4) collapses. Examples like `sample_claim()` in
//! `canonicalize.rs::tests` lock determinism for ONE shape; the
//! property below explores the input space and would catch a
//! regression where, say, `evidence: vec![]` and `evidence: vec![x]`
//! take different code paths that subtly disagree.

use proptest::collection::vec;
use proptest::prelude::*;
use unicode_normalization::UnicodeNormalization;

use crate::{Cid, ClaimReference, Confidence, Did, ReferenceType, UnsignedClaim};

// -----------------------------------------------------------------------------
// Component strategies — small, named, composable
// -----------------------------------------------------------------------------

/// A URI-shaped string. Slice-01 subjects are `github:org/repo`,
/// `mastodon:@user@inst`, etc. — short, ASCII, no spaces or control
/// chars. Length 4..=64 keeps the suite fast while covering boundary
/// shapes (single-segment, multi-segment, with-fragment).
fn arb_uri_shaped_string() -> impl Strategy<Value = String> {
    // ASCII alphanumerics + safe URI chars. Hypothesis-style class.
    "[A-Za-z0-9_./:@#-]{4,64}".prop_map(|s| s.to_string())
}

/// A URL-shaped string for the `evidence` field. `https://` prefix
/// matches the wire shape in US-001 examples; the path body is
/// free-form ASCII.
fn arb_url() -> impl Strategy<Value = String> {
    "https://[a-z0-9.-]{4,32}/[A-Za-z0-9_./-]{0,32}".prop_map(|s| s.to_string())
}

/// `[0.0, 1.0]` inclusive, NaN-free. Confidence is the one wire field
/// with documented numeric bounds (Lexicon `minimum`/`maximum`); the
/// canonical-CBOR float encoding requires deterministic bit patterns,
/// so NaN MUST be excluded (two NaN payloads have different bits and
/// would break the property for the wrong reason).
fn arb_confidence_value() -> impl Strategy<Value = f64> {
    // `0.0..=1.0_f64` excludes NaN and ±inf by construction. We do
    // NOT route through `Confidence::try_new` (still a RED-scaffold
    // panic at step 02-04 — the wrapper's smart constructor lands in
    // a later step). Building the wrapper directly is legitimate
    // here: the crate owns the invariant being asserted.
    0.0_f64..=1.0_f64
}

/// A DID with the OpenLore application fragment, matching the wire
/// shape used across slice-01 fixtures (`did:plc:test-jeff#…`).
fn arb_author_did() -> impl Strategy<Value = Did> {
    "did:plc:[a-z0-9-]{4,24}#org.openlore.application".prop_map(Did)
}

/// An RFC3339 UTC timestamp. We generate from a UNIX-second range
/// (year 2024 → year 2030) and format deterministically, instead of
/// regex-generating which would frequently produce invalid dates.
fn arb_composed_at_rfc3339() -> impl Strategy<Value = String> {
    // 2024-01-01T00:00:00Z .. 2030-12-31T23:59:59Z, roughly.
    (1_704_067_200_i64..=1_924_991_999_i64).prop_map(|epoch_secs| {
        chrono::DateTime::<chrono::Utc>::from_timestamp(epoch_secs, 0)
            .expect("epoch within chrono's representable range")
            .format("%Y-%m-%dT%H:%M:%SZ")
            .to_string()
    })
}

/// A plausible CID string. Real CIDs are `bafyrei…` (CIDv1
/// base32-lower); for the property test we only need byte-stability,
/// not CID validity — the reference target is just a free-form string
/// in the canonical CBOR output. We generate `bafy…` to keep test
/// counter-examples readable.
fn arb_cid() -> impl Strategy<Value = Cid> {
    "bafy[a-z0-9]{52}".prop_map(Cid)
}

/// One typed reference. Slice-01 supports four types; pick one
/// uniformly so the strategy exercises each variant equally.
fn arb_reference_type() -> impl Strategy<Value = ReferenceType> {
    prop_oneof![
        Just(ReferenceType::Retracts),
        Just(ReferenceType::Corrects),
        Just(ReferenceType::Counters),
        Just(ReferenceType::Supersedes),
    ]
}

/// One `ClaimReference` value.
fn arb_claim_reference() -> impl Strategy<Value = ClaimReference> {
    (arb_reference_type(), arb_cid()).prop_map(|(ref_type, cid)| ClaimReference { ref_type, cid })
}

// -----------------------------------------------------------------------------
// Top-level strategy — composes the components
// -----------------------------------------------------------------------------

// -----------------------------------------------------------------------------
// `reason` text strategies — step 02-03 (WD-35 NFC normalization properties)
// -----------------------------------------------------------------------------

/// A domain-realistic `reason` text. Counter-claim reasons are
/// free-form human prose: ASCII, accented Latin, CJK, and — crucially
/// for the NFC properties — text that MAY arrive with combining marks
/// (the way some editors / IMEs emit accented letters). The character
/// class deliberately includes both precomposed accented letters
/// (`é` U+00E9, `ñ` U+00F1) AND a bare combining acute (U+0301) so the
/// generator naturally produces strings in mixed normalization forms,
/// exercising the idempotency path on already-NFC and not-yet-NFC input
/// alike. Length 0..=80 keeps the suite fast (LCC-3 needs ≥100 cases).
pub fn arb_reason_text() -> impl Strategy<Value = String> {
    // ASCII printable + a handful of precomposed Latin letters + a CJK
    // sample + a standalone combining acute. proptest's regex strategy
    // supports unicode escapes in the class.
    "[ -~éñüçJosé漢字\u{0301}]{0,80}".prop_map(|s| s.to_string())
}

/// A pair `(r, s)` that is byte-DISTINCT but canonically equivalent
/// (`NFC(r) == NFC(s)`), for the LCC-4 NFC-unification property.
///
/// Construction: generate a base text that is guaranteed to contain at
/// least one decomposable character, take its precomposed (NFC) form as
/// `r` and its decomposed (NFD) form as `s`. For every character with a
/// canonical decomposition, NFC and NFD differ byte-wise, so the pair is
/// distinct; NFC(NFD(x)) == NFC(x) by definition, so they are
/// canonically equivalent. We prepend a mandatory decomposable letter so
/// the pair is NEVER vacuously equal (a pure-ASCII base has identical
/// NFC and NFD forms).
pub fn arb_nfc_equivalent_pair() -> impl Strategy<Value = (String, String)> {
    // A non-empty pool of precomposed letters that each have a canonical
    // decomposition into base + COMBINING mark, so NFC != NFD byte-wise.
    // Letters whose glyph uses a STROKE/overlay (e.g. 'ø' U+00F8) have NO
    // canonical decomposition and would make the pair vacuously equal —
    // they are deliberately excluded, and the `prop_filter` below is a
    // belt-and-suspenders guard against any future pool regression.
    let decomposable = prop_oneof![
        Just('é'), // U+00E9 -> e + ◌́ (U+0301)
        Just('ñ'), // U+00F1 -> n + ◌̃ (U+0303)
        Just('ü'), // U+00FC -> u + ◌̈ (U+0308)
        Just('ç'), // U+00E7 -> c + ◌̧ (U+0327)
        Just('å'), // U+00E5 -> a + ◌̊ (U+030A)
        Just('ô'), // U+00F4 -> o + ◌̂ (U+0302)
    ];
    // An optional free-form prefix/suffix of ordinary text so the
    // property isn't only exercised on single-character inputs.
    let affix = "[ -~]{0,16}".prop_map(|s: String| s);
    (affix.clone(), decomposable, affix)
        .prop_map(|(prefix, letter, suffix)| {
            let base: String = format!("{prefix}{letter}{suffix}");
            let precomposed: String = base.nfc().collect();
            let decomposed: String = base.nfd().collect();
            (precomposed, decomposed)
        })
        // Guarantee the contract the property relies on: the pair is
        // byte-DISTINCT (otherwise NFC-unification would be vacuous).
        .prop_filter("pair must be byte-distinct", |(r, s)| r != s)
}

/// Strategy generating an arbitrary VALID `UnsignedClaim`.
///
/// "Valid" here means: every field is well-formed per the Lexicon
/// (URI-shaped subject/predicate/object, URL-shaped evidence,
/// in-range NaN-free confidence, DID-shaped author, RFC3339 timestamp,
/// typed references). Bound sizes (evidence 0..=5, references 0..=3)
/// stay small so 256 iterations finish in well under a second on CI.
///
/// Used by `tests/acceptance/lexicon_conformance.rs::lexicon_cid_is_byte_stable_across_n_re_canonicalizations`
/// (LC-3) and any future property test of the canonicalization pipeline.
pub fn arb_unsigned_claim() -> impl Strategy<Value = UnsignedClaim> {
    (
        arb_uri_shaped_string(),
        arb_uri_shaped_string(),
        arb_uri_shaped_string(),
        vec(arb_url(), 0..=5),
        arb_confidence_value(),
        arb_author_did(),
        arb_composed_at_rfc3339(),
        vec(arb_claim_reference(), 0..=3),
    )
        .prop_map(
            |(subject, predicate, object, evidence, confidence_value, author_did, composed_at, references)| {
                UnsignedClaim {
                    subject,
                    predicate,
                    object,
                    evidence,
                    // Direct construction — see arb_confidence_value docs.
                    confidence: Confidence(confidence_value),
                    author_did,
                    composed_at,
                    references,
                    // Slice-01 strategy generates non-counter claims; the
                    // counter-claim `reason` is exercised by
                    // `validate_counter_claim`'s own tests. `None` keeps the
                    // generated claim byte-stable with the slice-01 wire
                    // shape (serde skips `None`), preserving LC-3's CID
                    // byte-stability property.
                    reason: None,
                }
            },
        )
}
