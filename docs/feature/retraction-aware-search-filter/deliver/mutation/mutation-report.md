# `retraction-aware-search-filter` Mutation Report — `cargo-mutants` (25.3.1)

Feature: `retraction-aware-search-filter` (ADR-060). Feature commit range
`a2475aa~1..HEAD`. DELIVER Phase 5 — feature-scoped mutation testing.

Tool: **`cargo-mutants 25.3.1`** — available and working. **No `--in-place`**
(mutations built in an out-of-tree scratch copy; the source tree is left
pristine). Secondary files scoped with `--in-diff` against the feature diff so
only feature-introduced lines are mutated (unrelated render code is untouched).

Gate: **≥80% kill rate**. Result: **27 / 27 viable caught = 100%** across the
whole feature (PRIMARY `retraction.rs` = 100%). **PASS.**

---

## 1. PRIMARY — `appview-domain/src/retraction.rs` (the pure correctness core)

The single pure decision `partition_retracted` + its four helper predicates
(`is_own_retraction_marker`, `is_self_retracted`, `self_retraction_events`,
`is_withdrawn`). Gated against the FAST in-crate proptests + example tests
(`retraction::tests`).

Command:

```
cargo mutants -p appview-domain --file crates/appview-domain/src/retraction.rs -- --lib
```

| Outcome | Count |
|---|---|
| caught   | 17 |
| unviable | 2 |
| **survived** | **0** |

**Kill rate (viable) = 17 / 17 = 100%. PASS ≥ 80%.**

### Every high-value mutant caught

The exact operator classes the brief called out are all caught:

- **swap `==`/`!=`** — `is_own_retraction_marker` author check (`117:24`),
  `Retracts` cid match (`119:32`, `119:77`); `is_withdrawn` marker cid match
  (`149:28`). Caught by the example corpora (a self-retraction event must be
  detected by exact author + cid) + `no_self_retracted_original_survives`.
- **swap `&&`/`||`** — `is_own_retraction_marker` author-AND-marker (`118:9`,
  `119:59`); `is_withdrawn` original-OR-marker (`150:13`, `152:27`). Caught by
  the heckler's-veto guard (`third_party_counter_or_different_author_retract_never_hides`
  — `||` would fire on a different-author retract) + the event/marker drop tests.
- **author-check / whole-predicate removal** — `is_own_retraction_marker ->
  {true,false}` (`117:5`), `is_self_retracted -> {true,false}` (`126:5`),
  `is_withdrawn -> {true,false}` (`147:5`). `-> true` over-hides (killed by the
  no-heckler's-veto + identity-shaped corpora); `-> false` never hides (killed by
  `hidden_count_is_events_not_rows` + `no_self_retracted_original_survives`).
- **delete `!`** — `partition_retracted` opt-out guard (`84:8`) and the survivor
  filter negation (`99:23`). Killed by `identity_when_not_hiding` (flipping the
  guard drops/keeps the wrong set) + the survivor-set example asserts.
- **count / event-set removal** — `self_retraction_events -> HashSet::new()`
  (`135:5`). Killed by `hidden_count_is_events_not_rows` +
  `distinct_events_accumulate_the_count` (an empty event set ⇒ `hidden_count == 0`
  and nothing hidden, contradicting the crafted corpora).

### Unviable (not gaps — structural, no `Default` impl)

Both substitute `Default::default()` where the type has no `Default`:

- `partition_retracted -> RetractionPartition::default()` (`84:5`) —
  `RetractionPartition` implements no `Default`.
- `self_retraction_events -> HashSet::from_iter([(Default::default(),
  Default::default())])` (`135:5`) — `Did`/`Cid` implement no `Default`.

They fail to compile and are never testable behavior — a property of the types,
not a coverage hole. No action warranted.

**No tests added to the PRIMARY file — it was 100% on the first pass.**

---

## 2. SECONDARY — CLI disclosure render (`cli/src/render/search.rs`)

The `--hide-retracted` disclosure renderers `render_retraction_disclosure` +
`render_all_retracted_buffer`. `--in-diff`-scoped to the feature lines.

Command:

```
cargo mutants -p cli --file crates/cli/src/render/search.rs \
  --in-diff <feature.diff> --timeout 120 -- --lib
```

(`--timeout 120`: the cli `--lib` suite runs long cold, so the auto-20s test
timeout produced two false `TIMEOUT`s on the first pass; a generous per-mutant
timeout resolved both to `caught`. No subprocess acceptance run needed.)

### Before (no fast unit tier)

The 4 whole-function-replacement mutants (`String::new()` / `"xyzzy".into()` on
each of the two renderers) were covered ONLY by the SLOW subprocess acceptance
suite (`tests/acceptance/search_hide_retracted.rs`). No in-crate unit tier
existed, so a mutation run against the fast `--lib` tests would have surfaced
them as survivors.

### Tests added (2 — fast in-crate render unit tests)

Added `mod retraction_disclosure_tests` to `cli/src/render/search.rs` pinning the
content-frozen disclosure contract at the pure render-function boundary (the
driving port at domain scope):

1. `disclosure_footer_states_the_event_count_and_rerun_guidance` — asserts
   `render_retraction_disclosure(2)` contains `"2 retracted claim(s) hidden"` +
   the re-run guidance. Kills both `390:5` mutants.
2. `all_retracted_buffer_names_the_withdrawn_state_and_count` — asserts
   `render_all_retracted_buffer(3)` contains the `All 3 matching claim(s)`
   framing + `were soft-retracted` + `3 retracted claim(s) hidden` + the re-run
   guidance. Kills both `403:5` mutants.

### After

| Outcome | Count |
|---|---|
| caught   | 4 |
| survived | 0 |

**Kill rate = 4 / 4 = 100%.**

---

## 3. SECONDARY — viewer disclosure render (`viewer-domain/src/search.rs`)

The `FilteredResults`/`AllRetracted` variant selection + the notice/region
renderers + the active-toggle reflection. `--in-diff`-scoped to the feature
lines. Fast in-crate lib tests (no HTTP).

Command:

```
cargo mutants -p viewer-domain --file crates/viewer-domain/src/search.rs \
  --in-diff <feature.diff> --timeout 120 -- --lib
```

### Before (no fast unit tier)

The 6 whole-function-replacement mutants were covered ONLY by the SLOW
subprocess+HTTP acceptance suite (`tests/acceptance/viewer_search_hide_retracted.rs`):
`render_search_page` (`316:5`, ×2), `render_search_form` (`343:5`),
`render_search_result` (`373:5`), `render_retraction_notice` (`423:5`),
`render_all_retracted_region` (`437:5`).

### Tests added (3 — fast in-crate render unit tests)

Added to the existing search section of `viewer-domain/src/tests.rs` (reusing the
`search_result` helper), driving the pure `render_search_*` boundary:

1. `search_filtered_results_discloses_the_hidden_event_count_and_untick_guidance`
   — a `FilteredResults { hidden_count: 3 }` fragment contains `"3 retracted
   claim(s) hidden"` + `"Untick"` + the surviving row. Kills `render_retraction_notice`
   (`423:5`) and the `FilteredResults` arm of `render_search_result` (`373:5`).
2. `search_all_retracted_names_the_withdrawn_state_never_a_blank_region` — an
   `AllRetracted { hidden_count: 2 }` fragment contains `were soft-retracted` +
   `All 2 matching claim(s)` + `2 retracted claim(s) hidden` + `Untick`. Kills
   `render_all_retracted_region` (`437:5`) and the `AllRetracted` arm of
   `render_search_result` (`373:5`).
3. `search_page_reflects_the_active_hide_toggle_for_a_filter_bearing_state` —
   `render_search_page(&AllRetracted{..})` contains `"Network Search"` +
   the hide-retracted label + `checked`. Kills both `render_search_page` (`316:5`)
   String mutants and `render_search_form` (`343:5`, `Default` drops the checked
   checkbox).

### After

| Outcome | Count |
|---|---|
| caught   | 6 |
| survived | 0 |

**Kill rate = 6 / 6 = 100%.**

---

## 4. Verdict

| Surface | File | Viable | Caught | Survived | Kill |
|---|---|---:|---:|---:|---:|
| PRIMARY (pure core) | `appview-domain/src/retraction.rs` | 17 | 17 | 0 | **100%** |
| SECONDARY (cli disclosure) | `cli/src/render/search.rs` | 4 | 4 | 0 | **100%** |
| SECONDARY (viewer disclosure) | `viewer-domain/src/search.rs` | 6 | 6 | 0 | **100%** |
| **Feature total** | | **27** | **27** | **0** | **100%** |

- **PRIMARY `retraction.rs`: 100% (17/17 viable) — the decisive gate, PASS ≥ 80%.**
- **Feature overall: 100% (27/27 viable), 2 unviable (`Default::default()` on
  no-`Default` types — structural, not gaps).**
- 5 fast in-crate render unit tests added (2 cli, 3 viewer) — they add the
  missing fast unit tier for the pure disclosure renderers (previously covered
  only by the slow subprocess/HTTP acceptance suites) and kill all 10 secondary
  mutants without spawning a binary. No PRIMARY test was needed. No existing test
  weakened; no `tests/acceptance/*.rs` modified.
- No `--in-place`; `mutants.out*` scratch and the mutation-induced
  `appview-domain/proptest-regressions/` seeds removed; source tree pristine.

## 5. Post-run verification (all green)

- `cargo test -p appview-domain` → 28 passed
- `cargo test -p viewer-domain` → 145 passed (3 new)
- `cargo test -p cli --lib` → 65 passed (2 new)
- `cargo test -p cli --test search_hide_retracted` → 10 passed
- `cargo test -p cli --test viewer_search_hide_retracted` → 8 passed
- `cargo run -p xtask -- check-arch` → OK (21 workspace members)
</content>
</invoke>
