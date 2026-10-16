//! GitHub ownership proof (ADR-076 §4–5, DWD-10). PURE.
//!
//! A GitHub account is proven to belong to the signed-in Bluesky account
//! when its public bio holds the signed-in DID as an exact token. The only
//! way to obtain a [`VerifiedOwnership`] — the capability a scan needs — is
//! [`prove_ownership`] returning it for a passing check.

// The bio matcher lives in `claim_domain::identity_link` (ADR-085 §1, UC-4):
// one proof rule for the review app, the CLI and every reader.
pub use ports::claim_domain::identity_link::did_tokens;

/// What a bio says about the signed-in DID.
pub use ports::claim_domain::identity_link::BioVerdict as OwnershipVerdict;

/// The ownership verdict of a bio against the signed-in DID: exact,
/// case-sensitive token equality; several DIDs may appear, only the
/// signed-in one counts. Delegates to the shared identity-link proof.
pub fn ownership_verdict(bio: Option<&str>, session_did: &str) -> OwnershipVerdict {
    ports::claim_domain::identity_link::bio_verdict(bio, session_did)
}

/// A GitHub account's public profile, as read just now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GithubAccount<'a> {
    pub login: &'a str,
    /// The stable numeric GitHub user id.
    pub user_id: u64,
    pub bio: Option<&'a str>,
}

/// Why ownership is not (or no longer) proven. Each is explained to the
/// person and can be retried.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipRefusal {
    DidMissing,
    DifferentDid(String),
    NoBio,
    /// The login now names a different GitHub account than the one verified
    /// (renamed or re-registered).
    IdentityChanged,
    AccountNotFound,
    RateLimited,
    GithubUnavailable,
    /// The GitHub account is already verified for another Bluesky account.
    LinkedToAnotherAccount,
    /// Too many verification attempts in the last hour.
    TooManyAttempts,
}

impl OwnershipRefusal {
    /// `true` when the refusal shows the account is not (or no longer)
    /// proven: the link becomes unverified. Rate limits, outages and attempt
    /// limits prove nothing either way.
    pub fn disproves_ownership(&self) -> bool {
        matches!(
            self,
            Self::DidMissing
                | Self::DifferentDid(_)
                | Self::NoBio
                | Self::IdentityChanged
                | Self::AccountNotFound
        )
    }

    /// The operator-facing name (logs and `github_links.last_verdict`); never
    /// carries bio contents or a DID.
    pub fn label(&self) -> &'static str {
        match self {
            Self::DidMissing => "did_missing",
            Self::DifferentDid(_) => "different_did",
            Self::NoBio => "no_bio",
            Self::IdentityChanged => "identity_changed",
            Self::AccountNotFound => "account_not_found",
            Self::RateLimited => "rate_limited",
            Self::GithubUnavailable => "github_unavailable",
            Self::LinkedToAnotherAccount => "linked_to_another_account",
            Self::TooManyAttempts => "too_many_attempts",
        }
    }
}

/// A GitHub username: 1–39 ASCII letters, digits or hyphens, not starting
/// with a hyphen. Anything else names no account (and never reaches a URL).
/// The one login rule, shared with the CLI (`claim_domain::identity_link`).
pub use ports::claim_domain::identity_link::is_github_login;

/// The capability a scan needs: this DID proved this GitHub account is
/// theirs. Only [`prove_ownership`] mints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedOwnership {
    owner_did: String,
    github_login: String,
    github_user_id: u64,
}

impl VerifiedOwnership {
    pub fn owner_did(&self) -> &str {
        &self.owner_did
    }

    pub fn github_login(&self) -> &str {
        &self.github_login
    }

    pub fn github_user_id(&self) -> u64 {
        self.github_user_id
    }
}

/// Prove that `account` belongs to `session_did`. `linked_user_id` is the
/// numeric id recorded when the link was verified (re-verification before a
/// scan); a different id means the login changed hands. A profile naming no
/// account (account number 0, or no GitHub login) proves nothing either way:
/// it is refused as an unreadable GitHub answer (fail closed, ADR-085 §4).
pub fn prove_ownership(
    session_did: &str,
    account: &GithubAccount<'_>,
    linked_user_id: Option<u64>,
) -> Result<VerifiedOwnership, OwnershipRefusal> {
    if account.user_id == 0 || !is_github_login(account.login) {
        return Err(OwnershipRefusal::GithubUnavailable);
    }
    if linked_user_id.is_some_and(|id| id != account.user_id) {
        return Err(OwnershipRefusal::IdentityChanged);
    }
    match ownership_verdict(account.bio, session_did) {
        OwnershipVerdict::Verified => Ok(VerifiedOwnership {
            owner_did: session_did.to_string(),
            github_login: account.login.to_string(),
            github_user_id: account.user_id,
        }),
        OwnershipVerdict::DidMissing => Err(OwnershipRefusal::DidMissing),
        OwnershipVerdict::DifferentDid(found) => Err(OwnershipRefusal::DifferentDid(found)),
        OwnershipVerdict::NoBio => Err(OwnershipRefusal::NoBio),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn did() -> impl Strategy<Value = String> {
        "[a-z2-7]{24}".prop_map(|s| format!("did:plc:{s}"))
    }

    fn words() -> impl Strategy<Value = String> {
        "[A-Za-z ,!?()]{0,20}"
    }

    proptest! {
        /// Universe: (DID, surrounding words, one token-class character).
        /// Extending the DID on either side with a token character, or
        /// changing its case, never verifies; delimiting it with spaces or
        /// trailing sentence punctuation always does.
        #[test]
        fn only_a_delimited_byte_equal_did_token_verifies(
            did in did(), pre in words(), post in words(),
            extra in "[A-Za-z0-9._%-]", punct in "[.,;:!?)\\]}>]{1,3}",
        ) {
            let verdict = |token: &str| ownership_verdict(Some(&format!("{pre} {token} {post}")), &did);
            prop_assert_eq!(verdict(&did), OwnershipVerdict::Verified);
            prop_assert_eq!(verdict(&format!("({did}{punct}")), OwnershipVerdict::Verified);
            prop_assert_ne!(verdict(&format!("{did}{extra}0")), OwnershipVerdict::Verified);
            prop_assert_ne!(verdict(&format!("{extra}{did}")), OwnershipVerdict::Verified);
            prop_assert_ne!(verdict(&did.to_uppercase()), OwnershipVerdict::Verified);
            prop_assert_ne!(verdict(&did[..did.len() - 1]), OwnershipVerdict::Verified);
        }

        /// Universe: (session DID, bio, profile id, recorded id). The
        /// capability is minted exactly when the bio verifies and the id has
        /// not changed, and it names the session DID and that account.
        #[test]
        fn the_capability_is_minted_only_for_a_verified_unchanged_account(
            did in did(), other in did(), words in words(),
            holds in prop_oneof![Just(0u8), Just(1), Just(2)],
            user_id in 1u64..1_000, linked in proptest::option::of(1u64..1_000),
        ) {
            let bio = match holds {
                0 => format!("{words} {did}"),
                1 => format!("{words} {other}"),
                _ => words.clone(),
            };
            let account = GithubAccount { login: "priyaraman", user_id, bio: Some(&bio) };
            let proven = prove_ownership(&did, &account, linked);
            let expected_pass = ownership_verdict(Some(&bio), &did) == OwnershipVerdict::Verified
                && linked.is_none_or(|id| id == user_id);
            prop_assert_eq!(proven.is_ok(), expected_pass);
            if let Ok(capability) = proven {
                prop_assert_eq!(capability.owner_did(), did.as_str());
                prop_assert_eq!(capability.github_login(), "priyaraman");
                prop_assert_eq!(capability.github_user_id(), user_id);
            } else if linked.is_some_and(|id| id != user_id) {
                prop_assert_eq!(proven, Err(OwnershipRefusal::IdentityChanged));
            } else if let Err(refusal) = proven {
                prop_assert!(refusal.disproves_ownership());
            }
        }

        /// Universe: (session DID, bio naming it, recorded id, malformed
        /// account). An account numbered 0 or without a GitHub login is
        /// never proven — even when its bio names the session DID — and the
        /// refusal disproves nothing (the link is not marked unverified).
        #[test]
        fn an_account_github_did_not_name_is_never_proven(
            did in did(), words in words(),
            linked in proptest::option::of(0u64..1_000),
            malformed in prop_oneof![
                Just(("priyaraman", 0u64)),
                Just(("", 4_210_001)),
                Just(("-priya", 4_210_001)),
                Just(("priya/raman", 4_210_001)),
            ],
        ) {
            let bio = format!("{words} {did}");
            let (login, user_id) = malformed;
            let account = GithubAccount { login, user_id, bio: Some(&bio) };
            let refused = prove_ownership(&did, &account, linked);
            prop_assert_eq!(&refused, &Err(OwnershipRefusal::GithubUnavailable));
            prop_assert!(!refused.unwrap_err().disproves_ownership());
        }

        /// Universe: typed strings. Only GitHub-username-shaped text is
        /// accepted, so a typed login can never carry a path or query.
        #[test]
        fn only_github_username_shaped_text_is_a_login(typed in "[A-Za-z0-9/?#. -]{0,45}") {
            let shaped = (1..=39).contains(&typed.len())
                && !typed.starts_with('-')
                && typed.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
            prop_assert_eq!(is_github_login(&typed), shaped);
            if is_github_login(&typed) {
                prop_assert!(!typed.contains(['/', '?', '#', '.', ' ']));
            }
        }
    }
}
