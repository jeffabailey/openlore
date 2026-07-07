<!-- markdownlint-disable MD013 -->
# RED classification — RGSD-1 (real-github-signal-detection, walking skeleton)

DISTILL wave. Slice RGSD-1 (design §5 walking skeleton): detect
`SignalKind::MemorySafetyLanguage` from the real `/repos/{owner}/{repo}`
`language` field. This file is the pre-DELIVER fail-for-the-right-reason gate
output; DELIVER reads it at PREPARE/RED to confirm the RED is genuine.

## How the run was performed

- **Target language**: Rust (`Cargo.toml` markers; polyglot matrix → proptest /
  std `#[test]` / `#[ignore]`). State-delta port already present at
  `tests/common/state_delta.rs` (inherited — not re-bootstrapped).
- **Layer**: 3 (subprocess acceptance). Example-only per Mandate 11; sad/edge
  path enumerated explicitly, never PBT-generated. Assertions are on the
  observable CLI surface only (exit code + stdout) — no scraper-domain struct
  field is named (Mandate 8 universe = port-exposed CLI output).
- **Driving port**: the real `openlore` binary, spawned via the
  `run_openlore_scrape` harness (`OPENLORE_GITHUB_API_BASE` seam → in-process
  `FakeGithub` HTTP double). Build-before-run honored (`cargo build --bin
  openlore`).

### Gate commands + verdicts

| Gate | Command | Result |
|---|---|---|
| 1a compile | `cargo build -p openlore-test-support` | green (FakeGithub change compiles) |
| 1b compile | `cargo test -p cli --test scrape_real_signal_detection --no-run` | green (AT imports only the support harness — no new production symbol) |
| 1c no-regress compile | `cargo test -p cli --test scrape_candidates --no-run` | green (existing suite still compiles) |
| 1d compile | `cargo test -p openlore-test-support --no-run` | green (new posture unit tests compile) |
| — posture unit | `cargo test -p openlore-test-support fake_github` | 11 passed (incl. `for_public_repo_with_language_serves_language_and_no_signals`, `legacy_signal_posture_serves_null_language`) |
| 2 build | `cargo build --bin openlore` | green |
| 3 run | `cargo test -p cli --test scrape_real_signal_detection -- --test-threads=1` | 1 failed (happy RED), negative + harness state-delta tests green |
| 3 no-regress | `scrape_candidates` (7) · `scrape_github` (11) · `scrape_auth` (7) · `scrape_sign` (11) | all green — additive FakeGithub change caused zero regression |

## RED cause

`adapter-github::harvest_repo` reshapes the `/repos` body via
`client::parse_signals`, which reads a synthetic `signals[]` array the REAL
GitHub API never provides. The new realistic FakeGithub posture serves a body
with a top-level `language` string and NO `signals[]`, so harvest sees **0
signals → 0 candidates**. Observed happy-scenario stdout:

```
Resolving target rust-lang/cargo ... ok (repository)
Harvesting public signals ... 0 signals
No candidate claims could be derived from the harvested signals (nothing to propose).
```

The language-based detection (`parse_repo_facts` + `detect_signals` +
`MEMORY_SAFE_LANGUAGES`, design §2) does not exist yet, so the
`org.openlore.philosophy.memory-safety` candidate is absent. This is genuine
`MISSING_FUNCTIONALITY`: the scrape exits 0 and produces well-formed output,
the test reaches its business assertion (AT line 93), and the assertion fires
because the behavior is unimplemented — not an import/fixture/setup error.

## Per-scenario tally

| Scenario | Tag(s) | Today | Classification |
|---|---|---|---|
| `scrape_repo_whose_language_is_rust_derives_the_memory_safety_candidate` | `@rgsd-1 @walking_skeleton @real-io @driving_port @happy` | **RED** (memory-safety candidate absent) | `MISSING_FUNCTIONALITY` ✅ correct RED. Turns GREEN when DELIVER lands `detect_signals`'s language arm. |
| `scrape_repo_whose_language_is_cpp_proposes_no_memory_safety_candidate` | `@rgsd-1 @real-io @driving_port @edge @guardrail` | **GREEN-today** (no candidate produced regardless) | Guardrail. Load-bearing once detection exists: pins detection is LANGUAGE-gated (C++ ∉ `MEMORY_SAFE_LANGUAGES`, design §2) and must NOT over-fire. Must stay GREEN when the happy scenario turns GREEN. |

- BROKEN: 0 · SETUP_FAILURE: 0 · IMPORT_ERROR: 0 · WRONG_ASSERTION/OBSERVABLE_NOT_AT_PORT: 0.

## Scope note — pure unit RED is DELIVER's responsibility

Per design §7/§8, the pure `detect_signals` and `parse_repo_facts` unit tests
are **NOT authored in this DISTILL RED**. Those functions
(`detect_signals`, `parse_repo_facts`, the `RepoFacts` type,
`MEMORY_SAFE_LANGUAGES`) do not exist yet; referencing them from a Rust unit
test would be a **compile error = BROKEN**, not RED. Rust has no import-stub
scaffold that yields a clean assertion RED for a not-yet-declared symbol (unlike
a Python `__SCAFFOLD__` module). Therefore the DISTILL RED is the
**acceptance (subprocess) test only**; the crafter writes the pure-function
RED_UNIT tests when it introduces those symbols in DELIVER (inner TDD loop).

## Gate verdict

**PASS.** Exactly one genuine RED (`MISSING_FUNCTIONALITY`), one GREEN-today
guardrail, zero BROKEN/SETUP/IMPORT/WRONG-shape failures, zero regression in the
existing scrape suite. RED is genuine and ready to hand to DELIVER.

## DELIVER pointers (design §2/§4)

1. **`scraper-domain`** (PURE): add `RepoFacts { language: Option<String>,
   source_url: String }`, the curated `MEMORY_SAFE_LANGUAGES` const set
   (Rust/Go/Swift/Kotlin/Java/C#/Python/Ruby/Scala/Haskell/Elixir/Erlang/OCaml/
   Clojure — EXCLUDES C/C++), and `detect_signals(&RepoFacts) -> Vec<Signal>`
   (walking skeleton implements ONLY the `MemorySafetyLanguage` arm; case-
   insensitive language match). Emitted `Signal.value` is honest about what was
   measured — the primary language, never "no unsafe" (design §3). Write the
   pure property-test RED_UNIT here first.
2. **`adapter-github`** (EFFECT): add `parse_repo_facts(&Value) -> RepoFacts`
   (reads `language` + `html_url` from the `/repos` body); union the two paths in
   `harvest_repo`: `detect_signals(parse_repo_facts(body))` ∪
   `parse_signals(body)` (§4 legacy bridge), dedup by `SignalKind`. Add the
   `scraper-domain` dependency (adapter → domain, dependencies point inward; no
   cycle). `xtask check-arch` expected to stay green at 21 members.
3. Green order: pure `detect_signals`/`parse_repo_facts` unit RED → GREEN →
   wire the harvest union → the RGSD-1 happy acceptance turns GREEN, the C++
   guardrail stays GREEN.
