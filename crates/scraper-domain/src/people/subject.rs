//! The person a new surface names, validated once at the edge (D-8).

use claim_domain::identity_link::is_github_login;

/// A person the inference is ABOUT, named `github:<login>` (D-8) — the only
/// form a new surface accepts; validated once at the edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonSubject(String);

/// Why a named person was refused (D-8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonSubjectError {
    pub given: String,
}

impl std::fmt::Display for PersonSubjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "a person is named as github:<login> (e.g. github:BurntSushi), not `{}`",
            self.given
        )
    }
}

impl std::error::Error for PersonSubjectError {}

impl PersonSubject {
    /// Accept exactly `github:<login>` (login: 1–39 ASCII letters, digits and
    /// hyphens, not starting with a hyphen — the shared GitHub login rule);
    /// refuse anything else.
    pub fn parse(raw: &str) -> Result<Self, PersonSubjectError> {
        match raw.strip_prefix("github:") {
            Some(login) if is_github_login(login) => Ok(Self(raw.to_string())),
            _ => Err(PersonSubjectError {
                given: raw.to_string(),
            }),
        }
    }

    /// The `github:<login>` subject.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
