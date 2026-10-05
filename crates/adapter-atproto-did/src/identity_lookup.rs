//! Read-only identity lookup for the review app's sign-in
//! (`IdentityLookupPort`): handle → DID (`com.atproto.identity.resolveHandle`
//! at the configured resolver) → DID document (PLC directory, or `did:web`)
//! → the PDS endpoint and the handle the document confirms.
//!
//! ASYNC reqwest: the review app serves on a tokio runtime, where the blocking
//! client used by `peer_resolve` must not run.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ports::net_policy::{admitted_addresses, url_admissible, TransportPolicy};
use ports::{IdentityLookupError, IdentityLookupPort, ResolvedIdentity};
use serde_json::Value;

/// The whole-request deadline of one lookup.
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);

/// Resolves handles against a resolver service and a PLC directory.
pub struct IdentityLookup {
    client: reqwest::Client,
    handle_resolver_url: String,
    plc_url: String,
    /// The transport policy every lookup obeys; `None` = unguarded (the
    /// review app's sign-in, never the indexer).
    policy: Option<TransportPolicy>,
}

impl IdentityLookup {
    /// The UNGUARDED lookup (the review app's sign-in).
    pub fn new(handle_resolver_url: &str, plc_url: &str) -> Self {
        let client = reqwest::Client::builder()
            .timeout(LOOKUP_TIMEOUT)
            .user_agent("openlore-review-app")
            .build()
            .unwrap_or_default();
        Self::with_client(client, handle_resolver_url, plc_url, None)
    }

    /// The SSRF-guarded lookup the indexer wires (ADR-077 §4, DD-IPF-5): every
    /// directory / `did:web` URL is pre-checked against `policy`, hostnames
    /// resolve through [`GuardedResolver`] (refused addresses dropped, the
    /// connection goes to a checked address), no proxy, redirects not followed.
    /// A refused lookup is `Unavailable` (the DID is then unresolvable).
    pub fn guarded(handle_resolver_url: &str, plc_url: &str, policy: TransportPolicy) -> Self {
        let client = reqwest::Client::builder()
            .timeout(LOOKUP_TIMEOUT)
            .user_agent("openlore-indexer")
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .dns_resolver(Arc::new(GuardedResolver { policy }))
            .build()
            .unwrap_or_default();
        Self::with_client(client, handle_resolver_url, plc_url, Some(policy))
    }

    fn with_client(
        client: reqwest::Client,
        handle_resolver_url: &str,
        plc_url: &str,
        policy: Option<TransportPolicy>,
    ) -> Self {
        Self {
            client,
            handle_resolver_url: handle_resolver_url.trim_end_matches('/').to_string(),
            plc_url: plc_url.trim_end_matches('/').to_string(),
            policy,
        }
    }

    async fn get_json(&self, url: url::Url) -> Result<Lookup, IdentityLookupError> {
        if let Some(policy) = self.policy.filter(|policy| !url_admissible(&url, *policy)) {
            return Err(IdentityLookupError::Unavailable {
                detail: format!("{url} is refused by the transport policy {policy:?}"),
            });
        }
        let response = self.client.get(url).send().await.map_err(unavailable)?;
        let status = response.status();
        if status.is_server_error() {
            return Err(IdentityLookupError::Unavailable {
                detail: format!("HTTP {}", status.as_u16()),
            });
        }
        if !status.is_success() {
            return Ok(Lookup::Absent);
        }
        response
            .json::<Value>()
            .await
            .map(Lookup::Found)
            .map_err(unavailable)
    }

    async fn did_for_handle(&self, handle: &str) -> Result<String, IdentityLookupError> {
        let mut url = parse_url(&format!(
            "{}/xrpc/com.atproto.identity.resolveHandle",
            self.handle_resolver_url
        ))?;
        url.query_pairs_mut().append_pair("handle", handle);
        self.get_json(url)
            .await?
            .found()
            .and_then(|body| body["did"].as_str().map(str::to_string))
            .filter(|did| did.starts_with("did:"))
            .ok_or(IdentityLookupError::NotFound)
    }

    async fn did_document(&self, did: &str) -> Result<Value, IdentityLookupError> {
        let url = match did.strip_prefix("did:web:") {
            Some(host) => parse_url(&format!("https://{host}/.well-known/did.json"))?,
            None => parse_url(&format!("{}/{did}", self.plc_url))?,
        };
        self.get_json(url)
            .await?
            .found()
            .ok_or(IdentityLookupError::NotFound)
    }
}

/// The reqwest resolver of a guarded lookup: hands the connector only the
/// addresses the transport policy admits, so the connection goes to an
/// address that was checked (no second lookup, no rebinding window).
struct GuardedResolver {
    policy: TransportPolicy,
}

/// Every address a host resolved to is refused by the transport policy.
#[derive(Debug, thiserror::Error)]
#[error("{host}: every resolved address is refused by the transport policy")]
struct AddressRefusedByPolicy {
    host: String,
}

impl reqwest::dns::Resolve for GuardedResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        let policy = self.policy;
        Box::pin(async move {
            let resolved = tokio::net::lookup_host((host.as_str(), 0)).await?;
            let admitted = admitted_addresses(resolved, policy);
            if admitted.is_empty() {
                return Err(Box::new(AddressRefusedByPolicy { host }) as _);
            }
            Ok(Box::new(admitted.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

enum Lookup {
    Found(Value),
    Absent,
}

impl Lookup {
    fn found(self) -> Option<Value> {
        match self {
            Self::Found(value) => Some(value),
            Self::Absent => None,
        }
    }
}

fn unavailable(e: impl std::fmt::Display) -> IdentityLookupError {
    IdentityLookupError::Unavailable {
        detail: e.to_string(),
    }
}

fn parse_url(text: &str) -> Result<url::Url, IdentityLookupError> {
    url::Url::parse(text).map_err(unavailable)
}

/// The identity a DID document confirms for `handle` (pure): the document
/// must name the DID, list `at://<handle>` and declare an `#atproto_pds`.
pub fn confirmed_identity(did: &str, handle: &str, document: &Value) -> Option<ResolvedIdentity> {
    let claims_handle = document["alsoKnownAs"].as_array().is_some_and(|akas| {
        akas.iter()
            .any(|aka| aka.as_str() == Some(&format!("at://{handle}")))
    });
    match (claims_handle, pds_endpoint_of(did, document)) {
        (true, Some(pds_endpoint)) => Some(ResolvedIdentity {
            did: did.to_string(),
            verified_handle: handle.to_string(),
            pds_endpoint,
        }),
        _ => None,
    }
}

/// The PDS a DID document names for `did` (pure): the document must name the
/// DID itself and declare an `AtprotoPersonalDataServer` service with id
/// `#atproto_pds` (or `<did>#atproto_pds`). Trailing `/` trimmed.
pub fn pds_endpoint_of(did: &str, document: &Value) -> Option<String> {
    if document["id"].as_str() != Some(did) {
        return None;
    }
    document["service"]
        .as_array()?
        .iter()
        .find(|s| {
            let id = s["id"].as_str().unwrap_or_default();
            (id == "#atproto_pds" || id == format!("{did}#atproto_pds"))
                && s["type"].as_str() == Some("AtprotoPersonalDataServer")
        })
        .and_then(|s| s["serviceEndpoint"].as_str())
        .map(|endpoint| endpoint.trim_end_matches('/').to_string())
}

#[async_trait]
impl IdentityLookupPort for IdentityLookup {
    async fn resolve_identity(
        &self,
        handle: &str,
    ) -> Result<ResolvedIdentity, IdentityLookupError> {
        let did = self.did_for_handle(handle).await?;
        let document = self.did_document(&did).await?;
        confirmed_identity(&did, handle, &document).ok_or(IdentityLookupError::NotFound)
    }

    async fn resolve_did(&self, did: &str) -> Result<ResolvedIdentity, IdentityLookupError> {
        let document = self.did_document(did).await?;
        let claimed = claimed_handle(&document).unwrap_or_else(|| did.to_string());
        let resolves_back = match self.did_for_handle(&claimed).await {
            Ok(resolved) => resolved == did,
            Err(IdentityLookupError::NotFound) => false,
            Err(unavailable) => return Err(unavailable),
        };
        let handle = if resolves_back { claimed.as_str() } else { did };
        confirmed_identity(did, &claimed, &document)
            .map(|identity| ResolvedIdentity {
                verified_handle: handle.to_string(),
                ..identity
            })
            .ok_or(IdentityLookupError::NotFound)
    }

    /// The DID document only — no handle back-check (the indexer needs the PDS).
    async fn resolve_pds(&self, did: &str) -> Result<String, IdentityLookupError> {
        let document = self.did_document(did).await?;
        pds_endpoint_of(did, &document).ok_or(IdentityLookupError::NotFound)
    }
}

/// The first `at://` alias a DID document claims (pure; unverified).
pub fn claimed_handle(document: &Value) -> Option<String> {
    document["alsoKnownAs"]
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find_map(|aka| aka.strip_prefix("at://"))
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    fn document(id: &str, aka: &str, service_id: &str, kind: &str) -> Value {
        json!({
            "id": id,
            "alsoKnownAs": [aka],
            "service": [{"id": service_id, "type": kind, "serviceEndpoint": "https://pds.example/"}],
        })
    }

    proptest! {
        /// Universe: documents varying in the DID they name, the handle they
        /// claim and the service they declare. An identity is confirmed iff
        /// all three agree, and then it carries the declared PDS.
        #[test]
        fn an_identity_is_confirmed_only_when_the_document_agrees(
            same_did in any::<bool>(), same_handle in any::<bool>(),
            pds_id in any::<bool>(), pds_type in any::<bool>()
        ) {
            let did = "did:plc:abc";
            let handle = "priya.example";
            let doc = document(
                if same_did { did } else { "did:plc:other" },
                if same_handle { "at://priya.example" } else { "at://someone.else" },
                if pds_id { "#atproto_pds" } else { "#other" },
                if pds_type { "AtprotoPersonalDataServer" } else { "Other" },
            );
            let confirmed = confirmed_identity(did, handle, &doc);
            prop_assert_eq!(confirmed.is_some(), same_did && same_handle && pds_id && pds_type);
            if let Some(identity) = confirmed {
                prop_assert_eq!(identity.pds_endpoint, "https://pds.example");
            }
        }

        /// Universe: documents varying in the DID they name and the service
        /// they declare. A PDS is found iff the document names the DID and
        /// declares an `#atproto_pds` PDS service; the handle never matters.
        #[test]
        fn a_pds_is_read_only_from_a_document_naming_the_did(
            same_did in any::<bool>(), qualified_id in any::<bool>(),
            pds_id in any::<bool>(), pds_type in any::<bool>(), same_handle in any::<bool>()
        ) {
            let did = "did:plc:abc";
            let service_id = match (pds_id, qualified_id) {
                (true, true) => format!("{did}#atproto_pds"),
                (true, false) => "#atproto_pds".to_string(),
                (false, _) => "#other".to_string(),
            };
            let doc = document(
                if same_did { did } else { "did:plc:other" },
                if same_handle { "at://priya.example" } else { "at://someone.else" },
                &service_id,
                if pds_type { "AtprotoPersonalDataServer" } else { "Other" },
            );
            prop_assert_eq!(
                pds_endpoint_of(did, &doc),
                (same_did && pds_id && pds_type).then(|| "https://pds.example".to_string())
            );
        }
    }

    /// A one-shot fake PLC directory on loopback: every request is answered
    /// with `status` and `body`.
    fn fake_plc(status: u16, body: String) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind fake PLC");
        let address = listener.local_addr().expect("fake PLC address");
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut request = [0u8; 4096];
                let _ = stream.read(&mut request);
                let response = format!(
                    "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        format!("http://{address}")
    }

    // bypass: wiring over real HTTP — one example per DID-document failure
    // class (ADR-077); the document rules themselves are covered by property.
    #[test]
    fn every_did_document_failure_class_is_reported_through_resolve_pds() {
        let did = "did:plc:abc";
        let good = document(
            did,
            "at://priya.example",
            "#atproto_pds",
            "AtprotoPersonalDataServer",
        );
        let cases = [
            (200, good.to_string(), Ok("https://pds.example".to_string())),
            (404, String::new(), Err(IdentityLookupError::NotFound)),
            (
                200,
                document(
                    "did:plc:other",
                    "at://priya.example",
                    "#atproto_pds",
                    "AtprotoPersonalDataServer",
                )
                .to_string(),
                Err(IdentityLookupError::NotFound),
            ),
            (
                200,
                document(did, "at://priya.example", "#other", "Other").to_string(),
                Err(IdentityLookupError::NotFound),
            ),
        ];
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        for (status, body, expected) in cases {
            let plc = fake_plc(status, body);
            let lookup = IdentityLookup::new(&plc, &plc);
            assert_eq!(
                runtime.block_on(lookup.resolve_pds(did)),
                expected,
                "HTTP {status}"
            );
        }
        let plc = fake_plc(503, String::new());
        let lookup = IdentityLookup::new(&plc, &plc);
        assert!(
            matches!(
                runtime.block_on(lookup.resolve_pds(did)),
                Err(IdentityLookupError::Unavailable { .. })
            ),
            "a directory error is unavailability"
        );
    }

    // bypass: wiring over real sockets — the guarded lookup refuses a loopback
    // directory (IP literal and a name resolving to loopback) without
    // connecting, and admits loopback http only under the test policy.
    #[test]
    fn the_guarded_lookup_never_contacts_a_refused_directory() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind tripwire");
        listener.set_nonblocking(true).expect("nonblocking");
        let port = listener.local_addr().expect("tripwire address").port();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        for directory in [
            format!("http://127.0.0.1:{port}"),
            format!("https://127.0.0.1:{port}"),
            format!("https://localhost:{port}"),
        ] {
            let lookup =
                IdentityLookup::guarded(&directory, &directory, TransportPolicy::HttpsPublicOnly);
            assert!(
                matches!(
                    runtime.block_on(lookup.resolve_pds("did:plc:abc")),
                    Err(IdentityLookupError::Unavailable { .. })
                ),
                "{directory}"
            );
        }
        assert_eq!(
            std::iter::from_fn(|| listener.accept().ok()).count(),
            0,
            "a refused directory is never contacted"
        );

        let good = document(
            "did:plc:abc",
            "at://priya.example",
            "#atproto_pds",
            "AtprotoPersonalDataServer",
        );
        let plc = fake_plc(200, good.to_string());
        let lookup = IdentityLookup::guarded(&plc, &plc, TransportPolicy::HttpsOrLoopbackHttp);
        assert_eq!(
            runtime.block_on(lookup.resolve_pds("did:plc:abc")),
            Ok("https://pds.example".to_string())
        );
    }
}
