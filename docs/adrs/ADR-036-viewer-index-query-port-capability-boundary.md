# ADR-036: The Viewer's Indexer-Query Capability — Reuse the slice-05 `IndexQueryPort` Behind the Viewer Composition Root, Public-Data READ Only

- **Status**: Accepted (DESIGN — viewer-network-search, slice-08)
- **Date**: 2026-06-04
- **Deciders**: Morgan (nw-solution-architect), resolving OD-NS-1 + OD-NS-6 for viewer-network-search (slice-08).
- **Feature**: viewer-network-search (slice-08; brownfield DELTA on slices 05/06/07)
- **Extends**: ADR-007 (pure/effect split), ADR-009 (hexagonal + wire→probe→use), ADR-023 (signing-incapable capability boundary), ADR-027 (the `IndexQueryPort` + CLI→indexer HTTP/XRPC transport + graceful degradation), ADR-028 (the hand-rolled hyper viewer server), ADR-030 (the read-only `StoreReadPort` capability boundary precedent).
- **Resolves**: OD-NS-1 (indexer-query port shape — reuse vs new) + OD-NS-6 (indexer URL config surface).

## Context

slice-08 adds a network-discovery `/search` surface to the read-only `openlore ui`
viewer (slices 06/07). The viewer process today reads only the LOCAL DuckDB store
(`StoreReadPort`, ADR-030) and — for `/scrape` — public GitHub (`GithubPort`,
slice-02). It has no way to reach the slice-05 network indexer, so the browser
cannot show network discovery at all (US-NS-001 problem).

slice-05 already shipped the entire CLI-side query path: the `IndexQueryPort` trait
(`crates/ports/src/index_query.rs`), its `HttpIndexQueryAdapter` implementation
(`crates/adapter-index-query`), the `NetworkSearchResultRaw`/`NetworkResultRowRaw`
flat-attributed transport types (non-`Option` `author_did`), the
`IndexQueryError::Unreachable` SOFT/non-fatal outcome, the
`org.openlore.appview.searchClaims` XRPC contract, and the `OPENLORE_INDEXER_URL`
env-var seam resolving `[appview] indexer_url`. The `IndexQueryPort` is ASYNC,
READ-ONLY by construction (NO sign/write/publish method), and carries a required
`probe()` (I-4).

The question (OD-NS-1) is whether the viewer gets a NEW second outbound query path
or REUSES the slice-05 one. The cardinal constraint (WD-NS-3 / I-NS-1) is that the
viewer process holds NO signing key and exposes NO write/sign/subscribe surface —
the new capability must be a public-data READ ONLY, mirroring the slice-06
`GithubPort` capability boundary (a port that reads only public GitHub and holds no
signing/identity/PDS surface, ADR-030 / I-VIEW-1).

## Decision

**The viewer REUSES the slice-05 `ports::IndexQueryPort` + the
`adapter-index-query::HttpIndexQueryAdapter` verbatim — there is ONE outbound
indexer-query path in the workspace, not a second. The `cli` composition root (the
viewer's composition root — the `openlore ui` verb) wires an `HttpIndexQueryAdapter`
into the `ViewerServer` exactly as the slice-06 `GithubPort` is wired (an
`Option<Arc<dyn IndexQueryPort>>` shared across the hyper accept loop). The port is
public-data READ ONLY: it holds NO signing/identity/PDS surface, by the same
type-level construction that makes the CLI's use of it read-only (no sign/write
method exists on the trait). The indexer URL resolves through the SAME slice-05
resolution — config `[appview] indexer_url` with the `OPENLORE_INDEXER_URL`
env-var seam (OD-NS-6) — one source of truth for "where is the index".**

### Capability-boundary shape (mirrors the slice-06 GithubPort)

```text
pub struct ViewerServer {
    listener: TcpListener,
    local_addr: SocketAddr,
    store: SharedStore,                       // slice-06: StoreReadPort (read-only, no write/sign)
    github: Option<SharedGithub>,             // slice-06: GithubPort (public GitHub READ only)
    index_query: Option<SharedIndexQuery>,    // slice-08 NEW: IndexQueryPort (public index READ only)
}
pub type SharedIndexQuery = Arc<dyn IndexQueryPort>;   // Send + Sync; NO signing/identity/PDS surface
```

`None` for store-only viewers that never serve `/search` (then `/search` degrades to
the same `Unavailable` state an unconfigured indexer produces — never a 404 surprise,
ADR-037). A new `bind_with_github_and_index` (or a small builder) wires both optional
capabilities; the existing `bind` / `bind_with_github` constructors are unchanged
(no-regression for slice-06/07 store-only and scrape viewers).

### Soft probe at viewer startup (KPI-5 / I-NS-2 carried from ADR-027)

The `cli` composition root SOFT-probes the wired `HttpIndexQueryAdapter` — exactly
as the CLI `search` verb does (ADR-027): an unreachable/unconfigured indexer is
INFORMATIONAL, never a viewer startup refusal. The viewer MUST start and serve its
local-store views (`/claims`, `/peer-claims`, `/scrape`) with the indexer down. The
hard-probe gauntlet (store readable, loopback bind — ADR-028 `ViewerServer::probe`)
is unchanged; `index_query.probe()` is soft/informational, never gating
`health.startup.refused`.

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **A NEW viewer-process query port + adapter** (a parallel `ViewerIndexQueryPort`) | Decouples the viewer from the CLI's port. | **Rejected (OD-NS-1 default / DRY / drift).** A second query path duplicates the XRPC contract, the `Unreachable` soft-fail discipline, the anti-merging transport shape, and the probe — two surfaces to keep in lockstep with the indexer, two contract-test consumers. The slice-05 port is ALREADY read-only by construction and already graceful-degrading; there is no viewer-specific concern it fails to model. Reuse is the conservative, smaller-surface call. |
| **Viewer-only env var (`OPENLORE_VIEWER_INDEXER_URL`)** | Lets the viewer point at a different indexer than the CLI. | **Rejected (OD-NS-6 / one-source-of-truth).** Two config keys for "where is the index" invite divergence (the browser and the CLI disagreeing about the network corpus is a trust hazard for P-001). The slice-05 `[appview] indexer_url` + `OPENLORE_INDEXER_URL` seam is the single source of truth; the viewer reads the SAME resolution. |
| **A viewer flag (`openlore ui --indexer-url`)** | Explicit per-launch. | **Rejected.** Adds a third config surface and a new CLI flag for no requirement (US-NS-001 / OD-NS-6 ask for reuse of the existing resolution). A flag could be an ADDITIVE future override; it is not needed for the walking skeleton. |
| **Let the viewer link `adapter-index-query` directly without an `Arc<dyn IndexQueryPort>` indirection** | Fewer types. | **Rejected (hexagonal / testability).** The `Arc<dyn IndexQueryPort>` seam is what lets DISTILL drive the `/search` handler against a `FakeIndexQuery` (incl. its unreachable mode) — the same double slice-05 already ships. Wiring the concrete adapter would forfeit the test seam and the capability-boundary clarity. |

## Consequences

### Positive
- ONE outbound indexer-query path workspace-wide (CLI + viewer share `IndexQueryPort`
  + `HttpIndexQueryAdapter` + the `OPENLORE_INDEXER_URL`/`[appview] indexer_url`
  resolution). No second contract, no drift, one contract-test consumer relationship.
- The viewer's read-only / no-key invariant is preserved STRUCTURALLY: `IndexQueryPort`
  has no sign/write/publish method (type-level), so adding it to the viewer process
  cannot introduce a signing capability — the same argument that admits `GithubPort`
  (ADR-030 / I-VIEW-1). The viewer still holds NO signing key.
- Graceful degradation is inherited, not reimplemented: `IndexQueryError::Unreachable`
  is already SOFT/non-fatal; the viewer maps it to the `Unavailable` render state
  (ADR-037).
- The wiring mirrors `GithubPort` exactly (`Option<Arc<dyn …>>` shared across the
  accept loop), so the change to `ViewerServer` is small and familiar.

### Negative
- The `cli` crate (the viewer's composition root) now wires `HttpIndexQueryAdapter`
  into BOTH the `search` verb and the `ui` verb. Accepted: it is the same adapter and
  the same config resolution; the composition root is the right place (it already owns
  the tokio runtime, ADR-028).
- `adapter-http-viewer` gains a dependency edge on `ports::IndexQueryPort` (already a
  pure trait it can reference) and, transitively at wiring time, on
  `adapter-index-query` (via `cli`, not a direct link of the viewer adapter to the
  indexer's SERVER crate). The viewer adapter MUST NOT link
  `adapter-xrpc-query-server` / `adapter-index-store` / `adapter-atproto-ingest` (the
  indexer-internal crates) — enforced by `xtask check-arch` (ADR-038 §enforcement).

## Revisit Trigger
- A genuine need for the browser to target a different indexer than the CLI (e.g. a
  read-only public mirror) → add an ADDITIVE viewer override that still defaults to
  `[appview] indexer_url`; do not fork the port.
- The viewer needs a richer index capability than read query (it will not, by WD-NS-3)
  → that would require returning to the PO (it breaches read-only).
