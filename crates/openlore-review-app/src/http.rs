//! The HTTP driving port: a hand-rolled hyper 1.x server (ADR-072; axum is
//! banned). Routing and responses are pure; the accept loop and the page
//! handlers' port calls are the only I/O. Every response, on every listener,
//! carries the security headers.

use std::convert::Infallible;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::header::{HeaderName, HeaderValue, CACHE_CONTROL, CONTENT_TYPE, LOCATION, SET_COOKIE};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use ports::{
    GithubLinkPort, GithubPort, IdentityLookupPort, OAuthPort, PublishPlanPort, ReviewStateRead,
    ReviewStateWrite, ScanRunPort, SessionPort, UserRepoReadPort, UserRepoWritePort,
};
use review_domain::signin::PermissionMode;
use scraper_domain::SignalPredicateMapping;
use tokio::net::TcpListener;

use crate::limiter::ScanLimiter;
use crate::routes::github::VerifyAttempts;
use crate::routes::signin;

/// Sent on every response (AC-000.2, NFR-BRA-2): HTTPS only, own scripts
/// only, never framed, no MIME sniffing, no cross-origin referrer.
pub(crate) const SECURITY_HEADERS: [(&str, &str); 4] = [
    (
        "strict-transport-security",
        "max-age=63072000; includeSubDomains",
    ),
    (
        "content-security-policy",
        "default-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
    ),
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "same-origin"),
];

/// The largest form body a page accepts.
const MAX_FORM_BYTES: usize = 8 * 1024;

/// What the public listener serves, rendered once at startup.
pub(crate) struct Surface {
    pub(crate) landing: String,
    pub(crate) client_metadata: String,
    pub(crate) jwks: String,
}

/// The app's driven ports and its static surface.
pub(crate) struct App {
    pub(crate) surface: Surface,
    pub(crate) origin: String,
    pub(crate) permission_mode: PermissionMode,
    pub(crate) identity: Arc<dyn IdentityLookupPort>,
    pub(crate) oauth: Arc<dyn OAuthPort>,
    pub(crate) sessions: Arc<dyn SessionPort>,
    pub(crate) github: Arc<dyn GithubPort>,
    pub(crate) links: Arc<dyn GithubLinkPort>,
    pub(crate) scans: Arc<dyn ScanRunPort>,
    pub(crate) review_read: Arc<dyn ReviewStateRead>,
    pub(crate) review_write: Arc<dyn ReviewStateWrite>,
    pub(crate) plans: Arc<dyn PublishPlanPort>,
    pub(crate) repo_write: Arc<dyn UserRepoWritePort>,
    pub(crate) repo_read: Arc<dyn UserRepoReadPort>,
    pub(crate) mapping: SignalPredicateMapping,
    pub(crate) verify_attempts: VerifyAttempts,
    pub(crate) scan_limiter: ScanLimiter,
}

/// The static public routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    Landing,
    Healthz,
    Readyz,
    ClientMetadata,
    Jwks,
    CopyScript,
    TriageScript,
    NotFound,
}

/// Pure routing of the public listener's static surface.
pub(crate) fn route(method: &Method, path: &str) -> Route {
    if method != Method::GET && method != Method::HEAD {
        return Route::NotFound;
    }
    match path {
        "/" => Route::Landing,
        "/healthz" => Route::Healthz,
        "/readyz" => Route::Readyz,
        "/oauth/client-metadata.json" => Route::ClientMetadata,
        "/oauth/jwks.json" => Route::Jwks,
        "/assets/copy.js" => Route::CopyScript,
        "/assets/triage.js" => Route::TriageScript,
        _ => Route::NotFound,
    }
}

/// The operator listener has no routes yet (KPI and purge land later).
fn admin_route(_method: &Method, _path: &str) -> Route {
    Route::NotFound
}

/// A page request, already read: query, cookies, form and `Origin`.
#[derive(Debug, Clone, Default)]
pub(crate) struct PageRequest {
    pub(crate) query: Vec<(String, String)>,
    pub(crate) cookies: Vec<(String, String)>,
    pub(crate) form: Vec<(String, String)>,
    pub(crate) origin: Option<String>,
}

/// What a page handler answers (a value; [`render`] makes it a response).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reply {
    Page {
        status: StatusCode,
        html: String,
        set_cookie: Option<String>,
    },
    Redirect {
        location: String,
        set_cookie: Option<String>,
    },
}

fn secured(mut response: Response<Full<Bytes>>) -> Response<Full<Bytes>> {
    let headers = response.headers_mut();
    for (name, value) in SECURITY_HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}

/// Pure response for a static route, with the security headers applied.
pub(crate) fn respond(surface: &Surface, route: Route) -> Response<Full<Bytes>> {
    let html = "text/html; charset=utf-8";
    let json = "application/json";
    let text = "text/plain; charset=utf-8";
    let (status, content_type, body) = match route {
        Route::Landing => (StatusCode::OK, html, surface.landing.clone()),
        Route::Healthz | Route::Readyz => (StatusCode::OK, text, "ok".to_string()),
        Route::ClientMetadata => (StatusCode::OK, json, surface.client_metadata.clone()),
        Route::Jwks => (StatusCode::OK, json, surface.jwks.clone()),
        Route::CopyScript => (
            StatusCode::OK,
            "text/javascript; charset=utf-8",
            review_domain::views::COPY_SCRIPT.to_string(),
        ),
        Route::TriageScript => (
            StatusCode::OK,
            "text/javascript; charset=utf-8",
            review_domain::views::TRIAGE_SCRIPT.to_string(),
        ),
        Route::NotFound => (StatusCode::NOT_FOUND, text, "not found".to_string()),
    };
    let mut response = Response::new(Full::new(Bytes::from(body)));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    secured(response)
}

/// Pure response for a page reply. Pages are never cached.
pub(crate) fn render(reply: Reply) -> Response<Full<Bytes>> {
    let (status, body, location, set_cookie) = match reply {
        Reply::Page {
            status,
            html,
            set_cookie,
        } => (status, html, None, set_cookie),
        Reply::Redirect {
            location,
            set_cookie,
        } => (
            StatusCode::SEE_OTHER,
            String::new(),
            Some(location),
            set_cookie,
        ),
    };
    let mut response = Response::new(Full::new(Bytes::from(body)));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    let mut put = |name, value: Option<String>| {
        if let Some(value) = value.and_then(|v| HeaderValue::from_str(&v).ok()) {
            headers.insert(name, value);
        }
    };
    put(LOCATION, location);
    put(SET_COOKIE, set_cookie);
    secured(response)
}

/// Serve the public and the operator listener until either fails.
pub(crate) async fn serve(
    public: TcpListener,
    admin: TcpListener,
    app: Arc<App>,
) -> std::io::Result<()> {
    tokio::select! {
        result = accept_loop(public, app.clone(), Listener::Public) => result,
        result = accept_loop(admin, app, Listener::Admin) => result,
    }
}

#[derive(Debug, Clone, Copy)]
enum Listener {
    Public,
    Admin,
}

async fn accept_loop(listener: TcpListener, app: Arc<App>, kind: Listener) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let app = app.clone();
        tokio::spawn(async move {
            let service = service_fn(move |request: Request<Incoming>| {
                let app = app.clone();
                async move { Ok::<_, Infallible>(answer(&app, kind, request).await) }
            });
            // A broken client connection ends only that connection.
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
}

async fn answer(
    app: &Arc<App>,
    kind: Listener,
    request: Request<Incoming>,
) -> Response<Full<Bytes>> {
    let (parts, body) = request.into_parts();
    let path = parts.uri.path();
    let page = match kind {
        Listener::Public => signin::page_route(&parts.method, path),
        Listener::Admin => None,
    };
    let Some(page) = page else {
        let routes = match kind {
            Listener::Public => route,
            Listener::Admin => admin_route,
        };
        return respond(&app.surface, routes(&parts.method, path));
    };
    let form = match Limited::new(body, MAX_FORM_BYTES).collect().await {
        Ok(collected) => pairs(&collected.to_bytes()),
        Err(_) => return respond(&app.surface, Route::NotFound),
    };
    let header = |name: &str| {
        parts
            .headers
            .get_all(name)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .collect::<Vec<_>>()
    };
    let request = PageRequest {
        query: pairs(parts.uri.query().unwrap_or("").as_bytes()),
        cookies: header("cookie")
            .into_iter()
            .flat_map(cookie_pairs)
            .collect(),
        form,
        origin: header("origin").first().map(|o| o.to_string()),
    };
    render(signin::handle(app, page, &request).await)
}

/// `application/x-www-form-urlencoded` pairs (query strings and forms).
fn pairs(encoded: &[u8]) -> Vec<(String, String)> {
    url::form_urlencoded::parse(encoded)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect()
}

/// The `name=value` pairs of one `Cookie` header.
fn cookie_pairs(header: &str) -> Vec<(String, String)> {
    header
        .split(';')
        .filter_map(|pair| pair.trim().split_once('='))
        .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn surface() -> Surface {
        Surface {
            landing: "<html></html>".into(),
            client_metadata: "{}".into(),
            jwks: "{\"keys\":[]}".into(),
        }
    }

    fn method() -> impl Strategy<Value = Method> {
        prop_oneof![
            Just(Method::GET),
            Just(Method::HEAD),
            Just(Method::POST),
            Just(Method::PUT),
            Just(Method::DELETE),
        ]
    }

    fn path() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("/".to_string()),
            Just("/healthz".to_string()),
            Just("/readyz".to_string()),
            Just("/oauth/client-metadata.json".to_string()),
            Just("/oauth/jwks.json".to_string()),
            "/[a-z/._-]{0,30}",
        ]
    }

    fn reply() -> impl Strategy<Value = Reply> {
        let cookie = proptest::option::of("[a-zA-Z_=;-]{1,20}");
        prop_oneof![
            (any::<bool>(), ".{0,40}", cookie.clone()).prop_map(|(ok, html, set_cookie)| {
                Reply::Page {
                    status: if ok {
                        StatusCode::OK
                    } else {
                        StatusCode::BAD_REQUEST
                    },
                    html,
                    set_cookie,
                }
            }),
            ("/[a-z/]{0,20}", cookie).prop_map(|(location, set_cookie)| Reply::Redirect {
                location,
                set_cookie,
            }),
        ]
    }

    proptest! {
        /// Universe: every (method, path) on either listener. Every response
        /// carries every security header with exactly its declared value.
        #[test]
        fn every_response_on_every_route_carries_the_security_headers(
            method in method(), path in path(), on_admin in any::<bool>()
        ) {
            let routes: fn(&Method, &str) -> Route = if on_admin { admin_route } else { route };
            let response = respond(&surface(), routes(&method, &path));
            for (name, value) in SECURITY_HEADERS {
                prop_assert_eq!(response.headers().get(name).map(|v| v.to_str().unwrap()), Some(value));
            }
        }

        /// Universe: every page reply. Rendered pages carry the security
        /// headers too, and are never cached.
        #[test]
        fn every_page_reply_carries_the_security_headers_and_is_not_cached(reply in reply()) {
            let response = render(reply);
            for (name, value) in SECURITY_HEADERS {
                prop_assert_eq!(response.headers().get(name).map(|v| v.to_str().unwrap()), Some(value));
            }
            prop_assert_eq!(response.headers().get(CACHE_CONTROL).map(|v| v.to_str().unwrap()), Some("no-store"));
        }
    }
}
