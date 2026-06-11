# ATDD Infrastructure Policy

Per `nw-distill` § Project Infrastructure Policy. One file per project.
Apply-if-exists; write-if-absent; rewrite with `--policy=fresh`. Git history is
the audit trail. Records the CONCRETE MECHANISM for each port class's default
treatment (the Architecture of Reference fixes the port-class → treatment
defaults: driving = real adapter; driven-internal = real; driven-external /
non-deterministic = fake).

- **Bootstrapped**: 2026-05-28, slice-05 DISTILL (the file was absent through
  slices 01-04 — each deferred writing it per DD-11 / DD-FED-11 / DD-GRAPH-11;
  this wave materializes it with the cumulative slice-01..05 entries).
- **Language**: Rust (`[lang-mode] rust`). State-delta port:
  `tests/common/state_delta.rs` (slice-01 bootstrap; `[port-mode] inherit`).

## Driving

| Port | Mechanism | Note |
|---|---|---|
| `openlore` CLI (`claim add/publish`, `graph query`, `peer add/pull/remove`, `scrape`) | subprocess from `tempfile::TempDir` (`OPENLORE_HOME`) via `assert_cmd::cargo_bin("openlore")` | the production composition root; slice-01..04 |
| `openlore` CLI `search` verb (`--object/--contributor/--subject/--show/--share`) | subprocess via `assert_cmd::cargo_bin("openlore")` | slice-05; the ONLY network verb; degrades gracefully when the indexer is unreachable |
| `openlore-indexer` binary (`ingest` / `serve`) | subprocess via `assert_cmd::cargo_bin("openlore-indexer")`; `serve` bound to ephemeral `:0` (read back, parallel-safe) | slice-05; the SECOND composition root; signing-incapable; holds no local store |
| pure domain functions (`claim_domain::verify`, `scoring::score`, `appview_domain::ingest_decision` / `compose_results`) | direct in-process invocation (the function signature IS the port at domain scope) | layer-2 `@property`; no subprocess |
| `openlore` CLI `ui` verb (the read-only viewer; `GET /` `/claims` `/peer-claims` `/scrape` `/search` `/score` `/project` `/philosophy` `/claims/{cid}` `/peers` `/static/htmx.min.js`) | long-running subprocess `openlore ui --port 0` via `assert_cmd::cargo_bin("openlore")`, bound ephemeral `:0` (read back from `viewer.serve.listening`), driven over HTTP (`ViewerServer::get`/`get_htmx`/`post_form`); HX-Request fork | slice-06..15; loopback-only; holds NO signing key; slice-08 adds `GET /search`; slice-09 adds `GET /score`; slice-10 adds `GET /project` + `/philosophy`; slice-11 adds `GET /claims/{cid}`; slice-15 adds `GET /peers` (the read-only PEER-SUBSCRIPTIONS view — LOCAL active-subscription read `list_active_peer_subscriptions` + PURE `render_peers_*`, NO network — distinct from /scrape + /search; offline-STRONGER, mirrors /project + /philosophy); slice-16 adds the `/search` FOLLOW-STATE facet (the effect shell reads the LOCAL active set ONCE via the REUSED `list_active_peer_subscriptions` + resolves each result author in memory → SubscribedPeer "Following" vs NetworkUnfollowed `peer add`; NO new port, route, or seam — REUSES the slice-08 `start_with_indexer` + the slice-15 `peer add` seeding) |

## Driven internal (real)

| Port | Mechanism | Note |
|---|---|---|
| `StoragePort` (`adapter-duckdb`, the user's `openlore.duckdb`) | real DuckDB file under `OPENLORE_HOME`; seeded via the real `claim add` / `peer add` / `peer pull` verbs | slice-01/03/04; the source of truth; the indexer NEVER touches it (slice-05 ADR-023) |
| `StoreReadPort` (`adapter-duckdb`, the read-only view trait the `openlore ui` viewer holds) | SAME real DuckDB file; the viewer holds `StoreReadPort` + `IndexQueryPort` only (NO mutation method, NO key); reads seeded via the real CLI write verbs | slice-06..16 read-only viewer reads. slice-20 (ADR-057) adds TWO read-only presence reads `distinct_own_author_dids` (`SELECT DISTINCT author_did FROM claims`, seeded via the real `claim add` verb) + `distinct_cached_peer_author_dids` (`SELECT DISTINCT author_did FROM peer_claims`, NO `removed_at` filter, seeded via the real `peer add`+`peer pull`+`peer remove` no-`--purge` verbs); each read ONCE per `/search` render into an in-memory bare-DID set; mechanism UNCHANGED (real DuckDB seeded via real verbs). Per-read fault for the independent-degrade AT is a `#[cfg(debug_assertions)]` env seam (OQ-1 escalation — the real-binary subprocess harness cannot inject a fake-port `Err`), mirroring the slice-16 `OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` seam |
| `IndexStorePort` (`adapter-index-store`, the SEPARATE `index.duckdb`) | real SEPARATE DuckDB file (non-Option `author_did`; NO merged schema); seeded via the slice-05 ingest harness | slice-05; xtask check-arch extends `no_cross_table_join_elides_author` to the index-store SQL |
| `adapter-xrpc-query-server` + `adapter-index-query` (the B1 CLI↔indexer XRPC boundary) | real `openlore-indexer serve` over localhost ephemeral port + real CLI HTTP/XRPC client | slice-05; the response carries per-result `author_did` (D-D36 consumer-driven contract). slice-08: the `openlore ui` viewer is a SECOND consumer of the SAME `IndexQueryPort` (wired via `ViewerServer::start_with_indexer` → `OPENLORE_INDEXER_URL`); unreachable = `ClosedIndexerPort` (freed port), unconfigured = env unset → `SearchState::Unavailable` |
| `claim-domain` verify + compute_cid + `decode_ed25519_multibase` (pure core) | real pure core (reused at ingest; NO second verification path) | slice-01 verify/CID; slice-05 adds the ADR-026 production PLC `z6Mk` decode |
| `scoring` / `appview-domain` (pure cores) | direct in-process invocation (no adapter; no probe) | layer-2 `@property` + DELIVER mutation testing is the Earned-Trust analog |

## Driven external / non-deterministic (fake)

| Port | Fake | Note |
|---|---|---|
| `PdsPort` (the user's own ATProto PDS — publish) | `FakePds` (in-process HTTP, output-capturing; owns its tokio runtime) | slice-01 |
| peer-PDS (slice-03 federation source — `listRecords`) | `FakePeerPds` / `PeerPds` (in-process HTTP; canonical + adversarial peer fixtures) | slice-03; reused VERBATIM as the slice-05 discovery→federation funnel seed |
| `GithubPort` (the public GitHub API) | `FakeGithub` (recorded fixtures; authenticated/rate-limited/token-rejected postures) | slice-02 |
| `IdentityPort` (signing identity / DID resolution — author side) | `FakeIdentity` (deterministic Ed25519 seed) | slice-01/03 |
| `IngestSourcePort` (network ingest — bounded PULL of public records) | `FakeIngestSource` (bounded fixture record source hosting `listRecords`, incl. the adversarial set: unsigned / tampered-signature / cid-mismatch) | slice-05; INPUT-VALIDATES like the real adapter (nw-tdd-methodology Test Doubles contract) |
| `IdentityResolvePort` (verify-only PLC DID-doc → pubkey) | fixture PLC resolver carrying a REAL `z6Mk...` (a known test keypair) | slice-05; carries a real z6Mk so the ADR-026 decode it feeds runs the REAL path (the AV-4 gold test; the seam is release-forbidden) |

> Note: the policy CANNOT override the port-class defaults (a driven-internal
> port cannot become a fake without a documented waiver in
> `distill/wave-decisions.md`). It records only the MECHANISM for each default
> treatment. The slice-05 `IngestSourcePort` + `IdentityResolvePort` are
> driven-EXTERNAL (network ingest + PLC resolution) → FAKE is the correct
> default, not a waiver; the `IndexStorePort` + the B1 transport are
> driven-INTERNAL / driving → REAL.
