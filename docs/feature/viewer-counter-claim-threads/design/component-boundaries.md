<!-- markdownlint-disable MD013 MD024 -->
# Component Boundaries: viewer-counter-claim-threads (slice-11)

> DESIGN · Morgan · 2026-06-06 · reuse-first DELTA · no new crates (21 members)

Defines the responsibility of each touched crate, its inbound/outbound ports, and the
structural enforcement of every slice-11 invariant. Internal implementation (function
bodies, control flow) is the software-crafter's domain (GREEN + REFACTOR); this document
fixes only the BOUNDARIES and CONTRACTS.

---

## Touched crates (all pre-existing)

```
GET /claims/{cid}
      │  (driving port — HTTP)
      ▼
adapter-http-viewer (effect)  ── depends on ──▶ ports (StoreReadPort trait)
      │  builds view-models, calls render                     ▲
      │                                                       │ implemented by
      ├── reads via &dyn StoreReadPort ──▶ adapter-duckdb (effect) ── reads ──▶ DuckDB + artifacts
      │
      └── calls ──▶ viewer-domain (PURE: CounterThread ADT + render)
```

### `crates/ports` (PURE)

- **Add** to the `StoreReadPort` trait:
  `fn query_counter_claims(&self, target_cid: &str) -> Result<Vec<CounterClaimRow>, StoreReadError>;`
- **Add** the `CounterClaimRow` boundary DTO (see `data-models.md`).
- **Constraint**: NO mutation method may be added. The trait remains structurally
  write-free (I-CT-1). No new dependency (the DTO uses existing types: `String`, `f64`,
  `DateTime<Utc>`, `PeerOrigin`).

### `crates/adapter-duckdb` (EFFECT shell)

- **Add** the `query_counter_claims` impl on `DuckDbStoreReadAdapter`
  (`src/store_read.rs`).
- **Inbound**: the `StoreReadPort` trait method.
- **Outbound**: the shared `Arc<Mutex<Connection>>` (read-only SELECT) + `read_artifact_at`
  for the `reason` (own: `claims.artifact_path`; peer: `peer_claims.signed_record_path`).
- **Constraint**: read-only — SELECT only, no INSERT/UPDATE/DELETE. Reuses the
  `lock_conn` poison-recovery + the established UNION-ALL anti-merging form. Returns
  `Ok(vec![])` for an un-countered target.

### `crates/viewer-domain` (PURE)

- **Add** the `CounterThread` ADT + `CounterEntry` view-model + a projection from
  `Vec<CounterClaimRow>` → `CounterThread`.
- **Extend** `render_claim_detail_fragment` to render the neutral "Countered" flag +
  the thread BENEATH the existing `render_claim_fields` + `render_evidence_section`.
- **Inbound**: called by `adapter-http-viewer` with an already-shaped view-model.
- **Constraint**: PURE — no I/O, no `chrono`/`duckdb`/socket edge; total functions;
  reuses `render_confidence` (single-site) + the `(you)` annotation + the
  `/claims/{counter_cid}` link.

### `crates/adapter-http-viewer` (EFFECT shell)

- **Extend** `claim_detail_page` (`src/lib.rs`): on the `Ok(Some(detail))` arm, also
  call `store.query_counter_claims(cid)`, project to `CounterThread`, and pass it to the
  extended render. The `Shape` fork (fragment vs full page) is reused UNCHANGED.
- **Constraint**: no new route; read-only; no network; degrade a counter-read error to
  the SAME guided handling as the claim read (no stack trace leak).

### `crates/cli` (composition root)

- No code change. The concrete `DuckDbStoreReadAdapter` satisfies the extended trait
  once the method lands. Single composition root preserved (ADR-009).

### `xtask`

- No allowlist edge; no rule change. The viewer capability rule + anti-merging rule
  remain in force and passing (see `architecture-design.md` §6).

---

## Invariant enforcement (each invariant → a structural point)

| ID | Invariant | Enforced where (structural) | Backed by |
|---|---|---|---|
| I-CT-1 | Read-only viewer | `query_counter_claims` is a read method on a no-mutation trait; viewer adapter links no signing/PDS surface | type system + `check_viewer_capability_boundary` + behavioral gold (store row counts unchanged) |
| I-CT-2 | Shown, never applied | original claim built from `get_claim` and rendered UNCHANGED; counters are a SEPARATE `CounterThread` ADT rendered below; no code path feeds a counter back into the claim's confidence/fields | shown-never-applied gold (confidence byte-identical with/without counters) |
| I-CT-3 | Attribution without merging | `CounterThread::Countered { counters: Vec<CounterEntry> }` has NO aggregate variant; SQL UNION-ALL projects `author_did` + `cid` explicitly (no merging cross-store JOIN/GROUP BY/AVG) | anti-merging gold (two counters → two items, no "disputed by N") + `no_cross_table_join_elides_author` |
| I-CT-4 | Verbatim confidence | original confidence flows through the single `render_confidence`; counters carry no derived score | existing render_confidence property tests |
| I-CT-5 | LOCAL / offline | read over the shared connection + local artifacts; no network seam on the method; only vendored `/static/htmx.min.js` referenced | offline acceptance scenario + no-CDN scan |
| I-CT-6 | Progressive-enhancement parity | thread rendered INSIDE `render_claim_detail_fragment`; full page embeds that fragment | structural (one fragment fn) + parity gold |
| I-CT-7 | No new crates | `Cargo.toml` members unchanged | workspace-member count (21) |

---

## Edge-case dispositions (binding contracts)

| Edge case | Disposition | Where decided |
|---|---|---|
| Un-countered CID | `query_counter_claims` returns `Ok(vec![])`; `CounterThread::None`; NO section, NO "0 counters" noise | US-CT-001 Ex 3 / US-CT-003 |
| Empty-reason counter (peer record, ADR-015 wire-optional) | `reason` decodes to `None`/empty → `CounterEntry` renders an explicit **"no reason provided"** state (author DID + CID still shown); never a crash, never a blank line | US-CT-002 Ex 3 (ADR-047) |
| Unknown CID | `get_claim` returns `None` → existing guided 404; NO counter thread/flag added to the 404 path | US-CT-003 Ex 3 |
| A counter that is itself countered | **depth-1 only — do NOT recurse.** The counter's CID links to `/claims/{counter_cid}`; drilling there renders ITS own thread via the same route. No nested render. | US-CT-002 / out-of-scope (ADR-047) |
| Purged-peer counter | absent by construction (it lived in `peer_claims`, removed by hard-purge); operator's OWN counters persist (they live in `claims`) | feature-delta J-003c boundary |
| Unreadable/missing artifact for a counter | the adapter surfaces a `StoreReadError`; the handler degrades to the guided read handling (no stack trace) | NFR-VIEW-6 reuse |

---

## What this slice does NOT add (boundary guard)

- No write/sign/counter control on any surface (authoring stays the slice-03 CLI).
- No "net verdict" / "consensus" / "disputed by N" aggregate (no such ADT variant).
- No network seam, no signature re-verification, no CID recomputation on this route.
- No nested/recursive counter render.
- No `/claims` LIST-row annotation (deferred to slice-12).
- No new crate, no new route, no new KPI ID.
