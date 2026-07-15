# DESIGN Wave-Decisions — serverless-philosophy-federation

> Wave: **DESIGN** (application / component scope) · Mode: **propose** · Owner: Morgan
> (nw-solution-architect) · Date: 2026-07-15 · Primary artifact: `../feature-delta.md` (DESIGN
> sections) · ADR: **ADR-062** · Builds on SPIKE-00 (OD-SF-1 RESOLVED = opaque transport).
> ADDITIVE realization of ADR-023's deferred hosted mode; new `atproto/` TS/Workers target
> (D-2) alongside the shipped Rust workspace.

## Key Decisions (RECOMMENDED, with alternatives visible — PROPOSE mode)

| # | Open decision | RECOMMENDED | One-line why | Alternatives (override-able) |
|---|---|---|---|---|
| DDD-1 | OD-SF-1 boundary | Opaque content-addressed HTTP blob store | Only KPI-SF-1-safe shape (SPIKE-00 proved `putRecord` diverges on f16 floats) | `putRecord` (REJECTED); WASM core on Worker (REJECTED) |
| DDD-2 | Storage medium | **Durable Object** per instance | Strong read-after-write + atomic manifest; single-owner ⇒ single repo object | KV (eventual-consistency flake); R2 (large blobs later) |
| DDD-3 | Blob format | Verbatim lexicon-JSON, Rust-minted CID key | Reuses shipped `parse_signed_claim`; CBOR-free card | Raw canonical CBOR (needs new decode) |
| DDD-4 | OD-SF-2 grammar | `openlore publish {init,push,pull,status}` | Matches `peer`/`claim` group-verb style + git push/pull habit | `instance`/`mirror` noun (disambiguates `claim publish`) |
| DDD-5 | OD-SF-3 x-instance | (b) byte-preserving opaque read + local Rust verify | J-003 verify/attribution/anti-merging reused verbatim; spike's "free" holds only if byte-preserving | (a) Worker XRPC wrapper (zero-Rust-change, asymmetric Worker) |
| DDD-6 | OD-SF-4 card | Card at `/`, manual paste, no verification | Lean default; out of scope per DISCUSS | `did:web`/handle verification |
| DDD-7 | CID conformance | Leave ADR-006 as-is + revisit trigger | Opaque sidesteps it; a fix is a CID-changing wire break | Fix core to strict-float64 (expensive) |
| DDD-8 | Capability boundary | Split write `PublishPort` / read-only `InstanceReadPort` | Write must not leak into pull/card (extends ADR-023 signing-incapable) | Single port (REJECTED) |

## Architecture Summary

Additive to the ADR-009 hexagonal modular monolith. The `openlore` binary + `claim-domain` (the
SOLE canonicalizer) are REUSED UNCHANGED. A new `openlore publish {init,push,pull,status}` verb
group (sole composition root, `crates/cli`) pushes verbatim lexicon-JSON signed records — keyed by
the Rust-minted CID — to the user's OWN Cloudflare Worker (`atproto/`, TS), a dumb opaque
content-addressed store backed by a single Durable Object. Pull re-canonicalizes in Rust, recomputes
the CID, and byte-matches the key (KPI-SF-1). Cross-instance `peer pull` (J-003) resolves the peer
DID → serviceEndpoint (unchanged) and reads the same byte-preserving opaque surface, reusing J-003's
verify/attribution/anti-merging. The public card (`GET /`) renders read-only from the manifest,
signing- and write-incapable by construction. C4 L1 (System Context) + L2 (Container) in
`../feature-delta.md`; L3 not warranted.

## Reuse Analysis

- **REUSE unchanged**: `openlore` binary, `claim-domain` (CID/sign/verify — single canonicalizer),
  `adapter-atproto-did` (`resolve_peer`), `StoragePort`/`adapter-duckdb` (local reconcile).
- **REUSE (parse logic, via hoist or duplicate)**: `parse_signed_claim` is TODAY a private fn in
  `adapter-atproto-pds`; cross-crate reuse in `adapter-publish-http` requires either hoisting it
  into `claim-domain`/`lexicon` (RECOMMENDED — one shared pure decode path, Q-SF-D4) or duplicating
  the ~40-line parse. It is NOT reusable as-is across the crate boundary; DELIVER picks hoist-vs-dup.
- **EXTEND**: `crates/ports` (`PublishPort` + `InstanceReadPort` + ADTs), `crates/cli` (publish
  verbs + peer-pull transport delta), `xtask check-arch` (write-capability isolation + pure-core
  allowlist + `atproto/` no-IPLD guard), J-003 peer-pull machinery (transport delta only).
- **CREATE NEW (all challenged + justified)**: `atproto/` Worker (TS — D-2 locked target),
  `crates/adapter-publish-http` (new external protocol), `crates/publish-domain` (pure decision core
  per the subsystem norm; fold-into-`cli` alternative documented).
- Crate count: 21 → 23 production (+2) = 25 members (24 if `publish-domain` folds into `cli`); the
  `atproto/` Worker is a separate TS deployment, not a workspace member.

## Technology Stack

- **Worker**: Cloudflare workerd (Apache-2.0) + wrangler (Apache-2.0/MIT) + TypeScript (Apache-2.0)
  + Durable Objects + a minimal router (itty-router MIT or hand-rolled). **NO** `@ipld/dag-cbor` /
  `multiformats` on the CID path (SPIKE-00 invariant).
- **Rust**: workspace `reqwest` (rustls, MIT/Apache-2.0) REUSED in `adapter-publish-http` — no new
  HTTP crate; `claim-domain` (`ciborium`) reused unchanged. cargo-deny-clean throughout.

## Team capability / feasibility

The `atproto/` Worker introduces TypeScript + the Cloudflare toolchain (wrangler/workerd) into an
otherwise all-Rust workspace — the one new-skill surface. Feasibility is LOW-risk: (a) the Worker is
deliberately DUMB — ~4 routes (`PUT/GET /records/:cid`, `GET /manifest`, `GET /`), one Durable
Object, NO domain logic, NO CBOR/IPLD, NO CID computation — so the correctness surface is small;
(b) SPIKE-00 already ran the mechanism on local `wrangler dev` (workerd), so the toolchain is proven
in-repo and the maintainer has hands-on exposure; (c) wrangler/workerd are well-documented and the
research-seed URLs (Cloudflare serverless-atproto blog + Statusphere tutorial) cover the exact
pattern. Ramp is measured in days, not weeks; no external training/hiring is a blocker for slice-01.
The riskiest property (byte round-trip / KPI-SF-1) is CI-guarded, not skill-dependent.

## Constraints (honored)

- Functional-Rust core / effect-shell + ports-and-adapters (ADR-007/009); `cli` the sole Rust
  composition root. Local-first / KPI-SF-5 (publish additive; local DuckDB canonical; offline
  authoring never depends on the instance). Data-sovereignty (each user owns their instance; no
  central authority — D-1/D-3/D-4). Anti-merging + verify-before-trust carry over to cross-instance
  pull (D-8). The instance/card is signing- AND write-incapable by construction (D-7, extended).
- KPI-SF-1 (round-trip CID integrity) is protected by the single-canonicalizer invariant + a
  `0.0`/`0.5`/`1.0` regression guard (probe + CI smoke test).

## Upstream Changes

- `xtask check-arch`: new `publish_write_capability_isolated` rule + `publish-domain` pure-core
  allowlist entry + `atproto/` no-IPLD/CBOR dependency guard (DELIVER wires them).
- `crates/ports`: `PublishPort` + `InstanceReadPort` + boundary ADTs + probe-refusal reasons
  (`publish.cid_roundtrip_failed`, `publish.instance_unreachable`).
- (Recommended) hoist `parse_signed_claim` (pure JSON→SignedClaim decode) from
  `adapter-atproto-pds` into `claim-domain`/`lexicon` for a shared single parse path (Q-SF-D4).
- ADR-062 promotes the ADR-023 "hosted deployment mode: deferred" line to the ADDITIVE
  self-hosted-serverless realization (ADR-023 NOT modified; referenced).

## Quality Gates

- [x] Requirements (US-SF-001..006) traced to components; boundaries + responsibilities defined.
- [x] ADR-062 with 6 alternatives + rejection rationale + consequences + Earned Trust (3-layer) +
      revisit triggers.
- [x] Dependency-inversion / pure-core compliance (`publish-domain` pure; write/read port split).
- [x] Simplest-solution: opaque dumb store; 3 CREATE-NEW challenged; 2+ rejected alternatives per
      decision documented.
- [x] C4 L1 + L2 (Mermaid); L3 not warranted (< 5 novel components/container).
- [x] OSS-first + license documented for every tech choice; no proprietary dep.
- [x] External integration (CLI↔Worker first-party HTTP) annotated for consumer-driven contract
      test; Bluesky link is a manual paste (no API, no contract test).
- [x] Architectural enforcement tooling recommended (`xtask check-arch` rule + `atproto/` dep guard).
- [x] Peer review (solution-architect-reviewer) — **APPROVED** (iteration 1: 0 critical / 4 high /
      3 medium). All 4 high issues RESOLVED in-place without architecture change: (1) concurrent-write
      + DO strong-consistency semantics added to ADR-062 §2 (idempotency key deferred to Q-SF-D3);
      (2) operating-cost estimate added to ADR-062 §2 (personal instance within Cloudflare free tier);
      (3) TS/Cloudflare team-capability assessed above (LOW-risk: dumb Worker, spike-proven toolchain,
      days-not-weeks ramp); (4) `parse_signed_claim` reuse clarified (hoist-or-duplicate, not
      reusable as-is — Q-SF-D4). Mediums addressed: availability/latency + Bluesky social-trust notes
      added to ADR-062; C4 L1+L2 confirmed present in `../feature-delta.md`. Priority validation:
      Q1 YES (KPI-SF-1 the North-Star risk, SPIKE-00-evidenced), Q2 ADEQUATE (7 alternatives),
      Q3 CORRECT, Q4 JUSTIFIED (SPIKE-00 hard data).

## Deferred to DISTILL / DELIVER

- Q-SF-D1 manifest schema (DELIVER) · Q-SF-D2 write-auth to my instance (DELIVER, out of WS scope)
  · Q-SF-D3 bulk/resume diff strategy (DELIVER) · Q-SF-D4 hoist `parse_signed_claim` (DELIVER)
  · Q-SF-D5 opaque-instance detection marker (DISTILL) · Q-SF-D6 `publish-domain` crate vs fold
  (DELIVER).

## Handoff to DISTILL (acceptance-designer)

- Gold fixtures MUST exercise the `0.0`/`0.5`/`1.0` f16-representable confidence round-trip (the
  regression guard against the rejected re-encode transport) — a claim at each value must push→pull
  CID-identical.
- Assert: round-trip CID integrity (KPI-SF-1) on the opaque transport; additive push (no local
  mutation); idempotent + interrupted-resume push (no duplicates); pull reconcile never silently
  overwrites + surfaces conflicts; card renders only pushed claims, per-author attribution, no
  consensus row; cross-instance pull reuses J-003 verify/attribution/anti-merging; offline authoring
  unaffected (KPI-SF-5).
- External integration: the CLI↔Worker HTTP contract is the one new contract-test surface (run
  against `wrangler dev`/workerd in CI); Bluesky link is a manual paste (no contract test).
