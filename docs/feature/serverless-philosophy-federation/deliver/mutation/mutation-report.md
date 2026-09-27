# Serverless Philosophy Federation — Mutation Report (pure publish core)

**Date:** 2026-09-27
**Tool:** cargo-mutants 25.3.1 (no `--in-place`; `--timeout 120 -j 2`)
**Gate:** kill rate = (caught + timeout) / viable (unviable excluded). ≥ 80% PASS · 70–80% WARN · < 70% FAIL.
**Verdict:** **PASS — 100% (55/55) on the gated surfaces after the killing tests** (baseline 81.8%, with publish-domain alone at 76.7% WARN).

## Commands

```
cargo test -p publish-domain -p lexicon                                            # pre-check: green
cargo mutants -p publish-domain --file crates/publish-domain/src/lib.rs --timeout 120 -j 2   # gated
cargo mutants -p lexicon        --file crates/lexicon/src/claim.rs      --timeout 120 -j 2   # gated
cargo mutants -p ports          --file crates/ports/src/publish.rs      --timeout 120 -j 2   # optional
cargo mutants -p cli            --file crates/cli/src/render/publish.rs --timeout 180 -j 2   # optional (baseline failed, see below)
```

## Tally (per file)

| File | Total | Caught | Timeout | Missed | Unviable | Viable | Kill rate |
|---|---|---|---|---|---|---|---|
| `publish-domain/src/lib.rs` (baseline) | 69 | 33 | 0 | 10 | 26 | 43 | 76.7% (WARN) |
| `publish-domain/src/lib.rs` (after tests) | 69 | **43** | 0 | **0** | 26 | 43 | **100%** |
| `lexicon/src/claim.rs` | 16 | 12 | 0 | 0 | 4 | 12 | **100%** |
| `ports/src/publish.rs` (optional, baseline) | 4 | 0 | 0 | 3 | 1 | 3 | 0% in the scoped run |
| `ports/src/publish.rs` (optional, after test) | 4 | **3** | 0 | **0** | 1 | 3 | **100%** |
| `cli/src/render/publish.rs` (optional) | 25 | — | — | — | — | — | not run (baseline failed) |

**Gated kill rate:** baseline (33 + 12) / (43 + 12) = 45/55 = **81.8%**. After the killing tests, (43 + 12) / 55 = **100%**. Both clear the 80% gate. The file-level WARN on publish-domain is cleared.

## Survivor analysis

None of the survivors was equivalent. Each one was a real gap and now has a killing property test.

### publish-domain — 10 MISSED, all killed

| Mutant(s) | Gap | Killed by (`crates/publish-domain/src/tests.rs`) |
|---|---|---|
| `497:9 in_sync -> true`, `497:9 in_sync -> false`, `497:18/41/63 == → !=`, `497:23/46 && → \|\|` (7) | Existing properties only checked that the tally *sums* to the pulled count. Nothing checked `ReconcileTally::in_sync`, and the tally-to-in-sync link is only shown by the CLI renderer. | `the_tally_counts_each_outcome_and_is_in_sync_iff_all_matched`. The tally equals the per-variant counts of `reconcile`'s outcomes, and `in_sync()` ⇔ every outcome is `Matched`. |
| `231:9 ReadbackMismatch::describe -> String::new()` / `-> "xyzzy".into()` (2) | The push-report refusal reason was never checked. | `a_readback_refusal_describes_its_reason`. Each variant names its own reason (the unreadable detail, the drifted CID, "altered"), and the three reasons are distinct. |
| `299:5 manifest_cids -> vec![]` (1) | Only the CLI push planner uses it, so no test in this crate pinned it. | `manifest_cids_lists_every_entry_cid_in_order`. A manifest of `display_projection`s lists the pushed claims' CIDs in order. |

### lexicon `claim.rs` — 0 survivors

All 12 viable mutants were caught by the existing lexicon tests. No test added.

### ports `publish.rs` (optional) — 3 MISSED, all killed

| Mutant(s) | Gap | Killed by |
|---|---|---|
| `111:9 InstanceError::reason_code -> None / Some("") / Some("xyzzy")` | The `adapter-publish-http` integration tests already catch these across crates (`opaque_instance_http.rs` checks `reason_code` in the probe refusal payload and `refused.reason_code()`). A scoped `-p ports` run does not see those tests. | `ports::publish::tests::each_refusal_failure_carries_its_dotted_reason_code`. This pins the dotted `publish.*` code for every `InstanceError` variant inside the crate. |

### cli `render/publish.rs` (optional) — not measured

cargo-mutants stopped with `cargo test failed in an unmutated tree`. In the scratch copy, 21 tests in `-p cli --test appview_search` fail on the baseline. These tests depend on the environment and fail in the copied tree for reasons unrelated to this feature, so no mutants were tested. The renderers are exercised by the workspace acceptance suite (`tests/acceptance`), which is a separate package from `-p cli`. This surface was optional, so it is not counted toward the gate.

## Conclusion

- **publish-domain:** 76.7% → **100%** (43/43). 3 property tests added.
- **lexicon `claim.rs`:** **100%** (12/12). No change.
- **Gated overall:** **100%** (55/55). **PASS** against the 80% gate.
- **Optional ports `publish.rs`:** **100%** (3/3) after 1 contract test. The cli renderer was not measurable because of a baseline failure in the scratch tree.
- No equivalent mutants. No `mutants.out*` artifacts committed.
