# SPIKE-00 findings — Cloudflare Worker ↔ CLI CID round-trip

- **Feature**: serverless-philosophy-federation · **Resolves**: OD-SF-1 · **Blocks**: slice-01
- **Date**: 2026-07-14 · **Runtime**: `npx wrangler dev` (workerd), local, no deploy, no Cloudflare account, no API token
- **Probe code**: `/tmp/spike_serverless_philosophy_federation/` — **DISCARDED 2026-07-14** (promotion gate: DISCARD → DESIGN; findings captured here)

## Assumption tested (KPI-SF-1)
Does a signed `org.openlore.claim`'s content-addressed CID survive a round-trip through a Cloudflare Worker?

## BINARY VERDICT — **WORKS**, conditionally
The CID survives **iff the Worker is an opaque, CID-addressed byte store** (stores and returns the record bytes verbatim, never re-encodes). If the Worker re-canonicalizes the record (the ATProto server-assigned-CID `putRecord` model), the **CID does NOT survive**. That distinction is the central finding and decides OD-SF-1.

## Evidence
- Used the 5 gold fixtures (`tests/fixtures/gold_cids/claim_00{1..5}`, same pairs `lexicon_conformance.rs` asserts): unicode/emoji, multi-`references`, empty-evidence + confidence `0.0`, 4-URL evidence + `0.99`.
- Recomputed canonical CBOR from the **real Rust core** (throwaway cargo bin path-dep'ing `crates/claim-domain`, calling `canonicalize()` + `compute_cid()`); printed CIDs matched the frozen `.cid` files exactly.
- **Opaque model** (`PUT/GET /records/:cid`, raw bytes): all 5 round-tripped **byte-identical**; CID recomputed from pulled bytes **matched gold 5/5**. KPI-SF-1 met.
- **Re-encode model** (`POST /xrpc/com.atproto.repo.putRecord`, JS `@ipld/dag-cbor`): CID matched **4/5, diverged on claim_004** (confidence `0.0`). Live: gold `bafyrei…thxny` → PDS `bafyrei…rsvq`.

## Root cause of the divergence (confirmed against the real core) — HIGH VALUE
openlore's Rust `claim-domain` (`ciborium`) emits RFC 8949 **shortest-form floats** — `0.0→f90000`, `0.5→f93800`, `1.0→f93c00` (float16). **IPLD DAG-CBOR mandates strict float64** (and encodes whole floats as integers). So openlore's canonical bytes advertise codec `0x71` (dag-cbor) but are **not DAG-CBOR-conformant for shortest-float values**.

Consequence: **any "round" confidence a user types (0.0 / 0.25 / 0.5 / 0.75 / 1.0 …) makes a standard JS `@ipld/dag-cbor` PDS compute a DIFFERENT CID than the Rust core.** The 4 matching fixtures matched only by luck (their decimals aren't f16-representable, so both sides used float64).

| confidence | Rust ciborium | `@ipld/dag-cbor` |
|---|---|---|
| `0.0` | `f9 0000` (f16) | integer `0` → diverges |
| `0.5` | `f9 3800` (f16) | float64 → diverges |
| `1.0` | `f9 3c00` (f16) | integer `1` → diverges |
| `0.86` | `fb …` (f64) | float64 → matches |

This is a latent ADR-006 conformance gap. The opaque-transport design **sidesteps** it (the Worker never computes CIDs), but DESIGN should decide whether to ALSO fix the core's dag-cbor conformance (separate concern).

## OD-SF-1 recommendation — **opaque, content-addressed HTTP blob store** (option b)
- **(a) ATProto `putRecord`, server-assigned CID — REJECT**: proven to diverge for f16-representable confidence; would require reimplementing ciborium shortest-float encoding in the Worker — a permanent cross-language federation hazard.
- **(b) Opaque CID-addressed blob store — ADOPT**: perfect byte fidelity; CID survives unconditionally; the Worker needs zero Lexicon/CBOR knowledge; the pull side recomputes CID + verifies signature locally via `claim-domain` (trust anchor stays in Rust). Trade-off: gives up ATProto `putRecord` wire-compat + server-side field indexing, in exchange for absolute CID stability and a trivial, language-agnostic Worker — correct when the CID *is* the contract and must be minted by exactly one canonicalizer.
- **(c) Full standard PDS — not warranted** for slice-01 (inherits (a)'s trap).

## OD-SF-3 (reasoned, not built)
A peer DID document resolving its PDS `serviceEndpoint` to such a Worker lets J-003 `peer pull` work **unchanged, iff the transport is byte-preserving** (option b): pull fetches bytes by CID and verifies locally. A re-encoding peer PDS (option a) would fail pull-side CID verification — another reason to standardize on the opaque transport.

## Go / No-Go for slice-01 — **GO**
Build an **opaque, content-addressed HTTP blob transport**: `PUT /records/:cid` stores raw openlore record bytes verbatim (KV/DO/R2, DESIGN's choice) keyed by the CLI-computed CID; `GET /records/:cid` returns those exact bytes. **Invariants**: the CID is minted only by `claim-domain::compute_cid`; the Worker never canonicalizes/re-encodes; no JS IPLD library derives CIDs; the pull side recomputes CID + verifies signature locally. Add a **regression guard** for the shortest-float divergence (a claim at confidence `0.0`/`0.5`/`1.0` crossing the boundary) so option (a) can't creep back in.

## Design implications handed to DESIGN
1. Adopt option (b) opaque transport; the Worker is a dumb content-addressed store.
2. Storage medium (KV vs Durable Object vs R2) is DESIGN's; the transport contract is medium-agnostic.
3. Decide separately whether to fix the core's ciborium-vs-DAG-CBOR float nonconformance (ADR-006) — the opaque transport does not require it, but a future standard-PDS interop would.
4. New ADR recommended: record option (b) as the additive, self-hosted-serverless realization of ADR-023's deferred hosted mode.
