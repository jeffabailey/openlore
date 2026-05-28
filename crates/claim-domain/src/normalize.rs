//! NFC normalization of free-text `reason` payloads (WD-35 / ADR-015).
//!
//! Step 02-03. A counter-claim's `--reason` text is NFC-normalized at
//! compose time so the normalized bytes are what get signed AND
//! displayed in the preview. NFC normalization is mandatory for CID
//! determinism (ADR-006): two visually-identical strings with different
//! Unicode normalization forms would otherwise produce different
//! canonical CBOR and therefore different CIDs, silently breaking
//! copy-paste workflows.
//!
//! ## Functional discipline
//!
//! `normalize_reason` is a PURE, total transformation: `&str -> String`.
//! No I/O, no mutation of shared state, no panics. It is a thin,
//! well-named wrapper over the `unicode-normalization` crate's NFC pass
//! (WD-35: a PURE dependency — `cargo xtask check-arch` permits it in
//! the pure core). Keeping the wrapper named after the DOMAIN concept
//! (`normalize_reason`, not `nfc`) documents WHY normalization happens
//! here, per nw-fp-usable-design.

use unicode_normalization::UnicodeNormalization;

/// NFC-normalize a `reason` text.
///
/// Returns the input in Unicode Normalization Form C (canonical
/// composition). The transformation is:
///
/// - **pure** — same input always yields the same output, no effects;
/// - **idempotent** — `normalize_reason(normalize_reason(r)) ==
///   normalize_reason(r)` (NFC is by definition a fixed point of itself);
/// - **NFC-unifying** — any two byte-distinct strings with the same NFC
///   form map to the SAME output (e.g. precomposed "é" U+00E9 and
///   decomposed "e\u{0301}" both become the precomposed form).
///
/// These three properties are the load-bearing invariants asserted by
/// LCC-3 + LCC-4 (data-models.md properties 2 + 3).
pub fn normalize_reason(reason: &str) -> String {
    reason.nfc().collect()
}

#[cfg(test)]
mod tests {
    use super::normalize_reason;

    // The idempotence + NFC-unification INVARIANTS are proven as
    // layer-2 @property tests (LCC-3 + LCC-4 in
    // tests/acceptance/lexicon_counter_claim.rs) over the proptest
    // generators in `proptest_strategies`. Per nw-tdd-methodology
    // ("No Code Without a Requiring Test" + behavior budget), we do NOT
    // re-assert those properties at the unit layer — that would be Test
    // Duplication. This single example instead pins the ONE concrete,
    // load-bearing copy-paste scenario WD-35 names explicitly, which
    // documents the contract independently of the random generators.

    /// Known load-bearing example: the precomposed "café" (ending in
    /// U+00E9) and the decomposed "café" ("cafe" + combining acute
    /// U+0301) are byte-DISTINCT but canonically equivalent — both must
    /// normalize to the precomposed NFC form. This is the exact
    /// copy-paste scenario WD-35 protects against.
    #[test]
    fn precomposed_and_decomposed_accent_normalize_identically() {
        let precomposed = "caf\u{00E9}"; // café  (1 char é)
        let decomposed = "cafe\u{0301}"; // café  (e + ´)
        assert_ne!(
            precomposed, decomposed,
            "precondition: the two spellings must be byte-distinct"
        );
        assert_eq!(
            normalize_reason(precomposed),
            normalize_reason(decomposed),
            "byte-distinct NFC-equivalent strings must unify under normalize_reason"
        );
        assert_eq!(
            normalize_reason(decomposed),
            precomposed,
            "NFC composes combining marks: the decomposed form must yield the precomposed bytes"
        );
    }
}
