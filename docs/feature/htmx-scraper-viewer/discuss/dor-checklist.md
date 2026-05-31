# Definition of Ready: htmx-scraper-viewer (slice-06)

9-item DoR hard gate. Each item validated with evidence per story. Applies to the 5 stories
in `user-stories.md` (US-VIEW-001..005). Plus the two cross-cutting hard checks introduced
for this project: **job_id traceability** and **Elevator Pitch presence**.

---

## Per-story validation

### US-VIEW-001 — See my store in the browser (Walking Skeleton)

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1. Problem statement clear, domain language | PASS | "blind to her own node's persisted contents without writing raw SQL" — domain framed (Maria, claims, store). |
| 2. User/persona specific | PASS | Node operator Maria Santos, weeks of signing, on localhost. |
| 3. 3+ domain examples, real data | PASS | rust-lang/rust @0.90 happy; empty store (Tom); locked store error — real names/CIDs. |
| 4. UAT in G/W/T (3–7) | PASS | 4 scenarios (zero-SQL view, read-only start, empty store, unreadable store). |
| 5. AC derived from UAT | PASS | AC-001.1–.6 trace 1:1 to scenarios + 2 @property guardrails. |
| 6. Right-sized (1–3 days, 3–7 scenarios) | PASS | Thin skeleton, 4 scenarios; single demoable thread. |
| 7. Technical notes: constraints/deps | PASS | adapter-duckdb, same store, OD-VIEW-1/2/6, I-SCR-1, KPI-5. |
| 8. Dependencies resolved/tracked | PASS | adapter-duckdb (exists), claims (slice-01, exists). |
| 9. Outcome KPIs measurable | PASS | KPI-VIEW-1 (<10s, zero SQL) + KPI-VIEW-2 guardrail. |
| job_id traceability | PASS | job_id = Job 1 (jtbd-job-stories.md). |
| Elevator Pitch | PASS | Before/After/Decision present; After cites `/claims` browser entry point; output observable (HTML list). |

### US-VIEW-002 — Inspect one claim's full evidence

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1 | PASS | "cannot confirm a claim's full evidence without SQL." |
| 2 | PASS | Node operator reviewing a specific claim. |
| 3 | PASS | bafyrei...1 w/ 2 URLs; no-evidence claim; unknown CID. |
| 4 | PASS | 3 scenarios. |
| 5 | PASS | AC-002.1–.4. |
| 6 | PASS | Single detail page, 3 scenarios; ~1 day. |
| 7 | PASS | OD-VIEW-3; depends US-VIEW-001. |
| 8 | PASS | Depends US-VIEW-001 (in this slice). |
| 9 | PASS | KPI-VIEW-1 legibility (100% evidence on page). |
| job_id | PASS | Job 1. |
| Elevator Pitch | PASS | After cites `/claims/{cid}`; output observable (evidence URLs). |

### US-VIEW-003 — Distinguish federated peer claims

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1 | PASS | "cannot tell, in the browser, what came from peers vs authored." |
| 2 | PASS | Operator of a federated node, 1,840 peer claims / 4 peers. |
| 3 | PASS | axum/axum from peer-A; no-peers; unknown-origin. |
| 4 | PASS | 3 scenarios. |
| 5 | PASS | AC-003.1–.4. |
| 6 | PASS | Single peer view, 3 scenarios; ~1–2 days. |
| 7 | PASS | OD-VIEW-3; depends US-VIEW-001 + slice-03. |
| 8 | PASS | peer_claims (slice-03, exists); US-VIEW-001. |
| 9 | PASS | KPI-VIEW-3 (100% origin shown, separable). |
| job_id | PASS | Job 1. |
| Elevator Pitch | PASS | After cites `/peer-claims`; output observable (rows w/ origin). |

### US-VIEW-004 — Navigate a large store with pagination

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1 | PASS | "unbounded single-page render is slow and unscannable at real scale." |
| 2 | PASS | Operator with 312 own + 1,840 peer claims. |
| 3 | PASS | 312@50/page next; last page bound; 12-claim single page. |
| 4 | PASS | 3 scenarios. |
| 5 | PASS | AC-004.1–.4. |
| 6 | PASS | Pagination on existing views, 3 scenarios; ~1–2 days. |
| 7 | PASS | OD-VIEW-4; depends US-VIEW-002. |
| 8 | PASS | Depends US-VIEW-002 (in this slice). |
| 9 | PASS | KPI-VIEW-1 at scale (<10s first page). |
| job_id | PASS | Job 1. |
| Elevator Pitch | PASS | After cites paging on My Claims; output observable (position indicator). |

### US-VIEW-005 — Browse live scrape proposals before signing in CLI

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1 | PASS | "deciding which candidates matter is awkward in CLI batch text." |
| 2 | PASS | Operator triaging candidates, network available. |
| 3 | PASS | tokio-rs/tokio 7 candidates; empty-repo; network down. |
| 4 | PASS | 4 scenarios. |
| 5 | PASS | AC-005.1–.5. |
| 6 | PASS | Single live view, 4 scenarios; ~2–3 days (reuses foundation). |
| 7 | PASS | Network req; slice-02 propose; no sign path (I-SCR-1); OD-VIEW-5. |
| 8 | PASS | US-VIEW-001 + slice-02 propose pipeline (exists). |
| 9 | PASS | KPI-VIEW-4 (browser triage → CLI sign). |
| job_id | PASS | Job 2. |
| Elevator Pitch | PASS | After cites `/scrape`; output observable (candidate list + derived-from). |

---

## Aggregate DoR Status

| Story | Job | DoR | Notes |
|-------|-----|-----|-------|
| US-VIEW-001 | Job 1 | PASS | Walking skeleton |
| US-VIEW-002 | Job 1 | PASS | |
| US-VIEW-003 | Job 1 | PASS | |
| US-VIEW-004 | Job 1 | PASS | |
| US-VIEW-005 | Job 2 | PASS | |

### DoR Status: PASSED (5/5 stories, all 9 items + job_id + Elevator Pitch)

- Every story traces to Job 1 or Job 2 (no orphans; no `infrastructure-only` needed —
  the walking skeleton US-VIEW-001 is itself a user-visible Job 1 story).
- Every non-infrastructure story has an Elevator Pitch with a real user-invocable entry
  point and observable output.
- Slice-level value check: every story is user-visible (no all-infrastructure slice).
- Happy-path bias check: every story carries edge + error scenarios; the read-only/no-sign
  guardrails are encoded as @property ACs.
