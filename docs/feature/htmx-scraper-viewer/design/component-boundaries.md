# Component Boundaries: htmx-scraper-viewer (slice-06)

> **DELTA** on the slice-01..05 component-boundaries. Two NEW crates + three EXTENDED.
> Functional Rust (ADR-007): pure rendering core, effect shell at the edges,
> function-signature ports. Governed by ADR-028/029/030.

## Crate layout (production 19 → 21)

```
crates/
  viewer-domain/          # NEW  — PURE: view-models + render_* (maud → HTML)
  adapter-http-viewer/    # NEW  — EFFECT: hyper server + route table + probe
  ports/                  # EXTEND — add StoreReadPort + boundary types (ADR-030)
  adapter-duckdb/         # EXTEND — impl StoreReadPort (new paginated reads)
  cli/                    # EXTEND — the `ui` verb (wiring + serve loop)
xtask/                    # EXTEND — check-arch: viewer pure-core + maud allowlist + capability rule
```

---

## `crates/viewer-domain` (NEW, PURE)

**Responsibility**: turn read-side projections (and live candidates) into HTML. Pure
functions only; no I/O, no DuckDB, no network, no hyper.

**Public surface (function-signature ports — the renderers ARE the contract):**

- View-model ADTs (immutable, owned): `ClaimRowView`, `ClaimDetailView`,
  `PeerClaimRowView`, `CandidateRowView`, `PageView<T>` (rows + `start`/`end`/`total` +
  `prev`/`next` page numbers), `EmptyState` (which view + the guided message), `ErrorView`
  (plain-language cause + next-step, no stack trace).
- `render_claims_page(&PageView<ClaimRowView>) -> String`
- `render_claim_detail(&ClaimDetailView) -> String`
- `render_peer_claims_page(&PageView<PeerClaimRowView>) -> String`
- `render_scrape_page(target: Option<&str>, candidates: &[CandidateRowView], state: ScrapeState) -> String`
- `render_empty(&EmptyState) -> String`, `render_error(&ErrorView) -> String`
- A minimal layout/shell helper (page chrome, read-only banner, nav, the inline CSS).

**Dependencies**: `maud` (whitelisted pure-core, ADR-029), `ports` (boundary types),
`serde`, `chrono`. **NO** `duckdb`/`tokio`/`reqwest`/`hyper`/`keyring`/`atrium-*`.

**Invariants enforced here:**
- **FR-VIEW-8**: `confidence` is rendered from the `f64` verbatim (no rounding/format).
- **I-VIEW-5 / WD-62**: `render_claims_page`/`render_claim_detail` have NO derived-from
  field; only `CandidateRowView` carries `derived_from`, rendered only by
  `render_scrape_page`. The TYPE makes the mistake impossible: persisted view-models have
  no derived-from slot.
- **BR-VIEW-1 / I-SCR-1**: `render_scrape_page` renders NO sign control; it renders the
  "nothing signed/saved — sign in the CLI" guidance.
- **NFR-VIEW-8**: semantic HTML (tables, labeled input, headings), compile-time-checked.

**Boundary**: registered in `xtask check-arch` as pure-core (I/O ban list applies).

---

## `crates/adapter-http-viewer` (NEW, EFFECT)

**Responsibility**: serve the read-only HTTP surface. Owns the hyper accept loop, the
route table, the per-route handlers, and the Earned-Trust `probe()`.

**Public surface:**
- `ViewerServer::bind(addr: SocketAddr, deps: ViewerDeps) -> Result<Self, ViewerError>`
  (binds the listener; reads back `local_addr` for `:0` — mirrors `XrpcQueryServer::bind`).
- `ViewerServer::local_addr(&self) -> SocketAddr`
- `ViewerServer::probe(&self) -> ProbeOutcome` (real, non-stub; ADR-030 §Earned Trust)
- `ViewerServer::serve(self) -> Result<(), ViewerError>` (async accept loop; called inside
  the reused tokio current-thread runtime)
- `ViewerDeps { store: Box<dyn StoreReadPort>, github: Box<dyn GithubPort> }` — the ONLY
  capabilities the server holds. **NO** `IdentityPort`, **NO** `PdsPort`, **NO** write
  `StoragePort`, **NO** signing key. This absence IS the read-only guarantee (I-VIEW-1/2).

**Route handlers** (build `viewer-domain` view-models, call `render_*`):
- `/claims` → `store.count_claims()` + `store.list_claims(page)` → `render_claims_page`
- `/claims/{cid}` → `store.get_claim(cid)` → `render_claim_detail` / not-found `render_error`
- `/peer-claims` → `store.count_peer_claims()` + `store.list_peer_claims(page)` →
  `render_peer_claims_page`
- `GET /scrape` → `render_scrape_page(None, &[], Form)`
- `POST /scrape` → parse target form → `github.resolve_target` + `harvest_*` +
  `scraper_domain::derive_candidates` → `render_scrape_page(Some(target), &candidates, ...)`;
  persists NOTHING, signs NOTHING
- `*` → 404 `render_error`

**Dependencies**: `hyper`/`hyper-util`/`http-body-util`, `tokio`, `ports`,
`viewer-domain`, `scraper-domain` (the pure `derive_candidates`), `serde`/`serde_json`,
`thiserror`. **MUST NOT** depend on another `adapter-*` (xtask invariant 4) — it takes
`GithubPort`/`StoreReadPort` as `Box<dyn _>` injected by the `cli` root, never importing
`adapter-github`/`adapter-duckdb` directly.

**Earned-Trust `probe()`** (real body; satisfies `xtask check-probes`; NOT allowlisted):
1. **store readability** — sentinel `store.count_claims()`; refuse with
   `health.startup.refused` if unreadable (US-VIEW-001 Ex 3 surfaced at startup).
2. **read-only capability** — asserts the wired `ViewerDeps` exposes no write/sign
   surface (structural: the struct has no such field; reinforced by the check-arch
   capability rule + the `viewer_is_read_only` gold test).
3. **loopback bind** — asserts `local_addr().ip().is_loopback()`; refuse otherwise (I-VIEW-4).

---

## `crates/ports` (EXTEND)

Add (ADR-030), in a new `store_read` submodule (mirroring `federated_row`/`graph`):

- `trait StoreReadPort { probe; count_claims; list_claims; get_claim; count_peer_claims;
  list_peer_claims; }` — SYNC, read-only, local DuckDB. NO write/sign method.
- Boundary projection types (owned by `ports`, the slice-04 `AttributedClaim` precedent):
  - `ClaimRow { cid, subject, predicate, object, confidence: f64, author_did: Did, composed_at }`
  - `ClaimDetail { row: ClaimRow, evidence: Vec<String> }`
  - `PeerClaimRow { cid, subject, predicate, object, confidence: f64, author_did: Did
    (peer origin), fetched_from_pds: String, composed_at }`
  - `PageRequest { page: u32, page_size: u32 }`, `Page<T> { rows: Vec<T>, total: u64,
    page: u32, page_size: u32 }`
  - `StoreReadError` (thiserror): `Unreadable { detail }`, `QueryFailed { message }`.

**Dependencies**: unchanged (`async-trait` allowed; NO tokio/reqwest/duckdb/keyring).
`StoreReadPort` is SYNC, so no async-trait needed for it.

---

## `crates/adapter-duckdb` (EXTEND)

Implement `StoreReadPort` for a read-only handle wrapper. AUGMENT the existing adapter
over the SAME store + SAME shared `Arc<Mutex<Connection>>` (WD-8 / Q-DELIVER-3); NO new
table, NO second handle, NO store swap.

- `count_claims` → `SELECT COUNT(*) FROM claims`
- `list_claims(page)` → `SELECT cid, subject, predicate, object, confidence, author_did,
  composed_at FROM claims ORDER BY composed_at DESC, cid ASC LIMIT ? OFFSET ?`
- `get_claim(cid)` → row by `cid` + `SELECT evidence FROM claim_evidence WHERE cid = ?
  ORDER BY ordinal`
- `count_peer_claims` → `SELECT COUNT(*) FROM peer_claims`
- `list_peer_claims(page)` → `SELECT cid, subject, predicate, object, confidence,
  author_did, fetched_from_pds, composed_at FROM peer_claims ORDER BY composed_at DESC,
  cid ASC LIMIT ? OFFSET ?`

Each query is SINGLE-table (`claims` OR `peer_claims`, never both), so the cross-store
`no_cross_table_join_elides_author` rule does not fire; the peer reads still project
`author_did` verbatim (attribution discipline carried forward). A read-only-handle
constructor (e.g. `DuckDbStorageAdapter::store_read_handle()`) returns a
`Box<dyn StoreReadPort>` that shares the connection but exposes only reads.

**Dependencies**: unchanged (`duckdb`, `ports`, `claim-domain`).

---

## `crates/cli` (EXTEND) — the `ui` verb (composition root for the viewer)

- New clap subcommand `ui` with `--port <u16>` (default 8788). NO `--host` (loopback only).
- Wiring (ADR-009 WIRE→PROBE→USE):
  1. resolve `OpenLorePaths`; open `DuckDbStorageAdapter::open(paths.duckdb_file())`;
     derive the read-only `Box<dyn StoreReadPort>` from it.
  2. build `GithubPort` via `GithubAdapter::from_env()` (same as `scrape github`).
  3. construct `SocketAddr` = `127.0.0.1:<port>` (hard-coded loopback IP).
  4. `ViewerServer::bind(addr, ViewerDeps { store, github })`.
  5. walk the viewer probe; on refusal emit `health.startup.refused` + a plain-language
     stderr line (port-in-use → suggest `--port`; store unreadable → name the path).
  6. print the listen URL + read-only notice (FR-VIEW-1).
  7. run `build_tokio_runtime().block_on(server.serve())` (the reused current-thread
     runtime, same shape `scrape github` / `claim publish` use).

The `ui` verb is the ONLY place that names `adapter-http-viewer` (invariant 5: only a
composition root links adapters). The CLI must NOT regress to linking the indexer's
server (`adapter-xrpc-query-server`) — unchanged by this slice.

**Dependencies**: gains `adapter-http-viewer` (+ transitively hyper/maud). Already has
`adapter-duckdb`, `adapter-github`, `tokio`, `clap`.

---

## `xtask` (EXTEND) — enforcement

- `check-arch`: add `"maud"` to `PURE_CORE_ALLOWED_CRATES`; add a
  `check_pure_core_no_io(workspace, "viewer-domain", ...)` arm; add a viewer
  capability-boundary check — `adapter-http-viewer`'s transitive deps MUST exclude
  `adapter-atproto-pds` (PDS write surface) and it must not reach a signing
  `IdentityPort` adapter; the `cli` root stays the only linker of `adapter-http-viewer`.
- `check-probes`: no rule change — `adapter-http-viewer`'s `probe()` must be a real
  (non-stub) body and is NOT added to `BOOTSTRAP_STUB_ALLOWLIST` (probe gate convention).

---

## Cross-component invariants (carried forward as I-VIEW-*)

| Invariant | Structural enforcement |
|-----------|------------------------|
| **I-VIEW-1 read-only** | Web process holds `StoreReadPort` (no mutation method) + no write `StoragePort`. check-arch capability rule + probe + `viewer_is_read_only` gold test. |
| **I-VIEW-2 no key in web process** | `ViewerDeps` holds no `IdentityPort`/`PdsPort`; `adapter-http-viewer` deps exclude `adapter-atproto-pds` + signing identity (check-arch). |
| **I-VIEW-3 human gate in CLI** | No sign control rendered (`viewer-domain` types have no sign affordance); signing path unreachable from the web process. |
| **I-VIEW-4 loopback only** | Hard-coded `127.0.0.1` bind; no `--host`; loopback self-probe refuses non-loopback. |
| **I-VIEW-5 derived-from display-only** | Persisted view-models carry NO derived-from slot (type-level); only `CandidateRowView` has it; gold test `derived_from_only_on_candidates`. |
| **I-VIEW-6 store views offline** | Store reads touch only local DuckDB; offline gold test. |
| **BR-VIEW-4 same store** | `StoreReadPort` impl shares the SAME DuckDB file + handle as the CLI writer; no second store/schema. |
| **CID / no new persisted types** | The viewer adds ZERO persisted types and ZERO new CID path — read + render only. |
