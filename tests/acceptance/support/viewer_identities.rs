//! github-did-identity-link slice 4 — the viewer's linked-identity region
//! (DISTILL 2026-10-19, Quinn).
//!
//! Step vocabulary for `viewer_linked_identity.rs` and
//! `viewer_linked_identity_invariants.rs`. Every page is fetched from the REAL
//! `openlore ui` viewer (the production composition root, Pillar 3) over the
//! REAL DuckDB store the identity verbs wrote; every GIVEN reuses the slice-1/2
//! vocabulary in `identity_links.rs` (real `identity link`/`verify`, real
//! `peer add`/`pull`/`remove`). The viewer is wired to the `FakeGithubAccounts`
//! double ONLY so that a request it made would be observed (C-VL-2).
//!
//! **Reading a page as a listing.** The region's lines are the `identity list`
//! lines character for character (D-VL-5), so a page is read back as a listing
//! ([`region_listing`]) and the slice-2 assertions (`assert_shown`,
//! `assert_check_age`) apply unchanged — one vocabulary for both readers.
//!
//! **Markup the tests rely on (DISTILL-owned, DELIVER may rename here, in one
//! place):** the region is the `<section>` labelled (`aria-labelledby`) by a
//! heading reading [`LINKED_IDENTITIES_HEADING`]; its lines are the `<li>` of a
//! `<ul>`; a line's text is the
//! element text with tags removed; the DID and the account are `<a href>`s to
//! `/score?contributor=` and `/project?subject=` (percent-encoded as every
//! viewer cross-link is, ADR-044). On `/search` a region sits inside each
//! author group (`<h2>Author: <did></h2>`).
//!
//! **Clock.** The viewer reads `now` through the same clock seam as the CLI
//! identity verbs (`OPENLORE_TEST_NOW`, development builds), so a page and an
//! `identity list` run at the same pinned instant must print the same ages.
//!
//! **One process at a time.** The viewer holds the DuckDB file for its whole
//! life; every CLI verb or seeded row runs while no viewer is open, so each
//! step opens a viewer, reads its pages and drops it.

use super::identity_links::*;
use super::people::*;
use super::*;
use openlore_test_support::RawRecordSpec;

// =============================================================================
// Wording and markup pins (viewer-only wording: D-VL-5 open question 3, defaulted)
// =============================================================================

/// The region heading.
pub const LINKED_IDENTITIES_HEADING: &str = "Linked identities";
/// The one legend line beneath the lines.
pub const LINKED_IDENTITIES_LEGEND: &str =
    "As you last checked them. The viewer never asks GitHub; re-check with openlore identity verify.";
/// What the region says when the link reads failed (D-VL-7).
pub const LINKED_IDENTITIES_UNREADABLE: &str =
    "Linked identities could not be read from your local store.";
/// `/search` with no network index configured (slice-08 wording).
pub const SEARCH_UNAVAILABLE: &str =
    "The network index is unavailable. Your local store views still work.";
/// The debug-only fault seam that fails one link read on every page request
/// (never the startup probe): value = [`LinkRead::token`].
pub const FAIL_IDENTITY_LINKS_ENV: &str = "OPENLORE_VIEWER_FAIL_IDENTITY_LINKS";
/// The startup event the viewer emits for its read-only link probe.
pub const IDENTITY_LINKS_PROBE_EVENT: &str = "viewer.identity_links.probe";

/// Rachel's second, older GitHub account (her withdrawn link).
pub const RACHEL_OLD_GH: Account = Account {
    login: "rachel-old",
    id: 2_345_999,
};

// =============================================================================
// Typed domain values
// =============================================================================

/// A page about one person (D-VL-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonPage {
    /// `/score?contributor=<did>` — a contributor.
    Contributor(&'static str),
    /// `/project?subject=github:<login>` — a GitHub person, spelled as typed.
    GithubPerson(String),
    /// `/claims/{cid}` — one of my claims; the region is about its author.
    ClaimDetail(String),
    /// `/search?object=<object>` — network search; a region per author group.
    Search(String),
}

impl PersonPage {
    /// The request path.
    pub fn path(&self) -> String {
        match self {
            PersonPage::Contributor(did) => format!("/score?contributor={did}"),
            PersonPage::GithubPerson(login) => format!("/project?subject=github:{login}"),
            PersonPage::ClaimDetail(cid) => format!("/claims/{cid}"),
            PersonPage::Search(object) => {
                format!("/search?object={}", encode_query_component_for_test(object))
            }
        }
    }

    /// The page about a GitHub account, spelled as the account's login.
    pub fn of_account(account: Account) -> Self {
        PersonPage::GithubPerson(account.login.to_string())
    }
}

/// One of the three reads behind the region (D-VL-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkRead {
    /// This reader's proof checks.
    Checks,
    /// The stored link candidates.
    Candidates,
    /// The retraction / supersession markers of the candidates.
    Lineage,
}

impl LinkRead {
    pub const ALL: [LinkRead; 3] = [LinkRead::Checks, LinkRead::Candidates, LinkRead::Lineage];

    /// The fault-seam value that fails this read.
    pub fn token(self) -> &'static str {
        match self {
            LinkRead::Checks => "checks",
            LinkRead::Candidates => "candidates",
            LinkRead::Lineage => "lineage",
        }
    }
}

/// How GitHub stands while the viewer serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GithubStanding {
    Up,
    Unreachable,
}

/// How a viewer is opened over my world.
#[derive(Default)]
pub struct ViewerOpts<'a> {
    /// Pinned clock (`None` = the real clock).
    pub now: Option<&'a str>,
    /// The network index `/search` queries (`None` = not configured).
    pub indexer: Option<IndexerHandle>,
    /// A link read that fails on every page request.
    pub failing_read: Option<LinkRead>,
}

// =============================================================================
// Driving the production viewer
// =============================================================================

/// Open the REAL `openlore ui` over my store, its GitHub base pointed at the
/// `FakeGithubAccounts` double (so any request it made is logged).
pub fn open_viewer(world: &LinkWorld, opts: ViewerOpts<'_>) -> ViewerServer {
    let mut extra: Vec<(&str, String)> = vec![(
        "OPENLORE_GITHUB_API_BASE",
        world.github.base_url().to_string(),
    )];
    if let Some(now) = opts.now {
        extra.push(("OPENLORE_TEST_NOW", now.to_string()));
    }
    if let Some(read) = opts.failing_read {
        extra.push((FAIL_IDENTITY_LINKS_ENV, read.token().to_string()));
    }
    ViewerServer::start_with_env(&world.env, opts.indexer, &extra)
}

/// WHEN: I open `page` in `viewer` (a plain browser request: the full page).
pub fn when_i_open(viewer: &ViewerServer, page: &PersonPage) -> ViewerResponse {
    viewer.get(&page.path())
}

/// WHEN: I open each page in a fresh viewer at `now`, then close it.
pub fn when_i_open_pages(
    world: &LinkWorld,
    now: Option<&str>,
    pages: &[PersonPage],
) -> Vec<ViewerResponse> {
    let viewer = open_viewer(
        world,
        ViewerOpts {
            now,
            ..ViewerOpts::default()
        },
    );
    pages.iter().map(|p| when_i_open(&viewer, p)).collect()
}

/// WHEN: I open one page in a fresh viewer at `now`, then close it.
pub fn when_i_open_page(world: &LinkWorld, now: Option<&str>, page: &PersonPage) -> ViewerResponse {
    when_i_open_pages(world, now, std::slice::from_ref(page))
        .pop()
        .expect("one page")
}

/// GIVEN: one of my own ordinary claims (real `claim add`). Returns its CID.
pub fn given_my_claim(world: &LinkWorld) -> String {
    seed_own_claim_with_evidence(
        &world.env,
        "owner/repo-linked-identity",
        "is-maintained-by",
        "The Maintainers",
        0.9,
        &[],
    )
}

/// GIVEN (degraded store, a real condition): my store's proof-check history
/// can no longer be read — the check table is gone while its schema version
/// stays recorded, as when a store is damaged. The candidate and lineage
/// reads still work; every read of checks fails.
pub fn given_my_proof_history_cannot_be_read(env: &TestEnv) {
    let conn = duckdb::Connection::open(env.duckdb_path()).expect("open DuckDB");
    conn.execute_batch("DROP TABLE identity_link_checks")
        .unwrap_or_else(|e| panic!("make the proof-check history unreadable: {e}"));
}

/// GIVEN: the network index holds these public records (REAL ingest + serve).
pub fn given_the_network_index_holds(env: &TestEnv, specs: Vec<RawRecordSpec>) -> IndexerHandle {
    seed_network_index_from_specs(env, specs)
}

/// A self-authored link record as the network index holds it.
pub fn indexed_link(did: &str, account: Account) -> RawRecordSpec {
    RawRecordSpec {
        predicate: SAME_AS.to_string(),
        evidence: vec![account.profile_url(), account.id_uri()],
        ..RawRecordSpec::valid(did, did, &account.subject(), 1.0)
    }
}

/// An indexed `github:` row that is not a link (U-7): `subject maintainedBy object`.
pub fn indexed_github_row(author: &str, subject: &str, account: Account) -> RawRecordSpec {
    RawRecordSpec {
        predicate: "maintainedBy".to_string(),
        ..RawRecordSpec::valid(author, subject, &account.subject(), 0.8)
    }
}

// =============================================================================
// Reading the region
// =============================================================================

/// Every linked-identities region on the page, as HTML, in page order.
pub fn regions_html(body: &str) -> Vec<String> {
    // Selected by role, not by class: the `<section>` whose heading element
    // reads exactly "Linked identities" (the heading labels the region).
    let heading_text = format!(">{LINKED_IDENTITIES_HEADING}</h");
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(rel) = body[from..].find(&heading_text) {
        let at = from + rel;
        let start = body[..at].rfind("<section").unwrap_or(at);
        let end = body[at..]
            .find("</section>")
            .map(|e| at + e + "</section>".len())
            .unwrap_or(body.len());
        found.push(body[start..end].to_string());
        from = end;
    }
    found
}

/// The region's `<section>` is labelled by its heading (`aria-labelledby` →
/// the heading's `id`) and its lines are a list (design § Accessibility).
fn is_labelled_list(region: &str) -> bool {
    let open = &region[..region.find('>').unwrap_or(region.len())];
    let labelled_by = open
        .split("aria-labelledby=\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next());
    labelled_by.is_some_and(|id| region.contains(&format!("id=\"{id}\"")))
        && (line_items_html(region).is_empty() || region.contains("<ul"))
}

/// The page with every linked-identities region cut out (for guardrails that
/// compare the rest of a page, e.g. IR-15).
pub fn without_linked_identities(body: &str) -> String {
    regions_html(body)
        .iter()
        .fold(body.to_string(), |page, region| {
            page.replacen(region, "", 1)
        })
}

/// The text of an HTML fragment: tags removed, entities decoded, whitespace
/// collapsed — what a reader (or a screen reader) is told.
pub fn html_text(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let decoded = text
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The `<li>` elements of one region, as HTML.
pub fn line_items_html(region: &str) -> Vec<String> {
    region
        .split("<li")
        .skip(1)
        .map(|rest| {
            let end = rest.find("</li>").unwrap_or(rest.len());
            format!("<li{}", &rest[..end])
        })
        .collect()
}

/// The lines of every region on the page, as text, in page order.
pub fn region_lines(body: &str) -> Vec<String> {
    regions_html(body)
        .iter()
        .flat_map(|r| line_items_html(r))
        .map(|li| html_text(&li))
        .collect()
}

/// The page read as an `identity list` listing: one line per region line, so
/// the slice-2 assertions apply. `stderr` carries the regions for context.
pub fn region_listing(page: &ViewerResponse) -> CliOutcome {
    CliOutcome {
        status: if page.status == 200 {
            0
        } else {
            i32::from(page.status)
        },
        stdout: region_lines(&page.body).join("\n"),
        stderr: format!("--- regions ---\n{}", regions_html(&page.body).join("\n")),
    }
}

/// An `identity list` run's lines as the viewer prints them: the CLI's
/// backticks become the `<code>` element (D-VL-5), so they are not text.
pub fn listed_lines_as_the_viewer_prints_them(listed: &CliOutcome) -> Vec<String> {
    listed
        .stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.replace('`', ""))
        .collect()
}

/// The HTML of one author group on a `/search` page: its `Author:` heading and
/// everything up to the next author's heading.
pub fn search_group_html(body: &str, author: &str) -> String {
    let heading = format!("Author: {author}");
    let Some(at) = body.find(&heading) else {
        return String::new();
    };
    let rest = &body[at..];
    let end = rest[heading.len()..]
        .find("Author: ")
        .map(|e| e + heading.len())
        .unwrap_or(rest.len());
    rest[..end].to_string()
}

/// The result rows of one search group (regions cut out), as HTML.
pub fn search_rows_html(group: &str) -> Vec<String> {
    without_linked_identities(group)
        .split(SEARCH_VERIFIED_ROW_START)
        .skip(1)
        .map(str::to_string)
        .collect()
}

/// Where each search result row begins (slice-08 row markup).
const SEARCH_VERIFIED_ROW_START: &str = "<span>[verified]</span>";

// =============================================================================
// THEN
// =============================================================================

fn page_context(row: &str, page: &ViewerResponse) -> String {
    format!(
        "\n[{row}] status {}\n--- page ---\n{}",
        page.status, page.body
    )
}

/// THEN: the page loads (200).
pub fn assert_page_loads(row: &str, page: &ViewerResponse) {
    assert_eq!(
        page.status,
        200,
        "[{row}] the page loads;{}",
        page_context(row, page)
    );
}

/// THEN: the page carries a linked-identities region: its heading, its lines
/// and the legend beneath them.
pub fn assert_region_with_legend(row: &str, page: &ViewerResponse) {
    assert_page_loads(row, page);
    let regions = regions_html(&page.body);
    assert!(
        !regions.is_empty(),
        "[{row}] expected a `{LINKED_IDENTITIES_HEADING}` region;{}",
        page_context(row, page)
    );
    for region in &regions {
        let text = html_text(region);
        assert!(
            text.contains(LINKED_IDENTITIES_HEADING) && text.contains(LINKED_IDENTITIES_LEGEND),
            "[{row}] the region has its heading and the legend `{LINKED_IDENTITIES_LEGEND}`: {text:?}"
        );
        assert!(
            is_labelled_list(region),
            "[{row}] the region is labelled by its heading and its lines are a list: {region}"
        );
    }
}

/// THEN: the page has no linked-identities region at all (no empty heading).
pub fn assert_no_region(row: &str, page: &ViewerResponse) {
    assert_page_loads(row, page);
    assert!(
        regions_html(&page.body).is_empty() && !page.body.contains(LINKED_IDENTITIES_HEADING),
        "[{row}] expected no `{LINKED_IDENTITIES_HEADING}` region;{}",
        page_context(row, page)
    );
}

/// THEN: every region on the page says the link reads failed, and nothing
/// else: no line, no standing word.
pub fn assert_region_unreadable(row: &str, page: &ViewerResponse) {
    assert_page_loads(row, page);
    let regions = regions_html(&page.body);
    assert!(
        !regions.is_empty(),
        "[{row}] expected a region saying `{LINKED_IDENTITIES_UNREADABLE}`;{}",
        page_context(row, page)
    );
    for region in &regions {
        let text = html_text(region);
        assert!(
            text.contains(LINKED_IDENTITIES_UNREADABLE),
            "[{row}] the region says `{LINKED_IDENTITIES_UNREADABLE}`: {text:?}"
        );
        assert!(
            line_items_html(region).is_empty()
                && [
                    "not checked",
                    "verified",
                    "proof gone",
                    "contested",
                    " claims "
                ]
                .iter()
                .all(|word| !text.contains(word)),
            "[{row}] an unreadable region shows no line and no standing: {text:?}"
        );
    }
}

/// THEN: the line naming `did` and `account` links the DID to its contributor
/// page and the account to its GitHub person page.
pub fn assert_line_links_to_both_people(
    row: &str,
    page: &ViewerResponse,
    did: &str,
    account: Account,
) {
    let object = account.subject();
    let item = regions_html(&page.body)
        .iter()
        .flat_map(|r| line_items_html(r))
        .find(|li| {
            let text = html_text(li);
            text.contains(did) && text.contains(&object)
        })
        .unwrap_or_else(|| {
            panic!(
                "[{row}] expected a line naming {did} and {object};{}",
                page_context(row, page)
            )
        });
    let to_did = format!(
        "href=\"/score?contributor={}\"",
        encode_query_component_for_test(did)
    );
    let to_account = format!(
        "href=\"/project?subject={}\"",
        encode_query_component_for_test(&object)
    );
    assert!(
        item.contains(&to_did) && item.contains(&to_account),
        "[{row}] the DID links with `{to_did}` and the account with `{to_account}`: {item}"
    );
}

/// THEN: the page's region lines equal the `identity list` lines, word for word.
pub fn assert_reads_as_listed(row: &str, page: &ViewerResponse, listed: &CliOutcome) {
    assert_eq!(
        listed.status, 0,
        "[{row}] precondition: `identity list` ran;\n{}\n{}",
        listed.stdout, listed.stderr
    );
    let expected = listed_lines_as_the_viewer_prints_them(listed);
    assert!(
        !expected.is_empty(),
        "[{row}] precondition: `identity list` shows a line;\n{}",
        listed.stdout
    );
    assert_eq!(
        region_lines(&page.body),
        expected,
        "[{row}] the viewer reads exactly as `identity list` prints it;{}",
        page_context(row, page)
    );
}

/// THEN: the viewer said, at startup, that its link probe was refused.
pub fn assert_link_probe_refused(viewer: &ViewerServer) {
    let refused = viewer.startup_lines().iter().any(|line| {
        serde_json::from_str::<serde_json::Value>(line)
            .map(|e| e["event"] == IDENTITY_LINKS_PROBE_EVENT && e["refused"] == true)
            .unwrap_or(false)
    });
    assert!(
        refused,
        "expected `{{\"event\":\"{IDENTITY_LINKS_PROBE_EVENT}\",\"refused\":true}}` at startup: {:?}",
        viewer.startup_lines()
    );
}

/// THEN: the viewer did NOT report a refused link probe at startup (per-request
/// failures and an upgraded store are not a damaged store).
pub fn assert_link_probe_not_refused(row: &str, viewer: &ViewerServer) {
    let refused = viewer.startup_lines().iter().any(|line| {
        serde_json::from_str::<serde_json::Value>(line)
            .map(|e| e["event"] == IDENTITY_LINKS_PROBE_EVENT && e["refused"] == true)
            .unwrap_or(false)
    });
    assert!(
        !refused,
        "[{row}] the startup link probe passed: {:?}",
        viewer.startup_lines()
    );
}

/// The lines of `identity list` (every person) at `now`, as the user reads
/// them: the driving-port view of "what Jeff's store says" (review VL-B1).
pub fn what_jeff_lists(world: &LinkWorld, now: Option<&str>) -> Vec<String> {
    let listed = when_i_list(world, now);
    assert_eq!(
        listed.status, 0,
        "`identity list` runs;\n{}\n{}",
        listed.stdout, listed.stderr
    );
    listed.stdout.lines().map(str::to_string).collect()
}
