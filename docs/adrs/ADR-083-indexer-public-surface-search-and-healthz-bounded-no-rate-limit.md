# ADR-083: The Public Indexer Surface Is Search Plus `/healthz`, Bounded In-Process and at Caddy, With No Per-IP Rate Limit

- **Status**: Proposed (2026-10-06)
- **Date**: 2026-10-06
- **Deciders**: Morgan (nw-solution-architect); Jeff Bailey (public read-only search, 2026-10-06)
- **Feature**: indexer-deployment (DESIGN). Resolves OQ-IXD-5 (public freshness) and OQ-IXD-7 (rate limiting),
  and fixes the health endpoint that the liveness alarm probes.
- **Builds on**: ADR-027 (search XRPC method), ADR-075 (Caddy site via the import hook), ADR-080
- **Unchanged**: the CLI default `indexer_url` (WD-IXD-9 and the user decision). The `searchClaims`
  lexicon is not changed.

## Context

`https://index.openlore.jeffbailey.us` is public and read-only (WD-IXD-3). Today the query server:

- routes only `POST /xrpc/org.openlore.appview.searchClaims` (everything else is 404);
- reads the **whole** request body with no limit;
- returns every matching row;
- has no header-read timeout or connection cap;
- has no health route.

NFR-IXD-7 requires that a 100-request burst in 10 s from one client cannot fail the PDS health check or
push the indexer past its memory cap, and that request and result sizes are bounded. Per-IP rate
limiting in Caddy needs a non-stock build (`caddy-ratelimit`), and Caddy is owned by the shared
tofu-aws-pds module (ADR-066). The audience is under 50 users. The user decided a liveness alarm,
part of which is "the public health check fails".

## Decision

1. **Public routes (two layers).**
   - **Caddy** (`index.caddy`, a stock directive set) proxies only `POST /xrpc/org.openlore.appview.searchClaims`
     and `GET /healthz`. Every other method or path gets 404 from Caddy, which caps the request body at
     **8 KiB** (`request_body max_size`).
   - **The binary's public router** independently serves only those two routes. The control channel
     (ADR-080) is a Unix socket and is never on this router.
2. **`GET /healthz`.** Returns **200** once the store probe has passed and the listener is up, with the
   body `{"status":"ok","last_successful_pass_at":"<RFC3339>"|null}`. It returns **503**
   (`{"status":"store_unusable"}`) while the store is marked unusable (ADR-080 §7), so the liveness
   alarm and the deploy readiness check never see green on a broken store.
   **Search returns HTTP 500 on a store error.** It never returns an empty 200, because the CLI would
   show that as "no results".
   - The timestamp is the end of the last pass with exit 0. It is kept in memory and set by the pass
     runner, and it is `null` after a restart until the next good pass.
   - No counts and no DIDs. Freshness is public, as a single timestamp (OQ-IXD-5: Maria can see "results
     as of"). The operator's detailed freshness view comes from the logs, not from this endpoint (see
     `docs/feature/indexer-deployment/design/architecture-design.md` §6).
   - `deploy.sh` uses `/healthz` for readiness, and the liveness alarm probes it.
3. **In-process bounds** (the binary, independent of Caddy):

   | Bound | Value | Behavior |
   |---|---|---|
   | Request body | ≤ 8 KiB | 413 |
   | `value` length | ≤ 512 bytes | 400 |
   | Rows per response | ≤ 1000, applied in the store query (SQL `LIMIT`, never an aggregate) | Truncated. `indexer.search.truncated {dimension, cap}` is logged (no query value). |
   | Header read timeout | 10 s | Connection closed |
   | Concurrent connections | ≤ 64 | Further accepts wait |

   Search is already serialized on the store mutex, so a burst queues and cannot multiply memory.
4. **No per-IP rate limiting in v1 (OQ-IXD-7).** The bounds above, together with the container memory and
   CPU caps (DEVOPS), cap the damage of a burst to "slower search". Revisit when one of these holds:
   - search p95 exceeds 1 s under real traffic;
   - the PDS health check fails during a burst;
   - the module gains a rate-limit-capable Caddy.
5. **Responses carry no `Server` header** (Caddy `header -Server`). Proxy failures during a deploy return
   a short 503 (`handle_errors`), as on the review-app site.

## Alternatives considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **Caddy `rate_limit` (custom xcaddy build)** | It changes the module-owned Caddy image, which every module consumer (TRB) would carry, for under 50 users. | Rejected |
| **In-process per-IP token bucket** | It needs the client IP from `X-Forwarded-For`, which is trustable only behind Caddy, plus a map with eviction (a memory-growth surface of its own). Serialization on the store already bounds concurrency. | Deferred |
| **Cloudflare proxy (orange cloud) with rate limiting** | It changes the wildcard DNS posture (DNS-only today) and the TLS model shared with the PDS (on-demand TLS). Cloudflare is proprietary. | Rejected |
| **Health on a private port only** | The liveness alarm must see the public route (Caddy plus the container). A private health check misses a broken site block. | Rejected |
| **Freshness in the search response (lexicon field)** | It changes the lexicon for a deployment feature. `/healthz` gives the same information with no wire change. | Deferred |
| **Pagination instead of a row cap** | It is a lexicon change. At under 50 authors, real results are far below 1000, so the cap is a safety bound and not a UX feature. | Deferred |

## Consequences

- **Positive**:
  - Writes and other paths are refused at two independent layers (AC-001.3).
  - Request size, result size and connection count are bounded (NFR-IXD-7).
  - Public freshness costs no new infrastructure.
  - No change to the shared Caddy build.
- **Negative**:
  - A determined flood can make search slow for everyone, though not unsafe for the PDS, which is
    protected by container caps.
  - Hitting the row cap silently truncates for the client in v1, so it is logged for the operator.
- **Binary changes**: body limit, `value` length check, SQL row cap, header timeout, connection cap, the
  `/healthz` route with 503 on an unusable store, and search 500 on store errors. The search handler
  becomes fallible, so the `QueryHandler` signature changes. All of this is in
  `adapter-xrpc-query-server` and the indexer's search wiring.
