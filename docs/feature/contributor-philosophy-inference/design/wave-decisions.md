# DESIGN Wave-Decisions — contributor-philosophy-inference

> Wave: **DESIGN** (application / component scope) · Mode: **propose** · Owner: Morgan
> (nw-solution-architect) · Date: 2026-09-27 · Primary artifact: `../feature-delta.md` (DESIGN
> sections) · ADRs: **ADR-063** (component architecture), **ADR-064** (person-adherence wire
> contract). Brownfield extension of the shipped scraper (ADR-017..019). ZERO new crates. ZERO
> Lexicon change.

## Locked inputs (recorded, not re-litigated)

D-1..D-4 (user) and OD-CPI-1..7 accepted exactly as recommended in DISCUSS (2026-09-27).

## Key Decisions

- [DDD-1] Pure inference core = EXTEND `crates/scraper-domain` (`people` area): same J-004 context and candidate invariants, already check-arch pure (see: ADR-063 §1)
- [DDD-2] `GithubPort::list_contributors`: one `per_page=100` request, raw rows, N applied in pure core; 0 requests when N = 0 (see: ADR-063 §2)
- [DDD-3] Bot rule (`type == Bot` or `[bot]` suffix), re-rank by contributions, de-dup by id, all pure (see: feature-delta DDD-3)
- [DDD-4] NEW `ContributionLinkPort` (no delete method) + `DuckDbContributionLinkAdapter` on the shared connection (see: ADR-063 §3)
- [DDD-5] Migration v5 `contribution_links`, case-folded PK, one-tx upsert, never delete, stale derived (see: ADR-063 §4)
- [DDD-6] Signed-claim inputs via REUSED `query_federated_by_subject` + `query_by_contributor`/`read_signed_claim` — no new StoragePort method (see: ADR-063 §5)
- [DDD-7] Support eligibility incl. ADR-060 D-RF-D3 rule hoisted into `claim-domain`; case-insensitive `github:` join (see: ADR-063 §5)
- [DDD-8] NEW / STRONGER(supersedes) / already-signed / SUPPORT WEAKENED(reason); deterministic numbering (see: ADR-063 §6)
- [DDD-9] Confidence in integer hundredths `min(29, 15+5(k−1), floor(100·max))`, arithmetic displayed (see: ADR-063 §6)
- [DDD-10] Provenance = `evidence[]` AT-URIs of supporting claims + `commits?author=` URLs (see: ADR-064 §3)
- [DDD-11] No predicate validator change needed — predicate is a free string today (see: ADR-064 Context)
- [DDD-12] Sign through the extracted shared scraper sign batch; `ComposedClaim.references` (default empty) (see: ADR-063 §7)
- [DDD-13] `infer people [--person] [--min-repos] [--sign]`; `scrape github <repo> --contributors N`; `scrape github <user>` person view; no `--json` (see: ADR-063 §8)
- [DDD-14] Scrape sequencing: no link write before a complete harvest; too-large/empty = notice, exit 0 (see: ADR-063 §9)
- [DDD-15] Live link-adapter probe (in-tx upsert-twice, rolled back); GitHub lies as gold fixtures (see: ADR-063 Earned Trust)
- [DDD-16] `xtask check-arch`: `contribution_links_append_only` + adapter-github port guard (see: ADR-063 Earned Trust)

## Architecture Summary

- Pattern: modular monolith, ports-and-adapters (ADR-009), unchanged
- Paradigm: FP (ADR-007) — pure core `scraper-domain`/`claim-domain`, effect shell in `adapter-github`, `adapter-duckdb`, `cli`
- Key components: `scraper-domain` people area (EXTEND), `GithubPort`/`adapter-github` (EXTEND), `ContributionLinkPort` + `DuckDbContributionLinkAdapter` + schema v5 (NEW port / EXTEND adapter), `infer people` verb (NEW file), `scrape github` verb + shared sign batch (EXTEND), `xtask check-arch` (EXTEND)

## Reuse Analysis

| Existing Component | File | Overlap | Decision | Justification |
|-------------------|------|---------|----------|---------------|
| candidate derivation | crates/scraper-domain/src/derive.rs | pure candidate proposals | EXTEND | sibling `people` area; same invariants |
| GithubPort / adapter | crates/ports/src/lib.rs, crates/adapter-github/src/lib.rs | public GitHub reads | EXTEND | one more allowlisted read |
| query_federated_by_subject | crates/adapter-duckdb/src/lib.rs | own ∪ peer claims + refs + relationship | REUSE | exact D-2 input shape |
| query_by_contributor / read_signed_claim | crates/adapter-duckdb/src/lib.rs | my claims | REUSE | bounded read |
| D-RF-D3 self-retraction | crates/appview-domain/src/retraction.rs | retraction rule | EXTEND (hoist to claim-domain) | one rule, two consumers |
| reference rules / Supersedes | crates/claim-domain/src/references.rs | supersede | REUSE | shipped ADR-008 |
| Lexicon claim | crates/lexicon/src/claim.rs | predicate/evidence/references | REUSE | no Lexicon change |
| scraper --sign batch | crates/cli/src/verbs/scrape_github.rs | sign flow | EXTEND (extract) | single sign path |
| ComposedClaim | crates/cli/src/verbs/claim_add.rs | compose shape | EXTEND (`references`) | empty default keeps CIDs |
| migrations | crates/adapter-duckdb/src/schema_v4.rs | forward-only migration | EXTEND (v5) | same pattern |
| ContributionLinkPort | crates/ports | — | CREATE NEW (trait) | extending StoragePort breaks every fake + mixes observations into the claim port |
| infer_people verb | crates/cli/src/verbs/infer_people.rs | — | CREATE NEW (file) | new locked verb; one file per verb |
| check-arch | xtask/src/check_arch.rs | enforcement | EXTEND | one rule + one guard |

## Technology Stack

- Rust workspace unchanged; no new dependency. `reqwest` (MIT/Apache-2.0) reused; DuckDB (MIT) `ON CONFLICT DO UPDATE`, probe-guarded; `proptest` (MIT/Apache-2.0) for the pure core.

## Constraints Established

- Contribution links are append/upsert-only: no delete API, no DELETE SQL (type + check-arch + byte-compare test).
- Only signed, active, non-retracted, non-superseded `embodiesPhilosophy` repo claims feed inference.
- Nothing is written to claim tables without `--sign`; the system never emits retract/counter/supersede on its own.
- Inferred confidence never exceeds 0.29 nor the strongest supporting claim's confidence.
- ≤ 1 extra GitHub request per repo scrape (0 at N = 0); public data only.
- New person surfaces say "person"; `--contributor <did>` stays claim author.

## Upstream Changes

- `design/upstream-changes.md` UC-1..UC-8 (clarifications: N = 0 no request; too-large/empty notice exit 0; `--contributors` rejected on user target; hand-authored never STRONGER; weakened incl. unsubscribed peer; case-insensitive `github:` join; evidence wording; numbering tied to filters).
- Changed Assumptions in `../feature-delta.md` (read port = StoragePort not StoreReadPort; provenance in payload for person candidates vs WD-62 display-only for repo candidates).

## Quality Gates

- [x] Requirements (US-CPI-001..005) traced to components
- [x] Boundaries + responsibilities; dependency inversion (pure core has no I/O; new port in `ports`)
- [x] ADR-063 + ADR-064 with alternatives and rejection rationale
- [x] C4 L1 + L2 + L3 (people area) in Mermaid
- [x] OSS-only; no new dependency
- [x] External integration (GitHub `/contributors`) annotated for recorded-fixture contract test
- [x] Enforcement tooling (`xtask check-arch` rule) specified
- [ ] Outcome collision check — NOT RUN (no shell in session; no `docs/product/outcomes/registry.yaml`)
- [ ] Peer review — skipped (optional per nw-design; consolidated review at end of DISTILL)

## Handoff to DISTILL (acceptance-designer)

- Gold fixtures: `FakeGithub` contributor lie catalogue (unsorted, Bot type, `[bot]`-with-User-type, duplicate ids, 204, 403 too-large, 403 rate-limit, 401, missing fields).
- Assert: exactly N humans and bot names; re-scrape keeps links + first_observed_at; no link write on failure; D-2 filter (unsigned/retracted/unsubscribed/counter/marker never support); confidence arithmetic; deterministic numbering; provenance round-trip in signed evidence; STRONGER adds `supersedes` and old claim byte-identical; SUPPORT WEAKENED reasons; hint silent when unchanged; offline `infer people`.
