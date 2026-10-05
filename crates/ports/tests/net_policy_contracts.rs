//! Behaviour contracts of the outbound transport policy (DD-IPF-5) against
//! concrete addresses: every refused range at its edges, the public addresses
//! just outside them, IPv4-mapped IPv6 judged as IPv4, loopback admitted only
//! under the test policy, and URLs with userinfo refused.

use std::net::{IpAddr, SocketAddr};

use ports::net_policy::{
    address_admitted, address_refused, admitted_addresses, is_loopback, url_admissible,
    TransportPolicy,
};

const PRODUCTION: TransportPolicy = TransportPolicy::HttpsPublicOnly;
const TEST_SEAM: TransportPolicy = TransportPolicy::HttpsOrLoopbackHttp;

fn ip(text: &str) -> IpAddr {
    text.parse().unwrap()
}

const REFUSED: [&str; 26] = [
    // 0.0.0.0/8
    "0.0.0.0",
    "0.255.255.255",
    // 127/8
    "127.0.0.1",
    "127.255.255.254",
    // 10/8
    "10.0.0.1",
    "10.255.255.255",
    // 172.16/12
    "172.16.0.1",
    "172.31.255.255",
    // 192.168/16
    "192.168.0.1",
    "192.168.255.255",
    // 169.254/16
    "169.254.0.1",
    "169.254.169.254",
    // ::, ::1
    "::",
    "::1",
    // fc00::/7
    "fc00::1",
    "fdff:ffff::1",
    // fe80::/10
    "fe80::1",
    "febf:ffff::1",
    // IPv4-mapped IPv6 judged as IPv4
    "::ffff:0.0.0.1",
    "::ffff:127.0.0.1",
    "::ffff:10.1.2.3",
    "::ffff:172.20.0.1",
    "::ffff:192.168.1.1",
    "::ffff:169.254.169.254",
    "::ffff:127.10.0.1",
    "::ffff:10.0.0.0",
];

const PUBLIC: [&str; 16] = [
    "1.1.1.1",
    "8.8.8.8",
    "1.0.0.0",
    "126.255.255.255",
    "128.0.0.1",
    "11.0.0.1",
    "172.15.255.255",
    "172.32.0.1",
    "192.167.255.255",
    "169.253.0.1",
    "2606:4700::1111",
    "::2",
    "fbff::1",
    "fe00::1",
    "fec0::1",
    "::ffff:8.8.8.8",
];

#[test]
fn every_refused_range_is_refused_at_its_edges() {
    for address in REFUSED {
        assert!(address_refused(ip(address)), "{address} must be refused");
        assert!(!address_admitted(ip(address), PRODUCTION), "{address}");
    }
}

#[test]
fn public_addresses_just_outside_the_refused_ranges_are_admitted() {
    for address in PUBLIC {
        assert!(
            !address_refused(ip(address)),
            "{address} must not be refused"
        );
        for policy in [PRODUCTION, TEST_SEAM] {
            assert!(
                address_admitted(ip(address), policy),
                "{address} {policy:?}"
            );
        }
    }
}

#[test]
fn loopback_is_exactly_127_8_and_ipv6_loopback_including_mapped() {
    for address in ["127.0.0.1", "127.9.9.9", "::1", "::ffff:127.0.0.1"] {
        assert!(is_loopback(ip(address)), "{address}");
    }
    for address in [
        "10.0.0.1",
        "8.8.8.8",
        "::",
        "fe80::1",
        "::ffff:10.0.0.1",
        "::ffff:8.8.8.8",
    ] {
        assert!(!is_loopback(ip(address)), "{address}");
    }
}

#[test]
fn the_test_seam_admits_loopback_and_no_other_refused_address() {
    for address in REFUSED {
        let admitted = address_admitted(ip(address), TEST_SEAM);
        assert_eq!(admitted, is_loopback(ip(address)), "{address}");
    }
    assert!(address_admitted(ip("127.0.0.1"), TEST_SEAM));
    assert!(address_admitted(ip("::1"), TEST_SEAM));
    assert!(address_admitted(ip("::ffff:127.0.0.1"), TEST_SEAM));
    assert!(!address_admitted(ip("10.0.0.1"), TEST_SEAM));
    assert!(!address_admitted(ip("::ffff:192.168.1.1"), TEST_SEAM));
}

#[test]
fn dns_results_keep_only_public_addresses_in_order_under_production() {
    let resolved: Vec<SocketAddr> = ["10.0.0.1", "1.1.1.1", "::1", "::ffff:8.8.8.8", "127.0.0.1"]
        .iter()
        .map(|address| SocketAddr::new(ip(address), 443))
        .collect();
    assert_eq!(
        admitted_addresses(resolved.clone(), PRODUCTION),
        vec![resolved[1], resolved[3]]
    );
    assert_eq!(
        admitted_addresses(resolved.clone(), TEST_SEAM),
        vec![resolved[1], resolved[2], resolved[3], resolved[4]]
    );
}

#[test]
fn a_url_carrying_a_username_or_a_password_is_refused() {
    for policy in [PRODUCTION, TEST_SEAM] {
        for text in [
            "https://jeff@pds.example.com/",
            "https://:secret@pds.example.com/",
            "https://jeff:secret@pds.example.com/",
            "https://jeff@1.1.1.1/",
        ] {
            let url = url::Url::parse(text).unwrap();
            assert!(!url_admissible(&url, policy), "{text} {policy:?}");
        }
        let clean = url::Url::parse("https://pds.example.com/").unwrap();
        assert!(url_admissible(&clean, policy));
    }
}

#[test]
fn https_to_an_ip_literal_follows_the_address_ranges() {
    for (text, admitted) in [
        ("https://1.1.1.1/", true),
        ("https://[2606:4700::1111]/", true),
        ("https://10.0.0.1/", false),
        ("https://[::ffff:10.0.0.1]/", false),
        ("https://[fe80::1]/", false),
        ("https://127.0.0.1/", false),
    ] {
        let url = url::Url::parse(text).unwrap();
        assert_eq!(url_admissible(&url, PRODUCTION), admitted, "{text}");
    }
}

#[test]
fn plain_http_is_admitted_only_to_loopback_under_the_test_seam() {
    let loopback = url::Url::parse("http://127.0.0.1:8080/").unwrap();
    assert!(url_admissible(&loopback, TEST_SEAM));
    assert!(!url_admissible(&loopback, PRODUCTION));
    for text in [
        "http://1.1.1.1/",
        "http://pds.example.com/",
        "http://10.0.0.1/",
    ] {
        let url = url::Url::parse(text).unwrap();
        assert!(!url_admissible(&url, TEST_SEAM), "{text}");
        assert!(!url_admissible(&url, PRODUCTION), "{text}");
    }
}

#[test]
fn transport_policies_report_their_event_tokens() {
    assert_eq!(PRODUCTION.token(), "https_public_only");
    assert_eq!(TEST_SEAM.token(), "https_or_loopback_http");
}
