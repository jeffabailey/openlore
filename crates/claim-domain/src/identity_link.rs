//! `identity_link` — the GitHub ↔ DID identity link, decided PURELY
//! (ADR-084 wire contract, ADR-085 verification and reader semantics).
//!
//! This module is the single home of the link's rules: the bio proof (the
//! DID-token matcher hoisted verbatim from `review-domain::ownership`,
//! ADR-085 §1), the recorded check verdicts (ADR-085 §4) and the link
//! predicate readers select candidate rows with (Q-RM-4/Q-RM-5: the predicate
//! literal appears nowhere else).

/// The reserved predicate of an identity-link claim (ADR-084 §2).
const LINK_PREDICATE: &str = "sameAs";

/// The predicate readers select candidate link rows with. Selection is only
/// a pre-filter: what is a link is decided by the projection, never by the
/// predicate alone (Q-RM-5).
pub fn link_predicate() -> &'static str {
    LINK_PREDICATE
}

// =============================================================================
// Bio proof (ADR-076 §4, hoisted by ADR-085 §1)
// =============================================================================

/// Characters a bio token is made of: maximal runs of `[A-Za-z0-9._:%-]`.
fn is_token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '%' | '-')
}

/// Sentence punctuation stripped from the end of a token (`(did:…).`).
fn is_trailing_punctuation(c: char) -> bool {
    matches!(c, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '>')
}

/// `did:<lowercase method>:<non-empty rest>`.
fn is_did_shaped(token: &str) -> bool {
    let mut parts = token.splitn(3, ':');
    let scheme = parts.next();
    let method = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or_default();
    scheme == Some("did")
        && !method.is_empty()
        && method.chars().all(|c| c.is_ascii_lowercase())
        && !rest.is_empty()
}

/// The DID-shaped tokens of a bio, in order of appearance.
pub fn did_tokens(bio: &str) -> Vec<&str> {
    bio.split(|c: char| !is_token_char(c))
        .map(|run| run.trim_end_matches(is_trailing_punctuation))
        .filter(|token| is_did_shaped(token))
        .collect()
}

/// What a bio says about one DID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BioVerdict {
    /// The DID is a byte-equal token of the bio.
    Verified,
    /// The bio holds no DID at all.
    DidMissing,
    /// The bio holds DIDs, none of them this one (the first found).
    DifferentDid(String),
    /// There is no public bio (absent or blank).
    NoBio,
}

/// The verdict of a bio against `did`: exact, case-sensitive token equality;
/// several DIDs may appear, only `did` counts.
pub fn bio_verdict(bio: Option<&str>, did: &str) -> BioVerdict {
    let Some(bio) = bio.filter(|b| !b.trim().is_empty()) else {
        return BioVerdict::NoBio;
    };
    let tokens = did_tokens(bio);
    if tokens.contains(&did) {
        BioVerdict::Verified
    } else {
        tokens
            .first()
            .map(|found| BioVerdict::DifferentDid((*found).to_string()))
            .unwrap_or(BioVerdict::DidMissing)
    }
}

// =============================================================================
// Recorded check verdicts (ADR-085 §4)
// =============================================================================

/// The closed set of verdicts a local proof check records (ADR-076 spelling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CheckVerdict {
    Verified,
    DidMissing,
    DifferentDid,
    NoBio,
    AccountNotFound,
}

impl CheckVerdict {
    /// Every recorded verdict.
    pub const ALL: [CheckVerdict; 5] = [
        CheckVerdict::Verified,
        CheckVerdict::DidMissing,
        CheckVerdict::DifferentDid,
        CheckVerdict::NoBio,
        CheckVerdict::AccountNotFound,
    ];

    /// The stored token of the verdict.
    pub fn token(self) -> &'static str {
        match self {
            CheckVerdict::Verified => "verified",
            CheckVerdict::DidMissing => "did_missing",
            CheckVerdict::DifferentDid => "different_did",
            CheckVerdict::NoBio => "no_bio",
            CheckVerdict::AccountNotFound => "account_not_found",
        }
    }

    /// The verdict a stored token names, if any.
    pub fn from_token(token: &str) -> Option<CheckVerdict> {
        CheckVerdict::ALL
            .into_iter()
            .find(|verdict| verdict.token() == token)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_check_verdict_round_trips_through_its_token() {
        for verdict in CheckVerdict::ALL {
            assert_eq!(CheckVerdict::from_token(verdict.token()), Some(verdict));
        }
        assert_eq!(CheckVerdict::from_token("identity_changed"), None);
    }
}
