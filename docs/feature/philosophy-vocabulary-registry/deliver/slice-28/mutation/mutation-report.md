# Slice-28 Mutation Report — `scraper-domain` pure core

**Scope:** `crates/scraper-domain/src/mapping.rs` (the pure claim-mapping core).
**Tool:** cargo-mutants 25.3.1 (`--file` scoped, no `--in-place`; source tree copied to scratch).
**Command:** `cargo mutants -p scraper-domain --file crates/scraper-domain/src/mapping.rs`
**Killers:** the in-crate `scraper-domain` tests — the slice-28 drift-rejection RED
(`load_mapping_rejects_object_not_in_seeded_vocabulary`), the SSOT guardrail
(`every_ssot_mapping_object_resolves_in_seeded_vocabulary`), the new error-value
contract test (`unknown_philosophy_display_names_the_offending_object`), and the
9 slice-02 mapping tests.

## Tally

| Run | Mutants | Caught | Missed | Unviable | Viable kill rate |
|-----|--------:|-------:|-------:|---------:|-----------------:|
| Initial | 13 | 8 | 1 | 4 | 8/9 = 88.9% |
| After survivor kill | 13 | 9 | 0 | 4 | **9/9 = 100%** |

Unviable mutants are the `-> Ok(Default::default())` / `Some(Default::default())`
replacements on `load_mapping`, `parse_entry`, `signal_kind_for_description`, and
`entry_for`: the domain types (`SignalPredicateMapping`, `MappingEntry`,
`SignalKind`) deliberately do **not** derive `Default`, so these mutants do not
compile. They cannot exist in real code and are excluded from the kill-rate
denominator (standard cargo-mutants accounting).

## Gate

**PASS — 100% (9/9) viable kill rate on the pure core, ≥ 80% threshold.**

## Slice-28 validation surface (the NEW code)

The slice-28 change added (a) the seeded-vocabulary guard in `parse_entry`
(`if lexicon::philosophy::find(&entry.predicate).is_none() { return Err(UnknownPhilosophy) }`),
(b) the `MappingError::UnknownPhilosophy` variant, and (c) its `Display` arm.

Mutants touching that surface:

| Mutant | Location | Initial | Final | Notes |
|--------|----------|---------|-------|-------|
| `parse_entry -> Ok(Default::default())` | `mapping.rs:154` | unviable | unviable | `MappingEntry` has no `Default`; would bypass both validations if it compiled — behaviourally covered by the drift-rejection + malformed-signal tests via `load_mapping`. |
| `<impl Display for MappingError>::fmt -> Ok(Default::default())` | `mapping.rs:61` | **MISSED** | **CAUGHT** | Empties the rendered message for **both** error arms, including the new `UnknownPhilosophy` arm. |

Note: cargo-mutants' operator set did not emit a standalone mutant for the
`.is_none()` guard condition itself (no negate-condition operator fired here); the
guard's behaviour is nonetheless pinned by the drift-rejection RED test, which
asserts `load_mapping(drift_yaml).is_err()`.

## Survivor analysis + fix

**Survivor (initial run):** `mapping.rs:61` — replacing the whole `Display::fmt`
body with `Ok(Default::default())` (writes nothing) went undetected. No test
asserted the *text* of any `MappingError`, so an empty message survived. For the
slice-28 `UnknownPhilosophy` arm this is a genuine gap: the KPI-PV-6 contract is
that a rejected drift string is **named** in the error so an operator can see
*which* orphan philosophy string failed — an empty/opaque message silently breaks
that.

**Fix (committed):** added one targeted in-crate example test —
`unknown_philosophy_display_names_the_offending_object` — asserting the rendered
`UnknownPhilosophy` message contains the offending object string. This is a real
observable-behaviour assertion (errors are values with meaningful, named content;
nw-fp-domain-modeling §8 railway-oriented), not a language-guarantee or
tautological test. Re-running mutation confirmed the survivor is now **CAUGHT**
and the pure core reaches 100% (9/9).

## Pre-existing slice-02 mutants (out of scope, all caught)

The remaining viable mutants live in pre-existing slice-02 code
(`entry_for` `==`→`!=` and `-> None`; the five `signal_kind_for_description`
match-arm deletions). All were already caught by the slice-02 test suite — no
action taken (this slice's mandate is the slice-28 validation change only).

## Post-run hygiene

- No `--in-place`: working tree never mutated.
- `mutants.out*` scratch removed.
- `cargo test -p scraper-domain` green (12 passed).
- `git status` clean of any mutation artifacts.
