# Shared Artifacts Registry — bluesky-claim-review-app

Every `${variable}` in the journey mockups has exactly one source of truth. "Private app
state" means server-side state readable only by the owning DID's session (I-BRA-1). DESIGN
chooses the store (OD-BRA-5).

| Artifact | Source of truth | Consumers (journey steps / surfaces) | Owner | Integration risk | Validation |
|----------|-----------------|---------------------------------------|-------|------------------|------------|
| `app_origin` | Deployment config (DESIGN, OD-BRA-2) | OAuth client metadata URL, landing page, profile URL, share-post link | hosting | **HIGH**: OAuth client id is a URL; a mismatch breaks sign-in | Client metadata URL is served from `app_origin`; link checks in share preview |
| `handle` | ATProto handle resolution of user input; refreshed from session | 1, 2, 4 (success copy), 9 profile, 10 post author | identity | MEDIUM: handles change; never used as a key | Displayed only; all keys use `user_did` |
| `user_did` | OAuth session subject (single source) | 3 (copy-to-bio text), 4 (ownership check), 6/7 (record repo), 9 profile, private-state key | identity | **HIGH**: the copy-to-bio DID, the verified DID and the writing DID must be identical | Copy text == session DID == ownership check DID == `at://` authority |
| `pds_endpoint` / `pds_host` | DID document `#atproto_pds` service of `user_did` | 7 preview ("Host: your PDS"), write target | identity | **HIGH**: writes must go to the user's OWN PDS, never openlore.jeffbailey.us (D-6) | Write target == DID-doc service endpoint |
| `github_login` | User input, confirmed by public GitHub profile fetch | 4, 5 scan, 7 subject `github:<login>/<repo>`, 9 | ownership | MEDIUM: login renames | Store the GitHub numeric user id with the link (as in OD-CPI-1) |
| `ownership_link` | Private app state `(user_did, github_login, github_user_id, verified_at)` | 4 success, scan gate, US-BRA-010 rescan | ownership | **HIGH**: the gate for I-BRA-4; per-DID only | No scan without a link for the session DID |
| `suggestion` (subject, predicate, object, default confidence, evidence, signal) | Private app state, derived by the existing scraper-domain pipeline (`scrape person` per-repo beats + J-004 signal→predicate mapping) | 5 scan list, 6 queue cards, 7 preview | review | **HIGH**: must reuse the shipped derivation (I-SCR-4/5), not fork it | Card "Why" names the signal; evidence URL equals the scraper's |
| `suggestion_key` | `(user_did, subject, predicate, object)`: identity for dedupe and suppression | queue, rejection set, rescan | review | MEDIUM: key shape is DESIGN (OD-BRA-5) | Rescan never duplicates; a declined key is never re-offered |
| `confidence` | Default 0.25 (jobs.yaml J-004 mapping) or owner edit; written as integer basis points (ADR-070) | 6 card, edit, 7 preview ("0.70 (stored as 7000)"), 9 profile, 10 post | claim | **HIGH**: preview, record and profile must agree; buckets are display-only (WD-10) | Preview value × 10000 == record `confidence`; no bucket in the record |
| `philosophy` (object) | Philosophy vocabulary (`org.openlore.philosophy.*`, J-002e seeds) | 6 card, edit dropdown, 7 preview, 9, 10 | vocabulary | MEDIUM: swap list must use the shared vocabulary | Dropdown values ⊆ known philosophy NSIDs (advisory, never rejecting) |
| `rejection_set` | Private app state keyed by `user_did` (D-4) | 5 suppression, 6 counts, US-BRA-012 purge | review | **HIGH**: must never reach the PDS or a public surface (I-BRA-2) | No PDS write on decline; not readable by another DID |
| `record_uri` / `record_cid` | PDS createRecord response | 8 confirmation, 9 profile entry, retract (US-BRA-011) | claim | MEDIUM | Confirmation URI == PDS-returned URI |
| `provenance_mode` | Derived by the OpenLore verify path: `app-signed` or `self-attested (repo-signed)` (D-5) | 8, 9 profile badge, viewer, search, peer pull | verify | **HIGH**: today's verify path (`crates/claim-domain/src/verify.rs`) expects the app-level signature | Every app-approved claim yields `self-attested`, never "unverified" |
| `profile_url` | `app_origin` + per-user route (OD-BRA-8) | 9, 10 post text/link | profile | MEDIUM | Post link resolves to the same profile |
| `share_post_text` | Generated from published, non-retracted claims of `user_did`, then edited by the owner | 10 preview, 11 post | share | **HIGH**: must never name pending or declined items (I-BRA-1/6) | Post philosophies ⊆ published claims |
| `pending_count` / `approved_count` / `declined_count` | Derived from private state + published records | 6, 8 | review | LOW | Counts sum to suggestions offered |

## CLI and UI vocabulary (consistent across surfaces)

| Term | Meaning | Never say |
|------|---------|-----------|
| suggestion | An unpublished, machine-inferred candidate in the private queue | "claim" (it isn't one until published) |
| approve / publish to my repo | Write a self-attested claim to the user's own PDS | "submit", "save" |
| not me / decline | Private decline; suppresses re-suggestion | "reject publicly", "counter" |
| retract | Public soft-retraction of a published claim (RC-02) | "delete" |
| self-attested | Provenance mode: claim written by the subject's own repo, signed by the PDS repo commit | "unverified", "unsigned" |
