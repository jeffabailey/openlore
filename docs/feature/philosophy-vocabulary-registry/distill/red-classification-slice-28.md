<!-- markdownlint-disable MD013 -->
# RED Classification — slice-28 (scraper-uses-seeded-philosophies)

> DISTILL Pre-DELIVER fail-for-the-right-reason gate (nw-distill §"Pre-DELIVER
> fail-for-the-right-reason gate"). The slice-28 primary RED scaffold + guardrail
> were run once against the CURRENT (unimplemented) production code and classified.
> DELIVER reads this file at the RED-phase entry gate (ADR-025 D2) to confirm RED
> is genuine.
>
> Owner: Quinn (nw-acceptance-designer) · 2026-07-06 · Rust / in-crate `#[cfg(test)]`
> unit shape (mirrors slice-22 `philosophy_validator_tests` in `lexicon/src/lib.rs`).
> Scope: US-PV-007 (AC-007.1..2 + KPI-PV-6) — the scraper's signal→predicate mapping
> must reference SEEDED philosophy records ONLY. slice-28 is the LAST slice of
> philosophy-vocabulary-registry; slice-22 (seed + list) and slice-23 (`philosophy
> show`) are SHIPPED.

## How the run was performed

```
cargo test -p scraper-domain --no-run                       # COMPILE gate (BROKEN check) → exit 0, Finished
cargo test -p scraper-domain -- --test-threads=1            # RED run → exit 101 (10 passed, 1 failed)
```

The `scraper-domain` test target COMPILES green (`--no-run` → `Finished`, exit 0). The
new slice-28 tests live in a fresh `#[cfg(test)] mod philosophy_vocabulary_tests` appended
to `crates/scraper-domain/src/mapping.rs`. The PRIMARY RED references only the EXISTING
`load_mapping` signature (`Result<SignalPredicateMapping, MappingError>`) and asserts
`.is_err()` — it names NO not-yet-existing symbol (no `validate_vocabulary`, no new
`MappingError` variant). Therefore its failure is a RUNTIME assertion panic, not a
compile/import error → RED, never BROKEN.

The GUARDRAIL resolves each shipped SSOT object against the real seeded vocabulary via
`lexicon::philosophy::find`. `lexicon` is a PURE crate (no I/O; no dependency on
scraper-domain, so no cycle) and was added to `crates/scraper-domain/Cargo.toml`
`[dev-dependencies]` for this test-only resolution. DELIVER promotes it to a normal
`[dependencies]` entry for the production seeded-object validation.

## What is missing today (the RED cause)

- **`load_mapping` has NO seeded-vocabulary validation.** It parses the embedded YAML and
  validates each entry's free-text `signal` description against `SignalKind` (unrecognized
  signal → `MappingError::MalformedEntry`), but it does NOT check that the entry's `object`
  (the `org.openlore.philosophy.*` string) is a SEEDED philosophy. A mapping with a VALID
  signal but a DRIFT object therefore parses `Ok` today.
  - Observed: `load_mapping(drift_yaml)` returned
    `Ok(SignalPredicateMapping { entries: [MappingEntry { signal_kind: MemorySafetyLanguage,
    object: "org.openlore.philosophy.mystery", default_confidence: 0.25 }] })`.
  - The primary test uses a VALID SSOT signal verbatim (`"Primary language is Rust OR
    memory-safety language + no unsafe blocks"` → `SignalKind::MemorySafetyLanguage`) so the
    signal resolves cleanly and the `Ok` is unambiguously the missing OBJECT validation, not
    a malformed-signal side effect. This is the distinction from the pre-existing
    `load_mapping_rejects_unknown_signal_description` test, which drifts the SIGNAL (and so
    already rejects today).

## Classification key

- **RED (MISSING_FUNCTIONALITY, assertion)** ✅ — the assertion fires because the
  seeded-object validation in `load_mapping` is unimplemented (drift object accepted as
  `Ok`). Correct RED.
- **GREEN-today (no-regression guardrail)** — pins KPI-PV-6 for the shipped SSOT (all 5
  objects are seeded); passes today, guards a future orphan-object edit.
- **BROKEN / SETUP / IMPORT** ❌ — would block handoff. **NONE remain.**

## Tally

| File | Test | AC | Classification | Why |
|---|---|---|---|---|
| `scraper-domain/src/mapping.rs` | `philosophy_vocabulary_tests::load_mapping_rejects_object_not_in_seeded_vocabulary` | AC-007.2 / KPI-PV-6 | RED ✅ | VALID signal + DRIFT object `org.openlore.philosophy.mystery`; `load_mapping` returns `Ok` (no vocabulary check) → `.is_err()` assertion panics at mapping.rs:219 → MISSING_FUNCTIONALITY |
| | `philosophy_vocabulary_tests::every_ssot_mapping_object_resolves_in_seeded_vocabulary` | AC-007.1 / KPI-PV-6 | GREEN-today (guardrail) | shipped SSOT parses `Ok` AND all 5 objects resolve via `lexicon::philosophy::find` (all seeded today) — pins 0 orphan philosophy strings |

### Numeric summary (slice-28 tests only; the other 9 passing tests are pre-existing slice-02 mapping/derive tests, unchanged)

| Classification | Count |
|---|---|
| RED — MISSING_FUNCTIONALITY (assertion, drift object accepted) | 1 |
| GREEN-today (no-regression guardrail) | 1 |
| **BROKEN / SETUP / IMPORT** | **0** |
| **Total slice-28 tests** | **2** |

Observed runner output: `test result: FAILED. 10 passed; 1 failed; 0 ignored`. The single
failure is `load_mapping_rejects_object_not_in_seeded_vocabulary` (the primary RED); the
guardrail and all 9 pre-existing tests pass. Exit 101 (test failure), NOT a compile/link
error.

## Optional acceptance-level guardrail — DECISION: SKIPPED

A `scrape`-through-the-bin AT (via the `FakeGithub` harness) asserting every proposed
candidate object `philosophy show`-resolves (KPI-PV-6 end-to-end) would be GREEN-today (the
5 SSOT objects are all seeded) and adds harness cost for no RED signal this slice. The
domain-level primary RED + the vocabulary-resolution guardrail cover AC-007.1/.2 + KPI-PV-6
at the pure core where the drift originates. Skipped per nw-distill (rely on domain-level
tests; note the decision here).

## Gate verdict

**PASS.** The one failing test fails for the RIGHT reason (MISSING_FUNCTIONALITY —
`load_mapping` has no seeded-object validation, so a drift object is accepted as `Ok`). It is
NOT in category 2 (IMPORT_ERROR / FIXTURE_BROKEN / SETUP_FAILURE — the target compiles green,
the failure is a runtime `assert!` panic against the EXISTING `load_mapping` API) nor category
3 (WRONG_ASSERTION / internal-struct coupling — the assertion inspects only the OBSERVABLE
`load_mapping` return `Result`, never a private field). Zero BROKEN / SETUP / IMPORT. Handoff
to DELIVER is UNBLOCKED for slice-28.

## Error/edge ratio note

2 slice-28 tests: 1 sad-path (drift-object rejection, AC-007.2) + 1 invariant guardrail
(every SSOT object seeded, AC-007.1). The load-bearing scenario is the sad path — the drift
rejection that does not yet exist. Example-based per Mandate 11 (layer-2 pure core; the
seeded-object rejection is a single enumerated sad path, no PBT generation).

## DELIVER pointers (from the observed RED)

1. Promote `lexicon` from `[dev-dependencies]` to `[dependencies]` in
   `crates/scraper-domain/Cargo.toml` (pure crate; no cycle — `lexicon` does not depend on
   scraper-domain). `xtask check-arch` pure-core allowlist may need `lexicon` added (it is a
   pure, no-I/O crate).
2. In `load_mapping` (or a new `validate_vocabulary` step it calls), validate each entry's
   `object` against the seeded vocabulary via `lexicon::philosophy::find(&object).is_some()`
   (single source — AC-007.1). An object with no seeded philosophy returns an `Err` naming
   the offending object string (AC-007.2 — the failure is EXPLICIT, no `mystery` drift). This
   turns the primary RED GREEN.
3. Optionally add a dedicated `MappingError` variant (e.g. `UnknownPhilosophy(String)`) so
   the error names the drift object distinctly from a `MalformedEntry` signal error — but the
   primary RED only asserts `.is_err()`, so a `MalformedEntry` carrying the object string also
   satisfies it. Keep the message naming the offending object either way.
4. Leave the SSOT mapping (`crates/scraper-domain/src/signal_predicate_mapping.yaml` /
   `jobs.yaml :: J-004.signal_predicate_mapping`) UNCHANGED — it already contains only seeded
   objects; the guardrail confirms KPI-PV-6 holds for it today.
