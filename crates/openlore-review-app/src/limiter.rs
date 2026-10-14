//! The in-memory limiter state (ADR-076 §2): the root holds it, the pure
//! `review_domain::budget` core decides. Resets on restart by design.

use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use std::collections::HashMap;

use review_domain::budget::{admit_scan, release_scan, spend_share, ScanAdmission, ScanBudget};

/// How far ahead the clock runs: set once at startup from the config, which
/// only a development build lets be non-zero (the expiry test seam).
static CLOCK_SKEW_SECS: OnceLock<u64> = OnceLock::new();

/// Run the clock `secs` ahead for the life of the process (first call wins).
pub(crate) fn set_clock_skew(secs: u64) {
    let _ = CLOCK_SKEW_SECS.set(secs);
}

/// Unix seconds now (the shell's clock, plus any test-seam skew).
pub(crate) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
        .saturating_add(CLOCK_SKEW_SECS.get().copied().unwrap_or(0))
}

/// Unix seconds now, signed as the pure core's clocks are.
pub(crate) fn unix_now_secs() -> i64 {
    i64::try_from(unix_now()).unwrap_or_default()
}

/// When each DID posted its recent shares (ADR-076 §2: 5 a day). In memory,
/// so it resets on restart, like the scan budget.
#[derive(Default)]
pub(crate) struct ShareAllowance(Mutex<HashMap<String, Vec<u64>>>);

impl ShareAllowance {
    /// May `owner_did` post a share now? (Nothing is spent.)
    pub(crate) fn allows(&self, owner_did: &str) -> bool {
        let Ok(posted) = self.0.lock() else {
            return false;
        };
        let recent = posted.get(owner_did).map_or(&[][..], Vec::as_slice);
        spend_share(recent, unix_now()).is_some()
    }

    /// Spend one share for `owner_did`; the time it was spent at, for a
    /// [`ShareAllowance::refund`] when the post does not land, or `None`
    /// when the day's shares are used up.
    pub(crate) fn try_spend(&self, owner_did: &str) -> Option<u64> {
        let mut posted = self.0.lock().ok()?;
        let now = unix_now();
        let recent = posted.get(owner_did).map_or(&[][..], Vec::as_slice);
        let spent = spend_share(recent, now)?;
        posted.insert(owner_did.to_string(), spent);
        Some(now)
    }

    /// Give back the share spent at `at`: its post did not land.
    pub(crate) fn refund(&self, owner_did: &str, at: u64) {
        if let Ok(mut posted) = self.0.lock() {
            if let Some(recent) = posted.get_mut(owner_did) {
                if let Some(index) = recent.iter().rposition(|t| *t == at) {
                    recent.remove(index);
                }
            }
        }
    }
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
