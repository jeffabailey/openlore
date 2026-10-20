//! U-OOS-8 recorded-fixture contract (ADR-087 Earned Trust, part of the
//! GitHub contract gate): GitHub's `GET /repos/{o}/{r}` and
//! `GET /repos/{o}/{r}/contents/.github/openlore-authority` replies (see
//! `crates/test-support/fixtures/github/PROVENANCE.md`) read through the
//! production `read_repo_authority`. Every found repo is read exactly twice
//! (the repo, then its authority file on the default branch), whoever asks; a
//! missing repo is `RepoNotFound` with no file read; any file that is not a
//! small UTF-8 regular file grants nothing; a shape error or a rate limit is
//! an error, never an observation, so callers record nothing.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use adapter_github::GithubAdapter;
use ports::{AuthorityFile, FoundRepo, GithubError, GithubPort, RepoObservation, RepoOwnerKind};

const RECORDED_HOST: &str = "https://api.github.com";
const AUTHORITY_FILE: &str = "/contents/.github/openlore-authority";

/// One recorded reply: the path it answered, its status, headers and body.
#[derive(Clone)]
struct RecordedReply {
    path: String,
    status: u16,
    headers: Vec<(String, String)>,
    body: serde_json::Value,
}

/// A recorded case: the production request path and GitHub's replies, in
/// the order production asks for them.
struct RecordedCase {
    request: String,
    replies: Vec<RecordedReply>,
}

impl RecordedCase {
    fn paths(&self) -> Vec<String> {
        self.replies
            .iter()
            .map(|reply| reply.path.clone())
            .collect()
    }

    /// The authority-file reply (the last one), for shape variations.
    fn file_reply(&mut self) -> &mut RecordedReply {
        self.replies.last_mut().expect("a file reply")
    }
}

fn fixture(file: &str) -> serde_json::Value {
    let path = format!(
        "{}/../test-support/fixtures/github/{file}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn recorded_case(case: &serde_json::Value) -> RecordedCase {
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

/// One of the five ADR-087 authority fixtures (`<name>.json`).
fn recorded(name: &str) -> RecordedCase {
    recorded_case(&fixture(&format!("{name}.json")))
}

/// A case of the shipped `/repos` read fixture (`repo_read_cases.json`).
fn recorded_repo_read(case: &str) -> RecordedCase {
    recorded_case(&fixture("repo_read_cases.json")["cases"][case])
}

/// A fake GitHub serving fixed replies by path (404 otherwise), its
/// `Location` headers pointed at itself, remembering every path asked for.
struct FakeGithub {
    base: String,
    asked: Arc<Mutex<Vec<String>>>,
}

impl FakeGithub {
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
}

/// Observe `case` through the production `read_repo_authority`; also what
/// was asked.
async fn observe(case: RecordedCase) -> (Result<RepoObservation, GithubError>, Vec<String>) {
    let (owner, name) = case
        .request
        .trim_start_matches("/repos/")
        .split_once('/')
        .expect("owner/name");
    let github = FakeGithub::serve(case.replies.clone());
    let observed = GithubAdapter::for_api_base(github.base.clone())
        .read_repo_authority(owner, name)
        .await;
    (observed, github.asked())
}

fn found(
    (repo_id, full_name, owner_id, owner_type): (u64, &str, u64, RepoOwnerKind),
    authority_file: AuthorityFile,
) -> RepoObservation {
    RepoObservation::Found(FoundRepo {
        repo_id,
        full_name_seen: full_name.to_string(),
        owner_id,
        owner_type,
        authority_file,
    })
}

const HELLO_WORLD: (u64, &str, u64, RepoOwnerKind) = (
    1_296_269,
    "octocat/Hello-World",
    583_231,
    RepoOwnerKind::User,
);
const GITIGNORE: (u64, &str, u64, RepoOwnerKind) = (
    1_062_897,
    "github/gitignore",
    9_919,
    RepoOwnerKind::Organization,
);
const RUSTUP: (u64, &str, u64, RepoOwnerKind) = (
    43_192_623,
    "rust-lang/rustup",
    5_430_905,
    RepoOwnerKind::Organization,
);

#[tokio::test]
async fn read_repo_authority_decodes_every_recorded_fixture() {
    let listed = AuthorityFile::Present(vec![
        "did:plc:openloreexamplemaintainer01".to_string(),
        "did:web:maintainer.example.com".to_string(),
    ]);
    let cases = [
        ("repo_owner_user", found(HELLO_WORLD, AuthorityFile::Absent)),
        ("repo_owner_org", found(GITIGNORE, AuthorityFile::Absent)),
        (
            "repo_renamed_redirect",
            found(RUSTUP, AuthorityFile::Absent),
        ),
        ("authority_file", found(GITIGNORE, listed)),
        (
            "authority_file_symlink",
            found(HELLO_WORLD, AuthorityFile::Invalid),
        ),
    ];
    for (name, expected) in cases {
        let case = recorded(name);
        let replies = case.paths();

        let (observed, asked) = observe(case).await;

        assert_eq!(observed, Ok(expected), "{name}");
        assert_eq!(asked, replies, "{name}: the repo once, then its file once");
        assert!(
            asked
                .last()
                .is_some_and(|path| path.ends_with(AUTHORITY_FILE)),
            "{name}: {asked:?}"
        );
    }
}

#[tokio::test]
async fn a_missing_repo_is_not_found_and_no_file_is_read() {
    let case = recorded_repo_read("missing_404");
    let request = case.request.clone();

    let (observed, asked) = observe(case).await;

    assert_eq!(observed, Ok(RepoObservation::RepoNotFound));
    assert_eq!(asked, [request], "no authority file is read");
}

#[tokio::test]
async fn an_authority_file_that_is_not_a_small_utf8_file_grants_nothing() {
    use base64::Engine as _;
    let encoded = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let oversize = format!(
        "did:plc:openloreexamplemaintainer01\n{}",
        "# padding\n".repeat(410)
    );
    let not_utf8 = [
        b"did:plc:openloreexamplemaintainer01\n".as_slice(),
        &[0xff, 0xfe],
    ]
    .concat();
    let shapes = [
        (
            "submodule",
            serde_json::json!({"type": "submodule", "size": 0, "name": "openlore-authority",
                "submodule_git_url": "https://github.com/someone/authority.git"}),
        ),
        (
            "directory",
            serde_json::json!([{"type": "file", "name": "dids", "size": 20,
                "path": ".github/openlore-authority/dids"}]),
        ),
        (
            "oversize",
            serde_json::json!({"type": "file", "encoding": "base64", "size": oversize.len(),
                "content": encoded(oversize.as_bytes())}),
        ),
        (
            "not UTF-8",
            serde_json::json!({"type": "file", "encoding": "base64", "size": not_utf8.len(),
                "content": encoded(&not_utf8)}),
        ),
    ];
    for (shape, body) in shapes {
        let mut case = recorded("authority_file");
        case.file_reply().body = body;

        let (observed, asked) = observe(case).await;

        assert_eq!(
            observed,
            Ok(found(GITIGNORE, AuthorityFile::Invalid)),
            "{shape}"
        );
        assert_eq!(asked.len(), 2, "{shape}");
    }
}

#[tokio::test]
async fn a_shape_error_or_a_rate_limit_is_no_observation() {
    let mut no_file_type = recorded("authority_file");
    no_file_type.file_reply().body = serde_json::json!({"name": "openlore-authority"});
    let (observed, _) = observe(no_file_type).await;
    assert!(
        matches!(observed, Err(GithubError::ApiShape(_))),
        "{observed:?}"
    );

    let mut no_owner = recorded("repo_owner_user");
    no_owner.replies[0].body["owner"] = serde_json::Value::Null;
    let (observed, asked) = observe(no_owner).await;
    assert!(
        matches!(observed, Err(GithubError::ApiShape(_))),
        "{observed:?}"
    );
    assert_eq!(
        asked.len(),
        1,
        "no file read for a repo body it cannot read"
    );

    let mut limited = recorded("repo_owner_org");
    let rate_limited = recorded_repo_read("rate_limited_403").replies[0].clone();
    *limited.file_reply() = RecordedReply {
        path: limited.file_reply().path.clone(),
        ..rate_limited
    };
    let (observed, _) = observe(limited).await;
    assert!(
        matches!(observed, Err(GithubError::RateLimited { .. })),
        "{observed:?}"
    );
}
