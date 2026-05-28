# ADR-018: Candidate-Claim Model + Signal->Predicate Mapping Contract + Display-Only Provenance

- **Status**: Accepted
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), per WD-52/WD-53/WD-58 locks from Luna (nw-product-owner) for openlore-github-scraper
- **Feature**: openlore-github-scraper (slice-02)
- **Extends**: ADR-005 (Lexicon namespace + forward-compat) + ADR-007 (functional Rust pure core). Both remain in force.

## Context

slice-02's load-bearing job (J-004b) is deriving auditable candidate claims from
harvested GitHub signals. DISCUSS locked three contracts:

- **WD-52**: candidate confidence defaults to 0.25 (speculative); only the human
  raises it; numeric-only persistence (inherited I-6).
- **WD-53**: a small, auditable signal->predicate mapping is the
  `jobs.yaml :: J-004.signal_predicate_mapping` SSOT; every candidate names its
  source signal; NO ML inference; `scraper-domain` consumes the SSOT, never a
  divergent hardcode.
- **WD-58 / OD-SCR-3**: `derived-from` provenance is informational and MUST NOT
  alter confidence or federation; whether it lives in the signed payload or
  stays display-only is a DESIGN call.

DESIGN owns: the `Signal` + `CandidateClaim` ADT shapes, the mapping load
strategy (embed-at-build vs read-at-runtime), and the provenance storage choice.

## Decision

### 1. The candidate-claim model is PURE and in-memory-only

`Signal` and `CandidateClaim` are pure ADTs in `scraper-domain`. `derive_candidates(signals, mapping) -> Vec<CandidateClaim>` is a pure function. A `CandidateClaim` is NEVER persisted as-is; it becomes a claim ONLY by being pre-filled into the slice-01 `UnsignedClaim` and signed by the human. (See `data-models.md` for the exact field shapes.)

Load-bearing invariants (enforced by `scraper-domain` property tests):

- `CandidateClaim.confidence == 0.25` for every candidate (the mapping default);
  no candidate proposed above 0.3; never auto-inflated (WD-52; I-SCR-3).
- `CandidateClaim.source_signals` is NON-EMPTY (auditability; WD-53; I-SCR-4).
- Multiple signals mapping to one predicate collapse into ONE candidate listing
  all contributing signals (US-SCR-002 Example 4).

### 2. The signal->predicate mapping is the `jobs.yaml` SSOT, embedded at build time

- The mapping is `docs/product/jobs.yaml :: J-004.signal_predicate_mapping`
  (5 entries in slice-02). `scraper-domain` consumes it; it does NOT hardcode a
  divergent copy (WD-53).
- Load strategy: EMBED the YAML snapshot at BUILD time (`include_str!`) and
  parse it with a pure YAML parser. This keeps `scraper-domain` PURE (no
  filesystem I/O at runtime; I-2 holds).
- A `mapping_matches_ssot` build-time/test gate asserts the embedded snapshot
  equals the `jobs.yaml` SSOT — failing the build on drift (WD-67; I-SCR-5).
- An xtask-codegen-from-`jobs.yaml` Rust table is an acceptable DELIVER
  alternative (SSOT still `jobs.yaml`).
- NO ML / inference (WD-53). The derivation is deterministic over the static
  mapping — which is what makes every candidate auditable and rejectable.

### 3. `derived-from` provenance is DISPLAY-ONLY in slice-02

- The provenance line (e.g. `derived-from: openlore-github-scraper (signal:
  Cargo.lock committed)`) appears in the compose preview + publish-success
  output ONLY. It is NOT a field on `org.openlore.claim`.
- It NEVER reaches `claim-domain::canonicalize`. The canonical CBOR that is
  CID'd and signed is byte-identical to a hand-authored claim with the same
  fields. **CID stability holds with ZERO new CID path** (I-6/I-10; I-SCR-7).
- CID-stable-when-absent (the WD-58 forward-compat concern) is TRIVIALLY
  satisfied because the field is never present in the payload at all.
- This is the smaller, safer change (matches the OD-SCR-3 default rationale:
  avoids a Lexicon change this slice). It is reversible: a future federation
  need can flip it to a stored OPTIONAL, CID-stable-when-absent field per ADR-005
  (mirroring the slice-03 `reason` field, WD-32/ADR-015) via a NEW ADR.

## Alternatives Considered

| Option | Rejection rationale |
|--------|---------------------|
| **Store `derived-from` as an optional signed-payload field on `org.openlore.claim` (mirroring slice-03 `reason`)** | Considered and explicitly available (the slice-03 precedent shows how to do it CID-stably). REJECTED for slice-02 because (a) it requires a Lexicon change + a conformance test for marginal slice-02 value, (b) the product contract (WD-58) only requires that a reader CAN see the origin — display-only satisfies that at the authoring surface — and (c) display-only is reversible whereas a wire field is a forward-compat commitment. Re-open in a federation slice if peers need to see scraper-origin on PULLED claims. |
| **ML / embedding-based predicate inference instead of a static mapping** | Locked rejected by WD-53. ML inference would make candidates UNAUDITABLE (a candidate could not name "the exact signal that produced it"), breaking J-004b and the no-surveillance/auditability promises. The static mapping is small, human-editable, and trivially testable. |
| **Hardcode the mapping as a Rust literal in `scraper-domain`** | Risks divergence from the `jobs.yaml` SSOT (WD-53). Embedding the YAML snapshot + `mapping_matches_ssot` keeps a single authoritative format. (Codegen from `jobs.yaml` is the acceptable middle ground.) |
| **Read the mapping from `jobs.yaml` at runtime** | Would add a filesystem read to `scraper-domain`, violating the pure-core rule (I-2). Embed-at-build avoids this. |
| **Auto-inflate confidence when multiple signals agree** | Locked rejected by WD-52. Confidence is a constant 0.25 until the human edits it; any "boost" heuristic would make the tool over-assert and defeat KPI-SCR-5 (the edit rate proves the human-in-the-loop is real). |
| **Collapse-to-highest-confidence when multiple signals map to one predicate** | Rejected — all candidates are 0.25, so there is nothing to maximize; the design collapses to ONE candidate that LISTS all contributing signals (preserving auditability), not one that picks a "best" signal. |

## Consequences

### Positive

- `scraper-domain` is trivially unit + mutation testable (pure function over
  pure inputs).
- Auditability is structural: a candidate cannot exist without naming its source
  signal(s).
- CID stability is preserved with zero new CID path and zero Lexicon change —
  the smallest possible footprint.
- The mapping is a single SSOT (`jobs.yaml`); product can revise it without
  touching code semantics; KPI-SCR-5 surfaces whether users disagree with it.

### Negative

- Display-only provenance means a PULLED claim (read via slice-03 federation)
  does NOT show its scraper origin to a remote reader. **Mitigation**: the
  product contract (WD-58) only requires the AUTHOR can see the origin at
  authoring time; if remote-visible provenance becomes a need, an ADR flips it
  to a wire field (reversible by design).
- The embedded-snapshot approach requires a `mapping_matches_ssot` gate to
  prevent drift. **Mitigation**: the gate is a cheap build-time string compare;
  it fails CI on drift.

## Earned Trust

- `scraper-domain` property tests (the pure-crate equivalent of a probe):
  determinism; non-empty `source_signals`; confidence == 0.25; collapse
  behavior; empty-input -> empty-output.
- `mapping_matches_ssot` build-time gate: the embedded mapping equals the
  `jobs.yaml` SSOT (catches a divergent hardcode — the WD-53 lie).
- The slice-01 CID gold test (unchanged) guarantees that a signed-from-scraper
  claim's CID equals a hand-authored claim's for identical fields — empirically
  demonstrating that display-only provenance does not perturb the signed payload
  (I-SCR-7). This is the Earned-Trust answer to "what if provenance leaks into
  the payload?": the gold test would fail.

## Revisit Trigger

- A federation slice needs scraper-origin visible on PULLED claims — flip
  provenance to an optional, CID-stable-when-absent wire field via a new ADR.
- KPI-SCR-5 edit-rate < 20% — investigate whether candidates are over-confident
  (despite the 0.25 default) or the mapping is too aggressive; revise `jobs.yaml`.
- The mapping grows beyond ~10 entries or needs per-language variants — consider
  a richer mapping schema (still auditable, still SSOT).
