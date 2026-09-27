# Slice 01 — Scraping a repo records who builds it

> Story: US-CPI-001 (J-004 / J-004d) · Persona: P-001 (Maria, contributor-evaluator hat)
> Depends on: shipped scraper (ADR-017..019) · Estimate: ~1 day

## Goal

`openlore scrape github owner/repo` also records the repo's top-N human contributors (one public
`/contributors` page, default 30, `--contributors N`, bots excluded) as local contribution links, and
shows which of them are already linked to other repos the user has scraped.

## Learning hypothesis

- **Disproves if it fails**: that repos a single user scrapes actually share human contributors once
  bots are filtered. If scraping `rust-lang/regex` then `BurntSushi/ripgrep` shows NO overlap, inference
  (slices 02-05) would be starved and the feature must be rethought (e.g. larger N, different signal).
- **Confirms if it succeeds**: one page of `/contributors` per repo is enough signal, costs ≤1 extra
  request, and the people-graph can grow as a by-product of scraping (D-4).

## IN scope

- One `/contributors` request per repo scrape; top N by commits; `0 ≤ N ≤ 100` (OD-CPI-7).
- Bot exclusion (GitHub `type == "Bot"` or login ending `[bot]`); excluded bots named in output.
- Local contribution-link storage keyed by (repo, person); person = `github:<login>` (+ numeric
  GitHub id, OD-CPI-1); upsert on re-scrape, never delete (OD-CPI-6).
- Output block: count, excluded bots, overlap with already-linked repos.
- Failure: rate-limit/auth/network → no partial links, cause named, exit non-zero.

## OUT of scope

- Inference, candidates about people, signing (→ 02/03). `scrape github <user>` changes (→ 05).
- Paging beyond 100; per-commit data; anonymous/email contributors.

## Acceptance criteria (production data)

- [ ] `openlore scrape github rust-lang/regex` then `openlore scrape github BurntSushi/ripgrep` records
      up to 30 humans each and the second run lists `BurntSushi → rust-lang/regex` as overlap.
- [ ] `openlore scrape github dtolnay/anyhow --contributors 5` records exactly 5, `dtolnay` first.
- [ ] Any `[bot]` account returned by GitHub (e.g. `dependabot[bot]`) is excluded and named.
- [ ] Re-scraping ripgrep keeps every earlier link; refreshed rows show new last-observed.
- [ ] Zero rows written to author_claims; request count increases by exactly 1 per repo scrape.
- [ ] Exhausted rate budget → no links recorded, `GITHUB_TOKEN` hint, non-zero exit.

## Dogfood moment

Maria scrapes two related repos she actually uses and reads the overlap line the same day.

## Dependencies

- `GithubPort` contributors read + `adapter-github`; `adapter-duckdb` link table (schema migration).
