# Slice 05 — Pull from another user's instance (cross-instance federated read)

> Release 2 · Story: US-SF-006 (J-003) · Persona: P-002 (federation-reader hat; P-001 also wears it)
> Depends on: slice-01 (a reachable peer instance to point at) · Estimate: ~1 day
> Requirement: 4 (pull from OTHER instances → expand datasets)

## Goal

Expand a user's dataset by pulling claims from ANOTHER user's Cloudflare instance. This REUSES the
shipped J-003 federated-read flow (`openlore peer add` / `openlore peer pull`) verbatim; the ONLY
delta is that the peer's DID document resolves its PDS endpoint to their Cloudflare instance
(`https://openlore.rachel.workers.dev`) instead of a bsky.social PDS. All J-003 invariants carry over.

## Learning hypothesis

If Maria can `peer add` a self-hosting peer whose DID resolves to their Cloudflare instance and
`peer pull` their claims — verified (signature + CID) and stored in the separate peer_claims layer with
attribution intact, none merged — then the Cloudflare-instance transport is just another PDS transport
for J-003, and peer-to-peer federation between users' OWN instances works with no parallel machinery.

## IN scope

- `openlore peer add <did>` / `openlore peer pull` unchanged, where the peer DID resolves to a
  Cloudflare instance PDS endpoint (OD-SF-3: standard ATProto DID-doc PDS resolution).
- Peer claims land in the separate peer_claims layer; author_claims untouched (J-003).
- Signature verification + CID recompute before store; failures rejected + reported (J-003 KPI-FED-6).
- Per-author attribution preserved; no consensus/merged row (anti-merging, D-8 → KPI-SF-4).

## OUT of scope

- Re-authoring any J-003 semantics — this slice REFERENCES `subscribe-and-read-federated.yaml`; it
  reuses the existing peer verbs, peer_claims store, verification, and `graph query --federated`.
- Counter-claim authoring / unsubscribe (already shipped in J-003; unchanged here).
- Discovery of WHICH instances to pull from (that is J-005 network discovery, not this feature).

## Acceptance criteria (from US-SF-006 UAT)

- [ ] `peer add` / `peer pull` work unchanged when the peer's PDS endpoint resolves to a Cloudflare
      instance (transport delta only, D-8).
- [ ] Peer claims land in peer_claims; author_claims are never modified (J-003).
- [ ] Every peer claim's signature is verified and CID recomputed before store; failures rejected +
      reported (J-003 KPI-FED-6 → KPI-SF-4).
- [ ] Output preserves per-author attribution; no consensus/merged row (anti-merging).
- [ ] An unreachable peer instance skips only that peer; others proceed; exit non-zero overall.

## Dependencies

- The shipped J-003 federated-read flow (peer verbs, peer_claims, verification) — reused.
- OD-SF-3 (DID-doc → Cloudflare-PDS resolution); a reachable peer Cloudflare instance (slice-01 shape).

## Estimate

~1 day: wire the DID-doc → Cloudflare-instance PDS resolution into the existing J-003 pull path; the
attribution/verification acceptance tests reuse the J-003 suite against the new transport.
