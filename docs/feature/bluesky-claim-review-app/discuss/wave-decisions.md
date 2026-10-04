# Wave Decisions — bluesky-claim-review-app (DISCUSS)

## Inputs

- Wizard: `../wizard-decisions.md`. No DIVERGE artifacts (`diverge/recommendation.md`,
  `diverge/job-analysis.md`) exist. **Risk noted:** the solution direction (hosted web app +
  ATProto OAuth + GitHub-only inference) was chosen in the wizard, not compared against
  alternatives in a DIVERGE wave. Mitigation: JTBD was run in this wave (`jtbd-*.md`), and
  the alternatives we considered are listed in `requirements.md` under "Alternatives
  considered".
- Mode: subagent, autonomous. The user resolved the decision points through the coordinator.
  Assumptions are listed in `requirements.md` under "Assumptions".

## Scope Assessment: SPLIT-WITHIN-FEATURE — 13 stories (1 @infrastructure), 5 contexts, about 18–24 days

The feature meets two oversized signals:

- It has more than 10 stories (13).
- It touches more than 3 contexts: ATProto identity/OAuth, GitHub scrape plus ownership proof,
  claim publishing to the user's PDS, the OpenLore read/verify path, and hosting.

We are not splitting it into separate feature directories. Instead, it is sliced into three
releases that can each ship on their own:

| Slice | Stories | Est. | Independently shippable outcome |
|-------|---------|------|---------------------------------|
| Walking Skeleton | US-BRA-000..004 | 7–9 d | One approved claim lands in the user's own PDS, and OpenLore accepts it as self-attested |
| Release 1 — Consent with control, then show it off | US-BRA-005..008 | 5–7 d | User edits or declines suggestions privately, then (opt-in) shares a profile of approved claims on Bluesky |
| Release 2 — Trust that lasts | US-BRA-009..012 | 6–8 d | Self-attested claims read correctly everywhere in OpenLore, can be retracted, rescans add only new suggestions, and the user can leave without residue |

**Split recommendation:** Release 2 has no dependency that blocks Release 1. If DESIGN or
DELIVER capacity is tight, carve Release 2 out as a follow-up feature
(`bluesky-claim-review-lifecycle`). We kept it here so the journey and the shared-artifact
registry stay whole.

## Decisions resolved during this wave (coordinator-relayed user decisions)

| ID | Decision | Date |
|----|----------|------|
| D-4 | Disapprovals are PRIVATE app-side state. They are never written to the PDS or anywhere public. There is no public counter-claim or rejection record. They are used only to suppress re-suggestion. | 2026-10-03 |
| D-5 | Provenance for app-approved claims = the PDS repo commit signature. There is no app-level `#org.openlore.application` signature and no PLC document change for Bluesky users. Consequence for DESIGN: readers, verifiers and the viewer must accept and distinguish a second provenance mode, "self-attested (repo-signed)". | 2026-10-03 |
| D-11 | The share post (`app.bsky.feed.post` linking to the profile page) is in v1, in Release 1, and strictly opt-in. The user previews it, explicitly confirms it, and declining has no side effect. The post reflects approved claims only. | 2026-10-03 |
| I-BRA-1 | Unapproved suggestions stay private until approved. Pending items get no PDS write, no share post, and no AppView, search, feed or profile exposure. Only the authenticated owner can see their queue. | 2026-10-03 |

## Platform

Web (hosted, browser). We loaded the UX skills ux-web-patterns, ux-principles and
ux-emotional-design. Accessibility target: WCAG 2.2 AA.
