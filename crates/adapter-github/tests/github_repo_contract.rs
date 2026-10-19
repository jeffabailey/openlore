//! U-RRA-3 recorded-fixture contract: GitHub's real `GET /repos/{o}/{r}`
//! replies (`repo_read_cases.json`, see
//! `crates/test-support/fixtures/github/PROVENANCE.md`) read through the
//! production `read_repo`: 200 for a user's and an org's repo, 301 for a
//! renamed and a transferred repo (one same-host redirect, facts from the
//! final body), 404 and 403. A redirect off GitHub's host, or a second one,
//! is refused, never followed (ADR-089 §1, N-2).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use adapter_github::GithubAdapter;
use ports::{GithubError, GithubPort, RepoFacts, RepoOwnerKind};

const RECORDED_HOST: &str = "https://api.github.com";

/// One recorded reply: the path it answered, its status, headers and body.
#[derive(Clone)]
struct RecordedReply {
    path: String,
    status: u16,
    headers: Vec<(String, String)>,
    body: serde_json::Value,
}

/// A recorded case: the production request path and GitHub's replies.
struct RecordedCase {
    request: String,
    replies: Vec<RecordedReply>,
}

fn recorded(case: &str) -> RecordedCase {
    let path = format!(
        "{}/../test-support/fixtures/github/repo_read_cases.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let fixture: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    let case = &fixture["cases"][case];
    let replies = case["replies"]
        .as_array()
        .expect("recorded replies")
        .iter()
        .map(|reply| {
            let mut headers: Vec<(String, String)> = reply["headers"]
                .as_object()
                .map(|h| {
                    h.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                        .collect()
                })
                .unwrap_or_default();
            if let Some(location) = reply["location"].as_str() {
                headers.push(("location".to_string(), location.to_string()));
            }
            RecordedReply {
                path: reply["url"]
                    .as_str()
                    .expect("url")
                    .trim_start_matches(RECORDED_HOST)
                    .to_string(),
                status: u16::try_from(reply["status"].as_u64().expect("status")).expect("u16"),
                headers,
                body: reply["body"].clone(),
            }
        })
        .collect();
    RecordedCase {
        request: case["request"].as_str().expect("request").to_string(),
        replies,
    }
}

/// A fake GitHub serving fixed replies by path (404 otherwise), its
/// `Location` headers pointed at itself, remembering every path asked for.
struct FakeRepos {
    base: String,
    asked: Arc<Mutex<Vec<String>>>,
}

impl FakeRepos {
    fn serve(replies: Vec<RecordedReply>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let replies: Vec<RecordedReply> = replies
            .into_iter()
            .map(|mut reply| {
                for (_, value) in &mut reply.headers {
                    *value = value.replace(RECORDED_HOST, &base);
                }
                reply
            })
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
                let reply = replies.iter().find(|r| r.path == path).cloned();
                let (status, headers, body) = reply.map_or_else(
                    || (404, Vec::new(), r#"{"message":"Not Found"}"#.to_string()),
                    |r| (r.status, r.headers, r.body.to_string()),
                );
                let headers: String = headers
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}\r\n"))
                    .collect();
                let reply = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\n{headers}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let mut stream = stream;
                let _ = stream.write_all(reply.as_bytes());
            }
        });
        Self { base, asked }
    }

    fn asked(&self) -> Vec<String> {
        self.asked.lock().expect("asked").clone()
    }

    async fn read(&self, request: &str) -> Result<RepoFacts, GithubError> {
        let (owner, name) = request
            .trim_start_matches("/repos/")
            .split_once('/')
            .expect("owner/name");
        GithubAdapter::for_api_base(self.base.clone())
            .read_repo(owner, name)
            .await
    }
}

/// Read `case` through the production `read_repo`; also what was asked.
async fn read_case(case: &str) -> (Result<RepoFacts, GithubError>, Vec<String>) {
    let case = recorded(case);
    let github = FakeRepos::serve(case.replies);
    let read = github.read(&case.request).await;
    (read, github.asked())
}

#[tokio::test]
async fn a_users_repo_and_an_orgs_repo_read_with_their_owner() {
    let cases = [
        (
            "user_200",
            1_296_269,
            "octocat/Hello-World",
            583_231,
            "octocat",
            RepoOwnerKind::User,
        ),
        (
            "org_200",
            1_062_897,
            "github/gitignore",
            9_919,
            "github",
            RepoOwnerKind::Organization,
        ),
    ];
    for (case, repo_id, full_name, owner_id, owner_login, owner_kind) in cases {
        let (read, asked) = read_case(case).await;

        let expected = RepoFacts {
            repo_id,
            full_name: full_name.to_string(),
            owner_id,
            owner_login: owner_login.to_string(),
            owner_kind,
            fork: false,
            archived: false,
        };
        assert_eq!(read, Ok(expected), "{case}");
        assert_eq!(asked, [recorded(case).request], "{case}: one request");
    }
}

#[tokio::test]
async fn a_renamed_or_transferred_repo_is_read_through_one_same_host_redirect() {
    let cases = [
        (
            "rename_301",
            43_192_623,
            "rust-lang/rustup",
            5_430_905,
            "rust-lang",
        ),
        (
            "transfer_301",
            23_138_984,
            "tokio-rs/mio",
            20_248_544,
            "tokio-rs",
        ),
    ];
    for (case, repo_id, full_name, owner_id, owner_login) in cases {
        let (read, asked) = read_case(case).await;

        let facts = read.unwrap_or_else(|e| panic!("{case}: {e}"));
        assert_eq!(
            (facts.repo_id, facts.full_name.as_str(), facts.owner_id),
            (repo_id, full_name, owner_id),
            "{case}: facts from the final body"
        );
        assert_eq!(facts.owner_login, owner_login, "{case}");
        let followed = format!("/repositories/{repo_id}");
        assert_eq!(asked, [recorded(case).request, followed], "{case}");
    }
}

#[tokio::test]
async fn a_missing_repo_is_not_found_and_a_spent_budget_is_a_rate_limit() {
    let (missing, asked) = read_case("missing_404").await;
    assert!(
        matches!(missing, Err(GithubError::NotFound { .. })),
        "{missing:?}"
    );
    assert_eq!(asked.len(), 1);

    let (limited, asked) = read_case("rate_limited_403").await;
    assert!(
        matches!(limited, Err(GithubError::RateLimited { .. })),
        "{limited:?}"
    );
    assert_eq!(asked.len(), 1);
}

#[tokio::test]
async fn a_redirect_off_githubs_host_is_refused_not_followed() {
    let mut case = recorded("transfer_301");
    case.replies[0].headers = vec![(
        "location".to_string(),
        "http://elsewhere.invalid/repositories/23138984".to_string(),
    )];
    let github = FakeRepos::serve(case.replies);

    let refused = github.read(&case.request).await;

    assert!(
        matches!(refused, Err(GithubError::ApiShape(_))),
        "{refused:?}"
    );
    assert_eq!(github.asked(), [case.request], "nothing else was asked");
}

#[tokio::test]
async fn a_second_redirect_is_refused_not_followed() {
    let mut case = recorded("rename_301");
    let final_body = case.replies[1].clone();
    let moved_again = "/repositories/43192623/moved-again".to_string();
    case.replies[1] = RecordedReply {
        headers: vec![(
            "location".to_string(),
            format!("{RECORDED_HOST}{moved_again}"),
        )],
        ..case.replies[0].clone()
    };
    case.replies[1].path = final_body.path.clone();
    case.replies.push(RecordedReply {
        path: moved_again,
        ..final_body
    });
    let github = FakeRepos::serve(case.replies);

    let refused = github.read(&case.request).await;

    assert!(
        matches!(refused, Err(GithubError::ApiShape(_))),
        "{refused:?}"
    );
    assert_eq!(
        github.asked().len(),
        2,
        "the second redirect is not followed"
    );
}
