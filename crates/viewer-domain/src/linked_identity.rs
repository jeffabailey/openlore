//! The linked-identities region (github-did-identity-link slice 4; D-VL-1..8).
//!
//! A page about one person shows that person's linked identities exactly as
//! `openlore identity list` prints them (D-VL-5): the same words, character for
//! character, with the DID and the account as links and the 30-day re-check
//! command as a `<code>` element. The lines come from this reader's last local
//! checks; the viewer never asks GitHub (D-VL-8).
//!
//! PURE: the effect shell reads the links and the clock and hands this module a
//! [`LinkedIdentities`] value; nothing here reads a clock or a store.

use chrono::{DateTime, Utc};
use claim_domain::identity_link::{
    check_age_days, links_of_person, suggests_recheck, LinkStanding, LinkedIdentity, ListedLink,
    PersonKey,
};

use super::{encode_query_component, html, Markup, PROJECT_URL, SCORE_URL};

/// The region heading (viewer-only wording, D-VL-5 open question 3).
pub const LINKED_IDENTITIES_HEADING: &str = "Linked identities";

/// The one legend line beneath the lines.
pub const LINKED_IDENTITIES_LEGEND: &str =
    "As you last checked them. The viewer never asks GitHub; re-check with openlore identity verify.";

/// What the region says, and all it says, when the link reads failed (D-VL-7).
pub const LINKED_IDENTITIES_UNREADABLE: &str =
    "Linked identities could not be read from your local store.";

/// The `id` of the heading that labels the region (`aria-labelledby`).
pub const LINKED_IDENTITIES_HEADING_ID: &str = "linked-identities-heading";

/// What a page shows about one person's linked identities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkedIdentities {
    /// No region at all: the person has no link, or the page is about no one.
    None,
    /// At least one line, as of `now`.
    Lines {
        listed: Vec<ListedLink>,
        now: DateTime<Utc>,
    },
    /// The link reads failed: the region shows the notice and no line.
    Unreadable,
}

impl LinkedIdentities {
    /// The region for the links a read listed as of `now`: no link, no region.
    pub fn listed(listed: Vec<ListedLink>, now: DateTime<Utc>) -> Self {
        if listed.is_empty() {
            LinkedIdentities::None
        } else {
            LinkedIdentities::Lines { listed, now }
        }
    }

    /// The lines that belong to `person` (U-6, D-VL-6), selected AFTER every
    /// standing was decided (C-VL-3): a selection never changes a line. No
    /// line left, no region; an unreadable region stays unreadable.
    pub fn of_person(self, person: &PersonKey) -> Self {
        match self {
            LinkedIdentities::Lines { listed, now } => {
                LinkedIdentities::listed(links_of_person(&listed, person), now)
            }
            other => other,
        }
    }
}

/// The GitHub person a `/project` subject names (D-VL-6): `github:<login>`,
/// the prefix in any case, keyed by the last-seen login (the page carries no
/// account id; `PersonKey::of_person` compares it case-insensitively). Any
/// other subject names no person, so its page shows no region.
pub fn github_person(subject: &str) -> Option<PersonKey> {
    subject
        .get(..GITHUB_SUBJECT_PREFIX.len())
        .filter(|prefix| prefix.eq_ignore_ascii_case(GITHUB_SUBJECT_PREFIX))
        .map(|_| PersonKey::of_person(subject, []))
}

/// The subject prefix of a GitHub person page.
const GITHUB_SUBJECT_PREFIX: &str = "github:";

/// Render the region: a `<section>` labelled by its heading, the lines as a
/// list and the legend beneath; the notice alone when unreadable; nothing for
/// [`LinkedIdentities::None`] (no empty heading).
pub fn render_linked_identities_region(linked: &LinkedIdentities) -> Markup {
    match linked {
        LinkedIdentities::None => html! {},
        LinkedIdentities::Lines { listed, now } => linked_identities_section(html! {
            ul {
                @for link in listed {
                    (render_listed_link(link, *now))
                }
            }
            p { (LINKED_IDENTITIES_LEGEND) }
        }),
        LinkedIdentities::Unreadable => linked_identities_section(html! {
            p { (LINKED_IDENTITIES_UNREADABLE) }
        }),
    }
}

/// The labelled section around a region body.
fn linked_identities_section(body: Markup) -> Markup {
    html! {
        section aria-labelledby=(LINKED_IDENTITIES_HEADING_ID) {
            h2 id=(LINKED_IDENTITIES_HEADING_ID) { (LINKED_IDENTITIES_HEADING) }
            (body)
        }
    }
}

/// One line: the standing, the re-check hint (U-8) and the record IDs (U-9).
fn render_listed_link(listed: &ListedLink, now: DateTime<Utc>) -> Markup {
    html! {
        li {
            (render_standing(&listed.identity, now))
            @if suggests_recheck(&listed.identity.standing, now) {
                "; re-check with " code { "openlore identity verify" }
            }
            (record_ids_text(&listed.record_ids))
        }
    }
}

/// The standing sentence, with the DID and the account as links.
fn render_standing(identity: &LinkedIdentity, now: DateTime<Utc>) -> Markup {
    let did = did_link(&identity.link.did);
    let account = account_link(&identity.link.login);
    match &identity.standing {
        LinkStanding::Associated { checked_at } => html! {
            (did) " is " (account) " — verified " (check_age(*checked_at, now))
        },
        LinkStanding::NotChecked => html! {
            (did) " claims " (account) " — not checked"
        },
        LinkStanding::ProofGone {
            verdict,
            checked_at,
        } => html! {
            (did) " claims " (account) " — proof gone (" (verdict.token()) "), checked "
            (check_age(*checked_at, now))
        },
        LinkStanding::Contested => html! {
            (account) " is contested — " (did) " claims it"
        },
    }
}

/// The DID, linked to its contributor page.
fn did_link(did: &str) -> Markup {
    html! {
        a href=(format!("{SCORE_URL}?contributor={}", encode_query_component(did))) { (did) }
    }
}

/// The account, linked to its GitHub person page.
fn account_link(login: &str) -> Markup {
    let account = format!("github:{login}");
    html! {
        a href=(format!("{PROJECT_URL}?subject={}", encode_query_component(&account))) { (account) }
    }
}

/// How long ago a check ran, in whole days, with the instant for assistive tools.
fn check_age(checked_at: DateTime<Utc>, now: DateTime<Utc>) -> Markup {
    let age = match check_age_days(checked_at, now) {
        0 => "today".to_string(),
        1 => "1 day ago".to_string(),
        days => format!("{days} days ago"),
    };
    html! { time datetime=(checked_at.to_rfc3339()) { (age) } }
}

/// The record IDs a line names: ` (record <id>)` or ` (records <id>, <id>)`.
fn record_ids_text(record_ids: &[String]) -> String {
    match record_ids {
        [] => String::new(),
        [only] => format!(" (record {only})"),
        many => format!(" (records {})", many.join(", ")),
    }
}
