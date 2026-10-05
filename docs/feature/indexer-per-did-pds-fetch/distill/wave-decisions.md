# Wave Decisions — indexer-per-did-pds-fetch (DISTILL)

- **Wave**: DISTILL · **Date**: 2026-10-05 · **Designer**: Quinn (nw-acceptance-designer), autonomous
- **Crafter target (DELIVER)**: `@nw-functional-software-crafter` (ADR-007)
- `[lang-mode] rust` · `[policy-mode] inherit` (`docs/architecture/atdd-infrastructure-policy.md`, 1 row appended) · `[port-mode] inherit` (`tests/common/state_delta.rs`)

## Inputs read

| Input | Status |
|---|---|
| `wizard-decisions.md` | ✓ |
| `discuss/` acceptance-criteria, user-stories, requirements, journey `.feature` + `.yaml`, story-map, wave-decisions | ✓ |
| `design/` architecture-design, component-boundaries, data-models, technology-stack, wave-decisions (incl. AC-002.8, 002.9, 003.4, 004.5 and the DISTILL notes) | ✓ |
| ADR-077, ADR-078, ADR-079 (+ ADR-024 / ADR-071 amendment notes) | ✓ |
| `devops/` | **missing**: warning only. The default environment matrix applies. CI notes are below (DWD-9). |
| House style: `bluesky-claim-review-app/distill/*`, `tests/acceptance/README.md`, `indexer_ingest.rs`, `review_app_self_attested_readers.rs`, `support/mod.rs`, `test-support/src/fake_atproto.rs` | ✓ |

## Reconciliation (HARD GATE)

**Reconciliation passed: 0 contradictions.** I checked every DISCUSS decision (WD-IPF-1..8,
I-IPF-1..6) against DESIGN (DD-IPF-1..12, ADR-077..079). There is no DEVOPS wave. Three
near-misses were examined, and none is a contradiction:

- **Exit codes.** WD-IPF-3 and AC-002.1 say "the pass exits 0". DD-IPF-6 adds exit 3 for a
  *total* outage. That is a user-approved refinement (2026-10-05), and a partial skip still exits
  0. The ATs assert both: 0 for a partial outage (IPF-10) and 3 for a total one (IPF-19).
- **`source_fallback` vs `source_skipped{fallback_used:true}`.** DESIGN resolved this explicitly
  (design/wave-decisions §"Resolved DISCUSS inconsistency"). The ATs follow DESIGN: a fallback
  *read* emits `source_fallback` (IPF-04), and only a fallback *failure* gives
  `source_skipped{fallback_used:true, fallback_failure}` (IPF-07, IPF-08).
- **"Fallback URL equals a resolved PDS"** (I-IPF-2) vs DD-IPF-2 (`Fallback` is always `Relay`,
  with no URL comparison). These are consistent. IPF-06 pins it adversarially.

## Decisions

| ID | Decision | Rationale |
|---|---|---|
| DWD-1 | Rust std `#[test]` suites in flat `tests/acceptance/*.rs`, with Gherkin in doc comments (house convention). 5 cli `[[test]]` targets (`indexer_per_did_fetch`, `indexer_per_did_resilience`, `indexer_per_did_config`, `search_self_attested_label`, `indexer_pass_core`) plus 1 `xtask/tests` file. | Same as every prior feature. Targets run with `cargo test -p cli --test <name>`. |
| DWD-2 | **Every new scenario is `#[ignore = "DELIVER <step>: …"]` at hand-off**, including both walking skeletons. `main` stays GREEN. DELIVER removes one ignore at a time, in the step order given in `test-scenarios.md`. RED was verified locally before the ignores were left in place (`red-classification.md`). | User instruction (lesson from the previous feature). |
| DWD-3 | **One multi-PDS fake**: `FakeAtprotoNetwork` is extended rather than a new double being written. It already hosts a PLC directory plus one loopback server per PDS host. New indexer postures: `ListingPosture` per host (`Serve`, `Status(code)`, `NotJson`, `RedirectTo`, `Slow`, `Hang`), `DidDocPosture` per DID (`NotFound`, `ServerError`, `Hang`, `IdMismatch`, `NoPdsService`, `PdsEndpoint(url)`), `move_account`, `serve_repo_as` (the foreign-repo lie), `start_with_extra_hosts`, an ordered `RequestSeen` log, and the network-wide peak `max_requests_in_flight`. There are 3 new self-tests, and all 10 `fake_atproto` self-tests are green. | Covers the full lies catalogue of architecture-design §9 with one double, and reuses the review-app fake that already models bsky-style DID documents. |
| DWD-4 | **Harness**: `tests/acceptance/support/indexer_network.rs`. It holds the typed vocabulary (`Author`, `Host`, `Claim`, `SkipReason`, `Provenance`, `RefusalReason`, `var::*`), one `IndexerWorld` (its methods are the Given/When steps), and a `PassReport` (its readers are the Then observables, e.g. `assert_pass_completed(code, own, fallback, skipped)` and `assert_skipped(did, reason)`, which also enforces AC-002.3's no-claim-content rule on every skip event). It also has a `Tripwire` (a loopback port that counts connections, which proves a refused address is never contacted) and a `CannedIndexer` (an old or misbehaving search server, for ADR-079 backward compatibility). Every run is bounded at 90 s, so a missing deadline fails rather than hangs. | Mandate-12: types + one world, and step bodies delegate. |
| DWD-5 | **Loopback seam**: every indexer run in the new suites sets `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1` unless the scenario is about its absence (IPF-08, IPF-29). The 5 existing indexer spawn sites (`support/mod.rs` ×4 and `review_app_self_attested_readers.rs`) now also set it. The variable is inert until DELIVER, so the existing suites are unchanged (re-run green; see `red-classification.md`). | DESIGN test-infra note. This prevents the parse_config step from breaking every existing indexer suite. |
| DWD-6 | **Layering**: subprocess scenarios are layer 3 and example-only. Failure modes are named examples (tables iterate named cases), per Mandate 11. Pure-core contracts are layer-2 proptest in `indexer_pass_core.rs`, with RED `sut_*` bindings (`todo!()`), as in the `review_app_core.rs` precedent. Closed-world tables (8 fetch-failure classes, 3 provenance tokens, the range edges) are exhaustive examples, not PBT (falsifier gate). | Mandate 9. |
| DWD-7 | **Tier B: not emitted.** The journey has chained passes, but the input space at the subprocess layer is a small closed set of postures. The domain-rich parts (URLs, IPs, DID lists, env maps, outcome vectors) are already generative at layer 2 (CORE-1..11). | Mandate 10 conditions not both met. |
| DWD-8 | **CI-only release guard (AC-004.5)**: IPF-30 runs the *release* binary. It carries a different ignore reason (`CI release-guard …`) and is run with `-- --ignored` after `cargo build --release -p openlore-indexer`, taking the binary from `OPENLORE_INDEXER_RELEASE_BIN` or `target/release/`. The always-on equivalent is CORE-8 (`parse_config(Release, seam=1) == Err`), the review-app pattern. **Decision needed (DEVOPS/user):** whether to add that CI step. If not, CORE-8 alone guards AC-004.5. | A release build inside `cargo test` would be very slow (bundled DuckDB). |
| DWD-9 | **DEVOPS gap (warning)**: there is no DEVOPS wave. The environment matrix is the project default. The ATs need no new CI service: everything is loopback and hermetic. Two optional CI items are a release-guard step (DWD-8) and making sure the `openlore-indexer` binary is built before the cli acceptance stage. It already is: it is a cli dev-dependency. | Graceful degradation. |

## Walking-skeleton strategy

Architecture of Reference (legacy label **B: real local, fake external**). The driving ports
(`openlore-indexer ingest`/`serve` and `openlore search`) and the driven-internal port
(`index.duckdb`) are real. The driven-external ports (PLC directory, author PDSes, fallback) are
faked by `FakeAtprotoNetwork`. WS-1 and WS-2 are tagged
`@walking_skeleton @driving_port @real-io`, and WS-1 also has `@driving_adapter`.

## What the doubles cannot model (earned-trust gaps, covered elsewhere)

| Gap | Where it is proven instead |
|---|---|
| Positive `did:web` resolution: the resolver fetches `https://<host>/.well-known/did.json` on port 443 with real TLS | DELIVER adapter tests: `pds_endpoint_of` property + fake-PLC/`did:web` integration (component-boundaries). The AT side covers "did:web accepted at config + unresolvable → fallback" (IPF-09) and the config refusal of other methods (IPF-21). |
| DNS answers that map a *public-looking name* to a private address (rebinding) | DELIVER adapter guard test with a stub resolver (10.0.0.1, `::1`, 169.254.169.254), per component-boundaries. The AT side uses `localhost`, which resolves to loopback only, under the production policy (IPF-08). |
| Index-store upsert failure (exit 2, earlier rows stay committed) | No hermetic way to make the real store fail mid-pass from a subprocess. DELIVER unit test on the gate loop. |
| The real bsky.social `listRecords` and PLC document shapes | DEVOPS contract tests (recommended in DESIGN §10). |

## Upstream issues (back-propagation)

1. **`--contributor` for a self-attested author (DESIGN, AC-005.3).** `openlore search
   --contributor did:plc:…` lifts a bare DID to the app identity (`…#org.openlore.application`,
   `crates/cli/src/verbs/search.rs` `resolve_contributor`) before the wire query. Self-attested
   rows are stored under the **bare** repo DID (`ingest.rs` `self_attested_indexed_claim`), so a
   contributor search for Priya would return nothing. The claim in DESIGN §6 that the result is
   "consistent across dimensions because all project through `flat_attributed_rows`" holds for
   rendering, but not for contributor *matching*. IPF-32 asserts the user-visible contract (Priya
   is found by contributor and labelled). DELIVER has to choose the mechanism: the store matches
   both forms, or the CLI does not lift when the indexer reports self-attested authors.
   **Decision needed** (Morgan/user). Until then this is in DELIVER scope as written.
2. **Existing-suite harness migration (DELIVER step 00, before parse_config lands).**
   `FakeIngestServer` builds `at://` URIs and `OPENLORE_INDEXER_REPO_DIDS` from the *app-signed
   author* (`did:plc:priya-test#org.openlore.application`). The new DID syntax rule (no `#`)
   will refuse those at startup, and the old suites also leave `OPENLORE_INDEXER_PLC_ENDPOINT`
   unset (the default is the live `plc.directory`). The migration is to (a) derive bare repo
   DIDs and URIs in `FakeIngestServer`, and (b) point the existing spawners at a hermetic
   directory that 404s. That routes every existing DID through the fallback, which reproduces
   today's behaviour exactly (design/wave-decisions §Regression). After this the existing
   `indexer_ingest`, `appview_search`, `search_hide_retracted`, `viewer_*` and
   `review_app_self_attested_readers` suites **are** the NFR-3 regression guard. RD-7 (relay
   refusal) then expects `source_url` to be the fallback, so it has to become "Priya
   unresolvable → read via the fallback = relay → refused".
3. **Stdout noise.** `pass_summary` must be the last event before exit (IPF-19 asserts this
   ordering, AC-002.8). The existing per-record `eprintln!` refusal line goes to stderr, so it
   does not conflict.

## Step-reuse ratio (Mandate-12 criterion 4, informational)

There are 37 subprocess scenarios over one `IndexerWorld`/`PassReport` vocabulary. Measured
invocations: 224 calls to 33 distinct step methods in scenario bodies, so the reuse ratio is
**about 6.8×**. That is high because every scenario reuses `configured_with`, `publishes_*`,
`one_ingest_pass_runs` and `assert_pass_completed`. Criteria 1–3: the typed vocabulary lives in `indexer_network.rs`
(Rust has no separate `domain_types` file; the enums sit at the top of the harness). Step
methods take typed `Author`/`Host`/`SkipReason`/`Provenance` and never a raw string where an
enum exists. The only raw-string steps are the deliberately malformed operator inputs
(`fallback_text_is`, `repo_dids_text_is`). Step bodies delegate to the fake or the binary, with
no business logic.
