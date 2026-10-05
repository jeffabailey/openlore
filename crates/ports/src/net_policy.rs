//! `net_policy` — the transport policy every outbound indexer request obeys
//! (DD-IPF-5). Pure, so adapters and `appview-domain` share it: the guarded
//! adapters check a URL before any request ([`url_admissible`]) and filter
//! what DNS returned ([`admitted_addresses`]) before connecting.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// Which endpoints the indexer may contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportPolicy {
    /// Production (always in release builds): https to public addresses only.
    HttpsPublicOnly,
    /// TEST-ONLY (`OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1`, debug builds):
    /// additionally admits plain http to a loopback address.
    HttpsOrLoopbackHttp,
}

impl TransportPolicy {
    /// The event token (`indexer.config.loaded.transport_policy`).
    pub const fn token(self) -> &'static str {
        match self {
            Self::HttpsPublicOnly => "https_public_only",
            Self::HttpsOrLoopbackHttp => "https_or_loopback_http",
        }
    }
}

/// Whether `ip` is in a refused range: 0.0.0.0/8, 127/8, 10/8, 172.16/12,
/// 192.168/16, 169.254/16, `::`, `::1`, fc00::/7, fe80::/10. An IPv4-mapped
/// IPv6 address is judged as its IPv4 address.
pub fn address_refused(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4_refused(v4),
        IpAddr::V6(v6) => v6
            .to_ipv4_mapped()
            .map_or_else(|| v6_refused(v6), v4_refused),
    }
}

/// Whether `ip` is a loopback address (127/8, `::1`, or IPv4-mapped 127/8).
pub fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => v6
            .to_ipv4_mapped()
            .map_or_else(|| v6.is_loopback(), |v4| v4.is_loopback()),
    }
}

/// Whether a connection to `ip` is admitted under `policy`: never to a refused
/// range, except loopback under [`TransportPolicy::HttpsOrLoopbackHttp`].
pub fn address_admitted(ip: IpAddr, policy: TransportPolicy) -> bool {
    !address_refused(ip) || (policy == TransportPolicy::HttpsOrLoopbackHttp && is_loopback(ip))
}

/// The resolved addresses a connection may go to under `policy`, in the order
/// DNS returned them. Empty = the host is refused.
pub fn admitted_addresses(
    resolved: impl IntoIterator<Item = SocketAddr>,
    policy: TransportPolicy,
) -> Vec<SocketAddr> {
    resolved
        .into_iter()
        .filter(|addr| address_admitted(addr.ip(), policy))
        .collect()
}

/// The pre-request check on a URL (before any DNS): `https`, or `http` only to
/// a loopback IP literal under [`TransportPolicy::HttpsOrLoopbackHttp`]; no
/// userinfo; an IP-literal host must be admitted. A hostname passes here and
/// is judged after DNS by [`admitted_addresses`].
pub fn url_admissible(url: &url::Url, policy: TransportPolicy) -> bool {
    let ip_literal = match url.host() {
        Some(url::Host::Ipv4(v4)) => Some(IpAddr::V4(v4)),
        Some(url::Host::Ipv6(v6)) => Some(IpAddr::V6(v6)),
        Some(url::Host::Domain(_)) => None,
        None => return false,
    };
    let loopback_under_test_policy =
        policy == TransportPolicy::HttpsOrLoopbackHttp && ip_literal.is_some_and(is_loopback);
    let scheme_admitted =
        url.scheme() == "https" || (url.scheme() == "http" && loopback_under_test_policy);
    let host_admitted = ip_literal.is_none_or(|ip| address_admitted(ip, policy));
    let no_userinfo = url.username().is_empty() && url.password().is_none();
    scheme_admitted && host_admitted && no_userinfo
}

fn v4_refused(ip: Ipv4Addr) -> bool {
    // is_private = 10/8, 172.16/12, 192.168/16; is_link_local = 169.254/16.
    ip.octets()[0] == 0 || ip.is_loopback() || ip.is_private() || ip.is_link_local()
}

fn v6_refused(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_unspecified()
        || ip.is_loopback()
        || (first & 0xfe00) == 0xfc00
        || (first & 0xffc0) == 0xfe80
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn arb_policy() -> impl Strategy<Value = TransportPolicy> {
        prop_oneof![
            Just(TransportPolicy::HttpsPublicOnly),
            Just(TransportPolicy::HttpsOrLoopbackHttp),
        ]
    }

    fn arb_ip() -> impl Strategy<Value = IpAddr> {
        prop_oneof![
            any::<u32>().prop_map(|v4| IpAddr::V4(Ipv4Addr::from(v4))),
            any::<[u8; 4]>().prop_map(|[a, b, c, d]| IpAddr::V4(Ipv4Addr::new(
                [0, 10, 127, 169, 172, 192][a as usize % 6],
                b,
                c,
                d
            ))),
            any::<u128>().prop_map(|v6| IpAddr::V6(Ipv6Addr::from(v6))),
            any::<u32>().prop_map(|v4| IpAddr::V6(Ipv4Addr::from(v4).to_ipv6_mapped())),
            Just(IpAddr::V6(Ipv6Addr::LOCALHOST)),
        ]
    }

    fn url_host(ip: IpAddr) -> String {
        match ip {
            IpAddr::V4(v4) => v4.to_string(),
            IpAddr::V6(v6) => format!("[{v6}]"),
        }
    }

    proptest! {
        /// Universe {resolved addresses × policy}: the filter keeps exactly the
        /// admitted addresses, in DNS order; under the production policy
        /// nothing refused survives, under the test policy only loopback does.
        #[test]
        fn dns_results_keep_exactly_the_admitted_addresses(
            ips in proptest::collection::vec(arb_ip(), 0..8),
            port in any::<u16>(),
            policy in arb_policy(),
        ) {
            let resolved: Vec<SocketAddr> = ips.iter().map(|ip| SocketAddr::new(*ip, port)).collect();
            let admitted = admitted_addresses(resolved.clone(), policy);
            let expected: Vec<SocketAddr> = resolved
                .into_iter()
                .filter(|a| !address_refused(a.ip()) || (policy == TransportPolicy::HttpsOrLoopbackHttp && is_loopback(a.ip())))
                .collect();
            prop_assert_eq!(&admitted, &expected);
            for addr in &admitted {
                prop_assert!(!address_refused(addr.ip()) || policy == TransportPolicy::HttpsOrLoopbackHttp);
                prop_assert!(!address_refused(addr.ip()) || is_loopback(addr.ip()));
            }
        }

        /// Universe {scheme × IP-literal host × policy}: a URL is admitted
        /// exactly when https goes to an admitted address, or http goes to
        /// loopback under the test policy.
        #[test]
        fn an_ip_literal_url_is_admitted_exactly_by_scheme_and_address(
            ip in arb_ip(),
            https in any::<bool>(),
            policy in arb_policy(),
        ) {
            let scheme = if https { "https" } else { "http" };
            let url = url::Url::parse(&format!("{scheme}://{}:8443/x", url_host(ip))).unwrap();
            let test_loopback = policy == TransportPolicy::HttpsOrLoopbackHttp && is_loopback(ip);
            let expected = (https && address_admitted(ip, policy)) || (!https && test_loopback);
            prop_assert_eq!(url_admissible(&url, policy), expected);
        }

        /// Universe {hostname × scheme × userinfo × policy}: a hostname passes
        /// the pre-check (DNS judges it) only over https and without userinfo.
        #[test]
        fn a_hostname_url_passes_only_over_https_without_userinfo(
            host in "[a-z]{1,10}\\.[a-z]{2,4}",
            https in any::<bool>(),
            userinfo in any::<bool>(),
            policy in arb_policy(),
        ) {
            let scheme = if https { "https" } else { "http" };
            let credentials = if userinfo { "jeff:secret@" } else { "" };
            let url = url::Url::parse(&format!("{scheme}://{credentials}{host}")).unwrap();
            prop_assert_eq!(url_admissible(&url, policy), https && !userinfo);
        }
    }
}
