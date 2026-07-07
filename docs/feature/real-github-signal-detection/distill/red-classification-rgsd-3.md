<!-- markdownlint-disable MD013 -->
# RED classification — RGSD-3 (real-github-signal-detection, semver + CHANGELOG)

DISTILL wave. Slice RGSD-3 (design §5): detect
`SignalKind::SemverAndChangelog` from the CONJUNCTION of (a) the repo's tags
following semver — `GET /repos/{owner}/{repo}/tags` carrying a semver-style name
— AND (b) a committed CHANGELOG — `GET /repos/{o}/{r}/contents/CHANGELOG.md` →
200. This file is the pre-DELIVER fail-for-the-right-reason gate output; DELIVER
reads it at PREPARE/RED to confirm the RED is genuine.

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
- **New endpoint added**: exactly ONE — the RGSD-3 harvest will issue `GET
  /repos/{o}/{r}/tags` (a JSON array of `{"name": …}`) and REUSE RGSD-2's
  `contents/*` fork for `contents/CHANGELOG.md` (200 = present, 404 = absent).
  SPIKE-verified against real GitHub (ripgrep: semver-style `tags` +
  `contents/CHANGELOG.md` → 200; torvalds/linux: semver-ish tags but
  `contents/CHANGELOG.md` → 404, a clean negative). The `FakeGithub` double was
  extended additively to serve `/tags`.

### FakeGithub `/tags` + CHANGELOG extension (additive; no-regression)

- Added `tags: Vec<String>` + `has_changelog: bool` posture fields, plus
  constructors `FakeGithub::for_public_repo_with_tags_and_changelog(target,
  tags, has_changelog)` (the general builder) and
  `FakeGithub::for_public_repo_with_semver_and_changelog(target)` (convenience:
  `["v1.2.3", "v1.2.2", "v1.0.0"]` + CHANGELOG present). `language` None and
  `has_cargo_lock` false on both, so ONLY semver-and-changelog can fire
  (isolating it from RGSD-1 memory-safety + RGSD-2 dependency-pinning).
- HTTP handler now routes `path.ends_with("/tags")` to `tags_response`: **200**
  with a JSON array of `{"name": <tag>}` objects from the posture's `tags` list
  — the real GitHub `tags` shape. The CHANGELOG half reuses the existing
  `/contents/` fork: `contents/CHANGELOG.md` → **200** (with the file `html_url`)
  when `has_changelog`, else **404**.
- **Default `[]`-tags guarantee**: any posture that does not set `tags` lists NO
  tags — `/tags` serves an empty array `[]`. **Default-404 CHANGELOG guarantee**:
  any posture that does not set `has_changelog` (all existing ones) → 404 on
  `contents/CHANGELOG.md`. Together these make the production `list_tags` +
  CHANGELOG probe read EVERY existing posture as "no semver tags AND no
  CHANGELOG", so no `SemverAndChangelog` signal can fire on them — the
  no-regression guarantee. `/tags` never 404s (an untagged repo returns `[]`).
- The repo resolve/harvest body (`GET /repos/{o}/{r}`, no `/contents/`, no
  `/tags`) is UNCHANGED. All existing constructors are untouched.
- 4 new posture unit tests pin the new surface (semver-tags array served;
  CHANGELOG 200; semver-tags-served-but-CHANGELOG-404; unconfigured → `[]`).

### Gate commands + verdicts

| Gate | Command | Result |
|---|---|---|
| 1a compile | `cargo build -p openlore-test-support` | green (FakeGithub `/tags` + CHANGELOG change compiles) |
| 1b compile | `cargo test -p cli --test scrape_semver_changelog --no-run` | green (AT imports only the support harness — no new production symbol) |
| 1c no-regress compile | `cargo test -p cli --test scrape_candidates --test scrape_dependency_pinning --test scrape_real_signal_detection --no-run` | green (existing suite still compiles) |
| 1d compile | `cargo test -p openlore-test-support --no-run` | green (new posture unit tests compile) |
| — posture unit | `cargo test -p openlore-test-support fake_github` | 19 passed (incl. the 4 new `/tags` + CHANGELOG posture tests) |
| 2 build | `cargo build --bin openlore` | green |
| 3 run | `cargo test -p cli --test scrape_semver_changelog -- --test-threads=1` | happy RED (panic at AT:114, semantic-versioning candidate absent), both negatives GREEN-today (4 passed / 1 failed incl. 2 support state_delta unit tests bundled) |
| 3 no-regress | `scrape_dependency_pinning` (4, RGSD-2 SHIPPED) · `scrape_real_signal_detection` (4, RGSD-1 SHIPPED) · `scrape_candidates` (7) · `scrape_github` (11) · `scrape_sign` (11) | all green — additive FakeGithub change caused zero regression |

## RED cause

`adapter-github::harvest_repo` today fetches `GET /repos/{o}/{r}` (RGSD-1
language) and probes `contents/Cargo.lock` (RGSD-2). It NEVER lists `/tags` and
NEVER checks a CHANGELOG, so no `SemverAndChangelog` signal is produced → no
`org.openlore.philosophy.semantic-versioning` candidate. The happy-scenario
assertion (AT line 114) fires because the behavior is unimplemented — the scrape
exits 0 and produces well-formed output, the test reaches its business
assertion, and the candidate is simply absent. This is genuine
`MISSING_FUNCTIONALITY`: not an import/fixture/setup error.

## Per-scenario tally

| Scenario | Tag(s) | Today | Classification |
|---|---|---|---|
| `scrape_repo_with_semver_tags_and_a_changelog_derives_the_semantic_versioning_candidate` | `@rgsd-3 @real-io @driving_port @happy` | **RED** (semantic-versioning candidate absent) | `MISSING_FUNCTIONALITY` ✅ correct RED. Exit-0 assertion passes; the candidate assertion (AT:114) fires. Turns GREEN when DELIVER lands `list_tags` + `is_semver_tag` + the CHANGELOG probe + `detect_signals`'s `SemverAndChangelog` arm. |
| `scrape_repo_with_semver_tags_but_no_changelog_proposes_no_semantic_versioning_candidate` | `@rgsd-3 @real-io @driving_port @edge @guardrail` | **GREEN-today** (no candidate produced regardless) | Conjunction-guard (under-firing side). Load-bearing once detection exists: semver tags WITHOUT a CHANGELOG must NOT fire (mirrors real torvalds/linux). Must stay GREEN when the happy scenario turns GREEN. |
| `scrape_repo_with_a_changelog_but_only_non_semver_tags_proposes_no_semantic_versioning_candidate` | `@rgsd-3 @real-io @driving_port @edge @guardrail` | **GREEN-today** (no candidate produced regardless) | Conjunction-guard (over-firing side). Load-bearing once detection exists: a CHANGELOG with only non-semver tags (`nightly`, `latest`) must NOT fire. Must stay GREEN when the happy scenario turns GREEN. |

- BROKEN: 0 · SETUP_FAILURE: 0 · IMPORT_ERROR: 0 · WRONG_ASSERTION/OBSERVABLE_NOT_AT_PORT: 0.

## Scope note — pure/effect unit RED is DELIVER's responsibility

Per design §7/§8, the pure/effect unit tests are **NOT authored in this DISTILL
RED**. The `list_tags(owner, repo) -> Result<Vec<String>, GithubError>` effect,
the pure `is_semver_tag(name) -> bool` predicate, the
`RepoFacts.{semver_tag, changelog_url}` fields, and the `SemverAndChangelog` arm
of `detect_signals` do NOT exist yet; referencing any of them from a Rust unit
test would be a **compile error = BROKEN**, not RED. Rust has no import-stub
scaffold that yields a clean assertion RED for a not-yet-declared symbol (unlike
a Python `__SCAFFOLD__` module). Therefore the DISTILL RED is the **acceptance
(subprocess) test only**; the crafter writes the pure/effect RED_UNIT tests (the
`list_tags` array/error mapping, the `is_semver_tag` table of positives/negatives,
`parse_repo_facts` reading `semver_tag`/`changelog_url`, the `SemverAndChangelog`
detect arm) when it introduces those symbols in DELIVER (inner TDD loop).

## Gate verdict

**PASS.** Exactly one genuine RED (`MISSING_FUNCTIONALITY`), two GREEN-today
conjunction-guardrails, zero BROKEN/SETUP/IMPORT/WRONG-shape failures, zero
regression in the existing scrape suite (37 scenarios across 5 suites stay
green, RGSD-1 + RGSD-2 SHIPPED included; plus 19 FakeGithub posture unit tests).
RED is genuine and ready to hand to DELIVER.

## DELIVER pointers (design §2/§4)

1. **`adapter-github`** (EFFECT): add `list_tags(owner, repo) ->
   Result<Vec<String>, GithubError>` — issues `GET /repos/{owner}/{repo}/tags`,
   parses the JSON array's `name` fields into `Vec<String>` (map any non-200 →
   `Err`). Reuse RGSD-2's `content_exists(owner, repo, "CHANGELOG.md")` for the
   CHANGELOG half (200 → `Some(html_url)`, 404 → `None`). Extend `RepoFacts` /
   `parse_repo_facts` with `semver_tag: Option<String>` (the first listed tag
   that `is_semver_tag`) + `changelog_url: Option<String>`. `harvest_repo` calls
   `list_tags` + the CHANGELOG probe and assembles the fuller `RepoFacts` before
   `detect_signals`.
2. **`scraper-domain`** (PURE): add `is_semver_tag(name: &str) -> bool` — loose,
   hand-rolled (NO new regex dep): a `N.N.N` core, optional leading `v`, optional
   `name-` prefix (e.g. `wincolor-0.1.6`), optional `-prerelease` / `+build`
   suffix. Add the `SemverAndChangelog` arm to `detect_signals` — fires one
   `SemverAndChangelog` signal ONLY on the CONJUNCTION
   (`semver_tag.is_some() && changelog_url.is_some()`), with the CHANGELOG
   `html_url` as its `source_url` and an honest `value` naming the semver tag +
   CHANGELOG (design §3). Write the pure `is_semver_tag` + `detect_signals` arm +
   `parse_repo_facts` RED_UNIT here first. The mapping SSOT
   (`signal_predicate_mapping.yaml` → `org.openlore.philosophy.semantic-versioning`)
   is UNCHANGED.
3. Green order: pure `is_semver_tag` + `detect_signals` `SemverAndChangelog` arm
   + `list_tags`/`semver_tag`/`changelog_url` unit RED → GREEN → wire the harvest
   `list_tags` + CHANGELOG probe → the RGSD-3 happy acceptance turns GREEN, both
   conjunction-guardrails stay GREEN, RGSD-1 + RGSD-2 + the legacy scrape suite
   stay GREEN (union bridge).
