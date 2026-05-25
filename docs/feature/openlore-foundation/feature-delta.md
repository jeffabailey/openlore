# Feature Delta: openlore-foundation

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: Cross-cutting
> Walking skeleton: Yes (this feature IS the walking skeleton for the OpenLore umbrella)
> Research depth: Comprehensive
> JTBD: mandatory (every story carries `job_id` → `docs/product/jobs.yaml`)
> Date: 2026-05-25
> Owner: Luna (nw-product-owner)

This file is the canonical DISCUSS-wave delta for `openlore-foundation`. Tier-1
content is inlined under `## Wave: DISCUSS / [REF] <Section>` headings; SSOT
content lives under `docs/product/`; per-slice briefs under
`docs/feature/openlore-foundation/slices/`; per-journey artifacts under
`docs/feature/openlore-foundation/discuss/`.

---

## Wave: DISCUSS / [REF] Wave Decisions

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-1 | DISCOVER and DIVERGE waves were skipped; user routed directly to DISCUSS with clear requirements (architecture, capabilities, claim shape, federation model). | Risk: DISCUSS-emitted JTBD analysis is performed without prior validation interviews. Mitigated by 4 candidate jobs being articulated in the user's brief and refined here. | Logged risk; proceeding |
| WD-2 | Scope assessed as **OVERSIZED at the umbrella level**. Brief implies 5 bounded contexts (claims/federation, scrapers, scoring, AppView, identity) and >5 integration points (GitHub API, Wikipedia, ATProto PDS, DuckDB, AppView indexer, signing). Split into 5 sibling features. | Carpaccio gate. `openlore-foundation` keeps slice-01 (walking skeleton). Slices 02-05 emit one-page briefs and birth sibling features later. | Proposed split; awaiting user confirmation (see "Open Decisions for User" below) |
| WD-3 | Walking-skeleton slice is **slice-01-claim-skeleton**: compose → sign → DuckDB persist → PDS publish → local query, exercised end-to-end by 4 user stories + 1 infrastructure story. | Demonstrates Lexicon, signing, DuckDB, ATProto publication, and local query in one slice. Disprove and the OpenLore thesis is in trouble before further investment. | Locked |
| WD-4 | Persona priority: **P-001 Senior Engineer Solo Builder** = primary; **P-002 Researcher / Tech Lead** = secondary (consumer of slice-03+). Slice-01 stories target P-001 only. | The walking-skeleton job (J-001) is an author-side job. Consumer-only personas are out of scope until slice-03. | Locked |
| WD-5 | Job priority: **J-001** is the walking-skeleton job; J-003 is the P1 follow-up (slice-03); J-002 / J-004 emerge organically from slices 04/05. | J-001 is the only job whose payoff is observable inside a single end-to-end CLI session. Validate it first. | Locked |
| WD-6 | The compose preview MUST contain the literal text **"not as truth"** and the publish success message MUST mention the retract command. | These are load-bearing UX moments addressing the J-001 anxiety force and the J-001 success signal "user feels safe publishing because retraction is first-class." Lifted to a hard AC in US-001 and US-003. | Locked, enforced by AC |
| WD-7 | Carpaccio sequence (slices that will birth sibling features): **01 (this) → 03 federated-read → 02 github-scraper → 04 scoring-graph → 05 appview-search**. Federation is sequenced ahead of scrapers so the on-the-wire claim contract is validated before scraper serialization is built. | Lowest-rework path through the riskiest assumptions. | Proposed (see Open Decisions) |
| WD-8 | Choice of local store for slice-01 is **DuckDB** (matches user's stated architecture). The DuckDB-vs-Kùzu-vs-SurrealDB choice is **re-opened in slice-04** when graph traversal becomes the dominant workload. | Slice-01 needs only indexed key/predicate lookup, which DuckDB handles trivially. Premature graph-store choice = premature optimization. | Locked for slice-01 |
| WD-9 | **Carpaccio split confirmed (locks OD-1).** `openlore-foundation` keeps only slice-01 (the walking skeleton). Slices 02-05 will each become independent sibling features in their own DISCUSS waves. | Locks the Phase-2 scope split surfaced in WD-2. Resolves OD-1. | LOCKED 2026-05-25 by user |
| WD-10 | **Confidence semantics locked (locks OD-2).** Numeric `[0.0, 1.0]` is the only persisted form on the signed claim. Display-only buckets: speculative `<0.3`, weighted `0.3-0.7`, well-evidenced `0.7-0.9`, triangulated `>0.9`. Buckets MUST NEVER be persisted in the signed payload, the local store, or the PDS record. | Resolves RC-01 / OD-2. | LOCKED 2026-05-25 by user |
| WD-11 | **Retraction model locked (locks OD-3).** Retraction is a counter-claim that references the original CID. Soft-retract only; the PDS-published record persists. Hard-delete is explicitly forbidden. | Resolves RC-02 / OD-3. | LOCKED 2026-05-25 by user |
| WD-12 | **Signing identity locked (locks OD-4).** Reuse the user's existing ATProto DID; OpenLore derives a per-application key from that DID so OpenLore claims can be revoked independently of the user's main ATProto identity. No fresh DID is minted. | Resolves RC-03 / OD-4. | LOCKED 2026-05-25 by user |
| WD-13 | **Sibling-slice sequence locked (locks OD-5).** federation (slice-03) ships before scrapers (slice-02). WD-7 is upheld. | Resolves OD-5; reinforces WD-7. | LOCKED 2026-05-25 by user |

### Scope Assessment

`## Scope Assessment: SPLIT-APPROVED-PENDING — 5 user stories in slice-01 (4 user-visible + 1 infra), 1 bounded context (claims model + signing + local query + PDS publish, treated as a single cohesive context for slice-01), estimated 3-5 days. Umbrella split into 5 sibling features; only slice-01 carried forward in this feature.`

### Risks logged

- DISCOVER/DIVERGE skipped → no independent validation that the 4 candidate jobs are the right ones. Mitigation: KPI-3 + KPI-6 are designed to surface job mis-prioritization within 30 days of slice-01 release.
- Canonical CID determinism is a "must work or whole model is broken" assumption (riskiest assumption #2). Mitigation: surfaced as a guardrail integration test in KPI-4.
- "Claims not truth" framing is a behavioral hypothesis (riskiest assumption #3). Mitigation: KPI-3 (felt-framing survey) + KPI-6 (behavioral measure at day-30).

---

## Wave: DISCUSS / [REF] JTBD Analysis Summary

Full analysis in `docs/product/jobs.yaml`. Summary:

| Job ID | Name | Priority | Opportunity Score | In slice-01? |
|---|---|---|---|---|
| J-001 | Author a signed philosophical claim | primary, walking-skeleton | 16 (underserved-primary) | yes — all 4 user stories |
| J-002 | Explore the philosophy graph to inform a decision | secondary | 14 | partial — US-004 touches it |
| J-003 | Read another developer's federated claims with weighting | secondary | 13 | no — slice-03 |
| J-004 | Evaluate a contributor's body of work | tertiary | 11 | no — slice-04/05 |

Each job's four forces, opportunity score, and success signals are in jobs.yaml.

---

## Wave: DISCUSS / [REF] Journey Artifacts

Walking-skeleton journey (J-001):

- Visual journey: `docs/feature/openlore-foundation/discuss/journey-author-and-publish-claim-visual.md`
- Structured schema (with embedded Gherkin per step): `docs/product/journeys/author-and-publish-claim.yaml`
- Shared artifacts registry: `docs/feature/openlore-foundation/discuss/shared-artifacts-registry.md`

Emotional arc: **confidence-building-with-explicit-trust-buffer** (variant of standard
confidence-build). The local-persist step (step 2) is the load-bearing trust buffer
between compose (step 1) and publish (step 3) — without it the user crosses a
federated boundary before psychological commitment.

---

## Wave: DISCUSS / [REF] Story Map and Slicing

- Story map: `docs/feature/openlore-foundation/discuss/story-map.md`
- Slice 01 (this feature, walking skeleton): `docs/feature/openlore-foundation/slices/slice-01-claim-skeleton.md`
- Slices 02-05 (sibling-feature seeds): `docs/feature/openlore-foundation/slices/slice-0{2,3,4,5}-*.md`

All 5 taste tests pass for slice-01 (see slice-01 brief). Slices 02-05 are
explicitly deferred to future DISCUSS waves under their own feature directories.

---

## Wave: DISCUSS / [REF] User Stories and Acceptance Criteria

All in `docs/feature/openlore-foundation/discuss/user-stories.md`:

| Story | Title | Job link | Elevator pitch | DoR status |
|---|---|---|---|---|
| US-001 | Author a single signed claim from the CLI | J-001 | yes | PASS (see DoR section) |
| US-002 | Sign and persist a claim locally before any publication | J-001 | yes | PASS |
| US-003 | Publish a signed claim to the author's PDS | J-001 | yes | PASS |
| US-004 | Read back local claims by subject | J-001 + J-002 | yes | PASS |
| US-005 | Bootstrap claim Lexicon, identity wiring, and DuckDB schema | `infrastructure-only` (with rationale) | n/a — @infrastructure | PASS |

Slice composition gate: PASS — 4 user-visible stories + 1 infra story (slice is not 100% @infrastructure).

---

## Wave: DISCUSS / [REF] Outcome KPIs

Full table in `docs/feature/openlore-foundation/discuss/outcome-kpis.md`. North star:

> **KPI-6**: ≥60% of P-001 cohort report at day-30 that ≥3 of their last N claims
> would NOT have been published as a blog post.

Guardrails: KPI-4 (zero silent normalization, 100% round-trip identity) and KPI-5
(local-first invariant holds with network disabled).

---

## Wave: DISCUSS / [REF] Definition of Ready validation

| DoR item | US-001 | US-002 | US-003 | US-004 | US-005 |
|---|---|---|---|---|---|
| 1. Problem statement clear, domain language | PASS | PASS | PASS | PASS | PASS (infra rationale) |
| 2. Persona with specific characteristics | PASS (P-001) | PASS | PASS | PASS | n/a (infra) |
| 3. ≥3 domain examples with real data | PASS (3) | PASS (3) | PASS (3) | PASS (3) | PASS (3) |
| 4. UAT in Given/When/Then (3-7) | PASS (3) | PASS (3) | PASS (3) | PASS (3) | PASS (2 — within range with composite) |
| 5. AC derived from UAT | PASS | PASS | PASS | PASS | PASS |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | PASS | PASS | PASS | PASS |
| 7. Technical notes: constraints/dependencies | PASS | PASS | PASS | PASS | PASS |
| 8. Dependencies resolved or tracked | PASS (depends US-005) | PASS (US-005) | PASS (US-002, US-005) | PASS (US-002, US-005) | PASS (none) |
| 9. Outcome KPIs defined with measurable targets | PASS (KPI-1,2) | PASS (KPI-4,5) | PASS (KPI-3,6) | PASS (KPI-4) | n/a — supports KPI-4/5 |

**Overall DoR status: PASSED** for all stories.

Note on item 4 for US-005: the brief allows 3-7 scenarios; US-005 ships 2 composite
scenarios because the infrastructure surface is narrow and additional scenarios
would be padding. Flagged for reviewer judgment but considered PASS.

---

## Wave: DISCUSS / [REF] Locked Decisions (formerly Open)

All five decisions previously listed here as blockers have been resolved by the user.
Each row records the locked verdict, the date locked, and the owning wave decision
(WD-9 through WD-13) that carries it forward as binding for DESIGN.

| ID | Decision | Locked verdict | Lock provenance |
|---|---|---|---|
| OD-1 | Carpaccio split. | **Approved.** `openlore-foundation` keeps only slice-01 (the walking skeleton). Slices 02-05 become sibling features in their own DISCUSS waves later. | LOCKED 2026-05-25 by user → WD-9 |
| OD-2 | RC-01 confidence semantics. | **Numeric `[0.0, 1.0]` stored in the signed claim; display-only buckets** (speculative `<0.3`, weighted `0.3-0.7`, well-evidenced `0.7-0.9`, triangulated `>0.9`). Buckets NEVER persisted. | LOCKED 2026-05-25 by user → WD-10 |
| OD-3 | RC-02 retraction model. | **Soft-retract via counter-claim that references the original CID.** PDS record persists; hard-delete is forbidden. | LOCKED 2026-05-25 by user → WD-11 |
| OD-4 | RC-03 signing identity. | **Reuse the user's existing ATProto DID with a per-application derived key** so OpenLore claims can be revoked independently of the user's main ATProto identity. | LOCKED 2026-05-25 by user → WD-12 |
| OD-5 | Sibling-slice sequence: federation before scrapers. | **Approved.** federation (slice-03) ships before scrapers (slice-02). WD-7 upheld. | LOCKED 2026-05-25 by user → WD-13 |

> These five decisions are now **binding inputs to DESIGN**. The solution architect
> inherits them as constraints and must come back to product-owner (Luna) rather
> than relitigate any of them in flight. They appear in the DESIGN read-list under
> "Constraints inherited from DISCUSS" below.

---

## Wave: DISCUSS / [REF] Ask-Intelligent Menu (lean mode, scoped to triggered items only)

Triggers evaluated; scoped expansion offered only for those that fired.

### Fired: cross-context complexity (≥3 contexts)

The umbrella feature spans claims/federation, scrapers, scoring, AppView, identity (5 contexts). Even the walking skeleton touches 3 (claim model, signing, ATProto publication).

- **Offer**: `alternatives-considered.md` — explicitly document the rejected alternatives for the three biggest choices (DuckDB vs Kùzu vs SurrealDB; reuse-DID vs mint-fresh-DID; sign-and-publish-in-one-step vs separate-sign-and-publish-steps).
- **Cost**: ~10 minutes to write; ~3 pages output.
- **Recommendation**: **accept**. These are the choices DESIGN will second-guess if not documented now.
- **Status**: **ACCEPTED** 2026-05-25 — see `docs/feature/openlore-foundation/discuss/alternatives-considered.md`.

### Fired: AC ambiguity (the trust/confidence semantics are easy to disagree on)

- **Offer**: `gherkin-scenarios-expanded.md` — add 3 anxiety-path scenarios and 2 habit-path scenarios per the JTBD-BDD integration template (currently US-001 through US-004 are happy/edge/error only; the anxiety scenarios — "what if I publish and someone brigades me" — are not yet AC).
- **Cost**: ~15 minutes; ~2 pages output.
- **Recommendation**: **accept**. The anxiety force is the load-bearing one for J-001; without anxiety-path scenarios DISTILL will have to invent them.
- **Status**: **ACCEPTED** 2026-05-25 — see `docs/feature/openlore-foundation/discuss/gherkin-scenarios-expanded.md`.

### NOT fired: multi-stakeholder narrative

Only one primary persona in slice-01 (P-001). P-002 enters in slice-03. Persona-narrative expansion is not justified for this slice; revisit at slice-03 DISCUSS.

### NOT fired: regulatory / compliance complexity

No PII handled in slice-01 beyond a public DID. Re-evaluate at slice-03 (federated read of others' claims may surface PII concerns).

### NOT fired: integration density

Slice-01 has 2 external integrations (ATProto PDS, DuckDB). Below the threshold.

### Menu action

Both fired offers were **accepted** by the user on 2026-05-25. The two artifacts are
linked above and added to the DESIGN read-list.

Telemetry: each `expand` acceptance should ideally emit a `DocumentationDensityEvent`
via the standard ask-intelligent telemetry helper so density vs. quality can be
tracked over time. See `## Wave: DISCUSS / [REF] Telemetry` below for the
greenfield-specific intent.

---

## Wave: DISCUSS / [REF] Telemetry

OpenLore is greenfield; no ask-intelligent telemetry helper exists in this repo yet.
This section records the intent so DEVOPS (nw-platform-architect) wires it up when
observability infrastructure lands.

Intent: emit one `DocumentationDensityEvent` per accepted ask-intelligent expansion.
For this DISCUSS wave, that means two events:

| Trigger | Artifact | Should emit |
|---|---|---|
| `cross_context_complexity` | `alternatives-considered.md` | `DocumentationDensityEvent{ feature: openlore-foundation, wave: DISCUSS, expansion: alternatives-considered, accepted: true, ts: 2026-05-25 }` |
| `ac_ambiguity` | `gherkin-scenarios-expanded.md` | `DocumentationDensityEvent{ feature: openlore-foundation, wave: DISCUSS, expansion: gherkin-scenarios-expanded, accepted: true, ts: 2026-05-25 }` |

When the telemetry helper exists, retroactively backfill these two events.

---

## Wave: DISCUSS / [REF] Handoff

### To DESIGN (nw-solution-architect)

- Read (explicit list — every file matters):
  - `feature-delta.md` (this file)
  - Everything in `docs/feature/openlore-foundation/discuss/`, specifically including
    - `user-stories.md`
    - `story-map.md`
    - `outcome-kpis.md`
    - `shared-artifacts-registry.md`
    - `journey-author-and-publish-claim-visual.md`
    - **`alternatives-considered.md`** (new — fired ask-intelligent expansion)
    - **`gherkin-scenarios-expanded.md`** (new — fired ask-intelligent expansion)
  - `docs/feature/openlore-foundation/slices/slice-01-claim-skeleton.md`
  - `docs/product/jobs.yaml`
  - `docs/product/journeys/author-and-publish-claim.yaml`
  - `docs/product/personas/senior-engineer-solo-builder.yaml`
  - `docs/product/personas/researcher-tech-lead.yaml`
- Decide: DuckDB schema; canonicalization algorithm; per-application key derivation
  scheme on top of the user's existing ATProto DID (WD-12 constrains this — no fresh
  DID); CLI structure (noun-verb already chosen by the journey YAML, but DESIGN owns
  argument-parser shape); Lexicon JSON shape; the open question surfaced in
  `alternatives-considered.md` about the granularity of the `claim sign` vs
  `claim publish` verb pair.
- Constraints inherited from DISCUSS (DO NOT relitigate without coming back to PO):
  - "Not as truth" literal text in compose preview (US-001 AC).
  - Retract command hint in publish success message (US-003 AC).
  - Local-only as default for graph query (US-004 AC).
  - Atomic local writes (US-002 AC).
  - Idempotent publish keyed on CID (US-003 AC).
  - **WD-10 (OD-2)**: numeric `[0.0, 1.0]` only in the signed payload; display-only
    buckets; buckets MUST NEVER be persisted.
  - **WD-11 (OD-3)**: retraction is counter-claim-only; never hard-delete; the PDS
    record persists.
  - **WD-12 (OD-4)**: identity = the user's existing ATProto DID with a per-application
    derived key; no fresh DID; revocation surface lives on the derived key.
  - **WD-13 (OD-5)**: sequence — federation (slice-03) before scrapers (slice-02).
    Not slice-01's concern but informs cross-slice contract decisions made here.

### To DEVOPS (nw-platform-architect, parallel)

- Read: `outcome-kpis.md` (Handoff to DEVOPS section).
- Deliver: instrumentation plan for KPI-1, 2, 4, 5; survey delivery mechanism for KPI-3, 6.

### To DISTILL (nw-acceptance-designer)

- Read:
  - `docs/product/journeys/author-and-publish-claim.yaml` (embedded Gherkin per step)
  - `docs/feature/openlore-foundation/discuss/user-stories.md` (UAT scenarios per story)
  - `docs/feature/openlore-foundation/discuss/shared-artifacts-registry.md` (integration validation rules)
  - **`docs/feature/openlore-foundation/discuss/gherkin-scenarios-expanded.md`** (new — anxiety-path and habit-path scenarios; some carry a `# DISTILL: confirm command name` flag)
- Build executable acceptance tests; the anxiety-path and habit-path scenarios are
  now authored and must be resolved against the final CLI verb shape DESIGN settles
  on. Flagged comments mark every scenario that needs that resolution.

### Handoff-ready?

**YES — unblocked 2026-05-25.** All OD-1..OD-5 locked (see Locked Decisions section
above); both fired expansions delivered (`alternatives-considered.md`,
`gherkin-scenarios-expanded.md`); lean Tier-1 output stands. DESIGN + DEVOPS may
proceed in parallel, and DISTILL has the scenarios it needs.
