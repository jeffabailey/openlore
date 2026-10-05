//! The operator listener (loopback only: `ADMIN_LISTEN_ADDR` must be a
//! loopback address, and Caddy proxies only the public port — AR-9). It
//! answers aggregate, owner-free KPI sums and forgets a person on request
//! without her session (ADR-074). The public listener never routes here.

use bytes::Bytes;
use http_body_util::Full;
use hyper::{Method, Response, StatusCode};
use review_domain::kpi::{parse_day, sum_counters, utc_day};

use crate::http::{plain, App};
use crate::limiter::unix_now;
use crate::routes::settings::forget;

/// The operator routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdminRoute {
    /// `GET /admin/kpi?from=YYYY-MM-DD&to=YYYY-MM-DD`: JSON sums.
    Kpi,
    /// `POST /admin/purge` with a DID as the body.
    Purge,
}

/// Pure routing of the operator listener.
pub(crate) fn admin_route(method: &Method, path: &str) -> Option<AdminRoute> {
    match (method.as_str(), path) {
        ("GET", "/admin/kpi") => Some(AdminRoute::Kpi),
        ("POST", "/admin/purge") => Some(AdminRoute::Purge),
        _ => None,
    }
}

pub(crate) async fn handle(
    app: &App,
    route: AdminRoute,
    query: &[(String, String)],
    body: &[u8],
) -> Response<Full<Bytes>> {
    match route {
        AdminRoute::Kpi => kpi(app, query),
        AdminRoute::Purge => purge(app, body).await,
    }
}

fn kpi(app: &App, query: &[(String, String)]) -> Response<Full<Bytes>> {
    let today = utc_day(i64::try_from(unix_now()).unwrap_or_default());
    let day = |name: &str| {
        query
            .iter()
            .find(|(key, _)| key == name)
            .map_or(Some(today.as_str()), |(_, value)| parse_day(value))
    };
    let (Some(from), Some(to)) = (day("from"), day("to")) else {
        return text(StatusCode::BAD_REQUEST, "from and to must be YYYY-MM-DD");
    };
    match app.kpi.counters_between(from, to) {
        Ok(rows) => plain(
            StatusCode::OK,
            "application/json",
            serde_json::json!(sum_counters(&rows)).to_string(),
        ),
        Err(_) => text(StatusCode::SERVICE_UNAVAILABLE, "counters unavailable"),
    }
}

async fn purge(app: &App, body: &[u8]) -> Response<Full<Bytes>> {
    let did = std::str::from_utf8(body).map(str::trim).unwrap_or_default();
    if !is_did(did) {
        return text(StatusCode::BAD_REQUEST, "the body must be a DID");
    }
    match forget(app, did).await {
        Ok(outcome) => text(StatusCode::OK, outcome.as_str()),
        Err(_) => text(
            StatusCode::SERVICE_UNAVAILABLE,
            "purge failed; nothing was removed",
        ),
    }
}

/// A DID-shaped body (`did:<method>:<id>`, no whitespace).
fn is_did(text: &str) -> bool {
    let mut parts = text.splitn(3, ':');
    let shaped = matches!(
        (parts.next(), parts.next(), parts.next()),
        (Some("did"), Some(method), Some(id)) if !method.is_empty() && !id.is_empty()
    );
    shaped && text.len() <= 2048 && !text.chars().any(char::is_whitespace)
}

fn text(status: StatusCode, body: &str) -> Response<Full<Bytes>> {
    plain(status, "text/plain; charset=utf-8", body.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// Universe: every (method, path). The operator listener answers
        /// exactly its two routes, and only with their own method.
        #[test]
        fn the_operator_listener_routes_only_kpi_and_purge(
            post in any::<bool>(), path in prop_oneof![Just("/admin/kpi".to_string()), Just("/admin/purge".to_string()), "/[a-z/]{0,20}"]
        ) {
            let method = if post { Method::POST } else { Method::GET };
            let expected = match (post, path.as_str()) {
                (false, "/admin/kpi") => Some(AdminRoute::Kpi),
                (true, "/admin/purge") => Some(AdminRoute::Purge),
                _ => None,
            };
            prop_assert_eq!(admin_route(&method, &path), expected);
        }

        /// Universe: arbitrary bodies. Only a DID-shaped body is purged.
        #[test]
        fn only_a_did_shaped_body_is_purged(method in "[a-z]{1,8}", id in "[a-z0-9.]{1,30}", noise in ".{0,30}") {
            let did = format!("did:{method}:{id}");
            prop_assert!(is_did(&did));
            let padded = format!(" {} x", did);
            prop_assert!(!is_did(&padded));
            prop_assume!(!noise.starts_with("did:"));
            prop_assert!(!is_did(&noise));
        }
    }
}
