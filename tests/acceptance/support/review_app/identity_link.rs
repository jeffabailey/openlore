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
    browser.press("Publish this link")
}

/// WHEN the owner confirms the link preview.
pub fn when_confirms_link(browser: &mut Browser) -> &Page {
    browser.press("Publish to my repo")
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
