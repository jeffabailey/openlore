# Story map: indexer-deployment

## Backbone (user activities, left to right)

| Ship it | Feed it | Serve it | Find claims | Watch it | Change it safely |
|---|---|---|---|---|---|
| Jeff deploys by digest | Timer runs passes; Jeff edits the DID list | Public read-only search | Maria searches | Alerts, freshness | Upgrade, roll back, resource gate |

## Walking skeleton (Release 1: "search is live")

| Ship it | Feed it | Serve it | Find claims |
|---|---|---|---|
| US-IXD-001 (first deploy) | US-IXD-002 (15-min non-overlapping passes, shared store) | US-IXD-001 (TLS, search only) | US-IXD-001/002 |

This is the thinnest end-to-end slice: one signed image, `serve` behind Caddy, and the timer
passes over one store. Maria's `openlore search` returns live multi-PDS results. The DID list
is read from the SSM parameter from day one (the minimal form of US-IXD-003). Edit-without-
redeploy hardening, last-good-list handling and the malformed-list path complete in R2.

## Release 2: "operate it safely" (before Maria is told the URL)

| Feed it | Watch it | Change it safely |
|---|---|---|
| US-IXD-003 (edit the DID list, no redeploy; J-005) | US-IXD-004 (2x exit 3, any exit 2) | US-IXD-006 (deploy, rollback, memory gate) |
| | US-IXD-005 (freshness; Should) | |

R2 contains a J-005 story (US-IXD-003), so the slice has user-visible value.

## Later / out of scope

- Public freshness for Maria (OQ-IXD-5). The public index as the CLI default (OQ-IXD-6).
- An alert on a dead timer or a dead serve (OQ-IXD-4, needs a user decision).
- Per-IP rate limiting (OQ-IXD-7). Purging removed DIDs (OQ-IXD-9).

## Priority Rationale

1. **US-IXD-001, then US-IXD-002** (P1, the walking skeleton). They are the only path to the J-005
   outcome. Nothing else has value until search is live and fed. 002 depends on 001's image and
   host wiring, and resolving store sharing (OQ-IXD-1) is the riskiest unknown, so it is
   proved first.
2. **US-IXD-006** (P2). Proves the 1 GiB host can carry the load (R-IXD-1). A failed gate changes
   the instance size, so it must happen before go-live is announced.
3. **US-IXD-004** (P2). Without alerts, a total outage or a local fault goes unseen. It is cheap
   (1 day, 2 alarms).
4. **US-IXD-003** (P2). The value of coverage grows with every review-app user, and the malformed-
   list path feeds the exit-2 alarm.
5. **US-IXD-005** (P3, Should). Diagnostics. The alarms cover the urgent cases, so this matters
   most for a dead timer (OQ-IXD-4).

All releases run after DEP-IXD-1 (v1.7.0 plus R-REPLACE). See `wave-decisions.md` for sequencing.
