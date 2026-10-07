//! indexer-deployment CORE-9 / CORE-9b — moved VERBATIM (view types,
//! bindings, properties) from `tests/acceptance/indexer_deployment_core.rs`
//! (roadmap review F2): the `cli` test target may not link this crate
//! (check-arch CLI_FORBIDDEN_INDEXER_DEPS), so the public-surface contracts are
//! checked here, against this crate's own routing and admission decisions.
//!
//! RED scaffold: DELIVER 02-04 replaces each `sut_*` body with ONE call into
//! this crate (ADR-083 §1, §3). The 2-route table is closed-world.
//
// SCAFFOLD: true

use proptest::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    Search,
    Health,
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Admission {
    Admitted,
    TooLarge,
    BadRequest,
}

fn sut_route(_method: &str, _path: &str) -> Route {
    todo!("DELIVER 02-04: bind to adapter-xrpc-query-server routing (ADR-083 §1)")
}

fn sut_admit(_body_len: usize, _value_len: usize) -> Admission {
    todo!("DELIVER 02-04: bind to the public request bounds (ADR-083 §3)")
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// CORE-9 @US-IXD-001 @AC-001.3 @FR-IXD-2 @ADR-083 @property @C6a @contract-shape:pure-function
    /// Of every method and path, exactly POST searchClaims is search and GET
    /// /healthz is health; everything else (near misses included) is not found.
    #[test]
    #[ignore = "DELIVER 02-04: public route allowlist"]
    fn only_two_routes_exist_on_the_public_listener(
        method in prop::sample::select(vec!["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS", "post"]),
        path in prop_oneof![
            Just("/xrpc/org.openlore.appview.searchClaims".to_string()),
            Just("/healthz".to_string()),
            Just("/xrpc/org.openlore.appview.searchClaims/".to_string()),
            Just("/HEALTHZ".to_string()),
            Just("/healthz/".to_string()),
            Just("/xrpc/com.atproto.repo.createRecord".to_string()),
            "/[a-zA-Z0-9./_-]{0,40}",
        ],
    ) {
        let expected = match (method, path.as_str()) {
            ("POST", "/xrpc/org.openlore.appview.searchClaims") => Route::Search,
            ("GET", "/healthz") => Route::Health,
            _ => Route::NotFound,
        };
        prop_assert_eq!(sut_route(method, &path), expected);
    }

    /// CORE-9b @US-IXD-001 @NFR-IXD-7 @ADR-083-3 @property @C1b @contract-shape:pure-function
    /// A body over 8 KiB is too large (checked first); otherwise a value over
    /// 512 bytes is a bad request; otherwise the search is admitted.
    #[test]
    #[ignore = "DELIVER 02-04: public request bounds"]
    fn requests_are_admitted_exactly_within_their_bounds(body_len in 0usize..20_000, value_len in 0usize..2_000) {
        let expected = if body_len > 8192 {
            Admission::TooLarge
        } else if value_len > 512 {
            Admission::BadRequest
        } else {
            Admission::Admitted
        };
        prop_assert_eq!(sut_admit(body_len, value_len), expected);
    }
}
