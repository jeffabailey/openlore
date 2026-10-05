# Component Boundaries: bluesky-claim-review-app

> **Status: IMPLEMENTED** (delivered 2026-10-04, steps 01-01..03-05). Copied from
> `docs/feature/bluesky-claim-review-app/design/` at finalize; history in
> `docs/evolution/bluesky-claim-review-app-evolution.md`.

> WHAT each component owns and its contract shape. The crafter owns HOW. Interface names are
> contracts; internal decomposition is not prescribed.

## 1. New crates (4)

### `crates/review-domain`: PURE

The review/consent bounded context. It has no I/O, no async, no clock (time is passed in), and no
randomness (ids are passed in).

| Area | Responsibility | Contract shape |
|---|---|---|
| Ownership | Tokenize a GitHub bio and return an `OwnershipVerdict` against the session DID (ADR-076 §4). Mint a `VerifiedOwnership` capability **only** for `Verified`. | Pure function (return-only) |
| Suggestion lifecycle | ADTs `Suggestion` and `SuggestionState` = `Pending`, `Declined`, `Published{uri,cid}`, `Retracted{retraction_cid}`. Legal transitions only. Visibility is **derived** (pending + link verified ⇒ visible and approvable). | Pure function |
| Reconcile | Combine the existing keys with the newly derived candidates to get *new pending* plus a summary (new / already published / declined-hidden), per BR-1/BR-2. | Pure function |
| Plans | `PublishPlan`, `RetractPlan`, `SharePostPlan`: the exact record JSON, rkey (CID through `claim-domain`), destination, preview fields. Plans are **values**, so preview equals write. | Pure function (Plan-value; unbounded preservation: never writes) |
| Edits | Parse and validate confidence (0.00–1.00, 2 dp, then basis points per BR-4) and the philosophy swap (vocabulary list, unknown allowed per BR-5). | Pure function |
| Share text | Generate post text (≤300 graphemes) and a link facet with byte offsets from **published, non-retracted** claims only. | Pure function |
| Budget | Token-bucket and window arithmetic for the per-DID, per-IP and global limits (ADR-076). | Pure function (state in, state out) |
| Sign-in pin | Check that the session `sub` equals the resolved DID (AC-001.6). | Pure function |
| Views | maud renderers for every page and fragment (page = chrome + fragment, as in ADR-032). Copy SSOT constants ("not as truth", "Private: only you can see these", the three "never" commitments, error messages from AC-002.3, AC-004.6 and others). | Pure function |

Allowed deps: `claim-domain`, `lexicon`, `scraper-domain`, `ports` (value types only), `maud`,
`serde`, `serde_json`, `unicode-segmentation` (grapheme count, pure, MIT/Apache).

### `crates/adapter-atproto-oauth`: EFFECT

- Implements **`OAuthPort`** (driving the handshake): begin authorization for a handle, complete
  the callback, restore a session, revoke.
- Implements **`UserRepoWritePort`** (create-only; `WritableCollection` is a closed ADT of
  `Claim` | `Post`). It is constructed only per authenticated DID. There is **no
  update/put/delete/applyWrites method** (I-BRA-8 non-representable).
- Uses `atrium-oauth` + `atrium-identity` + `atrium-api` with `default-features=false`, over the
  workspace `reqwest` 0.12 rustls (ADR-073).
- Persists OAuth state **only** through `SecretStorePort`, wired from `adapter-review-store` by
  the composition root. It never depends on that crate directly.
- Contract shape: bounded change. The mutation set is records in the user's repo, limited to the
  two collections.

### `crates/adapter-review-store`: EFFECT (DuckDB, `review-app.duckdb`)

- Implements **`ReviewStorePort`** (owner data). Every method takes an `OwnerScope`, which can
  only be obtained from `SessionPort::resolve(cookie_hash)`.
- Implements **`SessionPort`** (browser sessions: create, resolve, end).
- Implements **`SecretStorePort`** (AEAD-encrypted OAuth state and session blobs keyed by DID or
  state id).
- Implements **`PlanStorePort`** (stores a plan with a TTL; takes a plan by `(OwnerScope, cid)`
  exactly once).
- Implements **`KpiCounterPort`** (increment `(day, event)`; no owner column).
- `purge(OwnerScope)` is the only multi-table delete and runs in a single transaction.
- Contract shape: bounded change. The mutation set is rows `WHERE owner_did = scope`; the
  universe is this file only.

### `crates/openlore-review-app`: DRIVER / BINARY (third composition root, ADR-072)

- `serve | probe | kpi`.
- Wire, probe, use. The probe outcome is `health.startup.refused` on any hard-arm failure.
- Contains the hyper router and HTTP concerns (cookies, CSRF, headers, `Shape` fork), the plan
  executor, the scan task and the in-memory limiter state.
- **Decides nothing.** Every branch on business meaning calls `review-domain`.

## 2. Ports (crate `ports`): additions and changes

| Port | Change | Capability notes |
|---|---|---|
| `OAuthPort` | NEW | Driving the handshake only. Returns `AuthenticatedIdentity {did, handle, pds_endpoint, granted_scopes}`. |
| `UserRepoWritePort` | NEW | Create-only, two collections. No read methods (reads go through the read-only ingest port). |
| `ReviewStorePort` | NEW | Owner-scoped. **Split read/write** (principle 12): a `ReviewStateRead` for page renders and a `ReviewStateWrite` for transitions. The profile route receives **neither**. |
| `SessionPort`, `SecretStorePort`, `PlanStorePort`, `KpiCounterPort` | NEW | See §1 |
| `IdentityResolvePort` | EXTEND | + resolve identity (handle or DID → `{did, verified_handle, pds_endpoint}`), read-only |
| `GithubPort` / `PersonProfile` | EXTEND | `PersonProfile.id: u64` (numeric GitHub id) |
| `RawRecord` (ingest) | CHANGE | `raw_payload: ClaimRecord` (was `SignedClaim`) + `repo_did` + `origin: RecordOrigin {AuthorPds, Relay}` |
| `SignedRecord` / `PeerRecordPage` (peer read) | CHANGE | Carries `ClaimRecord` + `repo_did` |
| `IndexedClaim`, search DTO, `PeerClaimRow` / `ClaimRow` | EXTEND | Optional or default `provenance` |

`claim-domain` (PURE, EXTEND): the `ClaimRecord` ADT, `Provenance` ADT, `RecordOrigin`, the pure
provenance verdict, and the new reject reasons `MalformedProvenance`, `ForeignRepo` and
`UnverifiableProvenance`. **`verify` is unchanged.**

`lexicon` (EXTEND): description text only for `author` and `signature` (ADR-071). The schema
`required[]` is unchanged.

## 3. Reused and extended crates

| Crate | Change |
|---|---|
| `scraper-domain` | **Unchanged.** Reused: `select_person_repos`, `derive_candidates`, `normalize_target`. |
| `adapter-github` | Map `id` into `PersonProfile`. Nothing else. |
| `adapter-atproto-did` | Implement resolve identity. |
| `adapter-atproto-pds` | Peer-read parse of an absent `signature` into `ClaimRecord::SelfAttested`, carrying the repo DID. |
| `adapter-atproto-ingest` | <ul><li>Populate `repo_did` (from the `at://` URI) and the rkey.</li><li>Enumerate **one repo DID** (`listRecords` requires `repo`) with cursor paging, bounded. Both the profile page and the indexer need this.</li><li>The adapter reports the base URL it fetched from. The **root** computes `origin` by comparing that URL with the freshly resolved DID-document PDS (ADR-071 §4). The adapter makes no trust decision.</li></ul> |
| `appview-domain` | `ingest_decision` calls the provenance verdict. The self-attested `Index` arm. `RejectReason::UnverifiableProvenance`. |
| `adapter-duckdb`, `adapter-index-store` | Additive `provenance` migration; nullable signature columns for self-attested. |
| `viewer-domain`, `cli` render | The `[self-attested]` marker. Existing markers unchanged. |
| `cli` (`peer pull`) | Calls the provenance verdict in its per-record evaluation. |
| `test-support` | `FakeOAuth`, `FakeUserRepoWrite` (records creates; **panics on any non-create**), `FakeReviewStore`, self-attested record fixtures (valid, foreign-repo, fragment-without-signature, relay-origin). |

## 4. Dependency direction

```text
openlore-review-app (root)
  -> adapter-atproto-oauth, adapter-review-store, adapter-github,
     adapter-atproto-ingest, adapter-atproto-did, adapter-system-clock
  -> review-domain -> {claim-domain, lexicon, scraper-domain, ports(values)}
adapters -> ports (+ pure cores); never adapter -> adapter
```

## 5. `xtask check-arch` deltas (specify only; DELIVER implements)

1. `COMPOSITION_ROOTS` += `openlore-review-app` (ADR-072). Reword the I-3 doc comment.
2. `check_pure_core_no_io("review-domain", …)`. `unicode-segmentation` is added to
   `PURE_CORE_ALLOWED_CRATES` if it is not already transitively allowed.
3. **NEW `check_review_app_capability_boundary`**:
   - The `openlore-review-app` transitive deps exclude `adapter-duckdb`, `adapter-atproto-pds`,
     `adapter-publish-http`, `adapter-http-viewer`, `adapter-xrpc-query-server`,
     `adapter-index-store` and `adapter-index-query`.
   - `cli` and `openlore-indexer` transitive deps exclude `adapter-atproto-oauth`,
     `adapter-review-store` and `openlore-review-app`.
   - Only `openlore-review-app` (plus the exempt tooling) may reach `adapter-atproto-oauth` or
     `adapter-review-store`.
4. **NEW source scan `review_app_holds_no_signing_identity`**: `crates/openlore-review-app/src/**`
   must not name `IdentityPort` or the keychain identity constructor (the token list is fixed at
   DELIVER, mirroring the `publish_write_capability` scan).
5. **NEW source scan `user_repo_write_is_create_only`**: `crates/adapter-atproto-oauth/src/**`
   must not contain `deleteRecord`, `putRecord` or `applyWrites` (I-BRA-8, structural layer; the
   type layer is the port's method absence).
6. **NEW SQL scan `review_store_owner_scoped_sql`**: in `crates/adapter-review-store/src/**`:
   - every SQL literal that names an owner table (`accounts`, `github_links`, `suggestions`,
     `scan_runs`, `plans`, `oauth_sessions`, `web_sessions`) must contain `owner_did`;
   - `DELETE` is permitted only in the `purge` and `expiry` modules;
   - `kpi_counters` must never contain `owner_did`.
7. `check-probes`: the two new adapters implement a non-stub `probe()` within the 250 ms budget
   (I-4/I-5). OAuth soft arms run asynchronously after startup.

`deny.toml`:

- No new bans. The `axum` and `actix-web` bans stay; reword the comment (ADR-072).
- `openssl-sys` stays banned, which catches an accidental `atrium-oauth` default-feature enable.
- Licenses of the new tree (`atrium-*` MIT, `jose-*` MIT/Apache, `hickory-*` MIT/Apache,
  `chacha20poly1305` and `p256` Apache/MIT, `dashmap` MIT, `unicode-segmentation` MIT/Apache) are
  inside the allowlist. CI verifies this.

## 6. Requirement → component trace

| Requirement / AC | Component(s) and mechanism |
|---|---|
| FR-1, AC-001.* | `OAuthPort` + review-domain sign-in pin + the router's session cookie |
| AC-000.* | Caddy route (ADR-075), the client-metadata route, the self-probe, the "temporarily unavailable" copy |
| FR-2, AC-002.* | review-domain ownership verdict + `GithubPort.read_person` + `github_links` |
| FR-3, AC-003.*, I-BRA-4 | Scan task: `VerifiedOwnership` gate → `select_person_repos` → `derive_candidates` → reconcile → per-repo persist |
| I-BRA-1, AC-003.4/5, NFR-1 | `OwnerScope` + split read/write ports + profile route without a store port + SQL rule + cross-owner probe |
| FR-4/5, AC-004.*, AC-005.* | `PublishPlan` (pure) → `PlanStorePort` → executor → `UserRepoWritePort.create` → read-back verify |
| FR-6, AC-006.*, I-BRA-2 | Pure transition, no write-port access, suppression by key |
| FR-7, AC-007.* | `/@handle`: resolve identity → ingest port → provenance verdict → retraction filter → view |
| FR-8, AC-008.*, I-BRA-6 | `SharePostPlan` built from the live published list only; confirm creates the post |
| FR-9, AC-009.*, I-BRA-5 | ADR-071 across claim-domain, peer read, ingest, stores, render |
| FR-10, AC-010.* | Rescan = scan pipeline; hidden-by-derivation; reconcile summary |
| FR-11, AC-011.*, I-BRA-8 | `RetractPlan` (a create), create-only port |
| FR-12, AC-012.* | `purge(OwnerScope)` single transaction + revoke; no PDS call |
| NFR-BRA-3 | Scopes (ADR-073) + create-only port |
| NFR-BRA-11 / OD-BRA-11 | `KpiCounterPort` (no owner column, enforced by rule 6) |
