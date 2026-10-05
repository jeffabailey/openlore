# ADR-072: The Hosted Review App Is a Third Composition Root (`openlore-review-app`)

- **Status**: Accepted (2026-10-04) — implemented; see `docs/evolution/bluesky-claim-review-app-evolution.md` (proposed 2026-10-04)
- **Date**: 2026-10-04
- **Deciders**: Jeff Bailey (D-2 hosted web app; time-to-market + low ops), Morgan (nw-solution-architect)
- **Feature**: bluesky-claim-review-app (DESIGN), resolves OD-BRA-4
- **Amends**: cross-feature invariant **I-3** (already broadened to two roots by ADR-023)

## Context

I-3 says that only composition roots may wire adapters into ports. `xtask check-arch` encodes
`COMPOSITION_ROOTS = ["cli", "openlore-indexer"]`. The review app is a multi-user, public,
always-on HTTPS service. Its capability profile differs from both existing roots:

| Root | Signs claims | Holds user's local store | Writes to a PDS | Serves public HTTP |
|---|---|---|---|---|
| `cli` | yes (keychain) | yes | yes (app password) | loopback viewer only |
| `openlore-indexer` | no | no | no | yes (read-only XRPC) |
| **review app** | **no** | **no** | **yes, as the user, via OAuth, create-only** | **yes (authenticated + public pages)** |

The app must reuse the pure cores (`claim-domain`, `lexicon`, `scraper-domain`, `appview-domain`
helpers) and the read adapters (`adapter-github`, `adapter-atproto-ingest`, the resolve-only part of
`adapter-atproto-did`). It must not inherit the CLI's signing identity or local store.

## Decision

1. **Add a new binary crate `crates/openlore-review-app`. It is the third composition root.**
   - Verbs: `serve`, `probe` and `kpi`, mirroring the indexer's `serve|ingest|stats`.
   - Startup follows "wire, then probe, then use". If a probe fails, the app refuses to start and
     emits a structured `health.startup.refused` event.
   - The crate holds the hyper 1.x HTTP shell (routing, cookies, CSRF, shape fork) and the
     background scan task.
   - Everything decidable lives in the new pure crate `review-domain`.
2. **I-3 is reworded** to: "Only the declared composition roots (`cli`, `openlore-indexer`,
   `openlore-review-app`) may wire adapters. The roots wire *disjoint* capability sets."
   `COMPOSITION_ROOTS` gains the new crate.
3. **Capability boundary for the new root** (new `check-arch` rule
   `review_app_capability_boundary`, specified in `component-boundaries.md`):
   - **MUST NOT reach**: `adapter-duckdb` (the user's local store), `adapter-atproto-pds` (the
     app-password write surface), `adapter-publish-http`, `adapter-http-viewer`,
     `adapter-xrpc-query-server`, `adapter-index-store` or `adapter-index-query`.
   - **MAY link**: `adapter-atproto-did`, through the resolve-only `IdentityResolvePort`, as the
     indexer already does. A structural token scan forbids naming `IdentityPort` or a keychain
     constructor in its sources.
   - `cli` and `openlore-indexer` MUST NOT reach `adapter-atproto-oauth`, `adapter-review-store` or
     `openlore-review-app`.
   - Only `openlore-review-app` may link those two adapters.
4. **HTTP server: a hand-rolled hyper 1.x server, the same as `adapter-http-viewer` and
   `adapter-xrpc-query-server`.** The `deny.toml` bans on `axum` and `actix-web` **stay**. The ban's
   stated rationale ("we never run an HTTP server in-process") is reworded in DELIVER to "no web
   framework; hand-rolled hyper only".
5. **Paradigm (ADR-007)**:
   - Pure functions decide everything: suggestion lifecycle, ownership verdict, publish, retract
     and share **plans**, rate-limit arithmetic, and HTML rendering with maud.
   - The binary is a thin effect shell. It executes plans and never decides.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| **A `cli` verb (`openlore serve-review`)** | Rejected. The CLI links the keychain signing identity, the app-password PDS writer and the user's local DuckDB. A public multi-user process would inherit all of them, which breaks I-VIEW-3-style capability isolation. It would also make the CLI binary carry the OAuth stack. |
| **Fold the app into `openlore-indexer`** | Rejected. The indexer is signing-incapable *and* write-incapable by design (I-AV-5). The review app writes to users' PDSes. Merging them destroys the indexer's guarantee. |
| **Unban `axum` for faster routing** | Rejected for now. It needs a `deny.toml` ADR amendment and adds a second HTTP idiom to the codebase. Two hyper shells already exist, and the app has about 15 routes. Revisit if route count or middleware needs grow past what a hand-rolled router handles cleanly. |
| **A separate repository / service in another language** | Rejected. The app must reuse the Rust pure cores (CID, scraper mapping, provenance). Duplicating them reintroduces the CID-divergence class that ADR-062 documented. |

## Consequences

- **Positive**:
  - Pure cores are reused without duplication.
  - The capability set is minimal and enforced.
  - The existing CI (`ci.yml --workspace`) covers the new crates automatically.
- **Negative**:
  - +4 workspace members (25 production, 27 members). There is a third binary to build for
    linux/arm64.
  - `check-arch` grows by about 4 rules.
- **Brief update**: the I-3 row in `docs/product/architecture/brief.md` is reworded in the same
  commit as this ADR.
