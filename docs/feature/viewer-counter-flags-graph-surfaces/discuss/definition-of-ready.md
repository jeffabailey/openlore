# Definition of Ready: viewer-counter-flags-graph-surfaces (slice-13)

> 9-item hard gate. Each item PASSES with evidence. Brownfield DELTA on slices
> 06/07/10/11/12; REUSES the slice-12 `counter_presence_for` read (no new read method).

## Per-story DoR

### US-CF-001 — Reuse the batch counter-presence read (`@infrastructure`)

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1. Problem statement clear, domain language | PASS | "The slice-12 batch presence read exists and is proven on /claims, but the /peer-claims, /project, /philosophy handlers do not yet collect their page CID set nor call it; a naive per-row call would reintroduce N+1." |
| 2. User/persona with specific characteristics | PASS | Infra-wiring story; persona P-001 indirectly (the plumbing US-CF-002/003 consume). `job_id: infrastructure-only` with rationale (slice has 2 user-visible stories → release value). |
| 3. 3+ domain examples with real data | PASS | (1) /peer-claims 12 rows → ONE call; (2) /project 3 groups / 11 edges → ONE flattened call; (3) empty/all-un-countered page → empty set, no query. |
| 4. UAT in Given/When/Then (3-7) | PASS | 3 scenarios (peer page one-query, traversal one-query-across-groups, empty→no-query). |
| 5. AC derived from UAT | PASS | 6 AC: one call per render; flatten across groups; query-count invariant to size; empty→no query; existing reads/SQL unchanged; no new StoreReadPort method. |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | Handler wiring only; REUSES the read. <1 day. |
| 7. Technical notes: constraints/dependencies | PASS | REUSES `counter_presence_for` (confirmed at store_read.rs:360-384); depends on slice-12 (SHIPPED); CID sets from `EdgeRow.cid` / `PeerClaimRowView.cid`. |
| 8. Dependencies resolved or tracked | PASS | slice-12 read SHIPPED; slice-10 surveys + slice-06 peer list SHIPPED. No open dependency. |
| 9. Outcome KPIs with measurable targets | PASS | "Exactly 1 `counter_presence_for` call per render, invariant to row/edge/group count (0 N+1)"; measured via real-subprocess behavioral assertion + inherited slice-12 adapter property. |

### US-CF-002 — "Countered" flag on `/peer-claims`

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1. Problem statement clear, domain language | PASS | "Maria scans /peer-claims to decide which peers' reasoning to engage; today she cannot tell which peer claims have already drawn a counter without opening each thread." |
| 2. User/persona with specific characteristics | PASS | P-001 "Maria", counter-claim-scanner hat, scanning the FEDERATED surface. |
| 3. 3+ domain examples with real data | PASS | (1) Tobias's cargo/dependency-pinning claim `bafy...t0bi` (0.88) flagged; (2) Rachel's tokio/async-first `bafy...rach` un-flagged; (3) `bafy...dup` countered by two authors → ONE marker. |
| 4. UAT in Given/When/Then (3-7) | PASS | 3 scenarios (flag links to thread + origin/confidence unchanged; htmx/no-JS parity; two-author → one marker). |
| 5. AC derived from UAT | PASS | 6 AC mapping each scenario (marker; one-hop link; parity; neutral text not verdict/count; origin+confidence+CID verbatim; N-author → ONE marker). |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | One view-model field + one render arm on an existing route; <1 day; 3 scenarios. |
| 7. Technical notes: constraints/dependencies | PASS | `PeerClaimRowView.is_countered` (slice-12 `from_row_with_presence` pattern); REUSE `render_list_presence_flag` shape; `render_peer_claim_row` site (~line 1059). |
| 8. Dependencies resolved or tracked | PASS | Depends on US-CF-001 (in-slice). slice-06 peer list + slice-11 thread + slice-12 read all SHIPPED. |
| 9. Outcome KPIs with measurable targets | PASS | Leading indicator OF KPI-FED-3 (navigate peer-list flag → thread); extends KPI-VIEW-3; per-feature GREEN, cohort via opt-in telemetry. |

### US-CF-003 — "Countered" flag on `/project` + `/philosophy` edges

| DoR Item | Status | Evidence |
|----------|--------|----------|
| 1. Problem statement clear, domain language | PASS | "Maria traverses /project + /philosophy to find which contributors span the projects/philosophies she cares about; today she cannot tell which edges (claims) are contested without copying each edge CID and opening its thread." |
| 2. User/persona with specific characteristics | PASS | P-001 "Maria", counter-claim-scanner hat, TRAVERSING the graph; decorates J-002b without changing it. |
| 3. 3+ domain examples with real data | PASS | (1) Tobias edge `bafy...t0bi` (0.88) flagged in the dependency-pinning group; (2) /philosophy flags only 2 of 11 edges across 3 groups; (3) `bafy...dup` countered twice → ONE marker, grouping unchanged. |
| 4. UAT in Given/When/Then (3-7) | PASS | 3 scenarios (countered edge marker + thread link + verbatim fields + unchanged position; philosophy flags only countered + byte-identical grouping/order/contributor list; htmx/no-JS parity + two-author → one marker). |
| 5. AC derived from UAT | PASS | 7 AC (marker + one-hop link; one EdgeRow arm covers both routes; un-countered edge unchanged; NEVER re-group/re-order/contributor/cross-link — byte-identical; parity both routes; N-author → ONE marker; never a sort/filter/group control). |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | One shared `EdgeRow` field + one render arm covering BOTH routes; <1 day; 3 scenarios. |
| 7. Technical notes: constraints/dependencies | PASS | `EdgeRow.is_countered` via `group_by` (~line 2116); shared `EdgeRow` render serves both `render_project_fragment` + `render_philosophy_fragment`; byte-identity via slice-12 baseline+marker-elision tactic. |
| 8. Dependencies resolved or tracked | PASS | Depends on US-CF-001 (in-slice). slice-10 surveys + slice-11 thread + slice-12 read all SHIPPED. |
| 9. Outcome KPIs with measurable targets | PASS | Leading indicator OF KPI-FED-3 (navigate edge flag → thread); guardrail KPI-GRAPH-2/4 (0 cases of the flag changing grouping/order); per-feature GREEN via the byte-identity gold. |

## DoR Status: PASSED (9/9 for all 3 stories)

## Elevator-Pitch Test (Dimension 0 — checked first, BLOCKING)

| Story | Pitch present (Before/After/Decision)? | Real entry point? | Concrete output? | Decision enabled? | Verdict |
|---|---|---|---|---|---|
| US-CF-001 | N/A — `@infrastructure` (no pitch required; produces no user-visible output) | — | — | enables US-CF-002/003 | PASS (infra) |
| US-CF-002 | YES (Before/After/Decision) | YES — `GET /peer-claims` (HTTP route on the real `openlore ui`) | YES — a rendered "Countered" marker + `<a href="/claims/{cid}">` link, peer-origin + confidence visible | YES — which contested peer claim to open first | PASS |
| US-CF-003 | YES (Before/After/Decision) | YES — `GET /project?subject=<uri>` / `GET /philosophy?object=<uri>` | YES — a rendered marker on the edge + link; grouping/order/confidence visibly unchanged | YES — whether to drill into a contested edge before trusting it | PASS |

**Slice-level check (Dimension 0.5):** the slice contains TWO non-`@infrastructure`,
user-visible stories (US-CF-002, US-CF-003). NOT every story is `@infrastructure` → the
slice has release value. PASS.

## JTBD traceability (hard-blocking per Decision 1, 2026-04-28)

| Story | `job_id` | Valid? |
|---|---|---|
| US-CF-001 | `infrastructure-only` + `infrastructure_rationale` (present) | PASS — slice retains ≥1 non-infra story |
| US-CF-002 | `J-003b` (in `docs/product/jobs.yaml`) | PASS |
| US-CF-003 | `J-003b` (in `docs/product/jobs.yaml`) | PASS |

## Anti-pattern scan

| Anti-pattern | Present? | Note |
|---|---|---|
| Implement-X | No | Stories start from Maria's pain (cannot see contested peer claims/edges while scanning). |
| Generic data | No | Real DIDs/CIDs/predicates (Tobias `bafy...t0bi`, Rachel `bafy...rach`, cargo/dependency-pinning, 0.88). |
| Technical AC | No | AC are observable outcomes (marker shown, link target, order/grouping byte-identical, parity) — not "use SQL X". |
| Technical scenario title | No | Scenarios describe what Maria achieves (spot a contested row/edge), not how the read works. |
| Oversized story | No | 3 stories, ≤3 scenarios each, ~1 day total; `/score` carved out to slice-14. |
| Abstract requirements | No | 3+ concrete examples per story with real data. |

## DoR overall verdict: PASSED (9/9, Dimension 0 PASS, JTBD PASS, anti-patterns clean)
