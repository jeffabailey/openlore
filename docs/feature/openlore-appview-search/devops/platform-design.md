# Platform Design Delta — openlore-appview-search (slice-05)

- **Wave**: DEVOPS (design portion; the sibling-feature extension of slice-01/02/03/04 DEVOPS)
- **Date**: 2026-05-28
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-appview-search (sibling slice-05; the FINAL umbrella slice; discover-across-the-network)
- **Inherits**: openlore-foundation DEVOPS (D-D1..D-D13, ADR-010..ADR-012) — UNCHANGED in policy; openlore-federated-read DEVOPS (D-D14..D-D21) — UNCHANGED; openlore-github-scraper DEVOPS (D-D22..D-D29) — UNCHANGED; openlore-scoring-graph DEVOPS (D-D30..D-D34) — UNCHANGED
- **Paradigm context**: functional Rust (ADR-007, Accepted)
- **Runs in PARALLEL with DISTILL** — this DEVOPS wave depends on the APPROVED DESIGN (ADR-023..027, WD-111..124, I-AV-1..9), NOT on DISTILL outputs.

This is the DEVOPS platform-design **delta** for slice-05. The slice-01 platform
layer (operating model, branching, gate inventory, distribution, substrate
matrix) and the slice-02/03/04 deltas are the inherited baseline. This document
records only the new extensions. Read in conjunction with
`docs/feature/openlore-foundation/devops/platform-design.md`,
`docs/feature/openlore-federated-read/devops/platform-design.md`,
`docs/feature/openlore-github-scraper/devops/platform-design.md`, and
`docs/feature/openlore-scoring-graph/devops/platform-design.md`.

**Slice-05 is the architecturally heaviest DEVOPS slice of the umbrella.** Unlike
slice-04 (read-only LOCAL; no new external surface; no new deployable; no
`deny.toml` change), slice-05 stands up the FIRST genuine network service. It is
the first slice to add (a) a SECOND deployable (`openlore-indexer`, ADR-023), (b)
TWO external/cross-process contract boundaries since slice-01/02 (WD-123), (c) a
`deny.toml` change (the `axum` ban must be narrowed — see §10 + Upstream Issue),
and (d) an indexer-OPERATOR observability surface distinct from the CLI's
author-side telemetry. It does ALL of this while keeping the local-first CLI's
DEVOPS surface (CI flows, distribution, offline guarantee) structurally
unchanged — the indexer is an ADDITIVE deployable.

## 1. What did NOT change

| Concern | Status | Reference |
|---|---|---|
| Operating model (local-first, solo-dev, no SLOs, no on-call) for the CLI | UNCHANGED | foundation §1 |
| DORA framing (per-release tag, no fleet) | UNCHANGED | foundation §1 |
| CI tool (GitHub Actions) + branching (GitHub Flow) | UNCHANGED | foundation §7, D-D1, D-D7 |
| CLI distribution (`cargo install` + 4-platform binaries; Windows out) | UNCHANGED in policy; the release matrix GAINS one artifact (`openlore-indexer`) — see §3, D-D35 | ADR-011, D-D10 |
| Release artifact security (cosign + SBOM + SLSA) | UNCHANGED in policy; now covers BOTH binaries | ADR-012, D-D11 |
| Author-side telemetry opt-in policy (off by default; no endpoint operated) | UNCHANGED for the CLI; the indexer-operator surface is a DISTINCT concern (see `observability.md` delta §7) | ADR-010, D-D4 |
| Local quality gates (lefthook/pre-commit + pre-push) | UNCHANGED in shape; the mirrored commit-stage set widens with the new crates | foundation §5 |
| Quality-gate inventory taxonomy | UNCHANGED in shape; entries added | foundation §6; this doc §3 |
| Mutation testing POLICY (nightly-only, release-tag blocking, pure-core scope) | UNCHANGED in policy; SCOPE widens (+`appview-domain`; + the `claim-domain` decode helper is already in scope as part of `claim-domain`) | D-D8, D-D23, D-D31; this doc §5, D-D40 |

## 2. What DID change (the delta)

Slice-05 introduces SIX new platform-layer concerns; CRUCIALLY, four are genuinely
new to the umbrella (the prior four slices were all CLI-local):

| New concern | Where it lives | Why slice-05 introduces it |
|---|---|---|
| A SECOND deployable (`openlore-indexer`) | the ADR-011 release matrix gains one artifact set; a `cargo run -p openlore-indexer` dogfood tool for the walking skeleton | ADR-023: the AppView is a self-hostable single binary — the FIRST network service. ADR-011 already cites "slice-05 AppView" as a release-matrix revisit trigger. |
| TWO external/cross-process contract boundaries | `contract-pact-indexer-query` (CLI↔indexer) + `contract-pact-pds-network` (indexer→PDS/PLC) CI sub-jobs (new) | WD-123 / DESIGN §6.2: the first external boundaries since slice-01/02; both are adversarial-input-capable; contract tests pin the attribution (per-result `author_did`) + verify-gate (record + DID-doc) shapes. |
| Two release-blocking network-scale GUARDRAILS (verified-before-index + anti-merging-at-network-scale) | `at-indexer-rejects-unverified-claim` (KPI-AV-3) + `at-network-result-preserves-attribution` (KPI-AV-2) CI jobs (new) | KPI-AV-2/3 are cardinal DISCUSS disprovers (any failure UNSHIPPABLE); both extend the slice-03 guardrails (KPI-FED-6, KPI-FED-1) to network scale. |
| A new pure-core mutation target (`crates/appview-domain`) | nightly `cargo mutants --package` list extension (THIRD widening) | The `ingest_decision` gate + `compose_results` anti-merging composition are the load-bearing trust primitives; un-mutated, their correctness is unguarded — exactly the D-D23/D-D31 reasoning applied to the new pure core. |
| A `deny.toml` change (the `axum` ban narrowed; bs58 confirmed in-allowlist) | `deny.toml` `[bans]` edit (the FIRST `deny.toml` change since slice-01) | The slice-01 ban "OpenLore is a CLI; we never run an HTTP server in-process" is no longer true: the indexer IS a network service that serves HTTP. The ban premise is slice-05-obsolete. See §10 + Upstream Issue. |
| The indexer-OPERATOR observability surface + the index-coverage/freshness dashboard | indexer-side `tracing` events (`indexer.ingest.*`, `indexer.startup.*`) into the indexer's OWN log; the coverage/freshness diagnosis (KPI-AV-1 sparsity) | The indexer is a long-running service the operator runs; its ingest health + index coverage are operator concerns distinct from the author-side opt-in telemetry (ADR-010). See `observability.md` delta §2.6, §9. |

Everything else is additive within existing structures — new CI jobs in the
existing `ci.yml`, two Pact sub-jobs in the existing acceptance stage, the
mutation `--package` extension in `nightly.yml`, new `tracing` event names emitted
from new code paths, new adapter probes, and a narrowed `deny.toml` ban. NO new
workflow file (Simplest-Solution Alternative 3, rejected in §7).

## 3. The new deployable — `openlore-indexer` (ADR-023)

Slice-05 ships TWO single-purpose binaries from one workspace:

```
openlore                # the CLI (source of truth; offline-capable). UNCHANGED footprint + ONE new verb (search).
openlore-indexer        # NEW self-hostable network service (signing-incapable; holds no local store).
```

### 3.1 Build + release (the ADR-011 release-matrix delta — D-D35)

- The `openlore-indexer` binary is built in CI for the SAME 4-platform matrix as
  the CLI (`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`; ADR-011). It is built by the same workspace
  `cargo build` — no cross-compile, native build per target.
- The ADR-011 release matrix gains one artifact set: each platform release now
  ships TWO binaries (`openlore` + `openlore-indexer`), each accompanied by its
  `.sha256`, `.sig` (cosign), under the SAME release-wide `sbom.cdx.json` +
  `provenance.intoto.jsonl` (ADR-012 / D-D11; the SBOM now covers both binaries'
  dependency trees, including the new `axum` + `bs58` deps).
- `cargo install openlore-indexer`: the `openlore-indexer` crate is published to
  crates.io alongside `cli` (the OTHER workspace crates keep `publish = false`).
  This mirrors the `cargo install openlore` channel (ADR-011 §Channels).
- Windows stays deferred for BOTH binaries (ADR-011). ADR-011's revisit trigger
  ("slice-05 AppView introduces a need we can't avoid") is evaluated: it does NOT —
  the indexer is a Linux/macOS self-hosted service; no Windows need surfaces.
  Windows remains deferred (D-D35).

### 3.2 Run + lifecycle (self-hosted; the walking-skeleton shape)

- For the walking skeleton the indexer ships as a `cargo run -p openlore-indexer`-able
  dogfood tool, with two subcommands (ADR-023 / component-boundaries §`openlore-indexer`):
  - `openlore-indexer serve` — run the bounded pull-ingest loop + serve the HTTP/XRPC
    query API (the long-running mode).
  - `openlore-indexer ingest` — a one-shot bounded pull pass (no serve; useful for
    seeding/testing + the hermetic CI ingest fixture).
- A packaged service unit (systemd / launchd) is a FUTURE concern (D-D35; mirrors
  ADR-023 "the walking skeleton ships the indexer as a `cargo run`-able dogfood
  tool, not a packaged service"). DEVOPS documents the lifecycle but ships no
  unit file.
- The indexer's lifecycle is DISJOINT from the CLI's: the CLI is invoked per-command
  and exits; the indexer (`serve`) is long-running. The CLI is UNCHANGED for all
  local-first flows (compose/sign/publish/peer/graph query) — `search` is the only
  CLI verb that talks to the indexer, and it degrades gracefully (ADR-027 / I-AV-3).

### 3.3 Indexer runtime config (the indexer's OWN config — NOT identity.toml)

Per `design/data-models.md` §"indexer config" (ADR-023/024/025/026/027 — config-disjoint
from the CLI's `identity.toml`):

```toml
# <indexer config path>, e.g. ~/.config/openlore-indexer/config.toml
[indexer]
index_path      = "~/.local/share/openlore-indexer/index.duckdb"   # SEPARATE store (ADR-025); re-buildable
listen_addr     = "127.0.0.1:7619"          # the HTTP/XRPC query surface (ADR-027); localhost default
plc_endpoint    = "https://plc.directory"   # DID-document resolution default (ADR-026); the production PLC directory
ingest_interval = "15m"                       # bounded-pull cadence (ADR-024; DELIVER tunes — Q-DELIVER-AV-4)

[indexer.sources]
seed_dids = ["did:plc:...", "..."]            # bounded seed set (ADR-024)
relay     = "https://relay.example..."        # OPTIONAL; still PULL, not a firehose subscription
```

The CLI's `identity.toml` gains ONE optional key so `search` knows where to query:

```toml
[appview]
indexer_url = "http://127.0.0.1:7619"   # the self-hosted indexer (ADR-023/027); localhost default
```

Config defaults that are DEVOPS-load-bearing (locked as D-D35):

| Key | Default | Why DEVOPS cares |
|---|---|---|
| `plc_endpoint` | `https://plc.directory` | The PLC directory is the ADR-026 trust anchor for production pubkey decode (KPI-AV-3 against real data). It is an EXTERNAL dependency the indexer→PDS/PLC contract test pins (see `contract-test-ownership.md`). |
| `listen_addr` | `127.0.0.1:7619` | localhost-only by default — the indexer is NOT exposed to the network by default (no auth surface in the walking skeleton; ADR-023 multi-tenant deferred). A remote bind is the operator's explicit choice. |
| `index_path` | `~/.local/share/openlore-indexer/index.duckdb` | SEPARATE from `~/.local/share/openlore/openlore.duckdb` (the CLI's source of truth). The index store is RE-BUILDABLE; its "backup" is re-ingest, not a backup target (§8). |
| `ingest_interval` | `15m` (DELIVER tunes) | Drives the index-freshness window (the KPI-AV-1 sparsity / ingest-lag dashboard; `observability.md` delta §9). |
| Both configs | local-only, no telemetry | The CLI never reads the indexer's config; the indexer never reads `identity.toml` (config-disjoint, ADR-023). |

### 3.4 The index store is RE-BUILDABLE (the backup-story delta)

The `index.duckdb` store is a re-buildable cache of verified public claims (ADR-025).
Its "backup" story is "re-ingest", NOT a backup target — distinct from the CLI's
source-of-truth `openlore.duckdb`. DEVOPS does NOT design a backup/DR procedure for
`index.duckdb` (D-D35): on loss, the operator re-runs `openlore-indexer ingest`.
This is the inverse of the CLI store (the user's signed claims ARE the source of
truth and the artifact files under `claims/` are their backup).

## 4. Environment matrix (the acceptance + runtime environments)

Slice-05 carries the slice-03/04 graceful-degrade-by-default acceptance environments
AND adds indexer-specific runtime/substrate concerns. Hermetic by default (no real
network in PR/nightly; the real PLC + real PDS run release-only, gated, mirroring
the slice-03 real-bsky Pact policy D-D12).

### 4.1 Acceptance environments (the hermetic default set — carried + extended)

| Environment | Inherited shape | Slice-05 extension |
|---|---|---|
| **clean** (hermetic default) | fresh XDG dirs; no config; in-process fakes | + a `FakeIndexQuery` (CLI side) returning a fixture XRPC response with per-result `author_did`; + a `FakeIngestSource` + `FakeIndexStore` (indexer side) over an in-memory index; + the real-`z6Mk...` DID-doc fixture for the ADR-026 decode gold test. NO real network (WD-105 public-data-only is exercised against fixtures). |
| **with-pre-commit** (a developer machine with the lefthook/pre-commit hooks installed) | the foundation local-gate environment | UNCHANGED in shape; the mirrored commit-stage set now includes the new crates' fmt/clippy + the extended `arch-check`/`check-probes` rules (run in the pre-push subset). |
| **with-stale-config** (a config from a prior slice) | the slice-03/04 forward-compat environment | + an `identity.toml` WITHOUT the `[appview] indexer_url` key → `search` MUST treat a missing key as "no indexer configured" and degrade to the local-only message (NOT a fatal config error); + an indexer `config.toml` WITHOUT `[indexer.sources]` → the indexer refuses to start with a clear config error (an empty seed set is a config error, not a silent no-op ingest). |
| **indexer-in-container** (NEW — the indexer-specific environment) | n/a (slice-05 first) | The indexer is LIKELY run in a container; the `index.duckdb` fsync-honesty probe (ADR-025) MUST refuse to start if the container substrate lies about durability (overlayfs/DrvFs/tmpfs `fsync` no-op → `storage.fsync_unhonored` refusal). See §6. |
| **localhost-transport** (NEW — the CLI↔indexer transport environment) | n/a (slice-05 first) | The CLI↔indexer XRPC contract test + the `search` ATs run against a localhost-bound fixture indexer (`127.0.0.1` on an ephemeral port), NOT a real remote host. The transport is hermetic (an in-process test server reusing the chosen HTTP framework's test utilities, `technology-stack.md` §test-only). |

The default posture stays GRACEFUL-DEGRADE (like slice-03/04): the CLI `search`
verb is non-fatal when the indexer is unreachable (the `local_first_preserved`
gate; KPI-5). The indexer itself is FAIL-FAST at startup (it REFUSES to start on
any probe failure — `health.startup.refused` + exit 2; ADR-009/023) because a
mis-wired or substrate-lying indexer must not silently serve unverified or
attribution-losing results.

### 4.2 Runtime environment concerns (the indexer as a deployed service)

| Concern | Decision | Rationale |
|---|---|---|
| Container substrate (overlayfs/tmpfs/DrvFs) | the `adapter-index-store` probe exercises the fsync-honesty check and REFUSES on a durability lie (ADR-025) | The indexer's index is re-buildable, but a substrate that silently drops writes would corrupt the verified-row invariant; the probe makes "the container lies about durability" a startup refusal, not a runtime surprise. |
| Network egress (PLC + PDS reads) | the indexer makes OUTBOUND reads only (PLC DID-doc resolution + PDS `listRecords`); NO inbound network by default (`listen_addr` is localhost) | Public-data-only (WD-105 / I-AV-4); no auth-scoped read; no surveillance affordance; the indexer is not a public endpoint in the walking skeleton (ADR-023 multi-tenant deferred). |
| Resource sizing | no quotas/limits/HPA designed | Single-host single-file index store (ADR-025); a single self-hosted dogfood instance. K8s/autoscaling is explicitly OUT (§7 Alternative 1). |
| Listen-address exposure | localhost default; a remote bind is the operator's explicit, documented choice WITHOUT auth in the walking skeleton | A remote bind without auth is an operator decision flagged in the runbook (`observability.md` delta §9); auth (an API token header on `IndexQueryPort`) is the ADR-023/027 hosted-mode revisit trigger, not a slice-05 concern. |

## 5. Mutation scope (delta) — THIRD widening since slice-01

Per Apex Core Principle 9 + D-D8 (nightly-only, scoped to pure-core) + the D-D23
(slice-02 `scraper-domain`) and D-D31 (slice-04 `scoring`) precedents:

- **`crates/appview-domain` is added to the nightly `cargo mutants --package` list.**
  This is the THIRD mutation-scope widening, mirroring D-D23/D-D31's reasoning:
  slice-05 adds a GENUINELY NEW pure-core crate (WD-111 / DESIGN §5.1) — it MUST
  enter the `--package` list or the `ingest_decision` verify-before-index gate +
  the `compose_results` anti-merging composition are unguarded by mutation.
- **Kill-rate target ≥95%** (matches `claim-domain`, `scraper-domain`, `scoring`
  per ADR-006 Earned Trust). `ingest_decision` is the load-bearing trust gate
  (KPI-AV-3) and `compose_results` is the load-bearing anti-merging composition
  (KPI-AV-2); a surviving mutant in either would mean a tampered record could slip
  past the gate, or an author could be merged away, without any test failing —
  exactly the two cardinal disprovers.
- **The `claim-domain` decode helper (`decode_ed25519_multibase`, ADR-026) is
  mutated as part of the existing `claim-domain` mutation scope** (no new
  `--package` entry needed — it lives in the already-mutated `claim-domain` crate).
  Property: `decode∘encode == identity` for valid keys; malformed input errors
  (never panics, never mis-decodes); the mutation hardens THAT test. This is the
  load-bearing pubkey-decode primitive (I-AV-6).
- **The four new EFFECT crates are NOT mutated** (`adapter-atproto-ingest`,
  `adapter-index-store`, `adapter-index-query`, `adapter-xrpc-query-server`) —
  effect shell; their substrate concerns are Earned-Trust PROBE concerns + the
  acceptance/contract tests, per the D-D8 pure-core-only policy.
- Release-tag mutation re-run inherits the D-D8 blocking-on-regression gate;
  `appview-domain` is now in that re-run's scope.
- The `CLAUDE.md` `## Mutation Testing Strategy` section is UNCHANGED in POLICY
  (nightly-only per D-D8); only the `--package` list grows — a workflow-file edit,
  not a strategy change. Mirrors the D-D23/D-D31 note exactly.

Production crate count: 11 → 17 (per WD-111 / DESIGN component-boundaries; the new
`appview-domain` + 4 effect adapters + `openlore-indexer` binary). External
dependency count: **+2 minimal** (an HTTP server framework + a base58 crate, both
MIT, both with hand-rolled fallbacks; `technology-stack.md`). The `deny.toml`
change (§10) is the FIRST since slice-01.

## 6. Substrate matrix (delta)

The existing 8-cell release matrix and 4-cell PR subset are extended in TWO ways
(the first genuinely new substrate axis since slice-01):

- **The CLI cells** gain a `search` happy-path against a localhost fixture indexer
  AND the local-first degradation case (indexer unreachable → local-only message,
  no fatal error) — the same single DuckDB file the cell already provisions, plus
  a localhost-bound fixture indexer process (the `localhost-transport` environment).
- **A new indexer-store substrate concern** is added to the per-cell body: the
  `adapter-index-store` probe runs the fsync-honesty check on the cell's filesystem
  (the `index.duckdb` durability probe, ADR-025). On the tmpfs/overlayfs nightly
  cells (D-D9), this exercises the "container substrate lies about durability" case
  explicitly — the index-store probe REFUSES on a durability lie
  (`storage.fsync_unhonored`). This is the slice-05 application of the slice-01
  fsync-honesty discipline to the indexer's separate store.

No new platform/OS axis (the indexer ships on the same 4-platform matrix). The new
substrate concern is the indexer's SEPARATE store on the SAME filesystem/allocator
cells, plus the localhost transport.

## 7. Simplest Solution Check (per cicd-and-deployment skill)

Before extending CI/observability/deployment for slice-05's network service, three
simpler alternatives were considered. Per Apex Core Principle 4, complexity
(>3 components) requires documented rejected simpler alternatives.

### Alternative 1: "Run the indexer on Kubernetes / a cloud service with autoscaling + a managed store"
- **What**: deploy `openlore-indexer` as a containerized service on K8s (or a cloud
  run target) with an HPA, a managed Postgres/cloud-DB index store, and a managed
  ingress with auth.
- **Expected Impact**: meets ~100% of a HOSTED-multi-tenant indexer's needs.
- **Why insufficient / rejected**: the indexer is a SELF-HOSTABLE single binary
  (ADR-023) for a single dogfood operator validating the J-005 discovery thesis —
  NOT a multi-tenant hosted service. K8s/cloud/autoscaling/managed-DB is
  over-engineered: it re-introduces the "central service to operate" concern
  ADR-023 explicitly avoids, adds an ops surface the project is not resourced to
  own at the walking skeleton, and contradicts the local-first + data-sovereignty
  ethos. The index store is a single-file DuckDB (ADR-025) by the cardinal
  anti-merging-substrate reuse (WD-117). A hosted/multi-tenant deployment is the
  documented ADR-023 revisit trigger, not a slice-05 concern.

### Alternative 2: "Skip the two contract tests; trust the in-process fakes"
- **What**: rely on `FakeIndexQuery` / `FakeIngestSource` / `FakeIndexStore` for the
  CLI↔indexer + indexer→PDS/PLC boundaries; no Pact-style consumer-driven contracts.
- **Expected Impact**: meets ~50% of the boundary-safety need (the fakes exercise the
  HAPPY shapes the code expects).
- **Why insufficient / rejected**: WD-123 / DESIGN §6.4 flag both as the highest-risk
  boundaries (the first external/cross-process boundaries since slice-01/02). The
  CLI↔indexer response shape carries the cardinal per-result `author_did`
  (anti-merging across the transport, I-AV-2) — a PROVIDER change that dropped it
  would pass the consumer's fake but break attribution in production. The
  indexer→PDS/PLC record + DID-doc shapes are what the verify-before-index gate
  (I-AV-1) depends on — an ATProto/PLC response-shape drift would silently break
  ingest verification. Consumer-driven contracts pinning these shapes catch the
  drift at build time. Skipping them leaves the two cardinal guardrails'
  cross-boundary surface unguarded. (See `contract-test-ownership.md`.)

### Alternative 3: "Add a separate workflow file `indexer.yml` for the indexer build/test"
- **What**: a dedicated workflow trigger for the indexer crate; keep `ci.yml` for
  the CLI + prior slices.
- **Expected Impact**: meets ~100% of functional requirements but duplicates
  triggers, caches, toolchain setup, and branch-protection required-checks ceremony.
- **Why rejected**: identical reasoning to slice-03 Alternative 3 + slice-04
  Alternative 3. Both binaries ship from the SAME workspace; the CI is monorepo;
  `cargo build`/`nextest`/`clippy`/`check-arch` already operate over `--workspace`,
  so the new crates + the new binary are picked up by the EXISTING jobs by
  construction. Splitting workflows multiplies maintenance for zero isolation
  benefit. `ci.yml`, `nightly.yml`, and `release.yml` extend cleanly. DELIVER adds
  jobs to the EXISTING workflow files (see `ci-cd-pipeline.md` delta §3).

The chosen shape (a self-hostable single binary added to the existing release
matrix; extend `ci.yml` with the new acceptance + contract jobs; extend
`nightly.yml`'s mutation `--package` with `appview-domain`; narrow the `deny.toml`
`axum` ban; emit the privacy-preserving `search.*`/`indexer.*` events; the
index-coverage/freshness dashboard via the CLI-render + jq-fallback pattern) is the
minimum that satisfies the KPI-AV-1..6 instrumentation + the KPI-AV-2/3 + KPI-5
guardrails + the new-deployable + the two contract tests, WITHOUT introducing heavy
new infra (no k8s/cloud) and WITHOUT compromising local-first.

## 8. Risk register (delta)

New risks introduced by slice-05 (the first network service):

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| **The network lies** — a network source serves a tampered-signature / CID-mismatch / unsigned record and it enters the index (KPI-AV-3 collapse) | MEDIUM (the network is adversarial-input-capable by definition) | KPI-AV-3 collapses; discovery serves potentially fabricated reasoning; UNSHIPPABLE | The verified-before-index gate (`appview_domain::ingest_decision` reuses the pure `claim_domain::verify` + CID recompute, no second path; WD-104/121); `verified_against NOT NULL` schema invariant (ADR-025); the `adapter-atproto-ingest` probe rejects a fixture tampered/CID-mismatch record at startup; the `at-indexer-rejects-unverified-claim` release gate (KPI-AV-3); the `contract-pact-pds-network` contract pins the record-enumeration shape the gate depends on. |
| **The network lies about a key** — the indexer resolves a pubkey for a network author it has never met and the decode is wrong/forged (KPI-AV-3 against real data) | MEDIUM | a forged key could "verify" a tampered claim; KPI-AV-3 trust anchor breaks | The trust anchor is the PLC-resolved DID document (`plc.directory`), NEVER the record's (forgeable) `author` field; the real `decode_ed25519_multibase` (ADR-026, mutation-hardened in `claim-domain`); the `adapter-atproto-did` resolve-only probe decodes a real `z6Mk...` fixture + asserts it verifies a good signature AND rejects a tampered one (a seam-only pass is a CI failure); `no_pubkey_seam_in_release_build` xtask rule (I-AV-6); the `contract-pact-pds-network` contract pins the DID-doc/`publicKeyMultibase` shape. |
| **Silent author merge at network scale** — a future index-store SQL `GROUP BY object` drops `author_did` (KPI-AV-2 collapse) | LOW-MEDIUM (the corpus is large; an aggregating query is tempting) | KPI-AV-2 collapses; the AppView becomes indistinguishable from a faceless aggregator; UNSHIPPABLE | THREE-layer enforcement (WD-120): type-level (non-`Option` `author_did` on `IndexedClaim`/`NetworkResultRow`; `compose_results` returns a per-author structure with no merged-row API; aggregation in pure Rust, never SQL), structural (`no_cross_table_join_elides_author` xtask rule EXTENDED to `adapter-index-store` SQL literals), behavioral (`at-network-result-preserves-attribution` release gate; KPI-AV-2); the `contract-pact-indexer-query` contract pins that every wire result carries `author_did`. |
| **The container substrate lies about durability** — overlayfs/tmpfs/DrvFs `fsync` is a no-op; a verified row is silently dropped | MEDIUM (the indexer is likely run in a container) | the index silently loses verified rows; coverage diagnosis (KPI-AV-1) is corrupted | The `adapter-index-store` probe runs the fsync-honesty check and REFUSES to start (`storage.fsync_unhonored`) on a durability lie (ADR-025; the slice-01 discipline applied to the indexer's separate store); the tmpfs/overlayfs nightly substrate cells (D-D9) exercise the lie explicitly (§6). |
| **The local-first guardrail regresses** — adding the network service breaks offline compose/sign, or the CLI hard-fails when the indexer is down (KPI-5 collapse) | LOW | KPI-5 collapses; the architectural shift compromised the core promise; UNSHIPPABLE | The CLI links NO indexer code (the dependency graph excludes the indexer's store/ingest/server crates — `xtask check-arch`); `search` is the only network verb and degrades softly (`IndexQueryError::Unreachable` is non-fatal); the indexer is NOT probed at CLI startup (per-`search`-soft, ADR-027); the `at-local-first-preserved` release gate (KPI-5) runs offline compose/sign/graph-query + `search`-degrades under `unshare -n` with the indexer down. |
| **Index too sparse to discover anything** — a single self-hosted indexer ingested too little, or only already-followed authors (KPI-AV-1 < 20% disprover) | MEDIUM (the KPI-AV-1 north-star + coverage are coupled; flagged in DISCUSS Risks) | KPI-AV-1 near-zero; the J-005 discovery thesis weakened | The index-coverage/freshness dashboard (claims indexed, distinct authors indexed, ingest lag) handed to DEVOPS (`observability.md` delta §9); seed-set + relay config (ADR-024); the KPI-AV-1 < 20% disprover triggers a coverage/UX re-investigation BEFORE any web-AppView investment (NOT release-blocking — it is a post-release outcome metric; informational alert at day-30 < 30%). |
| **A `deny.toml` change opens a supply-chain gap** — narrowing the `axum` ban admits a transitive dep with a non-allowlisted license or a known advisory | LOW | a supply-chain regression | `cargo deny check` (licenses + bans + advisories + sources) runs on EVERY commit (I-11); the narrowed ban (§10) is reviewed against the actual `axum` dep tree (`tower`/`http`/`hyper` are all MIT/Apache-2.0); the change requires the ADR-012-amendment discipline (§10 records it as a `deny.toml` edit with rationale, not a silent unban). |

All foundation + slice-02/03/04 risks (atrium pre-1.0 churn, PDS drift,
substrate-lies, mutation slowness, supply-chain, Windows, GitHub API drift,
recursive-CTE cycle safety) remain in force and unchanged in mitigation.

## 9. Proposed ADRs

**No new ADRs at the DEVOPS layer (D-D43).** ADR-010..ADR-012 carry forward (D-D11/D-D43
note one CONSEQUENCE: the `deny.toml` ban narrowing in §10 is an application of
ADR-012's allowlist discipline to the slice-05 dependency surface, recorded as a
`deny.toml` edit with rationale — it does NOT cross the ADR threshold because
ADR-012 already governs the policy; the slice-05 DESIGN already justified `axum`
in `technology-stack.md`). Slice-05's DESIGN wave raised the five architectural
ADRs (ADR-023 self-hostable single binary, ADR-024 pull ingestion, ADR-025 index
store + anti-merging, ADR-026 production PLC decode, ADR-027 search verb/transport/
degradation) — those are DESIGN ADRs. Slice-05's DEVOPS decisions (the new release
artifact, the two contract jobs, the two network-scale guardrail gates, the
mutation-scope widening to `appview-domain`, the indexer-operator observability
surface, the `deny.toml` narrowing) are tactical extensions of existing decisions
(D-D8, D-D11, D-D12, D-D23, D-D31) — none crosses the DEVOPS-ADR threshold. Same
outcome as slice-03 (D-D21), slice-02 (D-D29), slice-04 (D-D34).

**Caveat (flagged, not blocking)**: the indexer-OPERATOR observability surface is
the FIRST genuine deviation from the "single-user CLI, no operator" model of slices
01-04. It does NOT meet the DEVOPS-ADR threshold for slice-05 (the walking skeleton
indexer is a single dogfood operator, no fleet, no SLO) — but a future hosted/
multi-tenant indexer (the ADR-023 revisit) WOULD need a DEVOPS ADR for SLOs, auth,
rate-limiting, and a real operational model. Recorded as a forward note (§Handoff).

## 10. `deny.toml` change (the FIRST since slice-01) — narrow the `axum` ban (D-D42)

The slice-01 `deny.toml` `[bans]` explicitly DENIES `axum` and `actix-web` with the
rationale: "HTTP server frameworks. OpenLore is a CLI + adapter set; we never run an
HTTP server in-process. Banning prevents drift." **That premise is slice-05-obsolete**:
the `openlore-indexer` IS a network service that serves an HTTP/XRPC query API
(ADR-027), and the DESIGN recommends `axum` (MIT, tokio-ecosystem-native;
`technology-stack.md` §NEW external dependencies).

### Decision (D-D42)

- **Narrow the `axum` ban so it does NOT apply to the `openlore-indexer` /
  `adapter-xrpc-query-server` crates** (the indexer's query server), while KEEPING
  the spirit of the ban for the CLI + the local-first adapter set (the CLI must
  never link an HTTP server — that is the I-AV-3 / KPI-5 structural guarantee,
  enforced by `xtask check-arch` `indexer_holds_no_signing_or_local_store` + the
  CLI-dep-graph exclusion, NOT by the `deny.toml` ban).
- **`actix-web` stays BANNED** (it is rejected in `technology-stack.md` as heavier
  with its own runtime; `axum` or a hand-rolled `hyper` handler is the choice). The
  ban on `actix-web` carries forward unchanged.
- **`bs58` (MIT) is confirmed in-allowlist** — the `deny.toml` `[licenses]` allow
  list already contains `MIT`, so `bs58` and `axum` (+ transitive `tower`/`http`/
  `hyper`, all MIT/Apache-2.0) pass `cargo deny check licenses` without a license
  addition. The ONLY `deny.toml` edit is the `[bans]` narrowing for `axum`.
- **If DELIVER chooses the hand-rolled `hyper` handler** (Q-DELIVER-AV-2) instead of
  `axum`, the `axum` ban narrowing is unnecessary (DELIVER may leave `axum` banned).
  `hyper` is already a transitive workspace dep (via `reqwest`) and is not banned.
  Same for `bs58` vs a hand-rolled base58btc (Q-DELIVER-AV-8) — the inline option
  needs no dep at all. The `deny.toml` edit is REQUIRED only on the `axum` path.

### Mechanism (the cargo-deny per-crate scope)

`cargo deny`'s `[bans]` `deny` list is global, so a true per-crate exception
requires either (a) removing `axum` from the `deny` list entirely (relying on the
`xtask check-arch` CLI-dep-graph exclusion to keep `axum` out of the CLI), or (b)
using cargo-deny's `[bans.deny.wrappers]`/scope features if the version in use
supports per-crate allow-scoping. **DEVOPS recommendation (D-D42)**: remove `axum`
from the `deny` list and rely on the STRUCTURAL `xtask check-arch` rule
(`indexer_holds_no_signing_or_local_store` + the CLI must-not-link-server rule,
component-boundaries §`cli`/§`xtask`) to enforce "the CLI links no HTTP server" —
this is a STRONGER, type-and-arch-level guarantee than a license-tool ban, and it
is already the I-AV-3/I-AV-5 enforcement surface. The `deny.toml` ban was always a
belt-and-suspenders for a property now enforced structurally. DELIVER lands the
edit (a `[bans]` change) with this rationale recorded inline in `deny.toml`.

This is recorded as an **Upstream Issue** (see `feature-delta.md` DEVOPS Upstream
Issues + this doc's §11) because it crosses a slice-01 supply-chain decision
(ADR-012 / the `deny.toml` ban) — it is non-blocking (the DESIGN already justified
`axum`; the narrowing is mechanical) but it MUST be applied before the indexer's
`axum`-path build is green.

## 11. Upstream Issues (flagged back; non-blocking)

1. **The slice-01 `deny.toml` `axum` ban premise is slice-05-obsolete.** The ban
   rationale ("OpenLore is a CLI; we never run an HTTP server in-process") no longer
   holds — slice-05 stands up the indexer network service (ADR-023/027). The ban must
   be narrowed (D-D42 / §10) before the `axum`-path indexer build is green. This is a
   genuine cross-slice supply-chain decision: it touches ADR-012's policy surface.
   Resolution: D-D42 narrows the ban (remove `axum` from `[bans].deny`; rely on the
   structural `xtask check-arch` CLI-must-not-link-server rule). NON-BLOCKING (DESIGN
   justified `axum` in `technology-stack.md`; the edit is mechanical) but flagged so
   the supply-chain decision is explicit, not silent. If DELIVER picks the hand-rolled
   `hyper` handler (Q-DELIVER-AV-2), no `deny.toml` edit is needed.

2. **ADR-010's telemetry-endpoint revisit trigger names "a future sibling-feature
   DEVOPS wave" — slice-05 is the candidate, but the indexer-OPERATOR observability
   surface is NOT the author-side opt-in telemetry endpoint.** Slice-04's handoff
   ("Future DEVOPS wave: slice-05 AppView or whichever sibling stands up the telemetry
   endpoint") assumed slice-05 might stand up the cohort telemetry endpoint. Slice-05
   does NOT: the indexer's `indexer.ingest.*` events are operator-side service logs
   (the operator runs the indexer and reads its own log), distinct from the author-side
   opt-in telemetry (ADR-010, off by default, no endpoint). The cohort-aggregation
   YELLOWs from slices 01-04 (KPI-3/6, KPI-FED-3/5, KPI-SCR-1/5, KPI-GRAPH-1/5/6
   cohort) REMAIN deferred — slice-05 does not resolve them. Recorded so the slice-04
   forward-expectation is corrected: slice-05 adds a service-operator log surface, NOT
   the cohort telemetry endpoint. NON-BLOCKING. (See `observability.md` delta §7, §9.)

3. **The KPI-AV-1 north-star (≥60% unfollowed-author discovery) is coupled to index
   COVERAGE, which depends on what a single self-hosted indexer ingested.** This is a
   DISCUSS-acknowledged risk (feature-delta Risks). DEVOPS mitigates with the
   index-coverage/freshness dashboard, but cannot GUARANTEE coverage at the walking
   skeleton (seed-set + relay config dependent). The KPI-AV-1 < 20% disprover is a
   coverage/UX re-investigation trigger, not a release gate. Recorded so the
   coverage↔north-star coupling is explicit to the PO at day-30. NON-BLOCKING.

## 12. References

- `docs/feature/openlore-appview-search/feature-delta.md` (WD-100..124; DESIGN + DEVOPS sections)
- `docs/feature/openlore-appview-search/discuss/outcome-kpis.md` (KPI-AV-1..6 + §Handoff to DEVOPS)
- `docs/feature/openlore-appview-search/design/architecture-design.md` (§6.3 probes, §7 deployment, §10 Earned Trust + telemetry hooks; §6.4 contract handoff)
- `docs/feature/openlore-appview-search/design/component-boundaries.md` (the DEVOPS annotation; the new crates + xtask rules; both composition roots)
- `docs/feature/openlore-appview-search/design/data-models.md` (the index schema; the indexer config; the XRPC query DTOs)
- `docs/feature/openlore-appview-search/design/technology-stack.md` (the +2 new deps; the `deny.toml` allowlist coverage)
- Sibling files in this dir: `ci-cd-pipeline.md`, `observability.md`, `kpi-instrumentation.md`, `contract-test-ownership.md`, `wave-decisions.md`
- Prior-slice DEVOPS docs (`docs/feature/openlore-{foundation,federated-read,github-scraper,scoring-graph}/devops/*.md`)
- ADR-010 (telemetry-opt-in), ADR-011 (release-matrix — gains the indexer artifact), ADR-012 (supply-chain — the `deny.toml` policy) — still in force
- ADR-023/024/025/026/027 (DESIGN-wave, slice-05) — the architectural axes; this DEVOPS doc adds no ADR (D-D43)
