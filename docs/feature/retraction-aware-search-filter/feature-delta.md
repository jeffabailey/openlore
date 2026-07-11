<!-- markdownlint-disable MD024 -->
# Feature Delta: retraction-aware-search-filter

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (an explicit, opt-in FILTER on network search — CLI + read-only viewer)
> Walking skeleton: **No** — brownfield DELTA reusing the slice-05 `IndexQueryPort` +
>   `adapter-index-query` + `appview-domain` and the slice-08 viewer `/search`; no new mechanism.
> UX depth: **Comprehensive** (full emotional arc, error/empty paths, Gherkin, @property criteria)
> JTBD: YES — every story traces to **J-005** (`docs/product/jobs.yaml`); one new sub-job **J-005d** appended (not a new primary job)
> Brownfield DELTA on: `openlore-appview-search` (slice-05), `viewer-network-search` (slice-08)
> Date: 2026-07-11 · Owner: Luna (nw-product-owner)

This file is the canonical DISCUSS-wave delta for `retraction-aware-search-filter`: an
**explicit, opt-in, non-destructive** way to HIDE soft-retracted claims from a network
search result set — `openlore search … --hide-retracted` on the CLI, and an equivalent
`?hide_retracted=1` toggle on the read-only `/search` viewer surface.

Today, network search obeys invariant **I-AV-9 / OD-AV-7** — *"counter shown, not applied":*
a soft-retracted (or countered) public verified claim STAYS discoverable and is annotated,
NEVER silently filtered or down-weighted. This feature does not weaken that invariant. It
adds a **user-invoked** view control that hides soft-retracted claims *from the current view
only*, discloses exactly what it hid, and changes the default behavior by exactly nothing.
The reconciliation of "add a filter" with "never silently filter" is the cardinal Locked
Decision **D-1** below.

This is a DELTA. It REUSES the slice-05 verified-attributed search stack and the slice-08
viewer `/search` render; it adds exactly ONE new pure decision function
(`appview-domain` retraction predicate) plus a flag (CLI) and a GET-param toggle (viewer).
Zero new crates. Tier-1 content is inlined here (lean); SSOT lives under `docs/product/`;
per-slice briefs under `slices/`.

---

## Wave: DISCUSS / [REF] Persona ID

Two personas, one per surface (mirrors the slice-05 CLI / slice-08 viewer split):

- **P-002 Researcher / Tech Lead** ("Rachel") — the CLI discovery operator (slice-05
  framed P-002 as primary for the network-search job). She runs `openlore search` to
  survey standing reasoning about a philosophy or project before a decision. When some
  indexed claims have been **soft-retracted by their own authors**, they are noise for
  *this* survey — she wants to exclude them from the working view without losing the
  guarantee that nothing was hidden behind her back.
- **P-001 Senior Engineer Solo Builder** ("Maria") — the read-only `openlore ui` viewer
  operator (slices 06/07/08). She glances at network discovery in the browser and wants
  the same explicit hide control there, honoring every read-only / verified / attributed
  guardrail she already trusts.

UX guardrails inherited (both surfaces): read-only, never silently mutate or re-rank,
confidence rendered verbatim, and — new here — **never hide anything without an explicit
action and an honest disclosure of what was hidden**.

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-005**: *When I am orienting a decision around a philosophy or project but do NOT
> already know which developers to follow, I want to discover the signed claims that exist
> across the whole network — verified and attributed — so I can find well-evidenced
> reasoning and the people behind it.*

This feature realizes a **new refinement of J-005** captured as sub-job **J-005d** (appended
to `docs/product/jobs.yaml` this wave):

> **J-005d** — *Optionally hide soft-retracted claims from a discovery view.* When I am
> surveying network claims for a decision and some have been **soft-retracted by their own
> authors**, I want to explicitly hide the retracted ones from THIS view — reversibly and
> with full disclosure of what was hidden — so I can focus on standing reasoning without the
> system ever silently deciding for me what I do not see.

| Sub-job | Name | Stories |
|---|---|---|
| J-005d | Optionally hide soft-retracted claims from a discovery view (opt-in, non-destructive, honest) | US-RF-001 (CLI), US-RF-002 (viewer) |

No new primary job. J-005d is `load_bearing: false` — it is an optional view control layered
on the discovery corpus, not the core discovery itself.

### Four Forces (for J-005d, feeding the BDD scenarios below)

- **Push**: A survey of a well-claimed philosophy returns rows an author has *withdrawn*.
  Under I-AV-9 they still show (correctly — nothing vanishes silently), but for a focused
  decision they are noise the operator must mentally filter every time.
- **Pull**: One explicit flag/toggle collapses that noise for the current view, and tells
  her exactly how much it hid — so she can trust the shorter list is *filtered*, not
  *empty-by-nature*.
- **Anxiety**: *"If I let the tool hide things, has it become the silent aggregator this whole
  product exists to avoid? What did it drop that I should have seen?"* → Mitigation: opt-in
  (D-1), self-disclosing "N hidden" (D-4/I-RF-3), reversible per-invocation (D-7), and it
  hides ONLY author-withdrawn claims, never claims a third party merely disagrees with (D-3).
- **Habit**: Devs expect `grep -v`, `--exclude`, faceted "hide X" filters to be a *view*
  operation that never mutates the source and is trivially reversible. `--hide-retracted`
  must feel exactly that ordinary — and the default (no flag) must be byte-identical to today.

---

## Wave: DISCUSS / [REF] Locked Decisions

Full rationale in `discuss/wave-decisions.md` is intentionally NOT duplicated (lean); the
binding form is here. All decisions are D-numbered per the wave contract.

| # | Decision | Status |
|---|---|---|
| **D-1** | **I-AV-9 RECONCILIATION (cardinal).** The retraction filter is reconciled with "never silently filter" by three simultaneous constraints: **(a) opt-in** — the default view is byte-identical to today (retracted claims still shown + annotated); the filter activates ONLY on explicit `--hide-retracted` / `?hide_retracted=1`. **(b) non-destructive** — it hides from the current view only; it never mutates the index, re-verifies, re-orders survivors, or re-weights their scores. **(c) honest** — when active it discloses "N retracted claim(s) hidden". A user-invoked, disclosed, reversible filter is not *silent* filtering; I-AV-9 forbids the latter and is preserved in full. Formalized as new invariant I-RF-1..3. | LOCKED |
| **D-2** | **Pure predicate in `appview-domain`.** The filter is a pure total function over already-composed results (indicative name `retain_visible(result, hide_retracted: bool) -> bool` or `partition_retracted`). CLI and viewer only invoke it and count what it dropped. No effectful filtering, no index round-trip (ADR-007 functional core; extends the slice-05 `appview-domain` pure-core allowlist). | LOCKED |
| **D-3** | **Soft-retract ONLY; third-party counters stay shown.** `--hide-retracted` hides a claim iff its OWN author soft-retracted it (a retraction counter-claim by the same author DID referencing the original CID — RC-02 / WD-11). A claim that a *different* author merely countered/disagrees with is NOT hidden — it is a standing claim and remains shown + annotated (I-AV-9). This forbids a heckler's veto and preserves anti-merging (I-AV-2): a disagreement never removes an author's row. | LOCKED |
| **D-4** | **Honesty line mandatory when active.** When the filter is active AND hid ≥1 result, the surface MUST state the count ("N retracted claim(s) hidden") — CLI footer line; viewer results-region notice. When active but nothing matched, it MUST NOT print a misleading "0 hidden as if something happened" (it may stay silent or say "no retracted claims to hide"). A silent hide is a build-fail. | LOCKED |
| **D-5** | **Non-destructive ordering & scoring.** Survivors keep their original relative order and their original confidence/score verbatim. Hiding N rows NEVER re-ranks or re-weights the remainder (I-AV-9 "never down-weighted" carried in; @property-tested). | LOCKED |
| **D-6** | **Both surfaces, CLI-first; zero new crates.** Slice 1 = CLI `--hide-retracted` (extends the ADR-027 `openlore search` verb). Slice 2 = viewer `?hide_retracted=1` parity (extends slice-08 `/search`). Extends `appview-domain` + `cli` + `viewer-domain` + `adapter-http-viewer`; workspace stays 21 members; `check-arch` stays green. | LOCKED |
| **D-7** | **Reversible, not persisted.** The filter is per-invocation (CLI) / per-request (viewer). There is NO persisted "hide retracted" preference — a stored default would drift toward silent-by-default and violate D-1. Every run/request re-declares intent explicitly. | LOCKED |

---

## Wave: DISCUSS / [REF] Inherited & New Invariants (I-RF-* extending I-AV-* / I-VIEW-* / I-HX-*)

Binding inputs to DESIGN; NOT relitigated here.

| ID | Inherits / Extends | Carries into this feature as |
|---|---|---|
| I-RF-1 | **I-AV-9** (slice-05) | **Opt-in.** Default behavior unchanged: without the flag/param, a soft-retracted verified claim is still shown + annotated. The filter activates only on explicit user action. |
| I-RF-2 | I-AV-9 ("never down-weighted") | **Non-destructive.** View-only: no index mutation, no re-verify, no re-rank, no re-weight of survivors; survivors keep original order + confidence. |
| I-RF-3 | I-AV-9 (spirit: nothing disappears silently) | **Self-disclosing.** When active and ≥1 hidden, the surface states "N retracted claim(s) hidden". A silent hide is forbidden (build-fail). |
| I-RF-4 | **I-AV-2** (anti-merging) / WD-11 / RC-02 | **Soft-retract only.** Hides author-withdrawn claims only; third-party counters remain shown + annotated (no heckler's veto; a disagreement never removes an author's row). |
| I-RF-5 | ADR-007 / slice-05 `appview-domain` allowlist | **Pure core.** The retraction predicate is a pure total function; CLI/viewer invoke it; the index is never queried a second time to filter. |
| I-RF-6 | I-VIEW-1/2/3/4, I-HX-1..5 (slices 06/07/08) | **Read-only viewer preserved.** The viewer toggle is a GET-param / htmx control — no write/sign/subscribe route, no key in the process, loopback bind, offline (no-CDN) chrome, full page without `HX-Request`. |
| I-RF-7 | D-7 | **Reversible / not persisted.** No stored preference; per-invocation/per-request only. |
| I-RF-8 | KPI-AV-2 / KPI-AV-3 (guardrails) | Every surviving row still carries `[verified]` + `author_did`; no merged/consensus row; confidence verbatim. Filtering removes rows, never alters the anatomy of the rows that remain. |

---

## Wave: DISCUSS / [REF] Story Map and Slicing

One journey: **focus-a-network-survey-without-losing-the-safety-net**. A single coherent
arc — run a survey → notice withdrawn claims are noise for this decision → explicitly hide
them → trust the shorter list *because the tool told me what it hid and I can undo it in one
step*.

### Emotional arc

**mild-friction → deliberate-control → focused-relief → grounded-trust**

- **Entry (mild-friction)**: the survey works, but retracted rows clutter a decision the
  operator is trying to make cleanly. (Not frustration — the data is *correct*; it is just
  more than she needs right now.)
- **Deliberate-control**: she opts in — `--hide-retracted` — a conscious, reversible act.
- **Focused-relief (peak)**: the working view shows only standing reasoning; the honesty
  line confirms *"3 retracted claims hidden"* so she knows the list is filtered, not sparse.
- **Grounded-trust (exit)**: she trusts the shorter list precisely because nothing vanished
  silently — she can re-run without the flag and see everything, unchanged. The tool never
  became "the aggregator that decides for her."

Transition safety: the one risky moment is *empty-after-filter* (every result was retracted).
The design MUST buffer it with an explicit "all N results were retracted; showing none — re-run
without `--hide-retracted` to see them" state, never a bare empty result that reads as "nothing
exists here" (which would betray discoverability, the whole point of J-005).

### Shared artifacts (tracked)

| Artifact | Source of truth | Consumers | Integration risk |
|---|---|---|---|
| `retracted` marker on a result | the composed result's references/retraction graph (slice-05 `SearchResultDto.references`, DV-5) — whether it distinguishes author-retraction from third-party counter is **OD-RF-1** | the pure predicate (D-2), the "N hidden" count (D-4), both surfaces | **HIGH** — if the DTO cannot distinguish author-retraction from disagreement, D-3 cannot be honored without a DTO/ingest extension (see OD-RF-1 + Risks) |
| `hidden_count` | computed by the surface as `len(unfiltered) - len(survivors)` after the pure predicate | CLI footer line, viewer notice | LOW — derived, single computation |
| `hide_retracted` intent | CLI `--hide-retracted` flag / viewer `?hide_retracted=1` param | the predicate call, the honesty-line trigger | LOW — per-invocation, not persisted (D-7) |

### Slicing (by outcome + risk, not feature grouping)

- **Slice 1 (CLI, ships the whole reconciliation)** — `slices/slice-01-cli-hide-retracted.md`:
  **US-RF-001**. The pure predicate + `openlore search --hide-retracted` + the "N hidden"
  honesty line + the empty-after-filter buffer + the default-unchanged regression guard. This
  slice alone proves D-1 end to end on the primary discovery surface. It is the thinnest thread
  that carries the entire I-AV-9 reconciliation.
- **Slice 2 (viewer parity)** — `slices/slice-02-viewer-toggle.md`: **US-RF-002**. The same
  explicit hide as a `?hide_retracted=1` toggle on the read-only `/search` viewer, with the
  honesty notice in both htmx shapes, graceful degradation, and read-only preserved.

### Priority Rationale

Slice 1 (CLI) first because it carries the **riskiest assumption and the entire cardinal
decision**: that a filter can be added to a "never silently filter" surface without violating
I-AV-9. If the opt-in + non-destructive + honest reconciliation (D-1) does not convince on the
CLI — the surface where P-002 does real survey work — the feature is disproven and slice 2 is
moot. Slice 1 also settles OD-RF-1 (does "retracted" mean author-withdrawn in the current DTO?)
against real data before the viewer inherits it. Slice 2 is pure surface parity over the same
pure predicate; its failure is survivable (the CLI already delivers the outcome). Within slice
1, the pure predicate + default-unchanged guard precede the honesty line, because the honesty
line has nothing truthful to report until the predicate and the count are correct.

---

## Wave: DISCUSS / [REF] System Constraints (cross-cutting)

Hold across every story (the I-RF-* invariants restated as build constraints):

- The retraction decision is a **pure total function in `appview-domain`**; both surfaces call
  it. No effectful/index-side filtering; no second index query to filter (I-RF-5, D-2).
- The **default path is byte-identical to today** — without the flag/param, output is
  unchanged from slice-05/08; this is a release-blocking regression guard (I-RF-1).
- Filtering is **view-only and reversible**: no index mutation, no re-verify, no re-rank, no
  re-weight of survivors, no persisted preference (I-RF-2, I-RF-7, D-5, D-7).
- **Only author-soft-retracted claims** are hidden; third-party counters stay shown +
  annotated (I-RF-4, D-3).
- When active and it hid ≥1 row, the surface **discloses the count**; a silent hide is a
  build-fail (I-RF-3, D-4).
- Surviving rows keep the full slice-05 anatomy: **`[verified]` + `author_did` + verbatim
  confidence, no merged row** (I-RF-8).
- The viewer toggle preserves **read-only / loopback / offline-chrome / no-JS-full-page**
  (I-RF-6).
- **Zero new crates**; `check-arch` stays green at 21 members (D-6).

---

## Wave: DISCUSS / [REF] User Stories and Acceptance Criteria

Both stories trace to **J-005** (sub-job **J-005d**). Neither is `@infrastructure` — each
delivers a user-visible, decision-enabling outcome, so each carries an Elevator Pitch. The
pure predicate (D-2) lives inside US-RF-001 as its core, not as a separate infra story (it is
one small pure function, and a standalone all-infra slice would carry no release value).

### US-RF-001: Hide soft-retracted claims from a CLI network search

- **job_id**: J-005 (sub-job J-005d)

#### Elevator Pitch

- **Before**: Rachel runs `openlore search --object org.openlore.philosophy.reproducible-builds`
  and the results include claims their own authors have since **soft-retracted** — correct to
  show (I-AV-9), but noise for the decision she is making right now; she filters them by eye
  on every run.
- **After**: she runs the same command with `--hide-retracted`; stdout shows only the standing
  claims, followed by an honest footer line — e.g. *"2 retracted claim(s) hidden (--hide-retracted
  active); re-run without it to see them."* — with every surviving row still `[verified]`,
  attributed, and confidence-verbatim, in the same order as before.
- **Decision enabled**: she decides which *standing* reasoning to pursue or cite, trusting the
  shorter list is deliberately filtered (she knows exactly how much, and can undo it in one
  re-run) rather than silently curated.

#### Problem

Rachel (P-002) surveys network claims about a philosophy or project to ground a decision.
Soft-retracted claims — withdrawn by their own authors — remain in the result set by design
(I-AV-9). For a focused survey they are noise, and eyeballing them out on every run is friction
that scales with the corpus. She needs an explicit, disclosed, reversible way to drop them from
the working view without the tool ever silently deciding what she does not see.

#### Who

- P-002 (Rachel), researcher / tech lead | at the CLI running `openlore search` | wants to
  focus a survey on standing reasoning, and will not trust a filter that hides silently.

#### Domain Examples

1. **Happy path** — Rachel runs `openlore search --object org.openlore.philosophy.reproducible-builds
   --hide-retracted`. The index holds 12 verified claims across 9 authors; `did:plc:priya-test`
   soft-retracted her `nixos/nixpkgs @ 0.90` claim and `did:plc:bjorn-test` soft-retracted one of
   his. The output shows 10 rows (the 9 authors' standing claims) and the footer
   *"2 retracted claim(s) hidden (--hide-retracted active); re-run without it to see them."*
2. **Default unchanged (I-AV-9 by default)** — Rachel runs the SAME search WITHOUT `--hide-retracted`.
   All 12 rows appear, and Priya's and Bjorn's retracted claims are shown WITH their retraction
   annotation — byte-identical to slice-05 today. No footer about hiding.
3. **Non-destructive** — with `--hide-retracted`, the 10 survivors appear in the exact same
   relative order and with the exact same verbatim confidence values (`0.85`, `0.78`, …) they had
   in the unfiltered run; no survivor is re-ranked or re-weighted.
4. **Empty-after-filter (buffer)** — Rachel searches `--object org.openlore.philosophy.dependency-pinning
   --hide-retracted`; the only 3 indexed claims for it were all soft-retracted by their authors.
   Output shows the guided line *"All 3 result(s) for this search were soft-retracted by their
   authors and were hidden (--hide-retracted active); re-run without it to see them."* — never a
   bare empty result that reads as "nothing exists".
5. **Third-party counter is NOT hidden (D-3)** — `did:plc:bjorn-test` has a standing claim about
   `github:bazelbuild/bazel` that `did:plc:maria` countered (disagreement, not a retraction). With
   `--hide-retracted`, Bjorn's claim is STILL shown, with Maria's counter-annotation inline — only
   author-withdrawn claims are hidden.

#### UAT Scenarios (BDD)

##### Scenario: Explicitly hiding soft-retracted claims focuses the survey and discloses what was hidden
```
Given the index holds 12 verified claims for a philosophy across 9 authors
And 2 of those claims were soft-retracted by their own authors
When Rachel runs the search with --hide-retracted
Then stdout shows only the 10 standing claims, each still verified, attributed, and confidence-verbatim
And a footer line states "2 retracted claim(s) hidden" and how to re-run without the flag
```

##### Scenario: Without the flag, retracted claims are still shown (I-AV-9 default unchanged)
```
Given the same index and the same search
When Rachel runs the search WITHOUT --hide-retracted
Then all 12 claims are shown, including the 2 soft-retracted ones with their retraction annotation
And the output is byte-identical to the pre-feature search
And no "hidden" footer appears
```

##### Scenario: A search where every result is retracted shows a guided state, not a bare empty result
```
Given a philosophy whose only 3 indexed claims were all soft-retracted by their authors
When Rachel runs the search with --hide-retracted
Then the output states that all 3 results were soft-retracted and hidden
And it tells her to re-run without --hide-retracted to see them
And the process does not present the result as "no claims exist for this philosophy"
```

##### Scenario: A claim a third party merely countered is NOT hidden
```
Given a standing claim by one author that a different author has countered (a disagreement, not a retraction)
When Rachel runs the search with --hide-retracted
Then that claim is still shown, with its counter-annotation inline
And only claims soft-retracted by their OWN author are hidden
```

##### @property Scenario: Hiding never re-orders or re-weights the survivors
```
Given any search result set and the --hide-retracted flag
Then the survivors appear in the same relative order as the unfiltered run
And each survivor's confidence value is identical to the unfiltered run
And no survivor's score is recomputed as a result of hiding others
```

#### Acceptance Criteria

- [ ] `openlore search … --hide-retracted` removes from stdout every claim its own author
      soft-retracted, and no other claim (D-3).
- [ ] Without `--hide-retracted`, output is byte-identical to the pre-feature search; retracted
      claims are shown with their retraction annotation (I-RF-1).
- [ ] When ≥1 claim is hidden, a footer line states the exact count and how to re-run without the
      flag (I-RF-3, D-4).
- [ ] When the filter is active but nothing matched, no misleading "hidden" line is printed
      (D-4).
- [ ] When every result is hidden, the guided "all N were soft-retracted / re-run to see them"
      state is shown — not a bare empty result (emotional-arc buffer).
- [ ] Survivors retain original order and verbatim confidence; each still carries `[verified]` +
      `author_did`; no merged row (I-RF-2, I-RF-8, D-5).
- [ ] The filter is a pure `appview-domain` decision invoked by the CLI; the index is not
      re-queried to filter (I-RF-5, D-2).

#### Outcome KPIs

- **Who**: P-002 CLI discovery operators · **Does what**: focus a survey by explicitly hiding
  author-retracted claims, while reporting the filtered view is trustworthy (they can state what
  was hidden) · **By how much**: KPI-RF-1 target — ≥50% of operators who hit a retracted-heavy
  result set adopt `--hide-retracted` within their session, AND ≥90% correctly report "the tool
  told me what it hid" on the day-30 comprehension prompt · **Measured by**: search telemetry
  (`--hide-retracted` usage rate; hidden_count distribution) + comprehension prompt · **Baseline**:
  0 (no filter exists before this feature).

#### Technical Notes

- Add the pure predicate to `appview-domain` and extend the `openlore search` arg parser
  (ADR-027) with `--hide-retracted`; the CLI counts survivors vs unfiltered for the footer.
- REUSE the slice-05 composition (`compose_results`) + `SearchResultDto`; the retraction marker
  is READ from the already-composed result (OD-RF-1 governs whether the current DTO already
  distinguishes author-retraction from third-party counter).
- Dependencies: slice-05 `appview-domain` + `adapter-index-query` + `IndexQueryPort` +
  `SearchResultDto.references` (all shipped). No new crate.

---

### US-RF-002: The same explicit hide toggle on the read-only `/search` viewer

- **job_id**: J-005 (sub-job J-005d)

#### Elevator Pitch

- **Before**: Maria discovers network claims in her browser `/search` (slice-08), but every
  survey includes soft-retracted rows; to drop them she must go back to the CLI's
  `--hide-retracted`.
- **After**: she ticks a "Hide retracted claims" control on `/search` (a plain GET-param
  `?hide_retracted=1`); the results region re-renders with only standing claims and a notice —
  *"2 retracted claim(s) hidden — showing standing claims only. Untick to see them."* — every
  surviving row still `[verified]`, attributed, confidence-verbatim, with no merged row and the
  page still read-only.
- **Decision enabled**: she decides which standing reasoning to act on from the browser,
  trusting the shorter list is a filtered *view* (she sees how many were hidden and can restore
  them in one click) — the read-only viewer never silently curates or follows for her.

#### Problem

Maria (P-001) uses the read-only viewer for network discovery. Soft-retracted rows clutter a
focused survey in the browser exactly as they do on the CLI, but the browser has no hide control
— forcing a context switch. The control must honor every viewer guardrail: read-only, no key,
loopback, offline chrome, full page without JS, and — like the CLI — hide nothing silently.

#### Who

- P-001 (Maria), node operator | at her loopback `openlore ui` `/search` | wants CLI-parity
  focus in the browser without giving the viewer any mutate/follow capability.

#### Solution

Extend the slice-08 `/search` form with a "Hide retracted claims" control that adds
`?hide_retracted=1` to the query. On submit, the viewer runs the SAME pure `appview-domain`
predicate (US-RF-001) over the composed results and renders survivors, with a results-region
notice stating the hidden count. Served as a full page without `HX-Request` and as a
results-region fragment with it (slice-07 `Shape` fork); the notice appears in both shapes. An
unreachable/unconfigured indexer degrades exactly as slice-08 (calm message), independent of the
toggle.

#### Domain Examples

1. **Happy path** — Maria searches object `org.openlore.philosophy.reproducible-builds` with the
   "Hide retracted claims" box ticked (`?object=…&hide_retracted=1`); the results region shows the
   9 authors' standing rows and the notice *"2 retracted claim(s) hidden — showing standing claims
   only. Untick to see them."*
2. **Default unchanged (I-AV-9 in the browser)** — Maria searches the same philosophy with the box
   unticked; all 12 rows render, the 2 soft-retracted ones shown with their retraction annotation —
   identical to slice-08 today; no "hidden" notice.
3. **htmx parity** — Maria (JS enabled) ticks the box and re-submits; only `#search-results` swaps,
   the form is preserved, and the swapped fragment (rows + notice) is structurally identical to the
   full-page render of `?…&hide_retracted=1`.
4. **Empty-after-filter (buffer)** — Maria searches a philosophy whose only 3 indexed claims were all
   soft-retracted, box ticked; the results region shows *"All 3 results were soft-retracted by their
   authors and are hidden. Untick 'Hide retracted claims' to see them."* — never a blank region.
5. **Read-only preserved / degradation** — the toggle is a GET-param only; the page exposes no
   write/sign/subscribe control. With the indexer unreachable, submitting (box ticked or not) shows
   the slice-08 calm "index unavailable; your local store views still work" message — no crash, no
   leaked transport internals.

#### UAT Scenarios (BDD)

##### Scenario: Ticking "Hide retracted claims" focuses the browser survey and discloses the count
```
Given Maria has run a philosophy search in /search with a reachable indexer
And 2 of the results were soft-retracted by their own authors
When she ticks "Hide retracted claims" and submits
Then the results region shows only the standing claims, each still verified, attributed, confidence-verbatim
And a notice states "2 retracted claim(s) hidden" and how to untick to see them
And no merged "network consensus" row appears
```

##### Scenario: With the box unticked, retracted claims are still shown (I-AV-9 default unchanged)
```
Given the same search with a reachable indexer
When Maria submits with "Hide retracted claims" unticked
Then every result including the soft-retracted ones is shown with its retraction annotation
And the render is identical to the pre-feature /search
And no "hidden" notice appears
```

##### Scenario: The hide result region swaps in place under htmx and matches the full page
```
Given Maria has JavaScript enabled and has run a philosophy search
When she ticks "Hide retracted claims" and re-submits
Then only the search-results region updates (the form is preserved)
And the swapped rows and the hidden-count notice are identical to the full-page render of the same filtered search
```

##### Scenario: A search where every result is retracted shows a guided state, not a blank region
```
Given Maria searches a philosophy whose only indexed claims were all soft-retracted
When she submits with "Hide retracted claims" ticked
Then the results region states all results were soft-retracted and are hidden
And it tells her to untick the control to see them
And the region is never blank and the viewer never crashes
```

##### Scenario: The hide control keeps the viewer read-only and degrades honestly
```
Given the /search page in the read-only viewer
Then the "Hide retracted claims" control is a plain GET-param toggle with no write/sign/subscribe action
And when the indexer is unreachable, submitting with the box ticked or unticked shows the calm "index unavailable" guidance
And no HTTP status, connection error, raw URL, or stack trace is shown
```

#### Acceptance Criteria

- [ ] `/search?…&hide_retracted=1` (no `HX-Request`) serves a full page whose results region
      shows only standing claims + the hidden-count notice (I-RF-1, I-RF-3).
- [ ] `/search?…` without the param renders identically to slice-08; retracted rows shown with
      annotation; no notice (I-RF-1).
- [ ] The same filtered submit WITH `HX-Request` returns only the results-region fragment,
      structurally identical to the full page's region — notice included (I-RF-6, slice-07 parity).
- [ ] Every surviving row carries `[verified]` + `author_did` + verbatim confidence; no merged
      row; survivors keep order (I-RF-2, I-RF-8).
- [ ] Every-result-retracted renders the guided "untick to see them" state, not a blank region.
- [ ] The control is a GET-param toggle only — no write/sign/subscribe route, no key in the
      process; an unreachable indexer degrades to the slice-08 calm message in both shapes (I-RF-6).
- [ ] The viewer invokes the SAME pure `appview-domain` predicate as the CLI (no second filter
      logic) (I-RF-5, D-2).

#### Outcome KPIs

- **Who**: P-001 viewer operators · **Does what**: focus a browser survey by explicitly hiding
  author-retracted claims, trusting the filtered view (they can state what was hidden) · **By how
  much**: realizes KPI-RF-1 on the browser surface (parity with the CLI) while holding all slice-08
  guardrails (read-only, verified, attributed, PE, degradation) · **Measured by**: viewer `/search`
  telemetry (`hide_retracted` toggle rate; hidden_count) + day-30 comprehension prompt · **Baseline**:
  0 (no browser hide control before this feature).

#### Technical Notes

- REUSE the US-RF-001 pure predicate + the slice-08 `/search` render + slice-07 `Shape` fork; add
  the toggle to the form and the notice to the results region.
- OD-RF-2 (control placement/labeling), OD-RF-3 (notice wording/placement) are DESIGN's.
- Dependencies: US-RF-001 (the pure predicate + the CLI reconciliation validated first);
  slice-08 `/search` + `adapter-http-viewer` + `viewer-domain` (shipped). No new crate.

---

## Wave: DISCUSS / [REF] Outcome KPIs

This feature MINTS one new leading KPI (the behavior it enables did not exist) and REALIZES it on
two surfaces; it inherits all slice-05/08 guardrails unchanged.

### Objective

Make a retracted-heavy network survey focusable on standing reasoning — without ever weakening the
"nothing disappears silently" promise that makes discovery trustworthy.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-RF-1 | P-002/P-001 discovery operators facing a retracted-heavy result set | adopt the explicit hide (flag/toggle) AND correctly report the tool disclosed what it hid | ≥50% adoption in-session; ≥90% correct "it told me what it hid" comprehension | 0 (no filter exists) | search telemetry (usage rate + hidden_count) + day-30 comprehension prompt | Leading (Outcome) |

### Metric Hierarchy

- **North Star (inherited)**: KPI-AV-1 — ≥60% of discovery sessions surface ≥1 verified claim by
  an unfollowed author. KPI-RF-1 is a *usability tributary*: a focusable view keeps a
  retracted-heavy corpus usable so discovery still lands.
- **Leading**: KPI-RF-1 (explicit-hide adoption + disclosure comprehension).
- **Guardrail Metrics (release-blocking, all inherited + one new)**: KPI-AV-2 (anti-merging),
  KPI-AV-3 (verified-before-index / every row `[verified]`), KPI-VIEW-2 (read-only), KPI-HX-G1
  (no-JS full page), KPI-HX-G2 (offline chrome), KPI-5 (local-first), and **NEW: the
  default-unchanged guard** — the without-flag/param path stays byte-identical (I-RF-1); any drift
  is a build-fail (this is the mechanical proof that I-AV-9 was not weakened).

### Measurement Plan

| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|------------|-------------------|-----------|-------|
| KPI-RF-1 adoption | CLI + viewer search telemetry | `--hide-retracted` / `hide_retracted=1` usage events + hidden_count | weekly | DEVOPS (platform-architect) |
| KPI-RF-1 comprehension | day-30 prompt | "Could you tell what the filter hid?" y/n | one-shot per cohort | product |
| default-unchanged guard | acceptance suite | byte-identical snapshot of without-flag output vs pre-feature | per build (CI) | DELIVER |

### Hypothesis

We believe that an opt-in, non-destructive, self-disclosing retraction filter for discovery
operators facing retracted-heavy surveys will let them focus on standing reasoning WITHOUT eroding
trust. We will know this is true when ≥50% adopt it in-session and ≥90% can state what it hid — and
when the without-flag path remains byte-identical (I-AV-9 intact).

A per-feature `outcome-kpis.md` is intentionally NOT duplicated (lean): KPI-RF-1 is defined here and
belongs in `docs/product/kpi-contracts.yaml` alongside KPI-AV-*. DEVOPS adds `hide_retracted` usage
+ `hidden_count` telemetry to the existing search event stream.

---

## Wave: DISCUSS / [REF] Out of Scope

- **Silent / default filtering** — never; the default is byte-identical to today (D-1, I-RF-1).
- **Hiding third-party-countered claims** — a disagreement is not a retraction; those stay shown +
  annotated (D-3, I-RF-4). (Whether a future "hide contested" control is worth building is a
  separate opportunity, not this feature.)
- **Hard-delete / index mutation / re-verification / re-scoring** — forbidden (D-5, I-RF-2;
  hard-delete is already forbidden by WD-11 even with `--force`).
- **A persisted "hide retracted" preference / config default** — would drift toward silent-by-default
  (D-7, I-RF-7).
- **Down-weighting or re-ranking retracted claims** instead of hiding — that is a scoring change,
  explicitly out (I-AV-9 "never down-weighted"; cross-user scoring is separately deferred, WD-79).
- **A browser shareable-link** encoding the filtered query — deferred (the slice-05 `--share` and
  its CLI re-run resolver are unchanged; a filtered `--share` is a future nicety, not this feature).
- **Firehose / real-time retraction propagation** — inherited deferral (ADR-024).

---

## Wave: DISCUSS / [REF] Walking Skeleton Strategy

Walking skeleton = **No** (per orchestrator config): this is a brownfield DELTA over a shipped
mechanism (slice-05 search stack + slice-08 viewer), not a new end-to-end mechanism. There is no
new integration axis to de-risk with a skeleton.

The equivalent thin thread is **US-RF-001** (slice 1): pure predicate → `--hide-retracted` flag →
survivors + honesty line, over the already-shipped search path. It touches exactly TWO net-new
points (the pure predicate in `appview-domain`; the flag + footer in `cli`) and REUSES the entire
slice-05 query/verify/compose path. It carries the whole cardinal decision (D-1) end to end, so
validating it validates the feature's thesis before the viewer inherits it.

---

## Wave: DISCUSS / [REF] Driving Ports (for DESIGN)

Names indicative; DESIGN owns shapes.

- **CLI (US-RF-001)**: extend the `openlore search` verb (ADR-027) with a `--hide-retracted`
  boolean flag. No new port; the flag is passed as a bool into the pure predicate applied to the
  composed results; the CLI computes `hidden_count` for the footer.
- **HTTP (US-RF-002)**: extend the slice-08 `/search` GET surface with a `hide_retracted` query
  param (`?hide_retracted=1`). No new outbound capability — REUSES the slice-08 indexer-query
  effect; the filter runs after composition.
- **Pure core (both)**: a new pure total function in `appview-domain` (indicative
  `retain_visible(result, hide_retracted) -> bool` or `partition_retracted(results, hide_retracted)
  -> (survivors, hidden_count)`), added to the slice-05 pure-core allowlist. This is the single
  source of the filter decision both surfaces invoke (I-RF-5).
- **Existing (reused, unchanged)**: the slice-05 `IndexQueryPort` + `adapter-index-query` +
  `compose_results` + `SearchResultDto`; the slice-08 `Shape::from_request` fork + page=chrome+fragment.

---

## Wave: DISCUSS / [REF] Pre-requisites and Open Decisions for DESIGN

### Pre-requisites (shipped, inherited)

- slice-05 `openlore-indexer` + `adapter-index-query` + `IndexQueryPort` + `appview-domain`
  (`compose_results`) + the `SearchResultDto.references` field (DV-5) + the `openlore search` verb
  (ADR-027).
- slice-08 `/search` route + `adapter-http-viewer` + `viewer-domain` render + slice-07 `Shape` fork.

### Open Decisions (OD-RF-*) — DESIGN owns

| ID | Decision | Default lean |
|---|---|---|
| **OD-RF-1** | **(HIGH — settle in slice 1)** Does the current `SearchResultDto.references` graph let the pure predicate distinguish an **author self-retraction** (same-DID retraction counter referencing the CID — the D-3 target) from a **third-party disagreement counter**? If not, DESIGN must surface a retraction marker (a small DTO/ingest field or a derivation rule at compose time). | Recommend a pure derivation at compose time: a result is `retracted` iff its references graph contains a retraction-type counter whose author DID equals the result's author DID. If the DTO cannot express "retraction-type", add a minimal marker at ingest (mirrors the DV-5 lesson: a cross-process invariant belongs in the wire shape). |
| **OD-RF-2** | Viewer control UI: checkbox vs a two-state link; label wording ("Hide retracted claims"). | Recommend a labeled checkbox that sets `?hide_retracted=1`, so the no-JS path is a plain GET navigation (consistent with slice-08 OD-NS-5 GET-form). |
| **OD-RF-3** | Honesty-line/notice exact wording + placement (CLI footer vs inline; viewer results-region notice vs banner) and the empty-after-filter copy. | Recommend a CLI footer line and a viewer results-region notice (co-located with the rows they describe); empty-after-filter gets the explicit "re-run/untick to see them" buffer copy. |
| **OD-RF-4** | Predicate signature: a per-row `retain_visible` predicate vs a `partition_retracted` that returns survivors + count in one pass. | Recommend `partition_retracted` (survivors + `hidden_count` in one pure pass) so the honesty count is computed by the same pure function, not re-derived by each surface. |

### Risks (surfaced, not managed here)

- **R-1 (technical, HIGH prob / HIGH impact if true)**: OD-RF-1 — if the shipped
  `SearchResultDto.references` does NOT distinguish author-retraction from third-party counter, D-3
  cannot be honored without a DTO/ingest extension, which enlarges slice 1 beyond a pure-core
  change. Mitigation: settle OD-RF-1 against real indexer data at the start of slice 1 (DESIGN);
  if an ingest change is needed, it is small and additive (a marker), but the user should know it
  is possible before DESIGN commits an estimate.
- **R-2 (product, LOW/MED)**: operators could read `--hide-retracted` as "the safe default" and
  over-hide. Mitigation: D-7 (never persisted) + I-RF-3 (always discloses the count) keep every
  hide a conscious, visible act.

---

## Wave: DISCUSS / [REF] Definition of Ready validation

| DoR item | US-RF-001 (CLI) | US-RF-002 (viewer) |
|---|---|---|
| 1. Problem statement clear, domain language | PASS | PASS |
| 2. Persona with specific characteristics | PASS (P-002 Rachel) | PASS (P-001 Maria) |
| 3. ≥3 domain examples with real data | PASS (5) | PASS (5) |
| 4. UAT in Given/When/Then (3-7) | PASS (5, incl. 1 @property) | PASS (5) |
| 5. AC derived from UAT | PASS (7) | PASS (7) |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS (~1 day, 5 scenarios) | PASS (~1.5 days, 5 scenarios) |
| 7. Technical notes: constraints/dependencies | PASS | PASS |
| 8. Dependencies resolved or tracked | PASS (slice-05 shipped; OD-RF-1 flagged as R-1) | PASS (US-RF-001 + slice-08 shipped) |
| 9. Outcome KPIs defined with measurable targets | PASS (KPI-RF-1) | PASS (KPI-RF-1 browser parity) |

**Overall DoR status: PASSED** for both stories.

Notes:
- Every story is user-visible and carries an Elevator Pitch with a real entry point
  (`openlore search --hide-retracted`; `/search?hide_retracted=1`) and concrete observable output
  (stdout footer sample; rendered notice) — passes Dimension 0.
- Neither story is `@infrastructure`; the slice is not 100% infrastructure — passes Dimension 0 §5.
- One open decision (OD-RF-1) is tracked as a HIGH risk (R-1) rather than a blocker: the feature is
  Ready to enter DESIGN, and DESIGN's first act on slice 1 is to settle OD-RF-1 against real data.

---

## Wave: DISCUSS / [REF] Wave-Decisions Summary

- **Feature type**: user-facing, opt-in FILTER on network search (CLI + read-only viewer).
- **Primary job**: J-005; new sub-job **J-005d** appended to `docs/product/jobs.yaml` (not a new
  primary job; `load_bearing: false`).
- **Cardinal decision**: **D-1** — the filter is reconciled with I-AV-9 by being opt-in +
  non-destructive + self-disclosing; the default view is byte-identical to today. Formalized as
  I-RF-1..3.
- **Scope**: **D-3** — soft-retract (author-withdrawn) claims ONLY; third-party counters stay shown.
- **Paradigm**: **D-2/I-RF-5** — pure total predicate in `appview-domain` (ADR-007); both surfaces
  invoke it; zero new crates (D-6; workspace stays 21 members).
- **Slices**: slice 1 = CLI `--hide-retracted` (carries the whole reconciliation); slice 2 = viewer
  `?hide_retracted=1` parity.
- **Open decisions**: OD-RF-1..4; OD-RF-1 is the one HIGH risk (does the DTO distinguish
  author-retraction from third-party counter?) — settle first in slice 1.
- **DoR**: PASSED (both stories).
- **Scope assessment**: PASS — 2 stories, 1 bounded context (`appview-domain` + 3 reused adapters),
  estimated ~2.5 days total; well within right-sized (no split needed).
- **DIVERGE artifacts**: none present for this feature (no `diverge/` dir); JTBD grounded directly
  in the shipped J-005 + RC-02/WD-11 retraction model. Noted as a (low) risk: no independent
  DIVERGE option-set preceded this narrowly-scoped brownfield delta.
