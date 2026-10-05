# Prioritization: indexer-per-did-pds-fetch

| Rank | Story | MoSCoW | Value | Risk reduced | Effort | Depends on |
|---|---|---|---|---|---|---|
| 1 | US-IPF-001 Find self-attested claims from authors on any PDS | Must | High: unblocks J-009 discoverability | High: proves own-PDS admission end to end | 2–3 d | — |
| 2 | US-IPF-002 One unreachable author never blocks the others | Must | High: keeps the index fresh | High: N third-party PDSes | 2 d | 001 |
| 3 | US-IPF-005 Search says which claims are self-attested | Must | Medium: honest labels | Medium: provenance misstatement | 1 d | 001 |
| 4 | US-IPF-003 Unresolvable authors fall back without weakening provenance | Should | Medium | Medium: fail-closed proof | 1 d | 001, 002 |
| 5 | US-IPF-004 Indexer config is explained at startup; single-source unchanged | Should | Low–medium (operator) | Medium: regression | 1 d | 001 |

Total is about 7–9 days in one release. The order follows outcome impact, then dependency (see `story-map.md`).
