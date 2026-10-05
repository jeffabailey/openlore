# Technology stack: indexer-per-did-pds-fetch

**No new crate enters the dependency tree.** One crate that is already transitive becomes a
direct dependency of `openlore-indexer`.

| Need | Choice | License | Status in tree | Rationale |
|---|---|---|---|---|
| Bounded concurrent fan-out | `futures-util` 0.3 (`StreamExt::buffered`) | MIT OR Apache-2.0 | **Already in `Cargo.lock`** (0.3.32, via reqwest/hyper, with `futures-macro` etc.) | Works with borrowed `&dyn Port` futures on the existing current-thread runtime: no `'static`, no spawn, no `Arc`-wrapping of the wiring. `buffered` keeps the configured order, which gives deterministic event order. Declare it with `default-features = false, features = ["alloc"]` (or `std`) to keep the feature surface minimal. |
| Per-DID deadline | `tokio::time::timeout_at` | MIT | Present. The workspace tokio already enables `time` (`Cargo.toml:73`), and crate features add to it. | No new feature flag. |
| HTTP | workspace `reqwest` (rustls) | MIT/Apache-2.0 | Present | Unchanged (ADR-004). |
| SSRF guard after DNS (rebinding-safe) | `reqwest::ClientBuilder::dns_resolver` with a custom `reqwest::dns::Resolve` impl that wraps `tokio::net::lookup_host` and filters with `ports::net_policy::address_refused`. Plus `redirect::Policy::none()`. | MIT/Apache-2.0 | Present: reqwest core API, no feature flag; tokio `net` is already enabled in the tree. The two adapters may need a direct `tokio` dep with `net`. | It checks exactly the addresses the connector uses, so there is no rebinding window. No new crate (for example no `hickory`, which `deny.toml` constrains). |
| URL / IP parsing for config and endpoint admissibility | `url` (already used by both adapters), `std::net::IpAddr` | MIT/Apache-2.0 | Present | `url` lives in the adapters and the indexer root. The pure `pds_endpoint_admissible` in `appview-domain` and `ports::net_policy` are written against `std::net` and string parsing so the pure core gains no dependency. If the crafter prefers `url` there, it must be added to the `appview-domain` pure-core allowlist. Avoiding that is the recommended path. |

## Rejected alternatives for fan-out

| Option | Why rejected |
|---|---|
| Sequential (cap 1 only) | Simplest. But with 14–50 DIDs, one slow PDS adds up to the full budget per DID serially (50 × 30 s = 25 min worst case). Cap 1 stays **allowed** through the env var. |
| `tokio::task::JoinSet` + `Semaphore` | Needs `'static + Send` tasks, so the wiring would have to be `Arc`-wrapped and the `sync` feature added. That is more change for the same result at this scale. |
| A job queue / worker pool / new infra | Out of proportion for fewer than 50 authors and a solo maintainer (time-to-market, ops cost). |
| `futures` (facade crate) | Not in the tree, and adds `futures-executor`. `futures-util` is enough. |

## cargo deny implications

- Licenses: `futures-util` is MIT OR Apache-2.0, which is within the existing allowlist. No
  change to `deny.toml`.
- Bans: none affected (no openssl, axum or hickory involvement).
- Advisories / sources: crates.io only. The version is already resolved in `Cargo.lock`, so the
  advisory surface is unchanged.
