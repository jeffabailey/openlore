# ADR-077: Indexer Reads Each Repo DID From Its Freshly Resolved PDS; the Source URL Becomes a Relay-Origin Fallback

- **Status**: Accepted (2026-10-05) — implemented; see `docs/evolution/indexer-per-did-pds-fetch-evolution.md` (proposed 2026-10-05)
- **Date**: 2026-10-05
- **Deciders**: Morgan (nw-solution-architect); user decision WD-IPF-2 (wizard)
- **Feature**: indexer-per-did-pds-fetch (DESIGN)
- **Supersedes in part**: ADR-024 §"Bounded source-discovery strategy" (the source set is now
  per-DID resolved PDSes plus an optional fallback, not "seed DIDs listed from one source") and
  ADR-024 §"Earned Trust" item 1 (no startup refusal on source reachability). ADR-071 §4, the
  indexer bullet ("the indexer's ingest source is an operator-configured URL...").
- **Affirms**: ADR-016 (fresh resolution on every pull), ADR-071 §4 rule (origin computed by
  exact match, never assumed), ADR-025 (attribution by repo DID), ADR-023 (read-only,
  signing-incapable).

## Context

The indexer lists every configured repo DID from one `OPENLORE_INDEXER_SOURCE_URL`. ADR-071 §4
admits a self-attested record only when the URL it was read from equals the PDS freshly resolved
from the DID document. The origin computation is correct, but with one source URL only DIDs
hosted there can ever be `AuthorPds`. Self-attested claims of review-app users on bsky.social
hosts are refused (`UnverifiableProvenance`) and never reach search. `peer pull` and the review
app already read from the resolved PDS. The indexer is the odd one out.

The existing `IdentityLookup` adapter resolves `did:plc` (PLC directory) and `did:web`
(`https://<host>/.well-known/did.json`). Its `resolve_did` also does a handle round-trip, which
the origin decision does not need. In the indexer that round-trip is wired against the PLC
endpoint, so it costs a wasted request per DID and adds a spurious failure mode. It also returns
`NotFound` for a document without an `at://` alias.

## Decision

1. **Per-DID, per-pass resolution.** Every pass resolves each configured repo DID to its DID
   document's `#atproto_pds` service. It uses a new **default-implemented**
   `IdentityLookupPort::resolve_pds` method, which `IdentityLookup` overrides with a
   document-only fetch plus a pure `pds_endpoint_of(did, document)` that requires
   `id == did`. The DID is listed (`listRecords`, `repo=<DID>`, the existing `take_page` bound)
   at that endpoint. There is no cache: a moved PDS is followed on the next pass (FR-2).
2. **Typed listing source.** The pure core gains `ListingSource = OwnPds(PdsEndpoint) |
   Fallback(FallbackUrl)` and `origin_of(&ListingSource, fetched_from) -> RecordOrigin`:
   - `OwnPds(e)` → `RecordOrigin::of(fetched_from, e)`. The exact match is still recomputed, so
     the adapter's `fetched_from` is checked, not trusted.
   - `Fallback(_)` → `Relay`, **with no URL comparison**. A fallback URL that happens to equal
     some DID's PDS cannot promote anything, because a resolved DID never gets a `Fallback`
     source (OQ-IPF-3).
3. **The source URL becomes an optional fallback.** `OPENLORE_INDEXER_SOURCE_URL` keeps its
   name, so existing deployments need no edit. It is used **only** for DIDs whose resolution
   failed (`NotFound`, `Unavailable`, `TimedOut`). Records read through it are
   relay origin: app-signed records go through the unchanged verify gate, and self-attested ones
   are refused (`provenance`). When it is unset, unresolvable DIDs are skipped
   (`did_unresolvable`, ADR-078).
4. **SSRF guard (user decision 2026-10-05).** A DID document's controller chooses the PDS URL
   the indexer fetches, so every outbound DID-document, PDS and fallback fetch goes through an
   **address-guarded client**:
   - **Scheme**: `https` only. No userinfo. Redirects disabled (`reqwest` redirect policy none).
   - **Refused addresses**: 0.0.0.0/8, 127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16,
     169.254.0.0/16, `::`, `::1`, fc00::/7, fe80::/10. IPv4-mapped IPv6 is unwrapped before
     the check. One pure classifier, `ports::net_policy::address_refused(IpAddr)`, is shared by
     both adapters.
   - **After DNS, rebinding-safe**: the adapters install a custom `reqwest` DNS resolver
     (`ClientBuilder::dns_resolver`). It resolves the host, drops refused addresses, and hands
     only the checked addresses to the connector. The connection is made to the address that was
     checked, so there is no window for a second resolution to rebind. If no admissible address
     remains, the fetch fails with `IngestError::AddressRefused`. IP-literal hosts never reach
     the resolver, so a pure pre-check refuses them.
   - **Outcome**: a resolved PDS that is refused (pre-check or DNS) skips the DID with
     **`pds_address_refused`**. This is **not** fallback-eligible, because the DID resolved
     (AC-003.2). The fallback URL is checked at startup (scheme, literal IP: refuse start) and at
     runtime (DNS: `fallback_failure: pds_address_refused`). A refused `did:web` document host
     is a resolution failure (`did_unresolvable`).
   - **Test-only override**: `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1` admits `http` and
     loopback addresses (127.0.0.0/8, `::1`) **only**. Private and link-local ranges stay
     refused. `parse_config` takes a `BuildProfile` derived from `cfg!(debug_assertions)`; a
     release build with the variable set refuses start (`IndexerConfigInvalid`). This mirrors
     `REVIEW_APP_ALLOW_LOOPBACK_HTTP` in `openlore-review-app/src/config.rs`. The review app's
     own `IdentityLookup` construction is unchanged (unguarded); the indexer builds its
     instances with the policy.
5. **DID methods.** `did:plc` and hostname-only `did:web` (the only forms ATProto permits) are
   supported. Any other method, or a malformed DID, refuses start as a config error naming
   `OPENLORE_INDEXER_REPO_DIDS` and the bad entry.
6. **Startup.** No network reachability probe of PLC, PDSes or the fallback. These are per-pass,
   per-DID runtime conditions governed by ADR-078 isolation, and refusing start on a third-party
   outage would turn a per-source fault into a total outage. The `IngestSourcePort` probe no
   longer refuses an empty source. A pure in-process `origin_classification_probe` asserts the
   classifier's arms at startup and refuses with `IndexerOriginClassificationUnsound`.
7. **Enforcement.** The new `xtask check-arch` rule `indexer_origin_only_via_listing_source`
   bans `RecordOrigin::of` and `RecordOrigin::AuthorPds` tokens in `crates/openlore-indexer`
   non-test source.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| **Keep one source; operator points it at a relay** | Rejected. A relay is never the author's PDS, so all self-attested claims stay refused (ADR-071 §4). It needs commit-proof verification first (ADR-071 revisit trigger, SPIKE-5). |
| **Per-DID configured source URLs (`did=url` pairs)** | Rejected. Operator-asserted origin is exactly what ADR-071 forbids ("computed, never assumed"). It is also stale when an author migrates PDS. |
| **Cache resolved PDS endpoints across passes (TTL)** | Rejected for now. It contradicts FR-2 (a moved PDS is followed next pass) and ADR-016 freshness. At under 50 DIDs, one PLC request per DID per pass is negligible. |
| **Reuse `resolve_did` unchanged (zero port change)** | Rejected narrowly. It works, but it costs an extra request per DID against an endpoint that cannot answer `resolveHandle`, adds an `Unavailable` failure mode unrelated to origin, and rejects alias-less documents. The default method keeps every other impl untouched. |
| **Fallback classified by URL comparison (`RecordOrigin::of(fetched_from, resolved)`)** | Rejected. With no resolved PDS there is nothing to compare, and a URL coincidence would be a latent promotion path. The ADT removes the question. |
| **SSRF: check only literal IPs, or check the DNS answer and then let reqwest resolve again** | Rejected. A literal-only check misses hostnames pointing at private ranges. Check-then-resolve has a DNS-rebinding window. The guarded resolver checks the exact addresses that get connected to. |
| **SSRF: egress firewall / proxy only** | Rejected as the sole control. It is an ops dependency outside the binary, and tests could not prove it. It can be layered on later. |
| **Also fall back when the resolved PDS is down** | Rejected. AC-003.2 says the fallback is never contacted for a resolved DID. It would also hide own-PDS outages behind relay-origin reads that silently drop self-attested claims. |

## Consequences

- **Positive**: self-attested claims from any author's own PDS are indexed (KPI J-005).
  App-signed indexing is unchanged. The fallback can never weaken provenance, by type. One
  request less per DID. `did:web` comes for free.
- **Negative**: the indexer now contacts many third-party PDS hosts (R-IPF-1). This is bounded
  by ADR-078. Plain-`http` PDSes and privately addressed fallbacks are not supported in
  production (deliberate). A PLC endpoint configured on a private address would fail every
  resolution under the guard. Self-attested trust remains TLS + DID resolution + WebPKI (ADR-071 negative,
  unchanged).
- **Regression contract (NFR-3)**: a DID hosted on the old source URL resolves to that same
  base, so it gets the same records and the same origin. When PLC is down, the fallback
  reproduces the old outcome.
- **Revisit trigger**: the indexer ingests from a relay or firehose as the primary path, which
  requires commit-proof verification (ADR-071 trigger). Or the DID count grows past about 500,
  at which point consider resolution caching with a short TTL.
