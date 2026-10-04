//! `PublishPlanPort`: the owner's publish plans between preview and confirm
//! (`plans`, ADR-074). Every statement is scoped by `owner_did`; `take`
//! deletes the plan in the same statement that returns it, so a plan is
//! executed at most once (a replayed confirm finds nothing).

use duckdb::params;
use ports::{
    PublishPlanPort, ReviewStoreError, StoredPublishPlan, SuggestionKey, TakenPublishPlan,
};

use crate::secrets::port_error;
use crate::{db_error, ReviewStore};

const PUT_PLAN: &str = "INSERT OR REPLACE INTO plans
    (owner_did, plan_id, kind, suggestion_key, plan, created_at, expires_at)
    VALUES (?, ?, 'publish', ?, ?, now(), to_timestamp(?))";

fn key_json(key: &SuggestionKey) -> String {
    serde_json::json!([key.subject, key.predicate, key.object]).to_string()
}

fn key_of_json(stored: &str) -> Option<SuggestionKey> {
    let parts: Vec<String> = serde_json::from_str(stored).ok()?;
    match parts.as_slice() {
        [subject, predicate, object] => Some(SuggestionKey {
            subject: subject.clone(),
            predicate: predicate.clone(),
            object: object.clone(),
        }),
        _ => None,
    }
}

impl PublishPlanPort for ReviewStore {
    fn put_publish_plan(
        &self,
        owner_did: &str,
        plan: &StoredPublishPlan,
        expires_at: i64,
    ) -> Result<(), ReviewStoreError> {
        self.with_connection(|conn| {
            conn.execute(
                PUT_PLAN,
                params![
                    owner_did,
                    plan.plan_id,
                    key_json(&plan.key),
                    plan.record_json,
                    expires_at
                ],
            )
            .map(drop)
            .map_err(db_error)
        })
        .map_err(port_error)
    }

    fn take_publish_plan(
        &self,
        owner_did: &str,
        plan_id: &str,
    ) -> Result<Option<TakenPublishPlan>, ReviewStoreError> {
        self.with_connection(|conn| {
            let taken = crate::expiry::take_publish_plan(conn, owner_did, plan_id)?;
            Ok(taken.and_then(|(key, record_json, expires_at)| {
                Some(TakenPublishPlan {
                    plan: StoredPublishPlan {
                        plan_id: plan_id.to_string(),
                        key: key_of_json(&key?)?,
                        record_json,
                    },
                    // An unreadable deadline is treated as already passed.
                    expires_at: expires_at.unwrap_or(i64::MIN),
                })
            }))
        })
        .map_err(port_error)
    }
}

#[cfg(test)]
mod tests {
    //! Universe: the `plans` rows of two owners. Taking a plan removes
    //! exactly that owner's plan, returns it with its deadline, and leaves
    //! the other owner's plan untouched; concurrent takes yield one plan.
    use super::*;
    use crate::DataKey;
    use proptest::prelude::*;
    use std::sync::Arc;

    fn plan(id: &str) -> StoredPublishPlan {
        StoredPublishPlan {
            plan_id: id.to_string(),
            key: SuggestionKey {
                subject: "github:a/b".into(),
                predicate: "embodiesPhilosophy".into(),
                object: "org.openlore.philosophy.test-driven".into(),
            },
            record_json: format!("{{\"plan\":\"{id}\"}}"),
        }
    }

    fn taken(id: &str, expires_at: i64) -> TakenPublishPlan {
        TakenPublishPlan {
            plan: plan(id),
            expires_at,
        }
    }

    fn store() -> ReviewStore {
        ReviewStore::open_in_memory(DataKey::from_bytes([3u8; 32])).expect("store")
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(16))]
        #[test]
        fn a_plan_is_taken_once_and_only_by_its_owner(
            id in "baf[a-z2-7]{10}",
            expires_at in 1_000_000_000i64..4_000_000_000,
        ) {
            let store = store();
            let (priya, maria) = ("did:plc:priya", "did:plc:maria");
            store.put_publish_plan(priya, &plan(&id), expires_at).expect("put");
            store.put_publish_plan(maria, &plan(&id), expires_at + 1).expect("put");
            prop_assert_eq!(store.take_publish_plan(priya, "other").expect("take"), None);
            prop_assert_eq!(store.take_publish_plan(priya, &id).expect("take"), Some(taken(&id, expires_at)));
            prop_assert_eq!(store.take_publish_plan(priya, &id).expect("take"), None);
            prop_assert_eq!(store.take_publish_plan(maria, &id).expect("take"), Some(taken(&id, expires_at + 1)));
        }

        /// Any number of confirms of one (owner, plan) racing each other:
        /// exactly one takes the plan, so at most one write can follow.
        #[test]
        fn concurrent_confirms_of_one_plan_take_it_exactly_once(
            id in "baf[a-z2-7]{10}",
            confirms in 2usize..8,
        ) {
            let store = Arc::new(store());
            store.put_publish_plan("did:plc:priya", &plan(&id), 2_000_000_000).expect("put");
            let takers: Vec<_> = (0..confirms)
                .map(|_| {
                    let store = Arc::clone(&store);
                    let id = id.clone();
                    std::thread::spawn(move || store.take_publish_plan("did:plc:priya", &id).expect("take"))
                })
                .collect();
            let took = takers
                .into_iter()
                .filter_map(|t| t.join().expect("taker"))
                .count();
            prop_assert_eq!(took, 1);
        }
    }
}
