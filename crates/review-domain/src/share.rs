//! The opt-in share post (US-BRA-008, I-BRA-6): PURE composition of the
//! Bluesky post that announces the owner's published claims.
//!
//! - The draft names only the philosophies of LIVE published claims (the
//!   caller passes `views::published_claims`, already minus retractions).
//! - A post is at most [`MAX_POST_GRAPHEMES`] grapheme clusters (Bluesky's
//!   limit); a longer one is refused before anything is sent.
//! - The link to the owner's profile is an `app.bsky.richtext.facet#link`
//!   whose `byteStart`/`byteEnd` are UTF-8 BYTE offsets (SPIKE-1), never
//!   char or grapheme counts.

use ports::{PlanKind, StoredPublishPlan, SuggestionKey};
use serde_json::{json, Value};
use unicode_segmentation::UnicodeSegmentation;

use crate::plans::PlanError;
use crate::views::PublishedClaim;

/// The ATProto collection a share post is created in.
pub const POST_COLLECTION: &str = "app.bsky.feed.post";

/// Bluesky's post length limit, in grapheme clusters.
pub const MAX_POST_GRAPHEMES: usize = 300;

const LINK_FEATURE: &str = "app.bsky.richtext.facet#link";
const DRAFT_LEAD: &str = "How I build, in claims I've published on OpenLore";

/// The link facet: the byte range of the profile URL inside the text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkFacet {
    pub byte_start: usize,
    pub byte_end: usize,
    pub uri: String,
}

/// A post ready to be created: its text and the facet linking the profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharePost {
    text: String,
    link: LinkFacet,
}

/// Why a post cannot be sent as written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShareRefusal {
    /// The owner has no live published claim: there is nothing to share.
    NothingPublished,
    /// The text is blank.
    Empty,
    /// The post would be longer than Bluesky allows.
    TooLong { graphemes: usize },
}

impl ShareRefusal {
    /// What the owner is told (nothing was posted).
    pub fn message(self) -> String {
        match self {
            Self::NothingPublished => {
                "There is nothing to share yet: publish a claim first.".to_string()
            }
            Self::Empty => "Write something to post. Nothing was posted.".to_string(),
            Self::TooLong { graphemes } => format!(
                "Bluesky posts can be at most {MAX_POST_GRAPHEMES} characters; this one has \
                 {graphemes} (with the link to your profile). Shorten it; nothing was posted."
            ),
        }
    }
}

impl SharePost {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn link(&self) -> &LinkFacet {
        &self.link
    }

    /// The exact `app.bsky.feed.post` record, created at `created_at`
    /// (RFC 3339 UTC).
    pub fn record(&self, created_at: &str) -> Value {
        json!({
            "$type": POST_COLLECTION,
            "text": self.text,
            "createdAt": created_at,
            "facets": [{
                "index": {"byteStart": self.link.byte_start, "byteEnd": self.link.byte_end},
                "features": [{"$type": LINK_FEATURE, "uri": self.link.uri}],
            }],
        })
    }
}

/// The post for the owner's `text`, linking `profile_url`: the link is
/// appended when the text does not carry it, and the whole post must fit
/// Bluesky's limit. Pure.
pub fn compose_post(text: &str, profile_url: &str) -> Result<SharePost, ShareRefusal> {
    let written = text.trim();
    if written.is_empty() {
        return Err(ShareRefusal::Empty);
    }
    let full = if written.contains(profile_url) {
        written.to_string()
    } else {
        format!("{written} {profile_url}")
    };
    let graphemes = full.graphemes(true).count();
    if graphemes > MAX_POST_GRAPHEMES {
        return Err(ShareRefusal::TooLong { graphemes });
    }
    let byte_start = full.find(profile_url).unwrap_or_default();
    Ok(SharePost {
        link: LinkFacet {
            byte_start,
            byte_end: byte_start + profile_url.len(),
            uri: profile_url.to_string(),
        },
        text: full,
    })
}

/// What the share preview offers: the editable draft and the profile link
/// every post carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharePostPlan {
    draft: SharePost,
}

impl SharePostPlan {
    pub fn draft(&self) -> &SharePost {
        &self.draft
    }

    pub fn profile_url(&self) -> &str {
        &self.draft.link.uri
    }

    /// The plan as kept between preview and confirm under `plan_id`.
    pub fn stored(&self, owner_did: &str, plan_id: &str) -> StoredPublishPlan {
        StoredPublishPlan {
            plan_id: plan_id.to_string(),
            kind: PlanKind::Share,
            key: share_key(owner_did, self.profile_url()),
            record_json: json!({"$type": POST_COLLECTION, "text": self.draft.text}).to_string(),
        }
    }
}

fn share_key(owner_did: &str, profile_url: &str) -> SuggestionKey {
    SuggestionKey {
        subject: owner_did.to_string(),
        predicate: POST_COLLECTION.to_string(),
        object: profile_url.to_string(),
    }
}

/// The share plan for the owner's live `published` claims, linking
/// `profile_url` — offered only when at least one claim is live (AC-008.5).
/// Pure; nothing is posted.
pub fn share_post_plan(
    profile_url: &str,
    published: &[PublishedClaim],
) -> Result<SharePostPlan, ShareRefusal> {
    if published.is_empty() {
        return Err(ShareRefusal::NothingPublished);
    }
    share_draft(profile_url, published)
}

/// The draft for `published`, linking `profile_url`: it names each
/// distinct philosophy of `published` and nothing else, as many as fit
/// Bluesky's limit. Pure.
pub fn share_draft(
    profile_url: &str,
    published: &[PublishedClaim],
) -> Result<SharePostPlan, ShareRefusal> {
    let philosophies = distinct_philosophies(published);
    let shortest = compose_post(&draft_text(&philosophies, 0), profile_url);
    (1..=philosophies.len())
        .rev()
        .map(|named| compose_post(&draft_text(&philosophies, named), profile_url))
        .find_map(Result::ok)
        .map_or(shortest, Ok)
        .map(|draft| SharePostPlan { draft })
}

/// Restore a stored share plan for `owner_did`: only a share plan of that
/// owner is ever executed (never a publish plan, never someone else's).
pub fn restore_share_plan(
    owner_did: &str,
    stored: &StoredPublishPlan,
) -> Result<SharePostPlan, PlanError> {
    let record: Value =
        serde_json::from_str(&stored.record_json).map_err(|_| PlanError::NotTheOwnersRecord)?;
    let draft = record["text"].as_str().unwrap_or_default();
    let is_share = stored.kind == PlanKind::Share
        && record["$type"] == POST_COLLECTION
        && stored.key == share_key(owner_did, &stored.key.object);
    match compose_post(draft, &stored.key.object) {
        Ok(draft) if is_share => Ok(SharePostPlan { draft }),
        _ => Err(PlanError::NotTheOwnersRecord),
    }
}

fn philosophy_slug(object: &str) -> &str {
    object.rsplit('.').next().unwrap_or(object)
}

fn distinct_philosophies(published: &[PublishedClaim]) -> Vec<&str> {
    published
        .iter()
        .map(|claim| philosophy_slug(&claim.object))
        .fold(Vec::new(), |mut seen, slug| {
            if !seen.contains(&slug) {
                seen.push(slug);
            }
            seen
        })
}

fn draft_text(philosophies: &[&str], named: usize) -> String {
    match (named, philosophies.len() - named) {
        (0, _) => format!("{DRAFT_LEAD}."),
        (_, 0) => format!("{DRAFT_LEAD}: {}.", philosophies.join(", ")),
        (_, more) => format!(
            "{DRAFT_LEAD}: {} and {more} more.",
            philosophies[..named].join(", ")
        ),
    }
}

#[cfg(test)]
mod tests {
    //! Universe: (profile URL incl. non-ASCII handles, published claims,
    //! the owner's typed text incl. multi-byte graphemes). State-delta:
    //! composing touches nothing but its own value; the facet always slices
    //! exactly the link out of the UTF-8 text; length is counted in
    //! graphemes; only published philosophies are named.
    use super::*;
    use proptest::prelude::*;

    const SLUGS: [&str; 7] = [
        "dependency-pinning",
        "memory-safety",
        "test-driven",
        "semantic-versioning",
        "documentation-first",
        "accessibility",
        "reproducible-builds",
    ];

    fn arb_profile_url() -> impl Strategy<Value = String> {
        "[a-zé漢🦀]{1,20}".prop_map(|h| format!("https://app.openlore.example/@{h}.bsky.social"))
    }

    fn arb_published() -> impl Strategy<Value = Vec<PublishedClaim>> {
        prop::collection::vec(("[a-z]{1,6}", prop::sample::select(SLUGS.to_vec())), 0..12).prop_map(
            |claims| {
                claims
                    .into_iter()
                    .map(|(repo, slug)| PublishedClaim {
                        subject: format!("github:{repo}/{repo}"),
                        object: format!("org.openlore.philosophy.{slug}"),
                        confidence_bp: 2500,
                        rkey: format!("baf{repo}"),
                    })
                    .collect()
            },
        )
    }

    fn arb_text() -> impl Strategy<Value = String> {
        prop_oneof![
            "[a-zA-Z !é漢]{0,320}",
            (1usize..400).prop_map(|n| "é".repeat(n)),
            (1usize..400).prop_map(|n| "👩‍💻".repeat(n)),
        ]
    }

    fn slices_to_link(post: &SharePost) -> bool {
        post.text.get(post.link.byte_start..post.link.byte_end) == Some(post.link.uri.as_str())
    }

    proptest! {
        #[test]
        fn a_composed_post_fits_and_its_facet_slices_exactly_the_link(
            text in arb_text(), url in arb_profile_url()
        ) {
            let with_link = if text.trim().contains(&url) {
                text.trim().to_string()
            } else {
                format!("{} {url}", text.trim())
            };
            let graphemes = with_link.graphemes(true).count();
            match compose_post(&text, &url) {
                Ok(post) => {
                    prop_assert!(!text.trim().is_empty());
                    prop_assert!(post.text.graphemes(true).count() <= MAX_POST_GRAPHEMES);
                    prop_assert!(post.text.starts_with(text.trim()));
                    prop_assert!(slices_to_link(&post));
                    let record = post.record("2026-10-04T15:02:11Z");
                    prop_assert_eq!(&record["text"], &json!(post.text));
                    prop_assert_eq!(&record["facets"][0]["index"]["byteStart"], &json!(post.link.byte_start));
                    prop_assert_eq!(&record["facets"][0]["index"]["byteEnd"], &json!(post.link.byte_end));
                    prop_assert_eq!(&record["facets"][0]["features"][0]["uri"], &json!(url));
                }
                Err(ShareRefusal::Empty) => prop_assert!(text.trim().is_empty()),
                Err(ShareRefusal::TooLong { graphemes: counted }) => {
                    prop_assert_eq!(counted, graphemes);
                    prop_assert!(counted > MAX_POST_GRAPHEMES);
                }
                Err(ShareRefusal::NothingPublished) => prop_assert!(false, "compose never says that"),
            }
        }

        #[test]
        fn the_draft_names_only_published_philosophies_and_links_the_profile(
            published in arb_published(), url in arb_profile_url()
        ) {
            match share_post_plan(&url, &published) {
                Ok(plan) => {
                    let draft = plan.draft();
                    prop_assert!(slices_to_link(draft));
                    prop_assert_eq!(plan.profile_url(), url.as_str());
                    let named: Vec<&str> = published.iter().map(|c| philosophy_slug(&c.object)).collect();
                    for slug in SLUGS {
                        prop_assert_eq!(draft.text().contains(slug), named.contains(&slug), "{}", slug);
                    }
                    let restored = restore_share_plan("did:plc:owner", &plan.stored("did:plc:owner", "p1"));
                    prop_assert_eq!(restored, Ok(plan.clone()));
                    prop_assert_eq!(
                        restore_share_plan("did:plc:other", &plan.stored("did:plc:owner", "p1")),
                        Err(PlanError::NotTheOwnersRecord)
                    );
                }
                Err(refusal) => {
                    prop_assert_eq!(refusal, ShareRefusal::NothingPublished);
                    prop_assert!(published.is_empty());
                    prop_assert!(share_draft(&url, &published).is_ok(), "the bare draft still links");
                }
            }
        }
    }
}
