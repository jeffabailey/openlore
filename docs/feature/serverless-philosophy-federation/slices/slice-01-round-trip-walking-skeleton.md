# Slice 01 — Walking skeleton: round-trip one signed claim to my own Worker

> Walking Skeleton · Stories: US-SF-001 (deploy+register, J-007) + US-SF-002 (round-trip, J-008)
> Persona: P-001 (Maria, publisher/sharer hat) · Depends on: SPIKE-00 (blocks this slice)
> Estimate: ~1 day (after SPIKE-00)

## Goal

The thinnest end-to-end thread that connects every core activity — deploy, register, push, pull. Maria
`wrangler deploy`s the `atproto/` Worker to her OWN Cloudflare account, runs
`openlore publish init <url>` to register it, `openlore publish push`es ONE signed claim, and
`openlore publish pull`s it back with an **identical recomputed CID**. Proves the CLI↔Worker pipe and
that canonicalization survives the boundary (KPI-SF-1) before any later slice depends on it.

## Learning hypothesis

If a signed claim pushed to a user's own Worker pulls back with a byte-identical CID — and the user's
local store is untouched and the instance holds no signing key — then the mirror thesis (J-008) and
everything the card will render (J-007) rest on solid ground, the self-hosted-serverless boundary is
real, and later slices (bulk push, reconcile, card, cross-instance) can build on it. Settling
**OD-SF-1** (the transport chosen in SPIKE-00) against a real push/pull is the load-bearing learning.

## IN scope

- `openlore publish init <instance-url>` — records a reachable instance URL as the publish target (the
  ADR-027 configurable URL, D-5); prints `instance_url` + unchanged `author_did` + derived `card_url`;
  refuses a dead URL.
- `openlore publish push` for ONE claim → the user's own Worker; the Worker recomputes + stores its CID.
- `openlore publish pull` fetches it back; recomputed CID **must** equal the local CID (KPI-SF-1).
- Guardrails proven: local store untouched (additive); instance unreachable → non-zero exit, local
  authoring/query unaffected (KPI-5); instance holds no signing key (D-7).

## OUT of scope

- Bulk/whole-graph push (→ slice-02); pull-reconcile / fresh-machine rebuild (→ slice-03); the public
  card UI (→ slice-04); cross-instance pull (→ slice-05).
- Production storage/auth hardening; `publish status`; verb-grammar finalization (OD-SF-2).

## Acceptance criteria (from US-SF-001 + US-SF-002 UAT)

- [ ] `openlore publish init <url>` records a reachable instance URL, prints instance_url + author_did
      + card_url, confirms the instance is the user's own with no central authority (D-1, D-4); refuses
      an unreachable URL; does not change signing identity or local store (D-5, D-6, D-7).
- [ ] One claim pushed and pulled back has an identical recomputed CID at both ends (KPI-SF-1).
- [ ] A CID mismatch (either end) rejects that claim as a canonicalization mismatch and reports it; no
      silent store (D-6).
- [ ] Push/pull is additive; the local claim is never modified.
- [ ] With the instance unreachable, `publish push` exits non-zero; `openlore graph query` and
      compose/sign still work offline (KPI-5).

## Dependencies

- **SPIKE-00** (settle first): the chosen transport + storage shape (OD-SF-1) and go/no-go.
- A Cloudflare account + `wrangler`; the `atproto/` Worker skeleton produced during this slice.
- Existing claim-domain CID/signing (reused for canonicalization).

## Estimate

~1 day after SPIKE-00: `publish init` + a one-claim `push`/`pull` against the spike-validated
transport, plus the round-trip CID assertion and the offline/no-overwrite guardrail checks.
