# Wave Decisions — DELIVER — openlore-appview-search (slice-05)

- **Wave**: DELIVER
- **Date**: 2026-05-28..29
- **Orchestrator**: Main Claude instance (nw-deliver)
- **Crafter**: @nw-functional-software-crafter (ADR-007)
- **Roadmap**: `deliver/roadmap.json` — 43 steps, 5 phases, all COMMIT/PASS
- **Rigor**: legacy 5-phase TDD; review + L1-L6 refactor + per-feature mutation enabled; models inherit.

## Execution summary

All 43 roadmap steps executed via DES-monitored crafter dispatches, each commit
carrying a `Step-ID: NN-NN` trailer. All 38 slice-05 acceptance scenarios GREEN:
22 `appview_search` (AV-8..29) + 9 `appview_core` (AVC-1..8, with AVC-3 split into
AVC-3a/3b) + 7 `indexer_ingest` (AV-1..7; the `indexer_ingest` binary reports 9
incl. 2 ingest-support self-tests). slice-01/02/03/04 suites show zero regression.
SIX NEW crates shipped (the indexer subsystem / the first network service): 1 pure
(`appview-domain`) + 4 effect (`adapter-atproto-ingest`, `adapter-index-store`,
`adapter-xrpc-query-server`, `adapter-index-query`) + 1 binary driver
(`openlore-indexer`, the SECOND composition root); crate count 13 → 19.

| Phase | Scope | Result |
|---|---|---|
| 01 Bootstrap (01-01..05) | bootstrap `appview-domain` + `claim-domain` decode helper; hoist appview ADTs to `ports` + 4 indexer ports; scaffold 4 indexer effect adapters + extend anti-merging SQL rule; scaffold `openlore-indexer` binary + cli `search` verb + capability/seam xtask rules; materialize ingest fixtures + acceptance harness + 3 test targets | fail-for-right-reason gate (DD-AV-13); all 38 ATs compile RED |
| 02 appview-domain pure core (02-01..09) | AVC-1..8 (ingest_decision verify-before-index gate, author-from-signed-payload, ingest/compose determinism, anti-merging compose, two-author compose, universal verified marker, counter annotation, near_match_suggestion) | green |
| 03 indexer ingest walking skeleton (03-01..07) | AV-1..7 (wire ingest skeleton → live index.duckdb, anti-merging at ingest, cardinal verify-before-index gate, real z6Mk decode gold path, capability boundary, wire-probe-use startup refusal + substrate-lie probes, public-data-only ingest) | green |
| 04 search walking skeleton + trust surface (04-01..07) | AV-8..14 + AV-23 (wire search B1 serve+query+render, cardinal anti-merging at search, public-data banner, universal [verified] marker, cardinal local-first, B1 localhost transport + author_did/reachable probes, --show trust inspection) | green |
| 05 discovery funnel + share (05-01..15) | AV-12, AV-15..22, AV-24..29 (empty near-match suggestion, contributor/subject search, subscribed-peer labels, discovery→federation funnel via slice-03 peer add, read-only discovery, affordance suppression, zero-residue purge, --show absent-cid usage error, counter shown-not-applied, --share query-encoding + CLI re-run resolver + not-a-stale-snapshot) | green |

## DELIVER-wave decisions

| # | Decision | Rationale |
|---|---|---|
| DV-1 | DES `project_id` header added to execution-log right after `des-init-log` (same hook-defect workaround as slice-02/03/04 DV-1). | Stop-hook reads `project_id`; des-init-log writes `feature_id`. |
| DV-2 | Mutation = per-feature 100% on the new PURE `appview-domain` production functions (`ingest_decision` / `compose_results` / `near_match_suggestion`; Phase 6), matching slice-02/03/04 DV-2. | Per-feature gate at deliver-time + DEVOPS nightly sweep backstop. The slice-04 cross-package lesson informed keeping the killing properties IN-CRATE (`appview_core` AVC-1..8 proptests) so the per-feature measurement reaches the real killing suite locally. |
| DV-3 | **`hyper`, NOT `axum`, for the XRPC query server** (`adapter-xrpc-query-server`). Q-DELIVER-AV-2 resolved at deliver-time. | `axum` is `deny.toml`-banned (it pulls a transitive tree the supply-chain policy narrows out); a hand-rolled `hyper` handler serves the single `org.openlore.appview.searchClaims` route with no new banned dependency. ADR-027 fixed the XRPC method + response shape; the framework was DELIVER's call within the rustls/`reqwest` ecosystem + `cargo deny` allowlist. |
| DV-4 | Found + fixed a REAL pre-existing **slice-03 bug**: `adapter-duckdb::hard_purge` failed a DuckDB FK constraint deleting `peer_claims` + child evidence in ONE transaction. Fixed via a two-transaction purge (commit `bf6df62`); slice-03 ATs still 10/10. | Masked by slice-03's childless PS-6 fixture; exposed by the slice-05 AV-22 discovery-funnel pull, which is the first time the purge path met a peer WITH child evidence. A latent slice-03 defect would have shipped silently; the AV-22 funnel surfaced it. |
| DV-5 | Added the `references` field to the `SearchResultDto` wire shape (a `data-models.md` gap; step 05-11). | The OD-AV-7 counter annotation (`annotate_counter_relationship`) needs the references graph across the CLI↔indexer transport to annotate "countered-by <cid> (by <did>)" on the CLI side. The DTO had to carry it. |
| DV-6 | Release-gated the slice-03 `OPENLORE_PEER_PUBKEY_HEX` seam via `cfg(debug_assertions)` + broadened the xtask `no_pubkey_seam_in_release_build` rule (commit `d6c8d9a`, ADR-026). | I-AV-6: production verification MUST use the REAL PLC `z6Mk...` decode; the slice-03 DV-4 test-only seam is release-forbidden. Closed a slice-03 carry-over (the seam was previously test-only by convention, not enforced in release builds). |
| DV-7 | Mutation-hardening: a targeted survivor-kill pass (D-D40, commit `4a1a357`) closed the last production survivors — confidence carry, levenshtein arithmetic, counter tiebreak — to 100% on `appview-domain` production functions. The 6 remaining survivors are in the proptest STRATEGY generators (test-infra), correctly out of D-D40 scope. | A generator mutation only narrows the explored input space; it cannot make a wrong production answer pass. The per-feature gate is on production functions. |

## Demo Evidence — 2026-05-28..29

The slice-05 search demos require a LIVE indexer; the acceptance harness stands one
up (a real `openlore-indexer` serving a live `index.duckdb`, queried by the real
`openlore` binary). The two **walking-skeleton acceptance tests are the executable
end-to-end demos**, both GREEN:

| Demo | AT | What it proves end-to-end (green) |
|---|---|---|
| Indexer ingest walking skeleton | **AV-1** (`indexer_ingest`, step 03-01) | The `openlore-indexer` ingests a public signed claim into a live `index.duckdb` — wire→probe→use through the real binary against a real index store. The ingest-side end-to-end demo. |
| Search walking skeleton | **AV-8** (`appview_search`, step 04-01) | B1 serve+query+render: the `openlore search` verb queries the live indexer over the localhost XRPC transport and renders per-author results. The search-side end-to-end demo. |

These two skeletons are the load-bearing demo evidence (the slice-02/03/04 model:
the walking skeleton through the real binary IS the demo). The remaining
user-visible capabilities are demonstrated by their GREEN acceptance scenarios
driving the real `openlore` + `openlore-indexer` binaries against a live seeded
`index.duckdb`:

| Story | Demo coverage (green acceptance scenario, real binaries + live index.duckdb) |
|---|---|
| US-AV-001 (@infrastructure / ingest) | AV-1 (ingest walking skeleton), AV-2 (anti-merging at ingest), AV-3 (cardinal verify-before-index gate), AV-4 (real z6Mk decode), AV-5 (capability boundary), AV-6 (wire-probe-use startup refusal), AV-7 (public-data-only ingest) |
| US-AV-002 (search by philosophy) | AV-8 (search walking skeleton), AV-9 (cardinal anti-merging at search), AV-10 (public-data banner), AV-11 (universal [verified] marker), AV-12 (empty → near-match suggestion, exit 0) |
| US-AV-003 (search by contributor/subject) | AV-15 (contributor trail, honest framing), AV-16 (subject-dimension anti-merging), AV-17 (empty contributor degrades, exit 0) |
| US-AV-004 (trust + inspect) | AV-23 (--show signature + cid-match lines), AV-24 (--show absent-cid usage error, nonzero exit) |
| US-AV-005 (discovery → federation funnel) | AV-18 (subscribed-peer label, no redundant affordance), AV-19 (funnel reuses slice-03 peer add), AV-20 (never auto-subscribes), AV-21 (affordance suppressed for subscribed peer), AV-22 (zero-residue purge via slice-03) |
| US-AV-006 (shareable discovery) | AV-26 (--share emits query-encoding link), AV-27 (CLI re-run resolver), AV-28 (current results, not a stale snapshot), AV-29 (--share contributor round-trip) |
| Local-first (cardinal) | AV-13 (offline authoring + soft search degradation), AV-14 (B1 transport + author_did/reachable probes) |
| Counter shown-not-applied | AV-25 (countered claim shown with annotation, not applied) |

Cardinal trust invariants end-to-end verified: every indexed/searched/shared result
carries a non-`Option` author DID (anti-merging at network scale — AV-9 / AVC-2;
no merged/consensus row exists), every result is `[verified]` by construction
(verify-before-index gate reusing the pure `claim_domain::verify`, no second path —
AV-3 / AV-11 / AVC-1; production uses the real z6Mk decode — AV-4), the indexer is
signing-incapable + holds no local store (AV-5), offline authoring is uncompromised
(AV-13), the discovery funnel reuses slice-03 `peer add` verbatim (AV-19), and the
`--share` link encodes the query not a snapshot (AV-28).

## Post-Merge Integration Gate — PASS

- Full slice-05 acceptance suite GREEN (appview_search 22 [AV-8..29] + appview_core
  9 [AVC-1..8, AVC-3 split] + indexer_ingest 7 [AV-1..7; binary reports 9 incl. 2
  support self-tests]); slice-01/02/03/04 suites zero regression (the full workspace
  acceptance suite green across all slices). xtask guards green (anti-merging rule
  extended to `adapter-index-store`; `indexer_holds_no_signing_or_local_store` +
  `no_pubkey_seam_in_release_build` active).
- Environment matrix: slice-05 acceptance is hermetic (the harness stands up a live
  `openlore-indexer` + a seeded `index.duckdb` + a `tempfile` HOME) and does NOT
  depend on a per-environment cross-product; the default matrix is satisfied by the
  hermetic design (same rationale as slice-02/03/04; DEVOPS graceful-degrade default).
- Known harness flake (NOT a slice-05 regression): the `adapter-system-clock`
  `now_utc_*` env-var contention under full-workspace PARALLEL lib-test runs (carried
  from slice-01/03/04); the acceptance targets pass single-threaded / in isolation.
  AV-4 env test-isolation was cleared in the Phase 4 refactor.

## Quality gates

- `cargo xtask check-arch`: OK (19 workspace members) — `appview-domain` pure-core
  allowlist + the anti-merging SQL rule extended to `adapter-index-store` + the new
  `indexer_holds_no_signing_or_local_store` + `no_pubkey_seam_in_release_build`
  rules; I-3 (composition-root rule) covers BOTH binaries (disjoint roots).
- `cargo xtask check-probes`: OK — the four new adapters' non-stub `probe()` bodies
  picked up (`adapter-atproto-did` resolve-probe de-allowlisted in the refactor);
  `appview-domain` correctly requires no `probe()` (pure crate).
- Per-phase L1-L6 refactor / adversarial review / mutation / integrity outcomes
  recorded below (Phases 4–7).

## Phase 4 — L1-L6 refactoring: cleared deferred debt

@nw-functional-software-crafter cleared the deferred DELIVER debt (commits `baebe89`
+ `d6c8d9a`):
- AV-4 env test-isolation (the `now_utc_*` / pubkey-seam env contention isolated).
- `appview-domain` clippy: `large_enum_variant` (boxed the large `IngestOutcome`
  variant) + doc-indent.
- `AtProtoDidAdapter` resolve-probe de-allowlist (the real resolve-probe replaced the
  allowlisted stub).
- The `OPENLORE_PEER_PUBKEY_HEX_<did>` seam release-gated via `cfg(debug_assertions)`
  (ADR-026, DV-6) + the broadened xtask `no_pubkey_seam_in_release_build` rule.
Pure-core purity intact (no I/O imports in `appview-domain`; ADTs make illegal states
unrepresentable — non-`Option` `author_did` everywhere, `IngestOutcome` /
`RejectReason` choice types).

## Phase 5 — Adversarial review: APPROVED (zero blockers)

@nw-software-crafter-reviewer verdict APPROVED. Zero blockers; Testing Theater clean
across all 43 steps. The two cardinal release gates verified load-bearing
(verify-before-index AV-3 rejects adversarial fixtures before the index, reusing the
pure core with no second path; anti-merging-at-network-scale AV-9 three-layer
enforcement verified real — non-`Option` author DID types + the xtask SQL rule on
`adapter-index-store` + behavioral). Local-first (AV-13) confirmed (the CLI links no
indexer code; `search` degrades softly; disjoint composition roots). The capability
boundary (AV-5), the real z6Mk decode (AV-4), and counter-shown-not-applied (AV-25)
all PASS.

## Phase 6 — Mutation testing (per-feature 100% on `appview-domain` production functions): PASS

Scope: the new pure `appview-domain` production functions — `ingest_decision`,
`compose_results`, `near_match_suggestion`. The slice-04 cross-package lesson was
applied from the start (the `appview_core` AVC-1..8 properties pin the production
functions IN/against the `appview-domain` crate, so the per-feature mutation
measurement reaches the real killing suite without slice-04's cross-package detour).

| Mutant category | Tested | Caught | Kill rate |
|---|---:|---:|---|
| `ingest_decision` / `compose_results` / `near_match_suggestion` production logic (incl. the D-D40 targeted kills: confidence carry, levenshtein arithmetic, counter tiebreak) | 37 | 37 | **100%** |

The targeted D-D40 pass (commit `4a1a357`, DV-7) closed the last production
survivors to 100%. The 6 remaining survivors are in the proptest STRATEGY generators
(`proptest_strategies`) — test-infrastructure, OUT of the D-D40 production-kill scope
(a generator mutation only narrows the explored input space; it cannot make a wrong
production answer pass). Gate SATISFIED (≥80%; actual 100% on the production scope).
DEVOPS nightly sweep is the ongoing backstop.

## Phase 7 — Deliver integrity verification: PASS

`des-verify-integrity docs/feature/openlore-appview-search/deliver/` → "All 43 steps
have complete DES traces" (exit 0).
</content>
