# Story Map: review-app-repos-and-accounts

## User: P-003 Bluesky developer (Priya Raman, @priyaraman.bsky.social), signed in to the live review app

## Goal: decide which of my repos and accounts the scan reads, and look at any public repo without ever publishing claims about repos that aren't mine

## Backbone

| 1. Link GitHub accounts | 2. Choose what to scan | 3. Scan | 4. Review privately | 5. Publish to my PDS |
|---|---|---|---|---|
| (live) One verified account | **See owned repos, top 5 pre-ticked, save (US-RRA-001)** | **Scan reads exactly the saved choice (US-RRA-002)** | (live) Private queue | (live) Preview + publish owned |
| Verify a second account (US-RRA-003) | New and vanished repos handled (US-RRA-002) | One scan covers every verified account (US-RRA-004) | Card names its account (US-RRA-004) | Publishing follows current ownership (US-RRA-010, R2) |
| Remove an account (US-RRA-008) | Name any public repo (US-RRA-005) | Imported repos read, no proof needed (US-RRA-005) | Imported card: "Not yours to publish", Decline only (US-RRA-006) | Imported suggestion can never be written (US-RRA-006) |
| | Remove an imported repo (US-RRA-009) | | | Administered repo publishable after proof (US-RRA-007, deferred, backlog) |

Bold = Release 1. (live) = already shipped by bluesky-claim-review-app.

---

### Walking Skeleton

**Not needed as a separate slice.** The review app is live and its end-to-end flow (sign in ->
verify -> scan -> review -> publish) already runs in production. Release 1 is the thinnest
change that touches the backbone: it changes "choose what to scan" and "scan", and proves through
the unchanged review and publish steps that a repo outside today's top 5 produces publishable
suggestions. No new integration point (no new external service, record type or OAuth scope).

### Release 1: Choose owned repos (US-RRA-001, US-RRA-002)

- Target outcome: KPI-RRA-1.
- Shippable alone: yes. With no saved choice, behaviour is today's top 5.
- Estimate: 4 days.

### Release 2: Second GitHub account (US-RRA-003, US-RRA-004, US-RRA-008, US-RRA-010)

- Target outcome: KPI-RRA-2, under the KPI-RRA-5 and KPI-RRA-4 guardrails.
- Shippable alone: yes. Without R1 each account's scan uses its top 5, limited by the room left under BR-RRA-3.
  US-RRA-010 (fresh owner + bio check at preview and confirm) ships here because R2 is the
  first release that makes more repos publishable.
- Estimate: 7 days.

### Release 3: Any public repo, unsigned (US-RRA-005, US-RRA-006, US-RRA-009) — one shipping unit

- Target outcome: KPI-RRA-3, under the KPI-RRA-4 guardrail.
- Shippable as one unit, after R2: importing (005) must never be live without the publish
  guard and BR-RRA-5 move (006); removing (009) completes the import list.
- Estimate: 5.5 days.

### Release 4 (backlog): Administered-repo publishing (US-RRA-007) — DEFERRED

- Deferred by the user (2026-10-19, D-RRA-12). R1-R3 ship with owned-repo publishing only.
- Kept in the backlog with three proof options and none chosen: GitHub sign-in, the
  `.github/openlore-authority` listing, or stay deferred. Candidate for its own feature.
- Target outcome when scheduled: KPI-RRA-6. Estimate: 2-3 days once a proof method is chosen.

## Priority Rationale

| Priority | Release | Value | Urgency | Effort | Score | Why this order |
|---|---|---|---|---|---|---|
| 1 | R1 Choose owned repos | 4 | 5 | 2 | 10.0 | The user hit this gap on the live app. Smallest change, no new trust rule; later releases reuse the repo list and the "read exactly what was chosen" rule. |
| 2 | R2 Second account + fresh publish check | 4 | 3 | 3 | 4.0 | Widens what is publishable under the existing DID-in-bio proof. Riskiest assumption: per-account re-check and hiding work without leaking a failed account's suggestions. |
| 3 | R3 Any public repo, unsigned | 4 | 3 | 4 | 3.0 | New trust boundary (suggestions that can never be published). After R2 so "owned by a verified account" already means "any of my accounts", which BR-RRA-5 needs. |
| 4 | R4 Administered repos | 3 | 1 | 4 | 0.75 | Deferred by the user: GitHub public data does not show admin rights and no proof method is chosen. |

Tie-break rule: dependencies first, then the riskiest assumption. The order matches the user's
suggested sequence. Values are PO estimates (no usage data yet; see wave-decisions).

## Story-to-outcome trace

| Story | Release | Outcome KPI | Depends on |
|---|---|---|---|
| US-RRA-001 | R1 | KPI-RRA-1 | live review app |
| US-RRA-002 | R1 | KPI-RRA-1, KPI-RRA-7 | US-RRA-001 |
| US-RRA-003 | R2 | KPI-RRA-2 | live GitHub step |
| US-RRA-004 | R2 | KPI-RRA-1, KPI-RRA-5 | US-RRA-003 |
| US-RRA-008 | R2 | KPI-RRA-2 (retention of accounts) | US-RRA-003 |
| US-RRA-010 | R2 | KPI-RRA-4 | live publish path |
| US-RRA-005 | R3 | KPI-RRA-3, KPI-RRA-7 | ships with 006, 009; after 010 |
| US-RRA-009 | R3 | KPI-RRA-3 | US-RRA-005 |
| US-RRA-006 | R3 | KPI-RRA-4 | US-RRA-005, US-RRA-010 |
| US-RRA-007 | R4 (deferred) | KPI-RRA-6 | US-RRA-006/010, a future proof method |
