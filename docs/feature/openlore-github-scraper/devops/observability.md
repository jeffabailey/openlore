# Observability Extension — openlore-github-scraper (slice-02)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex

This is the slice-02 **delta** to `observability.md` (foundation) and the
slice-03 observability delta. Operating model (developer-as-operator;
local-first; no remote sink; telemetry opt-in OFF by default; no dashboards;
no continuous alerting) is **UNCHANGED**. The tracing pipeline (D-D2:
`tracing` + `tracing-subscriber` + `tracing-appender` + JSON Lines local) is
**REUSED unchanged — no new endpoint, no new sink**. This doc adds:

1. New `tracing` event names emitted by the new scrape code paths.
2. A `from_scraper` tag on the REUSED slice-01 sign/publish events.
3. New per-event metric rows.
4. A new probe event for the GitHub harvest path.
5. New `openlore stats --scraper` rendering rows (assumes D-D5 verb landed;
   otherwise the same fallback applies — read events directly via `jq`).

## 1. Pillars (UNCHANGED)

Logs YES (same JSON Lines sink). Metrics YES (same on-demand aggregation from
log). Traces still DEFERRED with the same revisit trigger (slice-02 is still a
single-binary CLI; `scrape github` is a single bounded HTTP harvest from the
user's machine — nothing distributed). The `tracing` span per scrape operation
is in-process scoping for log enrichment + the `scrape_id` correlation, NOT
distributed tracing.

## 2. Logging — new events emitted

### 2.1 Scrape harvest boundary events (mandatory)

- `scrape.started` — payload `{ scrape_id, target: String, target_kind: "repo"|"user"|"unresolved", auth_mode: "anonymous"|"authenticated", public_data_banner_shown: bool }`. Emitted when `scrape github <target>` begins, AFTER the public-data banner is printed. `public_data_banner_shown` MUST be `true` (US-SCR-001 AC; CI-watched). `scrape_id` is a per-invocation UUID threaded through every subsequent event in this scrape (the KPI-SCR-1 correlation key — D-D27).
- `scrape.harvest.completed` — payload `{ scrape_id, target, target_kind, signal_count: u32, harvest_duration_ms: u64, rate_budget_remaining: Option<u32> }`. Emitted when `GithubPort` harvest returns. `signal_count` is the number of public signals harvested. `rate_budget_remaining` is read from `X-RateLimit-Remaining` (Some when budget headers present; None on a shape change — the probe step-4 guard catches a missing-header regression). **This is the KPI-SCR-1 start-of-duration anchor** (per `outcome-kpis.md` Handoff item 1). The token value is NEVER in any field.
- `scrape.candidates.derived` — payload `{ scrape_id, target, count: u32, source_signal_coverage: "all"|"partial", confidence_max: f64 }`. Emitted when `scraper-domain::derive_candidates` returns. `count` is the number of candidate claims derived. `source_signal_coverage` MUST be `"all"` (every candidate names ≥1 source signal — KPI-SCR-3; CI-watched). `confidence_max` MUST be ≤0.3 (no auto-inflate — KPI-SCR-2/WD-52; CI-watched). This event is the load-bearing KPI-SCR-3 measurement.
- `scrape.candidate.rendered` — payload `{ scrape_id, candidate_index: u32, predicate: String, source_signal_count: u32 }`. One event per candidate rendered to the candidate list. `source_signal_count` MUST be ≥1 for every emission (KPI-SCR-3 runtime guardrail; the field is logged so CI can assert the invariant rather than relying on adversarial review). DEBUG level.
- `scrape.completed` — payload `{ scrape_id, target, candidates_derived: u32, candidates_signed: u32, signed_unsigned_residue: u32, total_duration_ms: u64 }`. One event per `scrape github` invocation. `signed_unsigned_residue` MUST be 0 (nothing persisted unsigned — KPI-SCR-2/WD-55; CI-watched). When invoked without `--sign`, `candidates_signed` is 0 and the event still asserts `signed_unsigned_residue == 0`.

### 2.2 Refusal events (mandatory)

- `scrape.refused` — payload `{ scrape_id, target, reason: NotPublic|NotFound|RateLimited|TokenRejected|Network, detail: String }`. Emitted instead of `scrape.harvest.completed` when the harvest is refused. The `reason` field is the load-bearing per-refusal assertion; it MUST be exactly one of the listed variants. For `NotPublic` (KPI-SCR-4), zero candidates are rendered and the CLI exits non-zero. `detail` NEVER contains the token value. WARN level (a refusal is operator-visible by default — same level as the slice-03 `peer.pull.rejected`).

### 2.3 Sign-from-scraper events (REUSE + tag)

The candidate→compose→sign→publish path REUSES the slice-01 `VerbClaimAdd` +
`VerbClaimPublish` internals (WD-66). The existing `compose.*`, `sign.success`,
and `publish.*` events are emitted UNCHANGED, with ONE additional correlation
field and ONE new boundary event:

- The reused `sign.success` and `publish.published` events gain an OPTIONAL
  `scrape_id: Option<String>` field — present (Some) when the sign originated
  from a scraper candidate, absent (None) for a hand-authored claim. This is the
  KPI-SCR-1 end-of-duration anchor and the KPI-SCR-5 diff anchor. It does NOT
  alter the SIGNED payload (display-only provenance — WD-62); it's a log-only
  correlation field.
- `claim.signed.from_scraper` — payload `{ scrape_id, target, candidate_index: u32, predicate: String, proposed_confidence: f64, signed_confidence: f64, fields_edited: Vec<String>, edit_count: u32 }`. Emitted on EACH successful sign-from-scraper, AFTER the reused `sign.success`. `fields_edited` is the per-field diff between the proposed candidate and the signed payload (predicate / evidence / confidence). `edit_count` ≥1 on a real human-in-the-loop edit. **This event is the load-bearing KPI-SCR-5 measurement (edit-rate) AND the KPI-SCR-1 end anchor** (per `outcome-kpis.md` Handoff item 1). INFO level.

Invariant: for every `scrape.completed{candidates_signed: N}` there are exactly
N `claim.signed.from_scraper` events with the same `scrape_id`. This invariant
is testable via a `jq` script (see `kpi-instrumentation.md` delta §2).

### 2.4 Probe extension event (mandatory)

- `probe.github_harvest_path` — payload `{ adapter: "adapter-github", step: 1..5, outcome: ok|refused, reason: Option<ProbeRefusalReason>, detail, latency_ms: u64 }`. Emitted by the new `adapter-github` probe per the five-step contract (architecture-design §6.3): (1) public reachability; (2) private refusal; (3) auth-mode report; (4) rate-limit-header presence; (5) no-token-leak. On any refusal it emits `health.startup.refused{ reason: github.* }` and the system refuses to start (exit 2), per the foundation gauntlet semantics. `latency_ms` MUST be ≤250 (the I-5 probe budget). The token value is NEVER in any probe event field (step 5 asserts this).

### 2.5 KPI-SCR-1 timing (post-hoc default — D-D27)

KPI-SCR-1 (cost-to-first-claim: `scrape github` → signed claim, < 2 min) is
computed POST-HOC by joining, per `scrape_id`:

- start anchor: `scrape.harvest.completed.harvest_duration_ms` start timestamp (or `scrape.started` if including the banner-read time — the budget is from `scrape github` invocation, so `scrape.started` is the true start);
- end anchor: the first `claim.signed.from_scraper` event with the same `scrape_id`.

The interval `t(claim.signed.from_scraper) - t(scrape.started)` is the
cost-to-first-claim. NO state file (mirrors slice-03 D-D16's post-hoc default
for KPI-FED-5). The `scrape_id` removes the join ambiguity that slice-03's
peer_did-window join had; DELIVER threads the `scrape_id` (Q-DELIVER in
wave-decisions §Open questions to DELIVER item 2) — if DELIVER opts out, the jq
falls back to a session-window join. Bucketed by `target_kind` (small repo /
large repo / user) per `outcome-kpis.md` Handoff item 2.

### 2.6 What did NOT change

All foundation events (verb.invoked, port.call, port.return, compose.*,
sign.success, publish.*, retract.composed, query.executed, health.*) and all
slice-03 events (peer.*, claim.counter.*, federation.e2e.timing) remain emitted
unchanged. The new code paths emit ADDITIONAL events; the reused sign/publish
events gain ONE optional correlation field; nothing is suppressed or renamed.

## 3. Probes (extension)

Foundation `observability.md` §7.2 + slice-03 §3 list per-adapter probe
responsibilities. Slice-02 adds ONE new adapter probe:

| Adapter | New probe responsibility for slice-02 |
|---|---|
| `adapter-github` (GithubPort, NEW) | The full five-step probe (architecture-design §6.3): (1) `resolve_target` against a known-stable PUBLIC fixture returns `Repo` within the 250ms budget; (2) `resolve_target` against a known-private/inaccessible fixture returns `GithubError::NotPublic` (the KPI-SCR-4 probe-layer gate — a private 404 MUST be refused, not treated as missing-but-harvestable); (3) if `GITHUB_TOKEN` set, confirm it is accepted (read rate-limit headers) and report the budget, else refuse with `GithubTokenRejected`; (4) assert rate-limit headers are parseable; (5) assert the token value never appears in any structured probe event or log line. On any refusal: `health.startup.refused{ reason: github.* }` and exit 2. |

Three-layer probe enforcement (per ADR-009) extends to `adapter-github` exactly
as for the slice-01/03 adapters: compile-time (the trait requires `probe()`);
structural (`xtask check-probes` AST-walks the `impl GithubPort` block, asserts
a non-stub body — covered automatically, no CI change per ci-cd §2); behavioral
(the five-step gold-test against the FakeGithub substrate lies:
private-repo-404, rate-limit-403, rejected-token-401 — not just a happy-path
public fetch).

The `--offline` flag (foundation §7.3, slice-03 §8) SKIPS the
`adapter-github` probe (it is a network probe). However `scrape github` itself
REFUSES to run offline (it requires network — architecture-design §6.3); the
existing PDS-skip logic generalizes. So `--offline` skips the probe at startup,
but a subsequent `scrape` invocation refuses with `GithubError::Network`
("scrape requires network").

## 4. Metrics — new rows for `openlore stats`

Append to foundation §4.3 + slice-03 §4 table:

| Metric | Source events | Render shape | Used by |
|---|---|---|---|
| `scrapes_total` | count of `scrape.completed` | counter, per-target-kind breakdown | operational visibility |
| `scrape_harvest_signals` | `scrape.harvest.completed.signal_count` | histogram, per target-kind | context (harvest yield) |
| `scrape_to_sign_seconds` | post-hoc join `scrape.started` → first `claim.signed.from_scraper` per `scrape_id` (per §2.5) | histogram (p50/p95), per target-kind bucket | **KPI-SCR-1** (the north-star measurement) |
| `scrape_candidates_derived_total` | sum of `scrape.candidates.derived.count` | counter | context |
| `scraper_candidate_missing_source_total` | count of `scrape.candidate.rendered` where `source_signal_count == 0` | counter; target = 0 forever | **KPI-SCR-3** (runtime guardrail; non-zero is a P0 bug — analogous to slice-03 `peer_render_attribution_missing_total`) |
| `scraper_confidence_autoinflate_total` | count of `scrape.candidates.derived` where `confidence_max > 0.3` | counter; target = 0 forever | **KPI-SCR-2** (runtime guardrail; non-zero is a P0 bug) |
| `scraper_unsigned_residue_total` | sum of `scrape.completed.signed_unsigned_residue` | counter; target = 0 forever | **KPI-SCR-2** (runtime guardrail; non-zero is a P0 human-gate breach) |
| `claims_signed_from_scraper_total` | count of `claim.signed.from_scraper` | counter, per-target breakdown | KPI-SCR-1 / KPI-SCR-5 context |
| `scraper_edit_rate` | (count of `claim.signed.from_scraper` where `edit_count ≥ 1`) / (count of `claim.signed.from_scraper`) | ratio over a 30-day window | **KPI-SCR-5** (edit-rate ≥50%) |
| `scrape_refusals_total{reason}` | count of `scrape.refused` grouped by `reason` | counter, per-reason | KPI-SCR-4 context (NotPublic refusals are the system working correctly; non-zero is normal) |

### 4.1 `openlore stats` rendering additions

Append to foundation §4.2 + slice-03 §4.1 commands:

| Command | Renders |
|---|---|
| `openlore stats --scraper` | Summary card: total scrapes (per target-kind), scrape→sign p50/p95, candidates derived, claims signed from scraper, edit-rate (30-day), refusals (with reason breakdown), and the three guardrail counters (missing-source: 0, confidence-autoinflate: 0, unsigned-residue: 0). |
| `openlore stats --scraper --since <date>` | Same, filtered. |
| `openlore stats --json` | Already exists; now includes the new scraper metric rows. |

If D-D5 (the `openlore stats` verb) is deferred and the `scripts/kpi-*.jq`
snippets are the fallback, DELIVER ships:

- `scripts/kpi-scr-1.jq` — post-hoc scrape→sign duration aggregation per `scrape_id`, bucketed by target-kind (per §2.5).
- `scripts/kpi-scr-3.jq` — auditability check (asserts all `scrape.candidate.rendered` have `source_signal_count ≥ 1`; asserts all `scrape.candidates.derived` have `source_signal_coverage == "all"`).
- `scripts/kpi-scr-4.jq` — count of `scrape.refused{reason: NotPublic}` events (sanity that the refuse-private path fires in tests that exercise the `private_refusal` fixture; else the test passes for the wrong reason); plus a check that no event payload contains a token-shaped string.
- `scripts/kpi-scr-5.jq` — edit-rate aggregation over a 30-day window (% of `claim.signed.from_scraper` with `edit_count ≥ 1`).

## 5. Where logs go (UNCHANGED)

Same file: `$XDG_DATA_HOME/openlore/logs/openlore.log`. Same rotation policy.
Same stderr verbose mode. The new event names append to the same JSON Lines
stream. **No new endpoint, no new sink** (the task constraint: reuse the
slice-01/03 tracing pipeline — D-D2).

## 6. Verbosity controls (UNCHANGED)

Foundation §3.5 + slice-03 §6 table carry forward unchanged. The new events emit
at:

- INFO: `scrape.started`, `scrape.harvest.completed`, `scrape.candidates.derived`, `scrape.completed`, `claim.signed.from_scraper`.
- DEBUG: `scrape.candidate.rendered`.
- WARN: `scrape.refused` (a refusal — especially `NotPublic` — is operator-relevant; INFO is too quiet, ERROR is too loud; WARN surfaces in default stderr so the user sees "this target is private; I refused" without it being treated as a bug). Matches the slice-03 `peer.pull.rejected` WARN level and the foundation `port.error` WARN level.

The `GITHUB_TOKEN` value is NEVER included in ANY event field at ANY verbosity
level (WD-54; probe step 5; the contract no-token-leak assertion).

## 7. Telemetry (UNCHANGED policy)

Per ADR-010: telemetry remains opt-in, OFF by default, no endpoint operated.
If/when a future endpoint exists, the slice-02 events eligible for telemetry
rollup are:

- `scrapes_total` (counter only; target string NOT sent).
- `scrape_to_sign_seconds` histogram bucketed by target-kind (no target string).
- `scraper_edit_rate` (the ratio + the denominator count; no per-claim detail).
- `scrape_refusals_total{reason}` (counter only, grouped by reason class; no target string).

EXPLICITLY NEVER sent over telemetry (even when opted in):

- `GITHUB_TOKEN` (the credential — never anywhere, ever).
- `target` strings (a stream of GitHub handles/repos a user scrapes would reveal their evaluation targets — a surveillance-adjacent signal the no-surveillance promise forbids exposing, even in aggregate to an OpenLore endpoint).
- `predicate` text and `fields_edited` field-values from `claim.signed.from_scraper` (free-form user-authored content / the user's editorial choices).
- `scrape.refused.detail` (could contain target identifiers).
- `scrape_id` values (a correlation key that, joined across events, would reveal scrape↔sign behavior detail).

The `[telemetry]` section of `config.toml` (foundation §6.1) does NOT need a new
subsection for slice-02 — the existing on/off semantics suffice. The
target-string non-disclosure rule is the slice-02 analogue of slice-03's
peer_did non-disclosure rule.

## 8. Health checks (extension)

Per §3 above. Startup-gate behavior UNCHANGED — the binary still emits
`health.startup.refused` and exits 2 on probe refusal. The new five-step
`adapter-github` probe widens what `probe_all` covers; the exit-code semantics
carry. `--offline` skips the `adapter-github` probe (a network probe in
disguise); a subsequent `scrape` then refuses with `GithubError::Network`.

## 9. Dashboards (UNCHANGED — none)

Slice-02 ships NO dashboards. Same reasoning as foundation §8 + slice-03 §9:
solo dev, single user per binary, no central aggregation. The KPI-SCR-1 + KPI-SCR-5
"dashboards" called out in `outcome-kpis.md` §3 Handoff item 2 are — for
slice-02 — the `openlore stats --scraper` CLI render against the local log, NOT
a Grafana/Honeycomb instance. The DEVOPS-handoff item is satisfied by the CLI
surface plus the `scripts/kpi-scr-{1,5}.jq` fallback.

When a future telemetry endpoint exists (post-slice-05, the AppView wave's
problem), the same events feed cohort-level dashboards (KPI-SCR-1 percentiles
and KPI-SCR-5 edit-rate across users). The instrumentation contract is
forward-compatible.

## 10. Alerting (UNCHANGED policy; CI-time guardrail alerts)

Slice-02 ships NO continuous alerting (no on-call). However, the CI-time alerts
shipped (per `outcome-kpis.md` Handoff item 3) are:

| Alert | Trigger | Surface |
|---|---|---|
| KPI-SCR-2 != 100% in CI | `at-scraper-never-persists-unsigned` OR `at-candidate-confidence-no-autoinflate` test failure | GitHub Actions check; **release-blocking** via branch protection |
| KPI-SCR-4 != 100% in CI | `at-scraper-only-reads-public-data` OR `contract-pact-github` allowlist-assertion failure | GitHub Actions check; **release-blocking** |
| KPI-SCR-1 P95 > 4 minutes | informational only — escalate to PO; does NOT block release alone (per `outcome-kpis.md` Handoff item 3 + the disprover §3) | NO automated paging (no central metric store); the operator notices on their own `openlore stats --scraper` output, OR Luna (PO) reviews at day-30 |

The two GUARDRAIL alerts (KPI-SCR-2, KPI-SCR-4) are the release-blocking gates;
they map directly to the blocking acceptance/contract jobs in `ci-cd-pipeline.md`
delta §3. KPI-SCR-1's P95-over-4-min alert is informational (the kill-criterion
in the disprovers list is a re-design trigger, not an auto-block).

## 11. KPI-to-instrumentation mapping (cross-link)

See `kpi-instrumentation.md` delta in this dir for the per-KPI-SCR traceability table.

## 12. References

- `platform-design.md` (sibling, this dir)
- `ci-cd-pipeline.md` (sibling, this dir) — new acceptance/contract jobs that consume these events
- `kpi-instrumentation.md` (sibling, this dir)
- `wave-decisions.md` (sibling, this dir) — D-D22..D-D29
- Foundation `observability.md` + slice-03 `observability.md` delta
- `docs/feature/openlore-github-scraper/design/architecture-design.md` §6.3 (the five-step probe)
- `docs/feature/openlore-github-scraper/discuss/outcome-kpis.md` (KPI-SCR-1..5)
- ADR-010 (telemetry-opt-in) — still in force
