//! `net_policy` — the transport policy every outbound indexer request obeys
//! (DD-IPF-5). Pure, `std::net` only, so adapters and `appview-domain` share it.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Which endpoints the indexer may contact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportPolicy {
    /// Production (always in release builds): https to public addresses only.
    HttpsPublicOnly,
    /// TEST-ONLY (`OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1`, debug builds):
    /// additionally admits plain http to a loopback address.
    HttpsOrLoopbackHttp,
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
