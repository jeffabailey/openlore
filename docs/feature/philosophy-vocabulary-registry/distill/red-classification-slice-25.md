<!-- markdownlint-disable MD013 -->
# RED Classification — slice-25 (claim-compose-suggests-philosophy)

> DISTILL Pre-DELIVER fail-for-the-right-reason gate (nw-distill §"Pre-DELIVER
> fail-for-the-right-reason gate"). Every slice-25 acceptance scenario was run
> once against the CURRENT (unimplemented) production code and classified.
> DELIVER reads this file at the RED-phase entry gate (ADR-025 D2) to confirm RED
> is genuine.
>
> Owner: Quinn (nw-acceptance-designer) · 2026-07-08 · Rust / cucumber-free
> subprocess acceptance shape (mirrors slice-24 `philosophy_add.rs`).
> Scope: US-PV-004 (AC-004.1..3) — the `openlore claim add` COMPOSE ADVISORY for
> `--object` ONLY (job_id J-001; ADR-059 §5 row 25). `claim add` already
> composes → previews → SIGNS → persists (slice-01, SHIPPED). Slice-25 adds ONE
> display-only advisory line. Slices 22 (seed+list), 23 (show), 24 (mint), 28
> (scraper) are SHIPPED. The read-time aggregation under a canonical (slice-26)
> and alias triangulation in `graph query`/`score` are OUT.

## Wave-decision reconciliation

The feature uses the single `docs/feature/philosophy-vocabulary-registry/feature-delta.md`
SSOT — there are no separate `discuss/`, `design/`, `devops/` `wave-decisions.md`
files to cross-check. AC-004.1..3 (feature-delta.md US-PV-004, lines 101–110) and
the DESIGN row 25 ("cli (`claim_add` preview); lexicon (`VocabularyIndex`) | One
advisory line; signed bytes byte-unchanged (AC-004.3)") agree with each other and
with the slice brief (slices/slice-23-to-28-briefs.md §slice-25). **Reconciliation
passed — 0 contradictions.**

## How the run was performed

```
cargo build --bin openlore                                          # build-before-run (the AT spawns the real bin)
cargo test -p cli --test claim_compose_advisory --no-run            # COMPILE gate (BROKEN check)
cargo test -p cli --test claim_compose_advisory -- --test-threads=1
```

The acceptance target COMPILES green (`--no-run` → `Finished`; the 15 warnings
are all from the shared `support` harness — unused imports / unreachable match
arms — none from `claim_compose_advisory.rs`). It spawns the real `openlore` bin
via the existing `run_openlore_with_stdin` support harness and imports only that
harness (`mod support; use support::*`) plus `std::path` — NO new production
symbol, NO typed deserialization into a `claim_domain`/`lexicon` struct (the
persisted-artifact assertions read the JSON as TEXT). Therefore every acceptance
failure is a RUNTIME assertion against the observable CLI surface, not a
compile / import error → RED, never BROKEN.

A `[[test]]` target `claim_compose_advisory` was added to `crates/cli/Cargo.toml`
(mirroring the `philosophy_add` entry) so the workspace-root
`tests/acceptance/claim_compose_advisory.rs` is discoverable — the only
build-config change. No new crate; the workspace stays at 21 members.

## What is missing today (the RED cause)

- **No advisory line in the compose preview.** `claim add` composes / signs /
  persists fine (slice-01 SHIPPED), but today's
  `render_compose_preview` (`crates/cli/src/verbs/claim_add.rs` ~line 290) prints
  ONLY the slice-01 fields (`subject / predicate / object / evidence / confidence
  / author / composedAt` under the `not as truth` header). It emits NO advisory
  line for the `--object` — no `resolves to <canonical>`, no `(alias)` marker, no
  `not a known philosophy` warning (observed verbatim in every failing scenario's
  captured stdout). The alias-aware resolver seam (`VocabularyIndex` over
  `seeds()` + each seed's `aliases`) does not exist yet — `lexicon::find` today
  matches only bare-name / object-id, never aliases.
  - **CA-1 / CA-2 / CA-4** assert the preview shows the resolution advisory
    (`resolves to` + the canonical `memory-safety`, plus `(alias)` for the alias
    cases) → all fail on that substring, because the preview has no advisory line
    → MISSING_FUNCTIONALITY. (CA-2 is the strongest: the object line shows the
    typed alias `…mem-safety`, which does NOT contain `memory-safety`, so
    asserting the preview names the canonical proves the resolver surfaced it.)
  - **CA-3** (unknown, non-blocking) asserts exit 0 FIRST — that PASSES (an
    unknown object still signs; the advisory never gates) — and then asserts the
    preview shows `not a known philosophy` → FAILS, because that warning does not
    exist yet → MISSING_FUNCTIONALITY.
  - **CA-4** (byte-parity, LOAD-BEARING) asserts the byte-parity guarantee FIRST
    (signed object == typed alias `…mem-safety`, and NOT the canonical
    `…memory-safety`) — both PASS today (claim add signs the object verbatim; the
    captured artifact object is `org.openlore.philosophy.mem-safety`) — and then
    asserts the alias advisory was shown → FAILS. So CA-4 is RED because the
    advisory is absent, while the AC-004.3 byte-parity invariant it PINS is
    already intact → the test locks that invariant so DELIVER's advisory cannot
    corrupt it.

## Classification key

- **RED (MISSING_FUNCTIONALITY, assertion)** ✅ — the assertion fires because the
  `--object` compose advisory (the `VocabularyIndex` resolution line, the alias
  marker, the non-blocking unknown warning) is unimplemented. Correct RED.
- **GREEN-today (no-regression guardrail)** 🟢 — passes against today's code and
  must STAY green after DELIVER (an over-firing advisory would red it). Not a
  failure; not a blocker.
- **BROKEN / SETUP / IMPORT** ❌ — would block handoff. **NONE remain.**

## Tally

| File | Scenario | AC | Panic line | Classification | Why |
|---|---|---|---|---|---|
| `claim_compose_advisory.rs` | CA-1 `compose_advisory_known_object_shows_resolution_and_signs_normally` (WS) | AC-004.1 known | 267 | RED ✅ | exit 0 + persist PASS; the `resolves to memory-safety` advisory is absent from today's preview → resolution-substring assertion fails |
| | CA-2 `compose_advisory_alias_object_resolves_to_canonical_and_marks_alias` | AC-004.1 alias | 312 | RED ✅ | preview shows the typed alias `…mem-safety` but NOT the canonical `memory-safety` nor a `resolves to`/`alias` advisory → resolution-substring assertion fails |
| | CA-3 `compose_advisory_unknown_object_warns_but_still_signs_when_confirmed` | AC-004.2 | 370 | RED ✅ | exit 0 PASSES (non-blocking — the unknown object still signs); the `not a known philosophy` warning is absent → warning-substring assertion fails |
| | CA-4 `compose_advisory_does_not_alter_the_signed_object_bytes` | AC-004.3 (+.1) | 436 | RED ✅ | byte-parity PASSES (signed object == typed `…mem-safety`, not the canonical); the alias advisory is absent → advisory assertion fails (the invariant is pinned, the advisory is the RED driver) |
| | CA-5 `compose_advisory_absent_for_non_philosophy_object` | AC-004 no-regression | — | GREEN-today 🟢 | a non-philosophy `--object` draws NO advisory today; DELIVER must keep it so (over-firing / nagging guard) |

### Numeric summary (slice-25 scenarios only; excludes the 2 pre-existing `support::state_delta` framework self-tests bundled in the acceptance binary)

| Classification | Count |
|---|---|
| RED — MISSING_FUNCTIONALITY (assertion; advisory line absent from today's preview) | 4 |
| GREEN-today (no-regression / no-nagging guardrail) | 1 |
| **BROKEN / SETUP / IMPORT** | **0** |
| **Total slice-25 tests** | **5** |

RED total = **4**, all assertion-RED. One GREEN-today guardrail (CA-5). Zero
BROKEN. Observed runner output: `running 7 tests … 3 passed; 4 failed` — the 3
passes are CA-5 (the no-advisory guardrail) plus the 2 `support::state_delta::tests::*`
framework self-tests (present in every acceptance binary), NOT slice-25 RED
scenarios; the four `compose_advisory_*` RED functions appear in the `failures:`
list.

## Gate verdict

**PASS.** Every failing test fails for the RIGHT reason (MISSING_FUNCTIONALITY —
the `--object` compose advisory: the `VocabularyIndex` resolution line, the alias
marker, and the non-blocking unknown warning do not exist yet; today's
`render_compose_preview` prints no advisory). Zero tests are in category 2
(IMPORT_ERROR / FIXTURE_BROKEN / SETUP_FAILURE) or category 3 (WRONG_ASSERTION /
internal-struct coupling — every assertion scans the OBSERVABLE CLI stdout / exit
code, or reads the on-disk signed artifact as TEXT, never a `claim_domain` /
`lexicon` struct field). The one GREEN-today scenario (CA-5) is an intentional
over-firing guardrail, not a failure. Handoff to DELIVER is UNBLOCKED for
slice-25.

## Error/edge ratio note

5 scenarios: CA-1 (WS happy known-object mint) = 1 pure-happy; CA-2 (alias,
display-only cancel — edge) + CA-3 (unknown object non-blocking — error/edge) +
CA-4 (byte-parity invariant guard) + CA-5 (out-of-namespace no-regression — edge)
= 4 non-pure-happy = **80%** (≥40% target). The two named advisory branches
(AC-004.2 unknown warning, AC-004.1 alias) and the AC-004.3 byte-parity invariant
are covered explicitly by CA-3 / CA-2 / CA-4, example-based per Mandate 11 (no PBT
at layer 3 — the alias-aware `VocabularyIndex` resolver is property-tested at
layer 1 in `crates/lexicon` by DELIVER).

## Outcomes-registry note

Skipped — `docs/product/outcomes/registry.yaml` does not exist and the prior
philosophy slices (22 seed+list, 23 show, 24 mint) registered no OUT-N rows.
Following that precedent, no outcome is registered for the compose-advisory
operation. If the registry is later adopted for this feature, register the
`--object` advisory resolution as a `kind: specification` (a display-only nudge
rule) at that time.

## DELIVER pointers (from the observed RED)

1. **Add a pure alias-aware resolver seam to `crates/lexicon`** (`VocabularyIndex`
   per ADR-059 §5 row 25). Over `seeds()` + each seed's `aliases`, resolve an
   `--object` in the `org.openlore.philosophy.*` namespace to one of three
   display outcomes: `Known(canonical)` (the object id / bare name matches a
   seed — reuse `find`), `Alias(canonical)` (the object's normalized last segment
   matches a seed's `aliases` entry — this is the NEW arm `find` lacks today), or
   `Unknown` (in-namespace but neither). An object OUTSIDE the namespace resolves
   to "no advisory". Pure + total; property-test the resolution at layer 1 (every
   seed name AND every alias resolves to its canonical; arbitrary input never
   panics). Turns the resolver half of CA-1/CA-2/CA-3/CA-4 available.
2. **Emit ONE advisory line from `render_compose_preview`** (`claim_add.rs` ~line
   290) driven by the resolver — display-only, appended to the existing preview,
   NEVER folded into `build_unsigned_claim` (the signed payload must stay
   byte-identical, AC-004.3 / CA-4). Suggested wording (the tests pin these domain
   SUBSTRINGS case-insensitively — see §"advisory wording contract" below):
   `↳ resolves to <canonical>` for `Known`, `↳ resolves to <canonical> (alias)`
   for `Alias`, `⚠ not a known philosophy — will be signed as-is` for `Unknown`,
   and NOTHING for an out-of-namespace object. Turns CA-1/CA-2/CA-3/CA-4 GREEN
   while keeping CA-5 green.
3. **Keep the advisory strictly display-only + non-blocking.** The resolution
   feeds ONLY the preview string; `build_unsigned_claim` continues to pass
   `composed.object` through verbatim (AC-004.3). The unknown warning must NOT
   change the exit code or the sign flow — an unknown object still signs on
   confirm (CA-3). `xtask check-arch` stays 21 members / no new crate (one lexicon
   resolver fn + one preview line over the existing `seeds()` core).

## Upstream gaps for DELIVER to resolve

- **The exact advisory wording/layout is DELIVER's to choose — these tests pin
  only three domain SUBSTRINGS** (the "advisory wording contract"), matched
  case-insensitively so DELIVER owns capitalization, the ↳ / ⚠ glyphs, and the
  surrounding layout:
  - `resolves to` (AC-004.1 known + alias) — constant `ADVISORY_RESOLVES`
  - `alias` (AC-004.1 alias marker `(alias)`) — constant `ADVISORY_ALIAS`
  - `not a known philosophy` (AC-004.2 warning) — constant `ADVISORY_UNKNOWN`
  These are lifted verbatim from feature-delta.md US-PV-004 (line 105:
  `↳ resolves to memory-safety (alias)` / `⚠ not a known philosophy — will be
  signed as-is`). If DELIVER chooses different phrasing, it updates these three
  constants at the top of `claim_compose_advisory.rs` — they are the ONLY layout
  coupling; everything else (object line, canonical name, artifact object) is read
  from the observable output.
- **The byte-parity assertion approach for AC-004.3 (confirmed).** CA-4 proves
  display-only by reading the persisted `<cid>.json` as TEXT and asserting the
  signed object is byte-identical to the TYPED alias (`org.openlore.philosophy.mem-safety`)
  AND does NOT contain the resolved canonical (`org.openlore.philosophy.memory-safety`).
  The alias case is deliberate — it is the ONLY case where a naive resolver could
  "helpfully" rewrite the object to the canonical, so it is the strongest D3
  guard. This assertion PASSES today (claim add signs the object verbatim), which
  is correct: CA-4 is RED solely because the advisory is missing, and it PINS the
  byte-parity invariant so DELIVER's step 2 cannot regress it. DELIVER MUST NOT
  route the resolution through `build_unsigned_claim`.
- **Namespace-trigger boundary (confirm during implementation).** These tests
  assume the advisory fires only for `--object` in the `org.openlore.philosophy.*`
  namespace: KNOWN/ALIAS/UNKNOWN inside it (CA-1/2/3/4), NO advisory outside it
  (CA-5). This matches the slice brief ("advisory targets philosophy objects").
  If DELIVER instead keys the advisory on the `embodiesPhilosophy` PREDICATE
  rather than the object namespace, CA-5 (predicate `dependsOn`, non-philosophy
  object) still holds, but confirm the KNOWN/ALIAS/UNKNOWN objects are recognized
  by whichever trigger is chosen — the tests only observe the resulting advisory
  text, so either trigger satisfies them as long as the namespace objects resolve.
