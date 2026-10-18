//! github-did-identity-link slice 3 — the review app's "Publish this link" /
//! "Retract this link" step vocabulary (DISTILL 2026-10-14, Quinn).
//!
//! Chained on the shipped GitHub-step vocabulary: `given_github_verified` →
//! `given_link_published`. Step bodies only drive the app's HTTP driving port
//! (through [`Browser`]) or read a driven-EXTERNAL double (the user's PDS,
//! GitHub's ordered request log). Button labels and page phrases are the
//! DISTILL-owned wording pins (Q-GDL-D1); the confirm/cancel labels reuse the
//! shipped preview's ("Publish to my repo", "Confirm retraction", "Cancel").

use openlore_test_support::{StoredRecord, CLAIM_COLLECTION};

use super::browser::{Browser, Page};
use super::domain::*;
use super::html::{self, Form};
use super::world::*;

/// The reserved identity-link predicate (ADR-084 §2).
pub const SAME_AS: &str = "sameAs";

/// The numeric id of each persona's GitHub account in `github_fixtures()`.
pub fn github_id(persona: Persona) -> u64 {
    match persona {
        Persona::Priya => 4_210_001,
        Persona::Dmitri => 5_310_002,
        Persona::Aisha => 6_410_003,
        Persona::Sam => 7_510_004,
    }
}

/// `github:<login>` (ADR-064 person subject).
pub fn github_subject(persona: Persona) -> String {
    format!("github:{}", persona.github_login())
}

/// `https://api.github.com/user/<id>` (ADR-084 §2 evidence).
pub fn github_id_uri(persona: Persona) -> String {
    format!("https://api.github.com/user/{}", github_id(persona))
}

/// WHEN the verified owner presses "Publish this link" on the GitHub step
/// (the exact-record preview; nothing is written yet).
pub fn when_opens_link_preview(browser: &mut Browser) -> &Page {
    browser.open("/github");
    browser.press(PUBLISH_LINK)
}

/// The offer's button label on the GitHub step (Q-GDL-D1 wording pin).
pub const PUBLISH_LINK: &str = "Publish this link";

/// The confirm button on the link preview (reuses the shipped preview's).
pub const CONFIRM_LINK: &str = "Publish to my repo";

/// The field of the offer form that names the GitHub username to link
/// (U-26: the login comes from the offer's form field, as in the Verify form).
pub const LINK_LOGIN_FIELD: &str = "github_login";

/// Every refused link publish ends with this sentence (U-31, RA-5).
pub const NOTHING_PUBLISHED: &str = "Nothing was published.";

/// After a link write fails with a server error the app cannot tell "not
/// stored" from "stored, answer lost", so it says the link may have been
/// saved and that Retry will confirm (U-42, RA-12). It never says
/// `NOTHING_PUBLISHED` there.
pub const MAY_HAVE_BEEN_SAVED: &str = "may have been saved";

/// The refusal once the hourly verify budget is spent (ADR-076 §2, U-26).
pub const TOO_MANY_ATTEMPTS: &str = "Too many verification attempts in the last hour";

/// The refusal for a GitHub username with no account behind it.
pub fn account_not_found_message(login: &str) -> String {
    format!("We couldn't find a GitHub account at github.com/{login}")
}

/// The "Publish this link" offer form on the GitHub step, captured for
/// adversarial replays (forged token, another login, spent budget). Panics,
/// as `Browser::press` would, when the step offers no link to publish.
pub fn link_offer_form(browser: &mut Browser) -> Form {
    browser.open("/github");
    let form = browser
        .forms()
        .into_iter()
        .find(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, PUBLISH_LINK))
        })
        .unwrap_or_else(|| {
            panic!(
                "no button or link labelled {PUBLISH_LINK:?} on /github:\n{}",
                browser.page.text()
            )
        });
    assert!(
        form.fields.iter().any(|(k, _)| k == LINK_LOGIN_FIELD),
        "the offer names the account to link in a {LINK_LOGIN_FIELD:?} field: {form:?}"
    );
    form
}

/// The confirm form on the current link preview, captured for replays.
pub fn link_confirm_form(browser: &Browser) -> Form {
    browser
        .forms()
        .into_iter()
        .find(|f| {
            f.buttons
                .iter()
                .any(|(l, _, _)| html::label_matches(l, CONFIRM_LINK))
        })
        .unwrap_or_else(|| {
            panic!(
                "the link preview's confirm form ({CONFIRM_LINK:?}):\n{}",
                browser.page.text()
            )
        })
}

/// WHEN a (possibly altered) offer form is submitted: the link preview request.
pub fn when_submits_link_offer<'b>(browser: &'b mut Browser, offer: &Form) -> &'b Page {
    browser.submit(offer, Some(PUBLISH_LINK))
}

/// GIVEN the owner has spent this hour's GitHub check budget by pressing
/// Verify until the app refuses (at most `VERIFY_ATTEMPTS_PER_HOUR` + 2 tries).
pub fn uses_up_the_hours_verify_attempts(browser: &mut Browser, persona: Persona) {
    let spent = (0..22)
        .any(|_| when_verifies_github(browser, persona.github_login()).shows(TOO_MANY_ATTEMPTS));
    assert!(
        spent,
        "the verify budget never ran out:\n{}",
        browser.page.text()
    );
}

/// How many requests GitHub has served the app so far.
pub fn github_requests(world: &ReviewWorld) -> usize {
    world.github.requests().len()
}

/// THEN no link confirmation is offered: the request kept no link plan the
/// owner could confirm (a plan is reachable only through a confirm form; the
/// app's private store is never read by a step).
pub fn then_no_link_confirmation_is_offered(page: &Page) {
    assert!(
        !page.offers(CONFIRM_LINK),
        "no link plan may be offered for confirmation:\n{}",
        page.text()
    );
}

/// WHEN the owner confirms the link preview.
pub fn when_confirms_link(browser: &mut Browser) -> &Page {
    browser.press(CONFIRM_LINK)
}

/// GIVEN `persona` published their verified link through the app (chains
/// `given_github_verified` + preview + confirm).
pub fn given_link_published(world: &ReviewWorld, persona: Persona) -> Browser {
    let mut browser = given_github_verified(world, persona);
    when_opens_link_preview(&mut browser);
    when_confirms_link(&mut browser);
    then_sees_record_address(
        &browser.page,
        &then_pds_holds_self_attested_link(world, persona),
    );
    browser
}

/// THEN the page shows the exact address of `record`
/// (`at://<repo DID>/org.openlore.claim/<record key>`), not just any `at://`.
pub fn then_sees_record_address(page: &Page, record: &StoredRecord) {
    let address = format!("at://{}/{}/{}", record.repo, record.collection, record.rkey);
    assert!(
        page.shows(&address),
        "the page must show the record address {address}:\n{}",
        page.text()
    );
}

/// WHEN the owner presses "Retract this link" on the GitHub step.
pub fn when_opens_link_retraction(browser: &mut Browser) -> &Page {
    browser.open("/github");
    browser.press("Retract this link")
}

/// The `sameAs` records in `persona`'s repo.
pub fn links_in_pds(world: &ReviewWorld, persona: Persona) -> Vec<StoredRecord> {
    claims_in_pds(world, persona)
        .into_iter()
        .filter(|r| r.value["predicate"] == SAME_AS)
        .collect()
}

/// How many times GitHub served the profile of `persona` so far.
pub fn profile_reads_of(world: &ReviewWorld, persona: Persona) -> usize {
    world
        .github
        .requests()
        .iter()
        .filter(|r| r.is_profile_read_of(persona.github_login()))
        .count()
}

/// THEN the preview shows the exact link record and what publishing means.
pub fn then_sees_link_preview(page: &Page, persona: Persona) {
    for phrase in [
        format!("{} {SAME_AS} {}", persona.did(), github_subject(persona)),
        format!("https://github.com/{}", persona.github_login()),
        github_id_uri(persona),
        "public".to_string(),
        "withdraws it but does not erase it".to_string(),
    ] {
        assert!(
            page.shows(&phrase),
            "the link preview must show {phrase:?}:\n{}",
            page.text()
        );
    }
}

/// THEN `persona`'s PDS holds exactly one self-attested link to their GitHub
/// account in the ADR-084 §2 shape, keyed by its recomputed CID.
pub fn then_pds_holds_self_attested_link(world: &ReviewWorld, persona: Persona) -> StoredRecord {
    let links = links_in_pds(world, persona);
    assert_eq!(
        links.len(),
        1,
        "exactly one link record; repo holds: {:#?}",
        claims_in_pds(world, persona)
    );
    let record = links[0].clone();
    let v = &record.value;
    assert_eq!(v["$type"], CLAIM_COLLECTION);
    assert_eq!(
        v["subject"],
        persona.did(),
        "subject is the author's bare DID"
    );
    assert_eq!(v["object"], github_subject(persona).as_str());
    assert_eq!(
        v["evidence"],
        serde_json::json!([
            format!("https://github.com/{}", persona.github_login()),
            github_id_uri(persona)
        ]),
        "evidence is exactly [profile URL, id URI]"
    );
    assert_eq!(v["confidence"], 10000, "an identity link is 10000 bp");
    assert_eq!(v["author"], persona.did(), "self-attested: bare DID author");
    assert!(v.get("signature").is_none(), "no app-level signature");
    assert_eq!(
        recomputed_cid(v),
        record.rkey,
        "record key == recomputed CID"
    );
    record
}

/// The exact shape of a self-attested link as the app writes it (for the
/// reader suites, which exercise OpenLore's read path without the app).
pub fn self_attested_link_value(persona: Persona) -> serde_json::Value {
    serde_json::json!({
        "$type": CLAIM_COLLECTION,
        "subject": persona.did(),
        "predicate": SAME_AS,
        "object": github_subject(persona),
        "evidence": [
            format!("https://github.com/{}", persona.github_login()),
            github_id_uri(persona),
        ],
        "confidence": 10000,
        "author": persona.did(),
        "composedAt": "2026-10-14T15:02:11Z",
    })
}
