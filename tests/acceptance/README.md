# OpenLore Acceptance Tests

These are the outer-loop acceptance tests for OpenLore. They drive
Outside-In TDD: every scenario fails until the production code makes it
pass, and the scenarios collectively describe what slice-01 (the walking
skeleton) MUST do for the user.

## Where these come from

- DISCUSS wave: 5 user stories (US-001..US-005) tracing to job J-001
  ("Author a signed philosophical claim").
- DESIGN wave: 9 ADRs locking the hexagonal architecture, the two-prompt
  CLI contract (ADR-003), the retraction model (ADR-008), the CID scheme
  (ADR-006), the identity model (ADR-002).
- DISTILL wave (this directory): port-to-port acceptance tests that
  exercise the locked observable contract through the real CLI binary.

Authoritative design map: `docs/feature/openlore-foundation/distill/acceptance-tests.md`.

## Scope (what these tests cover)

| File | Coverage |
|---|---|
| `walking_skeleton.rs` | 17 user-journey scenarios driving the real `openlore` CLI as a subprocess; PDS + identity faked, everything else real |
| `lexicon_conformance.rs` | 8 scenarios validating the `org.openlore.claim` Lexicon shape (federation contract) — includes 1 property-based test |
| `federation_roundtrip.rs` | 4 scenarios validating publish→read-back round-trip integrity (CID stability, at-uri reconstruction, byte-for-byte query match) |
| `support/mod.rs` | Shared fixtures: `TestEnv` (temp HOME), `FakePds`, `FakeIdentity`, seed-claim builders, assertion helpers |

**29 total scenarios.** All RED at handoff (panic at `todo!()`).

## How to run (after DELIVER scaffolds `Cargo.toml`)

```bash
# all acceptance tests
cargo test --test walking_skeleton --test lexicon_conformance --test federation_roundtrip

# just walking skeleton
cargo test --test walking_skeleton

# single scenario
cargo test --test walking_skeleton walking_skeleton_compose_preview_contains_not_as_truth

# show test names with no run
cargo test --test walking_skeleton -- --list
```

## What's mocked, what's real

| Component | Treatment | Why |
|---|---|---|
| `openlore` CLI binary | REAL (subprocess via `assert_cmd`) | Driving adapter — must be exercised for slice-01 |
| `claim-domain`, `lexicon`, pure core | REAL (linked into the binary) | Pure functions — mocking would be testing-theater |
| `adapter-duckdb` (DuckDB + claims/<cid>.json filesystem) | REAL (DuckDB file in `tempfile::TempDir`) | Catches format drift, fsync issues, atomic-write bugs |
| `adapter-system-clock` | REAL (`std::time`) | Degenerate adapter |
| `adapter-atproto-pds` | FAKE (`FakePds` in-process double or recorded fixture replay) | Network non-determinism; real contract tested by DEVOPS via Pact in `tests/contract/pds/` (out of scope here) |
| `adapter-atproto-did` (identity) | FAKE (`FakeIdentity` with known `did:plc:test-jeff` + deterministic Ed25519 key) | OS keychain interaction tested by DELIVER's adapter integration test, not acceptance |

The PDS + identity doubles replace the federated boundary WITHOUT replacing
the local boundary. Every test still exercises:
- Real claim composition (clap parsing of all flags)
- Real canonicalization (CBOR encoder, RFC 8949)
- Real CID computation (sha2-256 over canonical CBOR)
- Real DuckDB write/read path
- Real `<cid>.json` atomic-write on the local filesystem

## Pre-requisites (DELIVER must land FIRST)

1. **Workspace `Cargo.toml`** with the 8 crates from
   `docs/feature/openlore-foundation/design/component-boundaries.md` §Crate
   (component) layout: `claim-domain`, `lexicon`, `ports`, `adapter-duckdb`,
   `adapter-atproto-did`, `adapter-atproto-pds`, `adapter-system-clock`,
   `cli`.
2. **A `test-support` crate** (or `tests/common/` module) providing
   `FakePds`, `FakeIdentity`, `TestEnv`, and the seed-fixture builders.
3. **The `openlore` binary entry point** at `crates/cli/src/main.rs`
   buildable via `cargo build --bin openlore` and discoverable by
   `assert_cmd::Command::cargo_bin("openlore")`.
4. **A `tests/common/state_delta.rs`** module per the nw-distill polyglot
   matrix (lazy bootstrap; the first scenario that needs Universe-bound
   assertion triggers the creation).

Until these land, `cargo test --test walking_skeleton` fails to compile
with `unresolved import openlore_test_support` and `cannot find binary
openlore`. This is the EXPECTED state at the DISTILL → DELIVER boundary.

## RED/GREEN flow

1. DISTILL ships every test body as `todo!("DELIVER will fill this in
   to make scenario green")` with the `// SCAFFOLD: true` marker on the
   module.
2. DELIVER scaffolds `Cargo.toml` + crates + test-support → tests now
   compile → tests classify as RED (panic at `todo!()`).
3. DELIVER picks ONE scenario, replaces `todo!()` with real wiring,
   writes the production code that makes it pass → that scenario goes
   GREEN.
4. DELIVER commits that scenario, picks the next one, repeats until all
   29 scenarios are green.
5. Final cleanup: zero `// SCAFFOLD: true` markers remain
   (`grep -r "SCAFFOLD: true" tests/` returns nothing).

## How scenario names encode intent

Test function names follow the shape `<file_role>_<user_goal_in_snake_case>`:

- `walking_skeleton_compose_preview_contains_not_as_truth_and_waits_for_confirmation`
- `lexicon_rejects_self_reference_in_references_array`
- `federation_roundtrip_publish_three_claims_different_predicates_all_round_trip_with_cids_intact`

A non-technical stakeholder reading just the test names should be able to
infer what the system does. Per Mandate 2 (business language) the test
names use domain terms (`claim`, `compose`, `sign`, `retract`, `publish`,
`evidence`, `confidence`) and zero technical jargon (`HTTP`, `database`,
`schema`, `endpoint`).

## What is NOT covered here (deferred elsewhere)

- **Infrastructure-failure scenarios** (disk full, fsync lies, network
  mid-write timeout, keychain crashed): DELIVER's adapter-level
  integration tests, not acceptance tests. See
  `docs/feature/openlore-foundation/distill/wave-decisions.md` DD-8.
- **Pact contract tests against the real ATProto PDS**: DEVOPS's
  deliverable per DESIGN §6.5; future location `tests/contract/pds/`.
- **Peer-published counter-claim flows** (anxiety scenario 1.1 from
  `gherkin-scenarios-expanded.md`): requires federation; deferred to
  slice-03's DISTILL wave.
- **`claim status` / `graph contrib` / `--from-url` / `--corrects`
  verbs**: not in ADR-003's slice-01 verb table; deferred to sibling
  features per DD-9.

## References

- `docs/feature/openlore-foundation/distill/acceptance-tests.md` — design map
- `docs/feature/openlore-foundation/distill/wave-decisions.md` — DD-1..DD-13
- `docs/feature/openlore-foundation/distill/traceability.md` — story/job mapping
- `docs/feature/openlore-foundation/design/architecture-design.md` — locked C4
- `docs/feature/openlore-foundation/design/component-boundaries.md` — crate layout
- `docs/adrs/ADR-003-cli-verb-contract.md` — locked verb surface
- `docs/adrs/ADR-008-retraction-counter-claim-no-hard-delete.md` — retraction model
- `docs/adrs/ADR-006-claim-addressing-cid.md` — CID scheme
