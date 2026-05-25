# Wave Decisions — DISTILL — openlore-foundation

- **Wave**: DISTILL
- **Date**: 2026-05-25
- **Acceptance Designer**: Quinn (nw-acceptance-designer)
- **Feature**: openlore-foundation (slice-01 walking skeleton)
- **Inherits**: WD-1..WD-13 (DISCUSS) + D-1..D-12 (DESIGN); all ADRs

This file records DISTILL-wave decisions (DD-prefix). Decisions that point at a
test artifact (a file in `tests/acceptance/`) are binding for DELIVER unless
re-opened.

---

## Wave-Decision Reconciliation result

**Reconciliation passed — 0 contradictions** between DISCUSS WD-1..WD-13 and
DESIGN D-1..D-12. See `acceptance-tests.md` §1 for the full pairwise audit.

---

## Locked decisions

| # | Decision | Rationale | Status |
|---|---|---|---|
| DD-1 | Test framework = Rust std `#[test]` + plain function names that encode the scenario (snake_case, prefixed by file role: `walking_skeleton_*`, `lexicon_*`, `federation_roundtrip_*`). NO `.feature` files for slice-01. | Rust's `cucumber` ecosystem (cucumber-rust) is workable but adds a runtime + a parser dependency for slice-01's 29 scenarios. Plain `#[test]` keeps the test inventory in one place per file, plays nicely with `cargo test --test <file>` filtering, and uses no extra crates. The scenario titles in the test function names are 1:1 with what `.feature` files would render. DELIVER may port to `.feature` in a later refactor without losing semantics. | LOCKED |
| DD-2 | Pre-DELIVER fail-for-right-reason gate is **deferred until `Cargo.toml` exists**. Skeletons compile only after DELIVER scaffolds the workspace. The gate runs as part of DELIVER's first roadmap step (post-`Cargo.toml`-bootstrap): every scenario must classify as RED (panic at `todo!()`) not BROKEN (import error / setup failure). | The source tree is empty at DISTILL handoff time — no `Cargo.toml`, no `src/`. We cannot run `cargo test` until DELIVER scaffolds the workspace. The gate is still a hard gate; it just runs one step into DELIVER. | LOCKED |
| DD-3 | State-delta + Universe assertion (Mandate 8) is **bootstrapped lazily** by DELIVER on first state-mutating scenario. The Rust port at `tests/common/state_delta.rs` does NOT ship in this DISTILL wave because no scenario can run yet to demand it. Skeleton tests use named assertion-helper functions in `support/mod.rs` (e.g. `assert_claim_file_exists_with_cid(cid)`, `assert_pds_contains_record_at(at_uri)`) as the Rust idiomatic mirror of universe-bound checks. DELIVER MUST migrate at least the WS-7 (CID stability) and FR-3 (at_uri reconstructibility) scenarios to `assert_state_delta(before, after, universe, expected)` form once the port exists. | The polyglot bootstrap policy (nw-distill matrix) says "apply-if-absent on first DISTILL." But the application requires a runtime, which requires `Cargo.toml`. Two-stage bootstrap: DISTILL declares the contract via named helper signatures; DELIVER materializes the `state_delta` module to back them. | LOCKED |
| DD-4 | Tier B (state-machine PBT) is **NOT added** for slice-01. Per Mandate 10 skip-criteria #3 ("the only observable is 'did it crash'") — slice-01's user-observable contract is "did the round-trip succeed and match field-for-field." That is a linear chained narrative (Pillar 2) without a state-machine model. The journey YAML has 4 steps but they are sequential per-user, not a domain-rich state space. | Re-evaluate when slice-03 federated-read lands a multi-author state machine. | LOCKED |
| DD-5 | Driving-adapter coverage = subprocess invocation of the real `openlore` binary via `assert_cmd` / `std::process::Command`. NO direct calls to library code from walking-skeleton tests. | Mandate 1 (Hexagonal Boundary). Direct library calls hide CLI wiring bugs (P1 RCA precedent from nw-distill). | LOCKED |
| DD-6 | PDS treatment = **fake double**, in-process HTTP stub OR recorded XRPC fixture (DELIVER picks the technique). Identity treatment = **fake double**, known test DID `did:plc:test-jeff` with a deterministic test key. Both doubles live in the new `crates/test-support/` crate or `tests/acceptance/support/` Rust module. The real ATProto contract suite is DEVOPS's deliverable (Pact, per DESIGN §6.5). | Network adapters are non-deterministic per the Architecture of Reference defaults; fakes appropriate. The real-contract validation is a separate test surface owned by DEVOPS. | LOCKED |
| DD-7 | Test directory = `tests/acceptance/` at workspace root (Rust integration-test convention). Files: `walking_skeleton.rs`, `lexicon_conformance.rs`, `federation_roundtrip.rs`, `support/mod.rs`, `README.md`. NO per-feature subdirectory since this is the first (and currently only) feature in the repo. | Future sibling features (slice-02 scrapers, slice-03 federation, etc.) will land their own `tests/<sibling_name>/acceptance/` directories. Slice-01 inherits the top-level location. | LOCKED |
| DD-8 | Error-path ratio in WS = 17.6% (3/17 in walking_skeleton.rs alone; 6/29 across all DISTILL files = 20.7%). This is below the 40% target. Infrastructure-failure scenarios (disk full, partial-write recovery, fsync lies, keychain locked, network mid-write timeout) are **deferred to DELIVER's adapter-level integration tests** because each requires per-OS fixture work that's better landed once the adapter implementations exist. | Slice-01's WS-layer covers the user-facing error paths the AC explicitly enumerates (out-of-range confidence, PDS unreachable, not-initialized). DELIVER's adapter tests are the right surface for infrastructure-failure coverage; they are NOT acceptance tests and not in DISTILL's scope. | LOCKED |
| DD-9 | Three `# DISTILL: confirm command name` flags from `gherkin-scenarios-expanded.md` resolve to **deferred sibling features**: (a) peer-counter scenarios → slice-03; (b) `claim status` inspect verb → slice-03; (c) `graph contrib` lurker-nudge → slice-04. The corrective-claim anxiety scenario (typo'd URL) binds to a **two-step workflow** using locked verbs (`claim retract <old-cid>` + `claim add` with corrected evidence). The `--edit` flow binds to **cancel + re-run** with new flags. The `--from-url` habit affordance defers to slice-02. | ADR-003 locks only 5 verbs (`init | claim add | claim publish | claim retract | graph query`). DESIGN §wave-decisions.md "Out of scope" explicitly defers the other CLI surfaces. DISTILL respects the locked surface. | LOCKED |
| DD-10 | Language detection = Rust (per ADR-009 + ADR-007). Test-framework matrix entry: `Rust | proptest | std #[test] | #[ignore] | <feature>_scenarios.rs + <feature>_specifications.rs (same module)`. We use the same-module shape with role-prefixed function names instead of file-suffixed splits, because plain `#[test]` makes the role read in the test name. | nw-distill polyglot adapter matrix. | LOCKED |
| DD-11 | Project Infrastructure Policy file at `docs/architecture/atdd-infrastructure-policy.md` is **NOT written by this DISTILL wave**. The orchestrator's brief limits DISTILL's write surface to `docs/feature/openlore-foundation/distill/` + `tests/acceptance/`. The first ATDD policy entries are documented inline here (see §"Inline policy entries" below) and should be migrated to the project-level file on the next DISTILL wave (slice-02 or slice-03) where the orchestrator scope permits. | Cross-wave write-surface convention. Inline documentation preserves the decision; file migration is a future move. | LOCKED |
| DD-12 | Lexicon conformance is its own test file (layer 2 acceptance) because the claim Lexicon is a **federation contract**, not a CLI concern. The CID-stability property test (LC-3) uses `proptest` (the Rust idiomatic PBT crate per nw-distill matrix) and is the ONLY `@property`-tagged scenario in slice-01. | ADR-006 §Earned Trust requires property + gold-fixture testing on canonicalization; the federation surface deserves dedicated test focus. | LOCKED |
| DD-13 | Walking-skeleton scenario count = 17 (not the 2-5 default from nw-test-design-mandates). The slice-01 journey has 4 sub-steps × ~4 AC variants per step ≈ 16 scenarios + 1 init bootstrap = 17. Each scenario is a single user-observable beat from the journey, not a "trace through layers" test, so they remain user-centric per the Walking Skeleton Litmus Test (each scenario title would pass a non-technical stakeholder's "yes, that is what users need" check). | Slice-01 IS the walking skeleton for the entire OpenLore umbrella; depth here pays for breadth in sibling features. The 4-step journey × 4 AC per step is a structural property of the slice scope, not bloat. | LOCKED |

---

## Inline policy entries (proto-`atdd-infrastructure-policy.md` content)

When the cross-wave write surface allows it (next DISTILL wave), the
following table content should land at
`docs/architecture/atdd-infrastructure-policy.md`:

```markdown
# ATDD Infrastructure Policy

Per `nw-distill` § Project Infrastructure Policy. One file per project.
Apply-if-exists; write-if-absent; rewrite with `--policy=fresh`. Git history
is the audit trail.

## Driving
| Port | Mechanism | Note |
|---|---|---|
| CLI (`openlore` binary) | subprocess from `tempfile::TempDir` via `assert_cmd` | sets HOME to TempDir so XDG paths are sandboxed |

## Driven internal (real)
| Port | Mechanism | Note |
|---|---|---|
| StoragePort (DuckDB + claims/<cid>.json) | real DuckDB file in `tempfile::TempDir`, fresh DB per test | atomicity probe via real fsync; substrate matrix lives in DEVOPS pipeline |
| ClockPort | real `std::time` | degenerate adapter |

## Driven external / non-deterministic (fake)
| Port | Fake | Note |
|---|---|---|
| PdsPort (ATProto XRPC) | `FakePds` (in-process HTTP stub or recorded fixture replay) | real-contract validation = DEVOPS Pact suite at `tests/contract/pds/` |
| IdentityPort (DID + key) | `FakeIdentity` with known test DID `did:plc:test-jeff` and deterministic Ed25519 key | real keychain integration = DELIVER's adapter test, not DISTILL |
```

---

## Open questions handed to DELIVER

These are deliberately deferred to the DELIVER wave:

1. **`cucumber-rust` adoption**: should the WS scenarios become `.feature`
   files in DELIVER? If yes, the per-test name reads better; if no, the
   plain `#[test]` form is simpler. Crafter's call after first scenario
   goes green.
2. **`test-support` crate location**: a workspace member (`crates/test-support/`)
   OR a `tests/common/` module hoisted into each integration test file's
   imports. DESIGN does not specify; both are idiomatic. Crafter's call.
3. **`FakePds` implementation technique**: in-process HTTP stub (e.g.
   `wiremock` crate) vs recorded XRPC fixture (a JSON file replayed
   verbatim). Both satisfy the slice-01 acceptance contract; crafter
   picks whichever has less ceremony for the first scenario.
4. **PROPTEST RNG seed pinning**: ADR-006 §Earned Trust says "property
   test in CI on every commit." For determinism, crafter should pin the
   proptest seed in `proptest.toml` so failing cases reproduce.

---

## Out of scope for this DISTILL (explicit deferrals)

- Sibling-feature acceptance tests (slices 02-05): each gets its own DISTILL
  wave under its own feature directory.
- CLI verbs flagged in `gherkin-scenarios-expanded.md` but not in slice-01
  (`claim status`, `claim counter`, `graph contrib`, `--from-url`,
  `--corrects`/`--supersedes` flags). See DD-9 for the deferral mapping.
- Infrastructure-failure scenarios at the WS layer (disk full, fsync lies,
  network mid-write, keychain crashed). See DD-8.
- Contract tests against the real ATProto PDS via Pact — DEVOPS's
  deliverable per DESIGN §6.5.
- Stress / residuality scenarios (none triggered).
- Bootstrapping `docs/architecture/atdd-infrastructure-policy.md` —
  deferred per DD-11.

---

## Handoff summary

| Recipient | Reads | Produces |
|---|---|---|
| DELIVER (functional-software-crafter) | `acceptance-tests.md`; `traceability.md`; the four test skeletons in `tests/acceptance/`; this file; the open-questions list above; DESIGN's component-boundaries.md for the Cargo workspace shape | `Cargo.toml` + 8 crates (per DESIGN) + `test-support` crate; one-at-a-time scenario implementation; `tests/common/state_delta.rs` lazy bootstrap; `proptest.toml` for LC-3. |

---

## Changelog

- 2026-05-25 — Quinn — initial DISTILL-wave decisions for slice-01. All
  decisions DD-1..DD-13 LOCKED. Reconciliation against DISCUSS/DESIGN
  passed with 0 contradictions.
