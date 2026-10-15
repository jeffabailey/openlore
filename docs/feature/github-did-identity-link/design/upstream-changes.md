# Upstream Changes: github-did-identity-link (DESIGN, 2026-10-14)

This feature has no DISCUSS wave. These are the prior-wave decisions and assumptions it changes or
realizes. Accepted by the user with the design on 2026-10-14. Further revisions from
owner-only-signing are listed in `docs/feature/owner-only-signing/design/upstream-changes.md`
(UC-1..UC-8, applied 2026-10-14).

| # | Prior decision / assumption | Source | Change | Impact |
|---|---|---|---|---|
| UC-1 | "Linking a GitHub login to an ATProto DID / identity merging — future, opt-in" (out of scope) | contributor-philosophy-inference feature-delta l.792, OD-CPI-1 l.834 | **Realized** as an opt-in, owner-signed `sameAs` claim (ADR-084). "Identity merging" stays out: association is display-only (ADR-085 §5). | None to shipped CPI behavior |
| UC-2 | "The stable numeric GitHub user id is stored LOCALLY on the contribution link only — it is NOT part of the signed payload" | ADR-064 §1 | **Extended**: the id IS signed in identity-link records (`evidence[]` `https://api.github.com/user/<id>`). Adherence claims are unchanged (still no id). | ADR-064 text should note the exception |
| UC-3 | "The existing `graph query` and scorer read the new signed claims as-is" / "Changes to the scoring formula" out of scope | CPI out-of-scope l.793; ADR-022 | **Amended**: `scoring::score` drops every `sameAs` claim before grouping (else a link becomes a (DID, `github:<login>`) pairing); one choke point for the CLI and viewer callers. Formula unchanged. | ADR-022 consequence note |
| UC-4 | Pure ownership matcher lives in `review-domain` | ADR-076 §4 | **Relocated** to `claim-domain::identity_link`; `review-domain` delegates; behavior identical (existing proptests are the guardrail). | Touches a shipped crate |
| UC-5 | "Person-level claims come later" (DD-9) | bluesky-claim-review-app design DD-9 | **Partly realized**: the review app can publish one person-level record kind — the identity link. Person adherence claims from the app remain later. | Review-app UI addition |
| UC-6 | `github_links` holds one verified link per DID and one owner per GitHub id | ADR-076, `ports/src/review.rs` | **Unchanged as private state.** On the wire, several links per DID are allowed (A-4); several DIDs per GitHub id are representable and shown as Contested, never resolved. | Projection rule, not schema |
| UC-7 | "Contribution links are not published/federated" | OD-CPI-2 | **Unchanged.** Links do not federate contribution data; only the owner's `sameAs` record is public. | — |
| UC-9 | `GithubPort` reads people by login only (`read_person`) | ADR-063, ADR-076 | **Extended** with `read_person_by_id` (`GET /user/{id}`) so renames are told apart from re-registrations (A-10). | One more endpoint on the `adapter-github` allowlist |
| UC-10 | ADR-076 verdict `IdentityChanged` disproves ownership | ADR-076 §4, `ownership.rs` | **Unchanged in the review app.** For published links the id read resolves it (rename → match the new profile's bio), so `identity_changed` is never recorded as a link check. | Projection rule only |
| UC-8 | `scrape github <user>` reads `/users/{user}` for display only | ADR-063 / CPI US-CPI-005 | **Extended**: the same response refreshes the proof check of already-published links for that login (zero extra requests). It never creates or suggests a link. | One extra local write |
