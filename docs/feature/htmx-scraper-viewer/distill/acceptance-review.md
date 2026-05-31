# DISTILL Acceptance Self-Review: htmx-scraper-viewer (slice-06)

Self-review against `nw-ad-critique-dimensions` (8 dimensions) + the mandate
compliance checks, before hand-off to DELIVER. Reviewer output is ephemeral; this
file records the self-review verdict + the coverage matrices.

## Story coverage matrix (Dimension 4 / 8 — every US-VIEW-001..005 has ≥1 scenario)

| Story | Release / MoSCoW | Happy | Edge | Error | Boundary | Guardrail | Total |
|-------|------------------|-------|------|-------|----------|-----------|-------|
| US-VIEW-001 | WS / Must | V-1, V-3 | V-2 | V-4 | — | V-INV-1/3/4 | 4 + 3 |
| US-VIEW-002 | R1 / Must | V-5 | V-6 | V-7 | — | V-INV-2/3 | 3 + 2 |
| US-VIEW-003 | R2 / Should | V-8 | V-9 | — | V-10 | V-INV-2/3 | 3 + 2 |
| US-VIEW-004 | R2 / Should | V-11 | V-13 | — | V-12 | — | 3 |
| US-VIEW-005 | R3 / Could | V-S1 | V-S3 | V-S4 | — | V-INV-1/2 | 3 + 2 |

**Verdict: PASS.** Every story has ≥1 scenario; every story has happy + at least
one non-happy (edge/error/boundary) path; the hard invariants get dedicated gold
guardrails. (AC coverage: every AC-00X.Y in `acceptance-criteria.md` maps to a
scenario or a guardrail — including the @property ACs AC-001.5/.6, AC-002.4,
AC-003.4, AC-004.4, AC-005.2/.5 → V-INV-1/2/3/4 + V-3.)

## Story × invariant coverage matrix

| Invariant | Meaning | Guarded by |
|-----------|---------|-----------|
| **I-VIEW-1** read-only | no route writes/signs | **V-INV-1** (`viewer_is_read_only` — row counts unchanged after EVERY route incl. POST /scrape) + V-INV-4 |
| **I-VIEW-2** no key in web process | process holds no signing key | **V-INV-4** (`web_process_holds_no_signing_key`) + V-3 (launch states read-only/no-key) |
| **I-VIEW-3** human gate in CLI | signing CLI-only; no sign control | V-S1 (no sign control on /scrape) + V-INV-4 (no sign control on store routes) |
| **I-VIEW-4** loopback only | binds 127.0.0.1 | V-3 (`base_url` is loopback) |
| **I-VIEW-5 / WD-62** derived-from honesty | derived-from only on candidates | **V-INV-2** (`derived_from_only_on_candidates`) + V-S1 (present on /scrape) |
| **I-VIEW-6** offline | store views work offline | **V-INV-3** (`store_views_work_offline`) |
| FR-VIEW-8 | confidence numeric verbatim | V-1, V-5 (assert "0.90"/"0.95" verbatim) |
| BR-VIEW-2 | candidates ephemeral, never persisted | V-S1 + V-INV-1 (row counts unchanged after POST /scrape) |
| BR-VIEW-4 | same store as CLI | structural across the corpus (seeded via `claim add`/`peer pull`, read via the viewer) |
| BR-VIEW-5 | peer vs own distinct | V-8 (peer origin on a separate route) |

**Verdict: PASS.** Every hard invariant (I-VIEW-1..6) has a dedicated behavioral
gold guardrail or a direct assertion.

## Dimension-by-dimension self-review

| Dim | Check | Verdict |
|-----|-------|---------|
| 1 — Happy-path bias | error/edge ≥ 40% | **PASS** — 70% corpus-wide (10 explicit error/edge/boundary + 4 guardrails of 20) |
| 2 — GWT compliance | one behavior, single When | **PASS** — each scenario is one route + one observable outcome; docstrings carry the GWT |
| 3 — Business-language purity | no tech jargon in titles/steps | **PASS** — titles are operator outcomes ("operator sees their signed claims…", "unknown CID guides the operator back"); HTTP/SQL/DuckDB live only in step bodies/helpers, never in scenario names or GWT prose |
| 4 — Coverage completeness | every story + AC | **PASS** — see story matrix above |
| 5 — WS user-centricity | V-1 is a user goal, not a layer flow | **PASS** — "operator sees their signed claims in the browser with zero SQL"; Then = a browser observation, not a DB side-effect |
| 6 — Priority validation | scenarios target the real bottleneck | **PASS** — WS is the highest-priority Must story (US-VIEW-001 P1); /scrape (Could/P4) is the thinnest add-on, not over-invested |
| 7 — Observable-behavior assertions | assert returned value/observable, not internal state | **PASS** — every Then asserts rendered HTML text (`body_contains`), status code, the bound loopback URL, or the port-exposed read-only universe (row counts). No private-field/method-call assertions. The read-only universe uses port-exposed names (`claims.row_count`, `peer_claims.row_count`), never internal adapter fields (Mandate 8) |
| 8 — Traceability | story-tag + environment | **PASS (Check A)** — every scenario carries a `@us-view-00X` tag. **Check B**: DEVOPS dir absent → default environment (clean local machine + real DuckDB); the WS Given establishes the store precondition. `@escalate:PA` not needed — no infra-readiness finding |
| 9 — WS boundary proof | WS uses real adapters, every adapter has real I/O | **PASS** — V-1 is `@real-io` over the REAL DuckDB; the GitHub adapter's real-I/O coverage is V-S1/S3/S4 against the reused `FakeGithub` HTTP double (the driven-external default treatment per the project policy). WS strategy is the Architecture-of-Reference default (driving = real subprocess; driven-internal DuckDB = real; driven-external GitHub = fake) — no per-feature strategy renegotiation |

## Mandate compliance evidence

- **CM-A (Mandate 1, hexagonal boundary)**: all three files `use support::*` and
  drive `ViewerServer` (the `openlore ui` subprocess) + HTTP. Zero imports of
  `viewer-domain` / `adapter-http-viewer` / `adapter-duckdb` internals into the
  scenarios. Driving-port-only.
- **CM-B (Mandate 2/3, business language + journeys)**: scenario titles + GWT
  use operator domain language; complete journeys (start viewer → open page → see
  outcome). `grep` for tech terms in scenario titles → none.
- **CM-C (Mandate 5, WS)**: exactly ONE `@walking_skeleton` scenario (V-1),
  user-goal framed, demo-able.
- **CM-E (Mandate 8, universe-bound assertion)**: V-INV-1 / V-INV-4 assert the
  read-only delta via `assert_store_read_only` (state-delta over the port-exposed
  universe `{claims.row_count, peer_claims.row_count}`). Other layer-3 store
  scenarios assert observable rendered text (the universe at the HTTP/HTML port).
- **CM-F (Mandate 9, PBT mode)**: NO `proptest!` / generative machinery in the
  viewer ATs — all layer-3/5, example-only. The `@property` tag on the V-INV gold
  tests marks them as universal invariants for the reader/crafter, NOT a request
  to PBT-generate at this layer.
- **CM-H (Mandate 11, sad paths example-based)**: every sad path (V-4 unreadable
  store, V-7 unknown CID, V-9 no peers, V-10 missing origin, V-S3 zero candidates,
  V-S4 network down) is a NAMED example test with explicit triggering input. No
  PBT machinery imported at layer 3+.
- **Tier-B (Mandate 10)**: **NOT emitted.** The richest journey (the store views)
  is a set of independent read-renders, not a ≥3-step chained state-transition
  journey with a domain-rich input space; the input space is "which seeded rows
  render" (config-shaped reads), and the only state mutation to model is "did a
  route write?" — already covered by the V-INV-1 read-only gold test. Tier B would
  add no coverage the example gold tests don't already give. Correctly skipped.

## RED classification confirmation

`cargo test -p cli --test viewer_store --test viewer_scrape --test viewer_invariants`:

- **Compiles** (`--no-run` succeeds; only warnings).
- 20 viewer scenarios FAIL, **all with `not yet implemented: DELIVER (slice-06):
  ...`** (the `todo!()` scaffolds) — 0 non-todo panics. Classification:
  **MISSING_FUNCTIONALITY (correct RED)**, not IMPORT_ERROR / FIXTURE_BROKEN /
  SETUP_FAILURE. The pre-DELIVER fail-for-the-right-reason gate is GREEN.
- The `ViewerServer` subprocess helper compiles before `openlore ui` exists
  (runtime-resolved binary), guaranteeing RED-not-BROKEN.

## One-at-a-time DELIVER strategy (scenario → future roadmap step)

DELIVER enables ONE scenario at a time (the inner-loop discipline). Suggested
ordering — each row is one TDD cycle / roadmap step:

| Step | Scenario | What it forces into existence |
|------|----------|-------------------------------|
| 06-01 (WS) | **V-1** | Materialize `ViewerServer` body + `seed_own_claim_with_evidence`; implement the `openlore ui` verb + loopback bind + serve loop + `StoreReadPort::list_claims/count_claims` (adapter-duckdb) + `render_claims_page` (viewer-domain). The thinnest end-to-end slice. |
| 06-02 | V-3 | Launch read-only notice + loopback URL + the startup probe (ADR-030 §Earned-Trust step 2/3). |
| 06-03 | V-2 | Empty-state `render_empty(NoClaims)`. |
| 06-04 | V-4 | Startup store-readability refusal (`health.startup.refused`, ADR-030 step 1) + `run_openlore_ui_expecting_startup_refusal`. |
| 06-05 | V-5 | `StoreReadPort::get_claim` + `render_claim_detail` (+ evidence[]). |
| 06-06 | V-6 | "no evidence attached" detail state. |
| 06-07 | V-7 | Unknown-CID guided 404 (`render_error` + back link). |
| 06-08 | V-8 | `StoreReadPort::list_peer_claims/count_peer_claims` + `render_peer_claims_page` (peer origin from author_did + fetched_from_pds) + `seed_peer_claims_via_pull`. |
| 06-09 | V-9 | Empty-state `render_empty(NoPeers)`. |
| 06-10 | V-10 | `PeerOrigin::Unknown` defensive render + `seed_peer_claim_with_blank_origin`. |
| 06-11 | V-11 | Pagination: `PageRequest`/`Page<T>` (offset/limit, size 50) + `PageView` "X–Y of N" indicator + `?page=N` + `seed_own_claims_via_cli`. |
| 06-12 | V-12 | Last-page bounds (no Next at the bound). |
| 06-13 | V-13 | Single-page store (no pagination controls). |
| 06-14 | V-S1 | `/scrape` form + live propose (reuse `GithubPort` + `derive_candidates`) + `render_scrape_page` (candidates + derived-from, no sign control) + `ViewerServer::start_with_github`. |
| 06-15 | V-S3 | Zero-candidates `render_empty(NoCandidates)`. |
| 06-16 | V-S4 | NetworkDown render (plain-language + offline note, no transport internals — the DESIGN nit resolution). |
| 06-17 | V-INV-1 | `viewer_is_read_only` gold test + `capture_store_row_count_universe` / `assert_store_read_only`. |
| 06-18 | V-INV-2 | `derived_from_only_on_candidates` gold test (largely green once V-S1 + the persisted view-models — which have no derived-from slot — exist; may be "remove the skip" if the type-level invariant holds). |
| 06-19 | V-INV-3 | `store_views_work_offline` gold test (likely green once the store routes exist — offline by construction; may be "remove the skip"). |
| 06-20 | V-INV-4 | `web_process_holds_no_signing_key` gold test (largely green once the no-sign-control surfaces + read-only delta exist). |

The V-INV gold tests (06-17..20) are deliberately LAST: several should flip green
"for free" once the routes they guard exist (the correct signal that the
structural invariants were designed in, not bolted on) — per the "if a step finds
already-implemented, just remove the skip" discipline.

## Anything not grounded in the existing harness

- **`reqwest` added as a `cli` dev-dependency** (blocking client, rustls). The
  prior ATs queried localhost servers through a `search` SUBPROCESS, so the
  workspace had no in-test HTTP client; the viewer ATs need one to GET/POST the
  HTML directly. Version/TLS match `adapter-index-query` (0.12, rustls). Flagged
  for the reviewer as the one new test-infra dependency.
- **`viewer.serve.listening` stdout event** is ASSUMED by the readiness path
  (mirrors the existing `indexer.serve.listening`). DELIVER must emit it from the
  `ui` verb when the listener binds, OR the `ViewerServer` body falls back to a
  pure TCP-connect poll. Both are noted in the `start_inner` scaffold message.
- Everything else reuses established harness primitives verbatim: `TestEnv`,
  `GithubServer` + `FakeGithub` (slice-02), the `claim add`/`peer pull` production
  write paths (slice-01/03), the `state_delta` port (slice-01 bootstrap), and the
  `assert_cmd::cargo_bin` + ephemeral-`:0` long-running-server pattern (slice-05
  `spawn_indexer_serve`).
