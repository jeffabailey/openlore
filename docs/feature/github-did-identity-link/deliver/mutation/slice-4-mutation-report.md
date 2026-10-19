# github-did-identity-link slice 4: mutation report

DELIVER Phase 5, per-feature mutation testing (gate: at least 80% of viable
mutants caught). Run on 2026-10-19 against `3ccd45a` (slice-4 steps
04-01..04-09, refactor `c85f2ce`, review fixes `e84a5d8` and `3ccd45a`).

**Result: 65 of 65 viable mutants caught (100%). PASS.**
The first pass caught 63 of 65 (96.9%). Two tests were added to kill the two
real survivors.

## Command

cargo-mutants 25.3.1 ignores tests that live in another package, and most
slice-4 behavior is covered by the `cli` acceptance targets. So, as in slices
1 to 3, the mutants were listed by cargo-mutants and run by a manual runner:

```
git diff 56964e0..HEAD -- <scope files> > slice4.diff
cargo mutants --list --in-diff slice4.diff --json --diff > mutants.json
```

For each mutant, the runner applied its diff in one of three scratch git
worktrees of HEAD (each with its own `CARGO_TARGET_DIR`; the main checkout was
never mutated). It then ran:

1. `cargo test --no-run` for the owning package, `cargo build --bin openlore-indexer`,
   and `cargo test --no-run -p cli --test <targets>`. A compile failure means
   **unviable**.
2. The owning crate's tests, then the acceptance targets, stopping at the first
   failure (**caught**). Anything else is **missed**. Time limit: 1500 s per
   command (no mutant hit it).

| Owning crate | Unit run | Acceptance targets (`-p cli`) |
|---|---|---|
| viewer-domain, adapter-http-viewer | `cargo test -p <crate>` | viewer_linked_identity, viewer_linked_identity_invariants, viewer_contributor_scoring, viewer_graph_traversal, viewer_counter_claim_threads, viewer_network_search, viewer_htmx |
| ports | `cargo test -p ports` | viewer_linked_identity, identity_link_readers, identity_link, infer_people, viewer_linked_identity_invariants |
| adapter-duckdb | `cargo test -p adapter-duckdb --lib --test link_candidates` | same as ports |
| cli | `cargo test -p cli --lib --bins` | same as ports, plus viewer_htmx for `ui.rs` |
| xtask | `cargo test -p xtask` | none |

Scope: `crates/viewer-domain/src/{linked_identity,score,traversal,detail,search,lib}.rs`,
`crates/adapter-http-viewer/src/lib.rs`,
`crates/ports/src/{identity_link_check,identity_link_listing}.rs`,
`crates/adapter-duckdb/src/identity_link_checks.rs`,
`crates/cli/src/verbs/{ui,identity_reads,identity_list,identity_link,infer_people}.rs`,
and the slice-4 hunks of `xtask/src/check_arch.rs`. Tests and
review-app-repos-and-accounts files are not in scope.

Baseline: every target passed on unmutated HEAD in the scratch worktree. The
targets that touch `/search` need the `openlore-indexer` binary, so the runner
builds it for each mutant.

## Totals

| Outcome | Count |
|---|---|
| caught | 65 (63 in the first pass, plus 2 after the new tests) |
| missed | 0 (2 in the first pass) |
| unviable | 87 |
| timeout | 0 |
| **total listed** | **152** |

**Kill rate = 65 / (65 + 0) = 100%.** First pass: 63 / 65 = 96.9%.

Most unviable mutants replace a return value with `Default::default()` where
the type has no `Default` (`LinkCandidate`, `LinkedIdentities`, `Response`
bodies built from `Default`, `StoredLinkLineage`, `ClaimLineage`, etc.). They
do not compile, so they are not test gaps. One unviable mutant (#49) is
explained below.

## Per-file breakdown (final)

| File | Caught | Missed | Unviable |
|---|---|---|---|
| crates/adapter-http-viewer/src/lib.rs | 17 | 0 | 66 |
| crates/viewer-domain/src/linked_identity.rs | 11 | 0 | 5 |
| crates/viewer-domain/src/search.rs | 11 | 0 | 0 |
| crates/viewer-domain/src/detail.rs | 3 | 0 | 0 |
| crates/viewer-domain/src/score.rs | 3 | 0 | 0 |
| crates/viewer-domain/src/traversal.rs | 3 | 0 | 0 |
| crates/ports/src/identity_link_listing.rs | 6 | 0 | 10 |
| crates/adapter-duckdb/src/identity_link_checks.rs | 2 | 0 | 4 |
| crates/cli/src/verbs/ui.rs | 5 | 0 | 1 |
| crates/cli/src/verbs/identity_reads.rs | 1 | 0 | 1 |
| xtask/src/check_arch.rs | 3 | 0 | 0 |

No mutants were generated for `viewer-domain/src/lib.rs`,
`ports/src/identity_link_check.rs`, `cli/src/verbs/identity_list.rs`,
`identity_link.rs` or `infer_people.rs`. Their slice-4 lines are imports,
re-exports or call-site arguments, which cargo-mutants does not mutate.

Mutants worth noting that were caught: the `of_author` bare-DID comparison
`== -> !=`, `score_contributor_did -> None/""/"xyzzy"`, the
`identity_links_read_with_fault_seam` arm deletions (`checks`, `candidates`,
`lineage`), `has_author_groups -> false`, `link_row_label -> None/""/"xyzzy"`,
`probed_link_reads -> None` and its arm deletion, every
`StoredLinkLineage`/`read_listed_links` emptying, and all three
`scan_identity_link_readers_hold_no_write_handle` replacements.

## Missed in the first pass, and how each was resolved

1. **#25, `adapter-http-viewer/src/lib.rs:503`, `route`: match guard
   `!cid.is_empty()` replaced with `true`.** This is a real gap, though not new
   behavior: `GET /claims/` (an empty CID) must be the terse route-miss 404, not
   a call into the claim detail handler. Nothing tested the empty segment.
   **Resolved:** added the in-crate unit test
   `route_answers_an_empty_claim_cid_with_the_route_miss_404`. It sends a real
   HTTP request to a bound `ViewerServer` and checks for `404` and the
   `<p>Not found.</p>` body. With the mutant applied, the test fails. **Caught.**

2. **#135, `viewer-domain/src/search.rs:269`, `SearchState::has_author_groups`
   replaced with `true`.** This is a real gap. The shell would read the links
   for the form, no-results, unavailable and all-retracted states, which goes
   against D-VL-10 ("only when there are groups"). Nothing would show on the
   page, because no author group exists to hold a region, so only the extra
   read changes. **Resolved:** added the pure unit test
   `only_search_states_with_author_groups_ask_for_linked_identities` in
   `viewer-domain/src/tests.rs`. It checks the predicate for all six variants.
   With the mutant applied, the test fails. **Caught.**

3. **#49, `adapter-http-viewer/src/lib.rs:952`: the release-only
   `identity_links_read_with_fault_seam` replaced with
   `Ok(Default::default())`.** This function is
   `#[cfg(not(debug_assertions))]`, so the debug test build never compiles it
   and the mutant cannot fail a test there. In the release build, where the
   function exists, the mutant does not compile:
   `cargo check --release -p adapter-http-viewer` fails with
   `E0277: LinkedIdentities: Default is not satisfied`. **Reclassified as
   unviable.** No test is needed.

## Acceptance gaps for the orchestrator

None. Both survivors were pinned at the crate level. #25 is an older route
rule, and #135 is observable only as a read count, which the acceptance layer
does not measure. Neither needs a new acceptance scenario.

## Post-mutation safety

- All three scratch worktrees were removed with `git worktree remove --force`.
  The main checkout was never mutated; its only changes are the two new tests
  and this report.
- On HEAD plus the new tests, `cargo test -p viewer-domain -p adapter-http-viewer`
  and the slice-4 acceptance targets pass (see the commit that adds this report).
