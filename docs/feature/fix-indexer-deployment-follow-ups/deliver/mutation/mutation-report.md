# Mutation report: fix-indexer-deployment-follow-ups

**Gate:** per-feature, kill rate >= 80%. **Result: PASS, 100% (11/11 viable mutants caught).**

## Scope

Production Rust changed by this bugfix (`git diff 6316916..HEAD`), restricted to the changed lines (`--in-diff`):

- `crates/openlore-indexer/src/search_handler.rs`: `search_handler`, `handle_search` (store-error reporting), `suggest_object`
- `crates/openlore-indexer/src/run.rs`: `serve_searches` wiring, `emit_search_truncated`, `emit_search_store_error`, `dimension_token`

`config.rs` and `control.rs` changed only inside `#[cfg(test)]` modules (test-only, out of scope). Shell scripts are out of scope (covered by xtask tests).

## Command

```
git diff 6316916..HEAD -- crates/openlore-indexer/src/search_handler.rs crates/openlore-indexer/src/run.rs > diff.patch
CARGO=cargo-wrap.sh cargo-mutants mutants -j 2 \
  --file crates/openlore-indexer/src/search_handler.rs --file crates/openlore-indexer/src/run.rs \
  --in-diff diff.patch --timeout 600 \
  -- --bins --test indexer_deployment_public_surface --test indexer_deployment_passes
```

cargo-mutants 25.3.1 ignored `--test-package` / `--test-workspace` here and always tested only `openlore-indexer`, whose unit tests cannot reach the `run.rs` log emitters (the indexer has no lib target; its acceptance suites live in the `cli` package and spawn the binary). A throwaway `CARGO` wrapper therefore rewrote `--package=openlore-indexer` to `-p openlore-indexer -p cli` and, in the build phase, also ran `cargo build -p openlore-indexer --bin openlore-indexer` so the acceptance suites spawn the mutated binary. Test set per mutant: openlore-indexer unit tests (46) + `indexer_deployment_passes` (20) + `indexer_deployment_public_surface` (9). Baseline: 67s build + 79s test, green.

## Results

| Outcome | Count |
|---|---|
| Caught | 11 |
| Missed | 0 |
| Unviable | 2 |
| Timeout | 0 |
| **Kill rate** | **11/11 = 100%** |

### Per file

| File | Caught | Missed | Unviable |
|---|---|---|---|
| search_handler.rs | 4 | 0 | 2 |
| run.rs | 7 | 0 | 0 |

### Caught

- search_handler.rs:151 delete match arm `(SearchDimension::Object, true)`
- search_handler.rs:172 `suggest_object` -> `Ok(None)` / `Ok(Some(String::new()))` / `Ok(Some("xyzzy"))`
- run.rs:454 `serve_searches` -> 0 / 1 / -1
- run.rs:526 `emit_search_truncated` -> `()`
- run.rs:537 `emit_search_store_error` -> `()`
- run.rs:545 `dimension_token` -> `""` / `"xyzzy"`

### Unviable (do not compile; not counted)

- search_handler.rs:60 `search_handler` -> `Default::default()` (`QueryHandler` is an `Arc<dyn Fn>`, no `Default`)
- search_handler.rs:125 `handle_search` -> `Ok(Default::default())` (`SearchQueryResponse` has no `Default`)

## Survivors

None. No tests added.

## Post-run safety

cargo-mutants mutates a temporary copy of the tree; the working tree was verified clean (`git status --short` empty) after the run.
