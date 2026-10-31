//! `runner_contract` — what whoever runs a pass hands it (D-STZ-42): the pass's
//! label and the lent purge capability. It is apart from `pass_runner`, so a
//! root that has no single-flight runner (Lambda) can run the same pass.
//!
//! Every pass carries a [`PassLabel`]: a boot nonce (the Unix time in
//! milliseconds at which its runner started) and the pass's sequence number,
//! written `<boot>-<seq>`. The sequence alone repeats after a restart; the boot
//! nonce keeps a label unique across restarts, so "exactly one `pass_summary`
//! per `pass_id`" stays checkable over a whole log stream.
//!
//! This is the ONLY module that may purge (ADR-082, check-arch
//! `index_purge_only_in_pass_runner`): a composition root hands the purge handle
//! to whatever runs its passes and keeps no copy; that runner lends each pass a
//! [`RunnerPurge`] for the pass's duration. Its one operation plans the purge
//! from the loaded list (pure `plan_purge`) and purges each removed author
//! through `IndexPurgePort`, at pass start, before any fetch.

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use appview_domain::pass_runner::PassId;
use appview_domain::{plan_purge, BareDid, PurgePlan, PurgeSuppressed};
use claim_domain::Did;
use ports::{IndexPurgePort, IndexStoreError, ProbeOutcome, PurgeReport};

/// The process-unique label of one pass: `<boot nonce>-<sequence>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PassLabel {
    boot: BootNonce,
    pass_id: PassId,
}

impl PassLabel {
    /// The label of pass `pass_id` of the runner that started at `boot`.
    #[must_use]
    pub const fn new(boot: BootNonce, pass_id: PassId) -> Self {
        Self { boot, pass_id }
    }

    /// The pass's sequence number within its runner (1 = its first pass).
    #[must_use]
    pub fn pass_id(&self) -> PassId {
        self.pass_id
    }
}

impl fmt::Display for PassLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.boot.0, self.pass_id.0)
    }
}

/// When a runner started (Unix milliseconds); distinguishes its passes from
/// those of an earlier run of the same process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootNonce(u128);

impl BootNonce {
    /// The nonce of a runner starting now.
    #[must_use]
    pub fn now() -> Self {
        Self(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.as_millis()),
        )
    }
}

/// The purge handle a composition root gives whatever runs its passes.
pub type SharedPurge = Arc<dyn IndexPurgePort + Send + Sync>;

/// The purge capability as lent to one pass. Building one takes the port
/// itself, which only a composition root may hold.
#[derive(Clone, Copy)]
pub struct RunnerPurge<'runner>(&'runner (dyn IndexPurgePort + Send + Sync));

impl<'runner> RunnerPurge<'runner> {
    /// Lend `purge` for one pass.
    #[must_use]
    pub fn lent(purge: &'runner (dyn IndexPurgePort + Send + Sync)) -> Self {
        Self(purge)
    }
}

impl RunnerPurge<'_> {
    /// Purge every indexed author the loaded, non-empty `listed` no longer
    /// names (ADR-082).
    #[must_use]
    pub fn purge_unlisted(self, listed: &[Did]) -> PurgeStep {
        purge_unlisted_authors(self.0, listed)
    }
}

/// One author a pass purged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PurgedAuthor {
    pub did: BareDid,
    pub claims_removed: u64,
}

/// What a pass's purge step did (ADR-082).
#[derive(Debug)]
pub enum PurgeStep {
    /// The list was empty: nobody was purged.
    Suppressed(PurgeSuppressed),
    /// Every removed author was purged (possibly none).
    Purged(Vec<PurgedAuthor>),
    /// The store failed; the authors before the failure stay purged and the
    /// next pass's plan finishes the rest.
    Failed {
        purged: Vec<PurgedAuthor>,
        error: IndexStoreError,
    },
}

/// Purge every indexed author the loaded, non-empty `listed` no longer
/// names. A listed author is never planned, whatever their fetch outcome.
fn purge_unlisted_authors(purge: &dyn IndexPurgePort, listed: &[Did]) -> PurgeStep {
    let indexed = match purge.indexed_authors() {
        Ok(indexed) => indexed,
        Err(error) => {
            return PurgeStep::Failed {
                purged: Vec::new(),
                error,
            }
        }
    };
    match plan_purge(listed, indexed.iter().map(String::as_str)) {
        PurgePlan::Suppressed(reason) => PurgeStep::Suppressed(reason),
        PurgePlan::Purge(removed) => purge_each(purge, removed),
    }
}

/// Purge `removed` one author at a time, stopping at the first store failure.
fn purge_each(purge: &dyn IndexPurgePort, removed: impl IntoIterator<Item = BareDid>) -> PurgeStep {
    let mut purged = Vec::new();
    for did in removed {
        match purge.purge_author(&did.0) {
            Ok(report) => purged.push(PurgedAuthor {
                did,
                claims_removed: report.claims_removed,
            }),
            Err(error) => return PurgeStep::Failed { purged, error },
        }
    }
    PurgeStep::Purged(purged)
}

/// TEST-FAULT seam (`OPENLORE_INDEXER_TEST_FAULT=purge_fails`, debug builds
/// only): `purge`, except that its first author purge fails. The purge is
/// resumable, so the next pass finishes it.
pub fn failing_first_purge(purge: SharedPurge) -> SharedPurge {
    Arc::new(FailingFirstPurge {
        purge,
        failed_once: AtomicBool::new(false),
    })
}

struct FailingFirstPurge {
    purge: SharedPurge,
    failed_once: AtomicBool,
}

impl IndexPurgePort for FailingFirstPurge {
    fn probe(&self) -> ProbeOutcome {
        self.purge.probe()
    }

    fn indexed_authors(&self) -> Result<std::collections::BTreeSet<String>, IndexStoreError> {
        self.purge.indexed_authors()
    }

    fn purge_author(&self, bare_did: &str) -> Result<PurgeReport, IndexStoreError> {
        if self.failed_once.swap(true, Ordering::SeqCst) {
            self.purge.purge_author(bare_did)
        } else {
            Err(IndexStoreError::QueryFailed {
                message: "test fault: the purge fails".to_string(),
            })
        }
    }
}
