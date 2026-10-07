//! The per-client token bucket and its bounded client map, on concrete
//! schedules (review H3): when a bucket counts as refilled, the Retry-After of
//! a partly refilled bucket, and which clients the limiter forgets when full.

use std::net::{IpAddr, Ipv4Addr};
use std::time::{Duration, Instant};

use adapter_xrpc_query_server::rate_limit::{take, RateDecision, RateLimiter, TokenBucket};
use adapter_xrpc_query_server::RateLimit;

/// One request a second, one at a time.
fn one_a_second() -> RateLimit {
    RateLimit::new(1, 1).expect("non-zero limit")
}

fn client(last_octet: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(203, 0, 113, last_octet))
}

#[test]
fn a_bucket_is_full_when_fresh_empty_after_a_request_and_full_again_after_a_second() {
    let limit = one_a_second();
    let start = Instant::now();
    let fresh = TokenBucket::full(limit, start);
    assert!(fresh.is_full_at(start, limit));

    let (spent, decision) = take(fresh, start, limit);
    assert_eq!(decision, RateDecision::Admitted);
    assert!(!spent.is_full_at(start, limit));
    assert!(!spent.is_full_at(start + Duration::from_millis(999), limit));
    assert!(spent.is_full_at(start + Duration::from_secs(1), limit));
}

#[test]
fn a_half_refilled_bucket_is_refused_with_retry_after_one_second() {
    let limit = one_a_second();
    let start = Instant::now();
    let (spent, _) = take(TokenBucket::full(limit, start), start, limit);
    let (_, decision) = take(spent, start + Duration::from_millis(500), limit);
    assert_eq!(
        decision,
        RateDecision::Limited {
            retry_after: Duration::from_secs(1)
        }
    );
}

#[test]
fn a_new_client_under_capacity_forgets_nobody() {
    let limiter = RateLimiter::with_capacity(one_a_second(), 10);
    let start = Instant::now();
    assert_eq!(limiter.check(client(1), start), RateDecision::Admitted);
    // client 1's bucket has refilled by now, but the map is not full.
    assert_eq!(
        limiter.check(client(2), start + Duration::from_secs(2)),
        RateDecision::Admitted
    );
    assert_eq!(limiter.tracked_clients(), 2);
}

#[test]
fn a_full_limiter_forgets_a_refilled_client_and_keeps_an_active_one() {
    let limiter = RateLimiter::with_capacity(one_a_second(), 2);
    let start = Instant::now();
    let later = start + Duration::from_secs(3);
    assert_eq!(limiter.check(client(1), start), RateDecision::Admitted);
    assert_eq!(limiter.check(client(2), later), RateDecision::Admitted);
    // The map is full: client 1 has refilled (forgettable), client 2 has not.
    assert_eq!(limiter.check(client(3), later), RateDecision::Admitted);
    assert_eq!(limiter.tracked_clients(), 2);
    // Client 2 was kept, so its spent bucket still refuses it.
    assert_eq!(
        limiter.check(client(2), later),
        RateDecision::Limited {
            retry_after: Duration::from_secs(1)
        }
    );
}
