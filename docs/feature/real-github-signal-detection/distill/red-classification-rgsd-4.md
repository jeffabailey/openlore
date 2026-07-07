<!-- markdownlint-disable MD013 -->
# RED classification — RGSD-4 (real-github-signal-detection, docs / substantial README)

DISTILL wave. Slice RGSD-4 (design §5): detect
`SignalKind::DocsPresentAndSubstantial` from documentation evidence — the
DISJUNCTION of EITHER (a) a SUBSTANTIAL README (`GET /repos/{owner}/{repo}/readme`
→ the `size` in bytes ≥ a threshold) OR (b) a present `docs/` directory (`GET
/repos/{o}/{r}/contents/docs` → 200). The "documentation-first" heuristic. This
file is the pre-DELIVER fail-for-the-right-reason gate output; DELIVER reads it
at PREPARE/RED to confirm the RED is genuine.

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
- **New endpoint added**: exactly ONE — the RGSD-4 harvest will issue `GET
  /repos/{o}/{r}/readme` (a JSON object carrying `size` + `html_url`) and REUSE
  RGSD-2's `contents/*` fork for `contents/docs` (200 = dir present, 404 =
  absent). SPIKE-verified against real GitHub (ripgrep: `/readme` size 21615 +
  `contents/docs` → 404, fires via the README alone; octocat/Hello-World:
  `/readme` size 13 + `contents/docs` → 404, a clean negative). The `FakeGithub`
  double was extended additively to serve `/readme` + a `contents/docs` dir
  posture.

### FakeGithub `/readme` + `contents/docs` extension (additive; no-regression)

- Added `readme_bytes: Option<u64>` + `has_docs_dir: bool` posture fields, plus
  constructors `FakeGithub::for_public_repo_with_docs_evidence(target,
  readme_bytes, has_docs_dir)` (the general builder),
  `FakeGithub::for_public_repo_with_readme(target, size_bytes)` (convenience:
  substantial README, no docs dir), and
  `FakeGithub::for_public_repo_with_docs_dir(target)` (convenience: docs dir, no
  README). `language` None, `has_cargo_lock` false, `tags` empty, `has_changelog`
  false on all three, so ONLY documentation-first can fire (isolating it from
  RGSD-1 memory-safety + RGSD-2 dependency-pinning + RGSD-3 semver/CHANGELOG).
- HTTP handler now routes `path.ends_with("/readme")` to `readme_response`:
  **200** with `{"name":"README.md","size":<readme_bytes>,"html_url":
  ".../blob/master/README.md"}` — the real GitHub `readme` shape carrying the
  `size` in bytes — when `readme_bytes` is `Some`, else **404** (no README). The
  docs-dir half reuses the existing `/contents/` fork: `contents/docs` → **200**
  with a JSON ARRAY dir-listing (the real GitHub `contents` shape for a
  directory, carrying an `html_url`) when `has_docs_dir`, else **404**.
- **Default-404 README guarantee**: any posture that does not set `readme_bytes`
  (all existing ones) → `/readme` → 404. **Default-404 docs guarantee**: any
  posture that does not set `has_docs_dir` → `contents/docs` → 404. Together
  these make the production `fetch_readme` + `contents/docs` probe read EVERY
  existing posture as "no substantial README AND no docs dir", so no
  `DocsPresentAndSubstantial` signal can fire on them — the no-regression
  guarantee.
- The repo resolve/harvest body (`GET /repos/{o}/{r}`, no `/contents/`, no
  `/tags`, no `/readme`) is UNCHANGED. All existing constructors are untouched.
- 4 new posture unit tests pin the new surface (readme 200 + size served;
  readme-only 404s docs; docs-dir 200 array served + readme 404; unconfigured →
  both 404).

### Gate commands + verdicts

| Gate | Command | Result |
|---|---|---|
| 1a compile | `cargo build -p openlore-test-support` | green (FakeGithub `/readme` + `contents/docs` change compiles) |
| 1b compile | `cargo test -p cli --test scrape_docs_substantial --no-run` | green (AT imports only the support harness — no new production symbol) |
| 1c no-regress compile | `cargo test -p cli --test scrape_semver_changelog --test scrape_dependency_pinning --test scrape_real_signal_detection --test scrape_candidates --test scrape_github --test scrape_sign --no-run` | green (existing suite still compiles) |
| 1d compile | `cargo test -p openlore-test-support fake_github` (build) | green (new posture unit tests compile) |
| — posture unit | `cargo test -p openlore-test-support fake_github` | 23 passed (incl. the 4 new `/readme` + `contents/docs` posture tests) |
| 2 build | `cargo build --bin openlore` | green |
| 3 run | `cargo test -p cli --test scrape_docs_substantial -- --test-threads=1` | happy A + happy B RED (panic at AT:118 / AT:189, documentation-first candidate absent), negative GREEN-today (3 passed / 2 failed — negative guardrail + 2 bundled state_delta unit tests) |
| 3 no-regress | `scrape_semver_changelog` (5, RGSD-3 SHIPPED) · `scrape_dependency_pinning` (4, RGSD-2 SHIPPED) · `scrape_real_signal_detection` (4, RGSD-1 SHIPPED) · `scrape_candidates` (7) · `scrape_github` (11) · `scrape_sign` (11) | all green — additive FakeGithub change caused zero regression (42 scenarios) |

## RED cause

`adapter-github::harvest_repo` today fetches `GET /repos/{o}/{r}` (RGSD-1
language), probes `contents/Cargo.lock` (RGSD-2), lists `/tags`, and probes
`contents/CHANGELOG.md` (RGSD-3). It NEVER fetches `/readme` and NEVER probes
`contents/docs`, so no `DocsPresentAndSubstantial` signal is produced → no
`org.openlore.philosophy.documentation-first` candidate. Both happy-scenario
assertions (AT lines 118 + 189) fire because the behavior is unimplemented — the
scrape exits 0 and produces well-formed output ("Harvesting public signals ... 0
signals" / "No candidate claims could be derived"), each test reaches its
business assertion, and the candidate is simply absent. This is genuine
`MISSING_FUNCTIONALITY`: not an import/fixture/setup error.

## Per-scenario tally

| Scenario | Tag(s) | Today | Classification |
|---|---|---|---|
| `scrape_repo_with_a_substantial_readme_derives_the_documentation_first_candidate` | `@rgsd-4 @real-io @driving_port @happy` | **RED** (documentation-first candidate absent) | `MISSING_FUNCTIONALITY` ✅ correct RED. Exit-0 assertion passes; the candidate assertion (AT:118) fires. Turns GREEN when DELIVER lands `fetch_readme` + `README_SUBSTANTIAL_BYTES` + `detect_signals`'s `DocsPresentAndSubstantial` arm. Pins the README disjunct (mirrors real ripgrep). |
| `scrape_repo_with_a_docs_directory_derives_the_documentation_first_candidate` | `@rgsd-4 @real-io @driving_port @happy` | **RED** (documentation-first candidate absent) | `MISSING_FUNCTIONALITY` ✅ correct RED. Exit-0 assertion passes; the candidate assertion (AT:189) fires. Turns GREEN when DELIVER lands the `contents/docs` probe + the `DocsPresentAndSubstantial` arm. Pins the OR: a `docs/` dir ALONE fires even with an absent README. |
| `scrape_repo_with_a_tiny_readme_and_no_docs_proposes_no_documentation_first_candidate` | `@rgsd-4 @real-io @driving_port @edge @guardrail` | **GREEN-today** (no candidate produced regardless) | Disjunction-guard (under-firing side). Load-bearing once detection exists: a tiny (below-threshold) README with no docs dir must NOT fire (mirrors real octocat/Hello-World). Must stay GREEN when the happy scenarios turn GREEN. |

- BROKEN: 0 · SETUP_FAILURE: 0 · IMPORT_ERROR: 0 · WRONG_ASSERTION/OBSERVABLE_NOT_AT_PORT: 0.

## Scope note — pure/effect unit RED is DELIVER's responsibility

Per design §7/§8, the pure/effect unit tests are **NOT authored in this DISTILL
RED**. The `fetch_readme(owner, repo) -> Result<Option<(u64, String)>,
GithubError>` effect, the `RepoFacts.{readme_bytes, readme_url, docs_url}`
fields, the `README_SUBSTANTIAL_BYTES` const (~3000), and the
`DocsPresentAndSubstantial` arm of `detect_signals` do NOT exist yet; referencing
any of them from a Rust unit test would be a **compile error = BROKEN**, not RED.
Rust has no import-stub scaffold that yields a clean assertion RED for a
not-yet-declared symbol (unlike a Python `__SCAFFOLD__` module). Therefore the
DISTILL RED is the **acceptance (subprocess) test only**; the crafter writes the
pure/effect RED_UNIT tests (the `fetch_readme` Some/None/error mapping, the
`README_SUBSTANTIAL_BYTES` threshold boundary, `parse_repo_facts` reading
`readme_bytes`/`readme_url`/`docs_url`, and the `DocsPresentAndSubstantial`
detect arm's disjunction table) when it introduces those symbols in DELIVER
(inner TDD loop).

## Gate verdict

**PASS.** Exactly two genuine REDs (`MISSING_FUNCTIONALITY` — the two happy
disjunct scenarios), one GREEN-today disjunction-guardrail, zero
BROKEN/SETUP/IMPORT/WRONG-shape failures, zero regression in the existing scrape
suite (42 scenarios across 6 suites stay green, RGSD-1 + RGSD-2 + RGSD-3 SHIPPED
included; plus 23 FakeGithub posture unit tests). RED is genuine and ready to
hand to DELIVER.

## DELIVER pointers (design §2/§5)

1. **`adapter-github`** (EFFECT): add `fetch_readme(owner, repo) ->
   Result<Option<(u64, String)>, GithubError>` — issues `GET
   /repos/{owner}/{repo}/readme`, maps **200** → `Some((size, html_url))` (read
   the `size` + `html_url` fields), **404** → `None` (no README), any other
   status → `Err`. Reuse RGSD-2's `content_exists(owner, repo, "docs")` for the
   docs-dir half (200 → `Some(html_url)` / dir present, 404 → `None`). Extend
   `RepoFacts` / `parse_repo_facts` with `readme_bytes: Option<u64>` +
   `readme_url: Option<String>` + `docs_url: Option<String>`. `harvest_repo`
   calls `fetch_readme` + the `contents/docs` probe and assembles the fuller
   `RepoFacts` before `detect_signals`.
2. **`scraper-domain`** (PURE): add a `README_SUBSTANTIAL_BYTES` const (~3000 —
   the SPIKE showed ripgrep 21615 fires, octocat 13 does not; ~3000 ≈ the design
   "> 200 lines" heuristic). Add the `DocsPresentAndSubstantial` arm to
   `detect_signals` — fires one `DocsPresentAndSubstantial` signal on the
   DISJUNCTION `readme_bytes.is_some_and(|b| b >= README_SUBSTANTIAL_BYTES) ||
   docs_url.is_some()`, with the README (or docs) `html_url` as its `source_url`
   and an honest `value` naming what was measured (design §3: the emitted signal
   never claims the deferred "high doc-comment density" refinement, which needs a
   code scan and is OUT of scope for the walking skeleton). Write the pure
   `detect_signals` arm + `parse_repo_facts` RED_UNIT here first. The mapping SSOT
   (`signal_predicate_mapping.yaml` → `org.openlore.philosophy.documentation-first`)
   is UNCHANGED.
3. Green order: pure `detect_signals` `DocsPresentAndSubstantial` arm +
   `README_SUBSTANTIAL_BYTES` threshold + `readme_bytes`/`readme_url`/`docs_url`
   unit RED → GREEN → wire the harvest `fetch_readme` + `contents/docs` probe →
   the RGSD-4 happy A (substantial README) + happy B (docs dir) acceptances turn
   GREEN, the disjunction-guardrail stays GREEN, RGSD-1 + RGSD-2 + RGSD-3 + the
   legacy scrape suite stay GREEN (union bridge).
