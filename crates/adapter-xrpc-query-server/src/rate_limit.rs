//! `rate_limit` — per-client admission on the public listener (review H3).
//!
//! Stock Caddy has no rate-limit module, so the limit lives in the binary: a
//! token bucket per client IP, `per_sec` tokens a second up to `burst`. The
//! client IP is the socket peer, unless the peer is a trusted proxy (loopback,
//! or a configured network such as Caddy's), in which case it is the address
//! that proxy appended to `X-Forwarded-For` (the rightmost entry). A client
//! cannot pick its own bucket by sending the header directly.
//!
//! The bucket arithmetic ([`take`]) and the client derivation ([`client_ip`])
//! are pure; [`RateLimiter`] is the small effect shell holding one bucket per
//! client in a map whose size is bounded ([`MAX_TRACKED_CLIENTS`]): a bucket
//! that has refilled to full is indistinguishable from a fresh one and is
//! evicted first.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Tokens a client regains per second (sustained requests per second).
pub const DEFAULT_RATE_PER_SEC: u32 = 10;

/// The most requests a client may make at once after being idle.
pub const DEFAULT_RATE_BURST: u32 = 50;

/// The most clients whose buckets are remembered at once.
pub const MAX_TRACKED_CLIENTS: usize = 10_000;

/// One token in the bucket's fixed-point unit (milli-tokens).
const TOKEN: u64 = 1_000;

/// A per-client limit: `per_sec` tokens a second, at most `burst` stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimit {
    per_sec: u32,
    burst: u32,
}

impl RateLimit {
    /// 10 requests a second, bursts of 50.
    pub const DEFAULT: Self = Self {
        per_sec: DEFAULT_RATE_PER_SEC,
        burst: DEFAULT_RATE_BURST,
    };

    /// A limit of `per_sec` a second and `burst` at once; `None` when either
    /// is zero (a limit that admits nothing is a misconfiguration).
    #[must_use]
    pub const fn new(per_sec: u32, burst: u32) -> Option<Self> {
        if per_sec == 0 || burst == 0 {
            None
        } else {
            Some(Self { per_sec, burst })
        }
    }

    #[must_use]
    pub const fn per_sec(self) -> u32 {
        self.per_sec
    }

    #[must_use]
    pub const fn burst(self) -> u32 {
        self.burst
    }

    const fn capacity(self) -> u64 {
        self.burst as u64 * TOKEN
    }
}

impl Default for RateLimit {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// One client's bucket: its tokens (milli-tokens) as of `refilled_at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenBucket {
    milli_tokens: u64,
    refilled_at: Instant,
}

impl TokenBucket {
    /// A bucket holding a whole burst at `now` (a client never seen before).
    #[must_use]
    pub const fn full(limit: RateLimit, now: Instant) -> Self {
        Self {
            milli_tokens: limit.capacity(),
            refilled_at: now,
        }
    }

    /// The bucket refilled up to `now` (never past the burst).
    #[must_use]
    fn refilled(self, now: Instant, limit: RateLimit) -> Self {
        let elapsed = now.saturating_duration_since(self.refilled_at);
        let gained = elapsed.as_nanos() * u128::from(limit.per_sec) / 1_000_000;
        let milli_tokens = u128::from(self.milli_tokens)
            .saturating_add(gained)
            .min(u128::from(limit.capacity()));
        Self {
            milli_tokens: u64::try_from(milli_tokens).unwrap_or(u64::MAX),
            refilled_at: now.max(self.refilled_at),
        }
    }

    /// Whether the bucket has refilled to a whole burst by `now`: it is then
    /// the same as a fresh bucket, so forgetting it changes nothing.
    #[must_use]
    pub fn is_full_at(self, now: Instant, limit: RateLimit) -> bool {
        self.refilled(now, limit).milli_tokens >= limit.capacity()
    }
}

/// Whether one request is admitted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateDecision {
    Admitted,
    /// Over the limit: try again after `retry_after` (whole seconds, ≥ 1).
    Limited {
        retry_after: Duration,
    },
}

/// One request against `bucket` at `now`: refill, then spend one token when a
/// whole one is there. Returns the bucket after the request and the decision.
#[must_use]
pub fn take(bucket: TokenBucket, now: Instant, limit: RateLimit) -> (TokenBucket, RateDecision) {
    let refilled = bucket.refilled(now, limit);
    if refilled.milli_tokens >= TOKEN {
        let spent = TokenBucket {
            milli_tokens: refilled.milli_tokens - TOKEN,
            ..refilled
        };
        (spent, RateDecision::Admitted)
    } else {
        let missing = TOKEN - refilled.milli_tokens;
        let per_sec = u64::from(limit.per_sec) * TOKEN;
        let retry_after = Duration::from_secs(missing.div_ceil(per_sec).max(1));
        (refilled, RateDecision::Limited { retry_after })
    }
}

/// The networks whose connections are a proxy speaking for a client.
/// Loopback is always trusted (the host's own Caddy).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustedProxies(Vec<IpNetwork>);

impl TrustedProxies {
    /// Loopback only.
    #[must_use]
    pub fn loopback_only() -> Self {
        Self::default()
    }

    /// A comma- or whitespace-separated list of addresses or CIDR networks
    /// (`172.18.0.0/16`, `fd00::/8`, `10.0.0.5`). `Err` names the first entry
    /// that is neither.
    pub fn parse(list: &str) -> Result<Self, String> {
        list.split(|c: char| c == ',' || c.is_whitespace())
            .filter(|entry| !entry.is_empty())
            .map(|entry| IpNetwork::parse(entry).ok_or_else(|| entry.to_string()))
            .collect::<Result<Vec<_>, _>>()
            .map(Self)
    }

    /// Whether a connection from `peer` is a trusted proxy.
    #[must_use]
    pub fn trusts(&self, peer: IpAddr) -> bool {
        let peer = peer.to_canonical();
        peer.is_loopback() || self.0.iter().any(|network| network.contains(peer))
    }
}

/// An address or a CIDR network.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IpNetwork {
    address: IpAddr,
    prefix: u8,
}

impl IpNetwork {
    fn parse(entry: &str) -> Option<Self> {
        let (address, prefix) = match entry.split_once('/') {
            Some((address, prefix)) => (address, Some(prefix)),
            None => (entry, None),
        };
        let address: IpAddr = address.parse().ok()?;
        let width = Self::width(address);
        let prefix = match prefix {
            Some(prefix) => prefix.parse::<u8>().ok().filter(|p| *p <= width)?,
            None => width,
        };
        Some(Self { address, prefix })
    }

    const fn width(address: IpAddr) -> u8 {
        match address {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        }
    }

    fn contains(self, address: IpAddr) -> bool {
        let bits = |ip: IpAddr| match ip {
            IpAddr::V4(v4) => u128::from(u32::from(v4)) << 96,
            IpAddr::V6(v6) => u128::from(v6),
        };
        let same_family = matches!(
            (self.address, address),
            (IpAddr::V4(_), IpAddr::V4(_)) | (IpAddr::V6(_), IpAddr::V6(_))
        );
        let mask = u128::MAX
            .checked_shl(128 - u32::from(self.prefix))
            .unwrap_or(0);
        same_family && (bits(self.address) & mask) == (bits(address) & mask)
    }
}

/// The client a request is counted against: the address a trusted proxy
/// appended to `X-Forwarded-For` (its rightmost entry), else the peer. A
/// header from an untrusted peer, or one that does not end in an address, is
/// ignored.
#[must_use]
pub fn client_ip(peer: IpAddr, forwarded_for: Option<&str>, trusted: &TrustedProxies) -> IpAddr {
    let forwarded = || {
        forwarded_for?
            .rsplit(',')
            .next()
            .and_then(|entry| entry.trim().parse::<IpAddr>().ok())
    };
    let client = if trusted.trusts(peer) {
        forwarded().unwrap_or(peer)
    } else {
        peer
    };
    client.to_canonical()
}

/// One bucket per client, in a map of at most `capacity` entries.
#[derive(Debug)]
pub struct RateLimiter {
    limit: RateLimit,
    capacity: usize,
    buckets: Mutex<HashMap<IpAddr, TokenBucket>>,
}

impl RateLimiter {
    /// A limiter remembering at most [`MAX_TRACKED_CLIENTS`] clients.
    #[must_use]
    pub fn new(limit: RateLimit) -> Self {
        Self::with_capacity(limit, MAX_TRACKED_CLIENTS)
    }

    /// A limiter remembering at most `capacity` (≥ 1) clients.
    #[must_use]
    pub fn with_capacity(limit: RateLimit, capacity: usize) -> Self {
        Self {
            limit,
            capacity: capacity.max(1),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Admit or refuse one request from `client` at `now`.
    pub fn check(&self, client: IpAddr, now: Instant) -> RateDecision {
        let mut buckets = self.buckets.lock().unwrap_or_else(PoisonError::into_inner);
        if !buckets.contains_key(&client) && buckets.len() >= self.capacity {
            make_room(&mut buckets, self.capacity, now, self.limit);
        }
        let bucket = buckets
            .get(&client)
            .copied()
            .unwrap_or_else(|| TokenBucket::full(self.limit, now));
        let (after, decision) = take(bucket, now, self.limit);
        buckets.insert(client, after);
        decision
    }

    /// How many clients are remembered now.
    #[must_use]
    pub fn tracked_clients(&self) -> usize {
        self.buckets
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

/// Forget every bucket that has refilled (it is a fresh bucket again); when
/// every client is still active, forget the one idle longest.
fn make_room(
    buckets: &mut HashMap<IpAddr, TokenBucket>,
    capacity: usize,
    now: Instant,
    limit: RateLimit,
) {
    buckets.retain(|_, bucket| !bucket.is_full_at(now, limit));
    if buckets.len() >= capacity {
        let idlest = buckets
            .iter()
            .min_by_key(|(_, bucket)| bucket.refilled_at)
            .map(|(client, _)| *client);
        if let Some(client) = idlest {
            buckets.remove(&client);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn limit_strategy() -> impl Strategy<Value = RateLimit> {
        (1u32..=50, 1u32..=100)
            .prop_map(|(per_sec, burst)| RateLimit::new(per_sec, burst).expect("non-zero limit"))
    }

    /// Run `arrivals_ms` (offsets from a common start) through one bucket and
    /// return each decision.
    fn decisions(limit: RateLimit, arrivals_ms: &[u64]) -> Vec<RateDecision> {
        let start = Instant::now();
        let mut bucket = TokenBucket::full(limit, start);
        arrivals_ms
            .iter()
            .map(|offset| {
                let (after, decision) = take(bucket, start + Duration::from_millis(*offset), limit);
                bucket = after;
                decision
            })
            .collect()
    }

    fn admitted(decisions: &[RateDecision]) -> usize {
        decisions
            .iter()
            .filter(|d| **d == RateDecision::Admitted)
            .count()
    }

    #[test]
    fn the_default_limit_is_ten_a_second_in_bursts_of_fifty() {
        assert_eq!(RateLimit::DEFAULT.per_sec(), 10);
        assert_eq!(RateLimit::DEFAULT.burst(), 50);
        assert_eq!(RateLimit::new(0, 50), None);
        assert_eq!(RateLimit::new(10, 0), None);
    }

    proptest! {
        /// Oracle: at one instant a fresh bucket admits exactly `burst`
        /// requests, and refuses every one after, each with Retry-After 1 s.
        #[test]
        fn a_fresh_client_gets_exactly_its_burst_at_once(
            limit in limit_strategy(),
            extra in 1usize..20,
        ) {
            let burst = limit.burst() as usize;
            let all = decisions(limit, &vec![0; burst + extra]);
            prop_assert_eq!(admitted(&all), burst);
            prop_assert!(all[..burst].iter().all(|d| *d == RateDecision::Admitted));
            for refused in &all[burst..] {
                prop_assert_eq!(
                    *refused,
                    RateDecision::Limited { retry_after: Duration::from_secs(1) }
                );
            }
        }

        /// Whatever the arrival schedule, a client never gets more than its
        /// burst plus `per_sec` for every whole second the schedule spans.
        #[test]
        fn no_schedule_gets_more_than_the_burst_plus_the_rate(
            limit in limit_strategy(),
            mut arrivals in proptest::collection::vec(0u64..5_000, 1..400),
        ) {
            arrivals.sort_unstable();
            let span_ms = arrivals.last().copied().unwrap_or(0) - arrivals[0];
            let ceiling = limit.burst() as u64 + u64::from(limit.per_sec()) * span_ms / 1_000;
            let got = admitted(&decisions(limit, &arrivals)) as u64;
            prop_assert!(got <= ceiling, "admitted {} > ceiling {}", got, ceiling);
        }

        /// A client that keeps to the rate is never refused, however long it
        /// keeps going (no false limiting).
        #[test]
        fn a_client_keeping_to_the_rate_is_always_admitted(
            limit in limit_strategy(),
            requests in 1usize..300,
        ) {
            let spacing = 1_000u64.div_ceil(u64::from(limit.per_sec()));
            let arrivals: Vec<u64> = (0..requests as u64).map(|i| i * spacing).collect();
            prop_assert_eq!(admitted(&decisions(limit, &arrivals)), requests);
        }

        /// Waiting the Retry-After a refusal names is enough for one more.
        #[test]
        fn waiting_the_retry_after_admits_the_next_request(limit in limit_strategy()) {
            let start = Instant::now();
            let mut bucket = TokenBucket::full(limit, start);
            for _ in 0..limit.burst() {
                bucket = take(bucket, start, limit).0;
            }
            let (bucket, refused) = take(bucket, start, limit);
            let RateDecision::Limited { retry_after } = refused else {
                return Err(TestCaseError::fail("the burst is spent"));
            };
            prop_assert_eq!(take(bucket, start + retry_after, limit).1, RateDecision::Admitted);
        }

        /// The limiter remembers at most its capacity, whatever the clients.
        #[test]
        fn the_limiter_never_remembers_more_than_its_capacity(
            capacity in 1usize..64,
            clients in proptest::collection::vec(any::<u32>(), 1..400),
        ) {
            let limiter = RateLimiter::with_capacity(RateLimit::DEFAULT, capacity);
            let now = Instant::now();
            for client in clients {
                let _ = limiter.check(IpAddr::V4(Ipv4Addr::from(client)), now);
                prop_assert!(limiter.tracked_clients() <= capacity);
            }
        }

        /// An untrusted peer is its own client whatever it forwards; a trusted
        /// one speaks for the rightmost forwarded address.
        #[test]
        fn only_a_trusted_proxy_names_the_client(
            peer in any::<u32>(),
            forwarded in any::<u32>(),
            earlier in any::<u32>(),
        ) {
            let peer = IpAddr::V4(Ipv4Addr::from(peer));
            let forwarded = IpAddr::V4(Ipv4Addr::from(forwarded));
            let header = format!("{}, {forwarded}", Ipv4Addr::from(earlier));
            let caddy_net = TrustedProxies::parse("0.0.0.0/0").expect("network");
            let nobody = TrustedProxies::parse("").expect("empty list");

            prop_assert_eq!(client_ip(peer, Some(&header), &caddy_net), forwarded);
            prop_assert_eq!(
                client_ip(peer, Some(&header), &nobody),
                if peer.is_loopback() { forwarded } else { peer }
            );
            prop_assert_eq!(client_ip(peer, None, &caddy_net), peer);
            prop_assert_eq!(client_ip(peer, Some("not-an-address"), &caddy_net), peer);
        }
    }

    #[test]
    fn a_cidr_network_trusts_exactly_its_addresses() {
        let trusted = TrustedProxies::parse("172.18.0.0/16, fd00::/8 10.0.0.5").expect("list");
        let cases: [(IpAddr, bool); 8] = [
            (Ipv4Addr::new(172, 18, 0, 2).into(), true),
            (Ipv4Addr::new(172, 18, 255, 255).into(), true),
            (Ipv4Addr::new(172, 19, 0, 1).into(), false),
            (Ipv4Addr::new(10, 0, 0, 5).into(), true),
            (Ipv4Addr::new(10, 0, 0, 6).into(), false),
            ("fd12::1".parse::<Ipv6Addr>().expect("v6").into(), true),
            ("fe80::1".parse::<Ipv6Addr>().expect("v6").into(), false),
            (Ipv4Addr::LOCALHOST.into(), true),
        ];
        for (address, expected) in cases {
            assert_eq!(trusted.trusts(address), expected, "{address}");
        }
        assert_eq!(
            TrustedProxies::parse("172.18.0.0/33"),
            Err("172.18.0.0/33".to_string())
        );
        assert_eq!(TrustedProxies::parse("caddy"), Err("caddy".to_string()));
    }
}
