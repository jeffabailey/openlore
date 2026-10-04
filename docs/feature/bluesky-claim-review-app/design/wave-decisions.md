# Wave Decisions: bluesky-claim-review-app (DESIGN)

- **Mode**: propose (autonomous subagent). The user answered the discovery questions on 2026-10-04
  (time to market + low operations cost; under 50 users; co-locate on the PDS host; 1 developer +
  agents).
- **Architect**: Morgan (nw-solution-architect).
- **Paradigm**: functional Rust (ADR-007).
- **Style**: the existing modular monolith with ports and adapters, plus a third composition root.

## Decisions

| ID | Decision | ADR | Resolves |
|---|---|---|---|
| DD-1 | Self-attested provenance is inferred from the record's shape plus its origin: no `signature`, a bare-DID `author` equal to the repo DID, fetched from the author's DID-document PDS. No lexicon field is added. The CID path and app-signed verify are unchanged. A relay origin fails closed until commit-proof verification exists. | ADR-071 | OD-BRA-1 |
| DD-2 | The new binary `openlore-review-app` is the third composition root, with a disjoint capability set: no keychain signing, no user local store, no app-password PDS writer. The hand-rolled hyper server stays; `axum` stays banned. | ADR-072 | OD-BRA-4 |
| DD-3 | Confidential OAuth client (`private_key_jwt`, ES256) via `atrium-oauth` (MIT, default features off, rustls `reqwest`). Create-only `UserRepoWritePort` (claim + post). Granular `repo:` scopes are the target, with `transition:generic` as the fallback. Post permission is requested at sign-in, with no incremental request. | ADR-073 | OD-BRA-2 (client), OD-BRA-7 |
| DD-4 | Private state lives in a separate `review-app.duckdb`, owner-scoped at the type, SQL-rule and probe layers. Secrets are AEAD-encrypted. Data is retained until disconnect. A declined key is never re-offered in v1. KPIs are aggregate-only. No backup in v1 (flagged). | ADR-074 | OD-BRA-5, OD-BRA-11 |
| DD-5 | Co-located on the PDS host at `app.openlore.jeffbailey.us` (already covered by the wildcard DNS). A container on the compose network, behind Caddy, via a generic `import /pds/caddy/sites/*.caddy` hook in tofu-aws-pds v1.7.0. Laptop-initiated SSM deploys. | ADR-075 | OD-BRA-2 (hosting) |
| DD-6 | One server GitHub PAT (public read). An in-process budget: global remaining-floor 300, at most 2 concurrent scans; per DID, 1 concurrent scan and 6 scans/day, 20 verifies/hour, 100 publishes/day, 5 posts/day; per IP, 20 sign-ins per 10 minutes. Exact-token DID bio matcher. A `VerifiedOwnership` capability gates every scrape. | ADR-076 | OD-BRA-3, OD-BRA-10, OD-BRA-12 |
| DD-7 | Plan-value pattern for every PDS write (publish, retract, share). A stored plan is taken exactly once on confirm, and the result is read back and CID-verified. | arch-design §6.3 | I-BRA-3, AC-004.4 |
| DD-8 | The profile page `${app_origin}/@<handle>` (a DID also works) reads live from the user's PDS with no cache. Its handler is given no private-store port. It is independent of the J-007 card. Share links use the handle form. | arch-design §6.5 | OD-BRA-8 |
| DD-9 | Repo-level subjects `github:<login>/<repo>` with `embodiesPhilosophy` in v1 (D-10). Person-level claims come later. | — | OD-BRA-6 (confirmed) |
| DD-10 | Scans run as in-process tokio tasks with per-repo persistence and resumable `scan_runs`. No queue. | ADR-076 | OD-BRA-3 |

## Spikes (run before or at the start of DELIVER)

| Spike | Question | Pass criterion | Blocks |
|---|---|---|---|
| **SPIKE-1** (OD-BRA-9) | Do bsky.social and the OpenLore PDS (pinned image) accept `org.openlore.claim` with a CID rkey, `validate` unset? Does `listRecords` return the record unchanged? | `createRecord` returns 200 with `validationStatus: unknown`; the read-back recomputed CID equals the rkey on both PDSes; `app.bsky.feed.post` with a link facet renders on bsky.app | US-BRA-004 (walking skeleton) |
| **SPIKE-2** | Does the `atrium-oauth` confidential flow work end to end (PAR, DPoP nonce, `private_key_jwt`, refresh, revoke) with `default-features=false` and a rustls client, and does `cargo deny check` pass? | Sign in, then createRecord, then refresh, then revoke against bsky.social and the OpenLore PDS; deny passes. On failure, switch to `atproto-oauth` (ADR-073 fallback). | US-BRA-001 |
| **SPIKE-3** | Are granular scopes (`repo:org.openlore.claim?action=create`, `repo:app.bsky.feed.post?action=create`) accepted by both PDSes, and are writes outside them refused? | Both accept, and a write outside the scope is refused. Otherwise use `transition:generic` plus disclosure. | NFR-BRA-3 |
| **SPIKE-4** | Does Caddy accept an `import` glob that matches no files? What is the app's RSS during a 10-repo scan next to the PDS on t4g.micro? | Caddy starts with an empty `sites/`; app RSS under 150 MB and no OOM. Otherwise move to t4g.small. | DEVOPS rollout |
| SPIKE-5 (later) | Commit-proof verification (`atrium-repo` + `atrium-crypto` + a proof walker) | Verifies a `sync.getRecord` CAR against the `#atproto` key | Only if relay/firehose ingest is adopted (ADR-071 trigger) |

## Risks

| Risk | P | I | Mitigation |
|---|---|---|---|
| `atrium-oauth` API churn or a bug (0.1.x) | M | H | Behind `OAuthPort`; pinned; SPIKE-2; fallback library |
| Granular scopes unavailable, so we over-grant | M | M | Create-only port (type layer); disclosure; config switch later |
| A third-party PDS strips or rejects unknown lexicons | L-M | H | SPIKE-1; read-back verify makes the failure loud |
| Shared fate with the PDS on 1 GiB | M | M | Memory caps; SPIKE-4; t4g.small fallback (stop/start) |
| The module hook needs an instance replacement | H | L | Option A (planned replacement after a verified identity backup) or B (hot patch); Jeff decides |
| Verify-path change regresses app-signed readers | M | H | `verify` unchanged; AC-009.4 guardrail; the existing suites must stay green |
| Losing private state loses declines, so declined suggestions get re-offered | L | M | Accepted v1 posture, flagged; a backup option is documented |
| GitHub token shared across users | L | L | Per-DID limits; remaining-floor pause |
| `app.` label collides with a future handle | L | M | Reserve the label in the runbook |

## User decisions (2026-10-04)

1. **ADR-075 rollout**: option A. Do one planned instance replacement, after a verified identity
   backup, to adopt the module v1.7.0 import hook. No hot-patch.
2. **Scope fallback**: if SPIKE-3 fails, use `transition:generic` and say on the landing page why
   the broad permission is needed. Do not block the release.
3. **Backups of private state**: none in v1. If the volume is lost, a declined suggestion can be
   offered once more. Approved claims are safe in users' own PDSes.
4. **Origin**: `app.openlore.jeffbailey.us`, confirmed.

## Upstream changes

None to DISCUSS decisions. D-1..D-12 and I-BRA-1..8 are honored as written. OD-BRA-1..12 are all
resolved or turned into spikes.
