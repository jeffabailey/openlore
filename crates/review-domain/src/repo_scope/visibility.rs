//! Which pending suggestions the owner sees (CORE-RRA-7, OQ-5). PURE.

use super::limits::Standing;
use super::PlanSource;

/// Visibility (CORE-RRA-7, OQ-5): each suggestion from `sources` is shown
/// iff it comes from a listed account that is verified, or from a repo on
/// the `imports` list. Derived, never stored: hidden is not deleted.
pub fn visible(sources: &[PlanSource], accounts: &[(u64, Standing)], imports: &[u64]) -> Vec<bool> {
    sources
        .iter()
        .map(|source| shown(*source, accounts, imports))
        .collect()
}

fn shown(source: PlanSource, accounts: &[(u64, Standing)], imports: &[u64]) -> bool {
    match source {
        PlanSource::Account(account_id) => accounts
            .iter()
            .any(|(id, standing)| *id == account_id && *standing == Standing::Verified),
        PlanSource::Imported(repo_id) => imports.contains(&repo_id),
    }
}
