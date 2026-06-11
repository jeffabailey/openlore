# Definition of Ready Validation: viewer-search-full-follow-state (slice-20)

> 9-item hard gate. Both stories must PASS all 9 before DESIGN handoff.
> Item 9 (Outcome KPIs) is the OpenLore 9th item beyond the LeanUX-skill 8.
> JTBD traceability is enforced per Decision 1 (2026-04-28): every story carries a
> `job_id` referencing `docs/product/jobs.yaml`, or `infrastructure-only` + rationale.

## Story: US-FS-001 (Resolve `You` + `UnsubscribedCache` on `/search`; `@infrastructure`)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "The slice-16 `/search` resolution resolves ONLY the binary; the operator's own claims and her soft-removed peers' cached claims both misclassify as `NetworkUnfollowed`." Domain terms (own claim, soft-removed, cached residue, active set). |
| 2 | User/persona with specific characteristics | PASS | P-001 ("Maria"), network-discovery hat — indirectly (the resolution plumbing US-FS-002 consumes). |
| 3 | 3+ domain examples with real data | PASS | 4 examples with real DIDs (`did:plc:maria-test` own, `did:plc:rachel-test` active, `did:plc:tobias-test` soft-removed/cached, `did:plc:priya-test` new): four-arm resolution, precedence (active>cached), fragment-strip, cached-read degrade. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (four-arm resolution; active-outranks-cached precedence; fragment match; independent cached-read degrade). |
| 5 | AC derived from UAT | PASS | 10 AC items: four-arm resolution, `You`/`UnsubscribedCache` membership, precedence, batch-once (no N+1), `bare_did` strip, no-network, independent degrade, no-merge, no-new-variant/read-only. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | ~0.5–1 day; 4 scenarios; two LOCAL reads + a precedence resolution + threading. |
| 7 | Technical notes: constraints/dependencies | PASS | Resolution seam (`resolve_search_state`/`to_indexed_claim`), the LOCAL-graph resolver precedent, two NEW read-only `StoreReadPort` reads, `bare_did` SSOT, deps (slice-16/15/08 SHIPPED; two new reads NEW), READ-ONLY. |
| 8 | Dependencies resolved or tracked | PASS | slice-16 binary resolution + `read_local_active_set` + four-variant enum (SHIPPED); slice-15 active read (SHIPPED); slice-08 `/search` (SHIPPED). NEW: two read-only presence reads (in-slice). |
| 9 | Outcome KPIs with measurable targets | PASS | Who (viewer process) / Does what (four-arm resolution from LOCAL batch reads) / By how much (≤1 read each per render, 0 N+1; 100% own→`You`, 100% cached-inactive→`UnsubscribedCache`) / Measured by (behavioral via real subprocess) / Baseline (both states resolve `NetworkUnfollowed` today). |

**JTBD trace**: `job_id: infrastructure-only` + `infrastructure_rationale` present; slice contains
one non-infra user-visible story (US-FS-002) → Dimension-0 slice-level check passes.

### DoR Status: PASSED (9/9)

---

## Story: US-FS-002 (own claim → self indicator; removed peer → residue indicator; slice-16 unchanged)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Since slice-16, Maria's own claims and her soft-removed peers' cached claims BOTH still show `peer add` — she's told to add herself, and a deliberately-removed peer's cache looks like a fresh find." Domain language. |
| 2 | User/persona with specific characteristics | PASS | P-001 ("Maria"), network-discovery hat, scanning `/search` results in the browser; wants the four honest states at a glance. |
| 3 | 3+ domain examples with real data | PASS | 3 examples with real DIDs: four-arm side-by-side (own/Following/residue/add); no-regression (followed+new byte-stable); neutral-framing residue (no pejorative copy). |
| 4 | UAT in Given/When/Then (3-7) | PASS | 5 scenarios (own→self; removed→residue+neutral; slice-16 unchanged/no-regression; four side-by-side attributed; htmx/no-JS parity). |
| 5 | AC derived from UAT | PASS | 9 AC items: own→self+no-add, removed→residue+no-add, neutral copy, slice-16 byte-stable, render-only TEXT, attribution/ranking unchanged, verified/confidence unchanged, parity, read-only+LOCAL. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | ~0.5–1 day; 5 scenarios; two render arms (the empty `@match` arms already exist) + no-regression. |
| 7 | Technical notes: constraints/dependencies | PASS | Render arms (fill the empty `You|UnsubscribedCache` arm), two new SSOT constants, slice-16 arms reused verbatim, total `match`, dep on US-FS-001. |
| 8 | Dependencies resolved or tracked | PASS | US-FS-001 (in-slice — resolution); slice-16 `render_following_indicator` + `render_follow_guidance` (SHIPPED, reused verbatim). |
| 9 | Outcome KPIs with measurable targets | PASS | Who (P-001 operators on `/search`) / Does what (distinguish four states, follow only new) / By how much (`peer add` shown only where actionable — 0% on own/removed-cached; slice-16 byte-stable) / Measured by (per-feature GREEN + opt-in telemetry) / Baseline (post-slice-16 100% of these two states wrongly re-offered). |

**JTBD trace**: `job_id: J-005c` (`docs/product/jobs.yaml`, sub-job of J-005). **Elevator Pitch**
present (Before/After/Decision-enabled; "After" references the real `http://127.0.0.1:<port>/search`
entry point + concrete observable output — the self indicator, the residue indicator, the
`openlore peer add <did>` command text).

### DoR Status: PASSED (9/9)

---

## Slice-level DoR summary

| Story | job_id | Elevator Pitch | DoR |
|---|---|---|---|
| US-FS-001 | infrastructure-only (+rationale) | N/A (`@infrastructure`) | PASSED 9/9 |
| US-FS-002 | J-005c | PRESENT (real `GET /search` entry + observable output) | PASSED 9/9 |

**Dimension-0 slice-level check**: the slice contains ≥1 non-`@infrastructure`, user-visible story
(US-FS-002) with a complete Elevator Pitch → the slice has release value. PASS.

**Overall slice DoR: PASSED (9/9 both stories).**
