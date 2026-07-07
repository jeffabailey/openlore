<!-- markdownlint-disable MD013 -->
# Evolution: real-github-signal-detection RGSD-4 (detect `DocsPresentAndSubstantial` — a substantial README OR a `docs/` dir)

> Fourth slice of the `real-github-signal-detection` feature. Paradigm: functional
> Rust (ADR-007). Design: `docs/feature/real-github-signal-detection/design/architecture-design.md`.

## Summary

RGSD-4 adds the fourth real detector: documentation evidence →
`SignalKind::DocsPresentAndSubstantial` → `org.openlore.philosophy.documentation-first`.
It fires on a DISJUNCTION — a substantial README (`GET /repos/{o}/{r}/readme` →
`size` ≥ 3000 bytes) OR a `docs/` directory (`content_exists(contents/docs)` → 200).

**Live proof:** `openlore scrape github BurntSushi/ripgrep` (real api.github.com) now
yields **4 candidates** — memory-safety + dependency-pinning + semantic-versioning +
documentation-first (from "substantial README (21615 bytes)").

### What shipped (one paragraph)

PURE (`scraper-domain`): `RepoFacts` gained `readme_bytes: Option<u64>` +
`readme_url: Option<String>` + `docs_url: Option<String>`; a
`README_SUBSTANTIAL_BYTES = 3000` honest floor; `is_substantial_readme`; and a
`DocsPresentAndSubstantial` `detect_signals` arm firing when
`readme_bytes.is_some_and(|b| b >= README_SUBSTANTIAL_BYTES)` OR `docs_url.is_some()`,
independent of the other three arms. EFFECT (`adapter-github`): `fetch_readme(owner,
repo)` (GET `/readme`; 200 → `Some((size, html_url))`, **404 → `Ok(None)`**, other →
`classify_refusal`); `harvest_repo` fetches `/readme` and probes
`content_exists(…, "docs")`, then assembles the fuller `RepoFacts`. Value is honest —
reports the actual README byte count or "docs/ directory present" (the deferred
"doc-comment density" is NOT claimed); confidence stays 0.25.

### Wave timeline

- **SPIKE** — ripgrep `/readme` size 21615 + no `docs/` (fires via README); octocat/Hello-World
  README 13 bytes + no docs (negative).
- **DISTILL** — `c3701c3`: 2 happy RED (README + docs/) + 1 disjunction guardrail +
  additive `FakeGithub` `/readme` route + `contents/docs` 200-dir posture (default-404).
- **DELIVER** — this slice.

## DELIVER steps (5-phase TDD, functional crafter, DES-traced; integrity exit 0)

- **01-01** `06b9ddd` — `RepoFacts.{readme_bytes,readme_url,docs_url}` +
  `README_SUBSTANTIAL_BYTES` + `is_substantial_readme` + the `DocsPresentAndSubstantial`
  disjunction arm + `fetch_readme` + `harvest_repo` readme/docs probes. Greened both
  happy ATs; the guardrail stayed green.
- **Phase-3 refactor** — none warranted.
- **Phase-5 mutation** — RGSD-4 detector **100% (4/4 viable)**, incl. the `>=` threshold
  boundary (pinned by exact-boundary tests) and the disjunction.
- **Phase-4 review → revision** `5153349` — the review REJECTED on a real MEDIUM defect
  (D1): the `docs/` probe reuses `content_exists`, whose 200 body for a DIRECTORY is a
  JSON array (no `html_url`), so `content_html_url` fell through to a `/blob/HEAD/docs`
  URL — a file path GitHub 404s for a directory. Fixed: array bodies reconstruct a
  `/tree/HEAD/{path}` URL (file/object cases unchanged), with a unit test pinning the
  file-object / directory-array / missing-url cases. (Detection was always correct; the
  fix corrects the evidence link.)

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — automated gate PASS (1 step / 3 production files = ratio 0.33, 2 happy RED owned + 1 guardrail) |
| DISTILL RED | genuine MISSING_FUNCTIONALITY, 0 BROKEN — gate PASS |
| Integrity | exit 0 — 1 step complete DES traces (own rgsd-4 subdir; other slices' logs untouched) |
| Phase-3 refactor | no changes warranted |
| Phase-4 adversarial review | REJECTED on 1 MEDIUM (D1 directory evidence URL) → **resolved with one revision pass** (`5153349`); all other dimensions PASS. The review caught a genuine defect. |
| Phase-5 mutation | RGSD-4 detector = **100% (4/4 viable, 0 survivors)** — gate ≥80% PASSED |
| check-arch | OK — 21 members |

Tests green: `scrape_docs_substantial` 3/3; `scraper-domain` 25; `adapter-github` 26
(incl. the new `content_html_url` directory test); RGSD-1/2/3 + existing scrape suite —
no regression.

## Next slices

- **RGSD-5** TestRatioOrCiMatrix (`git/trees?recursive=1` test/src ratio, or
  `.github/workflows`) · **RGSD-6** remove the legacy `signals[]` scaffold once all
  detectors are real.

## Commit trail

`c3701c3` (DISTILL RED), `06b9ddd` (01-01), `5153349` (Phase-4 review fix). All on
`main` (trunk-based, no PR).
