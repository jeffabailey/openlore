# DESIGN Wave-Decisions — retraction-aware-search-filter

> Wave: **DESIGN** (application scope) · Mode: **propose** · Owner: Morgan (nw-solution-architect)
> Date: 2026-07-11 · Primary artifact: `../feature-delta.md` (DESIGN sections) · ADR: **ADR-060**
> Brownfield DELTA on slice-05 (`openlore-appview-search`) + slice-08 (`viewer-network-search`).

## The decisive resolution — OD-RF-1 (R-1, HIGH)

**BRANCH A — SUFFICIENT AS-IS.** A PURE predicate distinguishes an author self-retraction from a
third-party counter using only data already on the wire. **Zero ingest / schema / DTO change.**
Slice 01 stays pure-core; R-1 retired.

Evidence (real code, file/line):

- `crates/cli/src/verbs/claim_retract.rs:98-115` — the retraction record mirrors the original's
  subject/predicate/object, is authored by the retracting user's OWN DID, and carries
  `references = [{Retracts, original_cid}]` ⇒ co-returned with the original by any dimensional
  query.
- `crates/lexicon/src/appview_query.rs:70-107` — `SearchResultDto` carries `author_did` +
  `references: [{ref_type, cid}]` on the wire (DV-5).
- `crates/adapter-index-query/src/lib.rs:204-251` + `crates/ports/src/index_query.rs:42-54` —
  decoded into `NetworkResultRowRaw{author_did, cid, references}`.
- `crates/cli/src/render/search.rs:223-243` — a `{counter_cid, counter_author, countered_cid}`
  projection over the same edges already exists.

Predicate: **C is author-self-retracted ⟺ ∃ row K with `K.author_did == C.author_did` carrying
`{Retracts, C.cid}`.** Third-party `Counters` (and any different-author `Retracts`) → shown +
annotated (D-3 / I-AV-9).

Two subtleties the code forced:
1. Run on the RAW rows — `compose_results`' `counter_annotation` is a lossy single-slot
   lowest-CID collapse (compose.rs:77-100) that can mask a self-retraction.
2. A retraction is ONE event = the withdrawn original + its same-author marker record (both
   hidden); `hidden_count` = EVENTS. This refines the DISCUSS `len`-diff note (which
   double-counts the marker row) — back-propagated to the slice-01 brief.

## Locked design decisions

| # | Decision |
|---|---|
| D-RF-D1 | OD-RF-1 = Branch A; pure predicate over the existing reference graph; no DTO/ingest/schema change. |
| D-RF-D2 | ONE `appview_domain::partition_retracted`; CLI + viewer both invoke it on raw `NetworkResultRowRaw`; no re-query, no duplicated logic. |
| D-RF-D3 | Self-retraction rule (D-3 literal): same-author `Retracts` referencing the CID; Counters + cross-author Retracts never hide. |
| D-RF-D4 | Retraction event = original C + its same-author marker K; both hidden together. |
| D-RF-D5 | `hidden_count` = retraction EVENTS (`|{C self-retracted}|`), refining the DISCUSS `len`-diff. |
| D-RF-D6 | Opt-in / non-destructive / self-disclosing (byte-identical default; order + verbatim confidence preserved; disclose when ≥1; no misleading line when 0). |
| D-RF-D7 | Viewer = plain `?hide_retracted=1` GET-param / htmx toggle; read-only, no key, loopback, offline, full page sans `HX-Request`; not persisted. |
| D-RF-D8 | Enforcement: in-crate `@property` (order+confidence) + default-unchanged byte guard + pure-core allowlist (`check-arch` = 21) + per-feature mutation gate. |

## Component decomposition (all EXTEND / REUSE — zero CREATE NEW)

| Component | Change | Note |
|---|---|---|
| `appview-domain` (pure) | EXTEND | `partition_retracted`; stays pure-core. |
| `cli` search verb + render | EXTEND | `--hide-retracted` flag + footer + empty buffer. |
| `viewer-domain` (pure) | EXTEND | toggle control + hidden-count notice. |
| `adapter-http-viewer` (effect) | EXTEND | parse `?hide_retracted=1`; call predicate pre-compose; no write surface. |
| `ports::NetworkResultRowRaw`, `lexicon::SearchResultDto`, `adapter-index-query`, indexer server/store | REUSE unchanged | reference graph already present (Branch A). |
| xtask | REUSE/verify | no new rule; workspace stays 21. |

Full Reuse Analysis table + C4 (System Context + Container, Mermaid) + I-RF-1..8 enforcement
mapping live in `../feature-delta.md` DESIGN sections.

## Quality gates

- [x] OD-RF-1 settled against real code (Branch A) with file/line evidence.
- [x] Requirements traced to components; component boundaries + responsibilities defined.
- [x] ADR-060 with 2+ alternatives + rejection rationale + consequences + Earned Trust.
- [x] Dependency-inversion / pure-core compliance (predicate in `appview-domain`; no I/O).
- [x] Simplest-solution: one pure fn extends an existing crate; 2+ simpler-or-rejected
      alternatives documented (Branch B additive marker; lossy-annotation reuse; index-side filter).
- [x] C4 L1 + L2 (Mermaid); L3 not warranted (< 5 components).
- [x] Reuse Analysis complete; NO CREATE-NEW verdicts.
- [x] Zero new crates; `check-arch` stays 21; `appview-domain` stays pure (no probe needed).
- [x] I-AV-9 reconciliation mechanically guarded (default-unchanged byte-identical).
- [x] Peer review (solution-architect-reviewer) — **APPROVED** (iteration 1; 0 critical / 0 high
      / 0 medium / 0 low). All 5 critique dimensions PASS; Reuse Analysis judged complete with
      zero hidden CREATE-NEW; ADR-060 rated HIGH quality; I-AV-9 reconciliation judged
      mechanically non-weakening; D-RF-D4/D5 judged defensible and within D-3 scope. One low-risk
      note deferred to DELIVER: predicate-execution observability (logs/metrics).

## Deferred to DISTILL / DELIVER

- OD-RF-2 (viewer control UI/label), OD-RF-3 (footer/notice wording + empty copy) → DISTILL.
- OD-RF-4 (predicate signature) → RESOLVED: `partition_retracted` (survivors + event count).
- OD-RF-5 (NEW) — default-view retraction-marker handling → OUT of scope; product/future.
- Q-DELIVER-RF-1 — exact `partition_retracted` input type → filter `NetworkResultRowRaw` directly.

## Handoff to DISTILL (acceptance-designer)

- Gold fixtures MUST model the original+marker pair (not the 12→10 arithmetic) and the
  self-vs-third-party contract (a claim BOTH self-retracted AND third-party-countered → hidden;
  a third-party counter alone → shown + annotated).
- Assert: default-unchanged byte-identical (release gate); `@property` order+confidence stable;
  empty-after-filter guided buffer; CLI footer + viewer notice disclosure; count = EVENTS.
- No new external integration; no new contract-test surface (localhost indexer already covered).
