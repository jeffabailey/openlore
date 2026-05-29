# Acceptance Test Design — openlore-appview-search (slice-05)

- **Wave**: DISTILL
- **Date**: 2026-05-28
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Feature**: openlore-appview-search (sibling feature; slice-05; the FINAL umbrella slice)
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (per ADR-007)
- **Inherits**: slice-01/02/03/04 DISTILL artifacts + slice-05 DISCUSS WD-100..WD-110 + OD-AV-1..7 + slice-05 DESIGN WD-111..WD-124 + ADR-023..ADR-027 + I-AV-1..9 + slice-05 DEVOPS D-D35..D-D43
- **Language**: Rust (per ADR-009; `[lang-mode] rust`)
- **Test framework**: same as slice-01/02/03/04 — Rust std `#[test]` + `proptest` for `@property` (per DD-AV-1)

This document is the human-readable map over the executable test skeletons in
`tests/acceptance/appview_search.rs` (layer 3, subprocess — US-AV-002..006 + the
cardinal search-side gates), `tests/acceptance/indexer_ingest.rs` (layer 3,
subprocess — US-AV-001 infra + the ingest-side cardinal gate), and
`tests/acceptance/appview_core.rs` (layer 2, pure-core — the
`appview-domain` verify-gate + anti-merging-composition property suite). The
`.rs` files are the SSOT for executable scenarios.

---

## 1. Wave-Decision Reconciliation result

**Reconciliation passed — 0 contradictions** between DISCUSS (WD-100..WD-110 +
OD-AV-1..7), DESIGN (WD-111..WD-124, ADR-023..027, I-AV-1..9), and DEVOPS
(D-D35..D-D43). The cross-wave matrix:

| DISCUSS commitment | DESIGN resolution | DEVOPS consequence | Verdict |
|---|---|---|---|
| WD-103 anti-merging at network scale | WD-120 three-layer enforcement; pure-core aggregation | D-D36 wire `author_did` per result; `indexer_query_attribution_missing_total` counter | CONSISTENT |
| WD-104 verify-before-index | WD-121 reuse pure core; `verified_against NOT NULL` | D-D35 `at-indexer-rejects-unverified-claim` gate; D-D37 PLC contract | CONSISTENT |
| WD-105 public-data-only framing | I-AV-4 public-data banner + public listRecords only | no claim-content telemetry (D-D40) | CONSISTENT |
| WD-106 local-first preserved | WD-116 soft `Unreachable`; not probed at CLI startup | D-D35 `at-local-first-preserved` gate (KPI-5) | CONSISTENT |
| WD-107 architecture is DESIGN's call | WD-112/115/116 self-hostable single binary + HTTP/XRPC + graceful degrade | D-D35 second deployable; D-D36 B1 contract; localhost transport | CONSISTENT |
| WD-108 Firehose an option, not a requirement | WD-114 PULL-based bounded ingestion | hermetic ingest fixtures; no daemon observability | CONSISTENT |
| WD-109 search by object/contributor/subject | WD-113 new `openlore search` verb | 7 search-scenario ATs (D-D35) | CONSISTENT |
| WD-110 funnel reuses `peer add`; share encodes query | WD-122 render-only affordance; query-encoding link | KPI-AV-4/6 events | CONSISTENT |
| (CAVEAT) production z6Mk PLC decode unresolved | WD-118 implement real PLC decode now (ADR-026); seam release-forbidden | D-D37 `plc.directory` contract + allowlist (D-D39); `claim-domain` decode in mutation scope | CONSISTENT |

The DISCUSS open-decisions table labels TWO concerns "OD-AV-6" (share resolver +
pubkey decode); DESIGN resolved both (WD-122 + WD-118) and recorded the numbering
as a NON-BLOCKING upstream observation (DESIGN wave-decisions §Note). DISTILL
treats it as already-resolved (it is not a contradiction; both downstream paths
are locked). No scenario was written against an ambiguous spec.

DEVOPS-wave artifacts are PRESENT and run in PARALLEL with DISTILL (D-D35..D-D43);
they DEPEND on DESIGN, not on DISTILL. Their event shapes + the two contract
boundaries + the hermetic fixture names are consumed below.

---

## 2. Resolved DISTILL flags (the DISCUSS `# DISTILL: confirm` set)

DISCUSS carried four headline `# DISTILL: confirm` flags; DESIGN RESOLVED all
four (DESIGN component-boundaries.md §Annotation for acceptance-designer +
wave-decisions.md §Handoff). DISTILL inherits the resolutions verbatim and binds
the scenarios:

| `# DISTILL: confirm` flag | Resolution | Bound scenario(s) |
|---|---|---|
| Discovery surface grammar: new `search` verb vs `--network` flag on `graph query`? | **NEW top-level `openlore search` verb** (WD-113 / ADR-027); `graph query` stays unambiguously local | every AV-8..AV-29 (the `search` verb subprocess scenarios) |
| Deployment shape: self-hostable single binary vs hosted service? | **Self-hostable single binary `openlore-indexer`, signing-incapable** (WD-112 / ADR-023) | AV-5 (capability boundary), AV-1..AV-7 (the `openlore-indexer` binary), AV-14 (localhost serve) |
| Ingestion model (the ADR-016 re-eval): pull vs Firehose? | **PULL-based bounded ingestion** (WD-114 / ADR-024); Firehose a documented future option | AV-1..AV-7 (`openlore-indexer ingest` bounded pull over a fake source) |
| Pubkey-decode mechanism: real PLC z6Mk decode vs the slice-03 test seam? | **Production PLC `z6Mk...` multibase decode NOW** (WD-118 / ADR-026); seam release-forbidden | AV-4 (the gold real-decode path; env seam UNSET); AVC-1/AVC-4 (the pure verify gate) |

Plus the OD-AV-7 default: countered claims appear normally, counter SHOWN not
applied (WD-119) → bound to AVC-6 (pure) + AV-25 (render). Zero open ambiguity
at scenario-write time.

The DESIGN→DELIVER deferrals (Q-DELIVER-AV-1..9) are NOT DISTILL `confirm` flags
— they are crafter-ergonomics calls WITHIN the locked contracts (exact DDL, HTTP
framework, link grammar, ingest cadence, one-method-vs-three, PLC config key,
delegate-vs-point degrade, base58 dep-vs-inline, orientation message). DISTILL
asserts the CONTRACT each one resolves to (e.g. AV-26 asserts
query-encoding-not-snapshot regardless of the exact grammar Q-DELIVER-AV-3 picks).

---

## 3. Scope and shape

Same hexagonal port-to-port discipline as slice-01/02/03/04: every subprocess
acceptance test enters through a DRIVING ADAPTER via a REAL binary — the
`openlore` CLI (`assert_cmd::cargo_bin`) for the `search` verb, and the NEW
`openlore-indexer` binary for the ingest/serve surface — exercises the real
`claim-domain` verify core + the NEW pure `appview-domain` core + the real
`index.duckdb` store, and the pure verify-gate + anti-merging-composition
PROPERTIES are exercised by direct pure-core invocation at layer 2.

### The single most important shape fact of slice-05

**Slice-05 introduces the FIRST network service + the FIRST cross-process
boundary + the FIRST adversarial-input external boundary.** Unlike slice-04 (a
local read over a seeded store, NO new fake), slice-05 needs hermetic doubles for
the two new external surfaces:

1. A **`FakeIngestSource`** (a bounded fixture network ingest source hosting a
   `listRecords`-style enumeration) carrying the adversarial set (unsigned /
   tampered-signature / cid-mismatch) + valid signed records.
2. A **fixture PLC DID-document resolver** carrying a REAL `z6Mk...` value (a
   known test keypair) so the ADR-026 decode runs the REAL decode path (NOT the
   slice-03 env seam — the gold test, AV-4).

The two binaries' BOUNDARY (CLI→indexer, B1) is exercised against a REAL
`openlore-indexer serve` over LOCALHOST bound to an EPHEMERAL port (`:0`, read
back — parallel-safe, DEVOPS open-q 8) — the production composition root, not an
in-process stub. The `index.duckdb` is a REAL separate DuckDB seeded via the
ingest harness. The discovery→federation funnel (AV-19/AV-22) reuses the slice-03
`PeerPds` double + `peer add`/`peer pull`/`peer remove` verbs VERBATIM.

### Layer placement (per nw-test-design-mandates Mandate 9 + DD-AV-3)

| Layer | Test file(s) | Real components | Test mode |
|---|---|---|---|
| Subprocess / FS acceptance (layer 3) | `appview_search.rs` (22) | `openlore` CLI + REAL `openlore-indexer serve` (localhost ephemeral port) + REAL `index.duckdb` + pure `appview-domain` + `adapter-index-query` (HTTP/XRPC); slice-03 `PeerPds` for the funnel | example-only (Mandate 11) |
| Subprocess / FS acceptance (layer 3) | `indexer_ingest.rs` (7) | `openlore-indexer` binary + `FakeIngestSource` + fixture real-`z6Mk` PLC resolver + REAL `index.duckdb` + pure `appview-domain` ingest gate + `claim-domain` verify | example-only (Mandate 11) |
| In-memory acceptance (layer 2) | `appview_core.rs` (8; 5 `@property`) | None — pure `appview-domain` core directly | example + `@property` proptest (Mandate 9 layer-2 PBT full) |

Layer 1 (pure-core unit tests for each RejectReason arm, the exact multibase
decode boundary cases, mutation testing of `ingest_decision`/`compose_results` +
the `claim-domain` decode helper) is OUT OF DISTILL SCOPE — DELIVER's inner TDD
loop (DD-AV-7). The four NEW adapters' `probe()` bodies (the substrate-lie
checks: index-store fsync-honesty, ingest network-lies, identity real-decode,
query-server author_did-present, index-query unreachable-soft) are DELIVER's
adapter-integration concern below the driving-port boundary (DESIGN §6.3) — NOT
DISTILL acceptance scenarios. The exception surfaced at the acceptance boundary:
AV-6 asserts the composition-root REFUSES to start on a probe failure (a
user-visible startup outcome, not a probe internal).

### What is real, what is faked (slice-05 additions to the slice-04 table)

| Component | Treatment | Why |
|---|---|---|
| Pure `appview-domain` core (`ingest_decision`, `compose_results`, `near_match_suggestion`, ADTs) | REAL — invoked directly at layer 2 (`appview_core.rs`), and through both binaries at layer 3 | The two trust primitives ARE the slice; the pure core is trivially testable with no fixtures (CM-D) |
| `claim-domain` verify + compute_cid + `decode_ed25519_multibase` | REAL — reused at ingest (no second verification path, WD-104/121) | The verify gate reuses the SAME pure core; the decode is the ADR-026 production path (AV-4 gold test) |
| `adapter-index-store` (`IndexStorePort` over `index.duckdb`) | REAL DuckDB (separate file; non-Option author_did; no merged schema) | Mandate 6 — exercised by AV-1/AV-2/AV-3 (ingest) + AV-8.. (query via serve) |
| `adapter-xrpc-query-server` + `adapter-index-query` (the B1 XRPC boundary) | REAL `openlore-indexer serve` over localhost ephemeral port; REAL CLI HTTP client | Mandate 6 + B1 — exercised by AV-14 (localhost transport) + every AV-8.. query |
| `IngestSourcePort` (the network ingest source) | **`FakeIngestSource`** (bounded fixture records incl. adversarial set) | Driven external / non-deterministic per the Architecture of Reference → FAKE with output capture |
| `IdentityResolvePort` (PLC DID-doc → pubkey) | **fixture real-`z6Mk` DID-document resolver** (a known test keypair) | Driven external / non-deterministic → FAKE; but it carries a REAL z6Mk so the REAL decode runs (AV-4) |
| Peer claim source (the funnel SEED) | slice-03 `PeerPds` double + `peer add`/`peer pull` | The discovery→federation funnel reuses slice-03 verbatim (no new fake; AV-19/AV-22) |
| `verified` boolean / merged-consensus row | NOT a column / NOT a table (the load-bearing absence) | AV-2 asserts no consensus/merged/aggregate table; AV-11/AVC-7 read `verified_against` as the universal marker |

### Test file rationale + placement (DD-AV-4)

FLAT layout under `tests/acceptance/` matching slice-01/02/03/04. Three new files
alongside the existing ones:

```
tests/acceptance/
  walking_skeleton.rs / lexicon_conformance.rs / federation_roundtrip.rs  # slice-01, unchanged
  peer_subscribe.rs / peer_pull.rs / counter_claim.rs / federated_query.rs / lexicon_counter_claim.rs  # slice-03, unchanged
  scrape_*.rs / scraper_domain.rs   # slice-02, unchanged
  graph_query_explore.rs / scoring_core.rs   # slice-04, unchanged
  appview_search.rs              # slice-05 NEW (22 scenarios; layer 3 subprocess; US-AV-002..006 + 5 cardinal gates)
  indexer_ingest.rs              # slice-05 NEW (7 scenarios; layer 3 subprocess; US-AV-001 infra + the ingest gate)
  appview_core.rs                # slice-05 NEW (8 scenarios; layer 2 pure-core; 5 @property)
  support/
    mod.rs                       # EXTENDED — adds the slice-05 indexer harness + seeders + assertion helpers
```

Rationale: preserves `cargo test --test <file>` ergonomics; the three new files
are clearly labeled by concern (the CLI `search` discovery surface vs the
`openlore-indexer` ingest infra vs the pure `appview-domain` core). Symmetric with
slice-04 (`graph_query_explore.rs` + `scoring_core.rs`), slice-02 (`scrape_*` +
`scraper_domain.rs`), and slice-03 (`peer_*` + `lexicon_counter_claim.rs`). The
indexer infra is its OWN file (not folded into `appview_search.rs`) because it
drives a DIFFERENT binary (`openlore-indexer`, the second composition root) — the
two driving ports are distinct.

---

## 4. Acceptance test inventory

Per Mandate 3 (User Journey Completeness) every test exercises a complete user
journey from observable trigger through observable outcome. Full per-scenario
docstrings + Given/When/Then are in the `.rs` files.

### `tests/acceptance/indexer_ingest.rs` — 7 scenarios (layer 3; US-AV-001)

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| AV-1 | `indexer_ingests_a_verified_attributed_claim_and_it_becomes_searchable` | US-AV-001 | happy / WS | `@walking_skeleton @real-io @driving_port @infrastructure @i-av-1 @kpi-av-3` |
| AV-2 | `indexer_stores_two_distinct_author_claims_without_merging_on_same_subject_object` | US-AV-001 | anti-merging | `@anti-merging @i-av-2 @kpi-av-2` |
| AV-3 | `indexer_rejects_unverified_claim` | US-AV-001 | RELEASE GATE / error | `@release-gate @i-av-1 @kpi-av-3 @error @adversarial` |
| AV-4 | `indexer_verifies_against_real_decoded_plc_z6mk_key_not_the_test_seam` | US-AV-001 | gold path | `@i-av-6 @adr-026 @gold-path` |
| AV-5 | `indexer_is_signing_incapable_and_touches_no_local_store` | US-AV-001 | capability boundary | `@i-av-5 @adr-023 @capability-boundary` |
| AV-6 | `indexer_refuses_to_start_when_a_driven_adapter_probe_fails` | US-AV-001 | infra / error | `@adr-009 @adr-023 @infrastructure @error` |
| AV-7 | `indexer_ingests_only_public_records_no_private_read` | US-AV-001 | public-data | `@i-av-4 @wd-105 @public-data` |

### `tests/acceptance/appview_search.rs` — 22 scenarios (layer 3; US-AV-002..006)

| # | Test name | Story | Type | Tag(s) |
|---|---|---|---|---|
| AV-8 | `search_by_object_surfaces_verified_claims_by_unfollowed_authors_attributed` | US-AV-002 | happy / WS | `@walking_skeleton @real-io @driving_port @kpi-av-1 @kpi-av-2` |
| AV-9 | `network_result_preserves_attribution` | US-AV-002 | RELEASE GATE / anti-merging | `@release-gate @i-av-2 @kpi-av-2 @anti-merging @edge` |
| AV-10 | `public_data_banner_shown` | US-AV-004 | RELEASE GATE / public-data | `@release-gate @i-av-4 @kpi-av-5 @public-data` |
| AV-11 | `verified_marker_is_universal` | US-AV-004 | RELEASE GATE / verified | `@release-gate @i-av-1 @verified-marker` |
| AV-12 | `search_by_object_unknown_philosophy_returns_empty_with_suggestion_exit_zero` | US-AV-002 | error | `@error @suggestion @edge` |
| AV-13 | `local_first_preserved` | US-AV-002 (+inherited) | RELEASE GATE / local-first | `@release-gate @i-av-3 @kpi-5 @local-first` |
| AV-14 | `search_succeeds_with_indexer_localhost` | US-AV-002 | RELEASE GATE / B1 | `@release-gate @b1-transport @wd-115 @kpi-av-2` |
| AV-15 | `search_by_contributor_lists_full_network_trail_with_honest_framing` | US-AV-003 | happy | `@kpi-av-1 @kpi-av-4 @happy` |
| AV-16 | `search_by_subject_surfaces_every_authors_verified_claims_attributed` | US-AV-003 | anti-merging | `@kpi-av-2 @anti-merging @happy` |
| AV-17 | `search_by_contributor_absent_from_index_degrades_gracefully_exit_zero` | US-AV-003 | error | `@error @edge` |
| AV-18 | `search_labels_a_followed_author_as_subscribed_peer` | US-AV-003 | relationship | `@relationship-label @edge` |
| AV-19 | `discovery_follow_reuses_slice03_path` | US-AV-005 | happy | `@driving_port @kpi-av-4 @i-av-7 @happy` |
| AV-20 | `discovery_never_auto_subscribes` | US-AV-005 | edge | `@i-av-7 @edge` |
| AV-21 | `already_followed_author_shows_no_redundant_follow_affordance` | US-AV-005 | relationship | `@relationship-label @edge` |
| AV-22 | `followed_discovery_author_purges_via_slice03_semantics_zero_residue` | US-AV-005 | edge | `@i-av-7 @edge` |
| AV-23 | `show_inspects_a_verified_record_with_signature_and_cid_match_lines` | US-AV-004 | happy | `@driving_port @kpi-av-3 @happy` |
| AV-24 | `show_on_cid_absent_from_result_set_is_a_usage_error_nonzero_exit` | US-AV-004 | error | `@error @edge` |
| AV-25 | `countered_claim_still_appears_in_search_with_annotation` | US-AV-002 | edge | `@od-av-7 @i-av-9 @edge` |
| AV-26 | `share_emits_stable_query_encoding_link_for_object_search` | US-AV-006 | happy | `@driving_port @kpi-av-6 @i-av-8 @happy` |
| AV-27 | `opening_a_shared_link_re_runs_the_query_yielding_same_attributed_results` | US-AV-006 | happy | `@driving_port @kpi-av-6 @kpi-av-2 @i-av-8 @anti-merging` |
| AV-28 | `shared_link_resolves_to_current_results_not_a_stale_snapshot` | US-AV-006 | edge | `@kpi-av-6 @i-av-8 @edge` |
| AV-29 | `share_encodes_contributor_dimension_resolving_to_the_trail` | US-AV-006 | edge | `@kpi-av-6 @i-av-8 @edge` |

### `tests/acceptance/appview_core.rs` — 8 scenarios (layer 2; 5 `@property`)

| # | Test name | Source | Type | Tag(s) |
|---|---|---|---|---|
| AVC-1 | `appview_ingest_gate_indexes_iff_verified_and_cid_matches_property` | I-AV-1 / WD-104 / KPI-AV-3 | `@property` | `@property @us-av-001 @i-av-1 @kpi-av-3 @release-gate` |
| AVC-2 | `appview_compose_preserves_every_author_property` | I-AV-2 / WD-103 / KPI-AV-2 | `@property` | `@property @us-av-002 @i-av-2 @kpi-av-2 @anti-merging @release-gate` |
| AVC-3a | `appview_ingest_decision_is_deterministic_property` | DESIGN 5.1 inv 4 | `@property` | `@property @us-av-001 @i-av-1` |
| AVC-3b | `appview_compose_results_is_deterministic_property` | DESIGN 5.1 inv 4 | `@property` | `@property @us-av-002 @us-av-006 @i-av-8` |
| AVC-4 | `appview_indexed_author_is_derived_from_signed_payload_property` | I-AV-2 type/derivation | `@property` | `@property @us-av-001 @i-av-2 @anti-merging` |
| AVC-5 | `appview_two_identical_content_distinct_author_claims_compose_to_two_groups` | I-AV-2 type-level / WD-103 | example | `@us-av-002 @i-av-2 @anti-merging @gate-type` |
| AVC-6 | `appview_countered_claim_still_appears_with_annotation` | OD-AV-7 / I-AV-9 | example | `@us-av-002 @od-av-7 @i-av-9` |
| AVC-7 | `appview_every_composed_row_carries_a_nonempty_verified_against` | I-AV-1 (verified marker) | example | `@us-av-004 @i-av-1 @verified-marker` |
| AVC-8 | `appview_near_match_suggestion_finds_closest_known_object` | US-AV-002 Ex4 | example | `@us-av-002 @suggestion @edge` |

(The AVC-3 determinism heading splits into AVC-3a [ingest] + AVC-3b [compose] →
the file has 9 distinct `#[test]` functions for the 8 numbered scenarios.)

### Total slice-05 scenarios

7 (indexer_ingest) + 22 (appview_search) + 8 (appview_core) = **37 scenarios**
authored, all RED-ready as `todo!()` scaffolds with `// SCAFFOLD: true` module
markers. Above the ~25-35 band for 6 stories — justified: slice-05 is the
architecturally heaviest slice (the first network service + 2 binaries + 5
cardinal release gates + the first adversarial-input boundary). The 8 release-gate
scenarios (AV-3, AV-9, AV-10, AV-11, AV-13, AV-14 + the pure AVC-1, AVC-2) are
load-bearing, not padding.

### §4-rationale — error-path ratio + guardrail-surface justification

Explicit `@error`-tagged: AV-3, AV-6, AV-12, AV-17, AV-24 = 5/37 = 13.5%. As with
slice-03/04 (READ-surface slices), the load-bearing slice-05 RISK is NOT
input-validation sad paths — it is the GUARDRAIL + adversarial surface. Counting
the NON-happy surface (the 8 release gates + anti-merging + capability-boundary +
adversarial + degradation + the 5 pure `@error` exits):

- Release gates / cardinal trust: AV-3, AV-9, AV-10, AV-11, AV-13, AV-14, AVC-1, AVC-2 (8)
- Anti-merging renders / type: AV-2, AV-16, AV-25, AV-27, AVC-4, AVC-5 (6)
- Capability boundary / public-data / probe-refusal: AV-5, AV-6, AV-7 (3)
- Adversarial / gold path: AV-3 (also above), AV-4 (1 net)
- Pure `@error` / degradation exits: AV-12, AV-13 (also above), AV-17, AV-24, AV-20 (no-auto-subscribe) (4 net)

The "non-happy / load-bearing" surface is **≥18/37 = 49%** — above the 40%
nw-test-design-mandates target. The 5 pure `@error` exits (typo'd object → exit 0
+ suggestion; absent contributor → exit 0; `--show` absent cid → non-zero usage
error; probe-failure startup refusal; the local-only soft degrade) are the true
validation sad paths the read/discovery surface admits, plus the adversarial
reject set (AV-3) which is the cardinal gate, not a mere validation path.

The SUBSTRATE sad paths (index-store fsync-lie, ingest network-lie, the decode
boundary cases, the unreachable-soft probe) live at the four NEW adapters' PROBE
layer per DESIGN §6.3 — DELIVER's adapter-integration concern below the
driving-port boundary, backed by the `FakeIngestSource` adversarial fixtures + the
fixture real-`z6Mk` DID-doc — NOT DISTILL acceptance scenarios (except AV-6, which
asserts the user-visible STARTUP REFUSAL the probes drive).

---

## 5. Driving Adapter coverage (Mandate 1 + RCA P1)

Two driving adapters (two binaries). Every NEW CLI verb/flag + the new indexer
subcommands is covered by at least one subprocess scenario.

### `openlore` CLI — the `search` verb (ADR-027)

| Flag (ADR-027) | Scenario coverage |
|---|---|
| `search --object <philosophy>` (NEW) | AV-8 (happy/WS), AV-9 (anti-merging gate), AV-12 (error), AV-13 (local-first gate), AV-14 (localhost gate), AV-25 (counter) |
| `search --contributor <did\|handle>` (NEW) | AV-15 (happy trail), AV-17 (absent), AV-18 (followed label), AV-29 (share) |
| `search --subject <project>` (NEW) | AV-16 (per-author survey) |
| `search --show <cid>` (NEW; combinable) | AV-23 (verified inspect), AV-24 (absent-cid usage error) |
| `search --share` (NEW; combinable) | AV-26 (object share), AV-27 (open → same results), AV-28 (current not stale), AV-29 (contributor share) |
| the public-data banner (every session) | AV-10 (banner gate) |
| the `[verified]` marker (every row) | AV-11 (universal-marker gate) |
| the `peer add` follow affordance (reused verbatim) | AV-19 (funnel), AV-20 (no auto-sub), AV-21 (no redundant), AV-22 (purge) |

### `openlore-indexer` binary — `ingest` / `serve` (ADR-023)

| Subcommand / behavior | Scenario coverage |
|---|---|
| `openlore-indexer ingest` (bounded pull pass, ADR-024) | AV-1 (verified attributed), AV-2 (anti-merge at ingest), AV-3 (reject gate), AV-4 (real decode), AV-7 (public-only) |
| `openlore-indexer serve` (HTTP query server, ADR-027) | AV-14 (CLI reaches it over localhost), AV-6 (startup probe refusal), every AV-8.. (the query backend) |
| the capability boundary (no sign/publish; no local store) | AV-5 |
| `openlore-indexer --help` (verb-set) | AV-5 (no sign/publish verb) |

Zero uncovered NEW verb/flag/subcommand. Every one is exercised via subprocess
(the real `openlore` + real `openlore-indexer` binaries) — pipeline/service-level
tests do NOT replace driving-adapter tests. The B1 CLI↔indexer XRPC boundary is
exercised end-to-end against a REAL `openlore-indexer serve` over localhost
(AV-14), not an in-process stub.

---

## 6. Driven adapter coverage (Mandate 6)

| Driven adapter | Real-I/O scenario? | Tag |
|---|---|---|
| `adapter-index-store` (`IndexStorePort` over `index.duckdb`) | YES — AV-1/AV-2/AV-3 (ingest write) + AV-8.. (query read via serve) over REAL DuckDB | `@real-io` |
| `adapter-xrpc-query-server` (HTTP query surface) | YES — AV-14 (real localhost serve) + every AV-8.. query backend | `@real-io` |
| `adapter-index-query` (CLI HTTP/XRPC client) | YES — AV-14 (reachable), AV-13 (unreachable → soft) | `@real-io` |
| `adapter-atproto-ingest` (`IngestSourcePort`) | FAKE (`FakeIngestSource`, driven-external per Arch-of-Reference) — AV-1..AV-7; the network-lies adversarial set in AV-3 | `@in-memory` (fake source) |
| `adapter-atproto-did` (`IdentityResolvePort`, verify-only) | FAKE resolver carrying a REAL `z6Mk` — AV-4 runs the REAL decode path (gold) | `@in-memory` (fixture DID-doc) + REAL decode |
| `claim-domain` verify + compute_cid + decode (pure) | REAL — AV-1/AV-3/AV-4 through the indexer; AVC-1/AVC-4 directly | `@real-io` / `@property` |
| Pure `appview-domain` core (not an adapter — no probe) | YES — AVC-1..AVC-8 invoke it directly (layer 2); AV-1.. + AV-8.. through the binaries | `@property` / `@real-io` |
| `adapter-duckdb` (the user's LOCAL store) | REAL — AV-5 asserts the indexer NEVER touches it; AV-13/AV-19 use it via the CLI authoring/graph verbs | `@real-io` |
| slice-03 `PeerPds` double (the funnel SEED) | REUSED — AV-19/AV-22 (discovery → `peer add` → `peer pull`) | `@real-io` (the seed mechanism) |

The four NEW adapters' `probe()` bodies (the substrate-lie checks: index-store
fsync-honesty / ingest network-lies / identity real-decode / query-server
author_did-present / index-query unreachable-soft) are DELIVER's
adapter-integration deliverable (DESIGN §6.3) — NOT DISTILL acceptance scenarios,
EXCEPT the user-visible startup REFUSAL the probes drive (AV-6) and the gold
real-decode path (AV-4). The pure `appview-domain` core has NO `probe()` (it
touches no substrate); its Earned-Trust analog is the layer-2 property suite
(`appview_core.rs`) + DELIVER's mutation testing (D-D40; DESIGN §10).

The `FakeIngestSource` + the fixture PLC resolver are driven-external /
non-deterministic ports (the Architecture of Reference → FAKE with output
capture). Per the Integration Test Contract (nw-tdd-methodology §Test Doubles
Must Validate Inputs): `FakeIngestSource` MUST reject the same malformed inputs
the real ingest adapter would, and the fixture resolver MUST carry a real `z6Mk`
so the decode it feeds is the production path (a permissive fake that "verified"
anything would hide the AV-3 reject-gate wiring — the exact bug class
nw-tdd-methodology warns about).

---

## 7. Release-gate coverage (the cardinal scenarios; DESIGN §10 + DEVOPS D-D35)

The eight cardinal release gates, each asserting its invariant at a load-bearing
boundary:

| Release gate | Where asserted | Invariant asserted | Mandatory for |
|---|---|---|---|
| `indexer_rejects_unverified_claim` | **AV-3** (layer-3 binary + adversarial wire fixtures) + AVC-1 (layer-2 pure gate, generative) | unsigned/tampered/cid-mismatch records REJECTED at ingest; NEVER indexed, NEVER searchable; the valid one IS; reuses `claim_domain::verify` (no second path); incl. the `did_unresolvable`→reject path via AV-6/AV-4 | **KPI-AV-3 (release-blocking)** |
| `network_result_preserves_attribution` | **AV-9** (layer-3 render + B1 transport) + AVC-2 (layer-2 pure compose, generative) + AVC-4/AVC-5 (type/derivation) | two distinct-author claims on the SAME (subject,object) → TWO attributed rows; NO merged/consensus row; the mean/aggregate NEVER appears as a row; footer count is a COUNT over rows | **KPI-AV-2 (release-blocking)** |
| `local_first_preserved` | **AV-13** (layer-3; indexer unreachable + network disabled) | `claim add` / offline `claim publish` / `graph query` ALL succeed; `search` degrades to a clear local-only message + non-fatal exit, no hang; indexer not probed at CLI startup | **KPI-5 (release-blocking)** |
| `public_data_banner_shown` | **AV-10** (layer-3; banner precedes results) | a banner states discovery indexes only PUBLIC signed claims, verified before indexing, nothing private read/aggregated | KPI-AV-5 |
| `verified_marker_is_universal` | **AV-11** (layer-3; every row across all dimensions) + AVC-7 (layer-2 construction) | EVERY returned claim shows `[verified]`; no `[unverified]` state exists by construction | I-AV-1 |
| `search_succeeds_with_indexer_localhost` | **AV-14** (layer-3; REAL localhost serve, ephemeral port) | the CLI reaches a real localhost indexer; the wire carries per-result `author_did`; attributed verified results render (B1 transport) | B1 / WD-115 / D-D36 |
| verify-before-index pure-core property | **AVC-1** (layer-2 `@property`, generative gate iff) | `Index` IFF verify + CID both pass; reuses the pure core | KPI-AV-3 (the cheap generative half) |
| anti-merging pure-core property | **AVC-2** (layer-2 `@property`, generative preserve-every-author) | `distinct_author_count == COUNT(DISTINCT author_did)`; no row dropped/merged | KPI-AV-2 (the cheap generative half) |

Plus the behavioral funnel/share/counter gates: `discovery_follow_reuses_slice03_path`
(AV-19; KPI-AV-4 / I-AV-7), `share_link_encodes_query_not_snapshot` (AV-26/AV-28;
KPI-AV-6 / I-AV-8), `countered_claim_still_appears` (AV-25 + AVC-6; OD-AV-7 /
I-AV-9). Every gate has ≥1 layer-3 behavioral assertion; the two cardinal trust
guarantees (KPI-AV-2/3) ALSO have a layer-2 generative property (the
debt-never-accumulates posture: the pure primitive is proven generatively, the
binary wiring is proven by example).

---

## 8. KPI coverage

| KPI | Description | Acceptance coverage |
|---|---|---|
| KPI-AV-1 | Discover an unfollowed-author claim (north star) | AV-8 (surfaces unfollowed, labeled), AV-15 (contributor trail before following); the RATE is DEVOPS telemetry (`search.discovery.unfollowed_author_hit`), not an acceptance assertion |
| KPI-AV-2 | Zero attribution loss at network scale (release-blocking) | AV-9 (load-bearing) + AV-2/AV-16/AV-27 + AVC-2/AVC-4/AVC-5 + AV-14 (wire author_did) |
| KPI-AV-3 | Verified-before-index (release-blocking) | AV-3 (load-bearing) + AVC-1 + AV-4 (real decode) + AV-11/AVC-7 (universal marker) |
| KPI-AV-4 | Discovery→federation funnel | AV-19 (the funnel closes); the RATE is DEVOPS telemetry (`search.discovery.follow_funnel`) |
| KPI-AV-5 | Public-data framing comprehension | AV-10 (banner shown); the COMPREHENSION is the DEVOPS one-shot prompt (D-D18/D-D40), not an acceptance assertion |
| KPI-AV-6 | Shared-link usage | AV-26/AV-27/AV-28/AV-29 (the link is emitable + re-runs + current); the USAGE is DEVOPS telemetry (`search.share.link_emitted/opened`) |
| KPI-5 (inherited) | Local-first preserved | AV-13 (load-bearing release gate) |
| KPI-4 (inherited) | Zero silent normalization | AV-23 (`--show` CID recomputed matches published record at network scale) |

KPI-AV-1/4/5/6 RATES are telemetry-measured (production cohort, deferred — DEVOPS
D-D40 YELLOW cohort), not asserted at the acceptance boundary — by design, per
outcome-kpis.md Measurement Plan. The suite proves the BEHAVIOR exists for the
telemetry to count (AV-8 surfaces an unfollowed author; AV-19 closes the funnel;
AV-26 emits the link).

---

## 9. Three Pillars compliance

| Pillar | How DISTILL satisfied it |
|---|---|
| 1 — Domain language | Scenario titles use `search`, `object`, `philosophy`, `subject`, `project`, `contributor`, `network trail`, `author DID`, `verified`, `attributed`, `unfollowed`, `subscribed peer`, `follow`, `share`, `public-data`, `reasoning trail`, `consensus` (only to FORBID it), `countered`, `cid`. Zero technical jargon in titles/step-names: NO `SQL`, `HTTP`, `XRPC`, `endpoint`, `JSON`, `schema`, `DuckDB`, `multibase`. (`z6mk`/`plc` appear in AV-4's title because the REAL z6Mk PLC decode IS the user-facing trust-anchor concept the gold test names — it is the domain term for "the author's real verification key", not a transport detail; `localhost`/`b1` appear only in tags + comments, never asserted as user-visible text.) |
| 2 — Chained narrative | The discover-trust-act arc reads in order: search (AV-8) → trust/inspect (AV-23, `--show` the AV-8 result's cid) → read-before-following (AV-15 contributor trail) → follow (AV-19 reuses the AV-8/AV-15 result's `peer add` affordance) → share (AV-26 shares the AV-8 query; AV-27 opens AV-26's link). AV-19's `Given` reuses the AV-8 search `When`; AV-27's `Given` reuses AV-26's emitted link; AV-22's `Given` reuses AV-19's follow. No copy-pasted fixture setup — the named ingest-harness seeders (`seed_network_index(NetworkIndexFixture::*)`) are the shared step composition. The layer-2 AVC-1..AVC-8 chain the same `ingest_decision` / `compose_results` Given across gate → determinism → derivation → anti-merging → counter. |
| 3 — App as in production | Every AV-1..AV-7 scenario spawns the REAL `openlore-indexer` binary; every AV-8..AV-29 spawns the REAL `openlore` CLI (the production composition roots) against a REAL `openlore-indexer serve` over localhost + a REAL `index.duckdb`. No hand-rebuilt wiring. The `FakeIngestSource` + the fixture PLC resolver substitute ONLY the external/non-deterministic network-ingest + DID-resolution boundaries (the Architecture of Reference defaults); the slice-03 `PeerPds` substitutes ONLY the external peer-PDS boundary as the funnel seed. AVC-1..AVC-8 (layer 2) invoke the pure `appview-domain` core directly (the function signature IS the port). |

---

## 10. Mandate compliance evidence (CM-A through CM-H)

| Mandate | Compliance evidence |
|---|---|
| CM-A (Mandate 1, hexagonal boundary) | All `appview_search.rs` + `indexer_ingest.rs` scenarios invoke the binaries via subprocess; ZERO direct imports of `adapter-index-store` / `adapter-xrpc-query-server` / `adapter-atproto-ingest` from the test bodies (the test files `use support::*` only). The `appview_core.rs` layer-2 tests directly invoke pure-core `appview_domain::ingest_decision` / `compose_results` / `near_match_suggestion` — appropriate at layer 2 per Mandate 9 (the pure function signature IS the driving port at domain scope) |
| CM-B (Mandate 2, business language) | Grep of test names: zero `HTTP`, `XRPC`, `endpoint`, `database`, `schema`, `JSON`, `SQL`, `DuckDB`, `multibase`. Domain terms only (`search`, `object`, `contributor`, `subject`, `verified`, `attributed`, `unfollowed`, `follow`, `share`, `public-data`, `countered`). The `z6mk`/`plc` in AV-4 + `localhost` in AV-14 are the trust-anchor + transport-boundary domain concepts the gold/B1 gates name (see Pillar 1 note) |
| CM-C (Mandate 3, complete journeys) | Every test traces to a user story → see traceability.md §4. The chained discover→trust→read→follow→share arc (AV-8→AV-23→AV-15→AV-19→AV-26→AV-27) + the AVC gate→determinism→anti-merging chain satisfy Pillar 2 |
| CM-D (Mandate 4, pure function extraction) | The two trust primitives (`ingest_decision` verify-gate + `compose_results` anti-merging) are PURE functions exercised DIRECTLY in `appview_core.rs` AVC-1..AVC-8 — no fixtures, no adapters, no environment cross-product. The CLI/indexer parameterization is just `tempfile::TempDir` for HOME + the ephemeral `:0` port (parallel-safe). Impure ingest/store/transport is behind the four NEW ports (real index store + real serve; fake ingest source + fixture resolver), seeded once per scenario |
| CM-E (Mandate 8, state-delta + Universe) | **DEFERRED to DELIVER** (DD-AV-10) — same status as slice-01 DD-3 / slice-03 DD-FED-10 / slice-04 DD-GRAPH-10. The Rust `state_delta` port at `tests/common/state_delta.rs` was bootstrapped by slice-01; slice-05 INHERITS it (`[port-mode] inherit`). Slice-05 scenarios DECLARE the port-exposed universe per scenario (see each docstring's "Universe (port-exposed)" line — e.g. `search.distinct_authors_in_output`, `indexed_claims.author_did set`, `indexer.ingest.rejected{reason} counts`, `search.exit_code`, `openlore.duckdb mtime/bytes` for the capability boundary, `peer_subscriptions before==after` for no-auto-subscribe). DELIVER migrates the load-bearing scenarios (AV-3, AV-9, AV-13, AV-5, AV-22) to `assert_state_delta(before, after, universe, expected)` form once the helper bodies are real. Universe entries are port-exposed (CLI stdout fields, exit codes, indexed-row author_did set, ingest counters, the local-store byte-unchanged guard) — NEVER internal store/compose struct fields |
| CM-F (Mandate 9, layered PBT mode) | AVC-1/AVC-2/AVC-3a/AVC-3b/AVC-4 are `@property` at layer 2 (proptest); AVC-5/AVC-6/AVC-7/AVC-8 are example-pinned; ALL 29 layer-3 subprocess scenarios (AV-1..AV-29) are example-only. ZERO proptest at layer 3+ (the adversarial sets in AV-3 are NAMED fixtures, not generated) |
| CM-G (Mandate 10, two-tier acceptance) | **Tier A + Tier B BOTH justified — but Tier B is DEFERRED to DELIVER with a documented decision (DD-AV-9).** The discover→trust→read→follow→share journey IS ≥3 chained scenarios with a domain-rich input space (philosophy URIs, DIDs, cids, multi-author corpora) — qualifying on BOTH Tier-B triggers. AND, per the slice-04 CM-G forward-note, slice-05 IS the surface where "multi-user/cohort aggregation introduces a genuine indexer state machine". HOWEVER: the load-bearing invariants are (a) the verify-GATE (a pure decision, not a state machine — Hebert ch.11 model-shape test: `ingest_decision` is a stateless function of (record, key)) and (b) anti-merging COMPOSITION (a pure FORALL over rows, no mutate-then-observe command protocol). The genuine state-machine candidate is the INGEST→STORE→QUERY lifecycle (ingest mutates `index.duckdb`; a later query observes it) — a real `@rule`(ingest a record) / `@invariant`(every stored row verified + attributed; distinct_author_count == COUNT DISTINCT) machine over an `InMemoryComposition` wiring in-memory `IngestSourcePort`/`IndexStorePort` doubles. This WOULD add detection value (it explores ingest/query interleavings the example AV-1..AV-3 + AVC-1..AVC-2 don't). It is DEFERRED to DELIVER (not authored RED here) because: the shared step-method vocabulary (`given_a_fake_source_with`, `when_the_indexer_ingests`, `then_every_stored_row_is_verified_and_attributed`) must first exist in the Tier A harness (`support/mod.rs`), and Mandate 10's contract is that Tier B `@rule`s invoke EXISTING Tier A step-methods. Recorded as DD-AV-9 + Open Item 9: DELIVER SHOULD add `tests/acceptance/appview_state_machine.rs` (`RuleBasedStateMachine`-equivalent via proptest's stateful API or `proptest-state-machine`) once the harness step-methods land, with `@invariant`s = the two cardinal gates. The two pure primitives are ALREADY explored generatively at layer 2 (AVC-1/AVC-2), so the example layer-3 suite is sound without Tier B; Tier B is amplification, not a gap |
| CM-H (Mandate 11, sad-paths example-based) | Every layer-3 sad path is a NAMED example scenario: AV-3 (the adversarial reject set — unsigned/tampered/cid-mismatch, named fixtures), AV-6 (probe-failure startup refusal), AV-12 (unknown object), AV-17 (absent contributor), AV-24 (`--show` absent cid usage error), AV-13 (the soft-unreachable degrade). ZERO proptest at layer 3+ for sad paths. The substrate sad paths (fsync-lie, network-lie, decode boundary, unreachable-soft) are DELIVER's adapter probes (DESIGN §6.3), not the acceptance suite |

---

## 11. Project Infrastructure Policy — slice-05 additions

Slice-01 DD-11 / slice-03 DD-FED-11 / slice-04 DD-GRAPH-11 deferred writing
`docs/architecture/atdd-infrastructure-policy.md`. This DISTILL wave BOOTSTRAPS
it (the policy file was absent; per `nw-distill` write-if-absent + the
`policy-bootstrap-template`) with the cumulative slice-01..05 entries — see
`docs/architecture/atdd-infrastructure-policy.md`. The slice-05 additions:

```markdown
## Driving (extends slice-01/02/03/04)
| Port | Mechanism | Note |
|---|---|---|
| CLI (`openlore search --object/--contributor/--subject/--show/--share`) | subprocess from `tempfile::TempDir` via `assert_cmd` | inherits the slice-01 cli mechanism; the ONLY network verb (degrades gracefully) |
| `openlore-indexer` binary (`ingest` / `serve`) | subprocess via `assert_cmd::cargo_bin("openlore-indexer")`; `serve` bound to ephemeral `:0` (read back) | the SECOND composition root; signing-incapable; holds no local store |

## Driven internal (real) — extends slice-01/03/04
| Port | Mechanism | Note |
|---|---|---|
| IndexStorePort (`index.duckdb`) | real SEPARATE DuckDB file (non-Option author_did; NO merged schema); seeded via the ingest harness | xtask check-arch extends `no_cross_table_join_elides_author` to the index-store SQL (DELIVER) |
| `adapter-xrpc-query-server` + `adapter-index-query` (the B1 XRPC boundary) | real `openlore-indexer serve` over localhost ephemeral port + real CLI HTTP client | the response carries per-result author_did (D-D36) |
| `appview-domain` (pure core) | direct in-process invocation (no adapter; no probe) | layer-2 `@property` + DELIVER mutation testing is its Earned-Trust analog (D-D40) |
| `claim-domain` verify + compute_cid + decode_ed25519_multibase | real pure core (reused; no second verification path) | the ADR-026 decode is the production path (AV-4 gold) |

## Driven external / non-deterministic (fake) — extends slice-02/03
| Port | Fake | Note |
|---|---|---|
| IngestSourcePort (network ingest) | `FakeIngestSource` (bounded fixture records incl. unsigned/tampered/cid-mismatch adversarial set) | validates inputs like the real adapter (nw-tdd-methodology Test Doubles contract) |
| IdentityResolvePort (PLC DID-doc → pubkey) | fixture PLC resolver carrying a REAL `z6Mk...` (a known test keypair) | carries a real z6Mk so the ADR-026 decode it feeds runs the REAL path (AV-4) |
| (the funnel SEED) | slice-03 `PeerPds` reused VERBATIM for `peer add`/`peer pull` | the discovery→federation funnel adds no new external surface (WD-110) |
```

---

## 12. Pre-requisites for compilation (DELIVER wiring expectations)

The slice-05 skeletons use `use appview_core`-adjacent crates: the NEW
`appview-domain` crate (`ingest_decision`/`compose_results`/`near_match_suggestion`
+ `IngestOutcome`/`RejectReason`/`NetworkResultRow`/`NetworkSearchResult`/
`CounterRef`/`SearchDimension`), the extended `ports` (the four NEW ports +
`RawRecord`/`IndexedClaim`/`AuthorRelationship::NetworkUnfollowed`), the extended
`claim-domain` (`decode_ed25519_multibase`), the extended `cli` (`search` verb
dispatch), the NEW `openlore-indexer` binary, AND `use support::*` for the new
indexer harness + seeders + helpers + `use openlore_test_support::fixtures_ingest::*`.
The intentional consequence:

1. **`cargo build --tests` will fail until DELIVER's first slice-05 step
   (the indexer-subsystem bootstrap, DEVOPS open-q 4)** lands:
   - The pure `crates/appview-domain` crate (ADTs + `ingest_decision`/
     `compose_results`/`near_match_suggestion` stubs `todo!()`) per
     component-boundaries.md §`crates/appview-domain`.
   - `crates/ports/` extended with `IndexQueryPort`/`IngestSourcePort`/
     `IndexStorePort`/`IdentityResolvePort` + `RawRecord`/`IndexedClaim`/
     `NetworkResultRow`/`NetworkSearchResult`/`SearchDimension`/`CounterRef` +
     `AuthorRelationship::NetworkUnfollowed`.
   - `crates/claim-domain/` `decode_ed25519_multibase` stub (+ `VerificationKey`/
     `KeyId`/`DecodeError`).
   - The four effect crates (`adapter-atproto-ingest`, `adapter-index-store`,
     `adapter-xrpc-query-server`, `adapter-index-query`) with `impl <Port>` +
     `probe()` stubs.
   - `crates/openlore-indexer/` binary (the second composition root; `serve` +
     `ingest` subcommands) `todo!()`.
   - `crates/cli/` `search` verb dispatch for the 5 flags (bodies `todo!()`).
   - `crates/test-support/src/fixtures_ingest.rs` (the adversarial + valid ingest
     fixtures + the real-`z6Mk` DID-doc keypair) + the `support/mod.rs` slice-05
     additions (`FakeIngestSource` wrapper, `IndexerHandle` for the localhost
     serve, `seed_network_index`, the `run_openlore_indexer` + `run_openlore`
     search helpers + the anti-merging/verified/banner/relationship assertion
     helper bodies).
   - The THREE `[[test]]` registrations (`appview_search`, `indexer_ingest`,
     `appview_core`) in `crates/cli/Cargo.toml` (DD-AV-14).

2. **Once those land**, the slice-05 tests compile to "all `#[test]` functions
   panic with `todo!()`" → tests RED per Mandate 7. DELIVER then unskips one at
   a time (Release 1: AV-1/AV-3/AV-8/AV-9/AV-10/AV-11/AV-13/AV-14/AV-23 + AVC-1/
   AVC-2/AVC-5/AVC-7; Release 2: AV-15/AV-16/AV-17/AV-18/AV-19/AV-20/AV-21/AV-22;
   Release 3: AV-26/AV-27/AV-28/AV-29).

3. **Rust scaffold marker** per Mandate 7 + slice-01/02/03/04 precedent: every
   `#[test]` body panics via `todo!("DELIVER (slice-05): ...")` with a
   `// SCAFFOLD: true` comment-marker on the surrounding module. Detection via
   `grep -r "SCAFFOLD: true" tests/ crates/test-support/`. The new
   `fixtures_ingest.rs` module + the `support/mod.rs` slice-05 additions carry the
   same marker.

4. **Pre-DELIVER fail-for-right-reason gate (slice-05)** is DEFERRED per the same
   logic as slice-01 DD-2 / slice-03 DD-FED-13 / slice-04 DD-GRAPH-13 (DD-AV-13):
   the `appview-domain` crate + the four ports + the four effect crates + the
   `openlore-indexer` binary + the cli `search` dispatch + the test-support
   bodies land in DELIVER's first slice-05 step; only after that step do the tests
   compile and reach the `todo!()` panic that classifies as RED (not BROKEN). The
   gate output (per-scenario MISSING_FUNCTIONALITY classification) is written to
   `red-classification.md` at that point (DELIVER's RED phase entry per ADR-025).

5. **`[[test]]` registration** for the three slice-05 files is DEFERRED to that
   same bootstrap step (DD-AV-14) — registering targets whose imports do not yet
   exist would fail `cargo build --tests` for a reason OTHER than the scaffold
   `todo!()` (BROKEN, not RED).

---

## 13. Definition of Done (DISTILL handoff to DELIVER)

- [x] All 37 slice-05 scenarios written as RED-ready Rust skeletons (7 indexer
      layer-3 + 22 search layer-3 + 8 core layer-2).
- [x] Every NEW CLI verb/flag (ADR-027 `search` + 5 flags) + the `openlore-indexer`
      subcommands (ADR-023 `ingest`/`serve`) covered by ≥1 subprocess scenario (§5).
- [x] Every NEW driven adapter mapped (real index store + real serve + real
      decode; fake ingest source + fixture resolver explicitly justified per the
      Architecture of Reference) — see §6.
- [x] Three Pillars verified (domain language, chained narrative, production
      composition — §9).
- [x] The four DISCUSS `# DISTILL: confirm` flags (search verb; deployment shape;
      pull-vs-Firehose; pubkey decode) RESOLVED per DESIGN and bound to scenarios
      (§2).
- [x] All five DISCUSS-tagged cardinal release gates + the two extra named gates
      (`search_succeeds_with_indexer_localhost`, `verified_marker_is_universal`)
      authored + load-bearing (§7).
- [x] Wave-decision reconciliation passed (0 contradictions DISCUSS WD-100..110 ↔
      DESIGN WD-111..124 ↔ DEVOPS D-D35..D-D43 — §1).
- [x] `[lang-mode] rust` detected + logged; state-delta port present (`[port-mode]
      inherit`, `tests/common/state_delta.rs`).
- [x] Project Infrastructure Policy bootstrapped (was absent;
      `docs/architecture/atdd-infrastructure-policy.md` written with the cumulative
      slice-01..05 entries — §11).
- [x] `traceability.md` written: every scenario → story → J-005 (+ sub-jobs) → WD
      lock → ADR → journey step → release gate → KPI.
- [x] `wave-decisions.md` written: DD-AV-1..DD-AV-14.
- [x] All 6 KPIs mapped to acceptance scenarios (or to telemetry — KPI-AV-1/4/5/6
      rates — §8).
- [ ] **Tier B state-machine acceptance** (DD-AV-9): DEFERRED to DELIVER — the
      ingest→store→query lifecycle qualifies, but Tier B `@rule`s must invoke
      existing Tier A step-methods that land in the bootstrap. Open Item 9.
- [ ] **Pre-DELIVER fail-for-right-reason gate**: DEFERRED until DELIVER
      bootstraps the slice-05 indexer subsystem (the `appview-domain` crate + 4
      ports + 4 effect crates + the `openlore-indexer` binary + the cli `search`
      dispatch + the test-support bodies + the 3 `[[test]]` registrations). See
      §12 + DD-AV-13.

Handoff-ready: **YES**, conditional on DELIVER's first slice-05 step landing the
indexer subsystem + the cli `search` dispatch + the test-support ingest
harness/fixtures/helper bodies + the three `[[test]]` registrations before
running the suite the first time.

---

## 14. Open items for DELIVER

1. **Bootstrap the slice-05 indexer subsystem** (DD-AV-13; DEVOPS open-q 4): the
   pure `crates/appview-domain` crate + ADTs; the four NEW ports + ADTs in
   `crates/ports/`; the `claim-domain` `decode_ed25519_multibase`; the four effect
   crates (`adapter-atproto-ingest`/`adapter-index-store`/`adapter-xrpc-query-server`/
   `adapter-index-query`) with `impl <Port>` + non-stub `probe()` (the substrate-lie
   checks, DESIGN §6.3); the `openlore-indexer` binary (the second composition root,
   wire→probe→use + the `capability_boundary_probe`); the cli `search` dispatch; the
   test-support ingest harness + fixtures + helper bodies; the three `[[test]]`
   registrations (DD-AV-14). At that point the suite classifies RED.
2. **Materialize the ingest harness in `support/mod.rs`** — `FakeIngestSource`
   (bounded fixture record source hosting `listRecords`, INPUT-VALIDATING like the
   real adapter), the fixture PLC resolver (carrying a real `z6Mk` keypair),
   `IndexerHandle` (spawns `openlore-indexer serve` on an ephemeral `:0` port, reads
   it back, points the CLI's `[appview] indexer_url` at it; the slice-04 `FakePds`/
   `PeerPds` runtime-ownership pattern), `seed_network_index(NetworkIndexFixture::*)`,
   `run_openlore_indexer`, and the search/banner/relationship/anti-merging assertion
   helpers (scaffolded by THIS wave; bodies `todo!()`). See per-helper docstrings
   for the universe + contract.
3. **Materialize `crates/test-support/src/fixtures_ingest.rs`** — the
   `RawRecordSpec` recipes for the valid + adversarial (unsigned / tampered-sig /
   cid-mismatch) records + the real-`z6Mk` DID-document fixture (a known test
   keypair). Consider `cargo xtask regenerate-ingest-fixtures` (D-D38, DELIVER-may-defer).
4. **Implement the pure `appview-domain` core**: `ingest_decision` MUST call
   `claim_domain::verify` + `compute_cid` (no second path; AVC-1/AV-3); `compose_results`
   MUST group by author with no merged-row API (AVC-2/AVC-5); `near_match_suggestion`
   (AVC-8); `annotate_counter_relationship` shows-not-applies (AVC-6/AV-25). Pin the
   exhaustive per-arm + decode-boundary + mutation tests in the crates'
   `#[cfg(test)] mod tests` (layer 1 — out of DISTILL scope, DELIVER's call).
5. **Implement the `cli` `search` verb**: the dimension router, the network-result
   renderer (per-author groups + `[verified]` + relationship labels + the up-front
   public-data banner + the no-merge footer + the `peer add` affordance for
   unfollowed authors), the `--show` verification lines (the SAME pure-core result;
   no second path), the `--share` query-encoding link emitter + the CLI re-run
   resolver. Fill the exact line formats AV-8/AV-9/AV-10/AV-23/AV-26 assert
   (Q-DELIVER-AV-3/7).
6. **Implement the `openlore-indexer` binary + its adapters**: the bounded
   pull-ingest loop (per-record/per-source fault isolation, ADR-024); the
   `index.duckdb` schema (Q-DELIVER-AV-1; every SQL projects `author_did`, no
   author-eliding aggregate); the HTTP query server (Q-DELIVER-AV-2; per-result
   `author_did` on the wire); the verify-only `IdentityResolvePort` real PLC z6Mk
   decode (Q-DELIVER-AV-6; the seam release-forbidden).
7. **xtask check-arch slice-05 extension**: `no_cross_table_join_elides_author`
   extends to the `adapter-index-store` SQL; add `indexer_holds_no_signing_or_local_store`
   (I-AV-5 structural; AV-5 is the behavioral layer); add `no_pubkey_seam_in_release_build`
   (I-AV-6 structural; AV-4 is the gold behavioral layer); add `appview-domain` to
   the pure-core allowlist; I-3 covers BOTH binaries.
8. **State-delta migration** (DD-AV-10): migrate the load-bearing scenarios
   (AV-3, AV-9, AV-13, AV-5, AV-22) to explicit `assert_state_delta(before, after,
   universe, expected)` form once the helper bodies are real. The universe entries
   are listed in each scenario docstring's "Universe (port-exposed)" line + in
   wave-decisions.md §"Open questions handed to DELIVER".
9. **Tier B state-machine acceptance** (DD-AV-9 / CM-G): add
   `tests/acceptance/appview_state_machine.rs` over an `InMemoryComposition`
   (in-memory `IngestSourcePort`/`IndexStorePort` doubles), `@rule`(ingest a
   record) / `@invariant`(every stored row verified+attributed; distinct_author_count
   == COUNT DISTINCT author_did) — invoking the SAME Tier A step-method vocabulary
   (`when_the_indexer_ingests`, `then_every_stored_row_is_verified_and_attributed`).
   Authored ONLY after the harness step-methods land (Mandate 10 shared-vocabulary
   contract). Slice-05 is the surface the slice-04 CM-G forward-note flagged.
10. **DEVOPS coordination** (parallel, D-D35..D-D43): the three release-blocking
    guardrail ATs (`at-indexer-rejects-unverified-claim` = AV-3,
    `at-network-result-preserves-attribution` = AV-9, `at-local-first-preserved` =
    AV-13) + the seven search-scenario ATs land in `ci.yml`; the two Pact suites
    (B1 = D-D36, B2 = D-D37) consume the same DTO/fixture shapes; the `appview-domain`
    crate enters the nightly mutation `--package` list (D-D40); `scripts/kpi-av-*.jq`
    + the runtime counter `indexer_query_attribution_missing_total`. Not DELIVER
    deliverables per se, but the AT names AV-3/AV-9/AV-13 are the CI gate handles.

---

## 15. References

- `docs/feature/openlore-appview-search/distill/wave-decisions.md`
- `docs/feature/openlore-appview-search/distill/traceability.md`
- `tests/acceptance/appview_search.rs`
- `tests/acceptance/indexer_ingest.rs`
- `tests/acceptance/appview_core.rs`
- `tests/acceptance/support/mod.rs` (slice-05 indexer harness + seeders + helpers)
- `crates/test-support/src/fixtures_ingest.rs`
- `docs/architecture/atdd-infrastructure-policy.md` (bootstrapped this wave)
- DISCUSS: `docs/feature/openlore-appview-search/feature-delta.md` (WD-100..110 +
  OD-AV-1..7) + `discuss/{user-stories,story-map,outcome-kpis,shared-artifacts-registry,journey-discover-across-the-network-visual}.md`
- DESIGN: `docs/feature/openlore-appview-search/design/{architecture-design,component-boundaries,data-models,technology-stack,wave-decisions}.md`
- DEVOPS: `docs/feature/openlore-appview-search/devops/{wave-decisions,ci-cd-pipeline,contract-test-ownership,observability,platform-design,kpi-instrumentation}.md`
- ADRs: ADR-023 (self-hostable single-binary indexer), ADR-024 (pull-based bounded
  ingestion / Firehose re-eval), ADR-025 (network index = separate index.duckdb +
  anti-merging), ADR-026 (production PLC pubkey decode), ADR-027 (search verb +
  CLI→indexer transport + graceful degradation)
- SSOT: `docs/product/journeys/discover-across-the-network.yaml` (to be produced)
  + `docs/product/jobs.yaml` (J-005 + sub-jobs J-005a/b/c)
- Inherited from prior slices:
  - `docs/feature/openlore-scoring-graph/distill/{acceptance-tests,wave-decisions,traceability}.md` (STRUCTURAL TEMPLATE)
  - `docs/feature/openlore-federated-read/distill/{acceptance-tests,wave-decisions,traceability}.md`
  - `tests/acceptance/{walking_skeleton,...,graph_query_explore,scoring_core}.rs`
  - `crates/test-support/src/{lib,fake_pds,fake_peer_pds,fixtures,fixtures_peer,fixtures_github,fixtures_scoring,identity}.rs`
