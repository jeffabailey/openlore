# Slice 02 — `openlore infer people`: auditable person→philosophy candidates

> Story: US-CPI-002 (J-004 / J-004e) · Persona: P-001 (Maria) · Depends on: slice-01
> Estimate: ~1 day

## Goal

A read-only, offline `openlore infer people [--person github:<login>]` that proposes
"person adheres to philosophy X" candidates from accumulated contribution links ∩ SIGNED,
non-retracted `embodiesPhilosophy` repo claims (the user's or a subscribed peer's), each with
provenance and a conservative confidence. Nothing is signed.

## Learning hypothesis

- **Disproves if it fails**: that signed repo claims + links produce a non-empty, credible candidate
  list on real data. If Maria's real store (2-4 signed repos) yields zero or obviously wrong
  candidates, the inference rule (OD-CPI-3 min-support, D-2 signed-only) needs rethinking before any
  signing surface is built.
- **Confirms**: provenance with per-author attribution (D-7) makes each candidate auditable at a glance.

## IN scope

- Pure inference core (ADR-007): links × signed repo philosophy claims → candidates with provenance
  (supporting repo, claim CID, claim author DID, contribution rank).
- Signed-only (D-2) and soft-retraction-aware input; already-signed (person, philosophy) pairs not
  re-proposed.
- Default confidence per OD-CPI-3 (speculative bucket) with its arithmetic shown.
- Deterministic numbering; `--person` filter; empty → "No inferred candidates", exit 0.
- Footer noting scraped repos with no signed philosophy claims.

## OUT of scope

- `--sign` (→ 03); NEW/STRONGER labels and scrape hint (→ 04); user-target scrape (→ 05).

## Acceptance criteria (production data)

- [ ] After slice-01 scrapes of `BurntSushi/ripgrep` + `rust-lang/regex`, Maria signs a
      `memory-safety` claim for ripgrep; a peer fixture (did:plc:rachel-test) signs one for regex;
      `openlore infer people` shows one BurntSushi memory-safety candidate citing BOTH claims, each
      with its own author DID.
- [ ] Scraped-but-unsigned `dtolnay/serde` contributes no candidate and is named in the footer.
- [ ] A soft-retracted supporting claim is excluded from provenance.
- [ ] Zero rows written anywhere; works with network disabled.

## Dogfood moment

Maria runs `infer people` on her real store and judges whether each candidate's "because" is credible.

## Dependencies

- slice-01 link table; `StoreReadPort` read of signed repo claims by subject (author ∪ active peers).
