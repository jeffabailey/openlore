<!-- markdownlint-disable MD013 -->
# Evolution: serverless-philosophy-federation: publish your philosophy to your own Cloudflare instance, round-trip it with an identical CID, and federate between instances

> A full nWave feature (DISCUSS → SPIKE → DESIGN → DEVOPS → DISTILL → DELIVER). It
> implements the hosted deployment mode that ADR-023 deferred, as a self-hosted
> serverless option: each user runs their own instance. Paradigm: functional Rust
> (ADR-007) plus one new TypeScript/Cloudflare Workers target (`atproto/`).
> Companion design: ADR-062. 15 DELIVER steps across 5 phases. **FEATURE COMPLETE**
> (2026-09-27).

## Summary

openlore could federate only through Bluesky PDSes (J-003 `peer pull`). There was no
surface a user owned where they could publish their signed reasoning, prove the copy was
faithful, and share it. This feature adds that surface for two jobs:

- **J-007 (self-host and share):** a user deploys their own Cloudflare Worker from
  `atproto/`, registers it with `openlore publish init <url>`, and gets a read-only
  **philosophy card** at `/` that they can paste into their Bluesky profile.
- **J-008 (round-trip sync):** `openlore publish push` and `openlore publish pull` mirror
  the local DuckDB graph to that instance and back. Every claim that comes back has a
  recomputed CID identical to the one pushed (KPI-SF-1, the North Star).

Cross-instance federation (US-SF-006) reuses J-003. `peer pull` resolves a peer DID to a
Cloudflare instance and reads it through the same byte-preserving opaque surface. The
existing checks are reused as they are: local verification, the `peer_claims` separation,
per-author attribution and anti-merging (D-8: an extra transport, not a new job).

### The load-bearing finding (SPIKE-00 → OD-SF-1)

SPIKE-00 (local `wrangler dev`, 5 gold CID fixtures, run against the real
`claim-domain`) returned **WORKS, conditionally**. The CID survives the Rust ↔ Worker
boundary **only if the Worker is an opaque, CID-addressed byte store**. The ATProto
`putRecord` model, where the server assigns the CID, diverged on `claim_004`
(confidence `0.0`). Root cause: `ciborium` emits RFC 8949 shortest-form floats
(`0.0/0.5/1.0` → float16), and IPLD DAG-CBOR requires float64 (with whole floats encoded
as integers). So a JS `@ipld/dag-cbor` PDS computes a **different CID** for any "round"
confidence. This is a latent ADR-006 conformance gap. The opaque transport sidesteps it
and does not fix it (DDD-7).

## Key decisions

| ID | Decision | Source |
|---|---|---|
| D-1 / D-3 / D-4 | Self-hosted serverless. Each user deploys and **owns** their instance. There is **no central authority** and federation is peer-to-peer between users' own instances. | DISCUSS |
| D-5 | The CLI reaches the instance through the ADR-027 configurable URL. The transport is purely additive; the CLI contract does not change. | DISCUSS |
| D-6 | Publishing is additive and CID-verified. Local DuckDB stays canonical, and compose, sign and local query never depend on the instance (KPI-SF-5). | DISCUSS |
| D-7 | The card is read-only and cannot sign by construction. It renders only pushed claims, each attributed, with no consensus row. | DISCUSS |
| D-8 | Pulling from other instances is J-003 with a new transport, not a new job. | DISCUSS |
| DDD-1 (OD-SF-1) | **Opaque content-addressed HTTP blob store** (`PUT/GET /records/:cid`, `GET /manifest`, `GET /`). `putRecord` and a WASM core on the Worker were **rejected**. | SPIKE-00, DESIGN, ADR-062 |
| DDD-2 | **One Durable Object per instance**: strong read-after-write and an atomic manifest. KV was rejected because eventual consistency makes tests flaky. | DESIGN |
| DDD-3 | Blob format is verbatim lexicon-JSON, keyed by a CID minted in Rust. The card needs no CBOR. | DESIGN |
| DDD-4 (OD-SF-2) | Verb group: `openlore publish {init,push,pull,status}`. | DESIGN |
| DDD-5 (OD-SF-3) | Cross-instance reads are byte-preserving opaque reads with local Rust verification. J-003 is reused as it is. | DESIGN |
| DDD-7 | ADR-006 float nonconformance is **left as-is**, with a revisit trigger. A fix would change CIDs and break the wire format. | DESIGN |
| DDD-8 | Capability split: a write `PublishPort` and a read-only `InstanceReadPort`. The write capability cannot leak into pull or the card. | DESIGN |
| Single canonicalizer | `claim-domain::compute_cid` is the only thing that mints CIDs. The Worker never re-encodes, and `atproto/` has no IPLD/CBOR dependency on the CID path. | SPIKE-00, ADR-062 |
| DV-1 | New CI contract test `publish-contract.yml`: the real Worker on local `wrangler dev` does `init → push → pull`, including a `0.0/0.5/1.0` float regression guard. | DEVOPS |
| DV-2 / DV-3 | No deploy in CI and no `CLOUDFLARE_API_TOKEN`. Each user runs `wrangler deploy` into their own account, and `publish init` only registers the URL. | DEVOPS |
| DV-4 (Q-SF-D2) | Writes are authed with a per-instance bearer token stored as a Worker secret. Reads (records, manifest, card) are public. | DEVOPS, step 02-03 |
| DV-8 | No central telemetry. Observability is for the owner only (Workers analytics, `wrangler tail`). | DEVOPS |
| Q-SF-D4 | `parse_signed_claim` was hoisted into `lexicon` (`encode_signed_claim` / `decode_signed_claim`), so there is one shared pure decode path. | DELIVER |
| Q-SF-D5 / ADR-062 §6 | `probe()` checks reachability and the `/manifest` marker. There is **no canary write**, because an append-only store would pollute the public card. CID round-trip is still checked on every push and in DV-1. | DELIVER roadmap (`47239ac`) |
| Q-SF-D6 | `publish-domain` is its own pure-core crate (not folded into `cli`). New adapter: `adapter-publish-http`. | DELIVER |

## Steps completed (DELIVER)

Every step logged `COMMIT EXECUTED/PASS` in `deliver/execution-log.json`.

| Step | Outcome | Commit | Notes |
|---|---|---|---|
| 01-01 | Walking skeleton: `publish init → push → pull` of one signed claim with an identical recomputed CID | `b71fa7b` | |
| 01-02 | `0.0/0.5/1.0` gold fixture round-trips CID-identical. Adds the real `atproto/` Worker and `publish-contract.yml` | `acdd6ee` | |
| 01-03 | `publish init` outcomes: registered, cannot-reach, not-an-openlore-instance | `113a1a6` | |
| 01-04 | `init` never touches identity or the local store. `status` is read-only | `4238f1c` | RED_UNIT skipped: pure renderer change driven by PI-5 |
| 01-05 | CID drift is rejected and never stored. An unreachable instance never blocks offline authoring | `1207ab9` | |
| 02-01 | Bulk push sends only new claims, is CID-verified, and never mutates local data | `c372817` | |
| 02-02 | Re-push is idempotent. An interrupted push resumes without duplicates | `0a5a8af`, `bb5ec0b` | Worker check-then-append moved into `blockConcurrencyWhile` |
| 02-03 | The write path requires the owner token. Token-less pushes are refused and store nothing | `c5e6324` | |
| 03-01 | An in-sync pull is an additive no-op reconcile, and every claim recomputes an identical CID | `806b6d9` | |
| 03-02 | A pull on a fresh machine rebuilds DuckDB with attribution intact. An unreachable instance leaves local data untouched | `6612ba1` | |
| 03-03 | A conflicting pulled record is shown with both CIDs and never overwrites the local claim | `3b18734` | |
| 04-01 | The card renders only pushed claims, each attributed. An empty instance shows "no claims published yet" | `8fc545b` | |
| 04-02 | The card cannot write, never merges authors into one row, and reads need no token | `01e007c` | |
| 05-01 | `peer pull` reads a peer's Cloudflare instance, verifies locally, and stores into `peer_claims` without merging | `e0cb4c7` | |
| 05-02 | A tampered peer claim is rejected while valid claims are stored. An unreachable peer is skipped while the others continue | `f462f38` | RED_UNIT skipped: green on activation, no new pure logic |

Refactor (L1-L2): `f398883` (plan push/pull from the probe's single `GET /manifest`),
`c9f300d` (tidy the pull verb and its renderer), `894edd1` (acceptance fixtures use the bare
DID; publish support helpers consolidated). Mutation kill tests: `b6790e3`.

## Mutation results

cargo-mutants 25.3.1 (`--timeout 120 -j 2`). Gate: at least 80% kill rate. Full report:
`docs/feature/serverless-philosophy-federation/deliver/mutation/mutation-report.md`.

| File | Before | After |
|---|---|---|
| `crates/publish-domain/src/lib.rs` (gated) | 33/43 = 76.7% (WARN) | **43/43 = 100%** |
| `crates/lexicon/src/claim.rs` (gated) | 12/12 = 100% | 12/12 = 100% |
| `crates/ports/src/publish.rs` (optional) | 0/3 in scoped run | **3/3 = 100%** |
| `crates/cli/src/render/publish.rs` (optional) | not measured | not measured (see Issues) |

**Gated total: 55/55 = 100%, PASS.** None of the 10 publish-domain survivors was
equivalent. The new killing properties are:

- `ReconcileTally::in_sync` ⇔ every outcome is `Matched`
- `ReadbackMismatch::describe` gives a distinct reason for each variant
- `manifest_cids` returns CIDs in order
- a `ports` contract test pins every `InstanceError` dotted reason code

## Issues encountered

- **The CLI renderer is not mutation-measured.** Running `cargo mutants -p cli --file
  crates/cli/src/render/publish.rs` stopped with "cargo test failed in an unmutated tree".
  21 `-p cli --test appview_search` tests fail in cargo-mutants' copied scratch tree. They
  depend on the environment and are unrelated to this feature. With no clean baseline, no
  mutants ran. The renderers are covered by the workspace acceptance suite
  (`tests/acceptance`, a separate package). The surface was optional and is not counted
  in the gate. **Follow-up:** make `appview_search` hermetic under a copied tree (or pass
  `--in-place` in a clean checkout) and then measure `render/publish.rs`.
- **Concurrent re-PUT race in the Worker (02-02).** The FakeInstance was already
  idempotent, but the real Worker's check-then-append on the manifest could duplicate
  entries under concurrent re-PUTs. Fixed by running it inside the Durable Object's
  `blockConcurrencyWhile`. The contract script now checks `pushed:0` on re-push and
  concurrent re-PUTs under `wrangler dev`. 02-02 was committed twice (`0a5a8af`,
  `bb5ec0b`), and the execution log has duplicate RED/GREEN events for 02-02 after the
  first COMMIT.
- **A canary-write probe was rejected mid-roadmap.** The first probe design wrote a test
  record to prove round-trip. On an append-only store, that record would have shown up on
  the public card. ADR-062 §6 was amended so the probe only checks reachability and the
  manifest marker.
- **The latent ADR-006 float nonconformance remains.** The opaque transport sidesteps it
  (DDD-7), but any future interop with a standard PDS would hit it. A regression guard
  (RT-4, plus the float fixtures in `publish-contract.yml`) stops the rejected re-encoding
  transport from coming back.
- **Host speed.** The full workspace suite is slow on the dev host. CI on `main` is the
  authoritative full-suite gate (trunk-based; see DV-7).

## Lessons learned

- **Spike the one load-bearing unknown and throw the probe away.** SPIKE-00 took under a
  day, ran against the real canonicalizer, and did more than answer "does it work". It
  found *why* it only sometimes works (shortest-form floats), and that finding shaped the
  whole architecture. Discarding the probe code kept DESIGN free to choose the storage
  medium.
- **Keep a single canonicalizer.** A CID is a contract only if exactly one encoder mints
  it. Keeping the Worker dumb (no IPLD, no CID computation) made the TypeScript surface
  small, and the riskiest property became a CI guard rather than something that depends
  on skill.
- **Fakes need to be as strict as the real system's concurrency model.** The
  FakeInstance's idempotency hid a real Durable Object race. The live `wrangler dev`
  contract test (DV-1) caught it, which shows it was worth keeping a real-transport CI
  lane alongside hermetic acceptance tests.
- **A probe must not have side effects on something public.** "Write a canary to prove
  it" sounds safe until the store is append-only and public. Check that probes are free
  of side effects against every read surface.
- **Mutation testing finds untested links.** Every publish-domain survivor was a real
  gap: a predicate or describe method that only the CLI exercised. Running mutation per
  crate made the missing in-crate properties visible.

## Links

- **Design:** `docs/adrs/ADR-062-serverless-opaque-federation-transport.md` (opaque
  transport; §6 probe amendment). Related: ADR-023 (self-hostable indexer; the deferred
  hosted mode), ADR-027 (configurable URL), ADR-006 (canonical CBOR / CID), ADR-009
  (hexagonal), ADR-007 (functional Rust).
- **Migrated scenarios:** `docs/scenarios/serverless-philosophy-federation/walking-skeleton.md`
  (status IMPLEMENTED).
- **Workspace** (kept): `docs/feature/serverless-philosophy-federation/`:
  `feature-delta.md` (all waves, C4 L1/L2, KPIs), `slices/slice-00..05`,
  `spike/{findings.md,wave-decisions.md}`, `design/wave-decisions.md`,
  `devops/wave-decisions.md`, `environments.yaml`,
  `distill/{test-scenarios.md,acceptance-review.md,walking-skeleton.md}`,
  `deliver/{roadmap.json,execution-log.json,mutation/mutation-report.md}`.
- **Code:** `atproto/` (Worker: `src/{index,instance,card}.ts`, `wrangler.toml`,
  `scripts/contract-roundtrip.sh`), `crates/publish-domain/`, `crates/adapter-publish-http/`,
  `crates/ports/src/publish.rs`, `crates/lexicon/src/claim.rs`,
  `crates/cli/src/render/publish.rs`.
- **CI:** `.github/workflows/publish-contract.yml` (DV-1).
- **Acceptance:** `tests/acceptance/{publish_roundtrip,publish_init,publish_push,publish_pull_reconcile,public_card,cross_instance_pull,peer_pull}.rs`;
  `crates/adapter-publish-http/tests/opaque_instance_http.rs`.
- **Commits:** DISCUSS `21d41ab`, SPIKE `1dc95ab`, DESIGN `d4406dd`, DEVOPS `f9ead64`,
  DISTILL `12b20a1`, roadmap `47239ac`, DELIVER `b71fa7b` … `f462f38`, refactor
  `f398883` / `c9f300d` / `894edd1`, mutation `b6790e3`.
