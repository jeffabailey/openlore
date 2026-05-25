# Slice 02 — GitHub Scraper (sibling feature seed)

**Status**: deferred — births sibling feature `openlore-github-scraper` after slice-01 + slice-03 land.
**Slice priority**: P2
**Effort estimate**: ~1 week
**Primary persona**: P-001 (authoring) + P-002 (consumer of resulting claims)
**Primary job**: J-001 (multiplies throughput) and J-002 (richer graph to query)

## Hypothesis

> A pluggable scraper can ingest a single GitHub repository (README, releases,
> CONTRIBUTING, top contributor profiles) and produce a set of **draft** claims
> that a human reviews and promotes. The scraper never publishes autonomously.

## Disproves if it fails

- GitHub data is too noisy to extract philosophy signals at usable precision.
- The draft-then-promote workflow adds more friction than it saves vs. authoring by hand.
- The scraper output cannot serialize to the same `org.openlore.claim` shape that
  slice-01 produced (forcing a Lexicon change).

## In scope (when this slice runs)

- One pluggable scraper interface (trait in Rust) with GitHub as the first concrete implementation.
- Draft claims persisted to a separate DuckDB table (`claim_drafts`).
- `openlore scrape github <org/repo>` produces drafts.
- `openlore claim promote <draft-id>` runs the slice-01 compose+sign+publish flow on a draft.
- Scraper provenance recorded (`scraper_name`, `scraper_version`, `ran_at`).

## Out of scope

- Wikipedia/docs scrapers (separate slices later).
- Auto-promotion of drafts.
- Scraper rate-limiting or auth orchestration beyond a single GitHub PAT.

## Why deferred behind slice-03

The federated read (slice-03) constrains the on-the-wire shape of `org.openlore.claim`.
Building a scraper that emits claims BEFORE we've stress-tested federation risks
serialization rework. Slice ordering: 01 → 03 → 02 → 04 → 05.

## Hand-off

Sibling feature directory at planning time: `docs/feature/openlore-github-scraper/`.
DISCUSS wave for slice-02 should be re-run as its own feature once slices 01 and 03
have landed.
