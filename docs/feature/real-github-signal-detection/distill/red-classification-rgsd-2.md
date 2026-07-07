<!-- markdownlint-disable MD013 -->
# RED classification — RGSD-2 (real-github-signal-detection, dependency-pinning)

DISTILL wave. Slice RGSD-2 (design §5): detect
`SignalKind::DependencyManifestPinned` from a committed `Cargo.lock` — the
presence of `GET /repos/{owner}/{repo}/contents/Cargo.lock` (200 = present).
This file is the pre-DELIVER fail-for-the-right-reason gate output; DELIVER
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
- **New endpoint added**: exactly ONE — the RGSD-2 harvest will issue a SECOND
  request `GET /repos/{o}/{r}/contents/Cargo.lock` (200 = present, 404 =
  absent), SPIKE-verified against real GitHub (ripgrep → 200, torvalds/linux →
  404). The `FakeGithub` double was extended additively to serve it.

### FakeGithub `contents` extension (additive; no-regression)

- Added a `has_cargo_lock: bool` posture field + constructor
  `FakeGithub::for_public_repo_with_cargo_lock(target)` (Cargo.lock present,
  `language` None so ONLY dependency-pinning can fire).
- HTTP handler now forks on `/contents/`: a configured present file
  (`Cargo.lock` on a cargo-lock posture) → **200** with a realistic `contents`
  body carrying the file `html_url`
  (`https://github.com/{owner}/{repo}/blob/master/Cargo.lock`); every OTHER
  `contents/*` path — and `Cargo.lock` when the posture has none — → **404**
  (absent).
- **Default-404 guarantee**: any UNCONFIGURED `contents/*` path 404s by
  construction. Every EXISTING posture keeps `has_cargo_lock == false`, so its
  `contents/Cargo.lock` probe → 404, and the production `content_exists` reads
  them all as "no Cargo.lock". This is what makes the change no-regression.
- The repo resolve/harvest body (`GET /repos/{o}/{r}`, no `/contents/`) is
  UNCHANGED. All existing constructors are untouched.

### Gate commands + verdicts

| Gate | Command | Result |
|---|---|---|
| 1a compile | `cargo build -p openlore-test-support` | green (FakeGithub `contents` change compiles) |
| 1b compile | `cargo test -p cli --test scrape_dependency_pinning --no-run` | green (AT imports only the support harness — no new production symbol) |
| 1c no-regress compile | `cargo test -p cli --test scrape_candidates --no-run` | green (existing suite still compiles) |
| 1d compile | `cargo test -p openlore-test-support --no-run` | green (new posture unit tests compile) |
| — posture unit | `cargo test -p openlore-test-support fake_github` | 15 passed (incl. the 4 new `contents/Cargo.lock` posture tests) |
| 2 build | `cargo build --bin openlore` | green |
| 3 run | `cargo test -p cli --test scrape_dependency_pinning -- --test-threads=1` | happy RED, negative GREEN-today, harness tests green (3 passed / 1 failed) |
| 3 no-regress | `scrape_candidates` (7) · `scrape_real_signal_detection` (4, RGSD-1) · `scrape_github` (11) · `scrape_sign` (11) · `scrape_auth` (7) | all green — additive FakeGithub change caused zero regression |

## RED cause

`adapter-github::harvest_repo` today fetches only `GET /repos/{o}/{r}` and
detects the RGSD-1 language signal. It NEVER probes `contents/Cargo.lock`, so no
`DependencyManifestPinned` signal is produced → no
`org.openlore.philosophy.dependency-pinning` candidate. The happy-scenario
assertion (AT line 100) fires because the behavior is unimplemented — the scrape
exits 0 and produces well-formed output, the test reaches its business
assertion, and the candidate is simply absent. This is genuine
`MISSING_FUNCTIONALITY`: not an import/fixture/setup error.

## Per-scenario tally

| Scenario | Tag(s) | Today | Classification |
|---|---|---|---|
| `scrape_repo_with_a_committed_cargo_lock_derives_the_dependency_pinning_candidate` | `@rgsd-2 @real-io @driving_port @happy` | **RED** (dependency-pinning candidate absent) | `MISSING_FUNCTIONALITY` ✅ correct RED. Turns GREEN when DELIVER lands `content_exists` + `detect_signals`'s `DependencyManifestPinned` arm. |
| `scrape_repo_without_a_cargo_lock_proposes_no_dependency_pinning_candidate` | `@rgsd-2 @real-io @driving_port @edge @guardrail` | **GREEN-today** (no candidate produced regardless) | Guardrail. Load-bearing once detection exists: pins detection is CARGO-LOCK-gated (no committed Cargo.lock → probe 404 → signal must NOT fire) and must NOT over-fire. Must stay GREEN when the happy scenario turns GREEN. |

- BROKEN: 0 · SETUP_FAILURE: 0 · IMPORT_ERROR: 0 · WRONG_ASSERTION/OBSERVABLE_NOT_AT_PORT: 0.

## Scope note — pure/effect unit RED is DELIVER's responsibility

Per design §7/§8, the pure/effect unit tests are **NOT authored in this DISTILL
RED**. The `content_exists(owner, repo, path)` effect, the
`RepoFacts.cargo_lock_url` field, and the `DependencyManifestPinned` arm of
`detect_signals` do NOT exist yet; referencing any of them from a Rust unit test
would be a **compile error = BROKEN**, not RED. Rust has no import-stub scaffold
that yields a clean assertion RED for a not-yet-declared symbol (unlike a Python
`__SCAFFOLD__` module). Therefore the DISTILL RED is the **acceptance
(subprocess) test only**; the crafter writes the pure/effect RED_UNIT tests
(the `content_exists` 200/404/error mapping, `parse_repo_facts` reading
`cargo_lock_url`, the `DependencyManifestPinned` detect arm) when it introduces
those symbols in DELIVER (inner TDD loop).

## Gate verdict

**PASS.** Exactly one genuine RED (`MISSING_FUNCTIONALITY`), one GREEN-today
guardrail, zero BROKEN/SETUP/IMPORT/WRONG-shape failures, zero regression in the
existing scrape suite (40 scenarios across 5 suites stay green, RGSD-1
included). RED is genuine and ready to hand to DELIVER.

## DELIVER pointers (design §2/§4)

1. **`adapter-github`** (EFFECT): add `content_exists(owner, repo, path) ->
   Result<Option<String>, GithubError>` — issues `GET /repos/{owner}/{repo}/
   contents/{path}`, mapping **200 → `Some(html_url)`** (read the `html_url`
   from the contents body), **404 → `None`**, any other status → `Err`. Extend
   `parse_repo_facts` / `RepoFacts` with `cargo_lock_url: Option<String>`.
   `harvest_repo` calls `content_exists(owner, repo, "Cargo.lock")` and
   assembles the fuller `RepoFacts` before `detect_signals`.
2. **`scraper-domain`** (PURE): add the `DependencyManifestPinned` arm to
   `detect_signals` — fires one `DependencyManifestPinned` signal when
   `cargo_lock_url.is_some()`, with the `html_url` as its `source_url` and an
   honest `value` naming the committed Cargo.lock (design §3). Write the pure
   `detect_signals` arm + `parse_repo_facts` `cargo_lock_url` RED_UNIT here
   first. The mapping SSOT
   (`signal_predicate_mapping.yaml` → `org.openlore.philosophy.dependency-pinning`)
   is UNCHANGED.
3. Green order: pure `detect_signals` `DependencyManifestPinned` arm +
   `content_exists`/`cargo_lock_url` unit RED → GREEN → wire the second harvest
   probe → the RGSD-2 happy acceptance turns GREEN, the no-Cargo.lock guardrail
   stays GREEN, RGSD-1 + the legacy scrape suite stay GREEN (union bridge).
