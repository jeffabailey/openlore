# Upstream Changes: owner-only-signing (DESIGN, 2026-10-14)

No DISCUSS wave. These are prior-wave decisions this feature changes or depends on. The
github-did-identity-link (GDL) files are **not** edited; the changes they need are listed here and
in ADR-086 §"Relationship to github-did-identity-link". Every row needs the user's confirmation
together with the matching OOS decision.

## Changes needed in github-did-identity-link (all GDL decisions are still PROPOSED)

| # | GDL decision / text | Source | Needed change | Why |
|---|---|---|---|---|
| UC-1 | "Display-only association everywhere"; A-6 "display-only association is enough for v1" | GDL-D8, ADR-085 §6, A-6 | **Extend**: a `verified` link is also the source of **person** and **user-owned-repo** authority (ADR-087 §1-2). Still no subject merging, no merged score; anti-merging unchanged. | Owner-only signing needs a proof that a DID owns `github:<login>` |
| UC-2 | Association rule: two `Verified` DIDs for one id → `Contested`, none associated | ADR-085 §5, GDL-D14 | **Unchanged for display.** For authority each `verified` DID counts: the account holder named both DIDs in the bio. | Someone with a personal and a work DID must be able to sign with either |
| UC-3 | Per-verb capability table | ADR-085 §7 | **Add rows** (existing rows retained, incl. `scrape github <user>` link-check write): `claim add`, `infer people --sign`, `claim publish` → check-read (identity + repo authority); `scrape github <owner/repo>` / `scrape person` → repo-authority write; `authority verify` → identity-link check write + repo-authority write + `GithubPort`; viewer → read only. `identity_link_readers_hold_no_write_handle` extends to `authority_readers_hold_no_write_handle`. | Gate needs local checks; least privilege |
| UC-4 | Slice order (CLI link → readers → review app) | GDL wave-decisions §Slices | **Prerequisite**: GDL slice 1 before OOS slice 1. GDL slice 3 (review-app "Publish this link") before review-app claims can read `owner`. | Authority depends on links |
| UC-5 | `identity verify` re-checks links only | ADR-085 §7, GDL-D10 | **Superset**: `openlore authority verify` re-checks the links it needs plus repo observations. `identity verify` stays as is. | One command for "can these claims be trusted" |
| UC-6 | Scorer drops `sameAs` claims | ADR-085 §6 | **Composes**: `sameAs` claims are a non-weight kind — no authority gate, never weighted (same outcome as GDL's drop, now via `kind`; KPI-GDL-2 holds). | Both rules at one choke point |
| UC-7 | "Index/search label links 'GitHub side not checked here'" | ADR-085 §8 | **Generalized**: index rows of `github:` assertions render "authority not checked here". Indexer unchanged (C-GDL-8 kept). | Same federation stance |
| UC-8 | Migration v7 `identity_link_checks` | ADR-085 §7 | **No change**; this feature adds v8 (`repo_authority_checks`, slice-5 `imported_candidates`). If GDL and this feature deliver together, DELIVER may fold both into one migration. | Migration numbering |

## Changes to other prior waves

| # | Prior decision | Source | Change | Impact |
|---|---|---|---|---|
| UC-9 | D-1 "Inferred adherence is stored as SIGNED inferred claims"; any user signs `github:<login> adheresToPhilosophy X` | contributor-philosophy-inference D-1, ADR-063/064 | **Narrowed**: `infer people --sign` signs only for the signer's own linked login. For everyone else inference is candidates-only. | Largest product change; A-8 |
| UC-10 | D-2 "Only SIGNED repo claims feed inference (own ∪ peers)" | CPI D-2 | **Unchanged rule**, but signed repo claims now come from repo authorities, so inputs shrink (R-1 sparsity risk rises). Inputs are not filtered by authority (candidates are unsigned; labels show each source claim's authority). | Sparser inference |
| UC-11 | Weighted view counts every claim; triangulation across distinct authors | ADR-022, WD-77 | **Default narrowed** via `score_attested`: weight only assertions that are non-`github:` or Attested; counters/retractions/links never weighted; `--include-unattested` shows unattested assertions separately. Formula unchanged. | Weighted views change for existing users |
| UC-12 | Scraper human gate: the human signs any candidate | WD-49, ADR-017, I-SCR-6 | **Gated**: `scrape github --sign` and `scrape person --sign` sign only permitted candidates; refused ones print the hand-off. Single publish path unchanged. | Behavior change for non-owners |
| UC-13 | Review app ownership checked at scan time only | ADR-076, `routes/publish.rs` | **Extended**: publish re-checks bio + repo authority in the request. Scan scope (`type=owner`) unchanged. | +1 GitHub request per publish request + 2 per repo (3 for one repo) |
| UC-14 | "Person-level claims come later" | review-app DD-9 | **Unchanged**; when added, they follow the person rule (own login only). | — |
| UC-15 | `GithubPort` methods; `build_client()` follows redirects to any host | ADR-063, WD-51, `adapter-github/src/client.rs` | **Extended** with `read_repo_authority` (public `/repos` + one fixed-path contents read, always both). **New redirect policy**: ≤ 1 redirect, same host only (also serves ADR-085 §4). Public-data-only kept. | One more allowlisted endpoint; redirect behaviour changes for every `adapter-github` read |
| UC-16 | Counter-claims (subject/predicate/object copied, reason required), "shown, never applied" | ADR-015 | **Exempt from the gate**, and **excluded from scoring weight** by `kind` (they appear to be weighted today, A-13). | Weighted views drop counter rows |
| UC-17 | Retraction | ADR-008, ADR-060 | **Exempt from the gate**; never refused; never weighted (acts only through the lineage). | Weighted views drop retraction rows if any are fed today |
| UC-18 | I-9 "Compose and sign MUST succeed with network disabled" | brief I-9, WS-10 | **Proposed exception** (ADR-086 §5): a `github:` assertion without a fresh local authority check is refused offline; signing itself never uses the network. Brief I-9 row annotated. | WS-10 scope unchanged (non-`github:` subject) — DISTILL to confirm |
| UC-19 | "Import" means printed candidates (CLI) | ADR-017 | **Extended (slice 5, if A-11)**: imported candidates persist locally, never federated. | One table, one read verb |
| UC-20 | Scoring feed has no reference column | `adapter-duckdb/src/graph_query.rs`, ADR-022 | **Extended**: `AttributedClaim` carries reference types so the pure `kind` can exclude non-weight kinds. Anti-merging SQL rule unchanged (still per-claim, author projected). | Fixes the inferred counter-weight bug (A-13) |
| UC-21 | Share post drafts from all published claims | review-app share (`review-domain/src/share.rs`) | **Narrowed**: quotes only `Attested` claims. | Share text changes |
| UC-22 | Inline `IdentityPort::sign` in four claim verbs | `claim_add.rs`, `claim_counter.rs`, `claim_retract.rs`, `sign_batch.rs` | **Refactored** behind `sign_permitted`; `publish_signed_claim` takes a permit. | Touches every claim signing verb |