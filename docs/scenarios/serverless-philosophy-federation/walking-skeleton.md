# Walking Skeleton — serverless-philosophy-federation

- **Wave**: DISTILL · **Date**: 2026-07-15 · **Designer**: Quinn (nw-acceptance-designer)
- **Slice**: 01 (US-SF-001 + US-SF-002) · **Jobs**: J-007 (self-host + share) + J-008 (round-trip sync)
- **Executable SSOT**: `tests/acceptance/publish_roundtrip.rs` → `WS-1`
  (`publish_round_trips_one_signed_claim_through_my_own_instance_with_identical_cid`)
- **Status**: **IMPLEMENTED** — green since DELIVER 01-01 (`b71fa7b`, 2026-09-26); the live
  `wrangler dev` round-trip runs in `.github/workflows/publish-contract.yml` (01-02, `acdd6ee`).
  (At DISTILL handoff it was RED via `todo!()`, not `#[ignore]`.)
- **Migrated**: 2026-09-27 from `docs/feature/serverless-philosophy-federation/distill/walking-skeleton.md`

## The one scenario

```gherkin
@walking_skeleton @driving_port @real-io @us-sf-001 @us-sf-002 @j-007 @j-008 @kpi-sf-1
Scenario: I round-trip one signed claim through my own instance with an identical CID
  Given I have deployed and registered my own serverless instance
  And I have one signed claim in my local store
  When I push it to my instance and pull it back
  Then my instance stores the claim and recomputes the identical content address
  And the pulled-back claim has that same content address
  And I am told the claim verified 1 of 1
  And my local claim is unchanged
```

## Why this is the walking skeleton (litmus test — Mandate 5)

1. **Title describes a user goal**, not a technical flow: "I round-trip one signed claim
   through my own instance with an identical CID" — a sovereign publisher proving her
   instance is a faithful mirror she owns.
2. **Given/When describe user actions**: deploy + register (`publish init`), have a signed
   claim, push then pull. Not "system state setup".
3. **Then describes user observations**: the claim is stored and verifies 1/1, the CID is
   identical, the local claim is untouched. Not internal side effects.
4. **A non-technical stakeholder confirms "yes, that is what users need"**: "I can put my
   reasoning on a surface I own and prove the copy is faithful, without a middleman."

## Why this slice carries the riskiest assumption

Per DISCUSS Priority Rationale + SPIKE-00 + ADR-062: the load-bearing risk is whether a
signed claim's content-addressed CID survives the Rust-CLI ↔ Cloudflare-Worker boundary
(KPI-SF-1, the North Star). If the round-trip does not hold, the mirror thesis (J-008) and
everything the card renders (J-007) collapse — every later slice is moot. SPIKE-00 proved
the CID survives IFF the Worker is an OPAQUE byte store (never re-encodes); this walking
skeleton is that proof made executable at the CLI contract layer.

## End-to-end path (what the scenario exercises)

```
openlore publish init <url>        REAL CLI  →  FakeInstance (opaque HTTP double: GET /manifest marker)
   ↓ registers target (ADR-027 configurable URL, D-5)
one signed claim in local DuckDB   REAL claim-domain (SOLE canonicalizer) + REAL adapter-duckdb
   ↓
openlore publish push              REAL CLI  →  PUT /records/:cid (verbatim bytes, FakeInstance)
   ↓
openlore publish pull              REAL CLI  ←  GET /records/:cid + GET /manifest
   ↓ REAL claim-domain re-parse + re-canonicalize + recompute CID + byte-match key
"1/1 verified" + identical CID + local untouched
```

REAL: `openlore` CLI, `claim-domain`, `lexicon`, `adapter-duckdb`, filesystem.
FAKE (external boundary only): `FakeInstance` — the CLI↔Worker opaque seam.

## Relationship to the DV-1 CI contract test (do NOT duplicate)

WS-1 asserts the observable CLI contract against the `FakeInstance` double (fast,
hermetic, hexagonal). The LIVE `wrangler dev`/workerd round-trip — the same
init→push→pull asserting CID match, INCLUDING the 0.0/0.5/1.0 float regression guard — is
the DEVOPS CI contract test `publish-contract.yml` (DV-1). WS-1 is the acceptance shape;
`publish-contract.yml` is the real-transport proof. The gold-fixture float guard also has
an acceptance-layer mirror at `publish_roundtrip.rs` RT-4.

## RED-ready state (historical — at DISTILL handoff)

> IMPLEMENTED: the `todo!()` scaffold was replaced by the real scenario body in DELIVER 01-01.

WS-1 body is `todo!("DELIVER (... WALKING SKELETON): ...")` under a `// SCAFFOLD: true`
module marker. It is NOT `#[ignore]`, so it runs and fails (RED — panic at `todo!()`),
which is exactly correct at the DISTILL→DELIVER boundary: DISTILL ships RED, DELIVER makes
green. It is a genuine failing test, not a stub that passes.
