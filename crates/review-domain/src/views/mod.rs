//! Page renderers (maud). Each page is chrome + body, as in ADR-032.
//! Copy constants here are the single source of truth for the app's text.
//!
//! One submodule per surface; every page, view and copy constant is
//! re-exported here, so callers name them `views::…`. The fragments several
//! pages share (chrome, anti-forgery field, plan forms, headlines,
//! confidence text) live in this module.

use maud::{html, Markup, DOCTYPE};
use ports::SuggestionKey;

use crate::lifecycle::BASIS_POINTS;

mod github;
mod landing;
mod profile;
mod publish;
mod queue;
mod retract;
mod settings;
mod share;

pub use github::*;
pub use landing::*;
pub use profile::*;
pub use publish::*;
pub use queue::*;
pub use retract::*;
pub use settings::*;
pub use share::*;

pub use crate::published::{
    live_published_claim, profile_subject, published_claims, ProfileSubject, PublishedClaim,
};

/// The way back from a form to where the person was.
pub const CANCEL_LABEL: &str = "Cancel";

/// The link text back to the signed-in queue.
const BACK_TO_QUEUE: &str = "Back to your review queue";

/// The link text back to the owner's own profile.
const BACK_TO_PROFILE: &str = "Back to your profile";

/// The shared page chrome. `body` is the page-specific fragment.
fn page(title: &str, body: Markup) -> String {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " · " (APP_NAME) }
            }
            body {
                header { strong { (APP_NAME) } }
                main { (body) }
            }
        }
    }
    .into_string()
}

/// The hidden anti-forgery field every state-changing form carries.
fn csrf_field(csrf_token: &str) -> Markup {
    html! { input type="hidden" name="csrf" value=(csrf_token); }
}

/// A one-button form that confirms (or retries) the kept plan `plan_id`.
fn plan_form(action: &str, plan_id: &str, csrf_token: &str, label: &str) -> Markup {
    html! {
        form method="post" action=(action) {
            (csrf_field(csrf_token))
            input type="hidden" name="plan" value=(plan_id);
            button type="submit" { (label) }
        }
    }
}

/// The paragraph linking back to the signed-in queue.
fn back_to_queue() -> Markup {
    html! { p { a href="/review" { (BACK_TO_QUEUE) } } }
}

/// The paragraph linking back to the owner's profile at `profile_path`.
fn back_to_profile(profile_path: &str) -> Markup {
    html! { p { a href=(profile_path) { (BACK_TO_PROFILE) } } }
}

/// `priyaraman/tidepool` of `github:priyaraman/tidepool`.
fn repo_path(subject: &str) -> &str {
    subject.strip_prefix("github:").unwrap_or(subject)
}

/// `dependency-pinning` of `org.openlore.philosophy.dependency-pinning`.
fn philosophy_slug(object: &str) -> &str {
    object.rsplit('.').next().unwrap_or(object)
}

/// "<owner>/<repo> embodies <slug>", linking the repo.
fn claim_headline(subject: &str, object: &str) -> Markup {
    let repo = repo_path(subject);
    html! {
        a href=(format!("https://github.com/{repo}/")) { (repo) }
        " embodies " (philosophy_slug(object))
    }
}

/// A suggestion's headline: "<repo> embodies <philosophy>".
fn suggestion_headline(key: &SuggestionKey) -> Markup {
    claim_headline(&key.subject, &key.object)
}

/// `0.25` of 2500 basis points.
pub fn confidence_text(basis_points: u16) -> String {
    format!(
        "{}.{:02}",
        basis_points / BASIS_POINTS,
        (basis_points % BASIS_POINTS) / 100
    )
}

/// The display-only bucket label of a confidence (WD-10).
pub fn bucket_label(basis_points: u16) -> &'static str {
    use ports::claim_domain::{confidence_bucket, ConfidenceBucket};
    match confidence_bucket(f64::from(basis_points) / f64::from(BASIS_POINTS)) {
        ConfidenceBucket::Speculative => "speculative",
        ConfidenceBucket::Weighted => "weighted",
        ConfidenceBucket::WellEvidenced => "well-evidenced",
        ConfidenceBucket::Triangulated => "triangulated",
    }
}

/// "Confidence as shown": `0.25 (speculative)`.
fn confidence_with_bucket(basis_points: u16) -> Markup {
    html! { (confidence_text(basis_points)) " (" (bucket_label(basis_points)) ")" }
}

/// A page that does not exist for this person (never says whose it is).
pub fn not_found_page() -> String {
    page(
        "Not found",
        html! {
            h1 { "Not found" }
            (back_to_queue())
        },
    )
}

/// The page script: copy a `data-copy` value to the clipboard.
pub const COPY_SCRIPT: &str = "document.querySelectorAll('[data-copy]').forEach(function (el) {\n\
  el.addEventListener('click', function (event) {\n\
    event.preventDefault();\n\
    navigator.clipboard.writeText(el.getAttribute('data-copy'));\n\
  });\n\
});\n";

/// The queue's keyboard triage (A approve, E edit, N not me, J/K move).
pub const TRIAGE_SCRIPT: &str = "(function () {\n\
  var cards = function () { return Array.prototype.slice.call(document.querySelectorAll('article')); };\n\
  var current = 0;\n\
  var focus = function (i) { var c = cards(); if (!c.length) return; current = Math.max(0, Math.min(i, c.length - 1)); var b = c[current].querySelector('button'); if (b) b.focus(); };\n\
  document.addEventListener('keydown', function (event) {\n\
    if (event.target && event.target.tagName === 'INPUT') return;\n\
    var key = event.key.toLowerCase();\n\
    if (key === 'j') focus(current + 1);\n\
    else if (key === 'k') focus(current - 1);\n\
    else { var c = cards()[current]; var b = c && c.querySelector('[data-key=\"' + key + '\"]'); if (b) b.click(); }\n\
  });\n\
})();\n";
