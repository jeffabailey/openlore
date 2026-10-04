//! The HTTP driving port: a hand-rolled hyper 1.x server (ADR-072; axum is
//! banned). Routing and responses are pure; only the accept loop does I/O.
//! Every response, on every listener, carries the security headers.

use std::convert::Infallible;
use std::sync::Arc;

use bytes::Bytes;
use http_body_util::Full;
use hyper::body::Incoming;
use hyper::header::{HeaderName, HeaderValue, CONTENT_TYPE};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

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

/// What the public listener serves, rendered once at startup.
pub(crate) struct Surface {
    pub(crate) landing: String,
    pub(crate) client_metadata: String,
    pub(crate) jwks: String,
}

/// The public routes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Route {
    Landing,
    Healthz,
    Readyz,
    ClientMetadata,
    Jwks,
    NotFound,
}

/// Pure routing of the public listener.
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
        _ => Route::NotFound,
    }
}

/// The operator listener has no routes yet (KPI and purge land later).
fn admin_route(_method: &Method, _path: &str) -> Route {
    Route::NotFound
}

/// Pure response for a route, with the security headers applied.
pub(crate) fn respond(surface: &Surface, route: Route) -> Response<Full<Bytes>> {
    let html = "text/html; charset=utf-8";
    let json = "application/json";
    let text = "text/plain; charset=utf-8";
    let (status, content_type, body) = match route {
        Route::Landing => (StatusCode::OK, html, surface.landing.clone()),
        Route::Healthz | Route::Readyz => (StatusCode::OK, text, "ok".to_string()),
        Route::ClientMetadata => (StatusCode::OK, json, surface.client_metadata.clone()),
        Route::Jwks => (StatusCode::OK, json, surface.jwks.clone()),
        Route::NotFound => (StatusCode::NOT_FOUND, text, "not found".to_string()),
    };
    let mut response = Response::new(Full::new(Bytes::from(body)));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    for (name, value) in SECURITY_HEADERS {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    response
}

/// Serve the public and the operator listener until either fails.
pub(crate) async fn serve(
    public: TcpListener,
    admin: TcpListener,
    surface: Arc<Surface>,
) -> std::io::Result<()> {
    tokio::select! {
        result = accept_loop(public, surface.clone(), route) => result,
        result = accept_loop(admin, surface, admin_route) => result,
    }
}

async fn accept_loop(
    listener: TcpListener,
    surface: Arc<Surface>,
    routes: fn(&Method, &str) -> Route,
) -> std::io::Result<()> {
    loop {
        let (stream, _) = listener.accept().await?;
        let surface = surface.clone();
        tokio::spawn(async move {
            let service = service_fn(move |request: Request<Incoming>| {
                let response = respond(&surface, routes(request.method(), request.uri().path()));
                async move { Ok::<_, Infallible>(response) }
            });
            // A broken client connection ends only that connection.
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
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
    }
}
