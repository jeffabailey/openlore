//! `adapter-system-clock` — `ClockPort` over `chrono::Utc::now()`.
//!
//! This is the **simplest adapter** in the openlore-foundation roster,
//! intentionally degenerate per ADR-009: `probe()` returns `Ok`
//! unconditionally because `chrono::Utc::now()` has no failure modes a
//! probe could meaningfully gate on (no I/O, no schema, no key material,
//! no network). Carrying the same `ClockPort` trait shape as the harder
//! adapters lets the composition root walk the probe gauntlet
//! uniformly — every port has at least one named adapter, every adapter
//! answers the same Earned-Trust contract, no special-casing.
//!
//! Shipping this first proves the port shape end-to-end before the
//! harder adapters (DuckDB storage, AT-Proto PDS, AT-Proto DID) land.

#![allow(dead_code)]
#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use ports::{ClockPort, ProbeOutcome};

pub struct SystemClockAdapter;

impl SystemClockAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SystemClockAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClockPort for SystemClockAdapter {
    fn probe(&self) -> ProbeOutcome {
        ProbeOutcome::Ok
    }

    fn now_utc(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

// -----------------------------------------------------------------------------
// Unit tests — port-to-port at the `ClockPort` boundary.
//
// The driving port IS the trait surface (`probe`, `now_utc`); these tests
// invoke the adapter through that trait and assert on the observable
// return values. No internal field inspection, no implementation coupling.
// -----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    /// Property: `probe()` returns `ProbeOutcome::Ok` unconditionally.
    ///
    /// The degenerate-adapter invariant (ADR-009): the system clock has
    /// no failure modes a probe could meaningfully gate on, so it
    /// always self-attests as ready.
    #[test]
    fn probe_returns_ok_unconditionally() {
        let adapter = SystemClockAdapter::new();

        let outcome = adapter.probe();

        assert!(
            matches!(outcome, ProbeOutcome::Ok),
            "SystemClockAdapter::probe() must return ProbeOutcome::Ok per ADR-009"
        );
    }

    /// Property: `now_utc()` returns a `DateTime<Utc>` within the last
    /// minute of the reference `chrono::Utc::now()` taken at assertion
    /// time. This pins the contract that the adapter delegates to the
    /// real system clock (not a frozen / mocked time source) without
    /// coupling to exact instant equality (which would flake on any
    /// scheduling jitter).
    #[test]
    fn now_utc_returns_time_within_one_minute_of_reference() {
        let adapter = SystemClockAdapter::new();

        let before = Utc::now();
        let observed = adapter.now_utc();
        let after = Utc::now();

        // Observed must fall within [before, after] — i.e. the adapter
        // sampled the real clock between our two reference samples.
        // Allow a one-minute slack window to absorb clock-skew edge
        // cases (NTP step, suspend/resume) without flaking.
        let slack = Duration::minutes(1);
        assert!(
            observed >= before - slack && observed <= after + slack,
            "now_utc() = {observed} must fall within [{before}, {after}] (+/- 1 min slack)",
        );
    }
}
