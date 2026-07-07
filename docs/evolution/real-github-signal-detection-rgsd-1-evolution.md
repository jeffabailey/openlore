<!-- markdownlint-disable MD013 -->
# Evolution: real-github-signal-detection RGSD-1 (walking skeleton — detect `MemorySafetyLanguage` from a repo's REAL `language` field)

> First slice of the `real-github-signal-detection` feature. Paradigm: functional
> Rust (ADR-007). Design: `docs/feature/real-github-signal-detection/design/architecture-design.md`.

## Summary

Before RGSD-1, `openlore scrape github <repo>` inferred **0 philosophies from any
real repository**: `adapter-github::harvest_repo` read a synthetic `signals[]`
array that only the `FakeGithub` test double provides — the real GitHub API never
returns it. RGSD-1 ships the walking skeleton of real detection: the scraper now
detects `SignalKind::MemorySafetyLanguage` from the repo's real `language` field
(zero new endpoints — reuses the resolve/harvest body). A live scrape of a
memory-safe-language repo now proposes the `org.openlore.philosophy.memory-safety`
candidate end-to-end.

**Live proof:** `openlore scrape github BurntSushi/ripgrep` (real api.github.com) →
`1 signal → org.openlore.philosophy.memory-safety from "primary language: Rust"`.
`electron/electron` (C++) → `0 signals` (correctly language-gated).

### What shipped (one paragraph)

PURE (`scraper-domain`, new `detect.rs`): `RepoFacts { language: Option<String>,
source_url }`, `MEMORY_SAFE_LANGUAGES` (curated set — Rust, Go, Swift, Kotlin, Java,
C#, Python, Ruby, Scala, Haskell, Elixir, Erlang, OCaml, Clojure; EXCLUDES C/C++/asm;
case-insensitive), and `detect_signals(&RepoFacts) -> Vec<Signal>` (RGSD-1 implements
only the `MemorySafetyLanguage` arm; the `value` is honest — `"primary language:
Rust"`, never claiming "no unsafe verified"; confidence stays the mapping default
`0.25`). EFFECT (`adapter-github`): `parse_repo_facts` reshapes the real `/repos`
JSON into `RepoFacts`; `harvest_repo` UNIONS `detect_signals(parse_repo_facts(body))`
with the legacy `parse_signals(body)` path (dedup by `SignalKind`). `adapter-github`
gained a pure `scraper-domain` dependency (adapter→domain, acyclic). No new crate
(21 members).

### The union bridge (why existing tests didn't need migration)

The synthetic `signals[]` field is a walking-skeleton scaffold; the whole existing
scrape acceptance suite injects signals through it via `FakeGithub`. `harvest_repo`
unions detection with the legacy path, so existing fake bodies (which carry
`language: null` → `detect_signals` yields `[]`) are untouched, while real repos and
the new test (which carry `language` but no `signals[]`) get real detection. A later
cleanup slice (RGSD-6) removes the scaffold once all detectors are real.

### Wave timeline

- **SPIKE** — verified the real `GET /repos/BurntSushi/ripgrep` carries
  `"language": "Rust"` (and `"C++"` for the negative case).
- **DESIGN** — `fafa0e9`: the feature architecture (pure/effect split, `RepoFacts`,
  the union bridge) + the 6-slice plan (RGSD-1 walking skeleton → RGSD-5 the other
  4 detectors → RGSD-6 scaffold removal).
- **DISTILL** — `a3c9ab4`: happy RED (`scrape…language_is_rust…memory_safety_candidate`)
  + C++ negative guardrail + the additive `FakeGithub::for_public_repo_with_language`
  posture. Gate PASS (1 genuine RED, 0 BROKEN; the pure-fn unit REDs are DELIVER's
  RED_UNIT since those symbols didn't exist yet).
- **DELIVER** — this slice.

## DELIVER steps (5-phase TDD, functional crafter, DES-traced; integrity exit 0)

- **01-01** `62de4a1` — `RepoFacts` + `detect_signals` (memory-safety arm) +
  `MEMORY_SAFE_LANGUAGES` + `parse_repo_facts` + `harvest_repo` union + the
  `adapter-github → scraper-domain` dep. Greened the happy AT; C++ guardrail stayed green.
- **Phase-3 refactor** — none warranted (clean Stage-0 FP).
- **Phase-5 mutation** `8c408e3` — pure detector `detect.rs` 100% (5/5 viable); also
  killed 2 effect-shell survivors in `parse_repo_facts`'s URL fallback.
- **Review nit** `f9b96f6` — clarified the `MEMORY_SAFE_LANGUAGES` comment.

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — automated gate PASS (1 step / 5 production files = ratio 0.2, 1 RED owned + 1 guardrail) |
| DISTILL RED | genuine MISSING_FUNCTIONALITY, 0 BROKEN — gate PASS |
| Integrity | exit 0 — 1 step complete DES traces |
| Phase-3 refactor | no changes warranted |
| Phase-4 adversarial review | APPROVED — 0 blockers, 0 testing theater, port-to-port verified, test budget 5/6; 1 doc nit (fixed) |
| Phase-5 mutation | pure detector = **100% (5/5 viable, 2 unviable no-Default, 0 survivors)**; +2 effect-shell survivors killed — gate ≥80% PASSED |
| check-arch | OK — 21 members; `adapter-github → scraper-domain` is a clean acyclic adapter→pure-domain edge |

Tests green: `scrape_real_signal_detection` 4/4; `scraper-domain` 15; `adapter-github`
25; existing scrape suite (`scrape_candidates` 7, `scrape_github` 11, `scrape_auth` 7,
`scrape_sign` 11) — no regression (union bridge).

## Honest scope note

RGSD-1 detects the LANGUAGE half of `MemorySafetyLanguage` only. The SSOT signal
description adds "+ no unsafe blocks"; verifying that needs a code scan and is
deferred. The emitted signal value says exactly what was measured ("primary
language: Rust"), and confidence stays 0.25 (speculative, human-signed).

## Next slices

- **RGSD-2** DependencyManifestPinned (`contents/Cargo.lock`) · **RGSD-3**
  SemverAndChangelog (`tags` + CHANGELOG) · **RGSD-4** DocsPresentAndSubstantial
  (`readme` size + `docs/`) · **RGSD-5** TestRatioOrCiMatrix (`git/trees` or
  `.github/workflows`) · **RGSD-6** remove the legacy `signals[]` scaffold once all
  detectors are real.

## Commit trail

`fafa0e9` (DESIGN), `a3c9ab4` (DISTILL RED), `62de4a1` (01-01), `8c408e3` (Phase-5
mutation), `f9b96f6` (review nit). All on `main` (trunk-based, no PR).
