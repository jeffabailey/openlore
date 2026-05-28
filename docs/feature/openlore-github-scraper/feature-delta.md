# Feature Delta: openlore-github-scraper

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: Cross-cutting (CLI + ports + two new crates + GitHub adapter)
> Walking skeleton: Yes (this sibling IS the walking skeleton for the scraper slice)
> Research depth: Comprehensive (the human-gate + no-surveillance invariants are load-bearing)
> JTBD: mandatory (every story carries `job_id` -> `docs/product/jobs.yaml`)
> Inherits from: `docs/feature/openlore-foundation/feature-delta.md` (WD-9..WD-13, ADR-001..012) and `docs/feature/openlore-federated-read/feature-delta.md` (WD-14..WD-25, ADR-013..016)
> Date: 2026-05-28
> Owner: Luna (nw-product-owner)

This file is the canonical DISCUSS-wave delta for `openlore-github-scraper`,
the third sibling feature shipped in the OpenLore umbrella (slice-02 in the
numbering; sequenced AFTER slice-03 federation per WD-13). Tier-1 content is
inlined under `## Wave: DISCUSS / [REF] <Section>` headings; SSOT content lives
under `docs/product/`; per-journey artifacts under
`docs/feature/openlore-github-scraper/discuss/`.

---

## Wave: DISCUSS / [REF] Wave Decisions

| # | Decision | Rationale | Status |
|---|---|---|---|
| WD-46 | Slice-02 ships in a SIBLING feature `openlore-github-scraper` (this feature) per the carpaccio split locked by WD-9. Slice-02 IS the walking skeleton for this feature (one slice = one feature). | Inherits WD-9. Sibling-feature pattern keeps each slice independently shippable. Sequenced after slice-03 per WD-13 (federation before scrapers). | LOCKED |
| WD-47 | Persona priority for slice-02: **P-002 Researcher / Tech Lead (contributor-evaluator hat) = primary**; **P-001 Senior Engineer Solo Builder = secondary** (evaluates a dependency's maintainers via the same hat). | Slice-02's load-bearing job (J-004) is a contributor/repo-evaluation job; P-002 is the natural evaluator. P-001 wears the same evaluator hat when vetting a dependency. | LOCKED |
| WD-48 | Job priority for slice-02: **J-004 is the walking-skeleton job for this feature** (opportunity score raised 11->13, promoted to underserved-primary-for-slice in jobs.yaml). Three sub-jobs are addressed: J-004a (harvest public signals — load-bearing), J-004b (derive editable candidates — load-bearing), J-004c (human always signs — load-bearing). J-004 bridges into J-001 (authoring). | J-004 was the umbrella tertiary; the scraper is its cost-lowering lever and the right walking-skeleton job for this sibling per the brief. | LOCKED |
| WD-49 | **Human-gate invariant (load-bearing).** The scraper PROPOSES candidate claims; the human SIGNS via the EXISTING slice-01 compose-sign-publish pipeline. No candidate is ever signed, persisted as a claim, or published automatically. The scraper has no signing key and no publish code path. | Preserves the slice-01 "claims are signed human assertions" invariant. This is the single most load-bearing decision of the slice; violating it collapses the trust model. | LOCKED, enforced by US-SCR-002/003 AC + KPI-SCR-2 |
| WD-50 | **Verb shape is the sugar verb `openlore scrape github <target> [--sign N[,N...]]`**, NOT a flag on `claim add` (rejected: `claim add --from-github`). | Symmetric with the slice-03 sugar verbs (`peer pull`, `claim counter`); discoverable; keeps `claim add` focused on hand-authoring. The `--sign` continuation reuses the slice-01 pipeline. Requires an ADR-003/ADR-013 verb-contract amendment as a DESIGN deliverable. Full rejected-alternatives in `discuss/alternatives-considered.md`. | LOCKED |
| WD-51 | **Public-data-only (no surveillance).** The scraper reads ONLY public GitHub data. The target is the SUBJECT of a possible claim, never a controller. Private/non-existent/inaccessible targets are refused with a "scraper only reads public data" message. | Resolves the J-004 anxiety ("will this become a surveillance / blacklist tool?"). Counter-claim + retraction remain first-class (inherited). | LOCKED, enforced by US-SCR-001 AC + KPI-SCR-4 |
| WD-52 | **Candidate confidence defaults to 0.25 (speculative bucket per WD-10); only the human may raise it.** No candidate is proposed above 0.3. Confidence is numeric-only in the signed payload (inherited WD-10). | The scraper has weak evidence (single public signal). A conservative default forces the human to consciously raise confidence rather than the tool over-asserting. Numeric-only persistence is inherited (I-6). | LOCKED, enforced by US-SCR-002 AC + acceptance test `candidate_confidence_no_autoinflate` |
| WD-53 | **Small, auditable signal->predicate mapping** lives in `docs/product/jobs.yaml :: J-004.signal_predicate_mapping` (the SSOT, 5 entries in slice-02). Every candidate names the exact signal that produced it. The human edits any candidate before signing. | Keeps derivation auditable and rejectable. NO ML inference (would make candidates unauditable). `scraper-domain` consumes the SSOT, never hardcodes a divergent copy. | LOCKED, enforced by US-SCR-002 AC + KPI-SCR-3 |
| WD-54 | **Optional PAT auth.** The scraper works unauthenticated for small targets (anonymous rate limit) and uses an optional Personal Access Token via `GITHUB_TOKEN` (env, or config — DESIGN's call) for higher limits / larger / contributor targets. The token is never logged, never written to a claim, never published. | Unauthenticated is the zero-friction default; the optional PAT is what makes the scraper usable on REAL evaluation targets. Token is an effect-shell credential only. | LOCKED, enforced by US-SCR-004 AC |
| WD-55 | **Output = rendered reviewable candidate list -> user selects -> flows into the slice-01 claim add/sign pipeline. Nothing is persisted unsigned.** Running `scrape github` WITHOUT `--sign` writes zero rows to `author_claims` and makes zero PDS writes. | Reinforces WD-49 at the storage layer. Candidates are in-memory ADTs (`scraper-domain` is pure); they materialize as claims only by the human signing. | LOCKED, enforced by US-SCR-003 AC + acceptance test `scraper_never_persists_unsigned` |
| WD-56 | **Pure / effect split (ADR-007).** `scraper-domain` is PURE (derives candidates from already-fetched data; no I/O). `adapter-github` is the EFFECT shell (GitHub REST/GraphQL over HTTPS behind a new `GithubPort`); it ships a `probe()` per ADR-009 I-4 with the 250ms budget (I-5). | Mirrors the slice-01/03 hexagonal discipline. The pure derivation is trivially unit/mutation-testable; the I/O is isolated and probeable. | LOCKED, enforced by US-SCR-006 AC + `xtask check-arch`/`check-probes` |
| WD-57 | **Two new production crates: `adapter-github` + `scraper-domain`** (per the brief). This is the FIRST slice to add crates since slice-01 (slice-03 added zero per WD-26). The brief's Component Inventory gains two rows at finalize. | The brief explicitly scopes slice-02 to "adds `adapter-github` + `scraper-domain`." No way to honor the pure/effect split (WD-56) without the two crates. | LOCKED |
| WD-58 | **`derived-from` provenance is informational and MUST NOT alter confidence or federation behavior.** Whether it lives in the signed payload (as an OPTIONAL, CID-stable-when-absent field per ADR-005, mirroring the slice-03 `reason` field WD-32/ADR-015) or stays display-only is a DESIGN call. | A reader should be able to see a claim originated from a scraper run, but provenance must never change the claim's semantics or wire stability. Optionality + CID-stability-when-absent preserves forward compatibility with slice-01/03 readers. | LOCKED (product contract); DESIGN owns the storage choice |

### Scope Assessment

`## Scope Assessment: PASS — 6 user stories (5 user-visible + 1 infra), 1 cohesive bounded context (GitHub harvest -> candidate-claim derivation -> hand to slice-01 sign path), estimated ~10 days. Single slice = single feature; no further sub-slicing recommended.`

Carpaccio gate evaluation (5 taste tests):

- **Stories**: 6 (within <=10 threshold). PASS.
- **Bounded contexts**: 1 (GitHub harvest -> candidate derivation -> slice-01 sign hand-off; a single coherent surface). PASS.
- **Walking-skeleton integration points**: 3 (GitHub API harvest, pure derivation, slice-01 compose-sign-publish reuse). The third is a REUSE, not a new integration. Within the <=5 threshold. PASS.
- **Estimated effort**: ~10 days (within <=2 weeks threshold). PASS.
- **Multiple independent outcomes**: NO — all 6 stories serve J-004 and its sub-jobs; auth (US-SCR-004) and batch-sign (US-SCR-005) are efficiency/reach enablers of the same scrape->propose->sign outcome, not independent outcomes. PASS.
- **Verdict**: RIGHT-SIZED. Single slice = single sibling feature. GitHub-only; multi-source (Mastodon/blogs) explicitly deferred (story-map "What is NOT in scope").

### Risks logged

- KPI-SCR-1 (cost-to-first-claim under 2 minutes) is the slice's load-bearing behavioral hypothesis. Mitigation: instrumentation via `scrape.to_sign.duration_seconds` histogram (handed off to DEVOPS).
- The signal->predicate mapping is a product judgment call made in auto-mode without user validation interviews. Mitigation: KPI-SCR-5 (edit rate) surfaces whether users systematically disagree with the default mapping within 30 days; the mapping is small and trivially revisable in `jobs.yaml`.
- GitHub API contract drift (rate-limit shapes, GraphQL schema changes) could break harvest silently. Mitigation: a CI contract test for the public GitHub endpoints `adapter-github` calls (handed off to DEVOPS + DISTILL).
- The "harvest a user/contributor target" path (US-SCR-001 Example 2) is bounded in slice-02; deep cross-repo triangulation is deferred to slice-04. Risk: users expect richer contributor profiles. Mitigation: the candidate-list footer and story-map "out of scope" table set the expectation explicitly.
- DISCOVER + DIVERGE skipped (same as slice-01/03). The four-forces analysis for J-004 was performed in this DISCUSS without prior validation interviews. Mitigation: KPI-SCR-1 + KPI-SCR-5 + day-30 study will surface mis-prioritization within 30 days of release.

---

## Wave: DISCUSS / [REF] JTBD Analysis Summary

Full analysis in `docs/product/jobs.yaml`. Summary for slice-02:

| Job | Name | Priority for slice-02 | Opportunity Score | In slice-02? |
|---|---|---|---|---|
| J-004 | Evaluate a contributor's body of work through a philosophy lens | primary (walking-skeleton for this feature) | 13 (raised from 11; underserved-primary-for-slice) | yes — all 6 stories |
| J-004a (sub-job of J-004) | Harvest a contributor's/repo's public GitHub signals | LOAD-BEARING | n/a (sub-job) | yes — US-SCR-001, US-SCR-004 |
| J-004b (sub-job of J-004) | Derive editable candidate claims from signals | LOAD-BEARING | n/a (sub-job) | yes — US-SCR-002 |
| J-004c (sub-job of J-004) | The human always signs — the scraper never asserts | LOAD-BEARING | n/a (sub-job) | yes — US-SCR-003, US-SCR-005 |
| J-001 | Author a signed philosophical claim | inherited (bridged) | 16 | partial — US-SCR-003/005 reuse the compose-sign-publish pipeline |
| J-002 | Explore the philosophy graph to inform a decision | inherited | 14 | partial — signed-from-scraper claims become queryable via slice-01 graph query |

J-004 was extended during this DISCUSS with three load-bearing sub-jobs
(harvest, derive, human-signs), a +2 opportunity-score bump, two cost-lowering
success signals, and the small auditable signal->predicate default mapping.

---

## Wave: DISCUSS / [REF] Journey Artifacts

One journey to map (scrape-propose-sign is the single coherent surface):

- Visual journey (scrape -> propose -> sign): `docs/feature/openlore-github-scraper/discuss/journey-scrape-propose-sign-visual.md`
- Structured schema (with embedded Gherkin per step): `docs/product/journeys/scrape-propose-sign.yaml`
- Shared artifacts registry: `docs/feature/openlore-github-scraper/discuss/shared-artifacts-registry.md`

Emotional arc:

- Scrape-propose-sign journey: **skeptical-to-confident-authorship (with a hard human-gate buffer)** — entry Curious-but-skeptical (surveillance? auto-publish?) through Curious-and-reassured (public-data banner) and In-control (candidates proposed, nothing signed) to Authoring (this is MY reasoning) and Confident-authorship at sign/publish. Half the sessions legitimately end at the candidate-review step (step 2); that is a valid exit, not a failure.

The human-gate guarantee (J-004c) is a CROSS-CUTTING invariant elevated to its
own section in the visual journey. It is enforced at harvest (public data only),
at derivation (candidates are in-memory proposals, never persisted/signed), at
sign (the slice-01 pipeline + human signing gesture is the ONLY path to a
claim), and at test time (`scraper_never_persists_unsigned`).

---

## Wave: DISCUSS / [REF] Story Map and Slicing

- Story map: `docs/feature/openlore-github-scraper/discuss/story-map.md`

Slicing summary:

- **Release 1 (walking skeleton)**: US-SCR-001 + US-SCR-002 + US-SCR-003 + US-SCR-006. Validates the scrape->propose->sign loop end-to-end.
- **Release 2 (authenticated / real-target reach)**: US-SCR-004. Makes the scraper usable on busy contributors / large repos.
- **Release 3 (batch candidate signing)**: US-SCR-005. Multi-claim efficiency on the already-validated single-sign flow.

Priority order is set by outcome impact and risk-of-failure consequence
(Release 1 fails = cost-lowering thesis dead AND human-gate is the riskiest
assumption; Release 3 fails = survivable efficiency defect). Rationale in
story-map.md `## Priority Rationale` section.

All 5 carpaccio taste tests evaluated for this slice (in the Scope Assessment
above): right-sized in stories, contexts, integration points, effort, and
outcome coherence. Verdict: SINGLE SLICE = SINGLE FEATURE; no further
sub-slicing.

---

## Wave: DISCUSS / [REF] User Stories and Acceptance Criteria

All in `docs/feature/openlore-github-scraper/discuss/user-stories.md`:

| Story | Title | Job link | Elevator Pitch | DoR status |
|---|---|---|---|---|
| US-SCR-001 | Harvest a public GitHub target's signals | J-004 | yes | PASS (see DoR section) |
| US-SCR-002 | Derive auditable candidate claims from signals | J-004 | yes | PASS |
| US-SCR-003 | Review, edit, and sign a candidate via slice-01 | J-004 + J-001 | yes | PASS |
| US-SCR-004 | Use an optional PAT for higher rate limits | J-004 | yes | PASS |
| US-SCR-005 | Select and sign several candidates in one pass | J-004 | yes | PASS |
| US-SCR-006 | Bootstrap GithubPort + adapter-github + scraper-domain (`@infrastructure`) | `infrastructure-only` | n/a — @infrastructure | PASS |

Slice composition gate: PASS — 5 user-visible stories + 1 infrastructure story;
slice is NOT 100% `@infrastructure` (per `nw-po-review-dimensions` Dimension 0 §5).

---

## Wave: DISCUSS / [REF] Outcome KPIs

Full table in `docs/feature/openlore-github-scraper/discuss/outcome-kpis.md`.
North star:

> **KPI-SCR-1**: Contributor-evaluator users produce an evidence-backed signed
> claim about a target by reviewing scraper candidates in **under 2 minutes**
> from `scrape github` to signed claim (including predicate-vocabulary discovery,
> which the candidate list removes).

Guardrails: KPI-SCR-2 (human-gate: zero unsigned persistence / auto-publish) and
KPI-SCR-4 (public-data-only: zero private endpoint calls). Both MUST hold; any
failure is unshippable.

Leading indicators: KPI-SCR-3 (auditability — every candidate names its source
signal) and KPI-SCR-5 (edit rate >=50% — proves the human-in-the-loop is real).

---

## Wave: DISCUSS / [REF] Definition of Ready validation

| DoR item | US-SCR-001 | US-SCR-002 | US-SCR-003 | US-SCR-004 | US-SCR-005 | US-SCR-006 |
|---|---|---|---|---|---|---|
| 1. Problem statement clear, domain language | PASS | PASS | PASS | PASS | PASS | PASS (infra rationale) |
| 2. Persona with specific characteristics | PASS (P-002) | PASS (P-002+P-001) | PASS (P-002+P-001) | PASS (P-002+P-001) | PASS (P-002+P-001) | n/a (infra) |
| 3. >=3 domain examples with real data | PASS (4) | PASS (4) | PASS (5) | PASS (4) | PASS (4) | PASS (2 — within range for narrow infra surface) |
| 4. UAT in Given/When/Then (3-7) | PASS (4) | PASS (4) | PASS (5) | PASS (4) | PASS (4) | PASS (2 — within range for narrow infra surface) |
| 5. AC derived from UAT | PASS | PASS | PASS | PASS | PASS | PASS |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS (1.5d, 4) | PASS (2d, 4) | PASS (2d, 5) | PASS (1.5d, 4) | PASS (1d, 4) | PASS (2d, 2) |
| 7. Technical notes: constraints/dependencies | PASS | PASS | PASS | PASS | PASS | PASS |
| 8. Dependencies resolved or tracked | PASS (depends US-SCR-006) | PASS (US-SCR-001, US-SCR-006) | PASS (US-SCR-002, US-SCR-006) | PASS (US-SCR-001, US-SCR-006) | PASS (US-SCR-003) | PASS (slice-01 crates) |
| 9. Outcome KPIs defined with measurable targets | PASS (KPI-SCR-1, 4) | PASS (KPI-SCR-3, 1) | PASS (KPI-SCR-1, 2, 5) | PASS (KPI-SCR-1) | PASS (KPI-SCR-1, 2) | n/a — supports KPI-SCR-1..4 |

**Overall DoR status: PASSED** for all stories.

Notes:
- Item 3 + Item 4 (US-SCR-006): the spec allows 3-7 scenarios; US-SCR-006 ships 2 composite scenarios because the infrastructure surface is narrow and additional scenarios would be padding. Same pattern as US-005 (slice-01) and US-FED-006 (slice-03). Flagged for reviewer judgment but considered PASS.
- Item 2 (US-SCR-006): infrastructure-only stories do not require a persona; `infrastructure_rationale` present per Decision 1.

### Elevator Pitch verification (BLOCKING per Dimension 0)

Per `nw-po-review-dimensions` Dimension 0 (checked first, BLOCKING):

| Story | Section present? | Real entry point? | Concrete output? | Job connection? | Verdict |
|---|---|---|---|---|---|
| US-SCR-001 | YES (Before/After/Decision enabled) | YES (`openlore scrape github rust-lang/cargo`) | YES (banner + "Harvested 5 public signals in 2.1s") | YES (decide in seconds whether a target has claimable signals) | PASS |
| US-SCR-002 | YES | YES (`openlore scrape github rust-lang/cargo` — candidate list rendered) | YES ("Candidate claims for subject github:rust-lang/cargo (5 derived — NOTHING is signed)" with per-candidate source signals) | YES (decide which proposed claims are worth pursuing by scanning a traceable list) | PASS |
| US-SCR-003 | YES | YES (`openlore scrape github rust-lang/cargo --sign 1`) | YES (slice-01 compose preview with "not as truth", confidence 0.55, derived-from line, publish at-uri) | YES (turn a proposal into MY signed claim, editing first) | PASS |
| US-SCR-004 | YES | YES (`GITHUB_TOKEN=... openlore scrape github torvalds`) | YES ("auth: authenticated (4982/5000 rate budget)" + completed harvest; unauthenticated remediation message) | YES (run the lens on REAL targets, not just toy repos) | PASS |
| US-SCR-005 | YES | YES (`openlore scrape github rust-lang/cargo --sign 1,3,4`) | YES (sequential compose previews + "(2 of 3 signed)" progress + "Published 3 claims") | YES (capture a whole evaluation session's claims in one pass without batch-signing) | PASS |
| US-SCR-006 | n/a (@infrastructure with rationale) | n/a | n/a | n/a (`infrastructure-only` per Decision 1) | PASS via rationale |

Slice-level Elevator Pitch check (Dimension 0 §5): the slice has 5 user-visible
stories + 1 infrastructure story. Slice is NOT 100% `@infrastructure`. PASS.

---

## Wave: DISCUSS / [REF] Locks inherited from openlore-foundation + openlore-federated-read

These are binding inputs to this feature's DESIGN wave. They are NOT relitigated
here; any change requires returning to the owning slice's product-owner review
first.

| ID | Inherited from | Carries into slice-02 as |
|---|---|---|
| WD-9 | openlore-foundation | Carpaccio split: each slice is an independent sibling feature. slice-02 is this feature. |
| WD-10 | openlore-foundation | Numeric `[0.0, 1.0]` is the only persisted confidence; display-only buckets. Scraper candidates default to 0.25 (speculative); the signed payload stores only the numeric value (I-6). |
| WD-11 | openlore-foundation | Retraction = counter-claim that references the original CID; soft-retract only. A signed-from-scraper claim is retractable/counter-claimable like any claim; no special path. |
| WD-12 | openlore-foundation | Identity = user's existing ATProto DID with per-application derived key. The scraper introduces NO new signing identity; the human signs with their existing key. |
| WD-13 | openlore-foundation | Sequence: federation (slice-03) before scrapers (slice-02). slice-03 has shipped; slice-02 is this deliverable. |
| ADR-003 | openlore-foundation | CLI verb contract. slice-02 EXTENDS it with `scrape github <target> [--sign ...]`. Requires an **ADR amendment** (next number after ADR-016) as a DESIGN deliverable, in the same spirit as the ADR-013 amendment slice-03 raised. |
| ADR-005 | openlore-foundation | Lexicon `org.openlore.*` stability; any added field must be optional. IF the `derived-from` provenance is stored in the signed payload, it is OPTIONAL and CID-stable when absent (mirrors slice-03 `reason`). |
| ADR-007 | openlore-foundation | Functional Rust paradigm. `scraper-domain` is PURE; `adapter-github` is the effect shell. |
| ADR-008 | openlore-foundation | Reference rules + `ReferenceType`. The scraper does NOT add a new ReferenceType; a scraped candidate is a plain claim (a user may later `claim counter` it via slice-03 if they wish). |
| ADR-009 | openlore-foundation | Hexagonal ports + adapters. The new `GithubPort` MUST ship a `probe()` (I-4) within the 250ms budget (I-5). |
| ADR-016 | openlore-federated-read | Peer DID resolution / XRPC reads (slice-03). NOT used by slice-02; listed so DESIGN knows the highest ADR number to date is 016 and the slice-02 verb amendment continues the sequence. |
| The literal "not as truth" framing | US-001 AC (foundation) | MUST appear in the compose preview reached from any scraper candidate (it is the SAME slice-01 preview). Content-frozen across slices (I-7). |
| Retract hint in publish success | US-003 AC (foundation) | A signed-from-scraper claim's publish success message mentions the retract command (I-8). |
| Single publish path (VerbClaimPublish internals) | ADR-003 + cli (foundation), reaffirmed WD-22 (slice-03) | Sign-from-scraper reuses this; no parallel publish code path. |

---

## Wave: DISCUSS / [REF] Ask-Intelligent Menu (lean mode, scoped to triggered items only)

Triggers evaluated; scoped expansion offered only for those that fired.

### Fired: cross-context complexity (>=3 contexts)

This slice spans CLI verbs + ports (new `GithubPort`) + two new crates
(`adapter-github` effect shell, `scraper-domain` pure core) + GitHub API
integration + reuse of the slice-01 compose-sign-publish pipeline. That is
>=3 contexts; the threshold fires.

- **Offer**: `alternatives-considered.md` — document the rejected alternatives for the three biggest choices (verb shape `scrape github` vs `claim add --from-github`; signal->predicate mapping as auditable-static vs ML-inferred; provenance in signed payload vs display-only).
- **Cost**: ~10 minutes; ~3 pages output.
- **Recommendation**: **accept**. These are the choices DESIGN will second-guess if not documented now (especially the auditable-static vs ML mapping choice, which is load-bearing for the no-surveillance and auditability promises).
- **Status**: **ACCEPTED (auto-mode)** 2026-05-28 — see `docs/feature/openlore-github-scraper/discuss/alternatives-considered.md`. (To be produced alongside DESIGN handoff; flagged as a DESIGN read.)

### Fired: AC ambiguity (the human-gate + no-surveillance semantics are easy to disagree on)

The human-gate (scraper proposes, human signs) and no-surveillance
(public-data-only) invariants are conceptually rich and the anxiety force is
load-bearing for J-004 ("will this become a surveillance tool / will it put
words in my mouth?"). The happy/edge/error scenarios in user-stories.md cover
the functional surface but not the anxiety-path force explicitly.

- **Offer**: `gherkin-scenarios-expanded.md` — add anxiety-path and habit-path scenarios per the JTBD-BDD integration template. Target: >=3 anxiety + >=2 habit.
- **Cost**: ~15 minutes; ~3 pages output.
- **Recommendation**: **accept**. The anxiety force (surveillance fear + assertion fear) is load-bearing for J-004; without dedicated scenarios DISTILL will have to invent them.
- **Status**: **ACCEPTED (auto-mode)** 2026-05-28 — to be produced as `docs/feature/openlore-github-scraper/discuss/gherkin-scenarios-expanded.md` (target: 3 anxiety covering surveillance-fear, assertion-fear, over-confidence-fear + 2 habit covering "I already scroll a GitHub profile" and "the compose preview is the slice-01 one I know"). Flagged as a DISTILL read.

### Fired: multi-stakeholder narrative (both personas active in this slice)

Slice-02 activates P-002 (contributor-evaluator hat) as primary AND extends
P-001 with the same evaluator hat. Both exercise the same verbs but from
different starting mental models (P-002: pragmatic team-tooling evaluator; P-001:
skeptical solo builder vetting a dependency's maintainers).

- **Offer**: extend `docs/product/personas/researcher-tech-lead.yaml` with a `contributor-evaluator` hat (typical session, anxieties, success signals, UX guardrails), mirroring the slice-03 `federation-reader` hat.
- **Cost**: ~5 minutes; ~1 page output.
- **Recommendation**: **accept**. Keeps the journey YAML solution-neutral without losing persona-specific guidance.
- **Status**: **ACCEPTED (auto-mode)** 2026-05-28 — to be added as a `contributor-evaluator` entry under the existing `hats:` section of `researcher-tech-lead.yaml`. Flagged as a DESIGN read.

### NOT fired: regulatory / compliance complexity

Slice-02 reads only PUBLIC GitHub data and the subject of a claim is a public
identifier (a GitHub handle / repo). No PII beyond what is publicly published on
GitHub. The no-surveillance invariant (WD-51) is the product-level mitigation.
GDPR/compliance concerns are inherent to publishing claims ABOUT people (a
slice-01-era concern, mitigated by counter-claim + retraction), not newly
introduced by reading public GitHub data. Re-evaluate at slice-04/05 when
aggregation + scoring widen the surface.

### NOT fired: integration density

Slice-02 adds 1 new external integration (the GitHub API) plus 1 pure derivation
and 1 REUSE of the slice-01 publish pipeline. Below the threshold.

### Menu action

Three fired offers were **accepted (auto-mode)** in this DISCUSS wave. Two of the
three artifacts (`alternatives-considered.md`, `gherkin-scenarios-expanded.md`)
are scoped to be produced alongside the DESIGN/DISTILL handoff and are flagged in
the read-lists below; the persona-hat extension is a small in-place edit to
`researcher-tech-lead.yaml`. (In strict interactive mode these would be offered
to the user; in auto-mode the recommended `accept` verdict is taken per the
auto-mode product-defaults instruction.)

Telemetry: each `expand` acceptance should ideally emit a
`DocumentationDensityEvent`. The helper does not yet exist (greenfield repo); the
events are recorded here for retroactive backfill.

| Trigger | Artifact | Should emit |
|---|---|---|
| `cross_context_complexity` | `alternatives-considered.md` | `DocumentationDensityEvent{ feature: openlore-github-scraper, wave: DISCUSS, expansion: alternatives-considered, accepted: true, ts: 2026-05-28 }` |
| `ac_ambiguity` | `gherkin-scenarios-expanded.md` | `DocumentationDensityEvent{ feature: openlore-github-scraper, wave: DISCUSS, expansion: gherkin-scenarios-expanded, accepted: true, ts: 2026-05-28 }` |
| `multi_stakeholder_narrative` | persona `contributor-evaluator` hat | `DocumentationDensityEvent{ feature: openlore-github-scraper, wave: DISCUSS, expansion: persona-hats, accepted: true, ts: 2026-05-28 }` |

---

## Wave: DISCUSS / [REF] Open Decisions for User

The decisions below are surfaced for user input. Auto-mode default verdicts are
noted (and locked as WDs above); the user may confirm or override.

| ID | Decision | Default verdict | Why it matters |
|---|---|---|---|
| OD-SCR-1 | Verb shape: sugar verb `scrape github <target>` (recommended) vs flag `claim add --from-github <target>`. | **Sugar verb** (`scrape github`) per WD-50 | Affects the ADR verb-contract amendment size and keeps `claim add` focused on hand-authoring. Locked as default; surface for override. |
| OD-SCR-2 | PAT config surface: env-var `GITHUB_TOKEN` only (minimum) vs env-var + config-file entry. | **Env-var only for slice-02** (config-file deferred to a later slice if a multi-account need emerges) per WD-54 | Minor; DESIGN can widen. Listed because DISTILL will ask how the token is provided in acceptance fixtures. |
| OD-SCR-3 | `derived-from` provenance: store in the signed payload (OPTIONAL, CID-stable-when-absent field) vs display-only. | **Display-only for slice-02** (recommended; avoids a Lexicon change this slice) per WD-58 | Affects whether US-SCR-006 ships a Lexicon field. Display-only is the smaller change; if DESIGN finds a strong federation reason to persist provenance, it can flip this with an ADR. |
| OD-SCR-4 | Contributor (user) target depth: bounded aggregate signals in slice-02 (recommended) vs deep cross-repo triangulation. | **Bounded aggregate in slice-02**; deep triangulation deferred to slice-04 (scoring-graph) | Sets user expectation for `scrape github <user>`. Deep triangulation is a scoring concern with its own JTBD. |

If the user has no objection, all four defaults LOCK on handoff to DESIGN
(OD-SCR-1 and the provenance/auth choices are already reflected in WD-50/54/58).

---

## Wave: DISCUSS / [REF] Handoff

### To DESIGN (nw-solution-architect)

- Read (explicit list — every file matters):
  - `feature-delta.md` (this file)
  - Everything in `docs/feature/openlore-github-scraper/discuss/`:
    - `user-stories.md`
    - `story-map.md`
    - `outcome-kpis.md`
    - `shared-artifacts-registry.md`
    - `journey-scrape-propose-sign-visual.md`
    - **`alternatives-considered.md`** (fired ask-intelligent expansion — to be produced)
    - **`gherkin-scenarios-expanded.md`** (fired ask-intelligent expansion — to be produced)
  - `docs/product/jobs.yaml` (J-004 deepened with sub-jobs J-004a/b/c and the signal->predicate mapping)
  - `docs/product/journeys/scrape-propose-sign.yaml`
  - `docs/product/personas/researcher-tech-lead.yaml` (to be extended with the contributor-evaluator hat)
  - Slice-01 + slice-03 lock context (do NOT relitigate; treat as inherited inputs):
    - `docs/feature/openlore-foundation/feature-delta.md` (especially WD-9..WD-13)
    - `docs/feature/openlore-foundation/design/architecture-design.md`
    - `docs/feature/openlore-federated-read/feature-delta.md` (especially WD-22 single-publish-path reuse, the ADR-013 verb-amendment precedent, and the WD-32/ADR-015 optional-field CID-stability precedent)
    - `docs/product/architecture/brief.md` (Component Inventory + cumulative CLI surface + invariants I-1..I-12)
    - ADR-003 (verb contract being amended), ADR-005, ADR-007, ADR-008, ADR-009

- Decide:
  - **Verb-contract amendment (next ADR number after ADR-016)**: add `scrape github <target> [--sign N[,N...]]`. Document verb grammar consistency (the sugar-verb pattern matches slice-03's `peer pull` / `claim counter` precedent).
  - **`GithubPort` surface**: method signatures for `resolve_target`, `harvest_repo`, `harvest_user`, plus the `probe()` per ADR-009 (fixture target + sentinel signal round-trip; 250ms budget).
  - **`scraper-domain` API**: `derive_candidates(signals, mapping) -> Vec<CandidateClaim>`; the `Signal` and `CandidateClaim` ADTs; the loader for the `jobs.yaml` signal->predicate mapping (embed-at-build vs read-at-runtime).
  - **`adapter-github`**: HTTP client choice (PREFER reusing the workspace client already pulled in by `adapter-atproto-pds` to avoid a new `cargo deny` surface — I-11); optional `GITHUB_TOKEN` handling; rate-limit detection + remediation messaging.
  - **`derived-from` provenance**: display-only (OD-SCR-3 default) vs an OPTIONAL signed-payload field (CID-stable when absent per ADR-005). If stored, define the serde shape and the lexicon conformance test.
  - **Candidate->compose pre-fill mapping**: how a `CandidateClaim` pre-fills the slice-01 `VerbClaimAdd` compose editor without forking the publish path.
  - **Component Inventory update**: two new rows (`adapter-github`, `scraper-domain`) for the brief at finalize; production crate count goes 8 -> 10.

- Constraints inherited from this DISCUSS (DO NOT relitigate without coming back to PO):
  - **WD-49**: human-gate — scraper proposes, human signs; no auto-sign, no auto-publish, no signing key in the scraper.
  - **WD-50**: verb shape is the `scrape github <target> [--sign ...]` sugar verb.
  - **WD-51**: public-data-only; private/non-existent targets refused.
  - **WD-52**: candidate confidence defaults to 0.25 (speculative); only the human raises it; numeric-only persistence.
  - **WD-53**: small auditable signal->predicate mapping (SSOT in jobs.yaml); no ML inference.
  - **WD-54**: optional PAT via `GITHUB_TOKEN`; works unauthenticated for small targets; token never logged/claimed/published.
  - **WD-55**: nothing persisted unsigned; `scrape` without `--sign` writes zero author_claims rows and zero PDS writes.
  - **WD-56**: pure/effect split — `scraper-domain` pure, `adapter-github` effect shell with `probe()`.
  - **WD-57**: two new production crates.
  - **WD-58**: provenance is informational and never alters confidence or federation; storage choice is DESIGN's (OD-SCR-3 default display-only).

### To DEVOPS (nw-platform-architect, parallel)

- Read: `outcome-kpis.md` (Handoff to DEVOPS section).
- Deliver:
  - Instrumentation plan for KPI-SCR-1..5 (especially the `scrape.to_sign.duration_seconds` histogram for KPI-SCR-1 and the `scrape.candidate.signed{fields_edited}` event for KPI-SCR-5).
  - **GitHub API contract test** in CI: assert `adapter-github` calls ONLY public GitHub endpoints (the KPI-SCR-4 no-surveillance guardrail) and round-trips a fixture target's signals stably. Extend the slice-01 Pact-style contract suite.
  - Dashboards for KPI-SCR-1 (P50/P95 of `scrape.to_sign.duration_seconds` per target-type bucket) and KPI-SCR-5 (edit-rate over a 30-day window).
  - Alerting on KPI-SCR-2 (human-gate) and KPI-SCR-4 (public-data-only) != 100% (release-blocking); informational alert on KPI-SCR-1 P95 > 4 minutes.
  - Verify the optional `GITHUB_TOKEN` path in CI uses a least-privilege fixture token (or a recorded fixture) and that the token never appears in logs.

### To DISTILL (nw-acceptance-designer)

- Read:
  - `docs/product/journeys/scrape-propose-sign.yaml` (embedded Gherkin per step)
  - `docs/feature/openlore-github-scraper/discuss/user-stories.md` (UAT scenarios per story)
  - `docs/feature/openlore-github-scraper/discuss/shared-artifacts-registry.md` (integration gates 1-5)
  - **`docs/feature/openlore-github-scraper/discuss/gherkin-scenarios-expanded.md`** (anxiety + habit scenarios; some will carry `# DISTILL: confirm` flags for verb-shape / provenance-storage resolution)
- Build executable acceptance tests including:
  - The five integration gates from the shared-artifacts registry: `scraper_never_persists_unsigned`, `candidate_names_source_signal`, `scraper_only_reads_public_data`, `candidate_confidence_no_autoinflate`, `scraper_reuses_slice01_publish_path`.
  - The harvest contract tests against a recorded/fixture GitHub target (DEVOPS provides the fixture).
  - The "no partial candidate list on rate-limit" behavior (US-SCR-004).
- The `# DISTILL: confirm` comments throughout `gherkin-scenarios-expanded.md` mark behaviors implied by the requirements but not yet locked (e.g. the exact skip gesture in batch sign, whether provenance is a signed field). Each must be resolved against DESIGN's final verb/flag/provenance decisions before building tests.

### Handoff-ready?

**YES.** All WD-46..WD-58 LOCKED in this DISCUSS; three ask-intelligent
expansions accepted (auto-mode) — `alternatives-considered.md` and
`gherkin-scenarios-expanded.md` scoped for production alongside the DESIGN/DISTILL
handoff, persona `contributor-evaluator` hat to be added in place; lean Tier-1
output stands. Four Open Decisions (OD-SCR-1..4) have auto-mode default verdicts
and may proceed unless the user overrides; none are blocking for DESIGN to start.

DESIGN + DEVOPS may proceed in parallel; DISTILL has the scenarios it needs.
