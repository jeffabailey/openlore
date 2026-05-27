# Feature Delta: openlore-federated-read

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: Cross-cutting (CLI + ports + DuckDB + Lexicon + ATProto adapter)
> Walking skeleton: Yes (this sibling IS the walking skeleton for the federation slice)
> Research depth: Comprehensive (trust model around peer attribution is load-bearing)
> JTBD: mandatory (every story carries `job_id` -> `docs/product/jobs.yaml`)
> Inherits from: `docs/feature/openlore-foundation/feature-delta.md` (WD-9..WD-13, ADR-001..012)
> Date: 2026-05-27
> Owner: Luna (nw-product-owner)

This file is the canonical DISCUSS-wave delta for `openlore-federated-read`,
the second sibling feature in the OpenLore umbrella (per WD-9). Tier-1
content is inlined under `## Wave: DISCUSS / [REF] <Section>` headings;
SSOT content lives under `docs/product/`; per-slice briefs under
`docs/feature/openlore-federated-read/slices/`; per-journey artifacts under
`docs/feature/openlore-federated-read/discuss/`.

---

## Wave: DISCUSS / [REF] Wave Decisions

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-14 | Slice-03 ships in a SIBLING feature `openlore-federated-read` (this feature) per the carpaccio split locked by WD-9. Slice-03 IS the walking skeleton for this feature (one slice = one feature). | Inherits WD-9. Sibling-feature pattern keeps each slice independently shippable. | LOCKED |
| WD-15 | Persona priority for slice-03: **P-002 Researcher / Tech Lead (federation-reader hat) = primary**; **P-001 Senior Engineer Solo Builder = secondary** (continues to author, now also reads peer claims). | Slice-03's load-bearing job (J-003) is a read-side job; P-002 was deferred to slice-03 at slice-01 time (WD-4) and is now activated. P-001 wears the same federation-reader hat when reading peer claims. | LOCKED |
| WD-16 | Job priority for slice-03: **J-003 is the walking-skeleton job for this feature** (opportunity score raised 13->15, promoted to underserved-primary-for-slice in jobs.yaml). Three sub-jobs are addressed: J-003a (anti-merging — load-bearing), J-003b (counter-claim as first-class disagreement), J-003c (revocability without residue). | J-003 was the slice-01-time secondary; it is the right walking-skeleton job for this sibling per the brief. Sub-jobs surfaced during this DISCUSS as distinct aspects worth their own Elevator Pitches. | LOCKED |
| WD-17 | Counter-claim verb shape is the sugar verb **`openlore claim counter <target_cid> --reason "..." [other claim flags]`**, NOT a flag on `claim add`. | Symmetric with `claim retract <cid>` (slice-01 DD-9); more discoverable in `graph query --federated` output tip lines; aligns with verb-noun-noun grammar; ADR-013 amendment is needed but small. Full rejected-alternatives in `discuss/alternatives-considered.md`. | LOCKED |
| WD-18 | Pull mechanism is **pull-on-demand** via `openlore peer pull`. Auto-pull-on-subscribe and push subscriptions are both out of slice-03. | Matches the brief; conflating subscribe with pull would widen subscribe's failure surface; push requires a daemon (violates CLI-first). | LOCKED |
| WD-19 | Peer storage layout is **single DuckDB file with two new tables** (`peer_subscriptions` + `peer_claims`) alongside the existing `author_claims`, enforced by an `xtask check-arch` rule that no query may JOIN author_claims and peer_claims in a way that elides the author_did column. | Single-file simplicity is the right tradeoff for slice-03; the anti-merging invariant is enforceable at the architecture-rule layer. Revisit if `peer_claims` grows beyond ~100k rows. | LOCKED |
| WD-20 | `--reason` is **REQUIRED** on `openlore claim counter`; length 1..=1000 chars. Silent counter-claims are forbidden. | Reason is the disagreement artifact; without it the system cannot distinguish a structured disagreement from a duplicate claim. Forces the user to articulate the disagreement before signing. | LOCKED, enforced by US-FED-004 AC |
| WD-21 | `openlore peer remove --purge` REQUIRES interactive confirmation. No `--yes` flag in slice-03 (defer to slice-04 if scripting need justifies). | --purge is the only destructive operation in the slice; confirmation is the safety valve. Deferring `--yes` keeps the J-003c trust promise from being undermined by accidental scripting. | LOCKED |
| WD-22 | Counter-claim authoring REUSES the slice-01 compose-sign-publish pipeline via the same `VerbClaimPublish` internals. No parallel publish code path. | Preserves ADR-003 single-publish-path invariant. Counter-claim is just a claim with `references[].type == Counters`; the publish path does not care which type. | LOCKED |
| WD-23 | Counter-claim Lexicon: introduces an optional `reason` field on `org.openlore.claim` (FORWARD-COMPATIBLE; slice-01 readers ignore unknown optional fields per ADR-005). NO new ReferenceType variant — uses the existing `Counters` variant from ADR-008. | Maintains wire stability with slice-01; no breaking change. The `reason` field semantically applies only to claims with `references[].type == Counters` but is permitted on any claim (slice-03 enforces in `claim counter` verb, not in Lexicon schema). | LOCKED |
| WD-24 | Per-claim signature verification at pull time is REQUIRED. Per-claim CID recomputation at pull time is REQUIRED. Either failure rejects that claim only; other claims in the same pull proceed. | This is the only mechanism that protects against adversarial peers publishing tampered records or records claiming false authorship. Two independent checks because they catch different attack surfaces (signature catches tampering after publication; CID catches canonicalization disagreement). | LOCKED, enforced by US-FED-002 AC + KPI-FED-6 |
| WD-25 | Soft-remove (default) RETAINS cached peer_claims; hard-purge (`--purge`) DELETES them. Counter-claims authored by the current user against the removed peer are NEVER deleted (they are the user's own published artifacts). | Two-level revocation: cheap soft-remove for "I'm done following but might come back" and expensive hard-purge for "leave no trace." The user's own counter-claims are public published artifacts the user owns; they survive peer removal. | LOCKED, enforced by US-FED-005 AC |

### Scope Assessment

`## Scope Assessment: PASS — 6 user stories (5 user-visible + 1 infra), 1 cohesive bounded context (federated read of peer claims + counter-claim authoring as a single coherent surface), estimated ~10 days. Single slice = single feature; no further sub-slicing recommended.`

Carpaccio gate evaluation:
- Stories: 6 (within ≤10 threshold)
- Bounded contexts: 1 (peer-claim federation; lives within the larger claim/federation context inherited from slice-01)
- Walking skeleton integration points: 3 new (peer DID resolution, peer PDS records read, peer_claims storage) — all extensions of slice-01 ports
- Estimated effort: ~10 days (within ≤2 weeks threshold)
- Multiple independent outcomes: NO — all 6 stories serve J-003 and its sub-jobs; counter-claim authoring (US-FED-004) is a tightly-coupled aspect of federated read, not an independent outcome
- Verdict: RIGHT-SIZED. Single slice = single sibling feature.

### Risks logged

- KPI-FED-3 behavioral validation (counter-claim publication rate ≥30% in 30 days) is the slice's load-bearing behavioral hypothesis. Mitigation: instrumentation via `claim.counter.published` tracing event (handed off to DEVOPS).
- Adversarial-peer fixture for KPI-FED-6 (tampered-signature rejection) requires a CI test PDS that publishes deliberately bad records. Mitigation: handed off to DISTILL + DEVOPS as a CI infrastructure deliverable.
- The `--federated` query layer's counter-claim annotation (`countered-by ...`) requires a join across `peer_claims` and `author_claims`. DESIGN's chosen query shape MUST preserve the per-row attribution invariant. Mitigation: integration test `federation_attribution_preserved` is mandatory.
- DISCOVER + DIVERGE skipped (same as slice-01). The four-forces analysis for J-003 was performed in this DISCUSS without prior validation interviews. Mitigation: KPI-FED-3 + day-30 survey will surface mis-prioritization within 30 days of release.

---

## Wave: DISCUSS / [REF] JTBD Analysis Summary

Full analysis in `docs/product/jobs.yaml`. Summary for slice-03:

| Job | Name | Priority for slice-03 | Opportunity Score | In slice-03? |
|---|---|---|---|---|
| J-003 | Read another developer's federated claims with weighting | primary (walking-skeleton for this feature) | 15 (raised from 13; underserved-primary-for-slice) | yes — all 6 stories |
| J-003a (sub-job of J-003) | Attribute every peer claim without merging | LOAD-BEARING | n/a (sub-job) | yes — US-FED-002, US-FED-003 |
| J-003b (sub-job of J-003) | Counter-claim authoring as first-class disagreement | high | n/a (sub-job) | yes — US-FED-004 |
| J-003c (sub-job of J-003) | Subscription is revocable without residue | medium | n/a (sub-job) | yes — US-FED-005 |
| J-001 | Author a signed philosophical claim | inherited | 16 | partial — US-FED-004 reuses the compose-sign-publish pipeline |
| J-002 | Explore the philosophy graph to inform a decision | inherited | 14 | partial — US-FED-003 extends graph query |

Each job's four forces, opportunity score, and success signals are in
jobs.yaml. J-003 was extended during this DISCUSS with three distinct
anxieties (bad-actor absorption, subscription regret, brigade reprisal)
and three sub-jobs.

---

## Wave: DISCUSS / [REF] Journey Artifacts

Two journeys to map (subscribe-and-read and counter-claim):

- Visual journey #1 (subscribe and pull peer claims): `docs/feature/openlore-federated-read/discuss/journey-subscribe-and-read-federated-visual.md`
- Visual journey #2 (counter-claim authoring): `docs/feature/openlore-federated-read/discuss/journey-author-counter-claim-visual.md`
- Structured schema #1 (with embedded Gherkin per step): `docs/product/journeys/subscribe-and-read-federated.yaml`
- Structured schema #2 (with embedded Gherkin per step): `docs/product/journeys/author-counter-claim.yaml`
- Shared artifacts registry: `docs/feature/openlore-federated-read/discuss/shared-artifacts-registry.md`

Emotional arcs:

- Subscribe-and-read journey: **discovery-with-explicit-sovereignty-buffer** — entry Curious-but-cautious through Trust-building (the peer-pull verification moment is load-bearing) to Sovereign-confident at federated query. Optional revocation step ends Reversed-cleanly.
- Counter-claim journey: **irritation-to-considered-public-stake** — entry Irritated through Targeted (CID identified) and Considered (forced to articulate `--reason`) to Publicly-staked at publish, Validated at observation.

The anti-merging guarantee (J-003a) is a CROSS-JOURNEY invariant elevated to
its own section in the subscribe-and-read visual journey. It is enforced at
ingest (separate stores), at query (group by author), at display (per-row
attribution), and at test time (`federation_attribution_preserved`
acceptance test).

---

## Wave: DISCUSS / [REF] Story Map and Slicing

- Story map: `docs/feature/openlore-federated-read/discuss/story-map.md`
- Slice 03 brief (this feature's only slice): `docs/feature/openlore-federated-read/slices/slice-03-federated-read.md`

Slicing summary:

- **Release 1 (walking skeleton)**: US-FED-001 + US-FED-002 + US-FED-003 + US-FED-006. Validates the federation contract end-to-end.
- **Release 2 (counter-claim authoring)**: US-FED-004. Validates J-003b behavior change.
- **Release 3 (subscription revocability)**: US-FED-005. Validates J-003c trust promise.

Priority order is set by outcome impact and risk-of-failure consequence
(Release 1 fails = federation thesis dead; Release 3 fails = survivable
UX defect). Rationale in story-map.md `## Priority Rationale` section.

All 5 carpaccio taste tests evaluated for this slice (in slice-03 brief):
right-sized in stories, contexts, integration points, effort, and outcome
coherence. Verdict: SINGLE SLICE = SINGLE FEATURE; no further sub-slicing.

---

## Wave: DISCUSS / [REF] User Stories and Acceptance Criteria

All in `docs/feature/openlore-federated-read/discuss/user-stories.md`:

| Story | Title | Job link | Elevator Pitch | DoR status |
|---|---|---|---|---|
| US-FED-001 | Subscribe to a peer's claim stream | J-003 | yes | PASS (see DoR section) |
| US-FED-002 | Pull peer claims with sig + CID verification | J-003 | yes | PASS |
| US-FED-003 | Read federated graph with per-author attribution | J-003 | yes | PASS |
| US-FED-004 | Author and publish a counter-claim | J-003 + J-001 | yes | PASS |
| US-FED-005 | Remove a peer subscription with optional purge | J-003 | yes | PASS |
| US-FED-006 | Bootstrap peer storage + PeerPort (`@infrastructure`) | `infrastructure-only` | n/a — @infrastructure | PASS |

Slice composition gate: PASS — 5 user-visible stories + 1 infrastructure
story; slice is NOT 100% `@infrastructure` (per `nw-po-review-dimensions`
Dimension 0 §5).

---

## Wave: DISCUSS / [REF] Outcome KPIs

Full table in `docs/feature/openlore-federated-read/discuss/outcome-kpis.md`.
North star:

> **KPI-FED-3**: ≥30% of dogfood cohort (federation-reader hat) publishes
> ≥1 counter-claim within 30 days of slice-03 release, AND describes the
> experience as "as light as posting a comment, but more structured."

Guardrails: KPI-FED-2 (zero merged rows), KPI-FED-4 (zero purge residue),
KPI-FED-6 (zero invalid signatures stored). Any guardrail failure is
unshippable.

---

## Wave: DISCUSS / [REF] Definition of Ready validation

| DoR item | US-FED-001 | US-FED-002 | US-FED-003 | US-FED-004 | US-FED-005 | US-FED-006 |
|---|---|---|---|---|---|---|
| 1. Problem statement clear, domain language | PASS | PASS | PASS | PASS | PASS | PASS (infra rationale) |
| 2. Persona with specific characteristics | PASS (P-002) | PASS (P-002) | PASS (P-002) | PASS (P-002+P-001) | PASS (P-002+P-001) | n/a (infra) |
| 3. ≥3 domain examples with real data | PASS (4) | PASS (3) | PASS (4) | PASS (4) | PASS (5) | PASS (2 — within range for narrow infra surface) |
| 4. UAT in Given/When/Then (3-7) | PASS (4) | PASS (4) | PASS (4) | PASS (5) | PASS (5) | PASS (2 — within range for narrow infra surface) |
| 5. AC derived from UAT | PASS | PASS | PASS | PASS | PASS | PASS |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS (1d, 4) | PASS (2d, 4) | PASS (2d, 4) | PASS (2d, 5) | PASS (1.5d, 5) | PASS (1.5d, 2) |
| 7. Technical notes: constraints/dependencies | PASS | PASS | PASS | PASS | PASS | PASS |
| 8. Dependencies resolved or tracked | PASS (depends US-FED-006) | PASS (US-FED-001, US-FED-006) | PASS (US-FED-002) | PASS (US-FED-002, US-FED-003) | PASS (US-FED-001, US-FED-002) | PASS (slice-01 schema) |
| 9. Outcome KPIs defined with measurable targets | PASS (KPI-FED-1, 5) | PASS (KPI-FED-1, 2, 6) | PASS (KPI-FED-1, 2) | PASS (KPI-FED-3) | PASS (KPI-FED-4) | n/a — supports KPI-FED-1, 2, 4, 6 |

**Overall DoR status: PASSED** for all stories.

Notes:
- Item 4 (US-FED-006): the spec allows 3-7 scenarios; US-FED-006 ships 2 composite scenarios because the infrastructure surface is narrow and additional scenarios would be padding. Same pattern as US-005 in slice-01. Flagged for reviewer judgment but considered PASS.
- Item 2 (US-FED-006): infrastructure-only stories do not require a persona; rationale field present per Decision 1.

### Elevator Pitch verification (BLOCKING per Dimension 0)

Per `nw-po-review-dimensions` Dimension 0 (checked first, BLOCKING):

| Story | Section present? | Real entry point? | Concrete output? | Job connection? | Verdict |
|---|---|---|---|---|---|
| US-FED-001 | YES (Before/After/Decision enabled) | YES (`openlore peer add did:plc:rachel-test`) | YES (specific stdout: "Resolving DID ... ok / Adding peer to subscription list ... ok") | YES (commit to following without polluting own claims) | PASS |
| US-FED-002 | YES | YES (`openlore peer pull`) | YES (specific stdout: pull summary with verified/rejected counts) | YES (ingest with cryptographic confidence) | PASS |
| US-FED-003 | YES | YES (`openlore graph query --subject ... --federated`) | YES (specific stdout: grouped-by-author with attribution and no-merge footer) | YES (synthesize defensible view from multiple developers) | PASS |
| US-FED-004 | YES | YES (`openlore claim counter <cid> --reason "..." ...`) | YES (specific stdout: compose preview with counter target + framing + signing/publishing output) | YES (publicly stake structured disagreement) | PASS |
| US-FED-005 | YES | YES (`openlore peer remove <did> [--purge]`) | YES (specific stdout: confirmation prompt + purge summary) | YES (subscribe freely knowing you can leave cleanly) | PASS |
| US-FED-006 | n/a (@infrastructure with rationale) | n/a | n/a | n/a (`infrastructure-only` per Decision 1) | PASS via rationale |

Slice-level Elevator Pitch check (Dimension 0 §5): the slice has 5
user-visible stories + 1 infrastructure story. Slice is NOT 100%
`@infrastructure`. PASS.

---

## Wave: DISCUSS / [REF] Locks inherited from openlore-foundation

These are binding inputs to this feature's DESIGN wave. They are NOT
relitigated here; any change requires returning to slice-01 product-owner
review first.

| ID | Inherited from | Carries into slice-03 as |
|---|---|---|
| WD-10 | openlore-foundation | Numeric `[0.0, 1.0]` is the only persisted form on the signed claim; display-only buckets. Slice-03 MUST preserve this for peer claims at ingest — no silent normalization on the inbound side. |
| WD-11 | openlore-foundation | Retraction = counter-claim that references the original CID; soft-retract only; no hard-delete. Slice-03's `claim counter` verb uses the SAME `references[]` mechanism. Slice-03's `peer remove --purge` deletes local CACHE only — it never deletes published records. |
| WD-12 | openlore-foundation | Identity = user's existing ATProto DID with per-application derived key. Slice-03 does NOT introduce a separate peer-identity surface; subscribing to a peer means accepting their existing DID. |
| WD-13 | openlore-foundation | Sequence: federation (slice-03) before scrapers (slice-02). This feature is the slice-03 deliverable; slice-02 starts after this lands. |
| ADR-003 | openlore-foundation | CLI verb contract: `init | claim add | claim publish | claim retract | graph query`. Slice-03 EXTENDS this list with `peer add`, `peer pull`, `peer remove`, `claim counter`, and adds `--federated` flag to `graph query`. Requires **ADR-013 amendment** as a DESIGN deliverable. |
| ADR-005 | openlore-foundation | Lexicon `org.openlore.*` stability; any added field must be optional. Slice-03's new `reason` field on `org.openlore.claim` IS optional, satisfying the rule. |
| ADR-007 | openlore-foundation | Functional Rust paradigm. Slice-03's new logic stays in pure core where it can (peer-claim CID recomputation, signature verification reuse, counter-relationship annotation derivation). |
| ADR-008 | openlore-foundation | Reference rules + `ReferenceType` enum. Slice-03 uses the existing `Counters` variant; does NOT add new variants. |
| ADR-009 | openlore-foundation | Hexagonal ports + adapters. Slice-03's new peer surface lives behind extensions to existing ports (`PdsPort`, `StoragePort`) OR a new `PeerPort` — DESIGN's call. Every new port surface MUST ship a `probe()`. |
| The literal "not as truth" framing | US-001 AC (foundation) | MUST appear in the `claim counter` compose preview too. Content-frozen across slices. |
| Single publish path (VerbClaimPublish internals) | ADR-003 + cli (foundation) | Counter-claim publish reuses this; no parallel code path. |

---

## Wave: DISCUSS / [REF] Ask-Intelligent Menu (lean mode, scoped to triggered items only)

Triggers evaluated; scoped expansion offered only for those that fired.

### Fired: cross-context complexity (≥3 contexts)

This slice spans CLI verbs + ports (extension/addition) + DuckDB schema +
Lexicon (new optional field) + ATProto PDS adapter (peer read methods).
That is 5 contexts; ≥3 threshold fires.

- **Offer**: `alternatives-considered.md` — explicitly document the rejected alternatives for the three biggest choices (counter-claim verb shape, peer storage layout, pull mechanism).
- **Cost**: ~10 minutes to write; ~3 pages output.
- **Recommendation**: **accept**. These are the choices DESIGN will second-guess if not documented now.
- **Status**: **ACCEPTED** 2026-05-27 — see `docs/feature/openlore-federated-read/discuss/alternatives-considered.md`.

### Fired: AC ambiguity (peer-trust + brigading + revocability semantics)

The peer-trust model is conceptually rich (signature verification + CID
recomputation + separate storage + per-claim attribution + soft-vs-hard
remove). Brigading and revocability anxieties are load-bearing for J-003
(unlike slice-01 where brigading was secondary). Happy/edge/error scenarios
in user-stories.md do not cover the anxiety-path force.

- **Offer**: `gherkin-scenarios-expanded.md` — add anxiety-path and habit-path scenarios per the JTBD-BDD integration template. Target: ≥3 anxiety + ≥2 habit.
- **Cost**: ~15 minutes; ~3 pages output.
- **Recommendation**: **accept**. The anxiety force is load-bearing for J-003 (three distinct anxieties); without dedicated scenarios DISTILL will have to invent them.
- **Status**: **ACCEPTED** 2026-05-27 — see `docs/feature/openlore-federated-read/discuss/gherkin-scenarios-expanded.md` (delivered 4 anxiety + 2 habit scenarios).

### Fired: multi-stakeholder narrative (multiple personas active in this slice)

Slice-03 activates P-002 as primary AND extends P-001 with a federation-reader
hat. Both personas exercise the same flows but from different starting
mental models (P-002: pragmatic reader; P-001: skeptical-curious author).

- **Offer**: extend `docs/product/personas/researcher-tech-lead.yaml` with a `hats:` section describing the federation-reader hat (typical session, anxieties, success signals, UX guardrails).
- **Cost**: ~5 minutes; ~1 page output.
- **Recommendation**: **accept**. Both personas exercise the same verbs; the hat doc keeps the journey YAMLs solution-neutral without losing persona-specific guidance.
- **Status**: **ACCEPTED** 2026-05-27 — see updated `docs/product/personas/researcher-tech-lead.yaml` (new `hats:` section).

### NOT fired: regulatory / compliance complexity

Slice-03 handles peer DIDs (public identifiers) and peer-published claims
(public, signed, federated). No PII beyond what is publicly published.
GDPR / compliance concerns are inherent to ATProto's federated model, not
introduced by slice-03. Re-evaluate at slice-05 (AppView/Search) when
aggregation surface widens.

### NOT fired: integration density

Slice-03 adds 3 new integration surfaces (peer DID resolution, peer PDS
records read, peer_claims storage), all extensions of slice-01 ports. No
new external service integrations.

### Menu action

Three fired offers were **accepted** in this DISCUSS wave. The three
artifacts are linked above and added to the DESIGN read-list.

Telemetry: each `expand` acceptance should ideally emit a
`DocumentationDensityEvent` via the standard ask-intelligent telemetry
helper. The helper does not yet exist (greenfield repo); the events are
recorded here for retroactive backfill when the helper lands.

| Trigger | Artifact | Should emit |
|---|---|---|
| `cross_context_complexity` | `alternatives-considered.md` | `DocumentationDensityEvent{ feature: openlore-federated-read, wave: DISCUSS, expansion: alternatives-considered, accepted: true, ts: 2026-05-27 }` |
| `ac_ambiguity` | `gherkin-scenarios-expanded.md` | `DocumentationDensityEvent{ feature: openlore-federated-read, wave: DISCUSS, expansion: gherkin-scenarios-expanded, accepted: true, ts: 2026-05-27 }` |
| `multi_stakeholder_narrative` | persona `hats:` extension | `DocumentationDensityEvent{ feature: openlore-federated-read, wave: DISCUSS, expansion: persona-hats, accepted: true, ts: 2026-05-27 }` |

---

## Wave: DISCUSS / [REF] Open Decisions for User

The three decisions below are surfaced for user input. Auto-mode default
verdicts are noted; the user may confirm or override.

| ID | Decision | Default verdict | Why it matters |
|---|---|---|---|
| OD-FED-1 | Counter-claim verb shape: sugar verb `claim counter <cid>` (recommended) vs flag `claim add --counters <cid>`. | **Sugar verb** (`claim counter`) per WD-17 | Affects ADR-013 amendment size and `graph query --federated` output tip-line shape. Locked as default; surface for user override. |
| OD-FED-2 | First-pull orientation messages (in habit scenario 1, gherkin-scenarios-expanded.md): show every first-pull-with-new-peers vs once-per-user via identity.toml state. | **Once-per-user** (recommended) | UX-only; minor. DESIGN can revisit. Listed here because the DISTILL acceptance designer will ask. |
| OD-FED-3 | `peer audit <did>` verb (anxiety scenario 3): ship in slice-03 as an additional verb vs defer to slice-04. | **Defer to slice-04** (recommended) | Slice-03 already adds 4 new verbs; a fifth would push the verb count up. The audit functionality can be derived from `peer list --include-purged` + `graph query --include-counters` in slice-03 without the dedicated verb. |

If the user has no objection, all three defaults LOCK on handoff to DESIGN.

---

## Wave: DISCUSS / [REF] Handoff

### To DESIGN (nw-solution-architect)

- Read (explicit list — every file matters):
  - `feature-delta.md` (this file)
  - Everything in `docs/feature/openlore-federated-read/discuss/`:
    - `user-stories.md`
    - `story-map.md`
    - `outcome-kpis.md`
    - `shared-artifacts-registry.md`
    - `journey-subscribe-and-read-federated-visual.md`
    - `journey-author-counter-claim-visual.md`
    - **`alternatives-considered.md`** (fired ask-intelligent expansion)
    - **`gherkin-scenarios-expanded.md`** (fired ask-intelligent expansion)
  - `docs/feature/openlore-federated-read/slices/slice-03-federated-read.md`
  - `docs/product/jobs.yaml` (J-003 deepened with sub-jobs and three anxieties)
  - `docs/product/journeys/subscribe-and-read-federated.yaml`
  - `docs/product/journeys/author-counter-claim.yaml`
  - `docs/product/personas/researcher-tech-lead.yaml` (extended with federation-reader hat)
  - Slice-01 lock context (do NOT relitigate; treat as inherited inputs):
    - `docs/feature/openlore-foundation/feature-delta.md` (especially WD-9..WD-13)
    - `docs/feature/openlore-foundation/design/architecture-design.md`
    - `docs/feature/openlore-foundation/design/component-boundaries.md`
    - ADR-003 (for the verb contract being amended)
    - ADR-005, ADR-007, ADR-008, ADR-009

- Decide:
  - **ADR-013 (amendment to ADR-003 verb contract)**: add `peer add`, `peer pull`, `peer remove`, `claim counter` verbs; add `--federated` flag to `graph query`. Document verb grammar consistency (noun-verb).
  - **Port surface for peer ops**: extend `PdsPort` with peer-read methods, AND extend `StoragePort` with peer-store methods, OR introduce a new `PeerPort` combining both. ADR-009 invariants apply (probe() required).
  - **`peer_subscriptions` and `peer_claims` DuckDB schemas**: column types, indexes, foreign-key shape between `peer_claims` and `peer_subscriptions` (must allow dangling FK after soft-remove). Forward-only migration from slice-01 schema.
  - **`reason` Lexicon field**: where in `org.openlore.claim` it lives; length validation; serde shape.
  - **`adapter-atproto-pds::probe()` extension** for peer-read paths: fixture peer DID + sentinel records, CID round-trip assertion.
  - **`xtask check-arch` rule** enforcing "no JOIN between author_claims and peer_claims that elides author_did column."
  - **Query shape** for `graph query --federated` that returns `peer_claims` AND `author_claims` rows grouped by author DID with bidirectional `counters`/`countered-by` annotation in a single call.

- Constraints inherited from this DISCUSS (DO NOT relitigate without coming back to PO):
  - **WD-17**: counter-claim verb shape is `claim counter` sugar verb.
  - **WD-18**: pull is pull-on-demand only.
  - **WD-19**: peer storage = single DuckDB, two new tables, check-arch enforces no-elide-author JOINs.
  - **WD-20**: `--reason` REQUIRED on counter-claims, 1..=1000 chars.
  - **WD-21**: `peer remove --purge` REQUIRES interactive confirmation; no `--yes` flag.
  - **WD-22**: counter-claim publish reuses slice-01 publish pipeline (no parallel path).
  - **WD-23**: `reason` field is OPTIONAL in Lexicon (forward-compatible with slice-01 readers); no new ReferenceType variant.
  - **WD-24**: per-claim signature verification AND CID recomputation at pull time, both required, both reject-per-claim on failure.
  - **WD-25**: soft-remove retains cache; hard-purge deletes peer claims only; counter-claims survive peer removal.

### To DEVOPS (nw-platform-architect, parallel)

- Read: `outcome-kpis.md` (Handoff to DEVOPS section).
- Deliver:
  - Instrumentation plan for KPI-FED-1..6 (especially the `claim.counter.published` tracing event for KPI-FED-3 and the `federation.e2e.duration_seconds` histogram for KPI-FED-5).
  - **Adversarial peer fixture** in CI: a test PDS that publishes deliberately tampered records for `peer_tampered_signature_rejected` test (KPI-FED-6).
  - Contract tests for the new peer-read paths on `adapter-atproto-pds` via Pact (extending the slice-01 Pact suite).
  - Dashboards for KPI-FED-3 (counter-claim publication rate per active reader-user, 30-day window) and KPI-FED-5 (P50/P95 of e2e latency per peer-cardinality bucket).
  - Alerting on KPI-FED-1, KPI-FED-2, KPI-FED-6 != 100% (release-blocking).

### To DISTILL (nw-acceptance-designer)

- Read:
  - `docs/product/journeys/subscribe-and-read-federated.yaml` (embedded Gherkin per step)
  - `docs/product/journeys/author-counter-claim.yaml` (embedded Gherkin per step)
  - `docs/feature/openlore-federated-read/discuss/user-stories.md` (UAT scenarios per story)
  - `docs/feature/openlore-federated-read/discuss/shared-artifacts-registry.md` (integration gates 1-4)
  - **`docs/feature/openlore-federated-read/discuss/gherkin-scenarios-expanded.md`** (anxiety + habit scenarios; some carry `# DISTILL: confirm` flags for verb-shape resolution)
- Build executable acceptance tests including:
  - The four integration gates from the shared-artifacts registry (`federation_attribution_preserved`, `peer_cid_round_trip`, `counter_target_cid_round_trip`, `peer_remove_purge_separation`).
  - The adversarial signature test (`peer_tampered_signature_rejected`) using the fixture peer DEVOPS sets up.
  - The first-counter-claim framing block test (habit scenario 2) once DESIGN settles the trigger condition.
- The `# DISTILL: confirm` comments throughout `gherkin-scenarios-expanded.md` mark verb shapes / behaviors implied by the requirements but not yet locked. Each must be resolved against DESIGN's final CLI verb / flag structure before building tests.

### Handoff-ready?

**YES.** All WD-14..WD-25 LOCKED in this DISCUSS; three ask-intelligent
expansions delivered (`alternatives-considered.md`,
`gherkin-scenarios-expanded.md`, persona `hats:` extension); lean Tier-1
output stands. Three Open Decisions (OD-FED-1, OD-FED-2, OD-FED-3) have
auto-mode default verdicts and may proceed unless the user overrides;
none are blocking for DESIGN to start.

DESIGN + DEVOPS may proceed in parallel; DISTILL has the scenarios it
needs.
