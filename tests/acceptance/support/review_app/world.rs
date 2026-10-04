//! The shared step vocabulary (Pillar 1 + Pillar 2) for every review-app suite.
//!
//! Each `given_*` step REUSES the previous scenario's `given_* + when_*`
//! (chained narrative): `given_signed_in` → `given_github_verified` →
//! `given_pending_suggestions` → `given_published`. Step bodies only drive the
//! app's HTTP driving port (through [`Browser`]) or read a driven-EXTERNAL
//! double's observations (the user's PDS, GitHub). They never touch the app's
//! private store.

use std::collections::{HashMap, HashSet};

use openlore_test_support::{
    BlueskyAccount, FakeAtprotoNetwork, FakeGithubAccounts, GithubAccount, GithubRepo,
    StoredRecord, CLAIM_COLLECTION, POST_COLLECTION,
};

use super::app::{AppSettings, ReviewApp, Startup};
use super::browser::{Browser, Page};
use super::domain::*;

// =============================================================================
// The world: the doubles + the real app
// =============================================================================

/// The hermetic world: GitHub + the ATProto network (fakes) and the REAL
/// review app wired to them.
pub struct ReviewWorld {
    pub github: FakeGithubAccounts,
    pub atproto: FakeAtprotoNetwork,
    pub app: ReviewApp,
}

/// The canonical GitHub accounts (requirements.md personas).
pub fn github_fixtures() -> Vec<GithubAccount> {
    vec![
        GithubAccount::new("priyaraman", 4_210_001)
            .bio(&format!("Rust, tide models. {}", Persona::Priya.did()))
            .repo(
                GithubRepo::owned("tidepool")
                    .language("Rust")
                    .stars(120)
                    .with_cargo_lock()
                    .with_ci_and_tests()
                    .with_semver_and_changelog(),
            )
            .repo(GithubRepo::owned("quill-docs").stars(40).with_docs())
            .repo(GithubRepo::owned("estuary").stars(5))
            .repo(GithubRepo::forked("serde")),
        GithubAccount::new("dvolkov", 5_310_002)
            .bio(&format!("Systems hacker. {DMITRI_OLD_DID}"))
            .repo(GithubRepo::owned("ferrite").stars(30).with_cargo_lock()),
        GithubAccount::new("aishab", 6_410_003)
            .bio(&format!("Learning Rust! {}", Persona::Aisha.did()))
            .repo(GithubRepo::forked("rustlings"))
            .repo(GithubRepo::forked("book")),
        GithubAccount::new("samortega", 7_510_004),
        GithubAccount::new(BURNTSUSHI, 456_674)
            .bio("I like Rust.")
            .repo(
                GithubRepo::owned("ripgrep")
                    .language("Rust")
                    .with_cargo_lock(),
            ),
    ]
}

/// The canonical Bluesky accounts.
pub fn bluesky_accounts() -> Vec<BlueskyAccount> {
    Persona::ALL
        .iter()
        .map(|p| BlueskyAccount::new(p.handle(), p.did(), p.pds_host()))
        .collect()
}

/// App settings pointing at the given doubles.
pub fn settings_for(github: &FakeGithubAccounts, atproto: &FakeAtprotoNetwork) -> AppSettings {
    AppSettings {
        github_base: github.base_url().to_string(),
        plc_url: atproto.directory_url().to_string(),
        handle_resolver_url: atproto.directory_url().to_string(),
        github_token: SERVER_GITHUB_TOKEN.to_string(),
        omit_secrets: Vec::new(),
        extra_env: Vec::new(),
    }
}

impl ReviewWorld {
    /// The canonical world with the app up and ready.
    pub fn new() -> Self {
        Self::with_settings(|_| {})
    }

    /// The canonical world with adjusted app settings.
    pub fn with_settings(tweak: impl FnOnce(&mut AppSettings)) -> Self {
        let github = FakeGithubAccounts::start(github_fixtures());
        let atproto = FakeAtprotoNetwork::start(bluesky_accounts());
        let mut settings = settings_for(&github, &atproto);
        tweak(&mut settings);
        let app = ReviewApp::start(settings);
        Self {
            github,
            atproto,
            app,
        }
    }

    /// Launch the app against fresh doubles, returning the startup outcome
    /// (for the startup-refusal scenarios).
    pub fn launch(
        prepare: impl FnOnce(&FakeGithubAccounts, &FakeAtprotoNetwork),
        tweak: impl FnOnce(&mut AppSettings),
    ) -> (FakeGithubAccounts, FakeAtprotoNetwork, Startup) {
        let github = FakeGithubAccounts::start(github_fixtures());
        let atproto = FakeAtprotoNetwork::start(bluesky_accounts());
        prepare(&github, &atproto);
        let mut settings = settings_for(&github, &atproto);
        tweak(&mut settings);
        let startup = ReviewApp::launch(settings);
        (github, atproto, startup)
    }

    /// A fresh browser (no cookies) pointed at the app.
    pub fn browser(&self) -> Browser {
        Browser::new(self.app.origin())
    }

    /// Stop and start the app on the same state.
    pub fn restart_app(self) -> Self {
        let ReviewWorld {
            github,
            atproto,
            app,
        } = self;
        let app = app.restart();
        Self {
            github,
            atproto,
            app,
        }
    }
}

impl Default for ReviewWorld {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// GitHub-side actions the people take outside the app
// =============================================================================

/// The person puts their CURRENT signed-in DID into their GitHub bio.
pub fn puts_did_in_bio(world: &ReviewWorld, persona: Persona) {
    world.github.set_bio(
        persona.github_login(),
        Some(&format!("Builder. {}", persona.did())),
    );
}

/// The person removes the DID from their GitHub bio.
pub fn removes_did_from_bio(world: &ReviewWorld, persona: Persona) {
    world
        .github
        .set_bio(persona.github_login(), Some("Rust, tide models."));
}

/// Priya adds a CHANGELOG + semver tags to `estuary` (US-BRA-010).
pub fn priya_adds_changelog_to_estuary(world: &ReviewWorld) {
    world.github.put_repo(
        "priyaraman",
        GithubRepo::owned("estuary")
            .stars(5)
            .with_semver_and_changelog(),
    );
}

// =============================================================================
// Sign-in (US-BRA-001)
// =============================================================================

/// WHEN `persona` signs in with Bluesky and authorizes the app at their PDS.
pub fn when_signs_in(browser: &mut Browser, persona: Persona) -> &Page {
    when_signs_in_with_handle(browser, persona.handle())
}

/// WHEN someone types `handle` and presses "Sign in with Bluesky".
pub fn when_signs_in_with_handle<'b>(browser: &'b mut Browser, handle: &str) -> &'b Page {
    browser.open("/");
    browser.fill("handle", handle);
    browser.press("Sign in with Bluesky")
}

/// GIVEN `persona` is signed in (WS-1's Given + When).
pub fn given_signed_in(world: &ReviewWorld, persona: Persona) -> Browser {
    let mut browser = world.browser();
    when_signs_in(&mut browser, persona);
    then_sees_signed_in_as(&browser.page, persona);
    browser
}

pub fn then_sees_signed_in_as(page: &Page, persona: Persona) {
    let expected = format!("Signed in as {}", persona.at_handle());
    assert!(
        page.shows(&expected),
        "expected {expected:?} after signing in; page {} ({}):\n{}",
        page.url,
        page.status,
        page.text()
    );
}

/// `true` when the browser holds a live session (the queue opens instead of
/// asking to sign in).
pub fn has_session(browser: &mut Browser) -> bool {
    let page = browser.open("/review");
    page.status == 200 && !page.offers("Sign in with Bluesky")
}

/// The CSRF token carried by the current page's forms (empty when none).
pub fn csrf_token(browser: &Browser) -> String {
    browser
        .forms()
        .iter()
        .flat_map(|f| f.fields.clone())
        .find(|(k, _)| k == "csrf")
        .map(|(_, v)| v)
        .unwrap_or_default()
}

/// WHEN the person signs out.
pub fn when_signs_out(browser: &mut Browser) -> &Page {
    browser.open("/review");
    browser.press("Sign out")
}

// =============================================================================
// Ownership proof (US-BRA-002)
// =============================================================================

/// WHEN the signed-in person verifies GitHub username `login`.
pub fn when_verifies_github<'b>(browser: &'b mut Browser, login: &str) -> &'b Page {
    browser.open("/github");
    browser.fill("github_login", login);
    browser.press("Verify")
}

pub fn then_sees_verified(page: &Page, persona: Persona) {
    let expected = format!(
        "Verified: github.com/{} belongs to {}",
        persona.github_login(),
        persona.at_handle()
    );
    assert!(
        page.shows(&expected),
        "expected {expected:?}; page:\n{}",
        page.text()
    );
}

/// GIVEN `persona` has verified their GitHub account (WS-2's Given + When).
pub fn given_github_verified(world: &ReviewWorld, persona: Persona) -> Browser {
    if persona == Persona::Dmitri {
        puts_did_in_bio(world, persona);
    }
    let mut browser = given_signed_in(world, persona);
    when_verifies_github(&mut browser, persona.github_login());
    then_sees_verified(&browser.page, persona);
    browser
}

// =============================================================================
// Scan (US-BRA-003 / US-BRA-010)
// =============================================================================

/// WHEN the person starts a scan and it finishes. Returns the final
/// `data-scan-status` (`completed`, `rate_limited`, `ownership_failed`, …).
pub fn when_scan_finishes(browser: &mut Browser) -> String {
    browser.open("/review");
    let started = ["Scan my repos", "Scan again", "Resume scan"]
        .iter()
        .any(|label| browser.try_press(label).is_ok());
    assert!(
        started,
        "no scan action offered on the queue:\n{}",
        browser.page.text()
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    let status = loop {
        let page = browser.open("/scan/status");
        let status = page.scan_status().unwrap_or_else(|| "missing".into());
        if status != "running" || std::time::Instant::now() > deadline {
            break status;
        }
        std::thread::sleep(std::time::Duration::from_millis(150));
    };
    browser.open("/review");
    status
}

/// GIVEN `persona` has pending suggestions from a finished scan (WS-3's Given + When).
pub fn given_pending_suggestions(world: &ReviewWorld, persona: Persona) -> Browser {
    let mut browser = given_github_verified(world, persona);
    let status = when_scan_finishes(&mut browser);
    assert_eq!(
        status,
        "completed",
        "the first scan must complete:\n{}",
        browser.page.text()
    );
    browser
}

/// The pending suggestions the owner currently sees on the queue.
pub fn pending_on_queue(browser: &mut Browser) -> Vec<String> {
    browser.open("/review").pending_cards()
}

pub fn phrases(suggestions: &[Suggestion]) -> Vec<String> {
    let mut p: Vec<String> = suggestions.iter().map(Suggestion::phrase).collect();
    p.sort();
    p
}

// =============================================================================
// Approve / publish (US-BRA-004 / 005)
// =============================================================================

fn parts(s: &Suggestion) -> [String; 2] {
    s.card_parts()
}

/// WHEN the owner presses Approve on a suggestion (the exact-record preview).
pub fn when_previews_approval(browser: &mut Browser, s: Suggestion) -> &Page {
    browser.open("/review");
    let p = parts(&s);
    browser.press_in_card(&[p[0].as_str(), p[1].as_str()], "Approve")
}

/// WHEN the owner confirms "Publish to my repo" on the preview.
pub fn when_confirms_publish(browser: &mut Browser) -> &Page {
    browser.press("Publish to my repo")
}

/// WHEN the owner edits a suggestion and previews the edited claim.
pub fn when_edits_and_previews<'b>(
    browser: &'b mut Browser,
    s: Suggestion,
    philosophy: Option<Philosophy>,
    confidence: Option<&str>,
) -> &'b Page {
    browser.open("/review");
    let p = parts(&s);
    browser.press_in_card(&[p[0].as_str(), p[1].as_str()], "Edit");
    if let Some(ph) = philosophy {
        browser.fill("object", &ph.object());
    }
    if let Some(c) = confidence {
        browser.fill("confidence", c);
    }
    browser.press("Preview")
}

/// GIVEN `persona` published these suggestions through the app (chains
/// `given_pending_suggestions` + approve/confirm per suggestion).
pub fn given_published(
    world: &ReviewWorld,
    persona: Persona,
    suggestions: &[Suggestion],
) -> Browser {
    let mut browser = given_pending_suggestions(world, persona);
    for s in suggestions {
        when_previews_approval(&mut browser, *s);
        when_confirms_publish(&mut browser);
        assert!(
            browser.page.shows("at://"),
            "publishing {} must show the record address:\n{}",
            s.phrase(),
            browser.page.text()
        );
    }
    browser
}

// =============================================================================
// Decline (US-BRA-006)
// =============================================================================

/// WHEN the owner chooses "Not me" on a suggestion.
pub fn when_declines(browser: &mut Browser, s: Suggestion) -> &Page {
    browser.open("/review");
    let p = parts(&s);
    browser.press_in_card(&[p[0].as_str(), p[1].as_str()], "Not me")
}

// =============================================================================
// Profile, share, retract, disconnect (US-BRA-007/008/011/012)
// =============================================================================

/// WHEN anyone opens `persona`'s public profile page.
pub fn when_opens_profile(browser: &mut Browser, persona: Persona) -> &Page {
    browser.open(&format!("/@{}", persona.handle()))
}

/// WHEN the owner opens "Share on Bluesky…".
pub fn when_opens_share_preview(browser: &mut Browser, persona: Persona) -> &Page {
    when_opens_profile(browser, persona);
    browser.press("Share on Bluesky…")
}

/// WHEN the owner opens the retract preview for a published claim.
pub fn when_opens_retract_preview(browser: &mut Browser, persona: Persona, s: Suggestion) -> &Page {
    when_opens_profile(browser, persona);
    let p = parts(&s);
    browser.press_in_card(&[p[0].as_str(), p[1].as_str()], "Retract")
}

/// WHEN the owner opens the forget-me confirmation.
pub fn when_opens_forget_me(browser: &mut Browser) -> &Page {
    browser.open("/settings");
    browser.press("Disconnect and forget me")
}

// =============================================================================
// Observations at the driven-external ports (the user's PDS)
// =============================================================================

pub fn claims_in_pds(world: &ReviewWorld, persona: Persona) -> Vec<StoredRecord> {
    world.atproto.records(persona.did(), CLAIM_COLLECTION)
}

pub fn posts_in_pds(world: &ReviewWorld, persona: Persona) -> Vec<StoredRecord> {
    world.atproto.records(persona.did(), POST_COLLECTION)
}

/// Recompute a claim record's CID with OpenLore's ONE canonicalizer
/// (`claim-domain`), exactly as every OpenLore reader does.
pub fn recomputed_cid(value: &serde_json::Value) -> String {
    let text = |k: &str| {
        value
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let references = value
        .get("references")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|r| {
            let ref_type = match r.get("type")?.as_str()? {
                "retracts" => claim_domain::ReferenceType::Retracts,
                "corrects" => claim_domain::ReferenceType::Corrects,
                "counters" => claim_domain::ReferenceType::Counters,
                "supersedes" => claim_domain::ReferenceType::Supersedes,
                _ => return None,
            };
            Some(claim_domain::ClaimReference {
                ref_type,
                cid: claim_domain::Cid(r.get("cid")?.as_str()?.to_string()),
            })
        })
        .collect();
    let claim = claim_domain::UnsignedClaim {
        subject: text("subject"),
        predicate: text("predicate"),
        object: text("object"),
        evidence: value
            .get("evidence")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        confidence: value
            .get("confidence")
            .and_then(claim_domain::Confidence::from_wire)
            .unwrap_or_else(|| claim_domain::Confidence::from_basis_points(-1)),
        author_did: claim_domain::Did(text("author")),
        composed_at: text("composedAt"),
        references,
        reason: value
            .get("reason")
            .and_then(|v| v.as_str())
            .map(str::to_string),
    };
    let bytes = claim_domain::canonicalize(&claim).expect("canonicalize a read-back claim");
    claim_domain::compute_cid(&bytes).0
}

/// THEN `persona`'s PDS holds exactly one self-attested claim for `s` at
/// `basis_points`, whose recomputed CID equals its record key.
pub fn then_pds_holds_self_attested_claim(
    world: &ReviewWorld,
    persona: Persona,
    subject: &str,
    philosophy: Philosophy,
    basis_points: i64,
) -> StoredRecord {
    let matching: Vec<StoredRecord> = claims_in_pds(world, persona)
        .into_iter()
        .filter(|r| {
            r.value["subject"] == subject && r.value["object"] == philosophy.object().as_str()
        })
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "{}'s PDS must hold exactly one claim {subject} {}; repo holds: {:#?}",
        persona.handle(),
        philosophy.slug(),
        claims_in_pds(world, persona)
    );
    let record = matching[0].clone();
    let v = &record.value;
    assert_eq!(v["$type"], CLAIM_COLLECTION);
    assert_eq!(v["predicate"], EMBODIES);
    assert_eq!(
        v["confidence"], basis_points,
        "confidence is integer basis points (ADR-070)"
    );
    assert_eq!(
        v["author"],
        persona.did(),
        "a self-attested author is the bare repo DID (ADR-071)"
    );
    assert!(v.get("signature").is_none(), "no app-level signature (D-5)");
    assert!(
        v.get("bucket").is_none() && !v.to_string().contains("speculative"),
        "a bucket is display-only, never stored (WD-10)"
    );
    assert_eq!(
        recomputed_cid(v),
        record.rkey,
        "the record key must be the recomputed CID of the stored record"
    );
    record
}

// =============================================================================
// Universe (Mandate 8): port-exposed observable state
// =============================================================================

/// Slots: the persona's PDS claims/posts, every write attempt anywhere,
/// forbidden write attempts, and the owner's visible pending queue.
pub fn universe() -> HashSet<String> {
    [
        "pds.claims",
        "pds.posts",
        "pds.write_attempts",
        "pds.forbidden_writes",
        "queue.pending",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Capture the universe (opens the owner's queue in `browser`).
pub fn capture(
    world: &ReviewWorld,
    browser: &mut Browser,
    persona: Persona,
) -> HashMap<String, String> {
    let mut claims: Vec<String> = claims_in_pds(world, persona)
        .iter()
        .map(|r| {
            format!(
                "{}|{}|{}|{}",
                r.rkey, r.value["subject"], r.value["object"], r.value["confidence"]
            )
        })
        .collect();
    claims.sort();
    let posts: Vec<String> = posts_in_pds(world, persona)
        .iter()
        .map(|r| r.value["text"].to_string())
        .collect();
    let mut snapshot = HashMap::new();
    snapshot.insert("pds.claims".into(), claims.join("\n"));
    snapshot.insert("pds.posts".into(), posts.join("\n"));
    snapshot.insert(
        "pds.write_attempts".into(),
        world.atproto.write_attempts().len().to_string(),
    );
    snapshot.insert(
        "pds.forbidden_writes".into(),
        world.atproto.forbidden_write_attempts().len().to_string(),
    );
    snapshot.insert("queue.pending".into(), pending_on_queue(browser).join("\n"));
    snapshot
}

/// Every log line must be a JSON object with an `event` from the closed set
/// (DEVOPS observability-design §3.1) and must not contain any of `secrets`.
pub fn then_logs_are_closed_and_leak_nothing(logs: &str, secrets: &[String]) {
    const EVENT_PREFIXES: [&str; 14] = [
        "app.",
        "health.",
        "secrets.",
        "sessions.",
        "schema.",
        "http.",
        "signin.",
        "signout",
        "disconnect",
        "github.",
        "scan.",
        "suggestion.",
        "plan.",
        "pds.",
    ];
    const MORE_PREFIXES: [&str; 6] = [
        "publish.",
        "share.",
        "retract.",
        "upstream.",
        "kpi.",
        "guardrail.",
    ];
    for line in logs.lines().filter(|l| !l.trim().is_empty()) {
        let parsed: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|_| panic!("every log line is JSON (LOG_FORMAT=json); got: {line}"));
        let event = parsed["event"]
            .as_str()
            .unwrap_or_else(|| panic!("log line without `event`: {line}"));
        assert!(
            EVENT_PREFIXES
                .iter()
                .chain(MORE_PREFIXES.iter())
                .any(|p| event.starts_with(p)),
            "event {event:?} is outside the closed catalogue"
        );
    }
    for secret in secrets {
        assert!(
            !logs.contains(secret.as_str()),
            "the logs must never contain {secret:?} (DEVOPS log contract)"
        );
    }
}

// =============================================================================
// Self-attested records as the app writes them (for the reader suites, which
// exercise OpenLore's read path independently of the app — ADR-071)
// =============================================================================

/// The exact shape of a self-attested claim (data-models.md §1): no
/// `signature`, `author` = the bare DID, confidence in basis points.
pub fn self_attested_claim_value(
    author: &str,
    s: Suggestion,
    basis_points: i64,
    references: &[(&str, &str)],
) -> serde_json::Value {
    let mut value = serde_json::json!({
        "$type": CLAIM_COLLECTION,
        "subject": s.subject(),
        "predicate": EMBODIES,
        "object": s.philosophy.object(),
        "evidence": [format!("https://github.com/{}", s.repo_path())],
        "confidence": basis_points,
        "author": author,
        "composedAt": "2026-10-04T15:02:11Z",
    });
    if !references.is_empty() {
        value["references"] = serde_json::Value::Array(
            references
                .iter()
                .map(|(kind, cid)| serde_json::json!({"type": kind, "cid": cid}))
                .collect(),
        );
    }
    value
}

/// Put a record into `repo_did`'s repo under `rkey` (or, with `None`, under
/// its recomputed CID — the honest key). Returns the key used.
pub fn seed_claim_record(
    net: &FakeAtprotoNetwork,
    repo_did: &str,
    value: serde_json::Value,
    rkey: Option<&str>,
) -> String {
    let key = rkey
        .map(str::to_string)
        .unwrap_or_else(|| recomputed_cid(&value));
    net.seed_record(repo_did, CLAIM_COLLECTION, &key, value);
    key
}
