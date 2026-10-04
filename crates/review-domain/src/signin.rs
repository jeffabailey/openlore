//! Sign-in decisions (US-BRA-001): handle syntax, the sign-in pin, how a
//! return from the user's PDS is read, and which permission mode the app
//! runs in. Pure: no I/O, no clock, no randomness.

/// A syntactically valid Bluesky handle, normalised (no `@`, lower case).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handle(String);

impl Handle {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Parse what a person typed into the handle field. Accepts a leading `@`
/// and surrounding whitespace; refuses anything that cannot be a handle
/// (atproto handle syntax: ≥ 2 dot-separated labels of `[a-z0-9-]`, no label
/// starting or ending with `-`, a top-level label not starting with a digit,
/// at most 253 characters).
pub fn parse_handle(typed: &str) -> Option<Handle> {
    let candidate = typed.trim().trim_start_matches('@').to_ascii_lowercase();
    let labels: Vec<&str> = candidate.split('.').collect();
    let label_ok = |label: &&str| {
        (1..=63).contains(&label.len())
            && label
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !label.starts_with('-')
            && !label.ends_with('-')
    };
    let top_level_ok = labels
        .last()
        .is_some_and(|tld| !tld.starts_with(|c: char| c.is_ascii_digit()));
    (candidate.len() <= 253 && labels.len() >= 2 && labels.iter().all(label_ok) && top_level_ok)
        .then_some(Handle(candidate))
}

/// The verdict of the sign-in pin (AC-001.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInPin {
    /// The token's subject is exactly the DID the handle resolved to.
    Accepted,
    /// The PDS answered for a different account: sign nobody in.
    Refused,
}

impl SignInPin {
    pub fn is_accepted(self) -> bool {
        self == Self::Accepted
    }
}

/// A sign-in is accepted iff the token's subject is byte-equal to the DID
/// the handle resolved to before authorization began.
pub fn sign_in_pin(token_subject: &str, resolved_did: &str) -> SignInPin {
    if token_subject == resolved_did {
        SignInPin::Accepted
    } else {
        SignInPin::Refused
    }
}

/// What the user's PDS sent back to the sign-in return address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdsReturn {
    /// The person approved: a code to exchange, bound to `state`.
    Approved {
        code: String,
        state: String,
        issuer: Option<String>,
    },
    /// The person cancelled (or the PDS refused) the authorization.
    Declined { state: Option<String> },
    /// Not a return any authorization of ours could produce.
    Malformed,
}

/// Read the query of a return to the callback address (pure, total).
pub fn read_pds_return(query: &[(String, String)]) -> PdsReturn {
    let field = |name: &str| {
        query
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .filter(|value| !value.is_empty())
    };
    match (field("error"), field("code"), field("state")) {
        (Some(_), _, state) => PdsReturn::Declined { state },
        (None, Some(code), Some(state)) => PdsReturn::Approved {
            code,
            state,
            issuer: field("iss"),
        },
        _ => PdsReturn::Malformed,
    }
}

/// Why a sign-in did not complete. Every one of them changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignInFailure {
    /// The handle is not a handle, or no account answers to it.
    HandleNotFound,
    /// The user's PDS (or the identity directory) could not be reached or
    /// could not identify the app.
    TemporarilyUnavailable,
    /// The person cancelled at their PDS.
    Cancelled,
    /// The PDS refused to exchange the sign-in code.
    ExchangeFailed,
    /// The PDS answered for an account other than the one the handle named.
    AccountMismatch,
    /// A return link that is unknown, expired or already used.
    ReturnNotRecognised,
}

/// Which OAuth permission set the deployment asks for (ADR-073).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    /// Claim + post creation only.
    Granular,
    /// The broad `transition:generic` fallback: disclosed on the landing page.
    BroadFallback,
}

/// Classify the configured scope string.
pub fn permission_mode(scopes: &str) -> PermissionMode {
    if scopes
        .split_whitespace()
        .any(|scope| scope == "transition:generic")
    {
        PermissionMode::BroadFallback
    } else {
        PermissionMode::Granular
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn label() -> impl Strategy<Value = String> {
        "[a-z0-9]([a-z0-9-]{0,10}[a-z0-9])?"
    }

    fn handle() -> impl Strategy<Value = String> {
        (prop::collection::vec(label(), 1..4), "[a-z][a-z0-9]{1,5}")
            .prop_map(|(labels, tld)| format!("{}.{tld}", labels.join(".")))
    }

    fn query_field() -> impl Strategy<Value = (String, String)> {
        (
            prop_oneof![
                Just("code"),
                Just("state"),
                Just("iss"),
                Just("error"),
                Just("other")
            ],
            "[a-z0-9-]{0,6}",
        )
            .prop_map(|(k, v)| (k.to_string(), v))
    }

    proptest! {
        /// Universe: every well-formed handle, typed with or without `@`,
        /// in any case, padded. It parses to its normalised self.
        #[test]
        fn every_well_formed_handle_parses_to_its_normalised_form(
            h in handle(), at in any::<bool>(), upper in any::<bool>()
        ) {
            let typed = format!(" {}{} ", if at { "@" } else { "" }, if upper { h.to_ascii_uppercase() } else { h.clone() });
            prop_assert_eq!(parse_handle(&typed).map(|p| p.as_str().to_string()), Some(h));
        }

        /// Universe: arbitrary text. Anything accepted is a handle: labels of
        /// `[a-z0-9-]`, at least two, none empty, no spaces.
        #[test]
        fn anything_accepted_has_handle_shape(typed in ".{0,40}") {
            if let Some(h) = parse_handle(&typed) {
                let s = h.as_str();
                prop_assert!(s.split('.').count() >= 2);
                prop_assert!(s.split('.').all(|l| !l.is_empty() && !l.starts_with('-') && !l.ends_with('-')));
                prop_assert!(s.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.'));
            }
        }

        /// Universe: (subject, resolved DID) pairs. The pin accepts exactly
        /// the byte-equal pairs.
        #[test]
        fn the_pin_accepts_exactly_equal_subjects(a in "did:plc:[a-z2-7]{4}", b in "did:plc:[a-z2-7]{4}") {
            prop_assert_eq!(sign_in_pin(&a, &b).is_accepted(), a == b);
            prop_assert!(sign_in_pin(&a, &a).is_accepted());
        }

        /// Universe: every callback query over the OAuth parameter names. An
        /// `error` always means declined; a code is exchanged only with a state.
        #[test]
        fn a_return_is_approved_only_with_code_and_state_and_no_error(
            query in prop::collection::vec(query_field(), 0..6)
        ) {
            let has = |k: &str| query.iter().find(|(key, _)| key == k).is_some_and(|(_, v)| !v.is_empty());
            let read = read_pds_return(&query);
            match read {
                PdsReturn::Declined { .. } => prop_assert!(has("error")),
                PdsReturn::Approved { .. } => prop_assert!(!has("error") && has("code") && has("state")),
                PdsReturn::Malformed => prop_assert!(!(has("error") || (has("code") && has("state")))),
            }
        }

        /// Universe: scope strings over the known scope names. Broad mode iff
        /// `transition:generic` is one of the scopes.
        #[test]
        fn broad_mode_iff_the_generic_scope_is_requested(
            scopes in prop::collection::vec(prop_oneof![
                Just("atproto"), Just("transition:generic"), Just("transition:genericx"),
                Just("repo:org.openlore.claim?action=create"),
            ], 0..4)
        ) {
            let joined = scopes.join(" ");
            prop_assert_eq!(
                permission_mode(&joined) == PermissionMode::BroadFallback,
                scopes.contains(&"transition:generic")
            );
        }
    }
}
