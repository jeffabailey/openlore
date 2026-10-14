//! The guarded DNS resolver every outbound identity and PDS fetch uses
//! (ADR-077 SSRF guard). It hands reqwest's connector only the addresses the
//! pure [`ports::net_policy`] transport policy admits, so the connection goes
//! to an address that was checked: there is no second lookup and no
//! rebinding window. None admitted is an error the caller can recognise with
//! [`refused_by_policy`].

use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::sync::Arc;

use ports::net_policy::{admitted_addresses, TransportPolicy};

/// Resolves a host name to its addresses (the system resolver in production,
/// a stub in tests).
pub type HostLookup = Arc<
    dyn Fn(
            String,
        )
            -> Pin<Box<dyn std::future::Future<Output = std::io::Result<Vec<IpAddr>>> + Send>>
        + Send
        + Sync,
>;

/// The system resolver.
pub fn system_lookup() -> HostLookup {
    Arc::new(|host: String| {
        Box::pin(async move {
            tokio::net::lookup_host((host.as_str(), 0))
                .await
                .map(|addrs| addrs.map(|addr| addr.ip()).collect())
        })
    })
}

/// Every address `host` resolved to is refused by the transport policy.
#[derive(Debug, thiserror::Error)]
#[error("{host}: every resolved address is refused by the transport policy")]
pub struct AddressRefusedByPolicy {
    pub host: String,
}

/// The reqwest resolver of a guarded client: only admitted addresses reach
/// the connector; none admitted = [`AddressRefusedByPolicy`].
pub struct GuardedResolver {
    policy: TransportPolicy,
    lookup: HostLookup,
}

impl GuardedResolver {
    /// Guard the system resolver with `policy`.
    pub fn new(policy: TransportPolicy) -> Self {
        Self::with_lookup(policy, system_lookup())
    }

    /// Guard `lookup` with `policy` (a stub resolver in tests).
    pub fn with_lookup(policy: TransportPolicy, lookup: HostLookup) -> Self {
        Self { policy, lookup }
    }
}

impl reqwest::dns::Resolve for GuardedResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        let lookup = (self.lookup)(host.clone());
        let policy = self.policy;
        Box::pin(async move {
            let resolved = lookup.await?;
            let admitted = admitted_addresses(
                resolved.into_iter().map(|ip| SocketAddr::new(ip, 0)),
                policy,
            );
            if admitted.is_empty() {
                return Err(Box::new(AddressRefusedByPolicy { host }) as _);
            }
            Ok(Box::new(admitted.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// Whether a request failed because the guard refused every address.
pub fn refused_by_policy(error: &(dyn std::error::Error + 'static)) -> bool {
    std::iter::successors(Some(error), |e| e.source())
        .any(|e| e.downcast_ref::<AddressRefusedByPolicy>().is_some())
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::dns::Resolve;

    fn answering(addresses: &'static [&'static str]) -> HostLookup {
        Arc::new(move |_host: String| {
            Box::pin(async move { Ok(addresses.iter().map(|a| a.parse().unwrap()).collect()) })
        })
    }

    async fn resolve(policy: TransportPolicy, lookup: HostLookup) -> Result<Vec<IpAddr>, String> {
        let name: reqwest::dns::Name = "pds.example.test".parse().unwrap();
        GuardedResolver::with_lookup(policy, lookup)
            .resolve(name)
            .await
            .map(|addrs| addrs.map(|a| a.ip()).collect())
            .map_err(|e| e.to_string())
    }

    // bypass: one literal answer per policy; the policy itself is
    // property-tested in ports::net_policy.
    #[tokio::test]
    async fn only_admitted_addresses_reach_the_connector() {
        let mixed = answering(&["10.0.0.1", "93.184.216.34", "::1", "169.254.169.254"]);
        assert_eq!(
            resolve(TransportPolicy::HttpsPublicOnly, mixed).await,
            Ok(vec!["93.184.216.34".parse().unwrap()])
        );
    }

    #[tokio::test]
    async fn a_name_with_only_refused_addresses_is_refused_by_the_policy() {
        let private = answering(&["10.0.0.1", "::1", "169.254.169.254"]);
        let refused = resolve(TransportPolicy::HttpsPublicOnly, private).await;
        assert_eq!(
            refused,
            Err(
                "pds.example.test: every resolved address is refused by the transport policy"
                    .to_string()
            )
        );
        let error: Box<dyn std::error::Error + Send + Sync> =
            Box::new(AddressRefusedByPolicy { host: "h".into() });
        assert!(refused_by_policy(error.as_ref()));
        assert!(!refused_by_policy(&std::fmt::Error));
    }
}
