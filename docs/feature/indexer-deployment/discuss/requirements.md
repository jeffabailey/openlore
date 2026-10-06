# Requirements: indexer-deployment

> Light DISCUSS. Binding user decisions are in `../wizard-decisions.md`. Decisions and sequencing
> are in `wave-decisions.md`. The requirements are solution-neutral, and mechanisms belong to
> DESIGN/DEVOPS (open questions OQ-IXD-*).

## Problem

`openlore-indexer` is built, tested and fault-isolated (ADR-077/078), but it runs nowhere. With
no image, service, schedule or alert, `openlore search` has no live endpoint. Maria's network
search falls back to local results only, and the authors that the review app brings in
(bsky.social users such as Priya Raman) stay undiscoverable.

## Personas

| Persona | Who | Wants |
|---|---|---|
| **Maria** (searcher, P-001/P-002) | A developer who cares about reproducible builds and does not know whom to follow | To run `openlore search` against a live public index and find claims from authors on many PDSes |
| **Jeff** (operator) | The sole maintainer, working from his laptop with AWS SSO, `gh` and `cosign` | To deploy and roll back by digest, edit the DID list without a redeploy, be emailed only when the indexer is really broken, and see when the last good pass ran |
| Priya Raman (author, beneficiary) | A bsky.social user, `did:plc:priyaraman7x2k` | To have her approved self-attested claims discoverable within half an hour |
| Dmitri Volkov (author, beneficiary) | A self-hosted PDS user, `did:plc:dvolkov3m9q` at `https://pds.volkov.dev` | Same as Priya |

The data (DIDs, times, counts) is illustrative but realistic.

## Jobs

- **J-005** (discover signed claims across the network). A live index is what serves J-005a at
  network scale. Beneficiary: **J-009** (Priya's curated claims become discoverable).
- Operator-only stories are `infrastructure-only` (see the rationale in each story).

## Functional requirements

| ID | Requirement | Story |
|---|---|---|
| FR-IXD-1 | The index search is reachable publicly at `https://index.openlore.jeffbailey.us` over TLS, with a publicly trusted certificate. Plain HTTP redirects to HTTPS. | US-IXD-001 |
| FR-IXD-2 | Only the read-only search method (`org.openlore.appview.searchClaims`), plus an optional minimal health response, is reachable publicly. Every other path or method is refused, and no request can change the index. | US-IXD-001 |
| FR-IXD-3 | An ingest pass runs every 15 minutes, unattended, and resumes after a host reboot or instance replacement plus redeploy. | US-IXD-002 |
| FR-IXD-4 | Passes never overlap. If a pass is still running when the next is due, the next one does not start a second concurrent pass. | US-IXD-002 |
| FR-IXD-5 | A pass and search share one index store. Search answers during a pass, and a pass is never refused because search holds the store. | US-IXD-002 |
| FR-IXD-6 | The DID list comes from an operator-edited SSM parameter. An edit takes effect on the next pass, with no redeploy or restart. | US-IXD-003 |
| FR-IXD-7 | If the list cannot be read at pass time, the pass uses the last good list. It never runs silently with an empty list because of a read failure. | US-IXD-003 |
| FR-IXD-8 | A malformed DID list refuses the pass with exit 2 and a message that names the bad entry (existing ADR-078 §6 behavior). Previously indexed claims stay searchable. | US-IXD-003 |
| FR-IXD-9 | The operator is emailed (existing SNS topic) when 2 consecutive passes exit 3, and when any pass exits 2. Exit 0, including a partial skip, never emails. A recovery is notified. | US-IXD-004 |
| FR-IXD-10 | With one laptop command, the operator sees the time of the last successful pass, its `pass_summary` counts, and how long ago it ran. | US-IXD-005 |
| FR-IXD-11 | Deploys pin a CI-built, signature-verified image by digest. Rollback to the previous digest is one command. | US-IXD-001, US-IXD-006 |

## Non-functional requirements

| ID | Requirement | Measure |
|---|---|---|
| NFR-IXD-1 Freshness | A new claim on a listed author's PDS is searchable within **30 minutes** (one 15-minute interval plus the pass duration). | Time from publish to first search hit, over 5 samples |
| NFR-IXD-2 Availability | Public search at **99% monthly**, the same target as the review app. A deploy interrupts search for **≤ 30 s**. | Probe from the laptop or nightly smoke |
| NFR-IXD-3 Latency | `openlore search` against the public index returns in **≤ 1 s p95** from Maria's laptop, at under 50 authors, including during a pass. | 20 timed searches, 10 of them during a pass |
| NFR-IXD-4 Memory | The indexer processes together peak at **≤ 256 MB**. Host `MemAvailable` stays **> 128 MB**, and swap-in is about 0, during a pass that runs alongside a review-app scan. The PDS `/xrpc/_health` answers 200 throughout. Otherwise move to t4g.small. | Re-measure gate (US-IXD-006), the same signals as review-app R-5 |
| NFR-IXD-5 CPU | 96 passes a day do not drain the burstable credits. `CPUCreditBalance` does not trend down over 24 h. | `AWS/EC2 CPUCreditBalance` (free) |
| NFR-IXD-6 Cost | **≤ $2/month** added, not counting the t4g.small fallback. | The DEVOPS cost table |
| NFR-IXD-7 Abuse posture | A burst of 100 search requests in 10 s from one client cannot make the PDS health check fail or push the indexer past its memory cap. Per-request result size and request size are bounded. | Load burst from the laptop |
| NFR-IXD-8 Isolation | Indexer containers get no AWS credentials, no `/pds` mount, a read-only root filesystem, a non-root user and no added capabilities. The only writable path is the index data directory. | Compose review and the existing mount guard |
| NFR-IXD-9 No regression | The PDS and the review app keep serving during the first deploy, later deploys and rollbacks. The module's compose project is unchanged beyond v1.7.0. | PDS `_health` and review-app `/healthz` checked before and after each deploy |

## Invariants carried in

- I-IXD-1 Capability boundary (ADR-023 / I-AV-5): the indexer is signing-incapable and holds no
  local store or secrets.
- I-IXD-2 Verify-before-index, anti-merging and the ADR-071 verdict are unchanged. This feature
  deploys the binary without changing it.
- I-IXD-3 Laptop-initiated deploys with no CI AWS roles (ADR-068/075).
- I-IXD-4 Observability is structural only (WD-105). Logs carry DIDs, URLs, counts and reasons,
  never claim content.

## Dependencies

| ID | Dependency | State |
|---|---|---|
| DEP-IXD-1 | tofu-aws-pds v1.7.0 (M-1 Caddy sites mount, M-2 IMDS hop limit 1) and R-REPLACE | Planned, shared with the review-app go-live. See the sequencing in `wave-decisions.md`. |
| DEP-IXD-2 | Existing SNS topic `openlore-pds-backup-alarm` | Exists, subscribed |
| DEP-IXD-3 | Wildcard DNS `*.openlore.jeffbailey.us` (Cloudflare, DNS only) | Exists |
| DEP-IXD-4 | CI image pipeline pattern (arm64, cosign keyless, GHCR public) | Exists for the review app. Extended for the indexer. |
| DEP-IXD-5 | Indexer exit codes 0/2/3 and events (`pass_summary`, `source_skipped`, `source_fallback`, `config.loaded`) | Delivered (ADR-078) |

## Open questions for DESIGN / DEVOPS

| ID | Question |
|---|---|
| OQ-IXD-1 | How do ingest and serve share `index.duckdb`? Options: a per-burst open like ADR-065 in the index store adapter; the pass builds a new file and swaps it in atomically; or `serve` performs the pass when the timer signals it. The requirement is FR-IXD-5 and NFR-IXD-3. |
| OQ-IXD-2 | One image for both verbs (expected, since it is one binary). One long-lived container plus a one-shot run per pass, or an exec into the serve container? |
| OQ-IXD-3 | How does the host render the SSM parameter into a read-only file each pass and keep the last good copy (FR-IXD-7)? Standard String parameter or SecureString? (The list is not secret.) |
| OQ-IXD-4 | **Needs a user decision.** Should there be an alert on "no pass ran for more than 1 h" (a dead timer) or "serve down"? The user's alert set cannot see either. Options: fold the indexer into the existing `host.health` timer line, or set `treat_missing_data = breaching` on a pass heartbeat. Either costs about $0.10/month per alarm. |
| OQ-IXD-5 | Should freshness also be public (a minimal field Maria can see), or operator-only? |
| OQ-IXD-6 | Should the CLI default `indexer_url` to the public index? This is a product decision outside this feature (WD-IXD-9). |
| OQ-IXD-7 | Is per-IP rate limiting needed? It would need a custom Caddy build, and Caddy is module-owned. An in-process concurrency cap may be enough for NFR-IXD-7. |
| OQ-IXD-8 | What are the exact memory caps for serve and for a pass? Measure them on the host. |
| OQ-IXD-9 | Removing a DID does not purge its indexed rows (ADR-024 has no delete). Is "delete `index.duckdb` and let the passes rebuild it" an acceptable purge runbook? |
