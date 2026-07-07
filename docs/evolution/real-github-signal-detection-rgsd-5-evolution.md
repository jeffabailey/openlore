<!-- markdownlint-disable MD013 -->
# Evolution: real-github-signal-detection RGSD-5 (detect `TestRatioOrCiMatrix` — CI workflows OR a `tests/` dir) — the FINAL detector

> Fifth and last detector of the `real-github-signal-detection` feature. Paradigm:
> functional Rust (ADR-007). Design: `docs/feature/real-github-signal-detection/design/architecture-design.md`.

## Summary

RGSD-5 ships the last of the five real detectors: testing evidence →
`SignalKind::TestRatioOrCiMatrix` → `org.openlore.philosophy.test-driven`. It fires on
a DISJUNCTION — CI workflows present (`content_exists(contents/.github/workflows)` →
200) OR a `tests/` directory (`content_exists(contents/tests)` → 200). Both reuse
RGSD-2's `content_exists` — **no new effect method**.

**Milestone — all five detectors live.** `openlore scrape github BurntSushi/ripgrep`
(real api.github.com) now yields **5 candidates**, the complete set:

```
 [1] memory-safety        ← primary language: Rust
 [2] dependency-pinning   ← Cargo.lock committed (pinned dependencies)
 [3] semantic-versioning  ← semver tags + CHANGELOG present
 [4] documentation-first  ← substantial README (21615 bytes)
 [5] test-driven          ← CI workflows present (.github/workflows)
```

Real-repo philosophy inference — the capability that returned 0 signals before this
feature — now covers the full bounded `SignalKind` set.

### What shipped (one paragraph)

PURE (`scraper-domain`): `RepoFacts` gained `ci_workflows_url: Option<String>` +
`tests_dir_url: Option<String>`; a `TestRatioOrCiMatrix` `detect_signals` arm firing on
`ci_workflows_url.is_some() || tests_dir_url.is_some()` with CI precedence, independent
of the other four arms. EFFECT (`adapter-github`): `harvest_repo` probes
`contents/.github/workflows` and `contents/tests` via the existing `content_exists`
(directory 200 → the RGSD-4-fixed `content_html_url` resolves an array body to a
`/tree/` URL). Value is honest — names "CI workflows present (.github/workflows)" or
"tests/ directory present"; the deferred "test/source ratio > 0.5" precision (a tree
walk) is NOT claimed. Confidence stays 0.25.

### Wave timeline

- **SPIKE** — ripgrep `.github/workflows` + `tests` both 200 (fires); octocat/Hello-World
  both 404 (negative).
- **DISTILL** — `66ec00b`: 2 happy RED (CI + tests/) + 1 disjunction guardrail + additive
  `FakeGithub` CI/tests 200-dir postures (default-404; `.github/workflows` path matched on
  the full suffix). Gate PASS.
- **DELIVER** — this slice.

## DELIVER steps (5-phase TDD, functional crafter, DES-traced; integrity exit 0)

- **01-01** `cb51e0f` — `RepoFacts.{ci_workflows_url,tests_dir_url}` + the
  `TestRatioOrCiMatrix` disjunction arm + the two `content_exists` probes. Greened both
  happy ATs; the guardrail stayed green.
- **Phase-3 refactor** — none warranted (the five arms differ materially; a shared
  disjunction helper would force swap-prone positional tuples — correctly rejected).
- **Phase-5 mutation** — RGSD-5 detector **100% (2/2 viable)**; whole-file 97.6% (the one
  miss is the known, re-proved RGSD-3 `is_semver_tag` equivalent mutant, out of scope).

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — automated gate PASS (1 step / 3 production files = ratio 0.33, 2 happy RED owned + 1 guardrail) |
| DISTILL RED | genuine MISSING_FUNCTIONALITY, 0 BROKEN — gate PASS |
| Integrity | exit 0 — 1 step complete DES traces (own rgsd-5 subdir; other slices' logs untouched) |
| Phase-3 refactor | no changes warranted |
| Phase-4 adversarial review | **APPROVED — 0 defects** (disjunction + CI precedence + honest semantics + 5-arm independence + no-regression all verified) |
| Phase-5 mutation | RGSD-5 detector = **100% (2/2 viable, 0 survivors)** — gate ≥80% PASSED |
| check-arch | OK — 21 members |

Tests green: `scrape_test_ci` 3/3; `scraper-domain` 26; `adapter-github` 26; RGSD-1..4 +
existing scrape suite — no regression.

## Feature status

**All 5 real detectors shipped**: RGSD-1 memory-safety (language) · RGSD-2 dependency-pinning
(Cargo.lock) · RGSD-3 semver+changelog · RGSD-4 docs (README/`docs/`) · RGSD-5 test/CI. The
harvest now reads REAL public repo metadata for every bounded `SignalKind`. The only remaining
slice is **RGSD-6** — remove the transitional legacy `signals[]` scaffold (the union bridge)
and migrate the few remaining `FakeGithub` `signals[]` fixtures to realistic bodies; a pure
cleanup with no new capability.

## Commit trail

`66ec00b` (DISTILL RED), `cb51e0f` (01-01). All on `main` (trunk-based, no PR).
