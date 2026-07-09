<!-- markdownlint-disable MD013 -->
# Slice-25 Mutation Report — philosophy-vocabulary-registry

**Date:** 2026-07-08
**Tool:** cargo-mutants 25.3.1
**Gate:** pure-core kill rate >= 80%
**Result:** **PASS** — pure-core viable kill rate = **100%** (18/18)

## Scope

- **Pure core (GATED surface):** `crates/lexicon/src/philosophy.rs` — the slice-25
  addition is `resolve_object_advisory(object) -> ObjectAdvisory` (the alias-aware
  advisory classifier), plus the `seeds` / `normalize` / `object_id` functions it
  reuses. Killed by the in-crate proptest
  `resolve_object_advisory_classifies_known_alias_unknown_and_non_philosophy`
  (every seed's object_id → Canonical, every alias → Alias{canonical}, an unknown
  in-namespace segment → UnknownInNamespace, a non-prefixed string → NotPhilosophy).
- **Effect shell (REPORTED, not gated):** `crates/cli/src/verbs/claim_add.rs` —
  the `compose_object_advisory_line` helper + its call in `render_compose_preview`.
  Covered by the CA-1..5 subprocess acceptance binary; classified qualitatively
  below.

## Command

```
cargo mutants -p lexicon --file crates/lexicon/src/philosophy.rs
```

## Tally

| Outcome  | Count |
|----------|-------|
| Total mutants | 22 |
| Caught (killed) | 18 |
| Unviable (did not compile) | 4 |
| **Missed / survived** | **0** |

Runtime: 22 mutants in 24s (5.7s baseline build + 0.6s baseline test; auto test
timeout 20s).

**Pure-core viable kill rate = caught / (caught + missed) = 18 / 18 = 100%.**
(The 4 unviable are the `Default::default()` substitutions over enums/structs with
no derived `Default` — including the new `ObjectAdvisory` — which never compile and
are excluded from the rate per cargo-mutants convention.)

## Coverage of the slice-25 resolver

The mutants over `resolve_object_advisory` — swapping the Canonical/Alias
precedence, flipping the `normalize` equality (`==`→`!=`) in the name/alias match,
and short-circuiting the namespace-prefix check — are all caught by the proptest,
which pins every seed's object_id → Canonical and every alias → Alias{canonical}
AND the two negative arms (unknown-in-namespace, non-prefixed). 0 survivors. The
canonical-before-alias ordering (the highest-value semantic target) is covered:
a mutant that reordered the arms reddens the seed-name-vs-alias property.

## Effect-shell qualitative assessment (reported, not gated)

- **`compose_object_advisory_line` + `render_compose_preview`** — the four-arm
  match (Canonical / Alias / UnknownInNamespace / NotPhilosophy → the resolution
  line / alias line / non-blocking warning / nothing) is pinned by CA-1 (known),
  CA-2 (alias), CA-3 (unknown-still-signs), and CA-5 (out-of-namespace → no
  advisory). A mutant dropping the advisory line reddens CA-1/2/3; a mutant
  emitting a line for `NotPhilosophy` reddens CA-5.
- **AC-004.3 byte-parity (CA-4)** — the load-bearing invariant. The advisory is a
  preview-STRING-only concern; `build_unsigned_claim` / the signed object bytes are
  untouched, so any mutant that routed the resolver's canonical/alias result into
  the signed payload reddens CA-4 (signed object == typed string verbatim).

## Gate verdict

**PASS** — pure-core viable kill rate 100% (18/18) ≥ 80%. 0 survivors. Effect shell
covered by CA-1..5 acceptance (incl. the CA-4 byte-parity guard).
