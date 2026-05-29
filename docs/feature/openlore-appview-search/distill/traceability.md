# Traceability — openlore-appview-search (slice-05) — DISTILL

- **Wave**: DISTILL
- **Date**: 2026-05-28
- **Acceptance Designer**: Quinn (nw-acceptance-designer)

Every slice-05 acceptance scenario traces: scenario → user story → job (J-005 +
sub-jobs) → WD/OD lock → ADR → I-AV invariant → journey step → release gate →
KPI. The 37 scenarios live in `tests/acceptance/{indexer_ingest,appview_search,appview_core}.rs`.

---

## 1. Story → scenario coverage (every story has ≥1 scenario)

| Story | Title | Scenarios | Release |
|---|---|---|---|
| US-AV-001 | Bootstrap the indexer + verified, attributed ingest (`@infrastructure`) | AV-1, AV-2, AV-3, AV-4, AV-5, AV-6, AV-7 + (pure) AVC-1, AVC-3a, AVC-4 | R1 (walking skeleton) |
| US-AV-002 | Search by philosophy (object) at network scale, attributed | AV-8, AV-9, AV-12, AV-13, AV-14, AV-25 + (pure) AVC-2, AVC-3b, AVC-5, AVC-8 | R1 (walking skeleton) |
| US-AV-003 | Search by contributor / subject at network scale | AV-15, AV-16, AV-17, AV-18 | R2 |
| US-AV-004 | Trust a result — `[verified]` + `--show` + public-data banner | AV-10, AV-11, AV-23, AV-24 + (pure) AVC-7 | R1 (walking skeleton) |
| US-AV-005 | Subscribe to a discovered author (discovery → federation) | AV-19, AV-20, AV-21, AV-22 | R2 |
| US-AV-006 | Share a network search result as a stable link | AV-26, AV-27, AV-28, AV-29 | R3 |

Zero stories with zero scenarios. (Check A — Story-to-Scenario mapping: PASS.)

## 2. Walking-skeleton subset (Release 1 — story-map.md)

The story-map walking skeleton = US-AV-001 (indexer + verified ingest) +
US-AV-002 (search by philosophy) + US-AV-004 (trust marker / `--show` /
public-data banner). The thinnest "trustworthy network discovery by philosophy"
slice, demo-able end-to-end:

| WS beat | Scenario | Tag |
|---|---|---|
| 1. Index the network (verified, attributed) | **AV-1** `indexer_ingests_a_verified_attributed_claim_and_it_becomes_searchable` | `@walking_skeleton @real-io @driving_port` |
| 2. Search by philosophy (surfaces unfollowed authors, attributed) | **AV-8** `search_by_object_surfaces_verified_claims_by_unfollowed_authors_attributed` | `@walking_skeleton @real-io @driving_port` |
| 3. Trust the result (`[verified]` + `--show` + banner) | AV-23 (`--show`) + AV-10 (banner) + AV-11 (`[verified]`) | `@release-gate` |

The end-to-end WS driving path (demo gate, story-map Phase 3.5): seed a verified
attributed claim by an UNFOLLOWED author into the index (AV-1, via real
`openlore-indexer ingest` over `FakeIngestSource` + the real-`z6Mk` resolver) →
`openlore search --object org.openlore.philosophy.reproducible-builds` surfaces it
attributed + `[verified]` + `(not subscribed)` (AV-8) → `--show <cid>` confirms
"Signature: VERIFIED against <did>" + "CID recomputed, matches published record"
(AV-23) → the public-data banner precedes results (AV-10) → compose/sign still
succeed network-disabled (AV-13). Two `@walking_skeleton`-tagged beats (AV-1
ingest, AV-8 search) close the loop; the trust beats are release gates riding the
same WS corpus.

## 3. Cardinal release gate → invariant → KPI

| Release gate (named scenario) | Invariant asserted | I-AV | KPI | Release-blocking? |
|---|---|---|---|---|
| `indexer_rejects_unverified_claim` (AV-3 + AVC-1) | unsigned/tampered/cid-mismatch REJECTED at ingest; never indexed/searchable; valid one IS; reuses `claim_domain::verify` (no 2nd path) | I-AV-1 | **KPI-AV-3** | YES |
| `network_result_preserves_attribution` (AV-9 + AVC-2/AVC-4/AVC-5) | two distinct-author same-(subject,object) claims → TWO attributed rows; NO merged/consensus row; mean/aggregate never a row | I-AV-2 | **KPI-AV-2** | YES |
| `local_first_preserved` (AV-13) | indexer down + network disabled → `claim add`/offline publish/`graph query` all succeed; `search` soft local-only msg + non-fatal, no hang | I-AV-3 | **KPI-5** | YES |
| `public_data_banner_shown` (AV-10) | a banner states public-signed-only + verified-before-indexing + nothing-private | I-AV-4 | KPI-AV-5 | (guardrail render) |
| `verified_marker_is_universal` (AV-11 + AVC-7) | every returned claim shows `[verified]`; no `[unverified]` state exists by construction | I-AV-1 | KPI-AV-3 | (construction) |
| `search_succeeds_with_indexer_localhost` (AV-14) | CLI reaches a real localhost indexer; wire carries per-result `author_did`; attributed verified results render | (B1) | KPI-AV-2 / WD-115 | (B1 transport) |
| `discovery_follow_reuses_slice03_path` (AV-19) | follow affordance reuses slice-03 `peer add` verbatim; after add+pull the author's claims in LOCAL graph; no parallel path; no auto-follow | I-AV-7 | KPI-AV-4 | (funnel) |
| `share_link_encodes_query_not_snapshot` (AV-26/AV-28) | `--share` emits a stable query-encoding link; opening re-runs → current per-author verified results; never a stored merged snapshot | I-AV-8 | KPI-AV-6 | (share) |
| `countered_claim_still_appears` (AV-25 + AVC-6) | countered/retracted public verified claim still discoverable; counter shown, never silently filtered/down-weighted | I-AV-9 | (OD-AV-7) | (counter) |
| (capability boundary) `indexer signing-incapable + no local store` (AV-5) | the indexer cannot author/sign/publish + cannot touch `openlore.duckdb` | I-AV-5 | (structural) | (capability) |
| (production decode is real) (AV-4) | production verification resolves + decodes the author's REAL PLC `z6Mk` key; the seam is release-forbidden | I-AV-6 | KPI-AV-3 (vs real data) | (gold path) |

Every I-AV-1..9 invariant has ≥1 acceptance scenario asserting its behavioral
layer. (I-AV-5/I-AV-6's type + structural layers are DELIVER's xtask/type concern;
AV-5/AV-4 are their behavioral layers.)

## 4. Scenario → story → job → WD/OD → ADR → I-AV → journey step → KPI (full grid)

| Scenario | Story | Job | WD/OD lock | ADR | I-AV | Journey step | KPI |
|---|---|---|---|---|---|---|---|
| AV-1 | US-AV-001 | J-005b | WD-104, WD-103 | 023/024/025/026 | I-AV-1, I-AV-2 | Index the network | KPI-AV-3 |
| AV-2 | US-AV-001 | J-005b | WD-103 | 025 | I-AV-2 | Index the network | KPI-AV-2 |
| AV-3 | US-AV-001 | J-005b | WD-104 | 024/026 | I-AV-1 | Verify before index | **KPI-AV-3** |
| AV-4 | US-AV-001 | J-005b | WD-118 | 026 | I-AV-6 | Verify before index | KPI-AV-3 (real) |
| AV-5 | US-AV-001 | J-005b | WD-112 | 023 | I-AV-5 | Index the network | (structural) |
| AV-6 | US-AV-001 | J-005b | WD-112 | 009/023 | (probe) | Index the network | (startup) |
| AV-7 | US-AV-001 | J-005b | WD-105 | 014/024 | I-AV-4 | Index the network | KPI-AV-5 |
| AV-8 | US-AV-002 | J-005a | WD-109, WD-103 | 027 | I-AV-2 | Search by philosophy | KPI-AV-1, KPI-AV-2 |
| AV-9 | US-AV-002 | J-005a | WD-103 | 025/027 | I-AV-2 | (Anti-merging at scale) | **KPI-AV-2** |
| AV-10 | US-AV-004 | J-005b | WD-105 | 014 | I-AV-4 | Public-data banner up front | KPI-AV-5 |
| AV-11 | US-AV-004 | J-005b | WD-104 | 024/026 | I-AV-1 | `[verified]` on every result | KPI-AV-3 |
| AV-12 | US-AV-002 | J-005a | WD-109 | 027 | — | Search by philosophy | (empty/exit-0) |
| AV-13 | US-AV-002 | J-005a | WD-106 | 027 | I-AV-3 | (Local-first preserved) | **KPI-5** |
| AV-14 | US-AV-002 | J-005a | WD-115 | 027 | (B1) | Search by philosophy | KPI-AV-2 |
| AV-15 | US-AV-003 | J-005a | WD-109 | 027 | I-AV-2 | Search by contributor | KPI-AV-1, KPI-AV-4 |
| AV-16 | US-AV-003 | J-005a | WD-109, WD-103 | 027 | I-AV-2 | Search by subject | KPI-AV-2 |
| AV-17 | US-AV-003 | J-005a | WD-109 | 027 | — | Search by contributor | (empty/exit-0) |
| AV-18 | US-AV-003 | J-005a | WD-110 | 013/027 | I-AV-7 | Search by contributor | KPI-AV-4 |
| AV-19 | US-AV-005 | J-005c | WD-110 | 013/027 | I-AV-7 | Follow a discovered author | **KPI-AV-4** |
| AV-20 | US-AV-005 | J-005c | WD-110 | 027 | I-AV-7 | Follow a discovered author | KPI-AV-4 |
| AV-21 | US-AV-005 | J-005c | WD-110 | 027 | I-AV-7 | Follow a discovered author | KPI-AV-4 |
| AV-22 | US-AV-005 | J-005c | WD-110, WD-22/I-FED-5 | 013/027 | I-AV-7 | Follow a discovered author | KPI-AV-4 |
| AV-23 | US-AV-004 | J-005b | WD-104, KPI-4 | 026/027 | I-AV-1 | `--show` signature + CID match | KPI-AV-3, KPI-4 |
| AV-24 | US-AV-004 | J-005b | WD-109 | 027 | — | `--show` | (usage error) |
| AV-25 | US-AV-002 | J-005a | WD-119/OD-AV-7 | 025 | I-AV-9 | Search by philosophy | (counter) |
| AV-26 | US-AV-006 | J-005a | WD-110 | 027 | I-AV-8 | Share a result | KPI-AV-6 |
| AV-27 | US-AV-006 | J-005a | WD-110, WD-103 | 027 | I-AV-8, I-AV-2 | Share a result | KPI-AV-6, KPI-AV-2 |
| AV-28 | US-AV-006 | J-005a | WD-110 | 027 | I-AV-8 | Share a result | KPI-AV-6 |
| AV-29 | US-AV-006 | J-005a | WD-110 | 027 | I-AV-8 | Share a result | KPI-AV-6 |
| AVC-1 | US-AV-001/004 | J-005b | WD-104, WD-121 | 024/026 | I-AV-1 | Verify before index | **KPI-AV-3** |
| AVC-2 | US-AV-002/003/006 | J-005a | WD-103, WD-120 | 025 | I-AV-2 | (Anti-merging at scale) | **KPI-AV-2** |
| AVC-3a | US-AV-001 | J-005b | (DESIGN 5.1 inv 4) | 024 | I-AV-1 | Verify before index | KPI-AV-3 |
| AVC-3b | US-AV-002/006 | J-005a | (DESIGN 5.1 inv 4), WD-110 | 027 | I-AV-8 | Share a result | KPI-AV-6 |
| AVC-4 | US-AV-001 | J-005b | WD-103 | 025 | I-AV-2 | Index the network | KPI-AV-2 |
| AVC-5 | US-AV-002 | J-005a | WD-103 | 025 | I-AV-2 | (Anti-merging at scale) | KPI-AV-2 |
| AVC-6 | US-AV-002 | J-005a | WD-119/OD-AV-7 | 025 | I-AV-9 | Search by philosophy | (counter) |
| AVC-7 | US-AV-004 | J-005b | WD-104 | 026 | I-AV-1 | `[verified]` on every result | KPI-AV-3 |
| AVC-8 | US-AV-002 | J-005a | WD-109 | 027 | — | Search by philosophy | (empty/suggestion) |

## 5. Environment-to-scenario mapping (DEVOPS environment matrix; Check B)

The DEVOPS environment matrix (D-D9 extended): clean | with-pre-commit |
with-stale-config | indexer-in-container | localhost-transport. The slice-05
walking-skeleton + degradation scenarios reference each:

| Environment | Scenario(s) referencing its preconditions |
|---|---|
| clean | AV-1, AV-8 (the WS beats over a fresh index + fresh HOME) |
| localhost-transport | AV-14 (REAL `openlore-indexer serve` over localhost ephemeral port — the B1 transport), every AV-8.. query backend |
| indexer-in-container (the container-substrate fsync lie) | AV-6 (the index-store fsync-honesty probe drives the startup refusal — the container-substrate-durability lie, DESIGN §6.3) |
| (indexer unreachable + network disabled) | AV-13 (`local_first_preserved` — the soft-degradation env) |
| with-pre-commit / with-stale-config | inherited slice-01..04 CLI environments (the `claim add` / `graph query` legs of AV-13 + AV-19 exercise the existing local environments) |

Every DEVOPS environment has ≥1 scenario referencing its preconditions. (Check B —
Environment-to-Scenario mapping: PASS. The container-substrate + localhost-transport
environments are slice-05-new; the clean/pre-commit/stale-config are inherited.)

## 6. Contract-boundary → scenario mapping (WD-123 / DEVOPS D-D36/D-D37)

| Contract boundary | Consumer | Scenario(s) pinning the shape |
|---|---|---|
| B1: CLI↔indexer XRPC (`org.openlore.appview.searchClaims`) | the `openlore` CLI | AV-14 (CLI reaches real serve, wire carries `author_did`), AV-9 (the per-result `author_did` render), every AV-8.. |
| B2: indexer→network-PDS `listRecords` + PLC DID-document | the `openlore-indexer` | AV-3 (the adversarial record set shapes), AV-4 (the PLC `z6Mk` DID-doc shape), AV-7 (the public `listRecords` shape) |

The DISTILL hermetic fixtures (`FakeIngestSource` records + the real-`z6Mk`
DID-doc fixture) MODEL the B2 shapes the DEVOPS Pact suite (D-D37) pins; the B1
shape (per-result `author_did`) is asserted at the wire by AV-14 + by the DEVOPS
Pact suite (D-D36). The AT and the contract test are complementary (the AT proves
the CLI driving port reaches the real server + renders; the Pact pins the wire DTO).

## 7. Coverage completeness summary

- **Story coverage**: 6/6 stories have ≥1 scenario (Check A PASS).
- **Environment coverage**: 5/5 DEVOPS environments referenced (Check B PASS).
- **Invariant coverage**: I-AV-1..9 each have ≥1 behavioral scenario.
- **Release-gate coverage**: all 8 cardinal gates + the funnel/share/counter
  gates authored + load-bearing (§3).
- **KPI coverage**: KPI-AV-1..6 + inherited KPI-5/KPI-4 mapped (acceptance
  assertion where the behavior is testable; telemetry where the rate is a
  production-cohort measure — §8 of acceptance-tests.md).
- **`# DISTILL: confirm` resolution**: all four flags resolved + bound (§2 of
  acceptance-tests.md).
