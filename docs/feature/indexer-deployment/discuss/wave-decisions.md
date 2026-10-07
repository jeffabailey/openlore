# DISCUSS wave decisions: indexer-deployment

- **Mode**: light DISCUSS (LIGHTWEIGHT research: happy path plus the key operational failure
  paths). Infrastructure feature (operator-facing deployment of an existing binary) with one
  user-visible outcome: live network search. Brownfield. It reuses the review-app deploy pattern
  (ADR-075, `deploy/review-app/*`, `docs/feature/bluesky-claim-review-app/devops/*`).
- **Inputs**: `../wizard-decisions.md` (binding user decisions, 2026-10-06), ADR-023/024/065/075/077/078,
  `docs/evolution/{indexer-per-did-pds-fetch,fix-indexer-follow-ups,bluesky-claim-review-app}-evolution.md`,
  review-app DEVOPS docs, `crates/openlore-indexer/src/{main,run}.rs`, `crates/cli/src/verbs/search.rs`.
- **DIVERGE artifacts**: none. Not run, by user decision (JTBD: NO). Stories map onto the existing
  J-005. Risk: low. The job and its success criteria were validated in openlore-appview-search.

## Scope Assessment: PASS: 6 stories, 1 bounded context, about 9-12 days

- One context: running the `openlore-indexer` composition root in production. There is no domain
  change. Stories touch packaging (image), host wiring (compose, timer, Caddy site), and
  CloudWatch resources.
- Integration points: GHCR image, host compose and timer, Caddy site, SSM DID parameter, and the
  CloudWatch alarms to the existing SNS topic. That is 5, at the limit.
- No oversized signals. No split needed.

## Decisions

| ID | Decision | Rationale |
|---|---|---|
| WD-IXD-1 | `openlore-indexer` runs co-located on the OpenLore PDS host as containers from one CI-built, signed arm64 image, deployed by digest from the laptop. | Wizard decision. ADR-075 pattern. |
| WD-IXD-2 | `serve` is a long-running service. `ingest` runs as a one-shot pass every 15 minutes from a host systemd timer. | User decision (2026-10-06). |
| WD-IXD-3 | Search is **public and read-only** at `https://index.openlore.jeffbailey.us`, behind the existing Caddy. The wildcard DNS already covers it. Only the search XRPC method is reachable. | User decision. |
| WD-IXD-4 | The DID list is a **static SSM parameter** that the operator edits. The next pass picks it up, with no redeploy. The container never holds AWS credentials. The host turns the parameter into a read-only file. | User decision. Same posture as the review-app secrets. |
| WD-IXD-5 | Alerts go to the existing SNS email in 2 cases: **2 consecutive exit-3 passes** and **any exit 2**. A partial skip (exit 0) never alerts. | User decision. Exit-code semantics are from ADR-078 §2. |
| WD-IXD-6 | Ingest and serve **share one index store safely**. A pass never fails because serve holds the store, and search never fails because a pass is writing. DESIGN picks the mechanism (OQ-IXD-1). | DuckDB allows one read-write holder per file, and a read-only open still conflicts (ADR-065). The local viewer and CLI hit this, and the review app's single process avoided it. Here two processes are guaranteed to collide every 15 minutes. |
| WD-IXD-7 | The index is **re-buildable** (ADR-023). Losing it means rebuilding on the next passes. It is not backed up, and rollback never needs a data restore. | Keeps cost and runbooks small. |
| WD-IXD-8 | Stories trace to **J-005** (US-IXD-001/002/003). The operator-only stories (alerts, freshness, deploy/rollback) are `infrastructure-only`, each with a rationale. Every release slice has at least one J-005 story. | jobs.yaml traceability rule. |
| WD-IXD-9 | Maria reaches the index by setting `OPENLORE_INDEXER_URL` (or `[appview] indexer_url`). It is **not** made the CLI default here (OQ-IXD-6). | Changing CLI defaults is a product decision outside a deployment feature. Local-first stays the default. |
| WD-IXD-10 | Alarm spend stays minimal: 2 new alarms, and about $1-2/month added in total. | Wizard decision ("keep alarm cost minimal"). |

## Sequencing with tofu-aws-pds v1.7.0 and the instance replacement (DEP-IXD-1)

1. The indexer needs **M-1** (the Caddy `sites` mount, for `index.caddy`) and **M-2** (IMDS hop
   limit 1, so the public indexer container cannot get the host role). Both ship in tofu-aws-pds
   v1.7.0. Adopting it needs the one planned instance replacement (R-REPLACE), which is shared
   with the review-app go-live.
2. This feature **adds no module change** and **must not cause a second replacement**. Its own
   resources are the SSM parameter, a host-role read policy for `/openlore/prod/indexer/*`, a log
   group, metric filters and 2 alarms. All of them are in-place creates in the OpenLore
   environment. `check-plan.sh` must show creates only.
3. Order: release v1.7.0, then R-REPLACE (once), then deploy the review app and the indexer in
   **either order**. If the indexer is ready first, it rides the same replacement window. If it
   is ready later, it deploys onto the replaced host with no infrastructure disruption.
4. Host systemd units (the ingest timer) do not survive a replacement. The deploy script
   reinstalls them, and the replacement runbook's last step ("redeploy apps") gains the indexer.

## Risks

- R-IXD-1: memory on the 1 GiB host, with the PDS, the review app, `serve`, and a pass running
  at the same time. Mitigated by a hard re-measure gate (US-IXD-006) and the t4g.small fallback
  (+$6.13/month).
- R-IXD-2: the user's alert set cannot detect a dead timer (no passes at all, so no exit codes)
  or a dead `serve`. US-IXD-005 makes a dead timer visible. Alerting on it is OQ-IXD-4 and
  needs a user decision.
- R-IXD-3: public endpoint abuse. Bounded by NFR-IXD-7. Per-IP rate limiting is OQ-IXD-7.
- R-IXD-4: the `serve` docstring in `main.rs` says it also runs an ingest loop, but the shipped
  body only serves. DESIGN must make sure one process at a time writes (WD-IXD-2).

## User decisions after DISCUSS (2026-10-06)
- **OQ-IXD-4, liveness:** YES. Add one liveness alarm (about $0.10/month) that fires when no `pass_summary` has arrived for 45 minutes (a stopped timer or wedged ingest) or the public /search health check fails (dead `serve`). That makes 3 alarm conditions: 2× exit 3, any exit 2, liveness.
- **OQ-IXD-9, removed DIDs:** PURGE. Claims from an author removed from the DID list are removed from the index on the next pass. Note the conflict with ADR-078's "skips never delete indexed claims": a skip (temporary failure) keeps claims, while removal from the list (an operator decision) purges them. DESIGN must keep the two distinct and add a purge path to the index store (check-arch currently allows DELETE only in purge/expiry files; mirror that). Add or extend a story and ACs.
- **OQ-IXD-6, CLI default:** NOT in this feature. `openlore search` keeps its current default; revisit once the index has been live for a while.
