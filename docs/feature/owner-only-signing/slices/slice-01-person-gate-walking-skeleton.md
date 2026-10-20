# Slice 01 — Only Jeff signs about Jeff's GitHub account (walking skeleton)

> Stories: S-OOS-1, S-OOS-2, S-OOS-3 (person checks) · Persona: P-001 (Jeff)
> Jobs: J-001, J-003 (J-003b) · Infrastructure-only: **no**
> Depends on: github-did-identity-link slice 1 (identity link + v7 link checks) · Estimate: ~3 days
> Derived from DESIGN slice 1 (`design/wave-decisions.md`); no DISCUSS interviews.

## User-visible outcome

`openlore claim add` about `github:jeffbailey` signs and publishes for Jeff, who has linked that
account. The same command about `github:rachelk` signs nothing and prints who can sign it: rachelk,
after `openlore identity link github rachelk`. `openlore infer people --sign` signs only Jeff's own
adherence. `openlore claim counter` and `openlore claim retract` always work. Offline, signing
uses a link check no older than 7 days, or refuses and names `openlore authority verify`.

## IN scope

- Pure `claim-domain::authority` person rules (`requirement`, `classify`, `may_sign`) and the
  hand-off text.
- Gate in `claim add` and `infer people --sign`. Counter and retract exempt.
- 7-day offline window over v7 link checks (I-9 exception).

## OUT of scope

- Repo authority, `authority verify` for repos, scrape `--sign` (slice 2). Labels and scoring
  (slice 3).

## Acceptance (scenarios)

WS-OOS-1, PS-2..PS-8, CORE-OOS-1..4.

## Learning hypothesis

If Jeff cannot sign about himself within a minute of linking his account, the person tier is too
heavy and GDL's link flow needs work before anything else ships.
