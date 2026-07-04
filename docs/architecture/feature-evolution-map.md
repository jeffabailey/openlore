# Feature Evolution Map

A single navigational entry point into OpenLore's delivered work: every shipped
slice, what it delivered, the architecture decisions that govern it, and the
per-feature evolution archive that records how it was built.

## How the record is organized

Truth about the system lives in three places, in decreasing stability:

| Layer | Location | What it holds |
|-------|----------|---------------|
| **Decisions** | [`docs/adrs/`](../adrs/) | ADRs — one architectural decision each, immutable once accepted. |
| **Evolution archives** | [`docs/evolution/`](../evolution/) | One per shipped feature/slice: wave timeline, gates, mutation report, lessons, deviations. |
| **This map** | here | The index tying slices → the ADRs that govern them → their evolution archive. |

Read a row's ADRs for *why the design is the way it is*; read its evolution
archive for *how the slice was built and what was learned*.

## Cross-cutting foundation

[ADR-007 — functional paradigm (pure claim-domain core, effect shell at I/O
edges)](../adrs/ADR-007-paradigm-functional-rust.md) governs **every** slice and
is omitted from the per-slice "Key ADRs" column below to avoid repetition. The
hexagonal/modular-monolith structure is
[ADR-009](../adrs/ADR-009-architecture-style-hexagonal-modular-monolith.md); the
21-member workspace boundary is enforced by `xtask check-arch`.

## The slices

The product was delivered as 20 vertical slices: slices 1–5 built the CLI +
federation + indexer core (jobs J-001…J-005); slices 6–20 built the read-only
`openlore ui` browser viewer on top of that core.

| # | Feature | What shipped | Key ADRs | Evolution archive |
|---|---------|--------------|----------|-------------------|
| 01 | openlore-foundation | Walking skeleton: author + sign + locally persist a claim | 001–012 | [archive](../evolution/openlore-foundation-evolution.md) |
| 02 | openlore-github-scraper | GitHub scrape → propose candidate claims → human signs | 017–019 | [archive](../evolution/openlore-github-scraper-evolution.md) |
| 03 | openlore-federated-read | Subscribe to / pull a peer's claims, attributed, revocably | 013–015 | [archive](../evolution/openlore-federated-read-evolution.md) |
| 04 | openlore-scoring-graph | Transparent adherence scoring + local graph explorer | 020–022 | [archive](../evolution/openlore-scoring-graph-evolution.md) |
| 05 | openlore-appview-search | Network indexer: verify + attribute + search public claims | 023–027 | [archive](../evolution/openlore-appview-search-evolution.md) |
| 06 | htmx-scraper-viewer | Read-only localhost htmx store viewer (`openlore ui`) | 028–030 | [archive](../evolution/htmx-scraper-viewer-evolution.md) |
| 07 | viewer-htmx-swaps | htmx partial-swaps as progressive enhancement | 031–035 | [archive](../evolution/viewer-htmx-swaps-evolution.md) |
| 08 | viewer-network-search | `/search` network-discovery view | 036–038 | [archive](../evolution/viewer-network-search-evolution.md) |
| 09 | viewer-contributor-scoring | `/score?contributor=` transparent scoring view | 039–041 | [archive](../evolution/viewer-contributor-scoring-evolution.md) |
| 10 | viewer-graph-traversal | `/project` + `/philosophy` edge-survey views | 042–045 | [archive](../evolution/viewer-graph-traversal-evolution.md) |
| 11 | viewer-counter-claim-threads | `/claims/{cid}` counter-claim threads | 046–047 | [archive](../evolution/viewer-counter-claim-threads-evolution.md) |
| 12 | viewer-counter-claim-list-flags | "Countered" presence flag on the `/claims` list | 048 | [archive](../evolution/viewer-counter-claim-list-flags-evolution.md) |
| 13 | viewer-counter-flags-graph-surfaces | Countered flag on `/peer-claims`, `/project`, `/philosophy` | 049–050 | [archive](../evolution/viewer-counter-flags-graph-surfaces-evolution.md) |
| 14 | viewer-counter-flags-score-surface | Countered flag on the `/score` surface | 051 | [archive](../evolution/viewer-counter-flags-score-surface-evolution.md) |
| 15 | viewer-peer-subscriptions | `/peers` federation-management view | 052 | [archive](../evolution/viewer-peer-subscriptions-evolution.md) |
| 16 | viewer-search-follow-state | Per-result follow-state on `/search` (binary resolution) | 053 | [archive](../evolution/viewer-search-follow-state-evolution.md) |
| 17 | viewer-landing-dashboard | `GET /` landing: nav hub + LOCAL store summary | 054 | [archive](../evolution/viewer-landing-dashboard-evolution.md) |
| 18 | viewer-counter-aware-counts | Countered-own-claims count on `/` + `/claims` header | 055 | [archive](../evolution/viewer-counter-aware-counts-evolution.md) |
| 19 | viewer-peer-counter-aware-counts | Countered-peer-claims count on `/` + `/peer-claims` header | 056 | [archive](../evolution/viewer-peer-counter-aware-counts-evolution.md) |
| 20 | viewer-search-full-follow-state | Completes the four-arm `AuthorRelationship` on `/search` | 057 | [archive](../evolution/viewer-search-full-follow-state-evolution.md) |

## Reading order for a newcomer

1. Start with the **jobs** ([`docs/product/jobs.yaml`](../product/jobs.yaml)) — the five JTBD the product serves.
2. Read the **foundation** archive (slice-01) for the walking skeleton and the pure-core/effect-shell split.
3. Follow the slice you care about across this table into its **ADRs** (why) and **evolution archive** (how).

> Maintenance note: when a new feature ships, add its row here alongside its
> `docs/evolution/*.md` archive so this map stays the current index of delivered
> work.
