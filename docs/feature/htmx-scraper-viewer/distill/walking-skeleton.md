# DISTILL Walking Skeleton: htmx-scraper-viewer (slice-06)

## V-1 — the walking-skeleton scenario

**File**: `tests/acceptance/viewer_store.rs` → `operator_sees_their_signed_claims_in_the_browser_with_zero_sql`
**Tags**: `@us-view-001 @walking_skeleton @driving_port @driving_adapter @real-io @kpi-view-1 @happy`

```gherkin
Given Maria has signed a claim ("rust-lang/rust","is-maintained-by","The Rust Project")
  at confidence 0.90 through the CLI
When she starts `openlore ui` and opens the My Claims page in her browser
Then she sees that claim as a row with subject, predicate, object, confidence 0.90,
  and its CID
And she wrote no SQL
```

## Why this is the thinnest end-to-end thread

V-1 closes the single load-bearing loop for the entire slice:

```
HTTP GET /claims  ->  read-only StoreReadPort  ->  REAL DuckDB query
   ->  viewer-domain render_claims_page  ->  HTML out
```

It is demo-able to a non-technical stakeholder ("Maria opens her browser and sees
the claim she signed — no SQL"), which is the litmus test for a user-centric
walking skeleton. It exercises, as a CONSEQUENCE of the user journey (not as a
design goal), every layer the slice introduces:

- the NEW `openlore ui` driving adapter (verb wiring + hyper serve loop, ADR-028);
- the NEW read-only `StoreReadPort` over the REAL DuckDB (ADR-030, BR-VIEW-4 — the
  SAME store the CLI writes);
- the NEW pure `viewer-domain` `render_claims_page` (maud HTML, ADR-029);
- the verbatim-confidence rendering (FR-VIEW-8 — `0.90` shown as the stored f64).

It is the thinnest thread because it touches exactly ONE row, ONE route, ZERO
network (offline by construction, I-VIEW-6), and ZERO writes — yet proves the
whole composition root wires together. Every other store scenario (detail,
peer-claims, pagination, the gold invariants) builds on the SAME driving adapter +
read path this skeleton stands up. The `/scrape` live view (US-VIEW-005) is the
ONE additional thread (it adds the reused GitHub boundary); it deliberately is NOT
the walking skeleton because it depends on the network and is the lowest-priority
(Could / P4) story.

## Seeding: production composition root (Pillar 3)

V-1 seeds its claim through the PRODUCTION write path — the real `openlore claim
add` verb (`seed_own_claim_with_evidence`), not a hand-inserted SQL row. The row
the viewer renders is produced by production code into the SAME `OPENLORE_HOME`-
resolved DuckDB the viewer then opens read-only. This makes the skeleton a genuine
end-to-end proof: the writer (CLI) and the reader (`openlore ui`) agree on the
same store, the same schema, the same columns (BR-VIEW-4).

## Spawned-server readiness + teardown approach

The `ViewerServer` harness helper (`tests/acceptance/support/mod.rs`) models the
long-running viewer exactly the way slice-05's `spawn_indexer_serve` models the
long-running `openlore-indexer serve`:

- **Spawn**: `openlore ui --port 0` via `assert_cmd::cargo_bin("openlore")` with
  `env_clear()` + the established clean seams (`OPENLORE_HOME` so the viewer opens
  the env's REAL store; `OPENLORE_DID` / `OPENLORE_KEY_SEED_HEX` /
  `OPENLORE_PDS_ENDPOINT`; and, only for `/scrape` scenarios,
  `OPENLORE_GITHUB_API_BASE` pointing at the reused `FakeGithub`). Port `0` =
  ephemeral, parallel-safe (each scenario gets a disjoint bound port).
- **Readiness**: read the bound `127.0.0.1:<port>` back off the spawned process's
  stdout `viewer.serve.listening` event (mirrors `indexer.serve.listening`), then
  poll a TCP connect until the listener accepts (timeout ~5 s). `base_url()` is
  then `http://127.0.0.1:<port>`.
- **Drive**: `get(path)` / `post_form(path, fields)` issue in-test HTTP via a
  `reqwest` blocking client and return `ViewerResponse { status, body }` (the
  rendered HTML). Scenarios assert on OBSERVABLE rendered text only.
- **Teardown**: `impl Drop for ViewerServer` kills the child (`child.kill()` +
  `wait()`), releasing the bound port — RAII per-scenario isolation, identical to
  `IndexerHandle` / `FakePds` / `PeerPds`. The reused `GithubServer` (when wired)
  is held inside the handle so the `/scrape` seam stays reachable for the viewer's
  lifetime and is released on drop.

## Build-before-run (DELIVER roadmap requirement)

`cargo test` does NOT rebuild the spawned `openlore` binary. The DELIVER
roadmap/run MUST `cargo build` the `openlore` bin before running these ATs so
`ViewerServer` spawns the CURRENT `openlore ui` — the same constraint the slice-05
indexer ATs carry. Note this in the roadmap's "run viewer ATs" step.

## SCAFFOLD state at hand-off

`ViewerServer::start*`, `get`, `post_form`, and the seeding/snapshot helpers are
`todo!()`-bodied (`// SCAFFOLD: true (slice-06)`). The helper COMPILES now (the
binary is resolved at runtime), so V-1 — and the whole corpus — fails at RUNTIME
with `not yet implemented: DELIVER (slice-06): ...` = correct RED. DELIVER's first
step materializes the `ViewerServer` body + `seed_own_claim_with_evidence`, then
flips V-1 green by implementing the `ui` verb + `StoreReadPort` + `render_claims_page`.
