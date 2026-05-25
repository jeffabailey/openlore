# Slice 05 — AppView and Search UI (sibling feature seed)

**Status**: deferred — births sibling feature `openlore-appview-search` after slices 01-04 land.
**Slice priority**: P4
**Effort estimate**: ~2-3 weeks
**Primary persona**: P-002 (Researcher / Tech Lead) primarily
**Primary job**: J-002 Explore the philosophy graph + J-004 Contributor lens

## Hypothesis

> An AppView aggregating signed claims from many DIDs serves a search experience
> that surfaces non-obvious philosophy↔project↔contributor connections useful
> enough that a Researcher-class user (P-002) prefers it over hand-research.

## Disproves if it fails

- Aggregation at the AppView level breaks the local-first invariants users
  expect (e.g. surfaces claims the user has not subscribed to as if they had).
- Search UX without weighting feels like another aggregator site (HN/Reddit/awesome-list).

## In scope (when this slice runs)

- AppView service that ingests claims from a configurable set of PDSes.
- Search by subject / predicate / object / contributor.
- Web UI surfacing the same graph the CLI exposes — feature parity, not feature replacement.
- API for programmatic graph queries.

## Out of scope

- Hosted multi-tenant SaaS for AppView. Initial AppView is single-instance, self-hosted.
- Recommendation engine ("you might like…"). That's a separate, later slice.

## Why deferred to last

UI work has the highest scope-creep risk and the most attractive distractions. The
CLI must be the canonical interface for the model; UI must layer on, not bypass.
Building UI early invites baking shortcuts into the model that we cannot reverse.

## Hand-off

Sibling feature directory at planning time: `docs/feature/openlore-appview-search/`.
