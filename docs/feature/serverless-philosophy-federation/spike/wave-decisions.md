# SPIKE Decisions — serverless-philosophy-federation

## Assumption Tested
Does a signed `org.openlore.claim`'s content-addressed CID survive a round-trip through a Cloudflare Worker? (OD-SF-1 / KPI-SF-1)

## Probe Verdict
**WORKS (conditionally)** — the CID survives **iff** the Worker is an opaque, CID-addressed byte store (stores/returns record bytes verbatim, never re-encodes). A re-canonicalizing ATProto server-assigned-CID `putRecord` model does NOT preserve the CID. Validated against the real `crates/claim-domain` with the 5 gold fixtures on local `wrangler dev` (workerd); opaque model matched 5/5 byte-identical.

## Promotion Decision
**DISCARD → DESIGN** (user, 2026-07-14). Rationale: the mechanism question is decisively answered and the finding is rich, so the probe's value is captured in `findings.md`. The finding materially shapes the architecture (opaque transport, not ATProto `putRecord`) AND surfaces a cross-cutting concern (the ciborium-vs-DAG-CBOR shortest-float CID nonconformance) that DESIGN should reckon with — and storage medium (KV/DO/R2) is a DESIGN choice — BEFORE any `atproto/` code is committed. Building a walking skeleton now would bake in decisions DESIGN should own. Probe code (`/tmp/spike_serverless_philosophy_federation/`) deleted.

## Walking Skeleton
Not built (DISCARD). slice-01 (the opaque-transport walking skeleton) is built in DELIVER after DESIGN, per the go-for-slice-01 in `findings.md`.

## Design Implications (handed to DESIGN)
1. **Adopt OD-SF-1 option (b): opaque content-addressed HTTP blob transport.** `PUT /records/:cid` stores raw openlore record bytes verbatim; `GET /records/:cid` returns them. The CID is minted only by `claim-domain::compute_cid`; the Worker never canonicalizes/re-encodes; no JS IPLD library derives CIDs; the pull side recomputes CID + verifies signature locally in Rust.
2. **Storage medium (KV vs Durable Object vs R2) is DESIGN's** — the transport contract is medium-agnostic.
3. **Decide separately whether to fix the core's ciborium-vs-DAG-CBOR float nonconformance (ADR-006).** The opaque transport does NOT require it, but a future standard-PDS interop would. This is a latent conformance gap, not introduced by this feature.
4. **OD-SF-3 falls out for free** under the opaque transport: a peer DID resolving its PDS endpoint to such a Worker lets J-003 `peer pull` work unchanged (byte-preserving pull + local verify).
5. **Write a NEW ADR** recording option (b) as the additive, self-hosted-serverless realization of ADR-023's deferred hosted mode (ADR-023 flagged in DISCUSS Changed Assumptions, not modified).

## Constraints Discovered
- The CID contract requires **exactly one canonicalizer** (the Rust core). Any second CID computer (a JS/Worker IPLD lib) is a divergence hazard for f16-representable confidence values (`0.0`, `0.25`, `0.5`, `0.75`, `1.0`, …).
- A regression guard is warranted: a claim at confidence `0.0`/`0.5`/`1.0` crossing the boundary must round-trip CID-identical, so the re-encoding transport can't creep back in.
