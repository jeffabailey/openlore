# Story Map: bluesky-claim-review-app

## User: P-003 Bluesky developer (Priya Raman, @priyaraman.bsky.social)

## Goal: publish a consented, self-attested picture of how I build into my own PDS, then show it off

## Backbone

| 1. Sign in | 2. Prove GitHub | 3. Get suggestions | 4. Review privately | 5. Publish to my PDS | 6. Show it off | 7. Keep it current / leave |
|------------|-----------------|--------------------|---------------------|----------------------|----------------|----------------------------|
| **Sign in with handle via OAuth (001)** | **DID-in-bio verify (002)** | **Scan owned repos → suggestions (003)** | **Private queue cards (003)** | **Preview + publish, self-attested accepted (004)** | Profile of approved claims (007) | Rescan adds only new (010) |
| *Public hosted origin (000, infra)* | Actionable failure messages (002) | Empty-state guidance (003) | Edit confidence / swap philosophy (005) | Publish failure keeps pending (004) | Opt-in share post with preview (008) | Retract a published claim (011) |
| Sign out | Per-DID link only (002) | Rate-limit resume (003) | Not me, privately (006) | Self-attested across viewer/search/peer pull (009) | | Disconnect and forget me (012) |
| | *Re-check proof later (DESIGN flag)* | | Undo a decline (006) | | | |
| | | | Keyboard triage (003) | | | |

---

### Walking Skeleton

Thinnest end-to-end slice: **sign in → prove GitHub → one suggestion → approve → record in my own PDS**.

- US-BRA-000 (@infrastructure): the app is reachable at a public HTTPS origin that hosts the OAuth client metadata
- US-BRA-001: sign in with my Bluesky handle
- US-BRA-002: prove my GitHub account with my DID in my bio
- US-BRA-003: see my private suggestion queue from my GitHub repos
- US-BRA-004: approve a suggestion into my own PDS, accepted by OpenLore as self-attested

Every backbone activity from 1 to 5 has a task above the line. Activities 6 and 7 are not
needed for the skeleton's outcome. The skeleton's outcome ends at the PDS write, as
configured. **Privacy guards ship in the skeleton:** pending items are private (I-BRA-1),
writes need an explicit confirm (I-BRA-3), and nothing is scanned before verification
(I-BRA-4).

### Release 1: Consent with control, then show it off

Target outcomes: KPI-BRA-3 (human-in-the-loop is real) and KPI-BRA-7 (share adoption).

- US-BRA-005: edit confidence or swap the philosophy before approving
- US-BRA-006: decline privately ("Not me"). No record is written anywhere. It is never re-suggested. Undo is available.
- US-BRA-007: my profile page, which shows approved and published claims only
- US-BRA-008: opt-in share post to Bluesky, previewed and confirmed, linking to the profile (user decision D-11)

Rationale: editing and declining make the approval meaningful rather than rubber-stamped.
Sharing is in v1 by user decision and needs the profile it links to.

### Release 2: Trust that lasts

Target outcomes: KPI-BRA-6 (self-attested claims never rendered unverified) and KPI-BRA-8
(return-visit value).

- US-BRA-009: self-attested claims are recognised and labelled in the OpenLore viewer, search and peer pull
- US-BRA-010: a rescan offers only new suggestions
- US-BRA-011: retract a claim I published through the app
- US-BRA-012: disconnect and forget me (purge app-side private state)

Split candidate: this release can become follow-up feature `bluesky-claim-review-lifecycle`
(see `wave-decisions.md`).

### Later (mapped, not crafted, out of v1)

| Item | Job | Why later |
|------|-----|-----------|
| Infer from my Bluesky posts | J-009 | Needs a text→philosophy classifier (likely an LLM), with new trust and cost questions |
| Person-level self-claims (subject = my DID) from J-004e inference | J-009 | v1 uses repo-level subjects; person subject is OD-BRA-6 |
| Alternative ownership proofs (gist, GitHub OAuth, rel=me) | J-009b | v1 is DID-in-bio only (D-3) |
| Custom feed: "people who share your philosophies" | J-010 | Needs a population of self-attested claims first |
| Labeler badges for philosophies | J-010 | Moderation and labeler policy surface; needs adoption first |
| Other code hosts (GitLab, Codeberg) | J-009 | Source expansion after the GitHub value is proven |

## Priority Rationale

1. **Walking Skeleton first.** It proves the riskiest end-to-end assumptions together: OAuth
   from a hosted origin, DID-in-bio proof, server-side reuse of the scrape pipeline, writing
   an `org.openlore.claim` to a third-party PDS, and the OpenLore verify path accepting the
   repo-signed provenance (D-5). These are integration risks that can kill the feature, so
   they go first (riskiest assumption first).
2. **Release 1 next.** It holds the highest-value outcomes after the skeleton: consent
   quality (O1, O4, O5) and the user-mandated share post (O7). It depends only on the skeleton.
3. **Release 2 last.** It is lifecycle and ecosystem polish (O6 beyond the skeleton's
   minimum, O8, O9) with lower urgency. It can be split off.
