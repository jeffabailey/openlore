# JTBD Opportunity Scores — bluesky-claim-review-app

Formula: `score = importance + max(importance − satisfaction, 0)`. Scale 1–10. Scores are
the product owner's estimates from the four-forces analysis. No survey data exists (risk,
noted in `wave-decisions.md`).

## Jobs

| Job | Importance | Satisfaction | Score | Verdict |
|-----|-----------|--------------|-------|---------|
| J-009 Curate a consented, self-attested profile from my Bluesky identity | 8 | 1 | **15** | underserved-primary-for-feature |
| J-010 Find like-minded developers inside Bluesky (LATER) | 6 | 2 | 10 | appropriately-served-later |

J-009 outranks J-010 because J-010 needs a population of published self-attested claims to
exist first. J-009 creates that supply.

## Desired outcomes for J-009 (ODI-style)

| # | Outcome statement | Imp | Sat | Score | Served by |
|---|-------------------|-----|-----|-------|-----------|
| O1 | Minimize the likelihood that anything about me is published without my explicit consent | 10 | 2 | 18 | I-BRA-1/3, US-BRA-003/004 |
| O2 | Minimize the time from sign-in to my first published claim | 8 | 1 | 15 | WS (US-BRA-001..004) |
| O3 | Minimize the likelihood that someone claims repos that aren't theirs | 9 | 3 | 15 | US-BRA-002 |
| O4 | Minimize the likelihood that a declined suggestion is exposed or re-suggested | 8 | 1 | 15 | US-BRA-006 |
| O5 | Maximize the accuracy of a published claim (my words, my confidence) | 8 | 2 | 14 | US-BRA-005 |
| O6 | Minimize the chance my claims are treated as unverified by OpenLore readers | 7 | 1 | 13 | US-BRA-004 AC, US-BRA-009 |
| O7 | Maximize ease of showing my approved profile to my followers | 6 | 1 | 11 | US-BRA-007/008 |
| O8 | Minimize effort to keep my suggestions current as my repos evolve | 5 | 1 | 9 | US-BRA-010 |
| O9 | Minimize residue when I leave | 6 | 2 | 10 | US-BRA-012 |

Prioritization follows these scores: consent and ownership (O1, O3, O4) shape the walking
skeleton's guards and Release 1; showing it off (O7) is in Release 1 by user decision;
lifecycle (O8, O9) is Release 2.
