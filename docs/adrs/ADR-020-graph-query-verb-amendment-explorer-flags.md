# ADR-020: Graph-Query Verb Amendment — Explorer Flags (`--object`, `--contributor`, `--traverse`/`--depth`, `--weighted`, `--explain`)

- **Status**: Proposed
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), per WD-75/WD-76/WD-77 locks from Luna (nw-product-owner) for openlore-scoring-graph
- **Feature**: openlore-scoring-graph (slice-04)
- **Amends**: ADR-003 (CLI Verb Contract) + ADR-013 (slice-03 `--federated` flag precedent). Both remain in force; this ADR EXTENDS the `graph query` flag surface. The two-prompt, single-publish-path, and idempotency invariants from ADR-003 do not apply (slice-04 is read-only; it neither prompts to sign nor publishes), but the verb-grammar consistency and exit-code discipline carry forward.

## Context

Slice-04 grows the read surface from "list attributed claims" to "explore,
traverse, and transparently weight the local federated graph." DISCUSS locked
the user-visible dimensions and behaviors (WD-75 dimensions; WD-76 bounded
traversal; WD-77 transparent weighting) but left the exact CLI grammar to
DESIGN (the ADR amendment was flagged as a slice-04 deliverable, "likely
ADR-020", in the DISCUSS handoff).

ADR-003 fixed the slice-01 verb surface; ADR-013 added the slice-03 `peer`
verbs + the `--federated` flag on `graph query`. Slice-04 adds NO new verb —
it amends `graph query` with explorer flags.

DESIGN owns:

1. Whether the new dimensions are flags, sub-verbs, or new verbs.
2. The exact flag shapes + combinability.
3. The default scope (do explorer flags imply federated?) — OD-GRAPH-4.
4. Exit-code semantics for the new flag combinations.

## Decision

**Amend the existing `graph query` verb with six explorer flags; add no new
verb.**

### Flag surface (added to `graph query`)

| Flag | Shape | Purpose | Combinable with |
|---|---|---|---|
| `--object <philosophy>` | dimension | Which projects embody this philosophy, grouped by subject (WD-75). | `--weighted`, `--traverse`, `--explain` |
| `--contributor <did>` | dimension | One developer's reasoning trail across all subjects (WD-75). | `--traverse` (weighting a contributor's whole trail is not slice-04 scope) |
| `--subject <project>` | dimension (inherited) | Unchanged from slice-01/03; the project-first listing. | `--weighted`, `--traverse`, `--explain`, `--federated` |
| `--traverse` | mode | Walk contributor↔project↔philosophy edges; render a tree + "Connections found" callout (WD-76). | any dimension; `--depth` |
| `--depth K` | modifier | Override the default traversal depth of 2 (WD-76). Only meaningful with `--traverse`. | `--traverse` |
| `--weighted` | mode | Rank (subject, object) pairings by the derived adherence weight; print the formula + bucket (WD-77). | `--object`, `--subject`, `--traverse`, `--explain` |
| `--explain <subject>` | modifier | Print the per-claim arithmetic for the named subject; reproduce the weight by hand (WD-77; US-GRAPH-005). Only meaningful with `--weighted`. | `--weighted` |

### Default scope — explorer flags imply federated (WD-87, resolves OD-GRAPH-4)

`--object`, `--contributor`, `--traverse`, and `--weighted` read the WHOLE
local graph (own + subscribed peers + unsubscribed-cache + scraper-signed) by
default; `--federated` remains accepted (a no-op when already implied) for
symmetry with slice-03. A bare `graph query --subject <project>` with NO new
flag is byte-identical to slice-01 behavior (own claims only), preserving
backward compatibility. The default federated scope removes friction that
would kill exploration (KPI-GRAPH-6); per-author attribution is preserved
regardless of scope (anti-merging, I-GRAPH-2).

### Verb-grammar consistency

- The new flags sit on `graph query`, the existing read-surface verb. A
  `graph query` result is still "attributed claims, possibly ranked or
  traversed" — flags modify scope, not the verb's observable kind. This is the
  same reasoning that made `--federated` a flag, not a verb, in ADR-013.
- `--object`/`--contributor` are symmetric with the inherited `--subject`:
  three dimensions, one verb. This avoids a verb explosion (`graph by-object`,
  `graph by-contributor`, `graph traverse`, `graph weighted` would be four
  verbs for one read concern).
- `--explain <subject>` mirrors `--depth K`: a modifier that refines a mode.

### Exit-code semantics (slice-04 flag combinations)

| Invocation | Exit 0 | Exit 1 | Exit 2 |
|---|---|---|---|
| `graph query --object/--subject/--contributor [--weighted] [--traverse]` | Query rendered (may show zero rows; unknown object/absent contributor is a valid empty result with a suggestion/hint) | Storage read error | Probe gauntlet refused at startup |
| `graph query --weighted --explain <subject>` | Subject is in the result set; breakdown rendered | **Subject NOT in the result set** (usage error, per US-GRAPH-005 Example 3) | Probe gauntlet refused |
| `graph query --traverse [--depth K]` | Tree rendered (may report "no connecting edges" or omitted edges) | Storage read error / non-terminating traversal caught by the bound | Probe gauntlet refused |

Note the deliberate asymmetry: an empty DIMENSION query exits 0 (a valid
"nothing matches"); `--explain` on an absent subject exits non-zero (a usage
error — the user named a subject that is not in the ranking).

### Output line conventions (slice-04)

- `--object` MUST emit a footer naming distinct-subject-count AND
  distinct-author-count AND the no-merge guarantee (content-frozen, extends
  the slice-03 anti-merging footer; I-GRAPH-2).
- `--contributor` MUST emit a footer "one developer's reasoning trail, not a
  community consensus" (content-frozen) + the slice-03 relationship labels
  (`you` / `subscribed peer` / `unsubscribed cache`).
- `--weighted` MUST print the formula AND the literal "no ML" AND a footer
  stating weights are a display-only aggregate view, never stored (WD-72).
- `--weighted` on a thin pairing MUST emit `[SPARSE]` + "based on N claims by
  M authors" + lead-not-conclusion advice (WD-74, content-frozen).
- `--traverse` MUST state "Traversal does not invent edges." + report omitted
  edges when bounded (WD-76).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **New sub-verbs `graph by-object` / `graph by-contributor` / `graph traverse` / `graph weighted`** | Verb explosion: four verbs for one read concern. Breaks the slice-03 precedent of `--federated` as a flag. The dimensions and modes compose (`--weighted --explain`, `--object --traverse`); flags compose cleanly where sub-verbs would not. |
| **`--philosophy` instead of `--object`** | `object` is the Lexicon field name (`org.openlore.claim.object`) and the slice-01 mental model; `--object` keeps CLI grammar aligned with the data model. (`--philosophy` would read nicer but diverge from the schema vocabulary.) |
| **Require `--federated` explicitly on every explorer flag** | Friction that kills exploration (KPI-GRAPH-6). The explorer is reading the whole local graph by intent; defaulting to federated (WD-87) with attribution preserved is the right ergonomics. Bare `--subject` stays own-only for backward compatibility. |
| **`--weighted` and `--explain` as one fused flag (`--explain` implies `--weighted`)** | The weighted ranking (US-GRAPH-003) and the per-claim drill-down (US-GRAPH-005) are distinct user moments; `--explain` refines `--weighted` rather than replacing it. Keeping them separate matches the story split and lets `--weighted` alone be the common case. |
| **Unbounded `--traverse`** | Locked rejected (WD-76); a dense graph fans out without bound. Default depth 2 + `--depth K` override. |

## Consequences

### Positive

- Verb count stays flat (no new verb after slice-04); `graph query` gains
  flags that compose.
- The explorer surface is discoverable from one `graph query --help`.
- The default-federated scope (WD-87) makes the common explorer flow
  zero-friction while preserving attribution.
- Read-only: none of ADR-003's prompt/publish machinery is invoked; the
  acceptance surface is simpler than the slice-03 verbs.

### Negative

- `graph query --help` grows; many flags on one verb. **Mitigation**: clap
  groups flags; the flags fall into clear families (dimension / mode /
  modifier). DISTILL asserts the help groups the explorer flags coherently.
- The default-federated scope (WD-87) means `--object` shows peer claims
  without an explicit opt-in, unlike slice-03's explicit `--federated`. A user
  could be surprised to see peer claims. **Mitigation**: the relationship
  labels (`subscribed peer` / `unsubscribed cache`) make the source explicit
  on every row; bare `--subject` stays own-only.

### Earned Trust

The CLI driver's probe set extends to cover the new flags as read-only
contracts (string-match probes runnable in CI):

1. `graph query --object <philosophy>` groups by subject; every row carries
   one `author_did`; footer states the no-merge guarantee.
2. `graph query --weighted` prints the formula + "no ML" + the never-stored
   footer; the displayed weight equals the sum of the `--explain` subtotals.
3. `graph query --weighted --explain <absent-subject>` exits non-zero.
4. `graph query --traverse` on a cyclic/dense fixture returns within budget,
   bounded to depth 2, with the omitted-edge line + "Traversal does not invent
   edges."
5. Every explorer invocation succeeds with the network disabled (local-first;
   extends I-9 / KPI-5).

## Revisit Trigger

- Dogfeed shows the default-federated scope (WD-87) confuses users (peer claims
  appear unexpectedly) — reconsider an explicit `--federated` opt-in for
  explorer flags.
- A scripting need requires machine-readable output (`--json`) — a future ADR
  amendment adds it on the same verb.
- A new dimension emerges (e.g., `--predicate`) — a fourth dimension flag,
  same template.
