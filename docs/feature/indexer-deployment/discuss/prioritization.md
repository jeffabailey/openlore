# Prioritization: indexer-deployment

| Rank | Story | job_id | Release | MoSCoW | Value | Risk retired | Size | Depends on |
|---|---|---|---|---|---|---|---|---|
| 1 | US-IXD-001 Search the live public network index | J-005 | WS | Must | High (the only path to the outcome) | Packaging, Caddy site, isolation | 2-3 d | DEP-IXD-1 |
| 2 | US-IXD-002 Refresh every 15 min while search keeps answering | J-005 | WS | Must | High | **Store sharing (OQ-IXD-1)** | 2-3 d | 001 |
| 3 | US-IXD-006 Ship or roll back without disturbing the PDS | infrastructure-only | R2 | Must | Medium | **Memory on 1 GiB (R-IXD-1)** | 2 d | 001 |
| 4 | US-IXD-004 Emailed only when really broken | infrastructure-only | R2 | Must | Medium | Silent outages | 1 d | 002 |
| 5 | US-IXD-003 Add an author by editing the DID list | J-005 | R2 | Must | Medium, growing | Coverage lag | 1-2 d | 002 |
| 6 | US-IXD-005 See when the last good pass ran | infrastructure-only | R2 | Should | Low-Medium | Dead timer unseen | 1 d | 002 |

Total about 9-12 days. The go-live gate is the R2 Must stories complete, plus the AC-004.5
alarm test and the AC-006.4 memory gate.
