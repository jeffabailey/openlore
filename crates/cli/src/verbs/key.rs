//! `openlore key` — the identity's claim-signing key, and the line that puts
//! it in the DID document.
//!
//! Claims are signed with an Ed25519 key that peers find in the author's DID
//! document as the `#org.openlore.application` verification method. This
//! verb makes sure the key exists (creating it in the OS keychain the first
//! time; with `OPENLORE_KEY_SEED_HEX` set it reports that key instead) and
//! prints its `did:key`, which the PDS deployment publishes into the DID
//! document (`verification_methods` on tofu-aws-pds). It runs before the
//! regular wiring because that wiring needs the key to already exist.

use anyhow::{anyhow, Result};

/// Where the key came from, for the report.
#[derive(Debug, PartialEq, Eq)]
pub enum KeySource {
    Created,
    Keychain,
    SeedEnv,
}

/// PURE: the report for `did`'s key.
pub fn render_key_report(did: &str, did_key: &str, source: &KeySource) -> String {
    let stored = match source {
        KeySource::Created => "created now, in the OS keychain (service \"openlore\")",
        KeySource::Keychain => "OS keychain (service \"openlore\")",
        KeySource::SeedEnv => "OPENLORE_KEY_SEED_HEX (not the keychain)",
    };
    format!(
        "Claim-signing key for {did}\n  method  : {did}#org.openlore.application\n  did:key : {did_key}\n  stored  : {stored}\n\nPeers verify your claims with this key once your DID document lists it.\nWith tofu-aws-pds, set on the PDS module:\n  verification_methods = {{ \"org.openlore.application\" = \"{did_key}\" }}\n"
    )
}

/// Run the verb: ensure the key, print the report.
pub fn run() -> Result<String> {
    let did = std::env::var("OPENLORE_DID")
        .map_err(|_| anyhow!("OPENLORE_DID is not set; it names the identity whose key to show"))?;
    let (public_key, source) = match std::env::var("OPENLORE_KEY_SEED_HEX") {
        Ok(seed_hex) => (
            adapter_atproto_did::public_key_for_seed_hex(&seed_hex)
                .map_err(|e| anyhow!("OPENLORE_KEY_SEED_HEX: {e}"))?,
            KeySource::SeedEnv,
        ),
        Err(_) => {
            let (key, created) = adapter_atproto_did::ensure_keychain_key(&did)
                .map_err(|e| anyhow!("keychain: {e}"))?;
            let source = if created {
                KeySource::Created
            } else {
                KeySource::Keychain
            };
            (key, source)
        }
    };
    let did_key = format!(
        "did:key:{}",
        claim_domain::encode_ed25519_multibase(&public_key)
    );
    Ok(render_key_report(&did, &did_key, &source))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_names_the_method_and_the_deployment_line() {
        let report = render_key_report("did:plc:abc", "did:key:z6MkTest", &KeySource::Created);
        assert!(report.contains("did:plc:abc#org.openlore.application"));
        assert!(report.contains("did:key : did:key:z6MkTest"));
        assert!(report.contains("created now"));
        assert!(report.contains(
            "verification_methods = { \"org.openlore.application\" = \"did:key:z6MkTest\" }"
        ));
    }
}
