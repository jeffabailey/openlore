# RGSD-1 Mutation Report — `cargo-mutants` (25.3.1)

Scope: RGSD-1 (commit `62de4a1`). Gated surface is the PURE detector
`crates/scraper-domain/src/detect.rs`; the EFFECT reshape
`crates/adapter-github/src/client.rs::parse_repo_facts` is reported, not gated.

Runner: `cargo-mutants 25.3.1`, **no `--in-place`** (mutations built in an
out-of-tree copy; source tree left pristine).

---

## 1. Gated surface — `scraper-domain/src/detect.rs` (PURE detector)

Command:

```
cargo mutants -p scraper-domain --file crates/scraper-domain/src/detect.rs
```

Killed by the in-crate proptests (`detect::tests`) — no test added, no gap.

| Outcome | Count | Mutants |
|---|---|---|
| caught  | 5 | `is_memory_safe_language -> true`; `is_memory_safe_language -> false`; `detect_signals -> vec![]`; `detect_memory_safety_language -> None`; `delete ! in detect_memory_safety_language` |
| unviable | 2 | `detect_signals -> vec![Default::default()]`; `detect_memory_safety_language -> Some(Default::default())` |
| survived | 0 | — |

### Kill rate (gated)

Viable mutants = caught + survived = 5 + 0 = **5**.
Kill rate = 5 / 5 = **100%**.

**PASS vs the 80% gate** (100% ≥ 80%).

### Unviable analysis (not gaps)

Both unviable mutants substitute `Default::default()` for a `Signal`. `Signal`
(in `ports`) implements no `Default`, so the mutants fail to compile and are
never testable behavior — a structural property of the type, not a coverage
hole. No action warranted.

### Which existing test kills what

- `every_memory_safe_language_any_case_yields_exactly_one_signal` — kills
  `-> false`, `detect_signals -> vec![]`, `detect_memory_safety_language -> None`,
  and the `delete !` mutant (all suppress the signal a memory-safe language must
  fire).
- `a_non_memory_safe_language_yields_no_signal` — kills `-> true` and the
  `delete !` mutant (both would fire on C/C++, the over-firing guard).
- `an_absent_language_yields_no_signal` — reinforces the `None`/empty arms.

---

## 2. Effect surface — `adapter-github/src/client.rs::parse_repo_facts` (reported, not gated)

Command (restricted to the reshape + its URL-fallback helper):

```
cargo mutants --file crates/adapter-github/src/client.rs \
  --re 'parse_repo_facts|repo_url_from_target'
```

### Before (initial run)

| Outcome | Count | Mutants |
|---|---|---|
| caught   | 0 | — |
| unviable | 1 | `parse_repo_facts -> Default::default()` (`RepoFacts` has no `Default`) |
| **survived** | **2** | `repo_url_from_target -> String::new()`; `repo_url_from_target -> "xyzzy".into()` |

**Genuine gap.** The existing reshape test only fed bodies that carry
`html_url`, so the `html_url`-absent fallback arm (`repo_url_from_target`, which
reconstructs the public evidence URL from `target.full_name`) was never
exercised — either constant survived.

### Test added (kill)

Extended the existing example test
`parse_repo_facts_reads_language_and_source_url_from_a_real_body` (no new test
function — same behavior, two added equivalence classes, budget-neutral) with:

1. body with **no** `html_url` but a `target.full_name` ⇒ `source_url` ==
   `https://github.com/rust-lang/cargo` (kills both constant mutants);
2. body with **neither** `html_url` nor `target` ⇒ `source_url` degrades to the
   bare host `https://github.com`.

### After (re-run)

| Outcome | Count | Mutants |
|---|---|---|
| caught   | 2 | `repo_url_from_target -> String::new()`; `repo_url_from_target -> "xyzzy".into()` |
| unviable | 1 | `parse_repo_facts -> Default::default()` |
| survived | 0 | — |

Effect-surface kill rate now 2/2 viable = **100%** (informational — this surface
is not gated).

---

## 3. Verdict

- **Gated pure detector (`detect.rs`): 100% kill (5/5 viable) — PASS ≥ 80%.**
- Effect reshape (`parse_repo_facts`): 2 genuine survivors found and killed by a
  budget-neutral extension of the existing in-crate example test.
- No `--in-place` used; `mutants.out*` scratch and mutation-induced
  `proptest-regressions` removed; source tree pristine.

## 4. Post-run verification (all green)

- `cargo test -p scraper-domain --lib` → 15 passed
- `cargo test -p adapter-github --lib` → 25 passed
- `cargo test -p cli --test scrape_real_signal_detection` → both RGSD-1 scenarios pass (Rust derives, C++ does not)
- `cargo test -p cli --test scrape_candidates` → 7 passed (union-bridge no-regression)
- `cargo run -p xtask -- check-arch` → OK (21 workspace members)
</content>
</invoke>
