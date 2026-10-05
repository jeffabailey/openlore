//! `GET /@{handle|did}`: the public profile (US-BRA-007). Resolves the
//! identity, reads the owner's claims live from their own PDS (no cache),
//! computes the record origin here at the root (ADR-071 §4), and renders
//! the pure projection. The handler is given only the identity and repo
//! listing ports: it holds no review-store port, so private state
//! (pending, declined, edited drafts) is structurally out of its reach.

use hyper::StatusCode;
use ports::claim_domain::RecordOrigin;
use ports::{IdentityLookupError, IdentityLookupPort, RepoListingPort, ResolvedIdentity};
use review_domain::views::{
    self, profile_subject, published_claims, ProfileContent, ProfileSubject, ProfileView,
};

use crate::http::Reply;

/// The only ports the profile reads through.
pub(crate) struct ProfilePorts<'a> {
    pub(crate) identity: &'a dyn IdentityLookupPort,
    pub(crate) repos: &'a dyn RepoListingPort,
}

/// The profile named by `path` (`/@<segment>`), as seen by `viewer_did`.
pub(crate) async fn profile(
    ports: ProfilePorts<'_>,
    path: &str,
    viewer_did: Option<&str>,
) -> Reply {
    let segment = path.strip_prefix("/@").unwrap_or_default();
    let Some(subject) = profile_subject(segment) else {
        return not_found();
    };
    let resolved = match &subject {
        ProfileSubject::Handle(handle) => ports.identity.resolve_identity(handle).await,
        ProfileSubject::Did(did) => ports.identity.resolve_did(did).await,
    };
    match resolved {
        Ok(identity) => {
            let content = read_published(ports.repos, &identity).await;
            let status = match content {
                ProfileContent::Claims(_) => StatusCode::OK,
                _ => StatusCode::SERVICE_UNAVAILABLE,
            };
            page(
                status,
                &identity.verified_handle,
                viewer_did == Some(identity.did.as_str()),
                &content,
            )
        }
        Err(IdentityLookupError::NotFound) => not_found(),
        Err(IdentityLookupError::Unavailable { .. }) => page(
            StatusCode::SERVICE_UNAVAILABLE,
            segment,
            false,
            &ProfileContent::DirectoryUnreachable,
        ),
    }
}

/// The owner's published claims, read live from the PDS their DID document
/// names. The origin is computed by comparing where the listing was read
/// from with that freshly resolved PDS; it is never assumed.
pub(crate) async fn read_published(
    repos: &dyn RepoListingPort,
    identity: &ResolvedIdentity,
) -> ProfileContent {
    match repos
        .list_repo_claims(&identity.pds_endpoint, &identity.did)
        .await
    {
        Ok(listing) => {
            let origin = RecordOrigin::of(&listing.fetched_from, &identity.pds_endpoint);
            ProfileContent::Claims(published_claims(&identity.did, &listing.records, origin))
        }
        Err(_) => ProfileContent::PdsUnreachable,
    }
}

fn page(
    status: StatusCode,
    handle: &str,
    viewer_is_owner: bool,
    content: &ProfileContent,
) -> Reply {
    Reply::page(
        status,
        views::profile_page(&ProfileView {
            handle,
            viewer_is_owner,
            content,
        }),
    )
}

fn not_found() -> Reply {
    Reply::page(StatusCode::NOT_FOUND, views::profile_not_found_page())
}
