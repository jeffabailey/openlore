//! U-RRA-3 / U-RRA-7 recorded-fixture contract: GitHub's real paged
//! owned-repo listing (`owned_repos_paged.json`, see
//! `crates/test-support/fixtures/github/PROVENANCE.md`) read through the
//! production `list_account_repos`: it follows `Link: rel="next"` for up to 3
//! pages, says whether the listing was complete, and never follows a next
//! page off GitHub's host (I-06, N-2).

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};

use adapter_github::GithubAdapter;
use ports::{GithubError, GithubPort, ListingExtent};

const RECORDED_HOST: &str = "https://api.github.com";
const FIRST_PAGE: &str = "/users/octocat/repos?type=owner&sort=pushed&per_page=100";

/// One recorded page: its `Link` header and JSON body.
struct RecordedPage {
    link: Option<String>,
    body: serde_json::Value,
}

fn recorded_pages() -> Vec<RecordedPage> {
    let path = format!(
        "{}/../test-support/fixtures/github/owned_repos_paged.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let fixture: serde_json::Value =
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"));
    fixture["pages"]
        .as_array()
        .expect("recorded pages")
        .iter()
        .map(|page| RecordedPage {
            link: page["link"].as_str().map(str::to_string),
            body: page["body"].clone(),
        })
        .collect()
}

/// The path (with query) of each recorded page after the first, in order.
fn recorded_next_paths(pages: &[RecordedPage]) -> Vec<String> {
    (2..=pages.len())
        .map(|page| format!("/user/583231/repos?type=owner&sort=pushed&per_page=2&page={page}"))
        .collect()
}

/// A fake GitHub serving fixed `(path, link, body)` replies (404 otherwise),
/// its `Link` headers pointed at itself, remembering every path asked for.
struct FakeListing {
    base: String,
    asked: Arc<Mutex<Vec<String>>>,
}

impl FakeListing {
    fn serve(routes: Vec<(String, Option<String>, serde_json::Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let base = format!("http://{}", listener.local_addr().expect("addr"));
        let routes: Vec<(String, Option<String>, String)> = routes
            .into_iter()
            .map(|(path, link, body)| {
                let link = link.map(|l| l.replace(RECORDED_HOST, &base));
                (path, link, body.to_string())
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
                let reply = match routes.iter().find(|(route, _, _)| *route == path) {
                    Some((_, link, body)) => format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n{}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                        link.as_ref().map(|l| format!("link: {l}\r\n")).unwrap_or_default(),
                        body.len()
                    ),
                    None => {
                        let body = r#"{"message":"Not Found"}"#;
                        format!(
                            "HTTP/1.1 404 Not Found\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                            body.len()
                        )
                    }
                };
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

/// The recorded pages served at the paths their `Link` headers name, the
/// first at the production listing path.
fn served(pages: Vec<RecordedPage>) -> Vec<(String, Option<String>, serde_json::Value)> {
    let paths = std::iter::once(FIRST_PAGE.to_string()).chain(recorded_next_paths(&pages));
    paths
        .zip(pages)
        .map(|(path, page)| (path, page.link, page.body))
        .collect()
}

fn ids_in(body: &serde_json::Value) -> Vec<u64> {
    body.as_array()
        .expect("a page is an array")
        .iter()
        .map(|row| row["id"].as_u64().expect("id"))
        .collect()
}

#[tokio::test]
async fn a_listing_with_a_fourth_page_reads_three_and_says_it_was_truncated() {
    let pages = recorded_pages();
    assert_eq!(pages.len(), 4, "the recording holds four pages");
    let expected: Vec<u64> = pages[..3].iter().flat_map(|p| ids_in(&p.body)).collect();
    let github = FakeListing::serve(served(pages));

    let listing = github
        .adapter()
        .list_account_repos("octocat")
        .await
        .expect("listed");

    assert_eq!(listing.extent, ListingExtent::Truncated);
    assert_eq!(
        listing.repos.iter().map(|r| r.repo_id).collect::<Vec<_>>(),
        expected
    );
    assert!(listing.repos.iter().all(|r| r.owner_id == 583_231));
    let mut asked = vec![FIRST_PAGE.to_string()];
    asked.extend(recorded_next_paths(&recorded_pages()).into_iter().take(2));
    assert_eq!(github.asked(), asked, "three pages read, the fourth never");
}

#[tokio::test]
async fn a_listing_whose_last_page_names_no_next_is_complete() {
    let mut pages = recorded_pages();
    let last_link = pages[3].link.clone();
    pages.truncate(2);
    pages[1].link = last_link;
    let expected: Vec<u64> = pages.iter().flat_map(|p| ids_in(&p.body)).collect();
    let github = FakeListing::serve(served(pages));

    let listing = github
        .adapter()
        .list_account_repos("octocat")
        .await
        .expect("listed");

    assert_eq!(listing.extent, ListingExtent::Complete);
    assert_eq!(
        listing.repos.iter().map(|r| r.repo_id).collect::<Vec<_>>(),
        expected
    );
    assert_eq!(github.asked().len(), 2);
}

#[tokio::test]
async fn a_next_page_on_another_host_is_refused_not_followed() {
    let mut pages = recorded_pages();
    pages.truncate(1);
    pages[0].link =
        Some("<https://elsewhere.example/user/583231/repos?page=2>; rel=\"next\"".to_string());
    let github = FakeListing::serve(served(pages));

    let refused = github.adapter().list_account_repos("octocat").await;

    assert!(
        matches!(refused, Err(GithubError::ApiShape(_))),
        "{refused:?}"
    );
    assert_eq!(github.asked(), [FIRST_PAGE], "nothing else was asked");
}
