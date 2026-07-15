# Slice 03 — Pull my instance back into local DuckDB (reconcile / rebuild)

> Release 1 · Story: US-SF-004 (J-008) · Persona: P-001 (Maria, publisher/sharer hat)
> Depends on: slice-02 · Estimate: ~1 day · Requirement: 2 (pull my instance → local DuckDB)

## Goal

Close the round-trip for real data: `openlore publish pull` fetches my instance's records into local
DuckDB as a **reconcile** — never a silent overwrite — with round-trip CID integrity, and rebuilds
local state from scratch on a fresh machine (durability). This makes the instance a verifiable mirror
I can recover from.

## Learning hypothesis

If Maria can pull her instance and (a) get a no-op when already in sync, (b) rebuild an empty local
store with attribution intact on a fresh laptop, and (c) see a genuine conflict SURFACED rather than
silently overwritten — then her instance is a durable, verifiable mirror and the never-silently-mutate
discipline holds across the host boundary.

## IN scope

- `openlore publish pull` fetches instance records; a record already present locally with a matching
  CID is a no-op.
- Fresh-machine rebuild: on an empty local store, reconstruct DuckDB with each record verified before
  insert; attribution (author DID) intact.
- Conflict surfacing: a pulled record whose CID differs from a local claim's CID for the same logical
  record is surfaced, NOT auto-resolved / overwritten (D-6).
- Round-trip integrity: a claim pushed (slice-02) and pulled here has an identical CID (KPI-SF-1).

## OUT of scope

- The card UI (→ slice-04); cross-instance pull (→ slice-05).
- Interactive conflict resolution UX (surface the conflict; resolution policy is a future concern).
- Multi-device merge policy beyond "additive reconcile + surface conflicts".

## Acceptance criteria (from US-SF-004 UAT)

- [ ] Pull reconciles additively; an in-sync record is a no-op; no silent overwrite of a local claim
      (D-6).
- [ ] On an empty local store, pull reconstructs local DuckDB with attribution intact; each record
      verified before insert.
- [ ] A CID conflict is surfaced, not auto-resolved.
- [ ] Round-trip integrity holds: a claim pushed in slice-02 and pulled here has an identical CID
      (KPI-SF-1).

## Dependencies

- slice-02 (a populated instance to pull from).
- The local store's existing never-silently-mutate discipline (reused).

## Estimate

~1 day: pull + reconcile logic (no-op / rebuild / conflict-surface) + verify-before-insert; the
fresh-machine rebuild and conflict tests.
