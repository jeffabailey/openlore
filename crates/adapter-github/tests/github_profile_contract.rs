//! DD-GDL-11 recorded-fixture contract: GitHub's real `/users/{login}` and
//! `/user/{id}` bodies (see `crates/test-support/fixtures/github/PROVENANCE.md`)
//! decode through the adapter's production reshape into the same account,
//! and a body naming no account is an API-shape error (ADR-085 §4).

use adapter_github::client::parse_account_profile;

fn recorded(name: &str) -> serde_json::Value {
    let path = format!(
        "{}/../test-support/fixtures/github/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn the_login_read_and_the_account_number_read_decode_to_the_same_account() {
    let by_login = parse_account_profile(&recorded("users_profile.json")).expect("by login");
    let by_id = parse_account_profile(&recorded("user_by_id.json")).expect("by id");

    assert_eq!(by_login.login, "octocat");
    assert_eq!(by_login.id, 583_231);
    assert_eq!(by_login.html_url, "https://github.com/octocat");
    assert_eq!(by_login.bio, None, "a null bio reads as no bio");
    assert_eq!(
        by_id, by_login,
        "/user/{{id}} serves the same profile shape"
    );
}

/// The recorded profile with one field replaced (`None` removes it).
fn recorded_with(field: &str, value: Option<serde_json::Value>) -> serde_json::Value {
    let mut body = recorded("user_by_id.json");
    let object = body.as_object_mut().expect("a profile is a JSON object");
    match value {
        Some(value) => object.insert(field.to_string(), value),
        None => object.remove(field),
    };
    body
}

#[test]
fn a_profile_without_an_account_number_or_login_names_no_account() {
    for (case, body) in [
        ("no id", recorded_with("id", None)),
        ("id 0", recorded_with("id", Some(serde_json::json!(0)))),
        (
            "id as a string",
            recorded_with("id", Some(serde_json::json!("583231"))),
        ),
        (
            "negative id",
            recorded_with("id", Some(serde_json::json!(-1))),
        ),
        ("no login", recorded_with("login", None)),
        (
            "login not a string",
            recorded_with("login", Some(serde_json::json!(583_231))),
        ),
        (
            "blank login",
            recorded_with("login", Some(serde_json::json!("  "))),
        ),
    ] {
        let detail = parse_account_profile(&body).expect_err(case);
        let field = if case.contains("login") {
            "login"
        } else {
            "id"
        };
        assert!(detail.contains(field), "{case}: {detail}");
    }
}

// =============================================================================
// The strict account of the last resolved user (UC-8 / U-4, ADR-085 §4)
// =============================================================================

use adapter_github::GithubAdapter;
use ports::GithubPort;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

/// A fake GitHub answering each path with a fixed body (404 otherwise) and
/// remembering every path it was asked for.
struct FakeProfiles {
    base: String,
    asked: Arc<Mutex<Vec<String>>>,
}

impl FakeProfiles {
    fn serve(routes: Vec<(&str, serde_json::Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let routes: Vec<(String, String)> = routes
            .into_iter()
            .map(|(path, body)| (path.to_string(), body.to_string()))
            .collect();
        let asked = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&asked);
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().expect("clone"));
                let mut request_line = String::new();
                if reader.read_line(&mut request_line).is_err() {
                    continue;
                }
                let mut header = String::new();
                while reader.read_line(&mut header).is_ok_and(|n| n > 2) {
                    header.clear();
                }
                let path = request_line
                    .split_whitespace()
                    .nth(1)
                    .unwrap_or_default()
                    .to_string();
                seen.lock().expect("asked").push(path.clone());
                let (status, body) = routes.iter().find(|(route, _)| *route == path).map_or(
                    ("404 Not Found", r#"{"message":"Not Found"}"#.to_string()),
                    |(_, body)| ("200 OK", body.clone()),
                );
                let reply = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let mut stream = stream;
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        Self { base, asked }
    }

    fn adapter(&self) -> GithubAdapter {
        GithubAdapter::for_api_base(self.base.clone())
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().expect("asked").clone()
    }
}

fn octocat_repo() -> serde_json::Value {
    serde_json::json!({"full_name": "octocat/hello", "private": false})
}

#[tokio::test]
async fn a_resolved_user_names_its_strict_account_without_another_request() {
    let github = FakeProfiles::serve(vec![("/users/octocat", recorded("users_profile.json"))]);
    let adapter = github.adapter();

    let kind = adapter.resolve_target("octocat").await.expect("resolves");
    let account = adapter.resolved_account("octocat").expect("an account");

    assert!(matches!(kind, ports::TargetKind::User { .. }), "{kind:?}");
    assert_eq!((account.login.as_str(), account.id), ("octocat", 583_231));
    assert_eq!(
        github.asked(),
        ["/users/octocat"],
        "one read, none for the account"
    );
}

#[tokio::test]
async fn a_resolved_user_body_naming_no_account_observes_none() {
    for (case, field) in [("no id", "id"), ("no login", "login")] {
        let mut body = recorded("users_profile.json");
        body.as_object_mut().expect("object").remove(field);
        let github = FakeProfiles::serve(vec![("/users/octocat", body)]);
        let adapter = github.adapter();

        adapter.resolve_target("octocat").await.expect(case);

        assert_eq!(adapter.resolved_account("octocat"), None, "{case}");
    }
}

#[tokio::test]
async fn no_resolve_observes_no_account() {
    let github = FakeProfiles::serve(vec![("/users/octocat", recorded("users_profile.json"))]);

    assert_eq!(github.adapter().resolved_account("octocat"), None);
    assert!(github.asked().is_empty(), "{:?}", github.asked());
}

#[tokio::test]
async fn any_later_resolve_clears_the_resolved_account() {
    let github = FakeProfiles::serve(vec![
        ("/users/octocat", recorded("users_profile.json")),
        ("/repos/octocat/hello", octocat_repo()),
    ]);
    for later in ["octocat/hello", "ghost"] {
        let adapter = github.adapter();
        adapter.resolve_target("octocat").await.expect("resolves");

        let _ = adapter.resolve_target(later).await;

        assert_eq!(adapter.resolved_account("octocat"), None, "after {later}");
    }
}

#[tokio::test]
async fn the_resolved_account_is_only_for_the_user_resolved() {
    let github = FakeProfiles::serve(vec![("/users/octocat", recorded("users_profile.json"))]);
    let adapter = github.adapter();
    adapter.resolve_target("octocat").await.expect("resolves");

    assert_eq!(adapter.resolved_account("hubot"), None);
}
