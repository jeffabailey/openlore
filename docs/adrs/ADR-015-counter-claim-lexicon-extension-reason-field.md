# ADR-015: Counter-Claim Lexicon Extension — Optional `reason` Field on `org.openlore.claim`

- **Status**: Accepted
- **Date**: 2026-05-27
- **Deciders**: Morgan (nw-solution-architect), per WD-20/WD-23 locks from Luna (nw-product-owner) for openlore-federated-read
- **Feature**: openlore-federated-read (slice-03)
- **Extends**: ADR-005 (Lexicon namespace) and ADR-008 (Retraction = counter-claim referencing original CID). The `org.openlore.claim` NSID is unchanged; the schema gains ONE optional field. No new NSID is introduced. The existing `ReferenceType.Counters` variant from ADR-008 is reused without modification.

## Context

slice-03 introduces the `claim counter <target_cid> --reason "..."` verb
(ADR-013). The `--reason` CLI argument is the user's articulated
disagreement; it must be:

- **Federated**: visible to peers who pull the counter-claim — without
  this, the disagreement isn't actually public.
- **Signed**: part of the claim's canonical payload — so any reader can
  verify the reason text is byte-equal to what the author signed.
- **Forward-compatible with slice-01 readers**: a slice-01-era binary
  ingesting a slice-03 counter-claim MUST NOT crash or reject; per ADR-005,
  unknown optional fields are ignored gracefully.
- **Bounded**: length 1..=1000 chars per WD-20. Empty or absent is a sign
  of failure (counter-claims without `--reason` are forbidden by the CLI;
  the Lexicon schema permits absence on non-counter claims).

DISCUSS locked the constraints (WD-20, WD-23). DESIGN owns:

1. WHERE in the `org.openlore.claim` Lexicon the field lives (top-level
   property vs nested in references[] entry).
2. The Lexicon JSON declaration (type, constraints, description).
3. The serde representation in the `lexicon` crate.
4. The validation rule placement (Lexicon-level vs verb-level vs both).
5. The interaction with canonicalization (CBOR field ordering; CID
   stability when `reason` is absent vs present).

## Decision

**Add `reason` as a TOP-LEVEL OPTIONAL property on the `org.openlore.claim`
Lexicon record, semantically applicable to any claim but enforced as
REQUIRED only at the `claim counter` verb layer.**

### Lexicon JSON change

Append to `lexicons/org/openlore/claim.json` `defs.main.record.properties`:

```json
"reason": {
  "type": "string",
  "minLength": 1,
  "maxLength": 1000,
  "description": "Optional free-text explanation. REQUIRED by the `claim counter` verb (CLI-level enforcement); permitted but unused on other claim types. UTF-8; NFC-normalized at compose time. When present, byte-stable across the federation round-trip per ADR-006 canonicalization."
}
```

NOT added to `required[]` — the field stays optional at the wire level.

### Why top-level, not nested in `references[]` entry

| Option | Pro | Con | Verdict |
|---|---|---|---|
| **Top-level `reason` (chosen)** | Single field; simple serde; obvious in graph query output; one place to validate length; cleanly omitted by serde when None | Semantically applies only to `claim counter`; non-counter claims may have a `reason` set (validation is verb-level, not schema-level) | **Chosen** |
| Nested in `references[].reason` | Strictly tied to a single reference; scales if future variants want per-reference reasons (e.g., a claim with multiple `references[]` entries each with their own reason) | Lexicon serde changes for an existing array; each reference entry grows by an optional field; renderer must walk references[] to find the reason; future `corrects` and `supersedes` sugar verbs would also need it, multiplying complexity | Rejected |
| New `reason_blocks` array at top level | Most general; permits multi-part reasons | Premature; YAGNI; no scenario today needs more than one block | Rejected |

The top-level placement also matches the user mental model: "I disagree
with this claim, here is why." The reason is about the claim itself, not
about a specific reference link.

### Why optional at the wire level, REQUIRED at the verb level

The Lexicon schema is the FEDERATION contract. It must be forward-
compatible with slice-01 readers (per ADR-005 invariant — fields added
must be optional). Therefore `reason` is optional on the wire.

The verb-level enforcement (`openlore claim counter` requires `--reason`)
is the JTBD contract per WD-20: silent counter-claims are forbidden
because the reason IS the disagreement artifact. The CLI rejects pre-
compose if `--reason` is missing or empty.

This produces an asymmetry: a peer's PDS COULD theoretically contain an
`org.openlore.claim` with `references[].type == counters` and NO `reason`
field (e.g., authored by a non-OpenLore client, or a future client that
abandons the verb-level enforcement). When the local user pulls such a
record, the slice-03 binary stores it as-is in `peer_claims` (it satisfies
the Lexicon schema); the federated query renders it with `reason: (empty)`
or omits the reason line entirely. This is the correct behavior — the
federation contract is the Lexicon schema, not the CLI verb.

### Serde representation in `lexicon` crate

```rust
// crates/lexicon/src/claim.rs — adds one field; slice-01 fields unchanged.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub subject:     String,
    pub predicate:   String,
    pub object:      String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence:    Vec<String>,
    pub confidence:  f64,
    pub author:      String,
    pub composed_at: String,        // RFC3339 UTC
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub references:  Vec<ClaimReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason:      Option<String>,    // NEW in slice-03; optional at wire level
    pub signature:   SignatureBlock,
}
```

`skip_serializing_if = "Option::is_none"` is critical: a claim with no
reason MUST serialize to the byte-identical CBOR a slice-01 binary would
produce. This preserves CID stability for non-counter claims across the
slice-01 -> slice-03 upgrade.

### Canonicalization (CBOR field ordering)

Per ADR-006, canonical CBOR field order is **lexicographic by key bytes**
(RFC 8949 §4.2.1 deterministic encoding). The new `reason` field slots
into the canonical sort between `references` and `signature` — which
matters ONLY for claims that CARRY the field; absent fields are not
serialized.

**CID stability invariant**: a slice-01 claim's CID is unchanged by the
addition of this field. A claim with `reason: None` produces the same
canonical CBOR a slice-01 binary would have produced for the same content.
Property test in `claim-domain` enforces this:

```
property: forall non_counter_claim C:
    cid_slice_01(C) == cid_slice_03(C with reason: None)
```

The slice-03 `compute_cid` implementation already satisfies this because
the field is skipped when None (serde `skip_serializing_if`).

### Validation placement (three layers)

| Layer | What it checks | When |
|---|---|---|
| **Lexicon schema** | When present, length 1..=1000 chars; UTF-8 valid; not whitespace-only (the `minLength: 1` rule rejects empty string) | Inbound parse (peer pull) + outbound serialize (own publish) |
| **CLI verb** (`claim counter`) | `--reason` CLI flag MUST be present and non-empty; rejected pre-compose with "counter-claims require --reason" | Pre-compose, before any disk write or network call |
| **`claim-domain` helper** `validate_counter_claim(claim) -> Result<()>` | If `references[]` contains any `Counters` entry AND `reason` is None, reject with `ClaimError::CounterReasonMissing` | Invoked by `VerbClaimCounter` after constructing the unsigned claim, before canonicalization |

The third layer is the BELT-AND-BRACES check: even if the CLI verb's
flag-parse misses `--reason`, the domain layer rejects the claim before
it can be signed. This is the same defense-in-depth pattern as
`reference_rules_validate` in slice-01 (CLI rejects self-reference; domain
layer rejects self-reference; both catch the bug).

### UTF-8 normalization

`--reason` is normalized to **NFC** (Unicode Normalization Form C) at
compose time, BEFORE canonicalization. This is necessary because:

- The byte form of the reason determines the CID.
- Two visually-identical strings with different normalization forms
  would produce different CIDs.
- NFC is the de-facto interchange form (W3C, IETF). NFD or unnormalized
  text breaks copy-paste and grep workflows the persona depends on.

Normalization is a pure function in `claim-domain::normalize_reason(s: &str)
-> String` and is invoked once at compose time. The normalized string is
what gets persisted in the signed payload AND displayed in the compose
preview (so what the user sees is what gets signed).

Property test in `claim-domain`:

```
property: forall string S:
    normalize_reason(S) == normalize_reason(normalize_reason(S))  // idempotent
property: forall string S, T where S != T and NFC(S) == NFC(T):
    normalize_reason(S) == normalize_reason(T)                    // unifying
```

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **New NSID `org.openlore.counter-claim`** | Rejected per ADR-008 precedent: counter-claims, retractions, corrections, and supersessions all use the SAME Lexicon (`org.openlore.claim`) with a typed `references[]` entry. Introducing a new NSID for counter would split the publish pipeline, break the WD-22 single-publish-path invariant, and require slice-01 readers to add a second NSID handler. |
| **New `ReferenceType.CountersWithReason` enum variant** | Rejected per WD-23: the `Counters` variant from ADR-008 is reused unchanged. Adding a parallel variant would force every reader to handle both, double the test matrix, and create ambiguity about which variant a peer "should" use. |
| **`reason` as `Vec<String>` (multi-part)** | YAGNI; no scenario today needs multi-block reasons. If a slice-04+ use case emerges, ADR-amend with `reason_blocks: Vec<ReasonBlock>` as a separate optional field; the single-string `reason` remains for backward compat. |
| **No length limit** | Rejected per WD-20: 1..=1000 chars. Unbounded text invites essays in the wrong field; the user's hint at length > 1000 is "publish a separate evidentiary claim and reference it" — same advice the JTBD analysis surfaced. |
| **Validate length at the Lexicon schema ONLY (not in the verb layer)** | Insufficient: Lexicon validation runs at parse/serialize boundaries; CLI verb rejection must happen BEFORE the compose preview is rendered, otherwise the user sees a preview with bad data only to have it rejected at sign time. Layered validation per the table above. |
| **Markdown formatting for reason** | Rejected: introduces a rendering ambiguity (which Markdown? CommonMark? GFM?); CLI output is plain text. The reason is verbatim — what the user types is what gets signed and displayed. The 78-col wrap at display time is a render-only concern (data is unwrapped on disk). |

## Consequences

### Positive

- Forward-compatible: slice-01 binaries ingest slice-03 counter-claims and
  ignore the unknown `reason` field per serde's default behavior — no wire
  break, no graceful-degradation special case needed (the slice-01 binary
  would render the claim without reason context, but that's the worst-case
  degradation).
- The Lexicon stays ONE schema for the entire reference-types family
  (`retracts | corrects | counters | supersedes`); future sugar verbs can
  add their own optional context fields without further NSID proliferation.
- CID stability for slice-01 claims is preserved by construction (the
  field serializes only when present).
- Three-layer validation (Lexicon / verb / domain) gives DELIVER
  belt-and-braces: a regression in any one layer is caught by another.
- NFC normalization is honest about the byte-determinism of CIDs and
  prevents copy-paste confusion (a reason copied from a richer text source
  with non-NFC characters produces the same CID as the same reason typed
  fresh).

### Negative

- The asymmetry "optional at wire / required at verb" is subtle and must
  be documented for future contributors. **Mitigation**: this ADR is the
  documentation; a comment block in `lexicon/src/claim.rs` cites this ADR
  by number at the field definition site.
- A non-OpenLore client publishing under `org.openlore.claim` could
  produce a counter-claim with no reason; the local user's federated query
  would render it with a blank/missing reason line. This is acceptable —
  the verb-level enforcement protects the OpenLore-internal workflow; the
  Lexicon-level openness preserves federation interop.
- NFC normalization is a small performance cost at compose time (~1µs per
  reason in practice). Negligible.

### Earned Trust

The `lexicon` crate's existing `probe()` (per ADR-005) MUST extend to:

1. Load the slice-03 Lexicon JSON and validate `reason` is declared with
   type string, minLength 1, maxLength 1000, NOT in `required[]`.
2. Serialize a sentinel `Claim` with `reason: Some("test")`, deserialize,
   assert byte-equal serde round-trip with `reason` preserved.
3. Serialize a sentinel `Claim` with `reason: None`, assert the serialized
   JSON does NOT contain the `"reason"` key (validates the
   `skip_serializing_if` behavior).
4. Run the CID-stability property test: a fixture claim from slice-01
   ground-truth (no reason field) MUST produce the same CID under
   slice-03's `compute_cid`.

The `claim-domain` crate gains a probe-equivalent property test for
`normalize_reason`: idempotence + NFC-unification (as above).

## Revisit Trigger

- A future slice's JTBD analysis surfaces a multi-block reason use case
  (e.g., separated "what I object to" + "what I propose instead"
  sections). Add `reason_blocks` as an additional optional field;
  `reason` remains for backward compat.
- A regulatory requirement (e.g., reason texts must be free of PII when
  the target is a person rather than a project). Add an opt-in
  `reason_classifier` field or move to a separate moderated reason
  surface.
- The 1000-char limit becomes a friction point per dogfood feedback (KPI
  TBD). Raise the limit or split into structured `summary` + `detail`
  fields with their own constraints.
