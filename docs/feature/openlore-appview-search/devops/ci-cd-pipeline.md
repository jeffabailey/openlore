# CI/CD Pipeline Delta — openlore-appview-search (slice-05)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex
- **Tool**: GitHub Actions (UNCHANGED from D-D1)
- **Branching**: GitHub Flow (UNCHANGED from D-D7)

This is the slice-05 **delta** to `ci-cd-pipeline.md` (foundation + slice-02/03/04).
Read those first. This document describes only the additions and the single-line
modifications. No YAML is written here. DELIVER lands the YAML into the EXISTING
`ci.yml`, `nightly.yml`, and `release.yml` workflow files — no new workflow file is
created (Simplest-Solution Alternative 3, rejected in `platform-design.md` §7).

Slice-05 is the FIRST slice to: add a SECOND binary to the build/release matrix,
add TWO external contract sub-jobs, edit `deny.toml` (the `axum` ban), and widen the
mutation `--package` list for the THIRD time. The CLI's existing jobs are all
`--workspace`-scoped, so the new crates + the new binary are picked up by the
EXISTING `fmt`/`clippy`/`test` jobs by construction — the additions are the new
acceptance + contract jobs, the xtask-rule extensions, and the workflow-file edits
for the new crates/binary.

## 1. Workflow files (no new files)

| File | Triggers (UNCHANGED) | Slice-05 additions |
|---|---|---|
| `.github/workflows/ci.yml` | `pull_request`, `push: [main]` | Three new acceptance-stage GUARDRAIL jobs (§3) + the search-scenario ATs + TWO new Pact sub-jobs (§3.5); the new crates picked up by the existing `--workspace` jobs (§2); the extended `check-arch`/`check-probes`/`deny` (§2, §6) |
| `.github/workflows/nightly.yml` | `schedule: cron 08:00 UTC`, `workflow_dispatch` | Mutation `--package` list += `crates/appview-domain` (§4) — the THIRD widening |
| `.github/workflows/release.yml` | `push: tags: ['v*']` | Builds + ships the SECOND binary (`openlore-indexer`) across the 4-platform matrix (§5); re-runs the new acceptance jobs; the two contract suites' real-provider variants (manual approval); release-tag mutation re-run covers `appview-domain` |

NOTE: `release.yml` does not yet exist in the repo (per the slice-01 nightly.yml
header note: "release.yml lands when the first vX.Y.Z tag is cut"). Slice-05 does
NOT create it; it documents the release-matrix delta (§5) for whenever `release.yml`
is authored. The CI + nightly edits ARE applicable now (the new crates/binary build
+ test in `ci.yml`/`nightly.yml`).

## 2. Commit-stage (the new crates + extended rules)

The existing `--workspace`-scoped jobs pick up the new crates by construction —
the FIRST thing DELIVER does is the bootstrap (the new crates + the new binary in
the workspace `Cargo.toml`), after which:

- **`fmt`** (`cargo fmt --all -- --check`): covers the new crates unchanged.
- **`clippy`** (`cargo clippy --workspace --all-targets -- -D warnings`): covers the
  new crates + the `openlore-indexer` binary unchanged. The `--workspace --all-targets`
  scope already includes a new binary crate.
- **`test`** (`cargo nextest run --workspace`): covers the new crates' unit +
  property tests + the new acceptance tests unchanged (they are workspace tests,
  discovered by `nextest`). NOTE: the existing `--test-threads=1` workaround (the
  `OPENLORE_TEST_NOW` env-var race) carries forward; the new localhost-transport
  ATs (a fixture indexer on an ephemeral port) must use a per-test ephemeral port to
  avoid a NEW parallel-bind race — DELIVER allocates the port per-test (a
  bind-to-:0-then-read-back pattern), NOT a fixed port, so they remain parallel-safe.
- **`deny`** (`cargo deny check`): see §6 — the `deny.toml` `axum` ban is narrowed
  (the FIRST `deny.toml` change since slice-01); `bs58` is already license-allowlisted.
- **`check-arch`** (`cargo test -p xtask && cargo run -p xtask -- check-arch`): runs
  unchanged in COMMAND, EXPANDED in rule set (§2.1). The `cargo test -p xtask` step
  (which pins the rule classifiers' positive/negative behavior, per the slice-01
  step-06 CI header) gains tests for the three NEW rules.
- **`check-probes`** (`cargo run -p xtask -- check-probes`): runs unchanged; the AST
  walker picks up the FOUR new adapters' `impl <Port> for <Adapter>` probe bodies by
  construction (the new trait set `IngestSourcePort`/`IndexStorePort`/`IdentityResolvePort`/
  `IndexQueryPort` + the `adapter-xrpc-query-server` bind/serve probe). `appview-domain`
  has no `probe()` (pure crate) — `check-probes` correctly does not require one.

### 2.1 `check-arch` rule extensions (THREE new rules + one extended scope + one allowlist entry)

DELIVER lands the rule code in the `xtask` crate; the CI job invocation is unchanged.
Per `design/component-boundaries.md` §`xtask` + the slice-05 invariant table:

1. **EXTEND `no_cross_table_join_elides_author`** (slice-03/04 rule) to cover the
   `adapter-index-store` SQL string literals: any literal aggregating over
   `indexed_claims` (GROUP BY / COUNT / SUM across authors) without projecting
   `author_did` fails CI. (I-AV-2 structural layer; the cardinal anti-merging-at-network-
   scale enforcement.)
2. **ADD `indexer_holds_no_signing_or_local_store`**: the `openlore-indexer` crate's
   dependency graph MUST NOT include the signing `IdentityPort` impl, the user's
   `StoragePort`/`adapter-duckdb`, or any PDS-write surface — AND (the slice-05
   addition, per `platform-design.md` §10) the CLI crate MUST NOT link
   `adapter-xrpc-query-server` (the CLI must not link an HTTP server; this is the
   STRUCTURAL replacement for the now-narrowed `deny.toml` `axum` ban). (I-AV-5 +
   I-AV-3 structural layer; mirrors the slice-02 I-SCR-1 rule for `adapter-github`.)
3. **ADD `no_pubkey_seam_in_release_build`**: a release binary that reads
   `OPENLORE_PEER_PUBKEY_HEX_<did>` fails the check (production uses the real ADR-026
   decode). (I-AV-6 structural layer; mirrors the slice-03 `no_autoconfirm_in_release_build`.)
4. **ADD `crates/appview-domain` to the pure-core allowlist** (alongside `claim-domain`,
   `lexicon`, `ports`, `scraper-domain`, `scoring`): enforce it imports NO I/O crate
   (no `duckdb`/`tokio`/`reqwest`/`std::fs`/`std::net`/`std::time::SystemTime` or any
   `adapter-*` crate; I-1/I-2). The `claim-domain` decode helper stays inside the
   already-allowlisted `claim-domain` (the base58 dep, if used, is whitelisted in the
   pure-core allowlist like slice-03's `unicode-normalization`).
5. **EXTEND I-3 (composition-root rule) to cover BOTH binaries**: `cli` is the only
   root wiring the USER's adapters; `openlore-indexer` is the only root wiring the
   indexer's adapters; neither wires the other's.

These run in the EXISTING `check-arch` stage (same command, expanded rule set). Per
the slice-01 step-06 CI header, the `cargo test -p xtask` step pins each rule's
classifier behavior BEFORE the check runs — DELIVER adds positive/negative tests for
rules 1-3 (the anti-merging-on-index-store classifier, the capability-boundary
classifier, the pubkey-seam classifier) mirroring the slice-03 `anti_merging` +
`autoconfirm_guard` rule-test pattern.

## 3. Acceptance-stage additions

All new jobs run in parallel within the existing acceptance stage, after the
commit-stage gates pass. Each is **blocking on PR** and **gates release**. The first
three are the cardinal release-blocking GUARDRAILS (the KPI-AV disprovers); the next
are the search-scenario functional ATs; the last two are the contract sub-jobs.

### 3.1 `at-indexer-rejects-unverified-claim` (GUARDRAIL — KPI-AV-3) — D-D35

- **Command**: `cargo nextest run --test indexer_rejects_unverified_claim`
- **What it does**: a `FakeIngestSource` (hermetic) serves adversarial records —
  tampered-signature, CID-mismatch, unsigned, and a `did_unresolvable` (a PLC doc
  that cannot be decoded) — interleaved with legitimately-signed records; runs an
  `openlore-indexer ingest` one-shot pass; asserts NONE of the adversarial records
  enter `indexed_claims` (no row; `verified_against` is `NOT NULL` for every row that
  DID enter); asserts the legitimately-signed records DID enter (a false-positive
  reject of a good claim is ALSO a failure — KPI-AV-3 cuts both ways); asserts a
  subsequent `search` over the corpus NEVER returns a rejected record; asserts the
  ingest gate reused `claim_domain::verify` (the per-record `indexer.ingest.rejected{reason}`
  events are emitted with the right reason). Uses the REAL `decode_ed25519_multibase`
  (the real-`z6Mk...` fixture DID-doc), NOT the test seam (a seam-only pass is a
  failure; the `no_pubkey_seam_in_release_build` rule backs it structurally).
- **Maps to**: KPI-AV-3 (verified-before-index = 100%); WD-104/121; I-AV-1; extends
  slice-03 `at-peer-tampered-signature-rejected`
- **Type**: blocking GUARDRAIL (release-blocking; a cardinal disprover)
- **Wall-clock target**: < 30 s

### 3.2 `at-network-result-preserves-attribution` (GUARDRAIL — KPI-AV-2) — D-D35

- **Command**: `cargo nextest run --test network_result_preserves_attribution`
- **What it does**: seeds the index with TWO records by DISTINCT authors on the SAME
  `(subject, object)`; runs `search --object <object>`; asserts the result has TWO
  attributed `NetworkResultRow`s with TWO distinct non-empty `author_did`s grouped
  under their respective authors (`by_author` has both); asserts `distinct_author_count == 2`;
  asserts NO row/struct/table represents both claims combined (no consensus row); asserts
  the index store has NO `consensus`/`merged` table; asserts a `--share` link re-resolves
  to the per-author result (never a merged snapshot, I-AV-8 cross-check); cross-checks the
  wire response (the B1 contract) carries `author_did` on every element. Asserts the
  aggregation (`distinct_author_count`) was computed in the PURE `appview-domain` core,
  NOT a SQL `GROUP BY` (the `no_cross_table_join_elides_author` rule on the index-store
  SQL backs it structurally).
- **Maps to**: KPI-AV-2 (anti-merging at network scale = 100%); WD-103/120; I-AV-2;
  extends slice-03 KPI-FED-1 + slice-04 KPI-GRAPH-2
- **Type**: blocking GUARDRAIL (release-blocking; a cardinal disprover)
- **Wall-clock target**: < 30 s
- **Backed by**: the runtime counter `indexer_query_attribution_missing_total` (target
  0 forever) + the `contract-pact-indexer-query` wire pin (the anti-merging-across-
  the-transport check).

### 3.3 `at-local-first-preserved` (GUARDRAIL — KPI-5) — D-D35

- **Command**: `cargo nextest run --test local_first_preserved` (the existing
  `kpi-5-offline` integration test EXTENDED for slice-05)
- **What it does**: with the indexer DOWN (no `openlore-indexer serve` running) AND
  network disabled (the existing `unshare -n` step, extended), asserts `claim add` /
  offline `claim publish` / `graph query` (the slice-01/03/04 local flows) ALL
  succeed; asserts `openlore search --object ...` prints the clear local-only message
  (pointing to `graph query`) and exits NON-FATALLY (exit 0, no hang, no panic);
  asserts the CLI STARTED without a reachable indexer (the indexer is not probed at
  CLI startup — the `IndexQueryPort` probe is soft); asserts no network call was made
  by the local flows (reuses the foundation `--features network-audit`
  `network_calls_total_debug == 0` assertion from ADR-010).
- **Maps to**: KPI-5 (local-first preserved); WD-106; I-AV-3; the cardinal
  local-first↔network-service tension
- **Type**: blocking GUARDRAIL (release-blocking; a cardinal disprover)
- **Wall-clock target**: < 30 s

### 3.4 Search-scenario functional ATs (US-AV-002..006) — D-D35

Blocking on PR; gate release. These cover the search surface beyond the three
guardrails. (DISTILL builds these against the resolved `# confirm` flags; DEVOPS
fixes the event shapes + the hermetic fixtures they consume.)

- **`at-search-by-object`** — `search --object <philosophy>`; asserts results group
  by author, include unfollowed authors labeled `(not subscribed)`, each row carries
  `[verified]` + numeric confidence + cid + author_did; the no-merge footer + the
  `peer add` pointer render. < 20 s.
- **`at-search-by-contributor-or-subject`** — `search --contributor <did>` /
  `--subject <project>`; asserts the per-author trail + the "one developer's reasoning
  trail, not a community consensus" footer. < 20 s.
- **`at-search-show-trust`** — `search --object ... --show <cid>`; asserts the full
  record + "Signature: VERIFIED against <did>" + "CID recomputed, matches published
  record" (the SAME pure-core verification result; no second path); asserts
  `--show <cid not in result>` is a usage error (non-zero exit, distinct from an empty
  search exit 0). < 20 s.
- **`at-public-data-banner-shown`** — asserts the up-front public-data banner prints
  before results (KPI-AV-5; WD-105). < 15 s.
- **`at-discovery-follow-reuses-slice03-path`** — asserts the follow affordance prints
  the slice-03 `openlore peer add <did>` command (render-only; no auto-subscribe; no
  parallel state); after `peer add` + `peer pull` the author's claims appear in local
  `graph query` (the funnel closes; KPI-AV-4; I-AV-7). < 30 s.
- **`at-share-link-encodes-query-not-snapshot`** — asserts `--share` emits a stable
  query-encoding link; opening it re-runs the query → current per-author-attributed
  verified results, never a stored merged snapshot (KPI-AV-6; I-AV-8). < 20 s.
- **`at-countered-claim-still-appears`** — asserts a countered/retracted public
  verified claim is still discoverable; the counter relationship is annotated when
  known, never silently filtered (OD-AV-7; I-AV-9). < 20 s.

### 3.5 Contract sub-jobs (TWO new; mirror slice-03 `contract-pact-pds-peer`) — D-D36/D-D37

- **`contract-pact-indexer-query`** (B1, CLI↔indexer): `cargo nextest run --test
  pact_indexer_query`. Consumer-driven contract; the response carries per-result
  `author_did` (the anti-merging-across-the-transport pin). PR/nightly: MOCKED +
  in-process provider verify. Release: re-verify vs a real localhost `openlore-indexer
  serve` (no third party). < 30 s. (See `contract-test-ownership.md` §2.)
- **`contract-pact-pds-network`** (B2, indexer→PDS/PLC): `cargo nextest run --test
  pact_pds_network`. EXTENDS the slice-03 `listRecords` Pact to network authors + ADDS
  the PLC DID-document contract (the `z6Mk...` `publicKeyMultibase` shape the ADR-026
  decode reads). PR/nightly: RECORDED fixtures. Release: re-verify vs real `bsky.social`
  + real `plc.directory` (manual approval, D-D12; the NEW `plc.directory` allowlist
  host, D-D39). < 30 s. (See `contract-test-ownership.md` §3.)

### 3.6 Acceptance-stage summary (delta)

| Stage | Wall-clock target | Type | Conditional? |
|---|---|---|---|
| at-indexer-rejects-unverified-claim | < 30 s | blocking GUARDRAIL (release-blocking) | no |
| at-network-result-preserves-attribution | < 30 s | blocking GUARDRAIL (release-blocking) | no |
| at-local-first-preserved | < 30 s | blocking GUARDRAIL (release-blocking) | no |
| at-search-by-object | < 20 s | blocking | no |
| at-search-by-contributor-or-subject | < 20 s | blocking | no |
| at-search-show-trust | < 20 s | blocking | no |
| at-public-data-banner-shown | < 15 s | blocking | no |
| at-discovery-follow-reuses-slice03-path | < 30 s | blocking | no |
| at-share-link-encodes-query-not-snapshot | < 20 s | blocking | no |
| at-countered-claim-still-appears | < 20 s | blocking | no |
| contract-pact-indexer-query (mocked/in-process) | < 30 s | blocking | no |
| contract-pact-pds-network (recorded fixtures) | < 30 s | blocking | no |
| contract-pact-indexer-query (real localhost) | ~30 s | manual approval at release | release-tag only |
| contract-pact-pds-network (real bsky + plc.directory) | ~2-3 min | manual approval at release | release-tag only |

Aggregate added wall-clock: **< 5 min per PR** (jobs parallelize within the
acceptance stage; all PR-stage jobs are hermetic — no real network). Release-tag
overhead: **~3-4 min** (the two real-provider contract variants under the existing
manual-approval gate + the new `plc.directory` host). Foundation's target (< 30 min
acceptance) is comfortably preserved.

## 4. Mutation testing (delta) — `crates/appview-domain` added to scope (THIRD widening)

Per Apex Core Principle 9 + D-D8 (nightly-only, pure-core) + the D-D23/D-D31 precedents:

- **`crates/appview-domain` is added to the `--package` list** of the nightly
  `cargo mutants` invocation. This is the THIRD mutation-scope widening (after
  slice-02's `scraper-domain` D-D23 + slice-04's `scoring` D-D31).
- **Kill-rate target: ≥95%** (matches `claim-domain` + `scraper-domain` + `scoring`
  per ADR-006 Earned Trust). The mutation surface: `ingest_decision` (the
  verify-before-index gate — a surviving mutant means a tampered record could slip
  past without test failure, the KPI-AV-3 disprover), `compose_results` (the
  anti-merging composition — a surviving mutant means an author could be merged away
  or the `distinct_author_count` miscomputed, the KPI-AV-2 disprover),
  `near_match_suggestion`, `annotate_counter_relationship`.
- **The `claim-domain` decode helper** (`decode_ed25519_multibase`, ADR-026) is
  mutated as part of the EXISTING `claim-domain` mutation scope (no new `--package`
  entry — it lives in the already-mutated `claim-domain`). Mutation hardens the
  `decode∘encode == identity` + malformed-input-errors tests (I-AV-6).
- The four new EFFECT crates are NOT mutated (effect shell; covered by the extended
  probes + the acceptance/contract tests; D-D8 pure-core-only policy).
- Release-tag mutation re-run inherits the D-D8 blocking-on-regression gate;
  `appview-domain` is now in scope.
- DELIVER updates the nightly workflow's `--package` list (a one-line edit:
  `cargo mutants -p claim-domain -p scraper-domain -p scoring -p appview-domain ...`).
  No new gate semantics; just a wider scope. The `CLAUDE.md` Mutation Testing Strategy
  section is UNCHANGED in POLICY (nightly-only per D-D8); only the `--package` list
  grows. Mirrors D-D31 exactly.

## 5. Release workflow (delta) — the SECOND binary in the matrix (D-D35)

Per `ci-cd-pipeline.md` (foundation) §7 + ADR-011 §Release matrix. Slice-05 inserts:

- 5.1 **The `openlore-indexer` binary is built across the SAME 4-platform matrix as
  the CLI** (`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`; ADR-011). Each platform release now ships TWO binaries.
  Same native-build-per-target (no cross-compile). The `build-release` matrix gains the
  second binary as an artifact per cell.
- 5.2 **Release artifact security covers BOTH binaries** (D-D11): each binary gets its
  `.sha256` + `.sig` (cosign); the release-wide `sbom.cdx.json` now covers BOTH
  dependency trees (including the new `axum` + `bs58` deps); the `provenance.intoto.jsonl`
  covers the two-binary build. SLSA L2-minimum/L3-target unchanged.
- 5.3 **`cargo install openlore-indexer`** publishes the indexer crate to crates.io
  alongside `cli` (ADR-011 §Channels; the other workspace crates keep `publish = false`).
- 5.4 The new acceptance-stage jobs (§3.1-3.4) are re-run on the tagged ref.
- 5.5 The two contract suites' real-provider variants run once at release under the
  existing manual-approval gate (§3.5; the new `plc.directory` allowlist host, D-D39).
- 5.6 The release-tag mutation re-run now covers `appview-domain` (§4).
- 5.7 Substrate matrix: the CLI cells gain a `search` happy-path + degradation case
  against a localhost fixture indexer; the indexer-store fsync-honesty probe runs per
  cell (the tmpfs/overlayfs nightly cells exercise the container-substrate-durability
  lie; `platform-design.md` §6).
- 5.8 Adversarial renderer review (the slice-03 D-D19 / slice-02 D-D28 / slice-04 D-D33
  checklist) gains one slice-05 line: "the network search/share renderer never collapses
  authors into a consensus row, always renders `[verified]` + the relationship label +
  the public-data banner, and `--share` encodes the query not a snapshot" (D-D41).
  Recorded in the release CHANGELOG.

Windows stays deferred for BOTH binaries (ADR-011; the "slice-05 AppView introduces a
need we can't avoid" revisit trigger is evaluated NO — the indexer is a Linux/macOS
self-hosted service). Estimated release wall-clock (delta): **+4 to +6 min** (the
second binary's build across 4 platforms + the two real-provider contract variants +
the wider mutation re-run). Acceptable.

## 6. `deny.toml` change (the FIRST since slice-01) — narrow the `axum` ban (D-D42)

This is the FIRST `deny.toml` change in the project (slices 02/03/04 all added zero
deps / made no `deny.toml` change). The slice-01 `[bans]` DENIES `axum` with the
rationale "OpenLore is a CLI; we never run an HTTP server in-process" — slice-05-obsolete
because the `openlore-indexer` IS a network service serving HTTP (ADR-027).

- **DELIVER edits `deny.toml` `[bans]`**: remove `axum` from the `deny` list (IF
  DELIVER chooses `axum` over a hand-rolled `hyper` handler, Q-DELIVER-AV-2). Rely on
  the STRUCTURAL `xtask check-arch` rule `indexer_holds_no_signing_or_local_store`
  (extended per §2.1 rule 2 to assert the CLI links no HTTP server) to enforce "the CLI
  links no HTTP server" — a stronger type-and-arch guarantee than a license-tool ban
  (the ban was always belt-and-suspenders for a property now enforced structurally).
- **`actix-web` stays BANNED** (rejected in `technology-stack.md`; carries forward).
- **`bs58` (MIT) needs NO license addition** — the `[licenses]` allow list already
  contains `MIT`; `axum` + transitive `tower`/`http`/`hyper` are all MIT/Apache-2.0,
  already allowlisted.
- **`cargo deny check`** runs on every commit (I-11) and verifies the narrowed ban +
  the new deps' licenses/advisories/sources. The edit's rationale is recorded inline
  in `deny.toml` (the ADR-012-amendment discipline; not a silent unban).
- **If DELIVER picks the hand-rolled `hyper` handler** (Q-DELIVER-AV-2) and inlines
  base58btc (Q-DELIVER-AV-8): NO `deny.toml` edit is needed (`hyper` is already a
  transitive workspace dep via `reqwest`, not banned; no new external crate). The
  `deny.toml` edit is REQUIRED only on the `axum` and/or `bs58` paths.

See `platform-design.md` §10 + the Upstream Issue.

## 7. Adversarial-fixture maintenance helper (proposed for DELIVER; reuses slice-03 D-D15)

To prevent the ingest adversarial fixtures + the real-`z6Mk...` DID-doc fixture from
drifting away from the live `org.openlore.claim` Lexicon + the decode contract:

- **`cargo xtask regenerate-ingest-fixtures`** (extends the slice-03
  `regenerate-peer-fixtures`): reads `lexicons/org/openlore/claim.json`; generates the
  tampered-signature / CID-mismatch / unsigned ingest fixtures + a real-`z6Mk...` DID-doc
  fixture (the known test keypair); writes them to `tests/fixtures/ingest-adversarial/`
  + the recorded PLC DID-doc to `tests/contracts/pact/`; updates the `FakeIngestSource`
  setup.
- **CI check**: a `--check` run in the existing `arch-check` stage fails if the committed
  fixtures drift (the slice-03 D-D15 pattern; no new top-level job).
- **DELIVER scope**: may defer the regenerator (the fixtures work without it; it just
  risks drift — the slice-03 D-D15 escape hatch).

## 8. Quality-gate enforcement summary (delta rows only)

Insert these rows into the foundation table at §9:

| Gate | Pre-PR (local) | PR | Nightly | Release-tag |
|---|---|---|---|---|
| at-indexer-rejects-unverified-claim | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-network-result-preserves-attribution | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-local-first-preserved | – | ✓ GUARDRAIL | – | ✓ GUARDRAIL |
| at-search-* (object/contributor-subject/show/banner/follow/share/countered) | – | ✓ blocking | – | ✓ blocking |
| contract-pact-indexer-query (mocked) | – | ✓ blocking | – | ✓ blocking |
| contract-pact-pds-network (recorded) | – | ✓ blocking | – | ✓ blocking |
| contract-pact-indexer-query (real localhost) | – | – | – | ✓ manual approval |
| contract-pact-pds-network (real bsky + plc.directory) | – | – | – | ✓ manual approval |
| mutation testing (`crates/appview-domain`) | – | – | ✓ advisory | ✓ blocking on regression |
| arch-check `no_cross_table_join_elides_author` (extended to index-store SQL) | (lint subset, pre-push) | ✓ blocking | – | ✓ blocking |
| arch-check `indexer_holds_no_signing_or_local_store` (+ CLI-no-HTTP-server) | (lint subset, pre-push) | ✓ blocking | – | ✓ blocking |
| arch-check `no_pubkey_seam_in_release_build` | (lint subset, pre-push) | ✓ blocking | – | ✓ blocking |
| arch-check pure-core allowlist (`appview-domain`) | (lint subset, pre-push) | ✓ blocking | – | ✓ blocking |
| deny (narrowed `axum` ban; `bs58` allowlisted) | – | ✓ blocking | – | ✓ blocking |

The "Pre-PR (local)" column is empty for the new acceptance/contract ATs (too slow
for pre-push; foundation pre-push runs only unit + property + arch). The `arch-check`
rule extensions run in the lint/arch subset pre-push already invokes. The pre-commit
and pre-push hook designs from foundation §5 are unchanged in shape; the mirrored
commit-stage set widens with the new crates (the local gate mirrors the remote
commit stage, per the cicd skill).

## 9. Branch protection rules (UNCHANGED)

Foundation §10 rules carry forward. The new acceptance + contract jobs are added to
the "required status checks" list at the same level as the existing acceptance jobs.
The three cardinal GUARDRAIL jobs (KPI-AV-2/3 + KPI-5) are required-checks like any
blocking AT; their release-blocking nature is the DISCUSS disprover policy, enforced
by being required checks.

## 10. References

- `platform-design.md` (sibling) — the deployable, the env matrix, the `deny.toml` change, mutation scope, risks
- `observability.md` (sibling) — the event shapes the new tests + the runtime counters consume
- `kpi-instrumentation.md` (sibling) — the KPI-AV gate mapping
- `contract-test-ownership.md` (sibling) — the two contract sub-jobs' ownership + allowlists
- `wave-decisions.md` (sibling) — D-D35..D-D43
- Foundation `ci-cd-pipeline.md` + slice-02/03/04 `ci-cd-pipeline.md` deltas — the base to extend
- `docs/feature/openlore-appview-search/design/component-boundaries.md` (the xtask rules; the crafter annotation)
- `docs/feature/openlore-appview-search/design/wave-decisions.md` (WD-111..124; the Q-DELIVER-AV set)
- `docs/feature/openlore-appview-search/discuss/outcome-kpis.md` (KPI-AV-1..6 + §Disprovers + §Handoff)
- ADR-011 (the release matrix — gains the indexer artifact), ADR-012 (supply-chain — the `deny.toml` policy)
