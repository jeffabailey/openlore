<!-- markdownlint-disable MD013 -->
# RED Classification — slice-22 (philosophy-vocabulary-registry-seed-and-list)

> DISTILL Pre-DELIVER fail-for-the-right-reason gate (nw-distill §"Pre-DELIVER
> fail-for-the-right-reason gate"). Every slice-22 acceptance scenario + the two
> in-crate `validate_philosophy_json` unit tests were run once against the CURRENT
> (unimplemented) production code and classified. DELIVER reads this file at the
> RED-phase entry gate (ADR-025 D2) to confirm RED is genuine.
>
> Owner: Quinn (nw-acceptance-designer) · 2026-07-05 · Rust / cucumber-free
> subprocess acceptance shape (mirrors slice-04/20/21) + layer-2 in-crate validator.
> Scope: US-PV-001 (AC-001.1..4) + `validate_philosophy_json` accept/reject arms ONLY.
> Slices 23–28 (show / mint / compose-advisory / alias / viewer / scraper) are OUT.

## How the run was performed

```
cargo test -p cli --test philosophy_vocabulary --no-run       # COMPILE gate (BROKEN check)
cargo build --bin openlore                                    # build-before-run (the AT spawns the real bin)
cargo test -p cli --test philosophy_vocabulary -- --test-threads=1
cargo test -p lexicon philosophy_validator_tests -- --test-threads=1
```

The acceptance target COMPILES green (`--no-run` → `Finished`; warnings only, all from
the shared `support` harness, none from `philosophy_vocabulary.rs`). It spawns the real
`openlore` bin via the existing `run_openlore` / `run_openlore_network_disabled` support
harness and imports only that harness + `serde_json` — no new production symbol. Therefore
every acceptance failure is a RUNTIME assertion, not a compile/import error → RED, never
BROKEN. The lexicon tests reference only the EXISTING scaffold signature
(`validate_philosophy_json(&Value) -> Result<Philosophy, LexiconError>`) + the schema-frozen
`description` field, so they compile against both the current and the ADR-059-reconciled
struct.

## What is missing today (the RED cause)

- **No `philosophy` CLI verb.** `openlore philosophy list` is rejected by clap:
  `error: unrecognized subcommand 'philosophy'` → **exit 2** (observed `left: 2, right: 0`).
  Every PV-* scenario asserts `status == 0` FIRST, so all six fail on that assertion →
  MISSING_FUNCTIONALITY (no verb + no embedded seeds), never a harness error.
- **`validate_philosophy_json` panics.** Its body is `panic!("Not yet implemented -- RED
  scaffold")` (lib.rs:101), and the `Philosophy` struct is the stale `{id, label,
  description}` shape (ADR-059 D1 reconciles it to `{name, description, aliases, seeAlso}`).
  Both in-crate tests fail via that panic BEFORE reaching their assertions →
  MISSING_FUNCTIONALITY.

## Classification key

- **RED (MISSING_FUNCTIONALITY, assertion)** ✅ — the `status == 0` assertion fires because
  the `philosophy list` verb + embedded seeds are unimplemented (clap exit 2). Correct RED.
- **RED (MISSING_FUNCTIONALITY, scaffold panic)** ✅ — `validate_philosophy_json` panics
  ("Not yet implemented -- RED scaffold"). Correct RED (Rust `panic!` = RED).
- **BROKEN / SETUP / IMPORT** ❌ — would block handoff. **NONE remain.**

## Tally

| File | Scenario | AC | Classification | Why |
|---|---|---|---|---|
| `philosophy_vocabulary.rs` | PV-1 `philosophy_list_prints_each_seed_object_id_name_and_description` (WS) | AC-001.1 | RED ✅ | `unrecognized subcommand 'philosophy'` → exit 2; no verb, no seeds |
| | PV-2 `the_seed_set_contains_at_least_ten_well_known_philosophies` | AC-001.2 / KPI-PV-1 | RED ✅ | same — verb absent; no ≥10 seed ids to count |
| | PV-3 `each_seed_object_id_matches_the_slice_one_claim_object_bytes` | AC-001.1 / ADR-059 D1 | RED ✅ | same — no derived `org.openlore.philosophy.<name>` id emitted |
| | PV-4 `philosophy_list_json_emits_each_record_with_name_and_description` | AC-001.3 | RED ✅ | same — `--json` verb absent; no JSON array to parse |
| | PV-5 `philosophy_list_defaults_to_human_text_not_json` | AC-001.3 | RED ✅ | same — no text-default view exists |
| | PV-6 `philosophy_list_succeeds_with_the_network_disabled` | AC-001.4 / I-9 | RED ✅ | same (network-disabled) — no offline seed render |
| `crates/lexicon/src/lib.rs` | `validates_well_formed_philosophy_record` (accept arm) | AC-001.2 / DoD-2 | RED ✅ | `validate_philosophy_json` panics ("RED scaffold") |
| | `rejects_missing_description_with_named_field_error` (reject arm) | AC-003.4 / DoD-2 | RED ✅ | validator panics before returning the `MissingField{description}` reject |

### Numeric summary (slice-22 scenarios only; excludes the 2 pre-existing `state_delta` framework self-tests per acceptance binary)

| Classification | Count |
|---|---|
| RED — MISSING_FUNCTIONALITY (assertion, clap exit 2) | 6 |
| RED — MISSING_FUNCTIONALITY (scaffold panic) | 2 |
| GREEN-today (no-regression guardrail) | 0 |
| **BROKEN / SETUP / IMPORT** | **0** |
| **Total slice-22 tests** | **8** |

RED total = **8** (6 acceptance assertion-RED + 2 validator scaffold-panic RED). Zero
GREEN-today, zero BROKEN. Observed runner output: acceptance `2 passed; 6 failed` (the 2
passes are the `support::state_delta` framework self-tests, NOT slice-22 scenarios); lexicon
`0 passed; 2 failed; 29 filtered out`.

## Gate verdict

**PASS.** Every failing test fails for the RIGHT reason (MISSING_FUNCTIONALITY — the
`philosophy list` verb + embedded seeds do not exist yet; `validate_philosophy_json`
panics). Zero tests are in category 2 (IMPORT_ERROR / FIXTURE_BROKEN / SETUP_FAILURE) or
category 3 (WRONG_ASSERTION / internal-struct coupling — every acceptance assertion scans
the OBSERVABLE CLI stdout / exit code, never a `lexicon` struct field; the two validator
tests reference only the public `validate_philosophy_json` result + the schema-frozen
`description` field). Handoff to DELIVER is UNBLOCKED for slice-22.

## Error/edge ratio note

6 acceptance scenarios: PV-1/2/4 happy (3), PV-3 backward-compat guardrail + PV-5
text-default edge + PV-6 offline edge (3 non-pure-happy = 50%). The genuine sad-path
surface of a single read-only, offline, embedded-seed `list` verb is small (unknown-name /
mint-collision belong to `philosophy show`/`add` — slices 23/24, explicitly OUT of scope);
the reject ARM of the vocabulary lives at layer 2 in `rejects_missing_description_...`.

## DELIVER pointers (from the observed RED)

1. Reconcile `lexicon::philosophy::Philosophy` to `{name, description, aliases: Vec<String>
   (default), see_also: Vec<String> (serde rename "seeAlso", default)}` and implement
   `validate_philosophy_json` as a per-field-gated pure fn mirroring `validate_claim_json`
   (required `name`/`description` → `LexiconError::MissingField`; array-of-string checks →
   `InvalidType`; serde catch-all → `SchemaMismatch`). Turns both lexicon tests GREEN.
2. Ship ≥10 embedded seed records (`include_str!("seeds.json")` per ADR-059 D3) incl. the
   six named well-known philosophies; a compile-adjacent test validates each through
   `validate_philosophy_json` (KPI-PV-1) and asserts no two seed names collide under
   `normalize` (ADR-059 D1).
3. Add `Command::Philosophy(PhilosophyCommand { List { json } })` + `verbs/philosophy_list.rs`
   (ADR-059 D7) that reads the embedded seeds and prints, per seed, the derived object id
   `object_id(normalize(name))` + name + one-line description (text default; `--json` opt-in
   emits the array of `{name, description, ...}` records). Offline by construction. Turns
   PV-1..6 GREEN.
4. `xtask check-arch` stays 21 members / no new crate (pure record + validator + seed data
   in `lexicon`; one CLI verb).
