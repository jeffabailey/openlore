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
// Production code is unsafe-free; the test module needs `unsafe` to
// call `std::env::{set_var, remove_var}` (Rust 1.91 marked these
// `unsafe fn` because mutating process env is unsound under
// concurrent reads). Scope `forbid` to non-test builds so the test
// module can opt-in.
#![cfg_attr(not(test), forbid(unsafe_code))]

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

    /// Return the current UTC instant.
    ///
    /// **Test-only seam (step 05-07):** if the `OPENLORE_TEST_NOW`
    /// environment variable is set AND parses as RFC3339, the pinned
    /// timestamp is returned instead of the real system clock. This
    /// keeps CID determinism testable across subprocess boundaries
    /// (WS-7) without introducing a separate `FakeClock` adapter.
    ///
    /// Production behavior is unchanged: in any process where
    /// `OPENLORE_TEST_NOW` is unset OR holds an unparseable value, the
    /// adapter falls through to `chrono::Utc::now()`. The seam is
    /// failure-tolerant by design — a malformed pin must never lock a
    /// real user out of writing claims.
    fn now_utc(&self) -> DateTime<Utc> {
        if let Ok(raw) = std::env::var("OPENLORE_TEST_NOW") {
            if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(&raw) {
                return parsed.with_timezone(&Utc);
            }
        }
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
    ///
    /// Guarded with a `remove_var` of `OPENLORE_TEST_NOW` so this test
    /// stays green regardless of whether a parent process has set the
    /// pin (step 05-07 added the env-var seam; pre-existing tests must
    /// keep verifying the unpinned-clock contract).
    #[test]
    fn now_utc_returns_time_within_one_minute_of_reference() {
        // SAFETY: tests in this module are not run in parallel against
        // OPENLORE_TEST_NOW because the pinned-time test below also
        // mutates it and we serialize via Rust's per-test process env
        // (these tests run in the same binary). We accept that nuance:
        // both tests set/unset the var deterministically before
        // observing the clock, and there is no third concurrent reader.
        unsafe {
            std::env::remove_var("OPENLORE_TEST_NOW");
        }
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

    /// Step 05-07: `now_utc()` honors `OPENLORE_TEST_NOW` as a
    /// test-only seam. When the env var holds a parseable RFC3339
    /// timestamp, the adapter returns that fixed value byte-for-byte.
    /// This unblocks WS-7 (CID determinism across subprocess runs)
    /// without introducing a separate FakeClock adapter.
    #[test]
    fn now_utc_honors_openlore_test_now_env_var() {
        // SAFETY: see sibling test above. We set, observe, then unset
        // to leave the env clean for any subsequent test.
        let pinned = "2026-05-26T12:00:00Z";
        unsafe {
            std::env::set_var("OPENLORE_TEST_NOW", pinned);
        }

        let adapter = SystemClockAdapter::new();
        let observed = adapter.now_utc();

        unsafe {
            std::env::remove_var("OPENLORE_TEST_NOW");
        }

        let expected: DateTime<Utc> = chrono::DateTime::parse_from_rfc3339(pinned)
            .expect("test pin parses as RFC3339")
            .with_timezone(&Utc);
        assert_eq!(
            observed, expected,
            "OPENLORE_TEST_NOW={pinned} must pin now_utc() to that instant"
        );
    }
}
