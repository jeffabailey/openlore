# DISTILL Test Scenarios: htmx-scraper-viewer (slice-06)

> Every acceptance scenario in Given-When-Then, with its V-id, the US-VIEW story
> it traces to, the route it exercises, and the invariant it guards (if any).
> The `.rs` acceptance files are the executable SSOT; this doc is the readable
> index + traceability matrix. Authored by Quinn (nw-acceptance-designer).

## Gate log (Phase 0/1/1.5)

- `[lang-mode] rust` · `[policy-mode] inherit` · `[port-mode] inherit`
  (`docs/architecture/atdd-infrastructure-policy.md` + `tests/common/state_delta.rs`
  both present — inherited, not bootstrapped this run).
- **Driving port identified** (Architecture SSOT present): the NEW `openlore ui
  --port <P>` verb (ADR-028) — a long-running hyper server on 127.0.0.1, no auth,
  no key. Scenarios spawn it as a subprocess and issue HTTP GET/POST.
- **Wave-Decision Reconciliation HARD GATE: PASSED — 0 contradictions.** No
  `wave-decisions.md` exists for any prior wave (discuss/design/devops), so there
  are no recorded decisions to contradict. (DEVOPS dir absent → WARN, default
  environment assumed: a clean local machine with a real DuckDB store.)

## Verb-name reconciliation (recorded resolution)

The DISCUSS user-stories used the placeholder verb **`openlore viewer`**; DESIGN
+ **ADR-028 (governing the verb shape)** settled it as **`openlore ui --port`**.
ADR-028 is the authoritative DESIGN decision, so the corpus uses `openlore ui`
throughout. This is a naming refinement, not a behavioral contradiction (same
read-only localhost viewer), so it does NOT trip the reconciliation gate.

## DISTILL resolution of the DESIGN low-nit (`/scrape` NetworkDown rendering)

DESIGN left one low nit open: how the `/scrape` NetworkDown state renders. **This
DISTILL resolves it** in scenario **V-S4** by pinning the assertion: the
network-down render (a) names the cause in domain language — "GitHub could not be
reached"; (b) reassures that the offline store view still works (NFR-VIEW-7); and
(c) leaks **NO** transport internals — no HTTP status codes, no
"connection refused"/"timed out"/"DNS", no raw URLs, no stack trace (NFR-VIEW-6).

## Build-before-run note (carry into DELIVER roadmap)

Like the slice-05 indexer ATs, `cargo test` does **NOT** rebuild a spawned binary
automatically. The roadmap/run MUST `cargo build` the `openlore` bin BEFORE
running these viewer ATs so `ViewerServer` spawns the CURRENT `openlore ui`, not
a stale one. (`reqwest` was added as a `cli` dev-dependency for the in-test HTTP
client; the indexer ATs queried through a `search` subprocess and never needed
one.)

## Driving / hexagonal discipline

- Every scenario enters through the CLI **driving port** — the REAL `openlore ui`
  subprocess (via the `ViewerServer` spawn helper) + in-test HTTP. NO scenario
  calls a `viewer-domain` render fn directly (those are unit-level, DELIVER's
  concern).
- The local **DuckDB is REAL** (BR-VIEW-4 — the SAME store the CLI writes), seeded
  through the PRODUCTION `claim add` / `peer pull` write paths (Pillar 3).
- **GitHub is the ONLY mocked boundary** — the REUSED slice-02 `FakeGithub` double
  (via `GithubServer`), wired into the viewer through `OPENLORE_GITHUB_API_BASE`,
  and only on `/scrape`. No new GitHub double was built.

---

## `tests/acceptance/viewer_store.rs` — store views (US-VIEW-001/002/003/004)

| V-id | Story | Route | Scenario (GWT summary) | Invariant guarded | Tags |
|------|-------|-------|------------------------|-------------------|------|
| **V-1** (WALKING SKELETON) | US-VIEW-001 | `GET /claims` | **G** Maria signed ("rust-lang/rust","is-maintained-by","The Rust Project") @0.90 via the CLI · **W** she starts `openlore ui` + opens My Claims · **T** sees that row (subject/predicate/object/0.90 verbatim/CID), zero SQL | FR-VIEW-8 (confidence verbatim) | `@walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1 @happy` |
| V-2 | US-VIEW-001 | `GET /claims` | **G** Tom signed nothing · **W** opens My Claims · **T** sees guidance "claims you sign with the CLI appear here", not a blank page | FR-VIEW-7 | `@driving_port @real-io @empty-state @edge` |
| V-3 | US-VIEW-001 | process start + `GET /` | **G** Maria has a local store · **W** starts `openlore ui` · **T** loopback listen URL + "read-only" stated; no key loaded | I-VIEW-2/3/4 | `@driving_port @driving_adapter @real-io @i-view-2 @i-view-4 @happy` |
| V-4 | US-VIEW-001 | process start (refusal) | **G** store file locked by another process · **W** starts the viewer · **T** plain-language refusal naming the path + "another process", no stack trace | NFR-VIEW-6 (startup probe, ADR-030) | `@driving_port @real-io @infrastructure-failure @error` |
| V-5 | US-VIEW-002 | `GET /claims/{cid}` | **G** claim has two evidence URLs · **W** opens its detail page · **T** all fields + both evidence URLs | FR-VIEW-3 | `@driving_port @real-io @happy` |
| V-6 | US-VIEW-002 | `GET /claims/{cid}` | **G** claim signed without evidence · **W** opens detail · **T** "no evidence attached", not a blank section | FR-VIEW-3 | `@driving_port @real-io @empty-state @edge` |
| V-7 | US-VIEW-002 | `GET /claims/{cid}` | **G** CID not in store · **W** opens that detail page · **T** "No claim with that identifier in your store" + back link | NFR-VIEW-6 (guided 404) | `@driving_port @real-io @error` |
| V-8 | US-VIEW-003 | `GET /peer-claims` | **G** Maria federated peer claims (+ has own) · **W** opens Peer Claims · **T** federated rows show peer origin (author_did), distinct route from own | BR-VIEW-5 | `@driving_port @real-io @happy` |
| V-9 | US-VIEW-003 | `GET /peer-claims` | **G** federated nothing · **W** opens Peer Claims · **T** "No federated claims yet" guidance | FR-VIEW-7 | `@driving_port @real-io @empty-state @edge` |
| V-10 | US-VIEW-003 | `GET /peer-claims` | **G** a peer claim has blank/absent origin · **W** opens Peer Claims · **T** that claim still renders, origin "unknown" (not dropped) | FR-VIEW-4 (defensive render) | `@driving_port @real-io @boundary @edge` |
| V-11 | US-VIEW-004 | `GET /claims?page=N` | **G** 312 claims, page size 50 · **W** opens My Claims + goes to page 2 · **T** "1–50 of 312" then "51–100 of 312" | FR-VIEW-6 (pagination) | `@driving_port @real-io @pagination @happy` |
| V-12 | US-VIEW-004 | `GET /claims?page=7` | **G** on the last page of 312 · **T** "301–312 of 312" + no link to page 8 | FR-VIEW-6 (bounds) | `@driving_port @real-io @pagination @boundary` |
| V-13 | US-VIEW-004 | `GET /claims` | **G** 12 claims, page size 50 · **W** opens My Claims · **T** "1–12 of 12" + no pagination controls | FR-VIEW-6 (single page) | `@driving_port @real-io @pagination @edge` |

## `tests/acceptance/viewer_scrape.rs` — live-scrape view (US-VIEW-005)

| V-id | Story | Route | Scenario (GWT summary) | Invariant guarded | Tags |
|------|-------|-------|------------------------|-------------------|------|
| V-S1 | US-VIEW-005 | `POST /scrape` | **G** a live scrape of "rust-lang/cargo" proposes candidates (FakeGithub) · **W** Maria submits that target · **T** candidates render (subject/predicate/object/confidence + **derived-from**); page states "nothing signed/saved"; **NO sign control**; directed to the CLI | BR-VIEW-1/2 + I-VIEW-5 (derived-from present here) | `@driving_port @driving_adapter @real-io @derived-from @happy` |
| V-S3 | US-VIEW-005 | `POST /scrape` | **G** "some-org/empty-repo" derives no candidates · **W** submits it · **T** "No candidate claims could be derived" + suggested alternative | FR-VIEW-7 | `@driving_port @real-io @empty-state @edge` |
| V-S4 | US-VIEW-005 | `POST /scrape` | **G** GitHub unreachable (`FakeGithub::offline()`) · **W** submits "tokio-rs/tokio" · **T** "GitHub could not be reached" + "store view still works offline"; **NO leaked transport internals** | NFR-VIEW-6/7 (**DESIGN nit resolution**) | `@driving_port @real-io @network-failure @error` |

> The cross-view half of AC-005.2 ("derived-from NEVER on persisted claims") is
> the gold guardrail **V-INV-2** below (it spans `/scrape` + `/claims`).

## `tests/acceptance/viewer_invariants.rs` — gold / guardrail invariants

| V-id | Gold name | Route(s) | Scenario (GWT summary) | Invariant guarded | Tags |
|------|-----------|----------|------------------------|-------------------|------|
| V-INV-1 | `viewer_is_read_only` | ALL incl. `POST /scrape` | **G** store seeded (own + peer) + reachable target · **W** exercise EVERY route incl. POST /scrape · **T** `claims` + `peer_claims` row counts UNCHANGED (universe-bound `assert_store_read_only`) | **I-VIEW-1** (read-only) | `@property @driving_port @real-io @i-view-1 @gold @kpi-view-2` |
| V-INV-2 | `derived_from_only_on_candidates` | `/scrape` + `/claims` + `/claims/{cid}` + `/peer-claims` | **G** own + peer persisted + live target · **W** render the live + every persisted view · **T** derived-from on candidates, ABSENT on every persisted view | **I-VIEW-5 / WD-62** | `@property @driving_port @real-io @i-view-5 @wd-62 @gold` |
| V-INV-3 | `store_views_work_offline` | `/claims` + `/claims/{cid}` + `/peer-claims` | **G** network unavailable + own + peer persisted · **W** render the store views · **T** each renders fully from the local store | **I-VIEW-6 / KPI-VIEW-5** | `@property @driving_port @real-io @i-view-6 @offline @gold` |
| V-INV-4 | `web_process_holds_no_signing_key` | `/` + `/claims` + `/claims/{cid}` + `/peer-claims` | **G** populated store + viewer running · **W** request every route · **T** no sign control on any page + store row counts unchanged (no write/sign) | **I-VIEW-1/2 / I-SCR-1** | `@property @driving_port @real-io @i-view-1 @i-view-2 @i-scr-1 @gold` |

---

## Error / edge-path ratio (Mandate: ≥ 40%)

| File | Total | Error/Edge/Boundary | Ratio |
|------|-------|----------------------|-------|
| viewer_store | 13 | V-2, V-4, V-6, V-7, V-9, V-10, V-12, V-13 (8) | 62% |
| viewer_scrape | 3 | V-S3, V-S4 (2) | 67% |
| viewer_invariants | 4 | guardrail invariants (read-only/offline/honesty/no-key) | guardrail |
| **Corpus** | **20** | **10 explicit error/edge/boundary + 4 guardrails** | **70%** |

Comfortably above the 40% floor: every story carries happy + edge + error
coverage, and the hard invariants get dedicated gold guardrails.

## Adapter coverage (Mandate 6)

| Driven port / adapter | Treatment | Real-I/O scenario(s) |
|-----------------------|-----------|----------------------|
| `StoreReadPort` (adapter-duckdb, read-only) | REAL DuckDB (driven-internal) | V-1, V-5, V-8, V-11, V-INV-1/3 (every store route over the REAL store) |
| `GithubPort` (adapter-github) | FAKE (`FakeGithub`, driven-external) — reused slice-02 | V-S1 (harvest), V-S3 (zero candidates), V-S4 (offline) |
| `openlore ui` HTTP surface (adapter-http-viewer) | REAL subprocess (driving) | ALL scenarios (spawned via `ViewerServer` + HTTP) |

No new external integration: the `/scrape` route reuses the slice-02 GitHub
boundary verbatim (ADR-028 handoff note). The store-read boundary is purely local
(offline by construction).

## RED classification (pre-DELIVER fail-for-the-right-reason gate)

All 20 viewer scenarios classify **RED = MISSING_FUNCTIONALITY** (panic at the
`todo!()` scaffolds in the seeding/harness helpers), NOT BROKEN
(ImportError/compile-error). Verified:

- `cargo test -p cli --test viewer_store --test viewer_scrape --test viewer_invariants --no-run` → COMPILES.
- Run: `viewer_store` 13 failed, `viewer_scrape` 3 failed, `viewer_invariants` 4
  failed — every panic message is `not yet implemented: DELIVER (slice-06): ...`
  (0 non-todo panics). The 2 "passed" per file are the pre-existing
  `state_delta` skeleton self-tests (compiled in via `#[path]`), not viewer
  scenarios.

The `ViewerServer` spawn helper compiles BEFORE `openlore ui` exists (the binary
is resolved at RUNTIME via `assert_cmd::cargo_bin`), so the corpus is RED, not
BROKEN — the correct DISTILL hand-off state.
