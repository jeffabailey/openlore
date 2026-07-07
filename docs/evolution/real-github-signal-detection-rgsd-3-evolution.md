<!-- markdownlint-disable MD013 -->
# Evolution: real-github-signal-detection RGSD-3 (detect `SemverAndChangelog` — semver tags AND a CHANGELOG)

> Third slice of the `real-github-signal-detection` feature. Paradigm: functional
> Rust (ADR-007). Design: `docs/feature/real-github-signal-detection/design/architecture-design.md`.

## Summary

RGSD-3 adds the third real detector: the CONJUNCTION of (a) the repo's tags follow
semver AND (b) a CHANGELOG is committed → `SignalKind::SemverAndChangelog` →
`org.openlore.philosophy.semantic-versioning`. It adds one endpoint (`GET
/repos/{o}/{r}/tags`) and reuses RGSD-2's `content_exists` for the CHANGELOG.

**Live proof:** `openlore scrape github BurntSushi/ripgrep` (real api.github.com) now
yields **3 candidates** — memory-safety (RGSD-1) + dependency-pinning (RGSD-2) +
semantic-versioning (from "semver tags + CHANGELOG present").

### What shipped (one paragraph)

PURE (`scraper-domain`): `RepoFacts` gained `semver_tag: Option<String>` +
`changelog_url: Option<String>`; a hand-rolled, regex-free `is_semver_tag(name)` —
LOOSE match for a `MAJOR.MINOR.PATCH` numeric core at a component boundary,
tolerating a leading `v`, a `<pkgname>-` prefix (`wincolor-0.1.6`), and a
`-prerelease`/`+build` suffix — plus `pick_semver_tag(names)`; and a
`SemverAndChangelog` `detect_signals` arm that fires ONLY on the conjunction (both
`Some`), independent of the other arms. EFFECT (`adapter-github`): `list_tags(owner,
repo)` (GET `/tags` → `Vec<String>` via `parse_tag_names`, errors via the shared
`classify_refusal`); `harvest_repo` lists tags → `pick_semver_tag`, probes
`content_exists(…, "CHANGELOG.md")`, and assembles the fuller `RepoFacts`. Value is
honest ("semver tags + CHANGELOG present"); confidence stays 0.25.

### The conjunction (why two guardrails)

`SemverAndChangelog` is an AND of two conditions, so it has two failure modes, each
pinned by a guardrail: **semver tags but no CHANGELOG** (the real torvalds/linux
case) → no signal; **CHANGELOG but only non-semver tags** (`nightly`/`latest`) → no
signal. Only when BOTH hold does the candidate appear.

### Wave timeline

- **SPIKE** — ripgrep `/tags` include semver names + `CHANGELOG.md` → 200; torvalds/linux
  has semver-ish tags but `CHANGELOG.md` → 404 (the conjunction negative).
- **DISTILL** — `f6e91d3`: happy RED + 2 conjunction guardrails + additive `FakeGithub`
  `/tags` route (default 200-`[]`) and semver/changelog postures. Gate PASS.
- **DELIVER** — this slice.

## DELIVER steps (5-phase TDD, functional crafter, DES-traced; integrity exit 0)

- **01-01** `c4d68d8` — `RepoFacts.{semver_tag,changelog_url}` + `is_semver_tag`/`pick_semver_tag`
  + `SemverAndChangelog` conjunction arm + `list_tags` + `harvest_repo` tags/CHANGELOG
  probes. Greened the happy AT; both guardrails stayed green.
- **Phase-3 refactor** — none warranted.
- **Phase-5 mutation** — pure detector `detect.rs` **97.2% (35/36 viable)**; the single
  survivor (`detect.rs:124` `&&→||` in `is_semver_tag`) is a proven EQUIVALENT mutant.
- **Review follow-up** `e54382b` — added a digit-boundary example test pinning the
  `is_semver_tag` scan behavior (`abc234.5.6` → true, matched at the boundary) and
  documenting the equivalent-mutant proof.

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — automated gate PASS (1 step / 3 production files = ratio 0.33, 1 RED owned + 2 guardrails) |
| DISTILL RED | genuine MISSING_FUNCTIONALITY, 0 BROKEN — gate PASS |
| Integrity | exit 0 — 1 step complete DES traces (own rgsd-3 subdir; other slices' logs untouched) |
| Phase-3 refactor | no changes warranted |
| Phase-4 adversarial review | APPROVED-WITH-NITS — 0 blockers. D2 (a claimed missing test for the mutation survivor) was independently RESOLVED: the survivor is a genuine equivalent mutant (proof: `matches_semver_core_at` depends only on the digit-run END + following char, so any mid-run match coincides with a boundary match; the review's `abc234.5.6` counterexample miscalculated — the original returns `true`). D1 addressed with the boundary test; D3 (a direct `parse_tag_names` unit test) is a LOW nit deferred. |
| Phase-5 mutation | pure detector = **97.2% (35/36 viable; 1 verified-equivalent survivor, 4 unviable no-Default)** — gate ≥80% PASSED |
| check-arch | OK — 21 members |

Tests green: `scrape_semver_changelog` 3/3; `scraper-domain` 23; `adapter-github` 25;
RGSD-1/RGSD-2 + existing scrape suite — no regression.

## Next slices

- **RGSD-4** DocsPresentAndSubstantial (`readme` size + `docs/`) · **RGSD-5**
  TestRatioOrCiMatrix (`git/trees` or `.github/workflows`) · **RGSD-6** remove the
  legacy `signals[]` scaffold once all detectors are real.

## Commit trail

`f6e91d3` (DISTILL RED), `c4d68d8` (01-01), `e54382b` (review boundary test). All on
`main` (trunk-based, no PR).
