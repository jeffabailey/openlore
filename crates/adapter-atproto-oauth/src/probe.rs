//! Earned-Trust probe for the OAuth client (ADR-073). Hard arms:
//!
//! 1. key: the client key signs a canary and the PUBLISHED key verifies it,
//!    so what the authorization server fetches matches what the app signs with;
//! 2. JWKS shape: every published key is ES256 and carries no private part.

use p256::ecdsa::signature::{Signer, Verifier};
use p256::ecdsa::{Signature, VerifyingKey};
use ports::{ProbeOutcome, ProbeRefusalReason};
use serde_json::{json, Value};

use crate::ClientKey;

const CANARY: &[u8] = b"openlore-review-oauth-canary";

type Arm = Result<(), (ProbeRefusalReason, String)>;

pub(crate) fn run(key: &ClientKey, published: &Value) -> ProbeOutcome {
    match classify_jwks(published).and_then(|()| key_arm(key)) {
        Ok(()) => ProbeOutcome::Ok,
        Err((reason, detail)) => ProbeOutcome::Refused {
            reason,
            structured: json!({ "probe": "oauth-client", "detail": detail }),
            detail,
        },
    }
}

fn key_arm(key: &ClientKey) -> Arm {
    let signature: Signature = p256::ecdsa::SigningKey::from(key.secret()).sign(CANARY);
    let published = match &key.public_jwk().key {
        jose_jwk::Key::Ec(ec) => p256::PublicKey::try_from(ec).ok(),
        _ => None,
    };
    let verified = published
        .map(|public| {
            VerifyingKey::from(&public)
                .verify(CANARY, &signature)
                .is_ok()
        })
        .unwrap_or(false);
    if verified {
        Ok(())
    } else {
        Err((
            ProbeRefusalReason::OAuthClientKeyUnusable,
            "the published client key does not verify the client's signature".into(),
        ))
    }
}

/// Pure: a non-empty key set of ES256 P-256 keys, none with a private part.
pub(crate) fn classify_jwks(jwks: &Value) -> Arm {
    let refuse = |detail: &str| Err((ProbeRefusalReason::OAuthJwksMalformed, detail.to_string()));
    let Some(keys) = jwks.get("keys").and_then(Value::as_array) else {
        return refuse("the JWKS has no key list");
    };
    if keys.is_empty() {
        return refuse("the JWKS is empty");
    }
    for key in keys {
        if key.get("d").is_some() {
            return refuse("a published key carries its private part");
        }
        let es256 = key.get("alg") == Some(&json!("ES256"))
            && key.get("kty") == Some(&json!("EC"))
            && key.get("crv") == Some(&json!("P-256"));
        if !es256 {
            return refuse("a published key is not ES256 (P-256)");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn published_key() -> Value {
        json!({"kty": "EC", "crv": "P-256", "alg": "ES256", "x": "x", "y": "y", "kid": "k"})
    }

    proptest! {
        /// Universe: key sets built from well-formed public keys, each
        /// possibly carrying a private part or a wrong alg. The set admits
        /// iff it is non-empty and every key is public ES256.
        #[test]
        fn jwks_admits_only_public_es256_keys(
            keys in proptest::collection::vec((any::<bool>(), any::<bool>()), 0..5)
        ) {
            let set: Vec<Value> = keys.iter().map(|(private, wrong_alg)| {
                let mut k = published_key();
                if *private { k["d"] = json!("secret"); }
                if *wrong_alg { k["alg"] = json!("RS256"); }
                k
            }).collect();
            let admits = !keys.is_empty() && keys.iter().all(|(p, w)| !p && !w);
            prop_assert_eq!(classify_jwks(&json!({"keys": set})).is_ok(), admits);
        }
    }

    // bypass: one wiring example — a generated key round-trips through its
    // JSON secret form and passes both hard arms.
    #[test]
    fn a_generated_key_survives_its_secret_form_and_passes_the_probe() {
        use ports::OAuthPort;
        let key = ClientKey::parse(&ClientKey::generate("k1").to_private_json()).unwrap();
        let adapter = crate::OAuthClientAdapter::new(key, "https://app.example", "atproto");
        assert!(matches!(adapter.probe(), ProbeOutcome::Ok));
        assert!(adapter.public_jwks()["keys"][0].get("d").is_none());
    }
}
