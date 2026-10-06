# DESIGN wave decisions: indexer-per-did-pds-fetch

- **Mode**: propose (autonomous). Drivers: time-to-market and low ops cost. Hard requirements:
  ADR-024 fault isolation and ADR-071 provenance honesty. Solo maintainer + agents (Conway: one
  team, one modular monolith, no boundary conflict).
- **Style**: unchanged (modular monolith, ports and adapters, functional Rust). No new crate,
  no schema change, no new crate in `Cargo.lock`.

## Decisions

| ID | Decision | ADR |
|---|---|---|
| DD-IPF-1 | Resolve every repo DID every pass via `IdentityLookupPort::resolve_pds` (a new default method; `IdentityLookup` overrides it with a document-only fetch). List at the resolved PDS. | ADR-077 |
| DD-IPF-2 | `ListingSource = OwnPds(PdsEndpoint) \| Fallback(FallbackUrl)`. `origin_of` gives `AuthorPds` only for `OwnPds` on exact match. `Fallback` is always `Relay`, with no URL comparison (OQ-IPF-3). | ADR-077 |
| DD-IPF-3 | `OPENLORE_INDEXER_SOURCE_URL` keeps its name (**confirmed by the user, 2026-10-05**) and is documented as the optional fallback, used only for resolution failures. | ADR-077 |
| DD-IPF-4 | `did:plc` and hostname-only `did:web` are supported (the existing resolver). Other methods or malformed DIDs refuse start (OQ-IPF-2). | ADR-077 |
| DD-IPF-5 | **SSRF guard (user, 2026-10-05).** https only. Refuse 0.0.0.0/8, 127/8, 10/8, 172.16/12, 192.168/16, 169.254/16, `::`, `::1`, fc00::/7, fe80::/10, checked **after DNS** in a guarded `reqwest` resolver: the connection goes to the checked IP, so it is rebinding-safe. IP literals are pre-checked. No redirects. This applies to DID documents, PDSes and the fallback. A refused resolved PDS → skip `pds_address_refused` (no fallback). TEST-ONLY `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1` admits http + loopback in debug builds; a release build refuses start (review-app pattern). | ADR-077 |
| DD-IPF-6 | One DID = one failure unit. Reasons: `did_unresolvable`, `pds_unreachable`, `pds_timeout`, `listing_failed`, `pds_address_refused`. **Exit codes (user, 2026-10-05)**: 0 when completed with ≥ 1 DID listed (partial skips included) or none configured; **3 when every configured DID is skipped**, after `pass_summary`; 2 for startup refusal, runtime construction failure, or an upsert failure (OQ-IPF-5). | ADR-078 |
| DD-IPF-7 | Fan-out: `futures-util` `buffered(cap)`, default cap **4** (1..=16). Per-DID single deadline **30 s** (1..=600). Two phases (fetch, then sequential gate). **Defaults confirmed by the user (2026-10-05).** | ADR-078 |
| DD-IPF-8 | No startup network probe. Config validation refuses with the variable and value. A pure `origin_classification_probe` is added. The `IngestSourcePort` probe no longer requires a source (OQ-IPF-1). | ADR-077/078 |
| DD-IPF-9 | Events: `source_skipped`, `source_fallback`, `pass_summary`, `config.loaded`, `rejected.by_reason.foreign_repo`. `stats` is not extended (OQ-IPF-4). | ADR-078 |
| DD-IPF-10 | `SearchResultDto.provenance: Option<String>`, always set by the server. Absent = app-signed. Unknown = row hidden with a notice. CLI shows `[verified] [self-attested]`. | ADR-079 |
| DD-IPF-11 | ADR housekeeping: ADR-024 is marked partly superseded and ADR-071 amended (status notes). ADR-077..079 are Proposed (OQ-IPF-6). | — |
| DD-IPF-12 | New check_arch rules `indexer_origin_only_via_listing_source` and `indexer_guarded_clients_only` (specified, not implemented). | component-boundaries.md |

## Resolved DISCUSS inconsistency

The US-IPF-003 elevator pitch prints `source_skipped ... fallback_used:true` for a DID whose
claims *were* read through the fallback. That conflicts with the summary arithmetic
(own_pds + fallback + skipped = configured) and with AC-003.3. **Design decision**: a DID read
via the fallback emits `indexer.ingest.source_fallback`. `source_skipped` with
`fallback_used: true` (plus `fallback_failure`) is emitted only when the fallback listing also
failed. DISTILL should write the ATs against this.

## Regression analysis (NFR-3, AC-004.4: single-source deployment)

Precondition: every configured DID's resolved PDS base equals the old source URL.
- Before: list(source) → `origin_for` compared source with the resolved PDS → `AuthorPds` when
  they matched, else `Relay`.
- After: list(resolved PDS = the same base) → `origin_of(OwnPds)` → `AuthorPds`. The records,
  the origin, the verdicts and the indexed set are identical.
- With PLC down: before, self-attested was `Relay` (refused) and app-signed was indexed. After,
  the fallback is the same URL, the source is `Relay`, and the outcome is identical.
- Visible differences, both intended: the `[self-attested]` label (ADR-079); new events; and
  `rejected.count` now includes `foreign_repo` (records of other repos were previously dropped
  silently, and none were indexed before or after).

## Sizing

The DISCUSS estimate of 7–9 days holds. US-IPF-002 is about 2.5 days, with no split needed: the
bounds are one combinator and one deadline. The thin slice is still US-IPF-001.

## Risks

| ID | Risk | Mitigation |
|---|---|---|
| R-IPF-1 | Third-party load on bsky.social PDS hosts | Cap 4, unchanged page bound, cadence unchanged, one PLC request per DID. 429 is surfaced as `pds_unreachable` detail. |
| R-IPF-3 | Exit 3 (total outage) is usually transient, but a naive supervisor may treat it as a hard failure | Document 3 as "retry next interval" versus 2 as "fix the deployment". DEVOPS alerts on repeated 3s. A partial outage still exits 0, so watch `pass_summary`. |
| R-IPF-4 | Author-key resolution in the gate phase is outside the per-DID budget | Unchanged behaviour, bounded by the resolve adapter's client timeout. Future: memoize keys per pass (distinct authors). |
| R-IPF-5 | `NetworkResultRowRaw` field addition touches many fixtures | Mechanical. Default `AppSigned` in the strategies. |
| R-IPF-6 | did:web with percent-encoded port (`did:web:host%3A8080`) is not resolvable | Not permitted by ATProto for production. It yields `did_unresolvable` and is fallback-eligible. |
| R-IPF-7 | SSRF via a DID document (or DNS rebinding) pointing the indexer at internal services or cloud metadata | DD-IPF-5 guard, applied after DNS and connecting to the checked IP. Residual risk: the guard depends on every outbound client being a guarded one. The xtask rule `indexer_guarded_clients_only` plus guard tests cover it. |
| R-IPF-8 | Deployments whose fallback or PLC mirror is on a private address, or an author PDS served over plain http, stop working | Deliberate (user decision). A fallback or PDS address surfaces as `pds_address_refused` / `fallback_failure` / exit 3. A PLC endpoint the policy refuses (http, an IP literal that is private, userinfo) is refused at startup, naming the variable (fix-indexer-follow-ups D2). Before that fix, with a fallback set, a refused PLC endpoint was silently read through the fallback as relay origin. A PLC hostname that resolves only to private addresses is still refused at runtime. Production PLC (`plc.directory`) and bsky.social hosts are public https. |
| R-IPF-9 | The test-only override leaks into production | It is refused at startup in release builds (`BuildProfile` from `cfg!(debug_assertions)`), and admits loopback only, never private ranges. |

## User decisions (2026-10-05): resolved

1. Fan-out defaults **confirmed**: cap 4, 30 s per DID, env-tunable.
2. **Changed**: total outage (every configured DID skipped) exits **3**, after `pass_summary`.
   Partial skips exit 0. Exit 2 is kept for local faults (config, probe, store).
3. **Changed**: full SSRF guard (DD-IPF-5) with skip reason `pds_address_refused` and a
   test-only, release-refused loopback override.
4. Env name `OPENLORE_INDEXER_SOURCE_URL` **confirmed** and documented as the fallback.

Nothing is left open for the user.

## AC additions for DISTILL (from the user decisions)

- **AC-002.8**: when every configured DID is skipped (`configured ≥ 1`, `own_pds + fallback == 0`),
  `openlore-indexer ingest` emits `pass_summary` (`exit_code: 3`) and then exits 3. With at
  least one DID listed it exits 0. With zero DIDs configured it exits 0.
- **AC-002.9**: a DID whose resolved PDS is plain `http`, a refused IP literal, or a hostname
  resolving only to refused addresses is skipped with `pds_address_refused` and `pds_url`. No
  connection is made to the refused address, and the fallback is not contacted.
- **AC-003.4**: a fallback whose hostname resolves only to refused addresses skips the affected
  unresolvable DIDs with `reason: did_unresolvable, fallback_used: true, fallback_failure:
  pds_address_refused`. A fallback URL that is non-https or a refused IP literal refuses start
  (variable + value).
- **AC-004.5**: `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1` on a release build refuses start,
  naming the variable. On a debug build it admits `http://127.0.0.1` fakes only (a fake
  advertising `http://10.0.0.1` is still refused).
- Test infrastructure note: the indexer ATs set the override (debug build) for the 127.0.0.1
  fakes, as the review-app harness does with `REVIEW_APP_ALLOW_LOOPBACK_HTTP`.

## Handoff

- **DISTILL**: architecture-design.md §6/§9, data-models.md §4/§5 (config and events are the
  assertion surface), the resolved inconsistency above.
- **DEVOPS**: route the 4 new events. Distinguish exit 3 (all sources failed; alert if it
  repeats) from exit 2 (local fault). Optional tuning vars. Never set
  `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP` in production (a release build refuses it).
  Contract tests are recommended for PDS `listRecords` (bsky.social hosts) and PLC DID-document
  shape.
- **Paradigm**: functional, via `@nw-functional-software-crafter`.
