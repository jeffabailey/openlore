# Slice 04 — Public philosophy card at a stable, Bluesky-linkable URL

> Release 2 · Story: US-SF-005 (J-007) · Persona: P-001 (Maria, publisher/sharer hat)
> Depends on: slice-02 (data on the instance) · Estimate: ~1 day
> Requirement: 1 (public card + link from Bluesky profile)

## Goal

The public/social payoff: the user's own Worker serves a READ-ONLY HTML philosophy card at a stable
URL (derived from `instance_url`) that renders ONLY explicitly-pushed claims, each attributed to the
author DID (anti-merging, no consensus row). The user pastes the URL into their Bluesky profile — the
first intentional PUBLIC identity surface for this otherwise-private, CLI-first persona.

## Learning hypothesis

If anyone can open `https://openlore.maria.workers.dev/` and see Maria's pushed claims, each attributed
to `did:plc:maria-test`, read-only, with no consensus row — and Maria can link it from her Bluesky
profile and confirm it exposes nothing she did not push — then the sovereign-yet-shareable promise of
J-007 lands: a public surface she OWNS, exposing only what she chose, with no central authority.

## IN scope

- A read-only `GET /` card served by the user's Worker rendering explicitly-pushed claims.
- Per-claim attribution to the author DID; NO merged/consensus row (anti-merging, D-7).
- Read-only: no authoring/editing/counter control; the Worker holds no signing key (D-7).
- Empty-but-valid state before any push ("no claims published yet").
- A stable URL a user pastes into a Bluesky profile (manual paste; no code).

## OUT of scope

- Cross-instance pull (→ slice-05).
- Bluesky handle-verification / `did:web` bidirectional link verification (OD-SF-4 default: manual
  link, no verification).
- Rich card interactivity, search, or graph traversal on the card (a future viewer concern); the card
  is a simple attributed read view.
- Any write path from the card (forbidden by D-7).

## Acceptance criteria (from US-SF-005 UAT)

- [ ] The card is served at a stable URL derived from `instance_url` and renders only explicitly-pushed
      claims (D-7).
- [ ] Every rendered claim is attributed to its author DID; no consensus/merged row (anti-merging).
- [ ] The card is read-only — no authoring/editing control; the Worker holds no signing key (D-7).
- [ ] The URL is directly linkable from a Bluesky profile; an unpushed local claim never appears.
- [ ] Before any push, the card renders an empty-but-valid state (not an error).

## Dependencies

- slice-02 (claims on the instance to render).
- The card HTTP surface shape / URL (OD-SF-1 / OD-SF-4).

## Estimate

~1 day: a read-only card route + template rendering pushed claims with attribution; the
attribution/no-merge and read-only assertions; README note on pasting the URL into Bluesky.
