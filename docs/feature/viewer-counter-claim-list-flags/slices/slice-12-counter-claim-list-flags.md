# Slice 12: viewer-counter-claim-list-flags

> Goal: On the `/claims` list, flag (neutrally) which of the operator's claims have
> been countered, so she can spot disagreement at a glance before drilling in.

## IN scope

- Extend `GET /claims` (slice-06 list) so each row whose claim has ≥1 counter shows a
  neutral "Countered" marker (slice-11 `COUNTERED_PRESENCE_FLAG`, reused verbatim),
  rendered as a one-hop `<a href="/claims/{cid}">` link to that claim's slice-11 thread.
- New read-only BATCH `StoreReadPort::counter_presence_for(&[cid])` — ONE aggregate
  query over the indexed `claim_references ∪ peer_claim_references` (no N+1, no per-row
  artifact read).
- Un-countered rows show no marker (no-noise); list order/paging/count/confidence
  byte-identical to slice-06 (shown-never-applied + no-regression).

## OUT scope

- Authoring/composing a counter on the viewer (stays CLI `claim counter`).
- Re-rank / re-order / filter / re-weight / paginate the list by counter presence.
- A count, "disputed by N", or any aggregate verdict on the list row (presence-only boolean).
- Reason text on the list flag (the verbatim reasons are the slice-11 thread's job).
- Any network seam; any per-row artifact read; any N+1 query.
- Flagging `/peer-claims`, `/project`, `/philosophy`, `/score` rows → deferred to slice-13.

## Learning hypothesis

- **Disproves if it fails**: if operators do NOT navigate from the list flag to the
  thread (telemetry/dogfood shows no list-flag → detail navigation), then at-a-glance
  list discoverability was NOT the missing piece for counter engagement — drill-in
  legibility (slice-11) was sufficient, and KPI-FED-3's READ side is not list-gated.
- **Confirms if it succeeds**: operators triage from the list (flag → thread) within
  the same session they author/pull a counter — list discoverability is a real leading
  indicator of KPI-FED-3.

## Acceptance criteria (summary; full BDD in user-stories.md)

- A countered row shows the neutral "Countered" marker linking to `/claims/{cid}`.
- The presence lookup is ONE query per page, invariant to page size (N+1 guard, gold).
- Un-countered rows render byte-identically to slice-06 (no marker, no noise).
- List order/paging/count and every row's confidence are byte-identical with/without the flag.
- A claim with N counters shows ONE neutral marker (no count, no verdict).
- Read-only gold: store row counts unchanged; offline render works (LOCAL read).

## Dependencies

- slice-06 `/claims` list (`list_claims`, `ClaimRowView`, `render_claim_row`) — exists.
- slice-11 `COUNTERED_PRESENCE_FLAG` + `/claims/{cid}` thread (link target) — exists.
- Indexed `claim_references` / `peer_claim_references` (slice-03) — exist.
- No new crate (workspace stays 21). No new route. No new KPI ID.

## Effort estimate

~1 day. One batch read method (widens slice-11 Step-A indexed lookup to `IN (...)`)
+ one per-row render marker on an existing route. Reference class: slice-11
(~1 day for the thread read + render on an existing route); slice-12 is thinner
(no Step-B artifact read, no new ADT — a bool field + a marker).

## Pre-slice SPIKE

None required. The presence read is a direct widening of an existing, tested query
(`query_counter_claims` Step-A); the render extension is a known pattern. The only
DESIGN decision (DuckDB `IN (...)` binding shape) is low-risk and well-trodden.

## Scope fork (DECISION for the user — see feature-delta.md)

`/claims`-only is the recommended thinnest slice. Flagging the slice-10
`/project`+`/philosophy` edge rows and the slice-09 `/score` rows is DEFERRED to a
recommended **slice-13 (`viewer-counter-flags-graph-surfaces`)**. Confirm before
expanding scope (expansion breaks the ≤1-day budget).
