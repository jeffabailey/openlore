# Evolution: indexer-per-did-pds-fetch

- **Type**: backend, brownfield; changes the behaviour of the existing indexer ingest pass
- **Dates**: 2026-10-05: DISCUSS (`2388cc2`) through the mutation report (`31599f9`)
- **Job**: J-005 (discover signed claims across the network). J-002 and J-009 benefit.
- **Origin**: the "Indexer source" follow-up of
  [`bluesky-claim-review-app-evolution.md`](bluesky-claim-review-app-evolution.md).
- **Design**: ADR-077 to ADR-079, all accepted at finalize. ADR-024 is partly superseded and
  ADR-071 amended (status notes).
  [`docs/architecture/indexer-per-did-pds-fetch/`](../architecture/indexer-per-did-pds-fetch/),
  [walking skeleton](../scenarios/indexer-per-did-pds-fetch/walking-skeleton.md),
  [journey](../ux/indexer-per-did-pds-fetch/). Wave records are in
  `docs/feature/indexer-per-did-pds-fetch/` (`wizard-decisions.md`, `*/wave-decisions.md`,
  `deliver/roadmap.json`, `deliver/execution-log.json`, `deliver/mutation/mutation-report.md`).
- **State**: code complete. Zero new crates, zero schema change, zero new crates in
  `Cargo.lock` (`futures-util` became a direct dependency). The first CI run of the new
  release-guard job is pending at finalize.

## Summary

Until now the indexer listed every DID in `OPENLORE_INDEXER_REPO_DIDS` from one
`OPENLORE_INDEXER_SOURCE_URL`. Under ADR-071 a self-attested record counts as own-PDS only
when it is read from the DID's resolved PDS. So a review-app user on bsky.social published a
claim that the indexer treated as relay-origin and refused, and it never reached
`openlore search`.

Now every pass resolves each repo DID to its own `#atproto_pds` (fresh every pass, never
cached) and lists its `org.openlore.claim` records there. The source URL is an optional
fallback used only for DIDs that can't be resolved, and records read through it are relay
origin by type, so self-attested ones are still refused. One failing DID is skipped with a
reason and the pass carries on. Fetches are bounded (4 concurrent, 30 s per DID), and every
outbound client refuses private and loopback addresses after DNS. Search results now carry
provenance on the wire, and the CLI labels `[self-attested]` claims.

DISTILL wrote 55 scenarios (all committed `#[ignore]`) on one multi-PDS fake network. 8 DELIVER
steps took them to green.

## Business context

- **J-005**: a reader searching the network should find every signed claim, wherever its
  author's PDS is.
- **Unblocks the review app**: claims that bsky.social users approve in
  `openlore-review-app` now appear in network search, labelled self-attested and attributed to
  the author's bare DID.
- **Provenance honesty**: without the wire field (ADR-079), readers would treat a
  self-attested row as app-signed. That would misstate provenance, so the field was in scope.

## Key decisions

User decisions:

| Decision | Source |
|---|---|
| Light DISCUSS, then DESIGN. DIVERGE not run (maps onto J-005). DEVOPS wave skipped. | wizard, DISCUSS |
| `OPENLORE_INDEXER_SOURCE_URL` becomes an optional fallback, used only for unresolvable DIDs. It is always relay origin, with no URL comparison. | WD-IPF-2, DD-IPF-2, ADR-077 |
| The variable keeps its name and is documented as the fallback. | DD-IPF-3 |
| Fan-out: 4 concurrent fetches (1..=16), one 30 s deadline per DID (1..=600), both env-tunable. | DD-IPF-7, ADR-078 |
| Exit 3 when every configured DID is skipped (after `pass_summary`). Partial skips and zero DIDs exit 0. Exit 2 stays for local faults (config, runtime construction, upsert). | DD-IPF-6, ADR-078 |
| SSRF guard: https only; 0.0.0.0/8, 127/8, 10/8, 172.16/12, 192.168/16, 169.254/16, `::`, `::1`, fc00::/7, fe80::/10 refused **after DNS**; the connection goes to the checked IP (rebinding-safe); no redirects. Applies to DID documents, PDSes and the fallback. A test-only loopback seam (`OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1`) works in debug builds and is refused at startup in release builds. | DD-IPF-5, ADR-077 §4 |

Architecture decisions: `IdentityLookupPort::resolve_pds` (a default method with a
document-only override, `did:plc` and hostname-only `did:web`). A pure
`appview_domain::ingest_pass` core: `ListingSource = OwnPds | Fallback`, and only `OwnPds` on
exact match yields `AuthorPds` (ADR-077). One DID is one failure unit, with skip reasons
`did_unresolvable | pds_unreachable | pds_timeout | listing_failed | pds_address_refused`
(ADR-078). An optional `SearchResultDto.provenance`: absent means app-signed, an unknown value
hides the row (ADR-079). Two check-arch rules: `indexer_origin_only_via_listing_source` and
`indexer_guarded_clients_only`.

## Work completed

| Step | What | Commit |
|---|---|---|
| — | DISCUSS (5 stories), DESIGN (ADR-077..079), DISTILL (55 scenarios, all ignored, multi-PDS fake), roadmap (8 steps, ignore tags renumbered) | `2388cc2`, `a07adaa`, `390f2b1`, `3c7a56b` |
| 01-01 | Harness migration: bare repo DIDs and a hermetic local directory for the existing indexer suites | `46bb15b` |
| 01-01 | Walking skeleton: resolve each repo DID to its own PDS every pass (WS-1, WS-2) | `5c39f33` |
| 01-02 | Listings bound to their repo; origin only via `ListingSource`; the fallback is always relay | `bbb858c` |
| 01-03 | Unresolvable DIDs fall back as relay origin; the single-source deployment is unchanged | `67d07ce` |
| 01-04 | One failing author is skipped with a reason; the pass carries on and deletes nothing | `c4aabe8` |
| 02-01 | 4 concurrent fetches, 30 s per author, exit 3 on total outage | `4d1978e` |
| 02-02 | https-only guarded clients refuse private and loopback addresses after DNS | `fe84b99` |
| 02-03 | Typed startup config validation; release builds refuse the loopback seam (CI release-guard job) | `b88e2bf` |
| 02-04 | Search labels self-attested claims; contributor search matches the bare DID | `3aa6073` |
| fix | Viewer test GETs retry transient transport errors | `417ddf3` |

Each step also has a `chore: execution log` commit.

## Quality gates

| Gate | Result |
|---|---|
| Roadmap review | Approved after a re-verdict (see lessons): 2 phases, 8 steps |
| Refactor | L1-L4 pass (`dcde36e`) |
| Adversarial review | APPROVED, 0 defects |
| Mutation (per feature, gate 80%) | `appview-domain` 38.2% → **100%**, `ports` 44.2% → **100%**, `openlore-indexer` (config.rs) 58.8% → **100%** (`4566a0c`). No equivalent mutants. Report: `deliver/mutation/mutation-report.md` (`31599f9`). |
| DES integrity | All 8 steps have complete DES traces |
| CI | The release-guard job (IPF-30 against the release binary) was added in `b88e2bf`; its first result is pending at finalize |

## Issues and lessons

- **Pre-existing bug fixed: one failed listing aborted the whole pass with exit 2.** That
  violated ADR-024's per-source fault isolation. Now one DID is skipped with a reason, and
  previously indexed claims stay searchable (`c4aabe8`).
- **Contributor search missed self-attested authors.** The CLI lifts a bare DID to the
  app identity (`…#org.openlore.application`), but self-attested rows are stored under the bare
  DID. DISTILL flagged it upstream. Fixed with a `starts_with` match on the bare DID in the
  index store (`3aa6073`).
- **The fake ingest server served every repo's records to every listing.** That hid the
  foreign-repo case. It now honours `listRecords repo=` (`bbb858c`). **Lesson:** a fake that is
  more generous than the real service hides whole classes of bugs.
- **Recurring viewer CI flake ("error sending request").** Mitigated with bounded retries on
  viewer test GETs (`417ddf3`). It is still flaky locally under parallel load.
- **DISTILL tests were committed all-ignored, so main stayed green.** This applied the
  previous feature's lesson. DELIVER removed one ignore at a time.
- **The roadmap reviewer flagged planned-but-not-yet-built work (the CI job) as a blocker.**
  A re-verdict cleared it. **Lesson:** a roadmap review judges the plan, not whether the plan
  is already built.
- **Mutation testing found tautological `net_policy` property tests.** They used the
  production functions as their own oracle, so a mutated range predicate went unnoticed. The
  code review missed this. They were replaced with concrete-address tests at every range edge.
  **Lesson:** a property needs an independent oracle.
- **Crafters sometimes skipped dependent suites.** The orchestrator ran them before pushing.
  **Lesson (repeated from the previous feature):** after a shared-crate change, run the
  dependent suites as well as the step's own.

## Open follow-ups

- **No test that earlier rows stay committed when an index-store write fails mid-pass.**
  DISTILL could not do it hermetically from a subprocess; the planned unit test on the gate
  loop is still missing.
- **A fallback after a resolution timeout shares the exhausted per-DID deadline**, so it
  always fails. Give the fallback its own budget, or skip it after a timeout.
- **The guarded DNS resolver is duplicated** in two adapters (about 20 lines). Sharing it
  needs a new crate.
- **Advisory mutation targets not run**: `adapter-index-query` (`decode_wire_provenance`)
  and `adapter-index-store` (contributor `starts_with`). Left for the CI nightly run.
- **First release-guard CI run**: result to be confirmed.
- Recommended in DESIGN and not built: contract tests for the real bsky.social `listRecords`
  and PLC DID-document shapes. Memoizing author keys per pass (R-IPF-4).

## Operator notes

- **Environment variables**:
  - `OPENLORE_INDEXER_REPO_DIDS` (required): bare `did:plc` or hostname-only `did:web` DIDs,
    no `#fragment`. Other methods or malformed DIDs refuse start.
  - `OPENLORE_INDEXER_SOURCE_URL` (optional): the relay-origin fallback for unresolvable DIDs.
    It must be https on a public address; anything else refuses start.
  - `OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES` (default 4, 1..=16) and
    `OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS` (default 30, 1..=600).
  - `OPENLORE_INDEXER_PLC_ENDPOINT` (default `https://plc.directory`) is not checked at
    startup, but DID-document fetches go through the guarded client, so a private or plain-http
    PLC mirror makes every DID unresolvable.
  - Never set `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP` in production; a release build refuses it.
- **Exit codes of `openlore-indexer ingest`**: 0 = completed (partial skips included; watch
  `pass_summary`); 3 = every configured DID skipped, usually transient, retry next interval
  and alert if it repeats; 2 = local fault (config, runtime, store), fix the deployment.
- **New events**: `indexer.ingest.source_skipped`, `source_fallback`, `pass_summary` (always
  the last stdout event), `config.loaded`, and `rejected.by_reason.foreign_repo`.
- **No consumer yet.** No deploy doc, scheduler or alarm in this repository runs
  `openlore-indexer ingest`. Whoever schedules it must treat exit 3 as "retry, alert on
  repeats" rather than a hard failure, and should route `source_skipped` and `pass_summary`.
