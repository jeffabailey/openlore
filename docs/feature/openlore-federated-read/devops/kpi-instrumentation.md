# KPI-FED Instrumentation — openlore-federated-read (slice-03)

- **Wave**: DEVOPS
- **Date**: 2026-05-27
- **Architect**: Apex
- **Source-of-truth KPIs**: `docs/feature/openlore-federated-read/discuss/outcome-kpis.md`
- **Foundation cross-link**: `docs/feature/openlore-foundation/devops/kpi-instrumentation.md` (KPI-1..6 — still in force, unchanged)

This document traces each of the 6 outcome KPI-FED targets to **what**
measures it, **where** the data lives, **how** the developer-as-operator
reads it, and a **feasibility tag** (GREEN/YELLOW/RED) for slice-03.

## 1. Summary table

| KPI | Type | Instrumentation | Read mechanism | Feasibility |
|---|---|---|---|---|
| KPI-FED-1 (attribution fidelity = 100%) | Leading (Outcome) | AT `at-federation-attribution-preserved` + runtime counter `peer_render_attribution_missing_total` | CI status + `openlore stats --federation` | **GREEN** |
| KPI-FED-2 (zero merged consensus rows) | Leading (Guardrail) | Same AT as KPI-FED-1 (both share the attribution invariant) + adversarial renderer review | CI status + manual review checklist at release | **GREEN** |
| KPI-FED-3 (≥30% dogfood cohort publishes ≥1 counter-claim in 30d) | Leading / North Star | `tracing` event `claim.counter.published` + 30-day survey delivered via the same prompt mechanism as KPI-3 | per-user: `openlore stats --federation`. Cohort: requires future telemetry endpoint | **YELLOW** (per-user GREEN; cohort aggregation deferred — same constraint as foundation KPI-3, KPI-6) |
| KPI-FED-4 (100% — zero residue after `peer remove --purge`) | Leading (Guardrail) | AT `at-peer-remove-purge-zero-residue` + runtime counter `peer_purge_residue_after_purge_total` | CI status + `openlore stats --federation` | **GREEN** |
| KPI-FED-5 (subscribe→pull→query end-to-end < 90s for peer ≤20 claims) | Leading (Outcome) | `tracing` events + post-hoc jq aggregation `scripts/kpi-fed-5.jq` (or `federation.e2e.timing` event if DESIGN picks state-file approach) | per-user: `openlore stats --federation`. Cohort: deferred | **GREEN** for per-user signal; **YELLOW** for cohort percentile |
| KPI-FED-6 (100% — no invalid signatures stored) | Leading (Guardrail / Security) | AT `at-peer-tampered-signature-rejected` against adversarial fixture + runtime counter `peer_records_rejected_total` | CI status (release-blocking); `openlore stats --rejections` for forensic detail | **GREEN** |

**No KPI-FED is RED.** Every one has a designed capture mechanism that
ships in slice-03. Two are YELLOW (KPI-FED-3 cohort and KPI-FED-5 cohort
percentile) because cohort aggregation across users requires the future
opt-in telemetry endpoint (per ADR-010) which slice-03 does NOT operate —
exactly the same constraint as foundation KPI-3 and KPI-6.

## 2. KPI-FED-1 — Attribution fidelity = 100%

### What
Every rendered peer claim carries a non-null `author_did` field traceable
back to the originating peer's DID. Across every UI surface (currently:
`peer pull` summary, `graph query --federated` output) every rendered row
has author attribution; ZERO rows show a "synthesized" or "merged" author.

### Where the data lives
- **CI signal**: `at-federation-attribution-preserved` test result (per `ci-cd-pipeline.md` delta §3.1).
- **Runtime signal**: `tracing` event `peer.claim.rendered` with `has_author_did_field` bool field; counter `peer_render_attribution_missing_total` aggregates events where the bool is false. Target = 0 forever.

### How the developer reads it
- **CI**: GitHub Actions check; release-blocking.
- **Runtime**: `openlore stats --federation` shows `Attribution-missing renders: 0` (anything else is a P0 bug-worthy event); also the WARN-level tracing emission surfaces it in stderr.

### Aggregation across cohort
- CI signal IS the cohort signal — if the test passes, EVERY user's binary has the property (same logic as foundation KPI-4).
- Per-user runtime counter is local; aggregation across users requires future telemetry endpoint.

### Feasibility: GREEN
CI gate designed in `ci-cd-pipeline.md` delta §3.1. Counter designed in
`observability.md` delta §4 (`peer_render_attribution_missing_total` row).

## 3. KPI-FED-2 — Zero merged consensus rows

### What
ZERO occurrences of "merged consensus" rendering of multi-author claims
across all UI surfaces. The system NEVER collapses two authors' claims
about the same subject into a single synthesized row.

### Where the data lives
Same as KPI-FED-1 — KPI-FED-2 is the negation of an attribution failure
mode. The `at-federation-attribution-preserved` test directly asserts no
synthesized-author rows are produced; it covers both KPIs by construction.

Additionally: adversarial renderer review at release time. DELIVER ships
`docs/dev/renderer-review-checklist.md` (proposed; flagged for DELIVER) —
a one-page checklist the developer walks through before tagging a release,
listing every renderer surface and asking "could this collapse authors?".
Solo dev = self-review; the checklist exists to prevent forgetting.

### How the developer reads it
- CI status (same as KPI-FED-1).
- Release-time checklist completion (a single line in the release CHANGELOG: "Renderer review: passed YYYY-MM-DD").

### Aggregation across cohort
CI signal IS the cohort signal.

### Feasibility: GREEN
CI gate same as KPI-FED-1. Renderer-review checklist is a documentation
deliverable for DELIVER; trivially feasible.

## 4. KPI-FED-3 — ≥30% dogfood cohort publishes ≥1 counter-claim within 30 days (NORTH STAR)

### What
Per `outcome-kpis.md` §North Star: ≥30% of dogfood cohort (federation-reader
hat — P-002 primary, P-001 in reader hat) publishes ≥1 counter-claim within
30 days of slice-03 release, AND describes the experience at day-30
interview as "as light as posting a comment, but more structured."

This is a behavioral hypothesis — the slice's value evaporates if engineers
technically CAN counter-claim but never DO.

### Where the data lives

**Per-user behavioral signal** (always captured):
- JSON log file (foundation §3.3): every `claim.counter.published` event is
  emitted with `{counter_cid, target_cid, target_author_did, reason_len}`.
- Per-user counter `counter_claims_published_total` aggregates from the log
  (per `observability.md` delta §4).

**Per-user qualitative signal** (one-shot survey delivered after first
counter-claim publish):
- Delivery state: `$XDG_DATA_HOME/openlore/surveys/post-counter-claim.status` — same one-shot file-presence pattern as foundation KPI-3 survey.
- Response: `$XDG_DATA_HOME/openlore/surveys/post-counter-claim.response.json`:
```json
{
  "kpi": "KPI-FED-3",
  "delivered_at": "2026-06-04T12:34:56Z",
  "answered_at": "2026-06-04T12:35:42Z",
  "score_lightness": 4,
  "score_structure": 5,
  "free_text_optional": null
}
```
Two 5-point Likert questions:
1. "Authoring the counter-claim felt as light as posting a comment." (1=disagree, 5=agree)
2. "The structure (target_cid + reason) helped me articulate the disagreement." (1=disagree, 5=agree)

**Cohort behavioral signal** (requires future telemetry endpoint):
- Aggregation of `counter_claims_published_total` across users + 30-day cohort denominator. Requires the future opt-in telemetry endpoint.

### How the developer reads it
- **Per-user**: `openlore stats --federation` shows `Counter-claims published: N`; `openlore stats --surveys` shows the qualitative response.
- **Per-user fallback**: `scripts/kpi-fed-3.jq` snippet (count of `claim.counter.published` events in the log; works offline against the JSON log).
- **Cohort**: NOT in slice-03. Out-of-band: Luna (PO) coordinates direct outreach to known dogfood users at day-30 for the survey + counter-claim count, AS the slice-01 model did for KPI-3 and KPI-6.

### Delivery mechanism for the survey
After the FIRST `claim.counter.published` event in a session AND the
post-counter-claim status file is absent, the CLI prints (after the success
message):
```
A quick one-time question to improve OpenLore (press Enter to skip):
  1. Authoring this counter-claim felt as light as posting a comment.
     Rate 1-5 (1=disagree, 5=agree, Enter=skip): _
  2. The structure (target + reason) helped me articulate the disagreement.
     Rate 1-5 (1=disagree, 5=agree, Enter=skip): _
```
Status file is written regardless of answer (so it never re-prompts).

### Aggregation across cohort
- **Per-user signal**: GREEN, fully captured in slice-03.
- **Cohort aggregation**: requires future telemetry endpoint OR out-of-band PO outreach. **Telemetry rule**: ONLY the Likert scores + the boolean "published-at-least-one-counter-claim-in-30d" can ever be sent; the `target_cid`, `target_author_did`, `reason_len`, and any free-text are NEVER sent (per `observability.md` delta §7).

### Feasibility: YELLOW
Delivery mechanism is solid and ships in slice-03. Per-user behavioral +
qualitative signal captured. Cohort aggregation requires future endpoint
(same constraint as foundation KPI-3, KPI-6 — DEVOPS-future problem).

The slice IS shippable with YELLOW status on this KPI — the per-user
signal suffices for dogfood-cohort PO outreach within 30 days of release;
the cohort aggregation is a nice-to-have for scale.

## 5. KPI-FED-4 — Zero residue after `peer remove --purge`

### What
For every `peer remove --purge` invocation, subsequent federated queries
return ZERO peer_claims rows for the purged DID. The user's own
counter-claims authored against that peer's claims survive (per WD-25).

### Where the data lives
- **CI signal**: `at-peer-remove-purge-zero-residue` test result (`ci-cd-pipeline.md` delta §3.5).
- **Runtime signal**: counter `peer_purge_residue_after_purge_total` aggregates `peer.removed.purge_complete.residue_rows_after_purge`. Target = 0 forever; non-zero is a P0 bug.

### How the developer reads it
- **CI**: GitHub Actions check; release-blocking.
- **Runtime**: `openlore stats --federation` shows `Purge residue after operation: 0`.
- **Forensic**: log entries `peer.removed{purge: true, residue_rows_before_purge: N}` followed by `peer.removed.purge_complete{residue_rows_after_purge: 0}` give the audit trail.

### Aggregation across cohort
CI gate IS cohort signal. The runtime counter aggregates per-user (deferred
to future endpoint, but irrelevant — if CI passes the value is 0 everywhere).

### Feasibility: GREEN
CI gate fully designed. Runtime counter fully designed.

## 6. KPI-FED-5 — End-to-end subscribe→pull→query < 90s (peer ≤20 claims)

### What
Wall-clock interval from `peer add <did>` invocation start to the first
result row rendered by `graph query --federated` after a `peer pull` of
that DID. Target: < 90s P50 for a peer publishing ≤20 claims; bucketed by
peer-claim-count (≤5 / 6-20 / 21-100 / >100) per `outcome-kpis.md` §3
Handoff to DEVOPS item 2.

### Where the data lives
- **Per-user**: JSON log events `peer.added`, `peer.pull.completed`, `query.executed{kind: federated}`. Aggregation via either:
  - **Post-hoc jq** (default, no state file): `scripts/kpi-fed-5.jq` joins events by peer_did within a session window, computes total ms. Per `observability.md` delta §2.5 default.
  - **State-file `flow_id`** (alternative if DESIGN picks): single `federation.e2e.timing` event with pre-joined fields. Cleaner reads; costs a state file in `$XDG_DATA_HOME/openlore/state/`.
- **CI sanity**: `at-peer-cid-round-trip` exercises a ≤20-record fixture; wall-clock of the test bounds the implementation. Not a strict KPI test; just a regression signal.

### How the developer reads it
- **Per-user (post-hoc default)**:
```
$ jq -sf scripts/kpi-fed-5.jq $XDG_DATA_HOME/openlore/logs/openlore.log
{
  "le5":    { "p50_s": 32, "p95_s": 58, "samples": 12 },
  "6to20":  { "p50_s": 51, "p95_s": 81, "samples":  7 },
  "21to100":{ "p50_s": 88, "p95_s": 154, "samples":  3 },
  "gt100":  { "p50_s": 0,  "p95_s": 0,  "samples":  0 }
}
```
- **Per-user (verb)**: `openlore stats --federation` renders the bucketed P50/P95.
- **Alert**: per `outcome-kpis.md` Handoff to DEVOPS item 3, P95 > 180s is informational (do NOT block release alone, escalate to PO). In slice-03 the "alert" is "the operator notices on their own `openlore stats` output"; no automated paging.

### Aggregation across cohort
- **Per-user**: GREEN.
- **Cohort percentiles** (P50/P95 across all users): requires future telemetry endpoint. The histogram event is shaped to be aggregable (bucketed peer-cardinality + bucketed duration); rollup is straightforward once an endpoint exists.

### Feasibility
**GREEN** for per-user signal (full capture, readable today).
**YELLOW** for cohort-percentile aggregation (deferred — future endpoint).

## 7. KPI-FED-6 — 100% no invalid signatures stored

### What
For every `peer pull` that encounters one or more invalid records
(tampered signature, CID mismatch, or both), ZERO of those records reach
the `peer_claims` table. The valid records in the same pull DO reach the
table (per-claim reject semantics — WD-24).

### Where the data lives
- **CI signal (PRIMARY)**: `at-peer-tampered-signature-rejected` test result (`ci-cd-pipeline.md` delta §3.3). Adversarial fixture publishes three tampered-record flavors mixed with valid records; the test asserts all three rejected, valid ones stored, structured-log `peer.pull.rejected{reason}` event emitted per rejected record.
- **Runtime signal**: counter `peer_records_rejected_total{reason}` aggregates `peer.pull.rejected` events; counter `peer_records_stored_total` aggregates `peer.pull.record.stored` events. The invariant is:
  - For every `peer.pull.record.received` there is exactly ONE `peer.pull.record.verified`.
  - For every `peer.pull.record.verified{verify_outcome: ok}` there is exactly ONE `peer.pull.record.stored`.
  - For every `peer.pull.record.verified{verify_outcome != ok}` there is exactly ONE `peer.pull.rejected`.
  - NO `peer.pull.record.stored` event is preceded by `peer.pull.record.verified{verify_outcome != ok}` for the same `record_cid`.

The fourth invariant is the load-bearing KPI-FED-6 invariant; it's
testable via the `scripts/kpi-fed-6.jq` snippet against the log.

### How the developer reads it
- **CI**: GitHub Actions check; release-blocking.
- **Runtime**: `openlore stats --federation` shows `Peer records rejected: N (sig_invalid: A, cid_mismatch: B, lexicon_invalid: C, duplicate: D)`. Non-zero rejections are NOT a bug — they mean the system worked when an adversarial record was encountered. Zero rejections in a session is also normal (no adversarial peers in the user's subscription set).
- **Forensic**: `openlore stats --rejections` shows per-event detail.

### Adversarial fixture
The fixture is the load-bearing piece of CI infrastructure for KPI-FED-6
(per task spec Phase 1 + Phase 2). Per `ci-cd-pipeline.md` delta §3.3:
- Location: `tests/fixtures/peer-adversarial/`
- Contents: three tampered-record bodies (bad_sig / mutated_body / wrong_cid)
  + a wiremock setup file that serves them via `com.atproto.repo.listRecords`
  alongside 2-3 valid records.
- Maintenance: `cargo xtask regenerate-peer-fixtures` regenerates from the
  current Lexicon (per `ci-cd-pipeline.md` delta §7); CI's `arch-check`
  stage verifies the committed bodies are current.

### Aggregation across cohort
CI gate IS cohort signal — if the test passes, EVERY user's binary
rejects tampered records. Per-user runtime counters are observational
metadata (how often does the user encounter adversarial peers), not a
guarantee — the guarantee is in the CI gate.

### Feasibility: GREEN
CI gate + adversarial fixture fully designed in `ci-cd-pipeline.md` delta
§3.3 + §7. Runtime counter fully designed.

## 8. Cross-references

- `observability.md` delta §2 — event names that back KPI-FED-1..6 measurement
- `observability.md` delta §4 — metric rows derived from those events
- `ci-cd-pipeline.md` delta §3.1 (KPI-FED-1, 2), §3.3 (KPI-FED-6), §3.5 (KPI-FED-4), §3.7 (timing target)
- Foundation `kpi-instrumentation.md` — KPI-1..6 unchanged
- `discuss/outcome-kpis.md` — authoritative KPI-FED definitions
- ADR-010 — telemetry-opt-in (governs the YELLOW path for KPI-FED-3 cohort and KPI-FED-5 cohort)

## 9. KPI-FED sign-off readiness (per task spec §Phase 6)

| KPI-FED | Per-user instrumented in slice-03? | Cohort aggregated? | Status |
|---|---|---|---|
| KPI-FED-1 | YES (CI gate + runtime counter) | YES (CI = cohort property) | **GREEN** |
| KPI-FED-2 | YES (CI gate, shared with KPI-FED-1) + release-time renderer review | YES (CI = cohort property) | **GREEN** |
| KPI-FED-3 | YES (`claim.counter.published` event + 2-question Likert survey) | NO (future telemetry endpoint OR PO out-of-band outreach at day-30) | **YELLOW** |
| KPI-FED-4 | YES (CI gate + runtime counter) | YES (CI = cohort property) | **GREEN** |
| KPI-FED-5 | YES (post-hoc jq or `federation.e2e.timing` event) | NO (future telemetry endpoint for cohort percentiles) | **GREEN per-user / YELLOW cohort** |
| KPI-FED-6 | YES (CI gate against adversarial fixture + runtime counter) | YES (CI = cohort property) | **GREEN** |

**No KPI-FED is RED.** All six are at minimum per-user-readable from the
slice-03 binary. The two YELLOW items (KPI-FED-3 cohort, KPI-FED-5 cohort
percentile) reflect the same deferred-cohort-endpoint constraint that
foundation KPI-3 and KPI-6 carry — not a capture gap.

## 10. Map to slice-01 KPIs (per `outcome-kpis.md` §Mapping)

| slice-01 KPI | Status in slice-03 instrumentation |
|---|---|
| KPI-4 (zero silent normalization, 100% round-trip identity) | Inherited and EXTENDED. The CI test `at-peer-cid-round-trip` extends KPI-4's round-trip invariant to peer-sourced claims; the foundation's `field_mismatch_total` counter scope widens (DELIVER amends the counter source to also emit on peer-claim render-time mismatches). |
| KPI-5 (local-first invariant, network-disabled correctness) | Inherited UNCHANGED. Federated queries on locally-cached peer claims work without network — already covered by `kpi-5-offline` integration test (DELIVER may extend the test setup to seed a peer_claims row before the `unshare -n` step; flagged as optional reinforcement). |
| KPI-1 (under 2 min e2e for slice-01 walking skeleton) | Slice-03 introduces KPI-FED-5 as the parallel measurement for the slice-03 walking skeleton (90s budget). Independent counter; coexists with KPI-1. |
