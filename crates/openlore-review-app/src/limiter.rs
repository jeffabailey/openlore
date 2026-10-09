//! The in-memory limiter state (ADR-076 §2): the root holds it, the pure
//! `review_domain::budget` core decides. Resets on restart by design.

use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use review_domain::budget::{admit_scan, release_scan, ScanAdmission, ScanBudget};

/// Unix seconds now (the shell's clock).
pub(crate) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

/// Unix seconds now, signed as the pure core's clocks are.
pub(crate) fn unix_now_secs() -> i64 {
    i64::try_from(unix_now()).unwrap_or_default()
}

/// Who is scanning and who scanned recently, and how many scans may run at
/// once.
pub(crate) struct ScanLimiter {
    budget: Mutex<ScanBudget>,
    max_running: usize,
}

impl ScanLimiter {
    /// A limiter admitting at most `max_running` concurrent scans.
    pub(crate) fn new(max_running: usize) -> Self {
        Self {
            budget: Mutex::new(ScanBudget::default()),
            max_running,
        }
    }

    /// Ask the budget whether `owner_did` may start a scan now; an admitted
    /// scan holds its slot until [`ScanLimiter::release`].
    pub(crate) fn admit(&self, owner_did: &str) -> ScanAdmission {
        let Ok(mut budget) = self.budget.lock() else {
            return ScanAdmission::AppBusy;
        };
        let (next, admission) = admit_scan(
            std::mem::take(&mut *budget),
            owner_did,
            unix_now(),
            self.max_running,
        );
        *budget = next;
        admission
    }

    /// The scan by `owner_did` ended.
    pub(crate) fn release(&self, owner_did: &str) {
        if let Ok(mut budget) = self.budget.lock() {
            *budget = release_scan(std::mem::take(&mut *budget), owner_did);
        }
    }
}
