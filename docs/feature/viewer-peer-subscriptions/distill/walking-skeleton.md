# Walking Skeleton — viewer-peer-subscriptions (slice-15) — DISTILL

> Wave: DISTILL · Owner: Quinn (nw-acceptance-designer) · 2026-06-09

## Strategy (Architecture of Reference + Project Infrastructure Policy)

Brownfield DELTA — NO new walking-skeleton Feature 0. The `openlore ui` viewer, the
read-only `StoreReadPort`, the `peer_subscriptions` table + `removed_at` soft-remove
(slice-03), the `page = chrome + fragment` render pattern (slice-06/07), and the
render-only-CLI-command pattern (slice-08 `render_follow_guidance`) all already exist.

Per the Project Infrastructure Policy (`docs/architecture/atdd-infrastructure-policy.md`),
the port treatments for this slice are inherited (no per-feature negotiation):

- **Driving** — `openlore ui` verb: long-running subprocess `openlore ui --port 0` via
  `assert_cmd::cargo_bin("openlore")`, bound ephemeral `:0`, driven over HTTP
  (`ViewerServer::get` / `get_htmx`); HX-Request fork. The policy row was extended this
  wave to record the new `GET /peers` route.
- **Driven internal (real)** — `StoragePort` (`adapter-duckdb`, the user's
  `openlore.duckdb`): real DuckDB file under `OPENLORE_HOME`, seeded via the real `peer add`
  / `peer pull` / `peer remove` verbs. The `/peers` read is a LOCAL SELECT over the SAME
  shared read handle the viewer already holds (BR-VIEW-4).
- **Driven external / non-deterministic (fake)** — NONE on this route. `/peers` is LOCAL +
  OFFLINE (offline-STRONGER than `/search`; no PDS / indexer / GitHub edge). The only fake
  used is the slice-03 `PeerPds` double — and ONLY in the SEED path (`peer add` + `peer
  pull` to land the rows), never on the `/peers` read itself.

## The walking-skeleton scenario

**PS-1** (`open_peers_with_htmx_returns_only_the_peers_fragment_with_did_count_and_revoke_command`,
`@walking_skeleton @driving_port @driving_adapter @real-io @htmx-fragment`) is the thinnest
complete end-to-end thread the slice can demo:

```
viewer (real openlore ui subprocess)
  → GET /peers WITH HX-Request
  → LOCAL active-subscription read (list_active_peer_subscriptions, ONE aggregate query)
  → pure projection (PeersView::Subscriptions)
  → HTML fragment (#peers region: per-peer DID + count + render-only `peer remove` command)
```

It seeds two ACTIVE peers (Rachel 5 cached claims, Tobias 3 cached claims) through the
PRODUCTION federation write path (`seed_peers_two_active_with_claims` → real `peer add` +
`peer pull`), starts the real `ViewerServer`, issues an `HX-Request` GET, and asserts the
observable rendered surface: ONLY the `#peers` fragment (no chrome), two attributed rows,
each its DID VERBATIM + its per-peer claim count (5 / 3) + the render-only `openlore peer
remove <did>` command.

## Litmus test (user-centric, demoable)

A non-technical stakeholder confirms "yes, that is what Maria needs": she opens the viewer's
`/peers` page and sees, at a glance, every peer she currently follows — its DID and how many
of its claims she holds locally — with the exact, clean CLI command to leave each one beside
it. The title describes the user goal (see who I follow + the clean revocation path), the
Given/When describe her actions (she follows two peers; she opens /peers), and the Then
describes what she SEES (the two attributed rows + the revoke commands) — not internal side
effects.

## Driving-adapter coverage

The DESIGN driving surface is the `openlore ui` `GET /peers` route (an HTTP endpoint). PS-1
exercises it via its actual protocol (an in-test HTTP GET with the `HX-Request` header
against the real subprocess), asserting status (200), output format (text/html, the `#peers`
fragment), and shape selection (fragment vs full page). PS-2 covers the no-JS full-page
shape. No CLI/endpoint/hook in DESIGN is left uncovered.

## Adapter coverage table (Mandate 6)

| Driven adapter | `@real-io` scenario | Covered by |
|---|---|---|
| `adapter-duckdb` `StoreReadPort::list_active_peer_subscriptions` (the new read impl + active-only + per-peer-count SQL) | YES | PS-1 / PS-3 / PS-4 / PS-5 + all GOLD — every scenario reads the REAL seeded LOCAL DuckDB through the real viewer; the read is the LOCAL boundary exercised on every `/peers` GET |
| `PeerPds` double (slice-03; seed path ONLY) | n/a (seed) | the seeds drive the real `peer add` + `peer pull` against the slice-03 `PeerPds` double — the SAME fake slice-03/09/10 use; it is NOT on the `/peers` read path |

There is NO net-new driven adapter requiring a fresh `@real-io @adapter-integration`
scenario beyond the new `adapter-duckdb` read impl, which every scenario exercises with real
I/O over the real DuckDB store.
