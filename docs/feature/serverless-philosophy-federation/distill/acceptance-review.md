# Acceptance Review — serverless-philosophy-federation (DISTILL self-review)

- **Wave**: DISTILL · **Date**: 2026-07-15 · **Reviewer**: Quinn (self-review, nw-ad-critique-dimensions)
- **Subject**: 30 scenarios across 6 `.rs` files + `test-scenarios.md` map + policy rows
- **Note**: The mandatory 4-reviewer parallel gate (Sentinel + PO/SA/PA on the full
  feature-delta) is the orchestrator's to dispatch; this is the designer's Dimension 1-9
  self-check to surface issues before that gate.

```yaml
review_id: "accept_rev_2026-07-15_serverless-philosophy-federation"
reviewer: "acceptance-designer (self-review mode)"

strengths:
  - "KPI-SF-1 (round-trip CID integrity, North Star) is asserted at 5 independent points (WS-1, RT-2, RT-4, PR-4, CI-5) plus the DV-1 CI contract test — defense in depth for the load-bearing guardrail."
  - "The rejected re-encode transport is fenced by an explicit GOLD FIXTURE (RT-4: confidence 0.0/0.5/1.0 each push→pull CID-identical), the exact SPIKE-00 failure the opaque transport sidesteps."
  - "Q-SF-D5 (opaque-instance detection marker) resolved with a testable three-outcome init contract (registered / unreachable / not-an-openlore-instance) bound to PI-1/PI-2/PI-4."
  - "Only the ONE new external boundary is faked (FakeInstance); the real single-canonicalizer core runs in every scenario, so KPI-SF-1 is proven against real CID computation, not a double."

issues_identified:
  happy_path_bias:
    - issue: "Error/edge ratio checked"
      severity: "none"
      recommendation: "14/30 = 46.7% error+edge (>= 40% target). Per-file: init 4/5, push 3/5, pull 2/5, roundtrip 2/4, card 2/6, cross-instance 2/5. PASS."

  gwt_format:
    - issue: "Rust #[test] carries GWT in the doc-comment + todo!() body, not literal Gherkin"
      severity: "none"
      recommendation: "Matches the sibling repo convention (ADR-009 Rust #[test], no pytest-bdd). Each scenario has a single When (one CLI invocation pair) and observable Then. PASS."

  business_language:
    - issue: "Scanned all 30 test names for technical jargon"
      severity: "none"
      recommendation: "Zero HTTP/endpoint/database/schema/JSON/worker in test names. Domain terms only (publish, push, pull, round-trip, claim, CID, instance, card, attribution, reconcile, peer). 'CID' is the codebase's domain term for the content address. PASS (Pillar 1)."

  coverage_gaps:
    - issue: "US-SF-001..006 story-to-scenario mapping"
      severity: "none"
      recommendation: "All six stories have >= 1 bound scenario (test-scenarios.md §5). Every AC traced. PASS (Dimension 4 + 8 Check A)."

  walking_skeleton_centricity:
    - issue: "WS-1 framing"
      severity: "none"
      recommendation: "Title is a user goal ('round-trip one signed claim through my own instance with an identical CID'); Given/When are user actions (deploy+register, push, pull); Then are user observations (1/1 verified, identical CID, local unchanged). Non-technical stakeholder confirmable. PASS (Dimension 5)."

  observable_behavior:
    - issue: "Then steps assert observable outcomes, but CM-E state-delta migration is deferred"
      severity: "low"
      recommendation: "Scenario todo!() bodies specify port-exposed universe entries (instance.records.cids, local.claims.row_count, card.rows[*].author_did — never internal fields). DELIVER migrates load-bearing scenarios to assert_state_delta. Tracked as CM-E deferred (same as sibling slice-01..05). PASS with note."

  traceability_coverage:
    - issue: "Environment-to-scenario mapping (Check B)"
      severity: "low"
      recommendation: "environments.yaml lists clean-instance / wrangler-dev / real-deploy. clean-instance is exercised by PC-2 (empty card) + PR-2 (fresh-machine rebuild) + WS-1 (first push). wrangler-dev + real-deploy are DV-1 CI-contract + dogfood environments, NOT acceptance-layer (the acceptance seam is FakeInstance by design). Noted, not a blocker — the real-transport environments are DEVOPS's per DV-1/DV-2."

  walking_skeleton_boundary:
    - issue: "WS mechanism declared"
      severity: "none"
      recommendation: "Architecture-of-Reference treatment recorded in docs/architecture/atdd-infrastructure-policy.md (publish/instance rows appended this wave): driving CLI = real subprocess; driven-internal claim-domain/duckdb = real; driven-external CLI↔Worker seam = FakeInstance. WS-1 tagged @real-io (real CLI + real core + real DuckDB). Deleting the real claim-domain WOULD fail WS-1 (it recomputes the CID) — so WS-1 proves wiring, not a double. PASS (Dimension 9)."

  priority_validation:
    - issue: "Riskiest-assumption-first"
      severity: "none"
      recommendation: "WS-1 carries KPI-SF-1 (the North-Star boundary risk, SPIKE-00-evidenced). Slice order (round-trip → bulk → reconcile → card → cross-instance) matches DISCUSS Priority Rationale. PASS (Dimension 6)."

approval_status: "conditionally_approved"
```

## Conditions carried to DELIVER

1. **CM-E (state-delta + Universe)** migration of the load-bearing scenarios (WS-1,
   PP-1/PP-4, PR-1/PR-3, CI-1/CI-4) — deferred, universe entries pre-specified in the
   `.rs` `todo!()` bodies (port-exposed only).
2. **Pre-DELIVER fail-for-right-reason gate** — deferred until DELIVER's first step
   scaffolds `FakeInstance` + `PublishPort`/`InstanceReadPort` + the publish verb
   handlers; only then do the tests compile and reach the `todo!()` RED panic.
3. **DV-1 `publish-contract.yml`** (live `wrangler dev` round-trip + 0.0/0.5/1.0 float
   guard) is the DEVOPS real-transport proof; the acceptance suite deliberately does not
   duplicate it (coordination point).

4. **TS Worker supply-chain posture** (PA review): when `atproto/` lands, `publish-contract.yml`
   runs `tsc --strict` + `npm audit --audit-level=high` before `wrangler dev`. Mutation stays
   out of scope (DV-6). Recorded here in place of a separate security doc.
5. **CI write-token fixture** (PA review): `publish-contract.yml` exercises the DV-4 write-auth
   against a throwaway fixture token (see `environments.yaml`); PP-5/PC-6 pin the observable
   contract against `FakeInstance`. Two layers, not a duplicate.

## 4-reviewer gate — 2026-09-25

| Reviewer | Verdict | Notes |
|---|---|---|
| Sentinel (acceptance-designer-reviewer) | conditionally approved | avg 9.1; conditions = #1, #2 above |
| Product owner reviewer | approved | 6/6 stories, all ACs traced, 0 drift, 0 scope creep |
| Solution architect reviewer | approved | 0 contradictions vs ADR-062/ADR-007; driving-port-only confirmed |
| Platform architect reviewer | rejected → **overruled** | see below |

**PA disposition.** Its two "blockers" are DELIVER-phase artifacts, not DISTILL defects:
`publish-contract.yml` (DV-1) drives the `atproto/` Worker, which does not exist until DELIVER,
and a Worker security scan needs Worker code. Both are carried as conditions #3/#4. Its two
documentation gaps were valid and are fixed in `environments.yaml`
(`OPENLORE_PUBLISH_ENDPOINT` seam + CI fixture token → condition #5). Its "suite cannot compile"
claim is wrong: the six `[[test]]` targets are registered in `crates/cli/Cargo.toml` and
`FakeInstance` appears only inside `todo!()` strings.

**Gate result: PASSED (conditionally)** — handoff to DELIVER.
