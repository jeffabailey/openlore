# ADR-023: AppView Indexer = Self-Hostable Single Binary (`openlore-indexer`), Signing-Incapable by Construction

- **Status**: Proposed
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), per WD-100..WD-110 locks from Luna (nw-product-owner) for openlore-appview-search (slice-05)
- **Feature**: openlore-appview-search (slice-05)
- **Extends**: ADR-009 (hexagonal modular monolith — "lets sibling features incl. AppView plug in"), ADR-007 (functional Rust). Inherits the slice-02 `adapter-github` human-gate pattern (I-SCR-1) and the slice-03 verification discipline (ADR-016 / KPI-FED-6).
- **Resolves**: OD-AV-1 (deployment shape).

## Context

slice-05 introduces the FIRST network service in a CLI-first / local-first
project: an **AppView / indexer** that ingests PUBLIC signed claims from across
the network, verifies + indexes them, and serves network-scale discovery
("search by philosophy"). This is the headline architectural shift flagged by
WD-107: the whole product has been CLI-first + local-first, and an AppView is
inherently a network service.

DISCUSS framed the product requirement (additive, attribution-preserving,
verified, gracefully-degrading; WD-103..107) and left the deployment shape as a
DESIGN decision (OD-AV-1). The two candidate shapes:

1. **Self-hostable single binary** the user runs themselves.
2. **Hosted service** the CLI queries (a central or community-operated indexer).

The forces:

- **Data sovereignty** (the P-001 non-negotiable, the product's reason to exist):
  a hosted service is a central authority and a trust/centralization concern —
  the exact failure mode the product exists to replace (the aggregator that hides
  provenance and becomes an unaccountable authority).
- **Single-binary Rust ethos** (ADR-009): the project ships one `openlore`
  binary today; a second self-hostable binary fits the operational model.
- **Friction**: a hosted service is lower-friction for a casual user (nothing to
  run), but the walking skeleton's job is to validate the J-005 discovery thesis
  with dogfood users who already run a CLI, not to optimize onboarding.
- **The local-first guardrail (KPI-5)**: whatever the shape, offline compose/sign
  and local query must never depend on the indexer.

DESIGN owns:

1. The deployment shape (this ADR).
2. The signing/publishing capability boundary of the indexer.
3. The composition-root wiring of the new binary (ADR-009 "wire then probe then
   use", applied to a second binary).

## Decision

**The AppView indexer ships as a self-hostable single binary, `openlore-indexer`,
a new workspace member distinct from the `openlore` CLI. It is signing-incapable
by construction (it holds no `IdentityPort` signing capability and no PDS write
capability — it CANNOT author, sign, mutate, or publish a claim), mirroring the
slice-02 `adapter-github` human-gate (I-SCR-1). A hosted/community-operated
deployment is a documented future option, NOT slice-05's call.**

### Why self-hostable single binary (over hosted service)

| Factor | Self-hostable single binary — **CHOSEN** | Hosted service |
|---|---|---|
| **Data sovereignty** | The user runs their own indexer; no third party is in the trust path. Preserves the local-first ethos and the P-001 non-negotiable. | Introduces a central authority — the trust/centralization concern the product exists to avoid. A hosted aggregator that users must trust re-creates the J-005 anxiety. |
| **Single-binary Rust ethos (ADR-009)** | A second self-hostable binary in the same workspace fits the "one cargo build, ship binaries" model. | A hosted service implies an ops surface (hosting, scaling, an SLA, an operator the project must trust) the project is not resourced to own at slice-05. |
| **Walking-skeleton fit** | Validates the J-005 thesis with dogfood users who already run a CLI; the indexer is `cargo run -p openlore-indexer` away. | Optimizes onboarding for a casual user — not the slice-05 hypothesis (KPI-AV-1 is a dogfood discovery-rate hypothesis, not an adoption-funnel one). |
| **Local-first guardrail (KPI-5)** | The indexer is a SEPARATE binary; the `openlore` CLI's compose/sign/local-query paths never link or depend on it. Offline authoring is structurally unaffected. | Same guarantee achievable, but a hosted dependency tempts a future "search is the default front-door" coupling that erodes local-first. |
| **Reversibility** | A hosted deployment can be added later WITHOUT changing the user-visible contract (the CLI talks to a URL; whether that URL is `localhost` or a remote host is config — see ADR-027). | Hard to walk back a central authority once users depend on it; the sovereignty regression is sticky. |

### The signing-incapable-by-construction boundary

The indexer is a READ/discovery surface. Per WD-106 and the I-SCR-1 precedent,
it MUST be incapable of authoring, signing, mutating, or publishing a claim —
not by policy, but by construction (the human-gate at the architecture layer):

- `openlore-indexer` holds NO `IdentityPort` with signing capability. It needs
  only the VERIFICATION key-resolution surface (resolve a network author's DID
  document → public key), never a private-key / signing surface.
- `openlore-indexer` holds NO PDS WRITE capability. It reads public records via
  the existing read-only `PdsPort` surface (extended for network ingest); it
  never calls a `putRecord` / publish path.
- `openlore-indexer` holds NO `StoragePort` reference to the user's local
  `openlore.duckdb` store. It owns a SEPARATE index store (ADR-025). It cannot
  read or mutate the user's own claims.

This is enforced structurally (see Earned Trust below) the same way slice-02's
`adapter-github` is enforced to hold no storage/identity/pds reference (I-SCR-1).

### Composition root: a SECOND "wire then probe then use" root

ADR-009 made `cli::main` the only composition root. slice-05 adds a SECOND
composition root, `openlore-indexer::main`, with the same invariant:

```
fn main() -> ExitCode {            // openlore-indexer
    // 1. WIRE: construct the indexer's adapters
    let clock      = SystemClockAdapter::new();
    let index_store = IndexStoreAdapter::open(&cfg.index_path)?;   // SEPARATE index.duckdb (ADR-025)
    let ingest_src  = AtProtoIngestAdapter::new(&cfg.relay)?;      // read-only network ingest source (ADR-024)
    let did_resolve = AtProtoDidAdapter::resolve_only(&cfg)?;      // VERIFY-ONLY: pubkey resolution, NO signing (ADR-026)
    let query_api   = HttpQueryServer::bind(&cfg.listen_addr)?;    // local HTTP query surface (ADR-027)

    // 2. PROBE: every adapter must demonstrate it can honor its contract
    for outcome in [ index_store.probe(), ingest_src.probe(),
                     did_resolve.probe(), query_api.probe() ] {
        if let Err(refused) = outcome {
            emit_health_event(refused);    // health.startup.refused (same ADR-009 mechanism)
            return ExitCode::from(2);      // hard refuse to start
        }
    }

    // 3. USE: run the ingest loop + serve queries
    indexer::run(cfg, index_store, ingest_src, did_resolve, query_api)
}
```

The `openlore` CLI remains the ONLY composition root that wires the USER's
adapters (their `StoragePort`/`IdentityPort`-with-signing/`PdsPort`-with-publish).
The two roots are disjoint: the CLI never wires the index store; the indexer
never wires the user's signing identity. `xtask check-arch` enforces I-3
(composition-root rule) for BOTH binaries.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Hosted service the CLI queries (central or community-operated)** | Introduces a central authority + a trust/centralization concern the product exists to avoid; implies an ops/hosting surface the project is not resourced to own at slice-05; harder to walk back than self-hosted. A hosted deployment can be ADDED later without changing the CLI contract (ADR-027), so deferring it costs nothing. Documented as a future option. |
| **In-process indexer (a module inside the `openlore` CLI, no separate binary)** | Rejected. The ingest loop is a long-running, network-bound workload with a different lifecycle than a CLI invocation; folding it into the CLI would (a) drag network-ingest dependencies into the local-first binary, threatening the KPI-5 "offline authoring" structural guarantee, and (b) couple the discovery corpus to a single user's process. The brief already states slice-05 "adds an indexer service (separate binary)". A separate binary keeps the local-first CLI's dependency surface clean. |
| **Indexer reuses the user's `openlore.duckdb` store (one file for both local + network)** | Rejected (ADR-025). The network index is a different concern with a different lifecycle (re-buildable, larger, owned by the indexer); commingling it with the user's source-of-truth local store breaks the WD-106 "indexer never overwrites/merges a local claim" guarantee at the storage layer and the I-SCR-1-style capability separation. Separate index store. |
| **Indexer holds a full `IdentityPort` (signing-capable) for symmetry with the CLI** | Rejected. Violates WD-106 + the I-SCR-1 human-gate. The indexer needs ONLY verification-key resolution, never a private key. A signing-capable indexer could (accidentally or via compromise) author/re-publish claims — re-creating the aggregator-as-authority failure mode. Verify-only identity surface (ADR-026). |

## Consequences

### Positive

- Data sovereignty preserved: the user runs their own discovery surface; no third
  party is in the trust path. The AppView strengthens local-first rather than
  centralizing it.
- The signing-incapable-by-construction boundary makes "the indexer is a
  read/discovery surface, never an authority" (WD-106) a structural property, not
  a policy — enforced like the slice-02 human-gate (I-SCR-1).
- The two-composition-root model extends ADR-009 cleanly; both binaries follow
  "wire then probe then use"; both are covered by `xtask check-arch` / `check-probes`.
- Reversible: a hosted deployment is purely additive (the CLI talks to a
  configurable URL; ADR-027) and never changes the user-visible contract.

### Negative

- **A second binary to build, test, ship, and document.** Mitigation: same
  workspace, one `cargo build`; the release matrix (ADR-011) gains one artifact.
  The walking skeleton ships the indexer as a `cargo run`-able dogfood tool, not a
  packaged service.
- **Onboarding friction**: a casual user must run the indexer (or point at one) to
  use `search`. Mitigation: graceful degradation (ADR-027 / OD-AV-3) means `search`
  without a reachable indexer degrades to a clear local-only message; the CLI's
  core value (compose/sign/local query/federation) needs no indexer at all.
- **Index coverage depends on what a single self-hosted indexer ingested** (the
  KPI-AV-1 sparsity risk). Mitigation: the index-coverage dashboard handed to
  DEVOPS; pull ingestion seeded from the user's federation graph + a configurable
  relay (ADR-024). A shared/community indexer is the documented future scale path.

### Earned Trust

The indexer is a new binary with new adapters; per principle 12 every dependency
it does not probe is an act of faith. The composition root runs "wire then probe
then use" (above). The signing-incapable boundary is enforced at three
semantically orthogonal layers (mirroring ADR-009 / ADR-014):

| Layer | What it checks | Tool |
|---|---|---|
| **Subtype / type** | `openlore-indexer` depends on a VERIFY-ONLY identity port (DID-doc → pubkey) and a READ-ONLY ingest port; neither exposes a `sign()` / `publish()` / `put_record()` method. The type system makes signing un-callable from the indexer. | Rust trait + crate dependency graph |
| **Structural / arch** | `xtask check-arch` extends I-3: the `openlore-indexer` crate MUST NOT depend on any signing-capable adapter (`adapter-atproto-did` SIGNING surface) or any PDS-write surface, and MUST NOT reference the user's local `StoragePort`. (Mirrors the I-SCR-1 rule that `adapter-github` holds no storage/identity/pds reference.) | `xtask check-arch` (new rule `indexer_holds_no_signing_or_local_store`) |
| **Behavioral / probe** | The `openlore-indexer` composition-root probe asserts (a) the index store is the SEPARATE `index.duckdb` (not the user's `openlore.duckdb`), and (b) the identity adapter is the resolve-only variant. A startup that wired a signing identity or the local store refuses to start with `health.startup.refused{reason: indexer.capability_boundary_violated}`. | composition-root probe (ADR-009 mechanism) |

## Revisit Trigger

- A shared/community indexer becomes a JTBD (index coverage from a single
  self-hosted instance is too sparse; KPI-AV-1 < 20% disprover fires and the
  diagnosis is coverage, not UX). Add a HOSTED deployment mode — the CLI already
  talks to a configurable URL (ADR-027), so this is additive.
- The indexer needs to scale beyond a single-host single-file index store
  (ADR-025 revisit trigger). Re-evaluate the store, not the binary shape.
- A genuine multi-tenant hosted offering is pursued — at which point auth,
  rate-limiting, and an operational SLA become first-class (out of scope for the
  walking skeleton).
