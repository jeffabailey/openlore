# Feature Delta: openlore-appview-search

> Wave: **DISCUSS** (lean mode v3.14 + ask-intelligent)
> Feature type: Cross-cutting (NEW indexer binary + verified network ingest + CLI `search` discovery surface + the local-first↔network-service architectural shift)
> Walking skeleton: Yes (this sibling IS the walking skeleton for the appview slice)
> Research depth: Comprehensive (anti-merging-at-network-scale + signature-verified-before-index are load-bearing; the local-first↔network-service shift is the headline product tension)
> JTBD: mandatory (every story carries `job_id` -> `docs/product/jobs.yaml`)
> Inherits from: `docs/feature/openlore-foundation/feature-delta.md` (WD-9..WD-13, ADR-001..012), `docs/feature/openlore-federated-read/feature-delta.md` (WD-14..WD-25, ADR-013..016), `docs/feature/openlore-github-scraper/feature-delta.md` (WD-46..WD-58 + WD-59/65/67, ADR-017..019), `docs/feature/openlore-scoring-graph/feature-delta.md` (WD-69..WD-79, ADR-020..022)
> Date: 2026-05-28
> Owner: Luna (nw-product-owner)

This file is the canonical DISCUSS-wave delta for `openlore-appview-search`, the
fifth and FINAL sibling feature in the OpenLore umbrella (slice-05). Tier-1
content is inlined under `## Wave: DISCUSS / [REF] <Section>` headings; SSOT
content lives under `docs/product/`; per-journey artifacts under
`docs/feature/openlore-appview-search/discuss/`.

slice-05 adds an **AppView / indexer service (a separate binary,
`openlore-indexer`)** that ingests PUBLIC signed claims from ACROSS the network —
beyond the user's own claims (slice-01) and beyond their manually-subscribed peers
(slice-03) — and delivers **query-driven discovery ("search by philosophy") at
network scale**. It closes the last unmet third of the J-001 push: claims today
are "unstructured, unsigned, and **undiscoverable**" — slices 01/02/04 solved
structure + signing, slice-03 solved manual federation, but a great claim by an
unknown author stayed undiscoverable. slice-05 closes the DISCOVERABILITY gap at
NETWORK scale.

The AppView is a READ / discovery surface. The CLI + signed claims remain the
source of truth; the indexer never overwrites, merges, signs, or publishes.
Discovery is a front-door that feeds the slice-03 federation flow. Per the brief,
slice-05 "adds an indexer service (separate binary)"; the deployment shape,
CLI→indexer transport, degraded-mode mechanism, and pull-vs-Firehose ingestion are
DESIGN's calls (OD-AV-1..4). DISCUSS frames the product requirement + its forces;
it does NOT resolve the architecture.

---

## Wave: DISCUSS / [REF] Wave Decisions

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-100 | slice-05 ships in a SIBLING feature `openlore-appview-search` (this feature) per the carpaccio split locked by WD-9. slice-05 IS the walking skeleton for this feature (one slice = one feature). It is the FINAL umbrella slice (the umbrella sequence WD-13 completes: federation -> scrapers -> scoring -> appview). | Inherits WD-9. Sibling-feature pattern keeps each slice independently shippable. slice-05 completes Activity 5 ("Explore") of the foundation story map at NETWORK scale. | LOCKED |
| WD-101 | Persona priority for slice-05: **P-002 Researcher / Tech Lead (network-discovery hat) = primary**; **P-001 Senior Engineer Solo Builder = secondary** (wears the same discovery hat when discovering aligned projects/maintainers before committing to a stack). | slice-05's load-bearing job (J-005) is a discovery/decision job; P-002 (read-mostly consumer) is the natural network-discoverer. P-001 wears the same hat at cold-start when committing to a stack. | LOCKED |
| WD-102 | **J-005 is a NEW first-class job** (network-scale discoverability), DISTINCT from J-002 (local-graph exploration). Same dimensions (subject/object/contributor), but the corpus is a NETWORK INDEX of many authors' PUBLIC claims, not the local store. J-005 closes the last unmet third of the J-001 push ("undiscoverable"). | slices 01/02/04 solved structure+signing; slice-03 solved MANUAL federation; but a claim by an author the user has never subscribed to stayed undiscoverable. That is a genuinely distinct job (cold-start discovery without a follow-list), not a variation of local exploration. Opportunity score 15 (importance 8, satisfaction 1). | LOCKED (J-005 added to jobs.yaml) |
| WD-103 | **Anti-merging extends to NETWORK aggregates (load-bearing, cardinal).** Every indexed / searched / shared result MUST preserve per-author attribution. No faceless "network consensus" row exists anywhere. Identical claims by different authors render as separate rows. The index has NO schema for a merged multi-author record. | Carries the slice-03 I-FED-1 (→ slice-04 I-GRAPH-1/2) anti-merging invariant into network scale — the single most load-bearing trust guarantee across all slices. Violating it collapses the AppView into yet another centralized aggregator that hides provenance, the exact failure mode the whole product exists to replace. The `xtask check-arch` `no_cross_table_join_elides_author` rule extends to the index query path. | LOCKED, enforced by US-AV-001/002/003/006 AC + KPI-AV-2 + acceptance test `network_result_preserves_attribution` |
| WD-104 | **Signature-verified-before-index (load-bearing, cardinal).** The indexer MUST verify each claim's signature AND recompute its CID against the published record BEFORE the claim enters the index. No unsigned/tampered/CID-mismatched claim is ever indexed or returned by a search. Reuses the pure `claim-domain` verification core (no second verification path). | The trust precondition for the whole slice. Mirrors slice-03 pull-time verification (KPI-FED-6) at network scale. Without it, discovery is just an untrusted aggregator that could serve fabricated reasoning. Every result carries a `[verified]` marker by construction (verification is an ingest gate, not a per-result runtime guess). | LOCKED, enforced by US-AV-001/004 AC + KPI-AV-3 + acceptance test `indexer_rejects_unverified_claim` |
| WD-105 | **Public-data-only / claims-are-public framing.** Indexing aggregates ONLY public signed claims (records the author published to their PDS). A "your peer-published claims are public and network-discoverable" expectation is surfaced honestly via an up-front banner. The indexer reads no private data and exposes no surveillance affordance. | ADR-014 deferred this framing to slice-05. The honest banner mitigates the "did I expose data I did not mean to?" + "surveillance tool?" anxieties (the latter inherited from the J-004 scraper mitigation: the contributor is the SUBJECT of public claims, never a controller). | LOCKED, enforced by US-AV-004 AC + KPI-AV-5 + acceptance test `public_data_banner_shown` |
| WD-106 | **CLI-first / local-first source of truth preserved; the AppView is a READ/discovery surface only.** The CLI + local signed claims remain the source of truth. The indexer never overwrites, merges, signs, or publishes; it holds no signing capability by construction (mirrors slice-02 `adapter-github`). Compose/sign/own-claim flows stay 100% local + offline-capable (KPI-5). `search` degrades to local-only when the index is unreachable. | The whole product is CLI-first + local-first (KPI-5). An AppView is inherently a NETWORK service — a genuine architectural shift (WD-107). The product requirement is that the shift is ADDITIVE: local-first authoring is never compromised, discovery degrades gracefully. | LOCKED, enforced by US-AV-001/002 AC + KPI-5 (inherited guardrail) + acceptance test `local_first_preserved` |
| WD-107 | **The local-first↔network-service tension is FLAGGED for DESIGN, not resolved in DISCUSS.** Whether the indexer is self-hostable vs hosted, how the CLI reaches it (HTTP/XRPC), the degraded local-only mechanism, and pull-vs-Firehose ingestion are DESIGN decisions (OD-AV-1..4). DISCUSS frames the PRODUCT requirement (additive, attribution-preserving, verified, graceful-degradation) and its forces; the user-visible contracts hold regardless of the architecture chosen. | This is the headline architectural shift of the slice and the umbrella's "easiest to scope-creep" slice. Pre-deciding the architecture in DISCUSS would over-constrain DESIGN and risk scope-creep; framing the forces + requirement keeps DISCUSS solution-neutral while making the tension explicit. | LOCKED (framing); architecture is DESIGN's call (OD-AV-1..4) |
| WD-108 | **Firehose is a DESIGN OPTION, not a slice-05 requirement.** Pull-based indexing may suffice for the walking skeleton. ADR-016 locked OUT push subscriptions for slice-03 with a "re-evaluate at slice-05" note; that re-evaluation is a DESIGN decision (OD-AV-4), not a DISCUSS commitment. | Treating Firehose as a requirement would balloon the slice and pre-empt DESIGN. The walking skeleton needs the index to exist and be trustworthy + searchable; HOW claims arrive (pull vs Firehose) is invisible to the user-visible contract. | LOCKED (Firehose deferred to DESIGN option) |
| WD-109 | **Search dimensions to ship: by object (philosophy), by contributor (DID/handle), by subject (project)** — mirroring slice-04, but over the NETWORK INDEX corpus, not the local graph. `--object` is the headline ("search by philosophy"). | J-005 functional needs all three dimensions. They mirror the slice-04 local dimensions for habit-continuity (the user already learned them), but the corpus is the network index. Each preserves per-author attribution (WD-103) and carries a `[verified]` marker (WD-104). | LOCKED, US-AV-002 (object), US-AV-003 (contributor/subject) |
| WD-110 | **Discovery feeds federation via the slice-03 `openlore peer add` path (no parallel subscription mechanism); the shareable link encodes a QUERY, not a frozen snapshot.** A discovered unfollowed author is one step from `peer add`; following is always an explicit human action (no auto-subscribe). A `--share` link encodes the query so it resolves to current per-author-attributed verified results. | The discovery→federation funnel is what makes the AppView STRENGTHEN the local-first graph instead of competing with it (KPI-AV-4). Reusing `peer add` verbatim preserves the slice-03 sovereignty model (revocable, no residue). Encoding the query (not a snapshot) keeps the shared artifact attribution-preserving + always-current (anti-merging across the share boundary, WD-103), realizing the J-004 shareable-link signal without becoming an aggregator. | LOCKED, US-AV-005 (funnel) + US-AV-006 (share) |

### Scope Assessment

`## Scope Assessment: PASS — 6 user stories (5 user-visible + 1 infra), 1 cohesive bounded context (network indexer + CLI discovery surface), estimated ~12.5 days. Single slice = single feature; a full web AppView UI is explicitly OUT of scope to prevent scope creep.`

Carpaccio gate evaluation (5 taste tests) — applied with extra rigor because this
is the umbrella's "easiest to scope-creep" slice:

- **Stories**: 6 (within <=10 threshold). PASS.
- **Bounded contexts**: 1 (network discovery: a verified-attributed index + a CLI
  `search` surface over it, feeding back into the existing federation context).
  The indexer is a new binary but a single cohesive context. PASS.
- **Walking-skeleton integration points**: the walking skeleton (US-AV-001 +
  US-AV-002 + US-AV-004) needs: (1) network ingest, (2) signature/CID verification
  (reused pure core), (3) the index store, (4) the CLI→indexer query transport,
  (5) the slice-03 relationship-labeling read. That is 5 — AT the <=5 threshold.
  PASS (at the boundary — flagged; the verification reuse and relationship-label
  read are reuses, not net-new, which keeps it tractable).
- **Estimated effort**: ~12.5 days (within <=2 weeks threshold, at the upper end).
  PASS — tractable because verification + subscription + relationship-labeling are
  REUSED from slices 01/03, not rebuilt.
- **Multiple independent outcomes**: NO — all 6 stories serve J-005 and its
  sub-jobs (search-at-scale, verified-attributed-ingest, discovery-feeds-
  federation). The follow funnel and shareable link are aspects of the same
  discover-trustworthily-then-act outcome, not independent outcomes. PASS.
- **Verdict**: RIGHT-SIZED **for the CLI discovery surface**. **Single slice =
  single sibling feature.** The single thing that WOULD make it oversized — a full
  presentational web AppView — is explicitly OUT of scope (story-map deferred
  table); a minimal link resolver is the most DESIGN may add and only if it keeps
  Release 3 right-sized (OD-AV-6). Cross-user/cohort SCORING and Firehose are
  deferred (WD-108 + story-map deferred table).

### Risks logged

- **KPI-AV-1 (discover an unfollowed author in >=60% of sessions)** is the slice's
  load-bearing behavioral hypothesis AND depends on INDEX COVERAGE. If the index
  is sparse or biased toward already-followed authors, discovery surfaces nothing
  new. Mitigation: an index-coverage dashboard (distinct authors indexed, ingest
  lag) handed to DEVOPS; KPI-AV-1 disprover (<20%) triggers a coverage/UX
  re-investigation before any web AppView investment.
- **Production multibase (z6Mk...) PLC DID-document pubkey decode is unresolved**
  (a slice-03 DV-4 test-only seam). True network-scale signature verification
  (WD-104 / KPI-AV-3) needs real PLC pubkey decode against arbitrary network
  authors. Mitigation: flagged as a hard DESIGN dependency; the `indexer_rejects_
  unverified_claim` gate is written against the verification CONTRACT regardless
  of the pubkey-decode mechanism, but DESIGN MUST resolve the decode for the gate
  to hold against real data. This is the single biggest technical risk inherited.
- **The local-first↔network-service shift (WD-107)** could lead DESIGN toward a
  hosted-only indexer that quietly undermines sovereignty. Mitigation: the
  `local_first_preserved` gate + KPI-5 guardrail make offline authoring +
  graceful degradation release-blocking; OD-AV-1 recommends self-hostable.
- **Scope creep into a full web AppView** is the umbrella-identified risk for this
  slice. Mitigation: the story-map deferred table draws a hard line; Release 3
  (share) is isolated last and held to a shareable-link + resolver contract;
  cross-user scoring + Firehose are explicitly deferred.
- **Anti-merging at network scale (WD-103)** is harder than locally because the
  corpus is large and an aggregating index query is tempting. Mitigation: the
  `no_cross_table_join_elides_author` xtask rule extends to the index query path
  (three-layer enforcement: non-`Option` author DID in the index record type +
  the xtask rule + the behavioral `network_result_preserves_attribution` test).
- DISCOVER + DIVERGE skipped (same as slices 01/02/03/04). The four-forces analysis
  for J-005 was performed in this DISCUSS without prior validation interviews.
  Mitigation: KPI-AV-1 + KPI-AV-4 + the day-30 study surface mis-prioritization
  within 30 days of release; the J-001 "undiscoverable" push is the validated
  source of the discoverability gap.

---

## Wave: DISCUSS / [REF] JTBD Analysis Summary

Full analysis in `docs/product/jobs.yaml`. Summary for slice-05:

| Job | Name | Priority for slice-05 | Opportunity Score | In slice-05? |
|---|---|---|---|---|
| J-005 | Discover signed claims across the network without knowing who to follow first | primary (walking-skeleton for this feature) | 15 (importance 8, satisfaction 1; underserved-primary-for-slice) | yes — all 6 stories |
| J-005a (sub-job) | Search by philosophy/subject/contributor at network scale | LOAD-BEARING | n/a (sub-job) | yes — US-AV-002, US-AV-003, US-AV-006 |
| J-005b (sub-job) | Index only signature-verified, attributed public claims | LOAD-BEARING | n/a (sub-job) | yes — US-AV-001, US-AV-004 |
| J-005c (sub-job) | Turn a discovery into a follow (discovery feeds federation) | supporting | n/a (sub-job) | yes — US-AV-005 |
| J-001 | Author a signed philosophical claim | source of the push | 16 | the "undiscoverable" third of J-001's push is what slice-05 closes at network scale |
| J-002 | Explore the philosophy graph (LOCAL) | sibling (built on) | 14 | DISTINCT — slice-04 explores the LOCAL graph; slice-05 searches the NETWORK index. The funnel (US-AV-005) feeds discovered authors into the slice-03/04 local surfaces |
| J-003 | Read another developer's federated claims | inherited (the follow target) | 15 | the discovery→federation funnel (US-AV-005) drops discovered authors into the slice-03 `peer add` flow |
| J-004 | Evaluate a contributor's body of work | inherited (related) | 13 | partial — the J-004 "shareable as a link to a query" success signal is realized by US-AV-006; the network contributor lens (US-AV-003) extends J-004's lens to network scale |

J-005 was ADDED during this DISCUSS as a first-class job (network-scale
discoverability), with three sub-jobs (search-at-scale, verified-attributed-
ingest, discovery-feeds-federation), an opportunity score of 15, and the
`walking_skeleton_for: openlore-appview-search` marker. J-005 is DISTINCT from
J-002: same dimensions, but the corpus is a network index of many authors' PUBLIC
claims, not the local store. slice-05 BUILDS ON J-003 (the follow funnel reuses
`peer add`) and REALIZES part of J-004 (the shareable-link signal + the contributor
lens at network scale). It does NOT relitigate J-002/J-003/J-004.

---

## Wave: DISCUSS / [REF] Journey Artifacts

One journey to map (discover-across-the-network is the single coherent surface;
the four steps search -> trust -> read-before-following -> act are one continuous
arc):

- Visual journey (search -> trust -> read -> act): `docs/feature/openlore-appview-search/discuss/journey-discover-across-the-network-visual.md`
- Structured schema (with embedded Gherkin per step): `docs/product/journeys/discover-across-the-network.yaml` (to be produced for DISTILL; mirrors the slice-04 `explore-the-graph.yaml` placement)
- Shared artifacts registry: `docs/feature/openlore-appview-search/discuss/shared-artifacts-registry.md`

Emotional arc:

- Discover-across-the-network journey: **cold-start-hope-to-connected-trust (with
  a verification buffer)** — entry Cold-start-hopeful-but-wary (cares about a
  philosophy but follows nobody who claims it; anxious the AppView is just another
  aggregator / serves tampered claims / betrays local-first) through Reassured
  (the public-data banner + `[verified]` markers build trust) and the Discovery-Joy
  peak (a relevant claim by an unfollowed author appears) to Connected + Defensible
  (follow the discovered author into the local graph; share the discovery). The
  TRUST step is deliberately placed immediately after the first results and BEFORE
  any action, so trust in the verified/attributed data precedes acting on it.

Three cross-cutting guarantees are elevated to their own section in the visual
journey:

- **Signature-verified-before-index** (J-005b / WD-104): verify signature +
  recompute CID before indexing; reuse the pure verification core; every result
  `[verified]` by construction; public-data banner up front.
- **Anti-merging in NETWORK aggregates** (extends slice-03 I-FED-1 + slice-04
  I-GRAPH-1/2; WD-103): every indexed/searched/shared result preserves per-author
  attribution; the index has no merged-record schema; the `xtask check-arch`
  no-elide-author rule extends to the index query path.
- **Local-first preserved despite the network-service shift** (WD-106 / KPI-5):
  authoring stays offline-capable; the indexer is signing-incapable by
  construction; `search` degrades gracefully; the architecture mechanics are
  DESIGN's call (WD-107).

---

## Wave: DISCUSS / [REF] Story Map and Slicing

- Story map: `docs/feature/openlore-appview-search/discuss/story-map.md`

Slicing summary:

- **Release 1 (walking skeleton)**: US-AV-001 + US-AV-002 + US-AV-004. Validates
  trustworthy network discovery by philosophy end-to-end — a verified, attributed
  index; a search that surfaces unfollowed authors; a visible trust contract.
- **Release 2 (dimensions + funnel)**: US-AV-003 + US-AV-005. Completes the
  contributor/subject dimensions and CLOSES the discovery→federation funnel (the
  behavior that makes the AppView strengthen the local-first graph).
- **Release 3 (shareable discovery)**: US-AV-006. Realizes the J-004 shareable-link
  signal; isolated last to fight scope creep.

Priority order is set by outcome impact and risk-of-failure consequence (Release 1
fails = the J-005 trustworthy-discovery-at-scale thesis is disproven AND the two
riskiest assumptions — verifiable+attributed index, search surfaces beyond local —
are unvalidated; Release 3 fails = survivable amplification gap). Rationale in
story-map.md `## Priority Rationale` section.

All 5 carpaccio taste tests evaluated (Scope Assessment above): right-sized in
stories, contexts, integration points (at the <=5 boundary, flagged), effort, and
outcome coherence. Verdict: SINGLE SLICE = SINGLE FEATURE; a full web AppView is
explicitly OUT of scope.

---

## Wave: DISCUSS / [REF] User Stories and Acceptance Criteria

All in `docs/feature/openlore-appview-search/discuss/user-stories.md`:

| Story | Title | Job link | Elevator Pitch | DoR status |
|---|---|---|---|---|
| US-AV-001 | Bootstrap the indexer + verified, attributed ingest (`@infrastructure`) | `infrastructure-only` | n/a — @infrastructure | PASS (with infra rationale) |
| US-AV-002 | Search by philosophy (object) at network scale, attribution preserved | J-005 | yes | PASS |
| US-AV-003 | Search by contributor or subject at network scale | J-005 | yes | PASS |
| US-AV-004 | Trust a discovered result — verified marker + public-data honesty | J-005 | yes | PASS |
| US-AV-005 | Subscribe to a discovered author (discovery → federation) | J-005 | yes | PASS |
| US-AV-006 | Share a network search result as a stable link | J-005 | yes | PASS |

Slice composition gate: PASS — 5 user-visible stories + 1 infrastructure story;
slice is NOT 100% `@infrastructure` (per `nw-po-review-dimensions` Dimension 0 §5).

---

## Wave: DISCUSS / [REF] Outcome KPIs

Full table in `docs/feature/openlore-appview-search/discuss/outcome-kpis.md`.
North star:

> **KPI-AV-1**: >=60% of dogfood discovery sessions surface >=1 relevant signed
> claim by an author the user does NOT already subscribe to, via a network search,
> within 30 days of release.

Guardrails: **KPI-AV-2 (anti-merging in NETWORK aggregates — zero attribution
loss)** and **KPI-AV-3 (signature-verified-before-index — zero unverified claims
indexed)** — the two cardinal trust guarantees; both MUST hold, any failure is
unshippable. Plus inherited guardrails KPI-5 (local-first authoring not
compromised) and KPI-4 (zero silent normalization at network scale).

Leading indicators: KPI-AV-4 (discovery→federation funnel — discovery grows the
trusted local graph), KPI-AV-6 (shared-link usage — discovery becomes a decision
artifact), KPI-AV-5 (public-data framing comprehension).

KPI numbering: KPI-AV-1..6.

---

## Wave: DISCUSS / [REF] Definition of Ready validation

| DoR item | US-AV-001 | US-AV-002 | US-AV-003 | US-AV-004 | US-AV-005 | US-AV-006 |
|---|---|---|---|---|---|---|
| 1. Problem statement clear, domain language | PASS (infra rationale) | PASS | PASS | PASS | PASS | PASS |
| 2. Persona with specific characteristics | n/a (infra) | PASS (P-002+P-001) | PASS (P-002+P-001) | PASS (P-002+P-001) | PASS (P-002+P-001) | PASS (P-002+P-001) |
| 3. >=3 domain examples with real data | PASS (2 — narrow infra surface) | PASS (4) | PASS (4) | PASS (4) | PASS (4) | PASS (4) |
| 4. UAT in Given/When/Then (3-7) | PASS (2 — narrow infra surface) | PASS (4) | PASS (4) | PASS (4) | PASS (4) | PASS (3) |
| 5. AC derived from UAT | PASS | PASS | PASS | PASS | PASS | PASS |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS (3d, 2) | PASS (2.5d, 4) | PASS (2d, 4) | PASS (1.5d, 4) | PASS (1.5d, 4) | PASS (2d, 3) |
| 7. Technical notes: constraints/dependencies | PASS | PASS | PASS | PASS | PASS | PASS |
| 8. Dependencies resolved or tracked | PASS (slice-01 verification, slice-03 pull precedent) | PASS (US-AV-001) | PASS (US-AV-001/002) | PASS (US-AV-001/002) | PASS (US-AV-002/003 + slice-03 peer verbs) | PASS (US-AV-002/003) |
| 9. Outcome KPIs defined with measurable targets | n/a — supports KPI-AV-1..4 | PASS (KPI-AV-1, 2, 3) | PASS (KPI-AV-1, 2, 4) | PASS (KPI-AV-3, 5) | PASS (KPI-AV-4) | PASS (KPI-AV-6, 2) |

**Overall DoR status: PASSED** for all stories.

Notes:
- Item 3 + Item 4 (US-AV-001): the spec allows 3-7 scenarios; US-AV-001 ships 2
  composite scenarios because the infrastructure surface is narrow and additional
  scenarios would be padding. Same pattern as US-005 (slice-01), US-FED-006
  (slice-03), US-SCR-006 (slice-02), and US-GRAPH-006 (slice-04). Flagged for
  reviewer judgment but considered PASS.
- Item 6 (US-AV-001): 3 days is the upper edge of the right-sized band (1-3 days);
  it is the indexer-bootstrap + verified-ingest pipeline (the slice's biggest infra
  surface). It reuses the slice-01 verification core and slice-03 verification
  discipline rather than rebuilding them, which keeps it tractable. Splitting it
  further (e.g., "bootstrap binary" vs "ingest gate") would fragment a single
  cohesive infra outcome and create a story with no demonstrable value at all.
  Flagged; considered PASS at the boundary.
- Item 2 (US-AV-001): infrastructure-only stories do not require a persona;
  `infrastructure_rationale` present per Decision 1.
- US-AV-002 is the largest user-visible story at 2.5 days / 4 scenarios — within
  the right-sized band. It is the headline "search by philosophy" surface;
  splitting it further would fragment the discoverability outcome.

### Elevator Pitch verification (BLOCKING per Dimension 0)

Per `nw-po-review-dimensions` Dimension 0 (checked first, BLOCKING):

| Story | Section present? | Real entry point? | Concrete output? | Job connection? | Verdict |
|---|---|---|---|---|---|
| US-AV-001 | n/a (@infrastructure with rationale) | n/a | n/a | n/a (`infrastructure-only` per Decision 1) | PASS via rationale |
| US-AV-002 | YES (Before/After/Decision enabled) | YES (`openlore search --object org.openlore.philosophy.dependency-pinning`) | YES (specific stdout: "Network results for ... (12 signed claims across 7 subjects, 9 distinct authors — all signature-verified)" grouped by author + `[verified]` + `(not subscribed)` + no-merge footer) | YES (discover well-evidenced reasoning across the network without first knowing whom to follow) | PASS |
| US-AV-003 | YES | YES (`openlore search --contributor github:priya` / `--subject github:bazelbuild/bazel`) | YES (specific stdout: "Network claims authored by did:plc:priya-test (8 verified claims across 6 subjects)" + "one developer's reasoning trail, not a community consensus") | YES (evaluate a discovered contributor's trail before following; survey a project at network scale) | PASS |
| US-AV-004 | YES | YES (`openlore search --object ... --show bafy...k2`) | YES (specific stdout: full record + "Signature: VERIFIED against did:plc:priya-test" + "CID recomputed, matches published record" + public-data banner) | YES (act on a network discovery the same way as a self-pulled peer, instead of dismissing it as aggregator noise) | PASS |
| US-AV-005 | YES | YES (`openlore peer add did:plc:priya-test` affordance shown in a result) | YES (specific stdout: "Added did:plc:priya-test ... next pull will ingest their claims" then claims appear in local `graph query`) | YES (turn a network discovery into a followed peer that grows the trusted local graph) | PASS |
| US-AV-006 | YES | YES (`openlore search --object ... --share`) | YES (specific stdout: "Shareable link: openlore://search?object=..." + "encodes the query, not a frozen snapshot") | YES (hand a teammate a reproducible, attributed, verified discovery as an ADR-citable artifact) | PASS |

Slice-level Elevator Pitch check (Dimension 0 §5): the slice has 5 user-visible
stories + 1 infrastructure story. Slice is NOT 100% `@infrastructure`. PASS.

---

## Wave: DISCUSS / [REF] Locks inherited from prior slices

These are binding inputs to this feature's DESIGN wave. They are NOT relitigated
here; any change requires returning to the owning slice's product-owner review
first.

| ID | Inherited from | Carries into slice-05 as |
|---|---|---|
| WD-9 | openlore-foundation | Carpaccio split: each slice is an independent sibling feature. slice-05 is this (final) feature. |
| WD-10 / I-6 | openlore-foundation | Numeric `[0.0, 1.0]` is the only persisted/indexed confidence; display-only buckets. The index stores/serves numeric confidence; search-result buckets are render-only. |
| WD-11 | openlore-foundation | Retraction = counter-claim referencing the original CID; soft-retract only. The indexer is read-only and authors no retractions; a countered/retracted claim, if published + verified, is still indexed + discoverable (slice-03 coexist semantics) — DESIGN may define a retraction-aware search filter (Open Decision OD-AV-7). |
| WD-12 | openlore-foundation | Identity = user's existing ATProto DID with per-application derived key. The indexer introduces NO new identity surface; it verifies signatures against authors' existing DIDs. |
| WD-13 | openlore-foundation | Sequence: federation -> scrapers -> scoring -> appview. slice-05 (appview) is the FINAL slice; the umbrella sequence completes here. |
| WD-22 / I-FED-5 | openlore-federated-read | Single publish/subscribe path. The AppView adds NO write surface; following a discovered author reuses the slice-03 `openlore peer add` verbatim (WD-110). The indexer signs/publishes nothing. |
| WD-25 / I-FED-1 | openlore-federated-read | Anti-merging at storage + query + display + test. slice-05 EXTENDS this to NETWORK aggregates (WD-103): the `no_cross_table_join_elides_author` rule + the non-`Option` author DID discipline extend to the index query path. |
| KPI-FED-6 | openlore-federated-read | Pull-time signature-verify + CID-recompute gate. slice-05 EXTENDS this to network-scale INGEST (WD-104): no unverified claim is indexed. **CAVEAT inherited**: production multibase (z6Mk...) PLC pubkey decode was a slice-03 test-only seam (DV-4); DESIGN MUST resolve it for KPI-AV-3 against real network data (flagged risk). |
| WD-58 | openlore-github-scraper | `derived-from` provenance is informational and never alters confidence/federation. Scraper-signed claims are normal author claims; if published + verified, they are indexed + discoverable like any author claim. |
| I-SCR-1 | openlore-github-scraper | The human-gate at the architecture layer (`adapter-github` holds no storage/identity/pds reference; it cannot sign or publish). slice-05 mirrors this: the indexer holds no signing/publishing capability by construction (US-AV-001 AC). |
| WD-72 / I-GRAPH-4 | openlore-scoring-graph | Weights/scores are DERIVED + DISPLAY-ONLY, never persisted. slice-05 inherits: the index stores VERIFIED CLAIMS, never derived aggregates; a shareable link encodes a QUERY, never a frozen merged snapshot (WD-110). |
| WD-73 / I-GRAPH-1/2 | openlore-scoring-graph | Anti-merging extends to aggregates. slice-05 carries it to NETWORK aggregates (WD-103). |
| ADR-014 | openlore-federated-read | Per-peer-DID partition / public-data framing. slice-05 REALIZES the deferred "claims-are-public" framing (WD-105) via the public-data banner. |
| ADR-016 | openlore-federated-read | Peer DID resolution / federated read; push subscriptions locked OUT with a "re-evaluate at slice-05" note. slice-05 re-evaluates Firehose as a DESIGN OPTION (WD-108 / OD-AV-4), not a requirement. |
| ADR-003 / ADR-013 / ADR-020 | foundation / federated-read / scoring-graph | CLI verb contract + the `--federated` flag + the `graph query` explorer-flag precedents. slice-05 adds a NEW `openlore search` verb (or a `--network` flag — OD-AV-5) + a NEW `openlore-indexer` binary. Requires an **ADR (next number after ADR-022, i.e. likely ADR-023+)** as a DESIGN deliverable. |
| ADR-007 | openlore-foundation | Functional Rust paradigm. The reused verification is PURE core; network I/O + index storage stay behind ports in the effect shell. |
| ADR-009 | openlore-foundation | Hexagonal ports + adapters. Any new port/adapter surface (indexer ingest, index query, CLI→indexer transport) MUST ship a `probe()` (I-4) within the 250ms budget (I-5). |
| ADR-001 | openlore-foundation | DuckDB single-file store. The index store choice (DuckDB FTS vs a search engine vs reuse) is DESIGN's call; user-visible contracts hold regardless. |
| KPI-5 | brief (cross-feature) | Local-first guardrail. slice-05 PRESERVES it despite the network-service shift (WD-106): offline compose/sign + graceful degradation are release-blocking (`local_first_preserved`). |
| KPI-4 | brief (cross-feature) | Zero silent normalization. slice-05 inherits: a discovered claim's fields match the author's published record (the index normalizes nothing); `--show` CID-recompute-matches-published enforces it at network scale. |

---

## Wave: DISCUSS / [REF] Ask-Intelligent Menu (lean mode, scoped to triggered items only)

Triggers evaluated; scoped expansion offered only for those that fired.

### Fired: cross-context complexity (>=3 contexts) + the architectural-shift decision

This slice spans a NEW `openlore-indexer` binary + a verified network ingest
pipeline + a new CLI `search` discovery surface + the index store + the
CLI→indexer transport + the local-first↔network-service architectural shift. That
is well beyond 3 contexts; the threshold fires strongly.

- **Offer**: `alternatives-considered.md` — document the rejected alternatives for
  the biggest choices (deployment: self-hostable single-binary vs hosted service;
  ingestion: pull-based vs ATProto Firehose; index store: DuckDB FTS vs a search
  engine; CLI→indexer transport: HTTP vs XRPC; discovery surface: new `search` verb
  vs `--network` flag on `graph query`).
- **Cost**: ~12 minutes; ~3-4 pages output.
- **Recommendation**: **accept**. These are the choices DESIGN will second-guess if
  not framed now — especially the local-first↔network-service shift (load-bearing
  for WD-106/WD-107) and the pull-vs-Firehose re-evaluation (ADR-016).
- **Status**: **ACCEPTED (auto-mode)** 2026-05-28 — to be produced as
  `docs/feature/openlore-appview-search/discuss/alternatives-considered.md`
  alongside the DESIGN handoff. Flagged as a DESIGN read.

### Fired: AC ambiguity (verified-before-index + anti-merging-at-network-scale + local-first-degradation semantics are easy to disagree on)

The verification gate (verify before index, no unverified result), anti-merging at
network scale (no consensus row, identical-different-author = two rows), and
graceful local-only degradation are conceptually rich, and the J-005 anxiety
forces (aggregator-distrust, tampered-claim, local-first-betrayal,
accidental-exposure) are load-bearing. The happy/edge/error scenarios in
user-stories.md cover the functional surface but not every anxiety-path force
explicitly.

- **Offer**: `gherkin-scenarios-expanded.md` — add anxiety-path and habit-path
  scenarios per the JTBD-BDD integration template. Target: >=4 anxiety
  (aggregator-distrust, tampered-claim, local-first-betrayal, accidental-exposure)
  + >=2 habit ("I already run `gh search`"; "the follow flow is the slice-03
  `peer add` I know").
- **Cost**: ~15 minutes; ~3 pages output.
- **Recommendation**: **accept**. The anxiety forces are load-bearing for J-005;
  without dedicated scenarios DISTILL will have to invent them.
- **Status**: **ACCEPTED (auto-mode)** 2026-05-28 — to be produced as
  `docs/feature/openlore-appview-search/discuss/gherkin-scenarios-expanded.md`.
  Flagged as a DISTILL read.

### Fired: multi-stakeholder narrative (both personas active in this slice)

slice-05 activates P-002 (network-discovery hat) as primary AND extends P-001 with
the same discovery hat. Both exercise `search` but from different starting mental
models (P-002: pragmatic team-tooling discovery; P-001: solo cold-start stack
discovery).

- **Offer**: extend `docs/product/personas/researcher-tech-lead.yaml` with a
  `network-discovery` hat (typical session, anxieties, success signals, UX
  guardrails), mirroring the slice-03 `federation-reader`, slice-02
  `contributor-evaluator`, and slice-04 `graph-explorer` hats.
- **Cost**: ~5 minutes; ~1 page output.
- **Recommendation**: **accept**.
- **Status**: **DONE (auto-mode)** 2026-05-28 — the `network-discovery` hat was
  ADDED in place under the existing `hats:` section of `researcher-tech-lead.yaml`
  during this DISCUSS. Flagged as a DESIGN read.

### Fired: regulatory / privacy complexity (NOW fires — the network-aggregation surface widens)

Unlike slice-04 (local-only), slice-05 aggregates many authors' claims into a
network index — a genuinely wider data surface. The slice-04 note said
"re-evaluate at slice-05 when cross-user aggregation widens the surface." It now
fires.

- **Offer**: a privacy/public-data framing section (folded into the system
  constraints + WD-105 + the public-data banner + KPI-AV-5). The mitigation is the
  public-data-only invariant: indexing covers ONLY public signed claims, the
  banner surfaces the expectation honestly, and the indexer reads no private data
  and exposes no surveillance affordance (inherited J-004 mitigation: the
  contributor is the SUBJECT of public claims, never a controller).
- **Cost**: handled inline (no separate artifact).
- **Recommendation**: **accept inline** — the framing is load-bearing (WD-105,
  KPI-AV-5) and is captured in the system constraints, US-AV-004, and the journey.
- **Status**: **ACCEPTED inline (auto-mode)** 2026-05-28. No separate artifact;
  captured in WD-105 + US-AV-004 + the public-data banner.

### NOT fired: integration density (external integrations)

slice-05 adds 1 new internal binary (`openlore-indexer`) + reuses the existing
ATProto read paths (slice-03) for ingestion + the existing `peer add` for the
funnel. The network ingestion is the existing ATProto surface, not a new external
integration. Below the threshold (the architectural-shift complexity is captured
by the cross-context-complexity trigger above, not a separate integration-density
expansion).

### Menu action

Four fired offers were **accepted (auto-mode)** in this DISCUSS wave. Two artifacts
(`alternatives-considered.md`, `gherkin-scenarios-expanded.md`) are scoped to be
produced alongside the DESIGN/DISTILL handoff and are flagged in the read-lists
below; the persona-hat extension is DONE in place; the privacy/public-data framing
is accepted inline. (In strict interactive mode these would be offered to the user;
in auto-mode the recommended `accept` verdict is taken per the auto-mode
product-defaults instruction.)

| Trigger | Artifact | Should emit |
|---|---|---|
| `cross_context_complexity` | `alternatives-considered.md` | `DocumentationDensityEvent{ feature: openlore-appview-search, wave: DISCUSS, expansion: alternatives-considered, accepted: true, ts: 2026-05-28 }` |
| `ac_ambiguity` | `gherkin-scenarios-expanded.md` | `DocumentationDensityEvent{ feature: openlore-appview-search, wave: DISCUSS, expansion: gherkin-scenarios-expanded, accepted: true, ts: 2026-05-28 }` |
| `multi_stakeholder_narrative` | persona `network-discovery` hat | `DocumentationDensityEvent{ feature: openlore-appview-search, wave: DISCUSS, expansion: persona-hats, accepted: true, ts: 2026-05-28 }` |
| `privacy_complexity` | inline (WD-105 + US-AV-004 + banner) | `DocumentationDensityEvent{ feature: openlore-appview-search, wave: DISCUSS, expansion: privacy-framing-inline, accepted: true, ts: 2026-05-28 }` |

---

## Wave: DISCUSS / [REF] Open Decisions for User / DESIGN

The decisions below are surfaced for user/DESIGN input. Auto-mode default verdicts
are noted (and locked as WDs above where applicable); the user may confirm or
override. OD-AV-1..4 are the headline local-first↔network-service architecture
decisions DESIGN owns.

| ID | Decision | Default verdict | Why it matters |
|---|---|---|---|
| OD-AV-1 | Indexer deployment shape: self-hostable single binary the user runs, vs a hosted service the CLI queries. | **DESIGN's call (WD-107); recommend SELF-HOSTABLE single binary for slice-05** to preserve the sovereignty/local-first ethos; a hosted service may follow | Self-hostable keeps data sovereignty (the P-001 non-negotiable) and matches the single-binary Rust ethos. A hosted service is lower-friction but introduces a trust/centralization concern the product exists to avoid. DESIGN owns the final call. |
| OD-AV-2 | CLI→indexer transport: HTTP vs ATProto XRPC vs a local IPC for the self-hosted case. | **DESIGN's call; recommend HTTP/XRPC consistent with the existing ATProto stack** | The transport is invisible to the user-visible contract but constrains the deployment shape and the degraded-mode mechanism. |
| OD-AV-3 | Degraded local-only mode mechanism when the index is unreachable. | **Product requirement: graceful degradation to a clear local-only/unavailable message (LOCKED, WD-106); the MECHANISM is DESIGN's** | KPI-5 guardrail: `search` must never block the local-first flows. HOW (fallback to local `graph query`, cached last-good index, etc.) is DESIGN's. |
| OD-AV-4 | Ingestion mode: pull-based indexing vs ATProto Firehose (the ADR-016 re-evaluation). | **DESIGN's call (WD-108); pull-based may suffice for the walking skeleton; Firehose is an OPTION, not a requirement** | Firehose is real-time but heavier and was locked OUT for slice-03. Pull-based is simpler and sufficient to validate the discovery thesis. DESIGN re-evaluates ADR-016. |
| OD-AV-5 | Discovery surface grammar: a new top-level `openlore search` verb vs a `--network` flag on the existing `openlore graph query`. | **Recommend a new `openlore search` verb** (clearly distinct corpus: network index vs local graph); DESIGN owns the final grammar | A distinct verb makes the corpus boundary (network vs local) unambiguous; a flag reuses the learned `graph query` surface. DISTILL will ask which; the product requirement is that the corpus distinction is clear. |
| OD-AV-6 | Minimal link resolver for `--share`: CLI re-run only, vs a minimal web AppView that renders a shared link. | **CLI re-run only for slice-05; a web AppView is OUT of scope** (story-map deferred table); DESIGN may add a minimal resolver only if it keeps Release 3 right-sized | The shareable link is the scope-creep risk surface. Holding it to a CLI re-run + a stable link keeps slice-05 tractable; a full web UI is a future slice. |
| OD-AV-7 | Retraction/counter-aware search: should a claim that has been countered or soft-retracted appear in network search normally, be flagged, or be excluded? | **Appear normally in slice-05 (with the counter relationship surfaced if known); filtering deferred** | slice-03 keeps countered claims visible (coexist, never overwrite). A retraction-aware search filter is a richer concern; for slice-05 the default is "all verified public claims are discoverable; the counter relationship is shown, not silently applied." Surface for override. |

If the user has no objection, the defaults LOCK on handoff to DESIGN (OD-AV-1..4
remain DESIGN's architecture calls per WD-107).

---

## Wave: DISCUSS / [REF] Handoff

### To DESIGN (nw-solution-architect)

- Read (explicit list — every file matters):
  - `feature-delta.md` (this file)
  - Everything in `docs/feature/openlore-appview-search/discuss/`:
    - `user-stories.md`
    - `story-map.md`
    - `outcome-kpis.md`
    - `shared-artifacts-registry.md`
    - `journey-discover-across-the-network-visual.md`
    - **`alternatives-considered.md`** (fired ask-intelligent expansion — to be produced)
    - **`gherkin-scenarios-expanded.md`** (fired ask-intelligent expansion — to be produced)
  - `docs/product/jobs.yaml` (J-005 added with sub-jobs J-005a/b/c)
  - `docs/product/personas/researcher-tech-lead.yaml` (extended with the network-discovery hat)
  - `docs/product/journeys/discover-across-the-network.yaml` (to be produced for DISTILL)
  - Prior-slice lock context (do NOT relitigate; treat as inherited inputs):
    - `docs/product/architecture/brief.md` (Component Inventory + cumulative CLI surface + invariants I-1..I-12 + the slice-05 "adds an indexer service (separate binary)" note)
    - `docs/feature/openlore-foundation/feature-delta.md` (WD-8..WD-13)
    - `docs/feature/openlore-federated-read/feature-delta.md` (WD-24/WD-25 anti-merging, the ADR-013 verb-amendment precedent, KPI-FED-6 pull-time verification) + `docs/feature/openlore-federated-read/design/data-models.md` (the `FederatedRow` non-`Option` author_did discipline + the DV-4 test-only peer-pubkey seam)
    - `docs/feature/openlore-scoring-graph/feature-delta.md` (WD-72 display-only / WD-73 anti-merging-in-aggregates)
    - ADR-001 (store), ADR-003/013/020 (verb/flag contract being extended), ADR-007, ADR-009, ADR-014 (public-data framing), ADR-016 (Firehose re-evaluation)

- Decide (the headline DESIGN decisions):
  - **The local-first↔network-service architecture (OD-AV-1..4)**: indexer
    deployment shape (self-hostable vs hosted — recommend self-hostable),
    CLI→indexer transport, the degraded-local-only mechanism, and pull-vs-Firehose
    ingestion (the ADR-016 re-evaluation; Firehose is an OPTION, not a requirement).
    Trade-off framing in `alternatives-considered.md`. Document in the slice-05
    design + a new ADR (likely ADR-023+). The PRODUCT requirements (additive,
    attribution-preserving, verified, graceful-degradation) are LOCKED (WD-103..107)
    and hold regardless of the architecture chosen.
  - **The verified ingest pipeline + the production pubkey-decode dependency**:
    reuse the pure `claim-domain` verification core (no second path); RESOLVE the
    production multibase (z6Mk...) PLC DID-document pubkey decode that slice-03 left
    as a test-only seam (DV-4) — this is the hard technical dependency for WD-104 /
    KPI-AV-3 against real network data. Define the indexed-record type with a
    non-`Option` `author_did` (anti-merging at ingest, mirroring `FederatedRow`).
  - **The index store + the anti-merging-preserving query shape**: choose the index
    store (DuckDB FTS vs a search engine); define the search query method(s) (by
    object/subject/contributor) that compose aggregates from individually-attributed
    records (never a stored merged row); extend the `xtask check-arch`
    `no_cross_table_join_elides_author` rule to the index query path. Any new port
    surface ships `probe()` (ADR-009).
  - **The `search` verb/flag ADR (likely ADR-023+)**: add `openlore search` (or a
    `--network` flag — resolve OD-AV-5) with `--object`/`--contributor`/`--subject`
    + `--show <cid>` + `--share`. Document grammar consistency with the slice-03
    `--federated` and slice-04 explorer-flag precedents.
  - **The discovery→federation funnel + the shareable-link contract**: the follow
    affordance reuses the slice-03 `peer add` verbatim (no parallel path); the
    `--share` link encodes the QUERY (not a snapshot). Resolve OD-AV-6 (CLI re-run
    vs minimal web resolver — a full web AppView is OUT of scope).
  - **Component Inventory update**: the new `openlore-indexer` binary (and any new
    index-store/ingest/query crates) gain rows in the brief's Component Inventory at
    finalize.

- Constraints inherited from this DISCUSS (DO NOT relitigate without coming back to PO):
  - **WD-103**: anti-merging extends to NETWORK aggregates; every indexed/searched/shared result preserves per-author attribution; no consensus row.
  - **WD-104**: signature-verified + CID-recomputed BEFORE indexing; reuse the pure verification core; every result `[verified]` by construction.
  - **WD-105**: public-data-only / claims-are-public framing; honest up-front banner; no private read, no surveillance affordance.
  - **WD-106**: CLI-first / local-first source of truth preserved; the AppView is read/discovery only; offline authoring + graceful degradation are guardrails; the indexer is signing-incapable by construction.
  - **WD-107**: the local-first↔network-service architecture is DESIGN's call (OD-AV-1..4); the user-visible contracts are architecture-neutral.
  - **WD-108**: Firehose is a DESIGN OPTION, not a requirement; pull-based may suffice.
  - **WD-109**: ship search dimensions by object/contributor/subject over the network corpus.
  - **WD-110**: discovery feeds federation via slice-03 `peer add` (no parallel path); the shareable link encodes a query, not a snapshot.

### To DEVOPS (nw-platform-architect, parallel)

- Read: `outcome-kpis.md` (Handoff to DEVOPS section).
- Deliver:
  - Instrumentation plan for KPI-AV-1..6 (especially `search.discovery.unfollowed_author_hit` for KPI-AV-1, `search.discovery.follow_funnel` for KPI-AV-4, and the indexer-side `indexer.ingest.verified` vs `indexer.ingest.rejected{reason}` counters for KPI-AV-3 — all privacy-preserving: structural counts + DIDs the user already saw, never claim contents or user-behavior surveillance).
  - An INDEX FRESHNESS / COVERAGE dashboard (claims indexed, distinct authors indexed, ingest lag) — feeds the KPI-AV-1 sparsity diagnosis (the load-bearing "is the index too sparse to discover anything?" question).
  - Dashboards for KPI-AV-1 (% of discovery sessions with >=1 unfollowed-author hit per 30-day window) and KPI-AV-4 (discovery→follow funnel conversion).
  - Alerting on KPI-AV-2 and KPI-AV-3 != 100% (release-blocking), KPI-5 regression (release-blocking); informational alert on KPI-AV-1 < 30% at day-30 and on index ingest lag exceeding a DESIGN-defined freshness budget.
  - The indexer is a NEW deployable (the architectural shift): DEVOPS must plan for it per the OD-AV-1 deployment shape (self-hostable single binary vs hosted). Confirm the local-first flows add no new network dependency (offline compose/sign unchanged).

### To DISTILL (nw-acceptance-designer)

- Read:
  - `docs/product/journeys/discover-across-the-network.yaml` (embedded Gherkin per step — to be produced)
  - `docs/feature/openlore-appview-search/discuss/user-stories.md` (UAT scenarios per story)
  - `docs/feature/openlore-appview-search/discuss/shared-artifacts-registry.md` (integration gates 1-7)
  - **`docs/feature/openlore-appview-search/discuss/gherkin-scenarios-expanded.md`** (anxiety + habit scenarios; some will carry `# DISTILL: confirm` flags for verb-shape / deployment / ingestion resolution)
- Build executable acceptance tests including:
  - **Network search by dimension**: `search --object`/`--contributor`/`--subject` return attributed, grouped, `[verified]` results including unfollowed authors (US-AV-002/003).
  - **Verified-before-index** (release-gate): `indexer_rejects_unverified_claim` drives tampered-signature/CID-mismatch/unsigned fixtures; none enter the index or a result (US-AV-001/004; KPI-AV-3).
  - **Anti-merging at network scale** (release-gate): `network_result_preserves_attribution` — every result row carries one author DID; identical-content-different-author = two rows; the `no_cross_table_join_elides_author` rule covers the index query path (US-AV-001/002/006; KPI-AV-2).
  - **Trust display**: `verified_marker_is_universal` + `--show` signature/CID lines (US-AV-004).
  - **Public-data framing**: `public_data_banner_shown` (US-AV-004; KPI-AV-5).
  - **Discovery→federation funnel**: `discovery_follow_reuses_slice03_path` — the follow affordance reuses `peer add`; no auto-subscribe; no parallel state (US-AV-005; KPI-AV-4).
  - **Shareable link**: `share_link_encodes_query_not_snapshot` — opening resolves to current per-author-attributed verified results, never a stored merged snapshot (US-AV-006; KPI-AV-6).
  - **Local-first preserved** (release-gate): `local_first_preserved` — offline compose/sign succeed; `search` degrades gracefully when the index is unreachable (KPI-5).
- The `# DISTILL: confirm` comments throughout `gherkin-scenarios-expanded.md` mark behaviors implied by the requirements but not yet locked (e.g. the `search` verb vs `--network` flag, the deployment shape, pull-vs-Firehose, the production pubkey-decode mechanism). Each must be resolved against DESIGN's final decisions before building tests.

### Handoff-ready?

**YES.** All WD-100..WD-110 LOCKED in this DISCUSS; J-005 added to jobs.yaml with
sub-jobs; the `network-discovery` persona hat added in place; four ask-intelligent
expansions accepted (auto-mode) — `alternatives-considered.md` and
`gherkin-scenarios-expanded.md` scoped for production alongside the DESIGN/DISTILL
handoff, the persona hat DONE in place, the privacy/public-data framing accepted
inline; lean Tier-1 output stands. Seven Open Decisions (OD-AV-1..7) have auto-mode
default verdicts and may proceed unless the user overrides; none are blocking for
DESIGN to start (OD-AV-1..4, the local-first↔network-service architecture, are
DESIGN's call by design).

One hard DESIGN dependency is flagged as a risk: production multibase (z6Mk...) PLC
DID-document pubkey decode (a slice-03 DV-4 test-only seam) MUST be resolved for the
WD-104 / KPI-AV-3 verification gate to hold against real network data.

DESIGN + DEVOPS may proceed in parallel; DISTILL has the scenarios it needs.

---
---

## Wave: DESIGN / [REF] Overview

> Wave: **DESIGN** (nw-solution-architect; auto-mode)
> Architect: Morgan
> Date: 2026-05-28
> Deliverables: `docs/feature/openlore-appview-search/design/` (architecture-design.md, component-boundaries.md, data-models.md, technology-stack.md, wave-decisions.md) + ADR-023..027 in `docs/adrs/`
> Inherits: ADR-001..022, WD-1..WD-93, the 12 cross-feature invariants + I-FED-1..7 + I-SCR-1..7 + I-GRAPH-1..8; this feature's DISCUSS WD-100..WD-110 + OD-AV-1..7

slice-05 is delivered as an ADDITIVE EXTENSION introducing the FIRST network
service: a self-hostable single binary `openlore-indexer` that PULLS public signed
claims, VERIFIES each (signature + CID, against the author's REAL PLC DID-doc key)
BEFORE indexing, and SERVES network-scale discovery via an HTTP/XRPC query method
that a new `openlore search` verb consumes. The CLI + local store remain the source
of truth; the indexer is signing-incapable by construction and holds no local-store
handle; `search` degrades gracefully when the indexer is unreachable. All cardinal
guarantees (anti-merging at network scale, verified-before-index, local-first) hold
by construction.

## Wave: DESIGN / [REF] Wave Decisions

| # | Decision | Rationale (short) | Status | ADR |
|---|---|---|---|---|
| WD-111 | Additive extension; first network service; TWO binaries in one workspace (no re-architecture). | Conservative scope; validates on top of the proven local-first/federated surface. | LOCKED | — |
| WD-112 | **OD-AV-1: self-hostable single binary `openlore-indexer`, signing-incapable by construction.** | Data sovereignty + single-binary ethos; hosted is an additive future option. | LOCKED | ADR-023 |
| WD-113 | **OD-AV-5: a NEW top-level `openlore search` verb** (not a `--network` flag). | Unambiguous LOCAL-vs-NETWORK corpus boundary; preserves "`graph query` is always local". | LOCKED | ADR-027 |
| WD-114 | **OD-AV-4 (ADR-016 re-eval): PULL-based bounded ingestion; Firehose deferred.** | Pull suffices for the discovery thesis; simpler, hermetically testable; reuses slice-03 verification. | LOCKED | ADR-024 |
| WD-115 | **OD-AV-2: CLI→indexer = HTTP carrying the `org.openlore.appview.searchClaims` XRPC method; configured URL (localhost default).** | Deployment-independent (makes hosted additive); ATProto-stack consistent; reuses `reqwest`. | LOCKED | ADR-027 |
| WD-116 | **OD-AV-3: graceful local-only degradation; unreachable = soft non-fatal; indexer not probed at CLI startup.** | KPI-5 cardinal: `search` never blocks local-first flows; per-`search`-soft check. | LOCKED | ADR-027 |
| WD-117 | **Index store = a SEPARATE `index.duckdb`, reusing DuckDB (not a search engine).** | Exact dimensional lookup (not free-text); reuses the proven anti-merging SQL substrate (the cardinal WD-103 reason). | LOCKED | ADR-025 |
| WD-118 | **OD-AV-6 (pubkey): implement the production PLC `z6Mk...` decode NOW; pure helper in `claim-domain` + effect resolution; the test seam release-forbidden.** | KPI-AV-3 cannot hold against real data with a test seam; deferred since slice-03; reuses the pure verify (no second path). | LOCKED | ADR-026 |
| WD-119 | **OD-AV-7: countered/retracted public verified claims appear normally; counter shown, never applied; filter deferred.** | Mirrors slice-03 coexist + slice-04 WD-85; hiding provenance would betray the model. | LOCKED | — |
| WD-120 | **Anti-merging at network scale (I-AV-2): three-layer enforcement; aggregation in the pure core, never SQL; NO merged schema.** | Cardinal WD-103 carried to the hardest surface; single-layer bypass caught by the other two. | LOCKED | ADR-025 |
| WD-121 | **Verified-before-index (I-AV-1) reuses the pure `claim-domain` verify+CID core; `verified_against NOT NULL`; no second path.** | Cardinal WD-104/KPI-AV-3; carries slice-03 KPI-FED-6 to network scale verbatim. | LOCKED | ADR-024/026 |
| WD-122 | **Discovery→federation reuses `peer add` verbatim (render-only; no parallel path; no auto-follow); `--share` encodes the query, not a snapshot.** | WD-110: strengthens the local-first graph; anti-merging across the share boundary. | LOCKED | ADR-027 |
| WD-123 | **Two external/cross-process boundaries (CLI→indexer; indexer→PDS/PLC), annotated for consumer-driven contract tests.** | First external boundaries since slice-01/02; highest-risk; contract tests pin attribution + verify-gate shapes. | LOCKED | — |
| WD-124 | ADR-023..027 accepted with this handoff; no further DESIGN iterations pending peer review. | Each ADR has 2+ alternatives + an Earned-Trust probe contract; the novel risks each met by a dedicated probe. | LOCKED pending review | — |

Full decision records (rationale + locks + the OD-AV-1..7 resolution table +
Q-DELIVER-AV deferrals): `docs/feature/openlore-appview-search/design/wave-decisions.md`.

## Wave: DESIGN / [REF] DDD / Bounded Context

slice-05 is ONE bounded context (network discovery: a verified-attributed index +
a CLI `search` surface over it) realized across a network boundary. The aggregate
root is the **verified indexed claim** (`IndexedClaim`) — its identity is its
verified CID, its attribution is its non-`Option` author DID, and it cannot exist
without having passed the verify-before-index gate. There is NO "network consensus"
aggregate (the load-bearing absence, WD-103): the only aggregate is a per-author
COMPOSITION computed at query time. The context maps to the federation context
(slice-03) via an Open Host Service relationship — discovery FEEDS `peer add` but
shares no write surface (I-FED-5 / WD-122).

## Wave: DESIGN / [REF] Component Decomposition

New crates (production count 11 → 17; +1 test-support +1 xtask = 19 members):

| Crate | Kind | Binary | New/Extends |
|---|---|---|---|
| `appview-domain` | PURE core | indexer | NEW — ingest-gate decision + search/grouping/anti-merging composition; no I/O |
| `adapter-atproto-ingest` | EFFECT | indexer | NEW — `IngestSourcePort` (bounded PULL, read-only) |
| `adapter-index-store` | EFFECT | indexer | NEW — `IndexStorePort` over `index.duckdb` |
| `adapter-xrpc-query-server` | EFFECT | indexer | NEW — serves `org.openlore.appview.searchClaims` over HTTP |
| `adapter-index-query` | EFFECT | cli | NEW — `IndexQueryPort` (CLI→indexer client; graceful degradation) |
| `openlore-indexer` | DRIVER (binary) | — | NEW — the SECOND composition root; signing-incapable; no local store |
| `claim-domain` | PURE core | both | EXTENDS — adds `decode_ed25519_multibase` (ADR-026); verify/CID reused unchanged |
| `lexicon` | PURE | both | EXTENDS — adds the `org.openlore.appview.searchClaims` READ query lexicon |
| `ports` | PURE traits | both | EXTENDS — `IndexQueryPort`/`IngestSourcePort`/`IndexStorePort`/`IdentityResolvePort` + `IndexedClaim`/`NetworkResultRow` ADTs |
| `adapter-atproto-did` | EFFECT | both | EXTENDS — the verify-only production PLC pubkey-decode path (ADR-026) |
| `cli` | DRIVER (binary) | — | EXTENDS — the `openlore search` verb; wires/soft-probes the index-query client |
| `xtask` | dev tooling | — | EXTENDS — anti-merging rule → index store; capability-boundary rule; no-pubkey-seam-in-release; appview-domain allowlist |

Full boundaries, composition-root wiring (both roots), and the crafter/DISTILL/
DEVOPS annotations: `docs/feature/openlore-appview-search/design/component-boundaries.md`.

## Wave: DESIGN / [REF] Driving / Driven Ports

| Port | Direction | Side | Surface |
|---|---|---|---|
| `openlore search` verb | DRIVING | cli | the user-facing discovery surface (ADR-027) |
| `org.openlore.appview.searchClaims` XRPC method | DRIVING | indexer | the inbound query API (served by `adapter-xrpc-query-server`) |
| `IndexQueryPort` | DRIVEN | cli | CLI→indexer transport; `Unreachable` is a SOFT outcome (graceful degradation) |
| `IngestSourcePort` | DRIVEN | indexer | bounded PULL of public records; read-only (no write/sign/publish method) |
| `IndexStorePort` | DRIVEN | indexer | `index.duckdb` read/write; non-`Option` author_did; no author-eliding aggregate |
| `IdentityResolvePort` | DRIVEN | shared | verify-only DID-doc → pubkey (production PLC `z6Mk...` decode); no signing method |

Every driven port's adapter ships a `probe()` within the 250ms budget (ADR-009
I-4/I-5), each exercising its catalogued "what if X lies?" substrate-lie scenario
(the network serves tampered records → reject; the container substrate lies about
fsync → refuse; a real `z6Mk...` key decodes + verifies/rejects correctly; an
unreachable indexer degrades softly, never a CLI hard-fail).

## Wave: DESIGN / [REF] Technology Choices

| Concern | Decision | Rationale | ADR |
|---|---|---|---|
| Deployment shape | Self-hostable single binary `openlore-indexer` | Sovereignty + single-binary ethos; hosted additive later | ADR-023 |
| CLI→indexer transport | HTTP + XRPC method `org.openlore.appview.searchClaims`; configured URL | Deployment-independent; ATProto-consistent; reuses `reqwest` | ADR-027 |
| Ingestion | PULL-based bounded (Firehose deferred) | Suffices for the thesis; simpler; hermetically testable; reuses slice-03 verify | ADR-024 |
| Index store | Separate `index.duckdb`, reuse DuckDB (no search engine) | Exact dimensional lookup; reuses the proven anti-merging SQL substrate | ADR-025 |
| Pubkey decode | Production PLC `z6Mk...` multibase decode (pure helper + effect resolution); seam release-forbidden | KPI-AV-3 against real data; resolves the slice-03 DV-4 deferral | ADR-026 |
| New external deps | Minimal: an HTTP server framework (axum, MIT) + a base58 crate (bs58, MIT) — both with hand-rolled fallbacks | OSS-first; tokio-ecosystem; pure-core allowlist for the decode | — |

Full stack + license documentation + rejected alternatives (search engines,
Firehose, gRPC, a second verification path): `docs/feature/openlore-appview-search/design/technology-stack.md`.

## Wave: DESIGN / [REF] slice-05 invariants (I-AV-*)

| # | Invariant | Cardinal? | Enforced by |
|---|---|---|---|
| I-AV-1 | Verified-before-index (real PLC key; pure core; no second path; `verified_against NOT NULL`) | YES (KPI-AV-3) | ingest gate + schema + ingest probe + `indexer_rejects_unverified_claim` |
| I-AV-2 | Anti-merging at network scale (non-`Option` author_did; NO merged schema) | YES (KPI-AV-2) | type / `no_cross_table_join_elides_author` (index store) / `network_result_preserves_attribution` |
| I-AV-3 | Local-first preserved (CLI links no indexer; `search` degrades; not probed at startup) | YES (KPI-5) | CLI dep graph + soft probe + `local_first_preserved` |
| I-AV-4 | Public-data-only (public reads only; no surveillance) | guardrail (KPI-AV-5) | public listRecords + `public_data_banner_shown` |
| I-AV-5 | Indexer signing-incapable + holds no local store (mirrors I-SCR-1) | structural | verify-only/read-only ports / `indexer_holds_no_signing_or_local_store` / capability probe |
| I-AV-6 | Production pubkey decode is real (seam release-forbidden) | structural | real decode + gold test + `no_pubkey_seam_in_release_build` |
| I-AV-7 | Discovery feeds federation via `peer add` verbatim (no parallel path; no auto-follow) | behavioral (KPI-AV-4) | render-only affordance + `discovery_follow_reuses_slice03_path` |
| I-AV-8 | Shareable link encodes the query, not a snapshot | behavioral (KPI-AV-6) | query-only encoding + `share_link_encodes_query_not_snapshot` |
| I-AV-9 | Counter shown, not applied | behavioral | annotation-only + `countered_claim_still_appears` |

Full table with enforcement columns: `design/component-boundaries.md` §"Cross-component invariants".

## Wave: DESIGN / [REF] Reuse Analysis

slice-05 is reuse-heavy by design (the conservative posture the task brief
requested):

| Reused from | What | How |
|---|---|---|
| slice-01 | `claim_domain::verify` + `compute_cid` (the PURE verification core) | The verified-before-index gate calls it directly — NO second verification path (WD-104/121). |
| slice-01 | DuckDB store pattern (connection/migration/probe; fsync substrate check) | `adapter-index-store` reuses it on a separate `index.duckdb` (ADR-025). |
| slice-01 | the `<cid>.json.tmp → fsync → rename` atomic artifact write | reused for `indexed_claims/<did>/<cid>.json`. |
| slice-02 | the `adapter-github` human-gate (I-SCR-1: holds no storage/identity/pds reference) | the indexer mirrors it — signing-incapable + no local-store handle (I-AV-5/ADR-023). |
| slice-02 | workspace `reqwest` (rustls) (no new transport crate) | reused for ALL slice-05 HTTP (ingest, CLI→indexer, PLC resolution). |
| slice-03 | the pull-time verification discipline + per-record/per-source fault isolation (ADR-016/KPI-FED-6) | reused at network-scale ingest (ADR-024). |
| slice-03 | the `FederatedRow` non-`Option<Did>` author discipline | `IndexedClaim`/`NetworkResultRow` mirror it (anti-merging, I-AV-2). |
| slice-03 | the `peer_claims/<did>/` partition + DID→safe-filename encoding | reused for `indexed_claims/<did>/`. |
| slice-03 | the `peer add`/`peer pull`/`peer remove` verbs | the discovery→federation funnel reuses `peer add` VERBATIM (no parallel path; WD-122/I-AV-7). |
| slice-03 | the DV-4 test-only `OPENLORE_PEER_PUBKEY_HEX_<did>` seam | RETAINED for hermetic tests but RESOLVED for production (the real PLC decode; ADR-026). |
| slice-03/04 | the `no_cross_table_join_elides_author` xtask rule | EXTENDED to the index-store SQL (the cardinal anti-merging-substrate reuse; ADR-025). |
| slice-04 | the pure-domain-core pattern (`scoring` as the symmetric counterpart) | `appview-domain` is the new pure core (ingest gate + search composition). |
| slice-04 | the `--object`/`--contributor`/`--subject` dimension grammar | the `search` verb reuses it (habit-continuity) over the network corpus. |
| slice-04 | the WD-72 display-only / never-persist-derived-aggregate discipline | the index persists VERIFIED CLAIMS, never derived aggregates; `--share` encodes a query, never a snapshot. |

NOT reused (explicitly DEFERRED, per DISCUSS): the slice-04 cross-user SCORING
(WD-79 — the index ranks nothing in slice-05); a full web AppView (WD-100 scope
line); ATProto Firehose (WD-108/ADR-024).

## Wave: DESIGN / [REF] Upstream Issues

Non-blocking observations flagged back to DISCUSS/PO (none block DELIVER):

1. **OD-AV-6 numbering collision (cosmetic).** The DISCUSS open-decisions table
   labels the share-link resolver "OD-AV-6", while the feature-delta Risks + the
   Handoff "Decide" list treat the production pubkey-decode dependency as a
   separate (effectively a seventh-plus) DESIGN decision also informally grouped
   under the OD-AV-6/risk umbrella. DESIGN resolved BOTH distinctly (WD-122 share
   resolver; WD-118 pubkey decode). Recommend the PO renumber the pubkey-decode
   dependency as its own OD (e.g. OD-AV-8) in a future DISCUSS touch for clarity.
   Non-blocking.

2. **`alternatives-considered.md` was flagged "to be produced alongside the DESIGN
   handoff" (DISCUSS ask-intelligent menu) but is not present** in
   `docs/feature/openlore-appview-search/discuss/`. The trade-off framing it was to
   carry (deployment, ingestion, store, transport, discovery-grammar alternatives)
   is FULLY captured in the five ADRs' "Alternatives Considered" sections +
   wave-decisions.md, which is the canonical place for rejected-alternative
   rationale. No DESIGN gap results; recorded so the DISCUSS artifact ledger is
   accurate. Non-blocking.

3. **`gherkin-scenarios-expanded.md` (DISTILL read) likewise flagged "to be
   produced" is not present.** Its `# DISTILL: confirm` flags (search verb shape,
   deployment, ingestion, pubkey-decode) are ALL resolved by this DESIGN
   (WD-112/113/114/118); DISTILL can resolve them against the ADRs +
   component-boundaries DISTILL annotation. Recommend DISCUSS produce the expanded
   gherkin (anxiety/habit scenarios) if the day-30 anxiety-path coverage is wanted;
   not a DESIGN blocker.

## Wave: DESIGN / [REF] Handoff (DESIGN → DISTILL / DEVOPS / DELIVER)

- **To DISTILL (nw-acceptance-designer)**: the 8 release/acceptance gates +
  resolved `# confirm` flags (see `design/wave-decisions.md` Handoff +
  `design/component-boundaries.md` DISTILL annotation). The cardinal release gates:
  `indexer_rejects_unverified_claim` (KPI-AV-3), `network_result_preserves_attribution`
  (KPI-AV-2), `local_first_preserved` (KPI-5).
- **To DEVOPS (nw-platform-architect)**: the new `openlore-indexer` deployable
  (ADR-023); the KPI-AV-1..6 instrumentation + the index-coverage/freshness
  dashboard; release-blocking alerts on KPI-AV-2/3 != 100% + KPI-5 regression; the
  TWO consumer-driven contract tests (CLI→indexer XRPC; indexer→PDS/PLC) — see
  `design/component-boundaries.md` DEVOPS annotation.
- **To DELIVER (nw-functional-software-crafter per ADR-007)**: the locked contracts
  (ADR-023..027 + I-AV-1..9) + the Q-DELIVER-AV-1..9 deferrals; the crafter/DELIVER
  annotation in `design/component-boundaries.md`. Pure cores: `appview-domain` +
  the `claim-domain` decode helper (property + mutation tested); effect adapters
  each with a "what if X lies?" probe.

**DESIGN handoff-ready: YES** (pending Atlas solution-architect-reviewer approval).

---
---

## Wave: DEVOPS / [REF] Overview

> Wave: **DEVOPS** (nw-platform-architect; auto-mode; runs in PARALLEL with DISTILL)
> Architect: Apex
> Date: 2026-05-28
> Deliverables: `docs/feature/openlore-appview-search/devops/` (platform-design.md, ci-cd-pipeline.md, observability.md, kpi-instrumentation.md, contract-test-ownership.md, wave-decisions.md) + these feature-delta sections
> Inherits: foundation DEVOPS D-D1..D-D13 + ADR-010..012; slice-03 D-D14..D-D21; slice-02 D-D22..D-D29; slice-04 D-D30..D-D34 — ALL carry forward unchanged
> Depends on: the APPROVED DESIGN (ADR-023..027, WD-111..124, I-AV-1..9), NOT on DISTILL outputs

slice-05 is the architecturally heaviest DEVOPS slice of the umbrella: it stands up
the FIRST genuine network service (`openlore-indexer`), the FIRST external/cross-process
contract boundaries since slice-01/02, the FIRST `deny.toml` change since slice-01, and
the FIRST indexer-OPERATOR observability surface — all while keeping the local-first
CLI's DEVOPS surface (CI flows, distribution, the offline guarantee) structurally
unchanged (the indexer is an ADDITIVE deployable). Conservative + reuse-heavy: extend
the existing single `ci.yml`/`nightly.yml`; no new workflow file; no heavy new infra
(no k8s/cloud — a self-hostable single binary). Decisions are numbered D-D35..D-D43
(continuing slice-04's D-D34).

## Wave: DEVOPS / [REF] Wave Decisions

| # | Decision | Rationale (short) | Status |
|---|---|---|---|
| D-D35 | **`openlore-indexer` ships as the SECOND deployable in the ADR-011 release matrix (4 platforms; `cargo install openlore-indexer`); three cardinal release-blocking GUARDRAIL ATs + 7 search-scenario ATs land in `ci.yml`.** | KPI-AV-2/3 + KPI-5 are cardinal disprovers; the indexer is the first network service (ADR-023); ADR-011 already cited slice-05 as the trigger; the index store is re-buildable (backup = re-ingest). | LOCKED |
| D-D36 | **Contract suite `contract-pact-indexer-query` (B1, CLI↔indexer) — consumer-driven; pins every wire result carries `author_did` (anti-merging across the transport).** | WD-123: a provider change dropping `author_did` would silently merge authors; caught at build time. PR/nightly mocked; release re-verify vs real localhost indexer (no third party). | LOCKED |
| D-D37 | **Contract suite `contract-pact-pds-network` (B2, indexer→PDS/PLC) — consumer-driven; pins the `listRecords` record shape (+ adversarial set) + the PLC DID-doc `z6Mk...` shape.** | WD-123: an ATProto/PLC shape drift would silently break the verify-before-index gate (KPI-AV-3). Release re-verify vs real `bsky.social` + `plc.directory` confirms KPI-AV-3 against real data. | LOCKED |
| D-D38 | **Ingest adversarial fixtures + the real-`z6Mk...` DID-doc fixture regenerated via `cargo xtask regenerate-ingest-fixtures` (extends slice-03 D-D15); `arch-check` `--check` fails on drift.** | KPI-AV-3 release gate; the only end-to-end exercise of the network-scale reject + real-decode path; auto-regen prevents Lexicon drift. DELIVER-may-defer escape hatch. | LOCKED |
| D-D39 | **The contract public-endpoint allowlist (slice-02 D-D22) EXTENDS with `plc.directory` (NEW) alongside `bsky.social`.** | `plc.directory` is the NEW external host (ADR-026 production decode trust anchor); the allowlist keeps the real-provider variant's egress auditable. | LOCKED |
| D-D40 | **Mutation scope widens (THIRD): `crates/appview-domain` added to nightly `cargo mutants` (≥95%). KPI-AV: no RED; KPI-AV-2/3 + KPI-5 GREEN+release-blocking; KPI-AV-1/4/5/6 GREEN per-user / YELLOW cohort.** | `appview-domain` is a new pure core (`ingest_decision` + `compose_results` — the two cardinal trust primitives); THIRD widening (after `scraper-domain` D-D23 + `scoring` D-D31). KPI-AV per-user/cohort split = the D-D17/D-D26/D-D32 policy; slice-05 tightens telemetry (NO DID in any `search.*` event). | LOCKED |
| D-D41 | **The renderer-review checklist (D-D19/D-D28/D-D33) gains one slice-05 line: the network search/share renderer never collapses authors, always renders `[verified]` + relationship label + public-data banner, and `--share` encodes the query not a snapshot.** | KPI-AV-2/5/6 guardrail/outcome concerns; the checklist backstops future renderers (the D-D19/D-D28/D-D33 reasoning). | LOCKED |
| D-D42 | **`deny.toml` change (the FIRST since slice-01): narrow the `axum` ban so the indexer's query server may use it; rely on the structural `xtask check-arch` rule (CLI links no HTTP server) instead. `actix-web` stays banned; `bs58` already allowlisted.** | The slice-01 ban premise ("we never run an HTTP server in-process") is slice-05-obsolete (the indexer IS one, ADR-027). The arch rule is a stronger guarantee than a license-tool ban. Hand-rolled-`hyper` fallback needs no edit (Q-DELIVER-AV-2). Flagged as an Upstream Issue. | LOCKED |
| D-D43 | **No new ADR at the DEVOPS layer.** ADR-010/011/012 carry forward (ADR-011 gains the indexer artifact; ADR-012's allowlist applied via D-D42). The DEVOPS decisions are tactical extensions of D-D8/D-D11/D-D12/D-D23/D-D31. CAVEAT: the indexer-operator surface is the first deviation from the single-user-CLI model; a FUTURE hosted indexer (ADR-023 revisit) would need a DEVOPS ADR. | Same outcome as slice-03 D-D21, slice-02 D-D29, slice-04 D-D34. The store/transport/decode ADRs are DESIGN's (ADR-023..027), not DEVOPS. | LOCKED |

Full decision records (rationale + inheritance + the DESIGN/DELIVER open-questions +
the explicit deferrals): `docs/feature/openlore-appview-search/devops/wave-decisions.md`.

## Wave: DEVOPS / [REF] The new deployable (the architectural shift)

`openlore-indexer` is the SECOND deployable — a self-hostable single binary (ADR-023),
signing-incapable, holding no local store. DEVOPS plan (D-D35):

| Concern | Decision |
|---|---|
| Build/release | the SAME ADR-011 4-platform matrix as the CLI (native build per target); each release ships TWO binaries under one SBOM + provenance (D-D11); `cargo install openlore-indexer`. Windows stays deferred (the ADR-011 slice-05 trigger evaluates NO). |
| Run | `cargo run -p openlore-indexer serve` (ingest loop + query server) / `ingest` (one-shot pass) for the walking skeleton; a packaged service unit is a future concern. |
| Lifecycle | DISJOINT from the CLI (long-running service vs per-command CLI); the CLI is UNCHANGED for all local-first flows; `search` is the only CLI verb touching the indexer + it degrades gracefully (I-AV-3). |
| Runtime config | the indexer's OWN `config.toml` (config-disjoint from `identity.toml`): `index_path`, `listen_addr` (`127.0.0.1:7619` localhost default), `plc_endpoint` (`https://plc.directory` — the ADR-026 production decode trust anchor), `ingest_interval` (`15m`, DELIVER tunes), `[indexer.sources]` (seed_dids + optional relay). The CLI's `identity.toml` gains one optional `[appview] indexer_url` key. |
| Backup/DR | NONE designed — `index.duckdb` is RE-BUILDABLE (backup = re-ingest), distinct from the CLI's source-of-truth `openlore.duckdb`. |

Full deployable + environment matrix + runtime concerns:
`docs/feature/openlore-appview-search/devops/platform-design.md` §3, §4.

## Wave: DEVOPS / [REF] Environment matrix

| Environment | Slice-05 shape |
|---|---|
| **clean** (hermetic default) | `FakeIndexQuery` (CLI) + `FakeIngestSource`/`FakeIndexStore` (indexer) + the real-`z6Mk...` DID-doc fixture; no real network (public-data-only exercised against fixtures). |
| **with-pre-commit** | UNCHANGED in shape; the mirrored commit-stage set widens with the new crates + the extended `arch-check`/`check-probes` rules. |
| **with-stale-config** | an `identity.toml` without `[appview] indexer_url` → `search` degrades to local-only (NOT a fatal config error); an indexer `config.toml` without `[indexer.sources]` → the indexer refuses to start (empty seed = config error, not silent no-op). |
| **indexer-in-container** (NEW) | the `index.duckdb` fsync-honesty probe (ADR-025) refuses to start on a container-substrate durability lie (overlayfs/DrvFs/tmpfs `fsync` no-op → `storage.fsync_unhonored`). |
| **localhost-transport** (NEW) | the CLI↔indexer contract + the `search` ATs run against a localhost-bound fixture indexer (`127.0.0.1` on a per-test ephemeral port), hermetic. |

Posture: GRACEFUL-DEGRADE by default for the CLI `search` verb (like slice-03/04;
the `local_first_preserved` gate); FAIL-FAST at startup for the indexer (it refuses on
any probe failure — a mis-wired/substrate-lying indexer must not serve unverified or
attribution-losing results). Full matrix:
`docs/feature/openlore-appview-search/devops/platform-design.md` §4.

## Wave: DEVOPS / [REF] Observability / Telemetry (KPI-AV instrumentation)

The KPI-AV telemetry events (privacy-preserving STRUCTURAL counts only; NO claim
contents; STRICTER than slice-04 — NO DID and NO subject/object VALUE in any `search.*`
event; the public-data framing does NOT extend to surveillance):

| Event | KPI | Side | Per-user / cohort |
|---|---|---|---|
| `search.discovery.unfollowed_author_hit{dimension, unfollowed_author_count}` | KPI-AV-1 (north star) | CLI (author) | per-user GREEN / cohort YELLOW (future endpoint OR PO outreach) |
| `search.discovery.follow_funnel{time_from_search_to_add_seconds, was_previously_unfollowed}` | KPI-AV-4 (funnel) | CLI (author) | per-user GREEN / cohort YELLOW |
| `search.share.link_emitted` / `link_opened{dimension, re_resolved_result_count}` | KPI-AV-6 (share) | CLI (author) | per-user GREEN / cohort YELLOW |
| `search.public_data_banner_shown{first_session}` + one-shot D-D18 comprehension prompt | KPI-AV-5 (framing) | CLI (author) | per-user GREEN / cohort YELLOW (PO survey) |
| `search.executed{dimension, indexer_reachable, ..., degraded_local_only}` + `search.latency_seconds` | KPI-AV latency + KPI-5 sanity | CLI (author) | per-user GREEN |
| `indexer.ingest.verified` vs `indexer.ingest.rejected{reason: bad_signature\|cid_mismatch\|unsigned\|schema_unknown\|did_unresolvable}` | KPI-AV-3 (verified-before-index) | indexer (OPERATOR) | CI = cohort; runtime ratio on `openlore-indexer stats` |
| `indexer.query.attribution_missing` → counter `indexer_query_attribution_missing_total` (target 0) | KPI-AV-2 (anti-merging runtime guardrail) | indexer (OPERATOR) | CI = cohort |
| `indexer.ingest.pass_completed{..., ingest_lag_seconds}` + `indexer_distinct_authors_indexed` + `indexer_claims_indexed_total` | KPI-AV-1 coverage/freshness (the sparsity diagnosis) | indexer (OPERATOR) | per-instance dashboard (`openlore-indexer stats`) |

**The per-user vs cohort split (D-D40)**: per-user signals are FULLY captured in
slice-05 for all six KPI-AV (CLI events + `openlore stats --discovery` + `scripts/kpi-av-*.jq`);
the two GUARDRAILS (KPI-AV-2/3) + KPI-5 are CI-gate signals where CI-pass = the property
holds for every binary (cohort = CI). The four OUTCOME metrics' cohort % is YELLOW =
pending the future opt-in telemetry endpoint (NOT stood up in slice-05) OR PO day-30
outreach — the SAME deferred-endpoint constraint every prior slice carries. The
indexer-OPERATOR surface is a SEPARATE per-INSTANCE signal (the index-coverage/freshness
dashboard), NOT author-side opt-in telemetry (ADR-010) and NOT the cohort endpoint.

Full event schemas + the indexer-operator surface + the coverage dashboard + the
alerting table: `docs/feature/openlore-appview-search/devops/observability.md` +
`docs/feature/openlore-appview-search/devops/kpi-instrumentation.md`.

## Wave: DEVOPS / [REF] Contract Tests (the two external boundaries)

| Boundary | Consumer | Provider | Pins | Suite | Allowlist |
|---|---|---|---|---|---|
| **B1** CLI→indexer (`org.openlore.appview.searchClaims` XRPC, ADR-027) | the CLI (`adapter-index-query`) | the indexer (`adapter-xrpc-query-server`) | every wire result carries `author_did` (anti-merging across the transport, I-AV-2/KPI-AV-2) | `contract-pact-indexer-query` (mocked PR; real localhost release) | none (localhost own-binary) |
| **B2** indexer→network PDS + PLC (ADR-024/026) | the indexer (`adapter-atproto-ingest` + `adapter-atproto-did` resolve-only) | network-author PDSes (`listRecords`) + `plc.directory` (DID-doc) | the record-enumeration shape (+ adversarial set) + the PLC `z6Mk...` `publicKeyMultibase` shape the verify-gate + ADR-026 decode read (I-AV-1/KPI-AV-3) | `contract-pact-pds-network` (recorded PR; real bsky+plc release) | `bsky.social` + `plc.directory` (D-D39) |

Both consumer-driven; both mocked/recorded in PR (hermetic), real-provider re-verified
at release under the existing manual-approval gate (D-D12). The local-first guardrail
(KPI-5) is the contract NEGATIVE: the CLI's compose/sign/local-query path links NO
indexer code (the `xtask check-arch` CLI-dep-graph exclusion); `at-local-first-preserved`
is the behavioral confirmation. Full ownership + the recorded-fixture discipline +
the adversarial-set Pact interactions:
`docs/feature/openlore-appview-search/devops/contract-test-ownership.md`.

## Wave: DEVOPS / [REF] CI / Release / Nightly / xtask extensions

| Surface | Extension (DELIVER applies) |
|---|---|
| `ci.yml` commit-stage | the new crates picked up by the existing `--workspace` `fmt`/`clippy`/`test` jobs; `check-arch` gains THREE new rules + one extended rule + one allowlist entry (below); `check-probes` picks up the 4 new adapter probes by construction; `deny` runs the narrowed `axum` ban (D-D42) |
| `ci.yml` acceptance-stage | 3 cardinal GUARDRAIL ATs (`at-indexer-rejects-unverified-claim` KPI-AV-3, `at-network-result-preserves-attribution` KPI-AV-2, `at-local-first-preserved` KPI-5) + 7 search-scenario ATs + 2 Pact sub-jobs (`contract-pact-indexer-query`, `contract-pact-pds-network`) |
| `nightly.yml` | mutation `--package` += `crates/appview-domain` (THIRD widening, ≥95%; D-D40); the `claim-domain` decode helper mutated within the existing `claim-domain` scope; `CLAUDE.md` Mutation Strategy unchanged in POLICY |
| `release.yml` (when authored) | the SECOND binary (`openlore-indexer`) across the 4-platform matrix + cosign/SBOM/provenance for both; re-run the new ATs; the two contract suites' real-provider variants (manual approval; the new `plc.directory` host); release-tag mutation re-run covers `appview-domain` |
| `deny.toml` | narrow the `axum` ban (D-D42; the FIRST `deny.toml` change since slice-01) — REQUIRED on the `axum` path, UNNEEDED on the hand-rolled `hyper` path; `bs58` already MIT-allowlisted |
| `xtask check-arch` rules | (1) EXTEND `no_cross_table_join_elides_author` to the `adapter-index-store` SQL (I-AV-2); (2) ADD `indexer_holds_no_signing_or_local_store` (+ assert the CLI links no HTTP server, replacing the narrowed `deny.toml` ban; I-AV-5/I-AV-3); (3) ADD `no_pubkey_seam_in_release_build` (I-AV-6); (4) ADD `appview-domain` to the pure-core allowlist; (5) EXTEND I-3 to cover BOTH binaries |
| fixtures | the ingest adversarial fixtures (tampered/CID-mismatch/unsigned) + the real-`z6Mk...` DID-doc fixture (D-D38; `regenerate-ingest-fixtures` extends slice-03 D-D15); the recorded `plc.directory` + `bsky.social` contract fixtures (DEVOPS one-time recording, DELIVER consumes) |
| renderer-review | one slice-05 line (D-D41) |

Full CI/release delta: `docs/feature/openlore-appview-search/devops/ci-cd-pipeline.md`.

## Wave: DEVOPS / [REF] Upstream Issues

Non-blocking observations flagged back (none block DELIVER):

1. **The slice-01 `deny.toml` `axum` ban premise is slice-05-obsolete.** The ban
   rationale ("OpenLore is a CLI; we never run an HTTP server in-process") no longer
   holds — the indexer IS a network service serving HTTP (ADR-023/027). The ban must be
   narrowed (D-D42) before the `axum`-path indexer build is green. This crosses a
   slice-01 supply-chain decision (ADR-012's `deny.toml` policy). Resolution: D-D42
   narrows the ban (remove `axum` from `[bans].deny`; rely on the structural
   `xtask check-arch` CLI-must-not-link-server rule — a stronger guarantee). NON-BLOCKING
   (DESIGN justified `axum`; the edit is mechanical; the hand-rolled `hyper` fallback
   needs no edit, Q-DELIVER-AV-2) but flagged so the supply-chain decision is explicit.

2. **ADR-010's telemetry-endpoint revisit trigger named "a future sibling-feature DEVOPS
   wave" and slice-04's handoff expected slice-05 to stand it up — slice-05 does NOT.**
   The indexer's `indexer.ingest.*` events are operator-side SERVICE logs (the operator
   runs the indexer + reads its own log), DISTINCT from the author-side opt-in cohort
   telemetry endpoint (ADR-010, off by default, no endpoint). The cohort-aggregation
   YELLOWs from slices 01-04 (KPI-3/6, KPI-FED-3/5, KPI-SCR-1/5, KPI-GRAPH-1/5/6 cohort)
   PLUS the new KPI-AV-1/4/5/6 cohort REMAIN deferred to a future endpoint. Recorded so
   the slice-04 forward-expectation is corrected. NON-BLOCKING.

3. **The KPI-AV-1 north-star (≥60% unfollowed-author discovery) is coupled to index
   COVERAGE, which depends on what a single self-hosted indexer ingested** (a
   DISCUSS-acknowledged risk). DEVOPS mitigates with the index-coverage/freshness
   dashboard (`openlore-indexer stats`), but cannot GUARANTEE coverage at the walking
   skeleton (seed-set + relay config dependent). The KPI-AV-1 < 20% disprover is a
   coverage/UX re-investigation trigger, not a release gate. Recorded so the
   coverage↔north-star coupling is explicit to the PO at day-30. NON-BLOCKING.

(These complement the DESIGN-wave Upstream Issues above; items 1-3 are DEVOPS-specific.)

## Wave: DEVOPS / [REF] Handoff (DEVOPS → DELIVER / DISTILL / operator)

- **To DELIVER (nw-functional-software-crafter per ADR-007)**: the bootstrap (new
  crates + the `openlore-indexer` binary); the `ci.yml`/`nightly.yml`/`release.yml`/
  `deny.toml`/`xtask` extensions (above); the ingest adversarial + real-`z6Mk...`
  fixtures; the recorded `plc.directory`/`bsky.social` contract fixtures; the
  `search.*`/`indexer.*` tracing events + the runtime guardrail counter + the 4 adapter
  probes + the capability-boundary probe; `scripts/kpi-av-*.jq` + `scripts/indexer-coverage.jq`;
  the slice-05 renderer-review line. The Q-DELIVER-AV set + the open-questions list:
  `devops/wave-decisions.md`.
- **To DISTILL (nw-acceptance-designer; PARALLEL)**: the DEVOPS-defined event shapes +
  the two contract-boundary shapes + the hermetic fixtures the 8 release/acceptance
  gates consume. (DEVOPS ran PARALLEL with DISTILL, reading DESIGN; DISTILL reads DESIGN
  + this DEVOPS doc.)
- **To the indexer OPERATOR (POST-DELIVER)**: `openlore-indexer stats` (the
  index-coverage/freshness dashboard) + the indexer log — the FIRST operator surface in
  the product (a single self-hosted dogfood operator; no fleet, no on-call). Watch the
  coverage/freshness (the KPI-AV-1 sparsity diagnosis) + the verified/rejected ratio
  (the KPI-AV-3 health).

**DEVOPS handoff-ready: YES.** D-D35..D-D43 LOCKED; no new DEVOPS ADR (D-D43); the new
deployable + the environment matrix + the KPI-AV instrumentation + the two contract
tests + the CI/release/nightly/xtask/`deny.toml` extensions all specified; three
non-blocking Upstream Issues flagged. No blockers for DELIVER.

---

## Wave: DISTILL / [REF] Overview

DISTILL authored the EXECUTABLE acceptance specification for slice-05 (37 scenarios
across 3 files) that DELIVER drives Outside-In TDD against. The `.feature`/`.rs`
files are the scenario SSOT; this section is the structured pointer. Full
human-readable map: `distill/acceptance-tests.md`; decision log:
`distill/wave-decisions.md` (DD-AV-1..14); full grid: `distill/traceability.md`.

- **Language**: Rust (`[lang-mode] rust`); framework Rust std `#[test]` + `proptest`
  (`@property`); state-delta port inherited (`tests/common/state_delta.rs`,
  `[port-mode] inherit`).
- **Reconciliation**: PASSED — 0 contradictions (DISCUSS WD-100..110 ↔ DESIGN
  WD-111..124 ↔ DEVOPS D-D35..D-D43); all four `# DISTILL: confirm` flags RESOLVED.
- **Project Infrastructure Policy**: BOOTSTRAPPED this wave (was absent;
  `docs/architecture/atdd-infrastructure-policy.md`, cumulative slice-01..05).

## Wave: DISTILL / [REF] Scenario list with tags

37 scenarios; 8 release gates; 2 `@walking_skeleton` beats (AV-1 ingest, AV-8
search). Full table: `distill/acceptance-tests.md §4`.

| File | Layer | Scenarios | `@property` | Stories |
|---|---|---|---|---|
| `tests/acceptance/indexer_ingest.rs` | 3 (subprocess, `openlore-indexer`) | AV-1..AV-7 (7) | 0 | US-AV-001 |
| `tests/acceptance/appview_search.rs` | 3 (subprocess, `openlore` CLI + real serve) | AV-8..AV-29 (22) | 0 | US-AV-002..006 |
| `tests/acceptance/appview_core.rs` | 2 (pure `appview-domain`) | AVC-1..AVC-8 (8) | 5 (AVC-1/2/3a/3b/4) | US-AV-001/002/004 |

Cardinal release gates (named, load-bearing): `indexer_rejects_unverified_claim`
(AV-3+AVC-1, **KPI-AV-3**), `network_result_preserves_attribution` (AV-9+AVC-2,
**KPI-AV-2**), `local_first_preserved` (AV-13, **KPI-5**), `public_data_banner_shown`
(AV-10, KPI-AV-5), `verified_marker_is_universal` (AV-11+AVC-7, I-AV-1),
`search_succeeds_with_indexer_localhost` (AV-14, B1/WD-115), + the verify-before-index
+ anti-merging pure-core properties (AVC-1/AVC-2). Plus the behavioral funnel/share/
counter gates (AV-19 KPI-AV-4, AV-26/AV-28 KPI-AV-6, AV-25+AVC-6 OD-AV-7).

## Wave: DISTILL / [REF] WS strategy

Per the Architecture of Reference (port-class → treatment), NOT a per-feature A/B/C/D
choice. Driving (both binaries) = REAL subprocess; driven-internal (`IndexStorePort`
`index.duckdb` + the B1 serve/client transport) = REAL; driven-external/
non-deterministic (`IngestSourcePort` network ingest + `IdentityResolvePort` PLC
resolution) = FAKE (`FakeIngestSource` + fixture real-`z6Mk` resolver). The funnel
reuses the slice-03 `PeerPds` verbatim. Two `@walking_skeleton @real-io @driving_port`
beats (AV-1, AV-8) close the trustworthy-network-discovery loop end-to-end through the
production composition roots.

## Wave: DISTILL / [REF] Adapter coverage table

| Driven adapter | Real-I/O scenario? | Treatment |
|---|---|---|
| `adapter-index-store` (`IndexStorePort`, `index.duckdb`) | YES — AV-1/2/3 (write) + AV-8.. (read via serve) | REAL DuckDB |
| `adapter-xrpc-query-server` + `adapter-index-query` (B1) | YES — AV-14 + every AV-8.. query | REAL localhost serve + REAL client |
| `adapter-atproto-ingest` (`IngestSourcePort`) | AV-1..AV-7 (adversarial set in AV-3) | FAKE (`FakeIngestSource`; input-validating) |
| `adapter-atproto-did` (`IdentityResolvePort`, verify-only) | AV-4 runs the REAL decode | FAKE resolver carrying a REAL `z6Mk` |
| `claim-domain` verify+cid+decode (pure) | AV-1/3/4 + AVC-1/4 | REAL (reused; no 2nd path) |
| `appview-domain` (pure core; no probe) | AVC-1..8 + AV-1.. + AV-8.. | REAL (layer-2 `@property` is its Earned-Trust analog) |
| `adapter-duckdb` (user's `openlore.duckdb`) | AV-5 (indexer never touches) + AV-13/19 (CLI authoring) | REAL |
| slice-03 `PeerPds` (funnel seed) | AV-19/22 | REUSED verbatim |

The four NEW adapters' `probe()` bodies (substrate-lie checks) are DELIVER's
adapter-integration deliverable below the driving-port boundary (DESIGN §6.3) —
except the user-visible startup REFUSAL (AV-6) + the gold real-decode path (AV-4).

## Wave: DISTILL / [REF] Scaffolds

3 RED-ready scaffold test files (`// SCAFFOLD: true`; all `#[test]` bodies
`todo!("DELIVER (slice-05): ...")`):
`tests/acceptance/{indexer_ingest,appview_search,appview_core}.rs`. Detection:
`grep -r "SCAFFOLD: true" tests/`. The production crates they import
(`appview-domain` + 4 ports + 4 effect crates + `openlore-indexer` binary + the cli
`search` dispatch + `claim-domain::decode_ed25519_multibase`) + the test-support
harness (`support/mod.rs` slice-05 additions + `crates/test-support/src/fixtures_ingest.rs`)
+ the 3 `[[test]]` registrations are DELIVER's bootstrap step (DD-AV-13/14) — until
then `cargo build --tests` fails on missing imports (BROKEN), which is why the
`[[test]]` registrations are deferred (registering before imports exist = BROKEN, not
RED). After bootstrap: all `#[test]`s reach `todo!()` → RED per Mandate 7.

## Wave: DISTILL / [REF] Test placement

FLAT `tests/acceptance/` (slice-01..04 precedent; `cargo test --test <file>`
ergonomics). Three new files split by concern + driving port: `indexer_ingest.rs`
(the `openlore-indexer` binary, the second composition root), `appview_search.rs`
(the `openlore` CLI `search` verb), `appview_core.rs` (the pure `appview-domain`
core, layer 2). Symmetric with slice-04's `graph_query_explore.rs` + `scoring_core.rs`.

## Wave: DISTILL / [REF] Driving Adapter coverage

Two driving adapters (two binaries). `openlore search` (`--object`/`--contributor`/
`--subject`/`--show`/`--share` + the banner + the `[verified]` marker + the `peer add`
affordance) — every flag covered by ≥1 subprocess scenario (AV-8..AV-29).
`openlore-indexer` (`ingest`/`serve` + the capability boundary + `--help` verb-set) —
covered by AV-1..AV-7 + AV-14. Zero uncovered NEW verb/flag/subcommand. The B1
CLI↔indexer XRPC boundary is exercised end-to-end against a REAL `openlore-indexer
serve` over localhost (AV-14), not an in-process stub. Full table:
`distill/acceptance-tests.md §5`.

## Wave: DISTILL / [REF] Pre-requisites

DESIGN driving ports: the `openlore search` verb (ADR-027) + the `openlore-indexer`
binary (ADR-023). DEVOPS environment matrix: clean | with-pre-commit |
with-stale-config | indexer-in-container (the fsync-lie → AV-6) | localhost-transport
(the B1 serve → AV-14). DEVOPS event shapes + the two contract-boundary shapes (B1
per-result `author_did`; B2 `listRecords` + PLC `z6Mk` DID-doc) + the hermetic
fixtures (`FakeIngestSource` + the real-`z6Mk` DID-doc) the 8 gates consume.
DELIVER's first step (the indexer-subsystem bootstrap) is the compilation
pre-requisite (DD-AV-13). Full list: `distill/acceptance-tests.md §12`.

## Wave: DISTILL / [REF] Handoff (DISTILL → DELIVER)

- **Receives from DESIGN/DISCUSS/DEVOPS**: the 8 release gates, the 4 resolved
  `# DISTILL: confirm` flags, the I-AV-1..9 invariants, the DEVOPS event shapes +
  contract boundaries + hermetic fixture names.
- **Hands to DELIVER**: 37 RED-ready acceptance scenarios (3 files); the
  walking-skeleton beats (AV-1, AV-8) + the one-at-a-time release sequence (R1:
  AV-1/3/8/9/10/11/13/14/23 + AVC-1/2/5/7; R2: AV-15..22; R3: AV-26..29); the
  bootstrap pre-requisite (DD-AV-13/14); the state-delta universe per load-bearing
  scenario (DD-AV-10); the Tier B deferral (DD-AV-9, Open Item 9); mandate-compliance
  evidence (CM-A..H, `acceptance-tests.md §10`); the bootstrapped Infrastructure
  Policy. **DISTILL handoff-ready: YES**, conditional on DELIVER's bootstrap step
  landing before the suite runs the first time (the Pre-DELIVER fail-for-right-reason
  gate fires then, DD-AV-13).
- **Upstream Issues**: NONE new. The DESIGN-recorded OD-AV-6 numbering ambiguity
  (share resolver + pubkey decode share the label; both resolved WD-122 + WD-118) is
  inherited as a non-blocking observation, not relitigated.
