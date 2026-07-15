# SPIKE-00 — Validate the Cloudflare Worker ↔ CLI mechanism (pre-slice, time-boxed)

> Pre-slice SPIKE (task type: Spike) · Blocks: slice-01 · Job: enables J-007 / J-008
> Resolves: OD-SF-1 (the headline open decision) · Time-box: ~1-2 days

## Why a spike (not a story)

This feature introduces a brand-new TS/Cloudflare-Workers stack in a new `atproto/` folder. The
single riskiest assumption (R-1/R-2) is that a signed claim's content-addressed **CID survives the
Rust-CLI ↔ Cloudflare-TS boundary** and that a Worker can store and serve an ATProto
`org.openlore.claim` record at all. Building slice-01 before validating this risks committing the
whole feature to an unworkable boundary. A time-boxed spike answers the mechanism question first.

## Learning objectives (fixed)

1. **Can a Cloudflare Worker store an `org.openlore.claim` ATProto record and serve it back?**
   (Which storage: KV / D1 / Durable Object / R2 — enough to prove the round-trip, not to finalize.)
2. **Does a claim's CID survive the boundary?** Push one signed claim from the CLI to a throwaway
   Worker, read it back, recompute the CID locally — does it byte-for-byte match? (KPI-SF-1.)
3. **What is the CLI→Worker publish shape?** Standard ATProto `putRecord`/XRPC vs a bespoke HTTP
   contract vs Rust-crates-in-WASM (the three OD-SF-1 options) — which one preserves canonicalization
   + verification with least reinvention?
4. **Does requirement 4 fall out for free?** If the Worker is a thin ATProto PDS, does a peer DID
   document resolve its PDS endpoint to the Worker so J-003 `peer pull` works unchanged (OD-SF-3)?

## Inputs

- Research seeds (user-provided): <https://blog.cloudflare.com/serverless-atproto/> and
  <https://atproto.com/guides/statusphere-tutorial>.
- Existing claim-domain CID/signing + ATProto adapters (for the canonicalization reference).
- A Cloudflare account + `wrangler` (throwaway project).

## Time-box and exit

- **~1-2 days.** Not a production build — a throwaway Worker + a scratch CLI path proving the
  round-trip.

## Definition of done (spike)

- [ ] A throwaway Worker stores one `org.openlore.claim` record and serves it back over HTTP.
- [ ] One signed claim pushed from the CLI, pulled back, recomputed CID **matches** the local CID
      (or a documented reason it does not + what canonicalization change is required).
- [ ] A recommendation for **OD-SF-1** (the boundary + storage + publish shape) with trade-offs, and a
      note on whether **OD-SF-3** (DID-doc → Worker-PDS resolution) falls out of the chosen shape.
- [ ] A go/no-go for slice-01 with the concrete transport to build.

## Out of scope

- Production storage choice, auth, multi-record bulk, the card UI, reconcile semantics, cross-instance
  pull (all downstream slices). This spike proves ONE claim round-trips; nothing more.

## Flag

**High-uncertainty — this is the pre-slice de-risk for the whole feature.** slice-01 (the walking
skeleton) does not start until this spike returns a go + a chosen transport.
