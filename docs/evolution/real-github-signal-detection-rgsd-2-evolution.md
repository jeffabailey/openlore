<!-- markdownlint-disable MD013 -->
# Evolution: real-github-signal-detection RGSD-2 (detect `DependencyManifestPinned` from a committed `Cargo.lock`)

> Second slice of the `real-github-signal-detection` feature. Paradigm: functional
> Rust (ADR-007). Design: `docs/feature/real-github-signal-detection/design/architecture-design.md`.

## Summary

RGSD-2 adds the second real detector: a committed `Cargo.lock` →
`SignalKind::DependencyManifestPinned` → `org.openlore.philosophy.dependency-pinning`.
It introduces the scraper's first SECOND-endpoint probe (`GET
/repos/{o}/{r}/contents/Cargo.lock`, +1 call/repo).

**Live proof:** `openlore scrape github BurntSushi/ripgrep` (real api.github.com) now
yields **2 candidates** — `memory-safety` (from "primary language: Rust", RGSD-1)
AND `dependency-pinning` (from "Cargo.lock committed (pinned dependencies)").

### What shipped (one paragraph)

EFFECT (`adapter-github`): `content_exists(owner, repo, path) -> Result<Option<String>,
GithubError>` — `GET …/contents/{path}`; **200 → `Ok(Some(html_url))`** (the file's
public URL), **404 → `Ok(None)`** (absent is NOT an error — it must not abort the
harvest), other statuses → the same railway `GithubError` classification as
`get_public` (403 → RateLimited, 401 → TokenRejected, transport → Network; token
never logged). Refactor extracted a shared `classify_refusal` used by both
`get_public` and `content_exists`. `harvest_repo` probes `contents/Cargo.lock`,
fills `RepoFacts.cargo_lock_url`, then unions detection with the legacy `parse_signals`
path. PURE (`scraper-domain`): `RepoFacts` gained `cargo_lock_url: Option<String>`;
`detect_signals` gained an INDEPENDENT `DependencyManifestPinned` arm (fires iff
`cargo_lock_url.is_some()`, alongside the language arm — a repo can fire both). Value
string is honest ("Cargo.lock committed (pinned dependencies)"); confidence stays 0.25.

### The 404-is-not-an-error subtlety (the load-bearing decision)

An absent `Cargo.lock` is a normal outcome, not a failure — `content_exists` returns
`Ok(None)` on 404 so the harvest continues (no dependency-pinning signal). Only
genuine failures (rate-limit, token, network) propagate as errors. This also makes
the new probe safe for the whole existing scrape suite: the `FakeGithub` default-404
for unconfigured `contents/*` paths reads as "absent", so every prior posture is
unaffected (verified: RGSD-1 + `scrape_candidates`/`scrape_github`/`scrape_auth`/
`scrape_sign` all green).

### Wave timeline

- **SPIKE** — real `contents/Cargo.lock`: ripgrep → 200 (body carries the file
  `html_url`), torvalds/linux → 404.
- **DISTILL** — `93bf420`: happy RED (`scrape…committed_cargo_lock…dependency_pinning`)
  + 404 negative guardrail + additive `FakeGithub::for_public_repo_with_cargo_lock`
  posture with the **default-404 guarantee** for unconfigured contents paths. Gate PASS.
- **DELIVER** — this slice.

## DELIVER steps (5-phase TDD, functional crafter, DES-traced; integrity exit 0)

- **01-01** `907c8e6` — `RepoFacts.cargo_lock_url` + `DependencyManifestPinned` arm +
  `content_exists` + `harvest_repo` probe + `content_html_url` parse helper + factored
  `send_get`. Greened the happy AT; 404 guardrail stayed green.
- **Phase-3 refactor** `7152bd9` — L2 dedup: extracted `classify_refusal` (shared by
  `get_public` + `content_exists`).
- **Phase-5 mutation** `e9cbe3a` — pure detector `detect.rs` 100% (6/6 viable; RGSD-2
  arm 2/2); no survivors.

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — automated gate PASS (1 step / 3 production files = ratio 0.33, 1 RED owned + 1 guardrail) |
| DISTILL RED | genuine MISSING_FUNCTIONALITY, 0 BROKEN — gate PASS |
| Integrity | exit 0 — 1 step complete DES traces (own rgsd-2 subdir; RGSD-1's log untouched) |
| Phase-3 refactor | L2 `classify_refusal` extraction (behavior-preserving) |
| Phase-4 adversarial review | APPROVED — 0 defects; the 404-is-not-an-error path and arm independence verified |
| Phase-5 mutation | pure detector = **100% (6/6 viable, RGSD-2 arm 2/2, 0 survivors)** — gate ≥80% PASSED |
| check-arch | OK — 21 members |

Tests green: `scrape_dependency_pinning` 4/4; `scraper-domain` 18; `adapter-github` 25;
RGSD-1 (`scrape_real_signal_detection`) + existing scrape suite — no regression.

## Next slices

- **RGSD-3** SemverAndChangelog (`tags` + CHANGELOG) · **RGSD-4** DocsPresentAndSubstantial
  (`readme` size + `docs/`) · **RGSD-5** TestRatioOrCiMatrix (`git/trees` or CI) ·
  **RGSD-6** remove the legacy `signals[]` scaffold once all detectors are real.

## Commit trail

`93bf420` (DISTILL RED), `907c8e6` (01-01), `7152bd9` (Phase-3 refactor), `e9cbe3a`
(Phase-5 mutation). All on `main` (trunk-based, no PR).
