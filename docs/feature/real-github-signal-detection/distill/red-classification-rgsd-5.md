<!-- markdownlint-disable MD013 -->
# RED classification — RGSD-5 (real-github-signal-detection, CI workflows / tests dir)

DISTILL wave. Slice RGSD-5 (design §5) — the FIFTH and last detector. Detect
`SignalKind::TestRatioOrCiMatrix` from test-infrastructure evidence — the
DISJUNCTION of EITHER (a) the repo has CI workflows (`GET
/repos/{o}/{r}/contents/.github/workflows` → 200) OR (b) a `tests/` directory
(`GET /repos/{o}/{r}/contents/tests` → 200). The "test-driven" heuristic. Both
probes REUSE RGSD-2's `content_exists` `contents/*` fork (200 = present, 404 =
absent) — NO new endpoint type. This file is the pre-DELIVER
fail-for-the-right-reason gate output; DELIVER reads it at PREPARE/RED to confirm
the RED is genuine.

## How the run was performed

- **Target language**: Rust (`Cargo.toml` markers; polyglot matrix → proptest /
  std `#[test]` / `#[ignore]`). State-delta port already present at
  `tests/common/state_delta.rs` (inherited — not re-bootstrapped).
- **Layer**: 3 (subprocess acceptance). Example-only per Mandate 11; sad/edge
  path enumerated explicitly, never PBT-generated. Assertions are on the
  observable CLI surface only (exit code + stdout) — no scraper-domain /
  adapter-github struct field is named (Mandate 8 universe = port-exposed CLI
  output).
- **Driving port**: the real `openlore` binary, spawned via the
  `run_openlore_scrape` harness (`OPENLORE_GITHUB_API_BASE` seam → in-process
  `FakeGithub` HTTP double). Build-before-run honored (`cargo build --bin
  openlore`).
- **New endpoint added**: ZERO new endpoint TYPES — the RGSD-5 harvest will
  REUSE RGSD-2's `content_exists(owner, repo, path)` for BOTH probes
  (`contents/.github/workflows` and `contents/tests`; 200 = dir present, 404 =
  absent). SPIKE-verified against real GitHub (ripgrep: `contents/.github/workflows`
  → 200 AND `contents/tests` → 200, fires via EITHER disjunct;
  octocat/Hello-World: 404/404, a clean negative). The `FakeGithub` double was
  extended additively to serve those two paths as 200 dir-listing arrays when
  configured, else the existing default 404.

### FakeGithub CI-workflows + tests-dir `contents` extension (additive; no-regression)

- Added `has_ci_workflows: bool` + `has_tests_dir: bool` posture fields, plus
  constructors `FakeGithub::for_public_repo_with_test_evidence(target,
  has_ci_workflows, has_tests_dir)` (the general builder),
  `FakeGithub::for_public_repo_with_ci_workflows(target)` (convenience: CI
  workflows present, no tests dir), and
  `FakeGithub::for_public_repo_with_tests_dir(target)` (convenience: tests dir,
  no CI). `language` None, `has_cargo_lock` false, `tags` empty, `has_changelog`
  false, `readme_bytes` None, `has_docs_dir` false on all three, so ONLY
  test-driven can fire (isolating it from RGSD-1 memory-safety + RGSD-2
  dependency-pinning + RGSD-3 semver/CHANGELOG + RGSD-4 documentation-first).
- The `/contents/` fork's `present` match gained two arms:
  `".github/workflows" => has_ci_workflows` and `"tests" => has_tests_dir`. Both
  are DIRECTORIES → the handler serves a 200 JSON ARRAY dir-listing (the real
  GitHub `contents` shape for a directory, carrying an `html_url`), reusing the
  RGSD-4 dir-array shape (`docs` | `.github/workflows` | `tests` now share the
  `is_directory` branch, generalized to key off `file_path`).
- **`.github/workflows` path-match note**: the probed path carries a dot AND a
  slash. The router matches on the FULL suffix after `/contents/` (the extracted
  `file_path` string, e.g. `".github/workflows"`), NOT the last path segment — so
  the dot+slash path resolves correctly. A unit test drives this exact URL and
  asserts the 200 array.
- **Default-404 CI guarantee**: any posture that does not set `has_ci_workflows`
  (all existing ones) → `contents/.github/workflows` → 404. **Default-404 tests
  guarantee**: any posture that does not set `has_tests_dir` → `contents/tests` →
  404. Together these make the production probe read EVERY existing posture as "no
  CI workflows AND no tests dir", so no `TestRatioOrCiMatrix` signal can fire on
  them — the no-regression guarantee.
- The repo resolve/harvest body (`GET /repos/{o}/{r}`), `/tags`, `/readme`, and
  every other `contents/*` path are UNCHANGED. All existing constructors are
  untouched (the two new fields default to `false` in every existing State
  literal; `authenticated` preserves `prev.has_ci_workflows`/`prev.has_tests_dir`).
- 3 new posture unit tests pin the new surface (CI-workflows 200 array + tests
  404; tests-dir 200 array + CI 404; unconfigured → both 404, mirroring
  octocat/Hello-World).

### Gate commands + verdicts

| Gate | Command | Result |
|---|---|---|
| 1a compile | `cargo build -p openlore-test-support` | green (FakeGithub CI/tests `contents` change compiles) |
| 1b compile | `cargo test -p cli --test scrape_test_ci --no-run` | green (AT imports only the support harness — no new production symbol) |
| 1d compile+unit | `cargo test -p openlore-test-support fake_github` | 26 passed (incl. the 3 new CI/tests posture tests; was 23) |
| 2 build | `cargo build --bin openlore` | green |
| 3 run | `cargo test -p cli --test scrape_test_ci -- --test-threads=1` | happy A + happy B RED (panic at AT:120 / AT:216, test-driven candidate absent — "0 signals"), negative GREEN-today (3 passed / 2 failed — negative guardrail + 2 bundled state_delta unit tests) |
| 3 no-regress | `scrape_docs_substantial` (5, RGSD-4 SHIPPED) · `scrape_semver_changelog` (5, RGSD-3) · `scrape_dependency_pinning` (4, RGSD-2) · `scrape_real_signal_detection` (4, RGSD-1) · `scrape_candidates` (7) · `scrape_github` (11) · `scrape_sign` (11) | all green — additive FakeGithub change caused zero regression (47 scenarios) |

## RED cause

`adapter-github::harvest_repo` today fetches `GET /repos/{o}/{r}` (RGSD-1
language), probes `contents/Cargo.lock` (RGSD-2), lists `/tags`, probes
`contents/CHANGELOG.md` (RGSD-3), fetches `/readme`, and probes `contents/docs`
(RGSD-4). It NEVER probes `contents/.github/workflows` nor `contents/tests`, so no
`TestRatioOrCiMatrix` signal is produced → no
`org.openlore.philosophy.test-driven` candidate. Both happy-scenario assertions
(AT lines 120 + 216) fire because the behavior is unimplemented — the scrape exits
0 and produces well-formed output ("Harvesting public signals ... 0 signals" / "No
candidate claims could be derived"), each test reaches its business assertion, and
the candidate is simply absent. This is genuine `MISSING_FUNCTIONALITY`: not an
import/fixture/setup error.

## Per-scenario tally

| Scenario | Tag(s) | Today | Classification |
|---|---|---|---|
| `scrape_repo_with_ci_workflows_derives_the_test_driven_candidate` | `@rgsd-5 @real-io @driving_port @happy` | **RED** (test-driven candidate absent) | `MISSING_FUNCTIONALITY` ✅ correct RED. Exit-0 assertion passes; the candidate assertion (AT:120) fires. Turns GREEN when DELIVER lands the `content_exists(contents/.github/workflows)` probe + `RepoFacts.ci_workflows_url` + `detect_signals`'s `TestRatioOrCiMatrix` arm. Pins the CI-workflows disjunct (mirrors real ripgrep). |
| `scrape_repo_with_a_tests_directory_derives_the_test_driven_candidate` | `@rgsd-5 @real-io @driving_port @happy` | **RED** (test-driven candidate absent) | `MISSING_FUNCTIONALITY` ✅ correct RED. Exit-0 assertion passes; the candidate assertion (AT:216) fires. Turns GREEN when DELIVER lands the `content_exists(contents/tests)` probe + the `TestRatioOrCiMatrix` arm. Pins the OR: a `tests/` dir ALONE fires even with no CI workflows. |
| `scrape_repo_with_neither_ci_nor_tests_proposes_no_test_driven_candidate` | `@rgsd-5 @real-io @driving_port @edge @guardrail` | **GREEN-today** (no candidate produced regardless) | Disjunction-guard (under-firing side). Load-bearing once detection exists: a repo with neither CI workflows nor a tests/ dir must NOT fire (mirrors real octocat/Hello-World 404/404). Must stay GREEN when the happy scenarios turn GREEN. |

- BROKEN: 0 · SETUP_FAILURE: 0 · IMPORT_ERROR: 0 · WRONG_ASSERTION/OBSERVABLE_NOT_AT_PORT: 0.

## Scope note — pure/effect unit RED is DELIVER's responsibility

Per design §7/§8, the pure/effect unit tests are **NOT authored in this DISTILL
RED**. The `RepoFacts.{ci_workflows_url, tests_dir_url}` fields and the
`TestRatioOrCiMatrix` arm of `detect_signals` do NOT exist yet; referencing any of
them from a Rust unit test would be a **compile error = BROKEN**, not RED. Rust has
no import-stub scaffold that yields a clean assertion RED for a not-yet-declared
symbol (unlike a Python `__SCAFFOLD__` module). Therefore the DISTILL RED is the
**acceptance (subprocess) test only**; the crafter writes the pure/effect RED_UNIT
tests (`parse_repo_facts` reading `ci_workflows_url`/`tests_dir_url`, and the
`TestRatioOrCiMatrix` detect arm's disjunction table:
`ci_workflows_url.is_some() || tests_dir_url.is_some()`) when it introduces those
symbols in DELIVER (inner TDD loop). The effect fetches REUSE RGSD-2's existing
`content_exists(owner, repo, path)` — no new effect signature is introduced.

## Gate verdict

**PASS.** Exactly two genuine REDs (`MISSING_FUNCTIONALITY` — the two happy
disjunct scenarios), one GREEN-today disjunction-guardrail, zero
BROKEN/SETUP/IMPORT/WRONG-shape failures, zero regression in the existing scrape
suite (47 scenarios across 7 suites stay green, RGSD-1 + RGSD-2 + RGSD-3 + RGSD-4
SHIPPED included; plus 26 FakeGithub posture unit tests). RED is genuine and ready
to hand to DELIVER.

## DELIVER pointers (design §2/§5)

1. **`adapter-github`** (EFFECT): REUSE RGSD-2's `content_exists(owner, repo,
   path) -> Result<Option<String>, GithubError>` for BOTH new probes — issue
   `content_exists(owner, repo, ".github/workflows")` and `content_exists(owner,
   repo, "tests")`, mapping **200** → `Some(html_url)` (dir present) / **404** →
   `None` (absent). NO new endpoint type, NO new effect signature. Extend
   `RepoFacts` / `parse_repo_facts` with `ci_workflows_url: Option<String>` +
   `tests_dir_url: Option<String>`. `harvest_repo` calls both probes and assembles
   the fuller `RepoFacts` before `detect_signals`.
2. **`scraper-domain`** (PURE): add the `TestRatioOrCiMatrix` arm to
   `detect_signals` — fires one `TestRatioOrCiMatrix` signal on the DISJUNCTION
   `ci_workflows_url.is_some() || tests_dir_url.is_some()`, with the workflows (or
   tests) `html_url` as its `source_url` and an honest `value` naming what was
   measured (design §3: "CI workflows present" / "tests/ directory present" — the
   emitted signal never claims the deferred "test/source ratio > 0.5" refinement,
   which needs a full recursive tree walk and is OUT of scope for the walking
   skeleton). Write the pure `detect_signals` arm + `parse_repo_facts` RED_UNIT
   here first. The mapping SSOT (`signal_predicate_mapping.yaml` →
   `org.openlore.philosophy.test-driven`) is UNCHANGED (seeded).
3. Green order: pure `detect_signals` `TestRatioOrCiMatrix` arm +
   `ci_workflows_url`/`tests_dir_url` unit RED → GREEN → wire the harvest
   `content_exists(.github/workflows)` + `content_exists(tests)` probes → the
   RGSD-5 happy A (CI workflows) + happy B (tests dir) acceptances turn GREEN, the
   disjunction-guardrail stays GREEN, RGSD-1 + RGSD-2 + RGSD-3 + RGSD-4 + the legacy
   scrape suite stay GREEN (union bridge). This is the LAST detector; RGSD-6 (design
   §5) is the cleanup slice that migrates the fake fixtures to realistic bodies and
   removes the `signals[]`/`parse_signals` scaffold.
