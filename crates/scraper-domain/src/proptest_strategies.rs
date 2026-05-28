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
