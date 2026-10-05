# OpenLore — Resume Context

## Current Task
Both `bluesky-claim-review-app` and `indexer-per-did-pds-fetch` are delivered and finalized (mutation gates passed). The review app is not deployed yet. History: `docs/evolution/`.

## Key Decisions
- The indexer resolves each repo DID to its own PDS every pass; the source URL is a relay-origin fallback (ADR-077..079).
- Indexer exit codes: 0 completed, 3 every DID skipped (retry, alert on repeats), 2 local fault. SSRF guard after DNS.
- Pushes are gated on green CI. After a shared-crate or schema change, run the dependent suites too.

## Next Steps
- Review-app go-live operator steps (tofu-aws-pds v1.7.0, SSM, replacement, deploy, alarms), then the live `canzantest` sign-in walkthrough.
- Indexer follow-ups: mid-pass upsert-failure test, fallback budget after a resolution timeout, shared guarded resolver, advisory mutation targets.
- Confirm the first release-guard CI run.
