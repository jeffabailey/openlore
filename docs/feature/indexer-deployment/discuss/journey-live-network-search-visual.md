# Journey: live network search (indexer-deployment)

> Light DISCUSS. Actors: Jeff (operator), the host timer, Maria (searcher). Variables in
> mockups (`${...}`) are sourced in `shared-artifacts-registry.md`.

## Flow

```text
 JEFF (laptop)                       PDS HOST (t4g.micro)                          MARIA (laptop)
 ─────────────                       ────────────────────                          ──────────────
 S1 deploy.sh deploy <sha>
    verify CI + cosign ──────────▶  pull ${image_digest}
                                    start serve ─▶ Caddy ${public_index_url}
                                    install ingest timer (15 min)
    "serve ready" ◀───────────────
          │                                                                         
 S2 aws ssm put-parameter            S3 timer tick (every 15 min, never overlapping)
    ${did_list_param} ───────────▶     host renders ${did_list_file} (last good kept)
                                       openlore-indexer ingest ──▶ ${index_store}
                                       emits config.loaded, source_skipped,
                                       pass_summary, exit ${exit_code}
                                             │
                                             ▼
                                    S4 serve answers search during and between passes
                                             │                                     S5 OPENLORE_INDEXER_URL=${public_index_url}
                                             └──────────────── HTTPS ───────────▶     openlore search --object ...
                                                                                      sees Priya (bsky.social),
                                                                                      Dmitri (pds.volkov.dev)
 S6 alarm email (2x exit 3 | any exit 2) ◀── CloudWatch ◀── ${exit_code}
 S7 freshness command ─────────────▶ ${last_successful_pass_at}, ${pass_summary}
 S8 deploy.sh rollback ────────────▶ previous ${image_digest}
```

## Emotional arc

| Step | Actor | Feeling before | Feeling after | What creates the shift |
|---|---|---|---|---|
| S1 | Jeff | Uneasy: the first public service besides the PDS | Confident | Verified digest, `serve ready`, PDS still 200 |
| S2 | Jeff | Wary of a redeploy for a list change | Relieved | One `put-parameter`, picked up next pass |
| S3-S4 | (system) | n/a | n/a | No overlap, no lock fights |
| S5 | Maria | Skeptical: "local only" again? | Reassured | Authors from several PDSes, honest provenance labels |
| S6 | Jeff | Worried that a silent failure goes unseen | In control | Emails only for real outages and local faults |
| S7 | Jeff | Unsure if the index is fresh | Informed | One command: last good pass, its age, skips |
| S8 | Jeff | Fear of a bad release | Calm | One-command rollback, no data restore |

## Mockups

S1 deploy (laptop):

```text
$ deploy/indexer/deploy.sh deploy 4f2c9a…e1
ci: build 4f2c9a…e1 passed; cosign: verified (github.com/jeffabailey/openlore)
digest: ${image_digest}
host: serve restarted; ingest timer active (every 15 min)
serve ready at ${public_index_url} (PDS _health 200, review-app /healthz 200)
```

S5 search (Maria):

```text
$ OPENLORE_INDEXER_URL=${public_index_url} openlore search --object org.openlore.philosophy.reproducible-builds
Author: did:plc:priyaraman7x2k (priyaraman.bsky.social)
  github:priyaraman/cargo-pin  confidence 0.82  [self-attested]
Author: did:plc:dvolkov3m9q
  github:dvolkov/nix-lockcheck confidence 0.67  [verified]
```

S7 freshness (Jeff):

```text
last successful pass: ${last_successful_pass_at} (11 min ago)
  configured 12  own_pds 11  fallback 0  skipped 1
  skipped: did:plc:dvolkov3m9q pds_unreachable
recent exits: 0 0 0 3 0 0 0 0
last pass of any kind: 11 min ago
```

## Error paths

| ID | Path | Outcome |
|---|---|---|
| E1 | Malformed DID list | Exit 2 names the entry. Alarm. Search keeps serving the old index. |
| E2 | SSM unreadable at pass time | Last good list is used |
| E3 | Every DID unreachable for 2 passes | Exit 3 twice, one alarm, OK on recovery |
| E4 | One PDS down | Exit 0 with a skip, no alarm, visible in S7 |
| E5 | Pass longer than 15 min | No second concurrent pass |
| E6 | New digest not ready | Automatic rollback |
| E7 | Write or unknown path on the public host | Refused |
| E8 | Memory gate fails | t4g.small before go-live |
| E9 | Timer stopped | Not alarmed (OQ-IXD-4). Visible in S7. |
