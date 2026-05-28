# Story Map: openlore-github-scraper (slice-02)

- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)

## User: P-002 Researcher / Tech Lead (contributor-evaluator hat)

Secondary persona: P-001 Senior Engineer Solo Builder (wears the
contributor-evaluator hat too when evaluating a dependency's maintainers).

## Goal

Lower the cost of producing well-evidenced claims about a contributor or repo
by harvesting their PUBLIC GitHub signals and turning them into editable
candidate claims the human reviews and signs — without the tool ever asserting
anything on the user's behalf.

## Backbone

| Harvest | Propose | Review & Edit | Sign & Publish | Read Back (optional) |
|------|------|------|------|------|
| Scrape github target | Derive candidates | Select a candidate | Compose (slice-01) | Query the signed claims |
| (public data only) | (name source signal) | Edit predicate/evidence/confidence | Sign locally | (slice-01 graph query) |
| (auth via PAT or anon) | (default 0.25 speculative) | (reject the rest) | Publish to PDS | |

---

## Walking Skeleton (slice-02 walking skeleton)

The minimum slice that exercises scrape -> propose -> sign end-to-end:

1. **Harvest** — `openlore scrape github rust-lang/cargo` reads public signals
2. **Propose** — the CLI derives a candidate list, each candidate naming its source signal, confidence 0.25
3. **Sign** — `openlore scrape github rust-lang/cargo --sign 1` carries one candidate through the slice-01 compose-sign-publish pipeline; the human signs

This skeleton validates:

- The harvest contract (adapter-github reads public artifacts and returns signals)
- The pure derivation (scraper-domain maps signals -> candidate claims auditably)
- The human-gate guarantee (scraper proposes; the slice-01 pipeline signs; nothing auto-published)
- Provenance (the signed claim records it was derived from a scraper run)

It does NOT include multi-candidate batch signing, contributor (user) targets
beyond the basic resolution, or any read-back surface — those are later
releases or inherited from slice-01. The walking skeleton is the thinnest
possible proof that the cost-lowering thesis holds.

---

## Release 1 — Walking Skeleton (target outcome: scrape->propose->sign validated)

| Story | Target outcome | KPI |
|---|---|---|
| US-SCR-001 | Harvest a public GitHub target's signals | KPI-SCR-1 (cost-to-first-claim), KPI-SCR-4 (public-data-only guardrail) |
| US-SCR-002 | Derive auditable candidate claims from signals | KPI-SCR-3 (auditability), KPI-SCR-1 |
| US-SCR-003 | Review, edit, and sign a candidate via the slice-01 pipeline | KPI-SCR-1 (cost-to-first-claim — north star), KPI-SCR-2 (human-gate) |
| US-SCR-006 | Bootstrap adapter-github + scraper-domain + GithubPort (`@infrastructure`) | supports KPI-SCR-1..4 |

**Rationale**: this is the minimum bundle that disproves the cost-lowering
hypothesis if it fails. Without ANY of these four stories, there is no
end-to-end scrape->propose->sign demo. US-SCR-003 is the value-capture story —
it is where a candidate becomes a signed human assertion.

**Demo gate (Phase 3.5)**: User runs `openlore scrape github rust-lang/cargo`,
sees a candidate list with traceable signals and speculative confidence, then
runs `--sign 1`, edits a field, signs, and publishes. A subsequent
`openlore graph query --subject github:rust-lang/cargo` shows the signed claim
with the `derived-from: openlore-github-scraper` provenance line.

---

## Release 2 — Authenticated + batch harvest (target outcome: real-target reach)

| Story | Target outcome | KPI |
|---|---|---|
| US-SCR-004 | Use an optional PAT for higher rate limits and larger/contributor targets | KPI-SCR-1 (cost-to-first-claim holds for real targets) |

**Rationale**: the walking skeleton works unauthenticated on small repos, but
real evaluation targets (a busy contributor's cross-repo signals, a large
monorepo) exhaust the anonymous rate budget. Optional-PAT support is the
behavior that makes the scraper usable on the targets P-002 actually cares
about. Sequenced AFTER the walking skeleton because if Release 1 fails the
whole feature dies and Release 2 effort is wasted.

**Demo gate**: With `GITHUB_TOKEN` set, Maria scrapes a contributor target
(`openlore scrape github torvalds`) that would exhaust the anonymous budget,
and the harvest completes; without the token the same target degrades
gracefully with a "set GITHUB_TOKEN for higher limits" message.

---

## Release 3 — Batch candidate signing (target outcome: multi-claim efficiency)

| Story | Target outcome | KPI |
|---|---|---|
| US-SCR-005 | Select and sign several candidates in one pass (`--sign 1,3,4`) | KPI-SCR-1 (amortized cost-to-claim across several claims) |

**Rationale**: signing candidates one at a time is usable but slow when the
user agrees with several. Batch signing is a pure efficiency multiplier on the
already-validated single-sign flow. It can ship LAST because the journey is
fully usable without it — the worst case is "I run `--sign` three times instead
of once." That is a survivable friction defect; the worst case for Release 1
(scraper auto-asserts / leaks unsigned claims) is unsurvivable. Hence the
priority order.

**Demo gate**: Maria runs `openlore scrape github rust-lang/cargo --sign 1,3,4`
and is walked through three compose previews in sequence, signing each, with a
running "(2 of 3 signed)" progress indicator.

---

## Priority Rationale

Priority order: **Release 1 (Walking Skeleton) > Release 2 (Auth/real-target) > Release 3 (Batch sign)**.

The ordering is set by outcome impact and risk-of-failure consequence, NOT by
feature volume or implementation order:

1. **Release 1 first** because if it fails, the cost-lowering hypothesis is
   disproven and the whole sibling feature collapses. The riskiest assumption
   (per `nw-user-story-mapping` "Riskiest Assumption First") is the human-gate:
   that a scraper can lower authoring cost WITHOUT crossing into auto-assertion.
   All four Release-1 stories are tightly coupled — none ships usable value
   alone, but all four together produce a complete end-to-end demo of J-004's
   scrape->propose->sign loop.

2. **Release 2 second** because authenticated harvest is what makes the scraper
   usable on REAL evaluation targets (busy contributors, large repos). It is the
   highest-value behavior after the skeleton because the unauthenticated rate
   budget is too small for the targets P-002 actually evaluates. It also
   benefits from being layered on a stable harvest path; if Release 1 has a
   latent signal-derivation bug, it surfaces in Release 1 rather than corrupting
   authenticated-harvest data in Release 2.

3. **Release 3 third** because batch signing is a pure efficiency multiplier, not
   a primary outcome. The journey is fully usable without it; the worst case
   ("I sign candidates one at a time") is survivable. By contrast the worst case
   for Release 1 (human-gate broken) is unsurvivable. Dependency-wise, US-SCR-005
   depends on US-SCR-003 (single-candidate sign must work first), so it cannot
   ship before Release 1 regardless of priority.

This ordering preserves the carpaccio principle: each release is independently
demo-able and delivers a verifiable working behavior. Release 1 alone is a
shippable end-to-end slice. Release 2 widens reach to real targets. Release 3
adds the batch-signing efficiency.

---

## What is NOT in scope (explicitly deferred)

These were considered and deferred to later sibling features, NOT just later
releases of this feature:

| Out-of-scope | Why deferred | Future home |
|---|---|---|
| Non-GitHub sources (Mastodon, blogs, GitLab) | The brief scopes slice-02 to GitHub ONLY; multi-source scraping balloons the bounded context | post-slice-05 (a `openlore-multi-source-scraper`) |
| Auto-confidence scoring from triangulation (multi-project) | Confidence weighting from cross-project triangulation is the J-004 "adherence weighting" function; it is an algorithmic concern with its own JTBD | `openlore-scoring-graph` (slice-04) |
| Auto-publishing of candidates | Forbidden by design — violates the slice-01 "claims are signed human assertions" invariant | NEVER (intentional non-goal) |
| Scheduled / daemon scraping | CLI-first, no daemon; the scraper is invoked on-demand | NEVER (intentional non-goal for the CLI product) |
| ML-based philosophy inference | The mapping is small + auditable by design; ML inference would make candidates unauditable | out of scope indefinitely |
| Counter-claiming a scraped candidate in one step | Counter-claim is a slice-03 verb; a scraped candidate can be hand-countered via the existing `claim counter` after signing | already covered by slice-03 |
| Contributor-graph traversal (cross-repo) UI | Slice-02 harvests one target; cross-repo triangulation rendering is a query/scoring concern | slice-04 / slice-05 |
