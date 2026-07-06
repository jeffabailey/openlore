<!-- markdownlint-disable MD013 -->
# RED Classification — slice-23 (philosophy-show)

> DISTILL Pre-DELIVER fail-for-the-right-reason gate (nw-distill §"Pre-DELIVER
> fail-for-the-right-reason gate"). Every slice-23 acceptance scenario was run
> once against the CURRENT (unimplemented) production code and classified.
> DELIVER reads this file at the RED-phase entry gate (ADR-025 D2) to confirm RED
> is genuine.
>
> Owner: Quinn (nw-acceptance-designer) · 2026-07-06 · Rust / cucumber-free
> subprocess acceptance shape (mirrors slice-22 `philosophy_vocabulary.rs`).
> Scope: US-PV-002 (AC-002.1..2) — the `openlore philosophy show <name-or-object>`
> read verb ONLY. Slice-22 (seed + list) is SHIPPED; slices 24–28 (mint /
> compose-advisory / alias / viewer / scraper) are OUT.

## How the run was performed

```
cargo test -p cli --test philosophy_show --no-run          # COMPILE gate (BROKEN check)
cargo build --bin openlore                                 # build-before-run (the AT spawns the real bin)
cargo test -p cli --test philosophy_show -- --test-threads=1
```

The acceptance target COMPILES green (`--no-run` → `Finished`; 16 warnings, all from the
shared `support` harness, none from `philosophy_show.rs`). It spawns the real `openlore`
bin via the existing `run_openlore` / `run_openlore_network_disabled` support harness and
imports only that harness (`mod support; use support::*`) — no new production symbol.
Therefore every acceptance failure is a RUNTIME assertion, not a compile/import error →
RED, never BROKEN. BUILD-BEFORE-RUN: the AT spawns the `openlore` bin built by
`cargo build --bin openlore`, not rebuilt by `cargo test`.

## What is missing today (the RED cause)

- **No `philosophy show` subcommand.** Slice-22 shipped the `philosophy` parent verb (with
  `list`), so `openlore philosophy show memory-safety` is rejected by clap at the
  SUBcommand level: `error: unrecognized subcommand 'show'` → **exit 2** (observed
  `left: 2, right: 0`).
  - PS-1 / PS-2 / PS-4 assert `status == 0` FIRST → all fail on that assertion (exit 2) →
    MISSING_FUNCTIONALITY (no `show` verb, no record render).
  - PS-3 (unknown-name sad path) asserts `status != 0` FIRST — that PASSES (exit 2 is
    non-zero) and the no-panic-marker guard PASSES (clap's usage text carries no panic /
    backtrace markers), but the next assertion — the combined output must NAME the miss and
    say "no such philosophy" + hint `philosophy list`/`philosophy add` — FAILS, because clap
    emitted only `unrecognized subcommand 'show'`. So PS-3 is RED because the plain
    unknown-name guidance does not exist yet → MISSING_FUNCTIONALITY.

## Classification key

- **RED (MISSING_FUNCTIONALITY, assertion)** ✅ — the assertion fires because the
  `philosophy show` verb (and its record render / unknown-name guidance) is unimplemented
  (clap exit 2). Correct RED.
- **BROKEN / SETUP / IMPORT** ❌ — would block handoff. **NONE remain.**

## Tally

| File | Scenario | AC | Classification | Why |
|---|---|---|---|---|
| `philosophy_show.rs` | PS-1 `philosophy_show_by_name_prints_name_description_aliases_and_see_also` (WS) | AC-002.1 | RED ✅ | `unrecognized subcommand 'show'` → exit 2; `status == 0` assertion fails; no verb, no record render |
| | PS-2 `philosophy_show_by_object_id_renders_the_same_record` | AC-002.1 | RED ✅ | same — no `show` verb; name-or-object acceptance absent |
| | PS-3 `philosophy_show_unknown_name_exits_non_zero_with_plain_guidance` | AC-002.2 | RED ✅ | exit 2 is non-zero (status guard passes) but the plain "no such philosophy" + list/add guidance is absent → guidance-substring assertion fails |
| | PS-4 `philosophy_show_succeeds_with_the_network_disabled` | AC-002.1 | RED ✅ | same (network-disabled) — no offline record render |

### Numeric summary (slice-23 scenarios only; excludes the 2 pre-existing `support::state_delta` framework self-tests bundled in the acceptance binary)

| Classification | Count |
|---|---|
| RED — MISSING_FUNCTIONALITY (assertion, clap exit 2 / absent guidance) | 4 |
| GREEN-today (no-regression guardrail) | 0 |
| **BROKEN / SETUP / IMPORT** | **0** |
| **Total slice-23 tests** | **4** |

RED total = **4**, all assertion-RED. Zero GREEN-today, zero BROKEN. Observed runner output:
`2 passed; 4 failed` — the 2 passes are the `support::state_delta` framework self-tests
(present in every acceptance binary), NOT slice-23 scenarios; all four `philosophy_show_*`
functions appear in the `failures:` list.

## Gate verdict

**PASS.** Every failing test fails for the RIGHT reason (MISSING_FUNCTIONALITY — the
`philosophy show` verb, its record render, and its unknown-name guidance do not exist yet;
clap rejects `show` with exit 2). Zero tests are in category 2 (IMPORT_ERROR /
FIXTURE_BROKEN / SETUP_FAILURE) or category 3 (WRONG_ASSERTION / internal-struct coupling —
every assertion scans the OBSERVABLE CLI stdout / stderr / exit code, never a `lexicon`
struct field). Handoff to DELIVER is UNBLOCKED for slice-23.

## Error/edge ratio note

4 scenarios: PS-1 (WS happy by name) + PS-2 (happy by object id) = 2 happy; PS-3
(unknown-name sad path) + PS-4 (offline edge) = 2 non-pure-happy = **50%** (≥40% target).
AC-002.2 (the unknown-name failure mode) is covered explicitly by PS-3, example-based per
Mandate 11 (no PBT at layer 3).

## DELIVER pointers (from the observed RED)

1. Add a `Show { name_or_object }` variant to the `philosophy` subcommand enum + a
   `verbs/philosophy_show.rs` (mirroring slice-22 `verbs/philosophy_list.rs`, ADR-059 D7)
   that resolves the arg against the embedded seeds by EITHER bare name OR full object id
   (`org.openlore.philosophy.<normalize(name)>`), then prints the name, the full
   `description` verbatim, the `aliases`, and the `seeAlso` links. Offline by construction
   (reads embedded constants; no store, no network). Turns PS-1 / PS-2 / PS-4 GREEN.
2. On an UNKNOWN name/object, exit NON-ZERO with a plain message that names the miss and
   hints the recovery verbs — e.g. `no such philosophy '<input>'; try \`philosophy list\` or
   \`philosophy add\`` — and NEVER panic / leak a backtrace (AC-002.2). Turns PS-3 GREEN.
3. Reuse the slice-22 name→object-id derivation (`object_id`/`normalize`) for the
   object-id-acceptance branch so `show` and the claim-graph join key stay byte-identical
   (no drift). No new seed data, no signer, no store — `show` is a pure read over the 12
   embedded seeds.
4. `xtask check-arch` stays 21 members / no new crate (one CLI verb over the existing
   `lexicon` vocabulary core).
