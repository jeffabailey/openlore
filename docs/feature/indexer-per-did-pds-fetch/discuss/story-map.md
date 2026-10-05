# Story map: indexer-per-did-pds-fetch

## Backbone (user activities, left to right)

| Configure the indexer | Resolve each author | Read each author's repo | Decide what to index | Observe the pass | Search the network |
|---|---|---|---|---|---|
| US-IPF-004 config explained at startup | US-IPF-001 resolve to own PDS | US-IPF-001 list on own PDS | US-IPF-001 verdict unchanged, repo binding | US-IPF-002 per-DID skip reasons | US-IPF-001 Priya found |
| | US-IPF-003 fallback for unresolvable | US-IPF-002 fault isolation, bounded fan-out | US-IPF-003 fallback is relay-origin | US-IPF-002 source counts | US-IPF-005 `[self-attested]` label |

## Walking skeleton (the one thin end-to-end slice)

**US-IPF-001.** Two authors on two different PDSes are resolved, listed on their own PDS, and
pass the unchanged verdict in one pass. Maria then finds Priya's self-attested claim through
`openlore search`. The slice touches every backbone column except "configure" and "observe".
Those keep today's behavior until R1.

## Release 1 (same feature, by outcome)

| Order | Story | Outcome it adds |
|---|---|---|
| 2 | US-IPF-002 | One bad PDS no longer blanks a whole pass, and Jeff knows why a DID was skipped. |
| 3 | US-IPF-005 | Maria can tell self-attested claims from app-signed ones. |
| 4 | US-IPF-003 | Unresolvable DIDs still contribute app-signed claims through the fallback, without weakening ADR-071. |
| 5 | US-IPF-004 | Jeff gets actionable startup errors, and single-source deployments are shown not to regress. |

## Priority Rationale

1. **US-IPF-001 first.** This is the whole point of the feature. It turns J-009's approved
   claims into J-005 results, and it is the riskiest assumption: that the resolved-PDS listing
   plus the unchanged verdict admits bsky.social records.
2. **US-IPF-002 next.** Fetching from N third-party PDSes multiplies failure sources. Today one
   listing failure aborts the pass (exit 2), so going live on 001 without 002 would let any
   bsky.social hiccup stop all indexing.
3. **US-IPF-005 before announcing.** Once 001 ships, self-attested rows reach search and would
   be shown as app-signed (absent field means app-signed). That is an ADR-071 §5 honesty gap.
   It depends only on 001.
4. **US-IPF-003.** This is the fallback. It is lower value because most DIDs resolve. Its
   behavior is close to today's single-source path, so the regression risk is low.
5. **US-IPF-004 last.** It is operator ergonomics and the no-regression proof. It is infrastructure.
