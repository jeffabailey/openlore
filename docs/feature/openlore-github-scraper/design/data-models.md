# Data Models — openlore-github-scraper (slice-02) — DELTA from slice-01

- **Wave**: DESIGN
- **Date**: 2026-05-28
- **Architect**: Morgan
- **Authoritative for**: the `Signal` type, the `CandidateClaim` type, the
  signal->predicate mapping load shape, how a `CandidateClaim` maps into the
  slice-01 `UnsignedClaim` at sign time, the display-only `derived-from`
  provenance handling, and the `scrape` verb output format
- **Extends**: `docs/feature/openlore-foundation/design/data-models.md`
- **Non-authoritative for**: the signed `org.openlore.claim` shape (UNCHANGED
  from slice-01; NO Lexicon change in slice-02 per WD-62) and the DuckDB schema
  (UNCHANGED; no scraper tables)

## Slice-02 introduces NO persisted data model

This is the single most important data-model fact of the slice: **slice-02 adds
zero persisted types**. There is no scraper table, no Lexicon change, no new CID
path. The only NEW types (`Signal`, `CandidateClaim`, `SignalPredicateMapping`)
are **in-memory pure ADTs** that live and die within a single `scrape github`
invocation. A `CandidateClaim` becomes a persisted artifact ONLY by being
pre-filled into the slice-01 `UnsignedClaim` and carried through the slice-01
sign pipeline by the human — at which point it is byte-shape-identical to a
hand-authored claim.

| Where | Representation | Lifetime | Persisted? |
|-------|----------------|----------|------------|
| Harvested signal | `scraper_domain::Signal` value | In-memory, single invocation | NO |
| Derived candidate | `scraper_domain::CandidateClaim` value | In-memory, single invocation | NO (unless signed) |
| Signal->predicate mapping | `scraper_domain::SignalPredicateMapping` (parsed from the embedded jobs.yaml SSOT snapshot) | In-memory, built once per process | Source lives in `jobs.yaml` (product SSOT) |
| Signed-from-scraper claim | slice-01 `SignedClaim` -> `claims/<cid>.json` + DuckDB `claims` row | Permanent (slice-01 path) | YES — identical to a hand-authored claim |
| `derived-from` provenance | A display-only string in the compose preview + publish-success output | Ephemeral (display only) | NO (WD-62 / OD-SCR-3) |

## The `Signal` type

A `Signal` is one public GitHub artifact or measurable property harvested by
`adapter-github`. It carries exactly enough to (a) name itself in a candidate's
source-signal line (auditability; KPI-SCR-3) and (b) be mapped to a predicate by
the SSOT mapping.

```rust
pub struct Signal {
    pub kind: SignalKind,     // typed; matches a mapping entry
    pub value: String,        // human-readable detail shown in the candidate's source-signal line
    pub source_url: String,   // the public GitHub URL evidencing the signal -> becomes the candidate's evidence
}

pub enum SignalKind {
    DependencyManifestPinned,   // "Cargo.lock committed", "== version pins"
    DocsPresentAndSubstantial,  // "docs/ present + README 412 lines + high doc-comment density"
    TestRatioOrCiMatrix,        // "test/source ratio 0.61", "CI runs a test matrix"
    SemverAndChangelog,         // "tags follow semver + CHANGELOG present"
    MemorySafetyLanguage,       // "primary language Rust, no unsafe blocks"
}
```

Notes:

- `SignalKind` is bounded by the signal->predicate mapping (5 entries in
  slice-02). `adapter-github` need not harvest signals the mapping cannot use
  (US-SCR-001 Technical Notes).
- `value` is the human-readable detail rendered verbatim in the candidate's
  source-signal line. It is NOT canonicalized or signed — it is display + audit
  metadata.
- `source_url` is a PUBLIC GitHub URL (e.g. the Cargo.lock file URL, the docs/
  tree URL). It flows into the `CandidateClaim.evidence` and thus into the
  signed claim's `evidence[]` if the human signs.

## The `CandidateClaim` type

A `CandidateClaim` is a PROPOSAL derived purely by `scraper-domain`. It is an
in-memory ADT, never persisted as-is.

```rust
pub struct CandidateClaim {
    pub subject: String,             // github:<owner>/<repo> or github:<user> (the github_target shared artifact)
    pub predicate: String,           // the relation, e.g. "embodiesPhilosophy"
    pub object: String,              // the philosophy NSID from the mapping (org.openlore.philosophy.*)
    pub evidence: Vec<String>,       // public GitHub URL(s) from the contributing signal(s)
    pub confidence: f64,             // LOAD-BEARING: always 0.25 (mapping default); never auto-inflated (WD-52)
    pub source_signals: Vec<Signal>, // LOAD-BEARING: NON-EMPTY; names the exact signal(s) (KPI-SCR-3)
}
```

Invariants (enforced by `scraper-domain` property tests):

- `confidence == 0.25` for every candidate at proposal time; no candidate is
  proposed above 0.3 (WD-52; I-SCR-3).
- `source_signals` is NON-EMPTY (a candidate with no traceable signal is
  unauditable and breaks J-004b; I-SCR-4).
- When multiple signals map to the SAME predicate, they collapse into ONE
  candidate whose `source_signals` lists ALL contributing signals (US-SCR-002
  Example 4). The `evidence` vector then carries each contributing signal's
  `source_url`.

## The signal->predicate mapping (SSOT)

The default mapping is the `docs/product/jobs.yaml ::
J-004.signal_predicate_mapping` SSOT (WD-53). It is small (5 entries in
slice-02) and auditable by design. `scraper-domain` consumes it — it does NOT
hardcode a divergent copy.

SSOT shape (from `jobs.yaml`, reproduced here for reference; the file is the
authority):

```yaml
signal_predicate_mapping:
  - signal: "Dependency manifest pins exact versions (Cargo.lock committed, == pins)"
    predicate: org.openlore.philosophy.dependency-pinning
    default_confidence: 0.25
  - signal: "Docs directory present + README > 200 lines + doc-comment density high"
    predicate: org.openlore.philosophy.documentation-first
    default_confidence: 0.25
  - signal: "Test-to-source file ratio > 0.5 OR CI runs a test matrix"
    predicate: org.openlore.philosophy.test-driven
    default_confidence: 0.25
  - signal: "Tags follow semver + CHANGELOG present"
    predicate: org.openlore.philosophy.semantic-versioning
    default_confidence: 0.25
  - signal: "Primary language is Rust OR memory-safety language + no unsafe blocks"
    predicate: org.openlore.philosophy.memory-safety
    default_confidence: 0.25
```

Typed parse (`scraper-domain`):

```rust
pub struct MappingEntry {
    pub signal_kind: SignalKind,      // matched to a harvested Signal's kind
    pub predicate: String,            // the relation
    pub object: String,               // the philosophy NSID (the mapping's `predicate` field is the philosophy)
    pub default_confidence: f64,      // 0.25
}

pub struct SignalPredicateMapping { pub entries: Vec<MappingEntry> }
```

NOTE on the SSOT field naming: in `jobs.yaml` the entry field is named
`predicate` but its value is a philosophy NSID (`org.openlore.philosophy.*`).
In the `CandidateClaim` this becomes the `object` (the philosophy being
embodied), with the relation `predicate` defaulting to `embodiesPhilosophy`.
DELIVER preserves this mapping (the SSOT `predicate:` value -> candidate
`object`; the relation verb -> candidate `predicate`). This is documented so
the parse does not silently mis-assign fields. The `mapping_matches_ssot`
build-time test asserts the embedded snapshot equals the `jobs.yaml` SSOT byte
content (WD-53; I-SCR-5).

Loading strategy (recommended; DELIVER confirms — Q-DELIVER-1):

- Embed the `jobs.yaml` mapping snapshot at BUILD time via `include_str!` (a
  pure compile-time include — does NOT make `scraper-domain` do filesystem I/O;
  preserves the pure-core rule I-2).
- `load_mapping(embedded_yaml)` parses it with a pure YAML parser
  (`serde_yaml` or equivalent — pure, no I/O).
- The `mapping_matches_ssot` build-time/test gate fails the build if the
  embedded snapshot drifts from `docs/product/jobs.yaml`.

## How a `CandidateClaim` maps into the slice-01 `UnsignedClaim` at sign time

The bridge is `cli::CandidatePrefill` — the ONLY path from a proposal to a
signed claim (the human-gate seam; I-SCR-1). It maps candidate fields into the
slice-01 `UnsignedClaim` pre-filled compose editor:

| `CandidateClaim` field | slice-01 `UnsignedClaim` field | Pre-fill behavior |
|------------------------|--------------------------------|-------------------|
| `subject` | `subject` | Pre-filled; editable |
| `predicate` | `predicate` | Pre-filled (e.g. `embodiesPhilosophy`); editable |
| `object` | `object` | Pre-filled (the philosophy NSID); editable |
| `evidence` | `evidence` | Pre-filled (public GitHub URL(s)); editable |
| `confidence` (0.25) | `confidence` | Pre-filled 0.25; editable; range `[0.0,1.0]` enforced; **never auto-inflated** |
| `source_signals` | (none) | NOT a claim field; rendered as the display-only `derived-from` line + the audit source-signal line |
| (none) | `author_did` | From the slice-01 `IdentityPort` (the human's DID) at sign time |
| (none) | `composed_at` | From the slice-01 `ClockPort` at sign time |
| (none) | `references` | Empty (a scraped candidate is a plain claim, NOT a counter; ADR-008 / inherited) |

After pre-fill, the flow is the slice-01 pipeline UNCHANGED: compose preview
(with "not as truth", I-7) -> human edits -> Enter to sign
(`claim-domain::canonicalize` + `compute_cid` + `sign`) -> Y to publish
(`VerbClaimPublish` internals). If the human edits NOTHING, the signed claim's
fields equal the candidate's proposed values byte-for-byte, confidence 0.25
(`candidate_confidence_no_autoinflate`; US-SCR-003 Example 2).

## `derived-from` provenance handling (OD-SCR-3 — CONFIRMED display-only)

Per WD-62 (resolving OD-SCR-3 at its DISCUSS default), the `derived-from`
provenance is **display-only in slice-02**. It is NOT a field on
`org.openlore.claim`. Consequences:

- The provenance string (e.g. `derived-from: openlore-github-scraper (signal:
  Cargo.lock committed)`) appears in the compose preview and the publish-success
  output ONLY.
- It NEVER reaches `claim-domain::canonicalize`. The canonical CBOR that is
  CID'd and signed is byte-identical to a hand-authored claim with the same
  fields. **CID stability holds with zero new CID path** (I-6 / I-10; I-SCR-7).
- It NEVER alters confidence or federation behavior (the WD-58 product
  contract).
- **CID-stable-when-absent is trivially satisfied** because the field is never
  present in the payload at all. (Were a future slice to store provenance in the
  payload, it MUST be an OPTIONAL, CID-stable-when-absent field per ADR-005,
  mirroring the slice-03 `reason` treatment WD-32/ADR-015 — but that is an
  ADR-gated change, NOT slice-02.)
- NO Lexicon conformance test is needed for provenance in slice-02 (there is no
  field to conform). The slice-01 CID gold test already guarantees that a claim
  with the slice-02 field set produces the slice-01 CID — because the field set
  IS the slice-01 field set.

This is the smaller, safer change (matches the OD-SCR-3 default rationale:
avoids a Lexicon change this slice). It is reversible: a future federation need
can flip it to a stored optional field with an ADR.

## The `scrape` verb output format

Two output shapes; DELIVER fills in exact line layout (DISTILL asserts specific
lines — Q-DELIVER-7).

### Without `--sign` (harvest + propose only; nothing persisted)

```
(banner) OpenLore scraper reads ONLY public GitHub data. Nothing is signed or published.
Resolving github:rust-lang/cargo ... repo (default branch: master)
Harvested 5 public signals in 2.1s.   [auth: unauthenticated (anonymous rate limit)]

Candidate claims for subject github:rust-lang/cargo (5 derived — NOTHING is signed)

  [1] dependency-pinning     confidence 0.25 (speculative)
      from signal: Cargo.lock committed
      evidence: https://github.com/rust-lang/cargo/blob/master/Cargo.lock
  [2] documentation-first    confidence 0.25 (speculative)
      from signals: docs/ present; README 412 lines; high doc-comment density
      evidence: https://github.com/rust-lang/cargo/tree/master/src/doc
  ...

These are PROPOSALS. None is a claim until YOU sign it.
Tip: openlore scrape github rust-lang/cargo --sign 1[,3,4]
```

### With `--sign N[,N...]` (reuse slice-01 compose-sign-publish per candidate)

```
(banner) ...
Harvested 5 public signals ... [auth: authenticated (4982/5000 rate budget)]
(candidate list as above)

Composing candidate [1] (1 of 3) ...
(SLICE-01 COMPOSE PREVIEW — unchanged: "not as truth", editable fields,
 confidence 0.25, plus a display-only line:)
  derived-from: openlore-github-scraper (signal: Cargo.lock committed)
(human edits confidence -> 0.55, Enter to sign, Y to publish)
Signed + published. (1 of 3 signed)   [retract: openlore claim retract <cid>]

Composing candidate [3] (2 of 3) ...
...
(3 of 3 signed). Published 3 claims.
```

Notes:

- The "(k of M signed)" progress line is the batch-mode affordance (US-SCR-005).
- The `derived-from` line is display-only (above).
- The "not as truth" text (I-7) and the retract hint (I-8) come from the
  slice-01 pipeline UNCHANGED.

## Shared artifact <-> data model mapping (slice-02)

Per `shared-artifacts-registry.md`, the slice-02 artifacts resolve to:

| Shared artifact | Source of truth | Data-model home |
|-----------------|-----------------|-----------------|
| `github_target` | the `<target>` CLI arg, resolved by `adapter-github` | `TargetKind` (ports) -> `CandidateClaim.subject` -> slice-01 `UnsignedClaim.subject` -> signed `claims.subject`; `must_match_across [1,2,3,4]` |
| `harvested_signal` | `adapter-github` harvest of PUBLIC artifacts | `scraper_domain::Signal`; named in `CandidateClaim.source_signals` |
| `signal_predicate_mapping` | `jobs.yaml :: J-004.signal_predicate_mapping` (SSOT) | `scraper_domain::SignalPredicateMapping` (embedded snapshot; `mapping_matches_ssot`) |
| `candidate_claim` | `scraper-domain` (PURE) derivation; in-memory only | `scraper_domain::CandidateClaim`; pre-filled into slice-01 `UnsignedClaim` by `CandidatePrefill` |
| `confidence` | default 0.25 from the mapping; human-editable in step 3 | `CandidateClaim.confidence` (0.25) -> editable pre-fill -> signed `claims.confidence` (numeric only, WD-10); never auto-inflated |
| `claim_cid` | `claim-domain::compute_cid` (PURE), at sign time only | slice-01 path UNCHANGED; scraper adds NO new CID path (I-6/I-10) |
| `derived_from_provenance` | `cli` sets the display-only line | display only; NOT a signed-payload field (WD-62); never alters CID/confidence/federation |

## Validation rules — translated to data assertions

| Registry rule | Data-model assertion |
|---------------|----------------------|
| `github_target` consistent across harvest/derive/compose/sign | The resolved `TargetKind` produces the `subject` string once; `CandidateClaim.subject` and the pre-filled `UnsignedClaim.subject` are the same value; the `shared_artifact_consistency` integration test asserts byte-equality across steps 1-4. |
| every candidate names its source signal | `derive_candidates` produces `CandidateClaim.source_signals` non-empty for every candidate (property test); the renderer prints every signal's `value`. (`candidate_names_source_signal`) |
| mapping is the SSOT | embedded snapshot equals `jobs.yaml` (`mapping_matches_ssot`); `scraper-domain` never hardcodes a divergent copy. |
| candidate is never persisted unsigned | `CandidateClaim` has no serialization-to-store path; the only bridge to a `SignedClaim` is `CandidatePrefill -> VerbClaimAdd`. (`scraper_never_persists_unsigned`) |
| confidence never auto-inflates | `CandidateClaim.confidence == 0.25` (property test); pre-fill carries it verbatim; sign-time confidence equals the proposal unless the human edits. (`candidate_confidence_no_autoinflate`) |
| CID stable / provenance display-only | provenance never reaches `canonicalize`; the slice-01 CID gold test guarantees the signed-from-scraper payload's CID equals a hand-authored claim's. (I-SCR-7) |

## Confidence buckets stay UNPERSISTED (inherits WD-10 / I-6)

Slice-02 does NOT change this. The candidate's display label ("speculative")
is render-only; the persisted/signed value is the numeric `0.25` (or the
human's edited numeric value). A scraped candidate carries no bucket string
anywhere; the signed claim carries only numeric `confidence`. The slice-01
`confidence_bucket` render helper is reused for the candidate-list display.
