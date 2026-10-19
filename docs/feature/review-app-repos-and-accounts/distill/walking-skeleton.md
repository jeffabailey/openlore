# Walking Skeleton — review-app-repos-and-accounts

DISCUSS (D-RRA-4) decided no separate skeleton slice: the review app's flow is live and R1 is the
thin slice. DISTILL still names ONE scenario as the feature's end-to-end proof, inside R1.

## The one scenario

`tests/acceptance/review_app_repos_and_accounts.rs::priya_publishes_a_claim_about_a_repo_her_old_top_5_never_read`
(WS-RRA-1, `#[ignore = "DELIVER 01-01: …"]`).

```gherkin
@walking_skeleton @driving_port @real-io @US-RRA-001 @US-RRA-002 @KPI-RRA-1 @kpi @contract-shape:bounded-change
Scenario: Priya publishes a claim about a repo her old top 5 never read
  Given Priya is verified as github.com/priyaraman with 7 owned public repos
  And she saved a choice that unticks driftwood and ticks ferrous-notes
  And her scan read exactly her 5 chosen repos
  And she is looking at the preview of "priyaraman/ferrous-notes embodies dependency-pinning", with nothing published yet
  When she presses "Publish to my repo"
  Then her Bluesky repo holds that one claim
  And her action on a suggestion today's top 5 would never have produced is counted
```

**Strategy B** (real app + real store, fake GitHub and PDS) — rationale and the environment
waiver (stale-config environments covered by MG-1..3, RC-7, AS-2/2c/7, AP-3, RC-5, AS-3, AP-2,
IC-5) are in `wave-decisions.md` § Walking-skeleton strategy.

## Litmus (Mandate 5)

A stakeholder reading it says "yes, that is the gap the user hit on the live app": a repo outside
today's top 5 becomes a published claim, chosen by the owner. It is observable at the user's PDS
and on the pages, not at a layer boundary.

## End-to-end path

Browser → `GET /repos` (live listing through `adapter-github`, ids) → `POST` Save (scope lock,
`repo_scope` Save admission, `repo_choices` rows) → Scan again (`scan_intent`, one admission, bio
check, frozen plan rows, harvest of exactly the chosen repos) → queue → Approve (permit from a
fresh `read_repo` + bio read; R1 may still use today's owner check until 02-14) → Publish to my
repo (create-only write) → `FakeAtprotoNetwork` records the claim; `/admin/kpi` counts
`suggestion.acted.outside_default`.

## Current RED

Un-ignored today it fails at the first Save (a Given): `no "Save choice" list for github.com/priyaraman on
the choose-repos page` (the page does not exist) — MISSING_FUNCTIONALITY. See
`red-classification.md`.

## What DELIVER lands to turn it green (thinnest thread)

1. `GET /repos` with one form per verified account (page contract in the vocabulary header).
2. `POST` Save storing ticked repo ids; "Saved. Your next scan reads N repos."
3. The scan reading the saved choice (frozen plan rows) instead of `select_person_repos` top 5.
4. The `suggestion.acted.outside_default` counter on approve (KPI-RRA-1).

Store v2 lands whole in R1 (ADR-088), so MG-1/MG-2 belong to the same release.
