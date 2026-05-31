# JTBD Opportunity Scores: htmx-scraper-viewer (slice-06)

Opportunity scoring (Ulwick ODI). Score = Importance + max(Importance − Satisfaction, 0).
Scale 1–10. **Importance** = how much the operator cares about getting the job done well.
**Satisfaction** = how well the current solution (raw SQL shell / CLI batch text) already
does it. High importance + low satisfaction = high opportunity = build first.

---

## Scoring

| Job | Importance | Satisfaction (status quo) | Opportunity Score | Rank |
|-----|-----------|----------------------------|-------------------|------|
| **Job 1 — See what is in my store** | 9 | 3 (SQL shell only; high friction, rarely done) | 9 + (9 − 3) = **15** | **1 (north star)** |
| **Job 2 — Browse scrape proposals in browser** | 6 | 5 (CLI `scrape github` already works; just awkward to scan) | 6 + (6 − 5) = **7** | 2 |

### Job 1 rationale (opportunity = 15, "over-served gap")

- **Importance 9**: Operators must be able to trust their node's state. Being unable to
  see persisted `claims` + `peer_claims` without SQL is a foundational legibility gap in a
  federated trust system — the operator's credibility rests on knowing what they hold.
- **Satisfaction 3**: The only current path is a DuckDB shell + hand-written SQL +
  schema knowledge. High friction means inspection rarely happens; the job is badly served.
- **Opportunity 15** is in the "high opportunity, under-served" band — strongest place to
  invest. This is correctly the north star and the walking-skeleton target.

### Job 2 rationale (opportunity = 7, "appropriately served, marginal gain")

- **Importance 6**: Useful for triage, but candidate review is an *occasional, pre-signing*
  activity, not a daily legibility need.
- **Satisfaction 5**: The CLI `scrape github <target>` propose step already does the job
  functionally; the browser only improves *scannability*, not *capability*.
- **Opportunity 7** sits in the "served correctly" band — worth doing, but clearly after
  Job 1. Building it second also lets it reuse the HTTP/render foundation laid by Job 1.

---

## Decision

1. **Job 1 (store inspection) is the north star and ships first** (walking skeleton +
   early release slices). Highest opportunity by a wide margin (15 vs 7).
2. **Job 2 (live-scrape browsing) is the secondary release**, sequenced after the store
   view is usable, reusing the HTTP server + HTML rendering foundation.

This ranking drives `story-map.md` (skeleton = Job 1 thin thread) and `prioritization.md`
(Job 1 releases precede the Job 2 release).
