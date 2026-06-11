# ADR-057: `/search` four-arm follow-state — two LOCAL presence reads (own-claim author DIDs + cached-peer author DIDs) and a total precedence resolution completing the `AuthorRelationship` ADT

## Status

Accepted (DESIGN wave, slice-20 `viewer-search-full-follow-state`, 2026-06-11). Extends
ADR-053 (slice-16 binary `/search` follow-state). Owner: Morgan (nw-solution-architect).

## Context

slice-16 (ADR-053) resolved per-result follow-state on the read-only `GET /search` view
BINARY: author ∈ the operator's LOCAL active subscriptions → `SubscribedPeer` ("Following");
else → `NetworkUnfollowed` (the render-only `openlore peer add <did>` guidance). ADR-053
EXPLICITLY DEFERRED (DV-SF-3) the two further arms of the already-four-variant
`AuthorRelationship` enum (`crates/ports/src/federated_row.rs` line 67):

- **`You`** — the result is the operator's OWN claim (own DID). slice-16 resolved this to
  `NetworkUnfollowed`, re-offering a self-follow that can never run.
- **`UnsubscribedCache`** — the result is a cached claim from a peer the operator
  soft-removed (`openlore peer remove`, no `--purge`): present in the LOCAL `peer_claims`
  cache but NOT in the active set (the slice-15 PS-4 retained-cache residue). slice-16
  resolved this to `NetworkUnfollowed`, indistinguishable from a never-subscribed author.

The render `@match` in `viewer-domain` (line 1924) ALREADY carries `You | UnsubscribedCache
=> {}` (two empty arms); the LOCAL-graph resolver (`adapter-duckdb::attributed_claim_from`)
ALREADY resolves all four arms for the `/project`/`/philosophy` surfaces (where a row carries
a `source_table` column). What is missing is the `/search` EFFECT-shell resolution producing
the two arms — which needs LOCAL presence that the `StoreReadPort` does not yet expose,
because a NETWORK INDEX row carries no `source_table` (the index is per-user-neutral; it never
learns who you are or whom you removed). Presence must therefore be resolved by DID-set
membership against two NEW LOCAL reads.

Constraints (DISCUSS, slice-20 C-1..C-11): read-only / no key (CARDINAL); LOCAL/offline
resolution; the index stays per-user-neutral; batch-once per render (no N+1); additive /
no-regression byte-stable for the slice-16 arms (CARDINAL); independent per-read graceful
degrade; neutral framing; no new route / variant / crate (workspace stays 21); functional
paradigm (ADR-007 — pure resolution fn, effect shell at the read edge).

## Decision

### D1 — Two SEPARATE single-table read-only presence reads on `StoreReadPort`

Add TWO read-only methods to `StoreReadPort`, each a sibling of the established
`counter_presence_for` / `count_*` presence reads:

```rust
/// Distinct OWN-claim author DIDs (for the `You` arm). Read-only DISTINCT over `claims`.
fn distinct_own_author_dids(&self) -> Result<HashSet<String>, StoreReadError>;

/// Distinct CACHED-peer author DIDs incl. soft-removed (for `UnsubscribedCache`).
/// Read-only DISTINCT over `peer_claims`, NO `removed_at` filter (the residue cache is
/// the point — a soft-removed peer's cached rows are exactly what `UnsubscribedCache` is).
fn distinct_cached_peer_author_dids(&self) -> Result<HashSet<String>, StoreReadError>;
```

Each returns the full distinct-DID SET for the store (read ONCE per render), not a
per-row/per-input lookup. SQL shape (each a SINGLE-TABLE literal):

```sql
-- distinct_own_author_dids
SELECT DISTINCT author_did FROM claims
-- distinct_cached_peer_author_dids   (NO removed_at filter — peer_claims has no such column;
--                                      soft-remove lives in peer_subscriptions, NOT here)
SELECT DISTINCT author_did FROM peer_claims
```

No bound parameters are needed (whole-table distinct projection), so `params_from_iter` /
injection-safety is N/A here — there is no caller-supplied input in the SQL. (The `bare_did`
normalization happens in-memory on BOTH sides at the comparison site, per D2, never in SQL.)
The reads materialize directly into `HashSet<String>` (the membership shape the resolution
consumes). Each returns `Ok(HashSet::new())` for an empty store — a SUCCESSFUL read of zero,
distinct from a FAILED read.

### D2 — Total precedence resolution as a pure fn over three presence sets

In the effect shell (`resolve_search_state`), read the THREE LOCAL sets ONCE each — active
(slice-16 `read_local_active_set`, REUSED), own (D1 NEW), cached (D1 NEW) — into
`HashSet<String>` of BARE DIDs, then thread all three into `to_indexed_claim`. The
classification is a TOTAL pure function over `(author_did, &own, &active, &cached)` returning
the FIRST matching arm in precedence order:

```
resolve(author) =
  let bare = bare_did(author)
  if own.contains(bare)     -> You              // strongest fact: it is your own claim
  else if active.contains(bare) -> SubscribedPeer  // active subscription outranks stale cache
  else if cached.contains(bare) -> UnsubscribedCache  // cache-without-active = the residue
  else                      -> NetworkUnfollowed   // genuinely new
```

`bare_did` is applied on the RESULT side (the index `author_did` may carry the
`#org.openlore.application` fragment) AND the LOCAL sets are bare by construction — one
normalization at the comparison site (the slice-16 SSOT, extended to the own + cached sets).
The function is **exhaustive** (an `if/else if` chain whose final `else` covers all remaining
inputs — every author maps to exactly one of the four variants) and **deterministic** (a pure
total function of three set memberships; no clock, no I/O, no ordering dependence). The
precedence MIRRORS the LOCAL-graph resolver (`attributed_claim_from`: `'Own' → You`, active →
`SubscribedPeer`, cached-inactive → `UnsubscribedCache`), adapted to the network corpus where
presence is DID-set membership rather than the `'Own'`/`'Peer'` column.

### D3 — Two neutral render-only indicators filling the empty `match` arms

Fill the existing `You | UnsubscribedCache => {}` arm (`viewer-domain` line 1924) with:

```rust
AuthorRelationship::You              => (render_self_indicator()),
AuthorRelationship::UnsubscribedCache => (render_cached_unsubscribed_indicator()),
```

making the render a TOTAL `match` over all four variants (no empty arm remains). Each new
indicator is a render-only `<p>` sibling of `render_following_indicator`, each backed by its
own SSOT constant (mirroring `SEARCH_FOLLOWING_INDICATOR`):

- `SEARCH_SELF_INDICATOR = "Your own claim"` (DESIGN-named; neutral self-attribution)
- `SEARCH_REMOVED_CACHED_INDICATOR = "A peer you removed (cached)"` (DESIGN-named; neutral
  residue note)

Both render with the slice-16 neutral `"Relationship: "` prefix so neither is a bare element;
both are TEXT, no command/button/form/`hx-*`. Neither `You` nor `UnsubscribedCache` renders an
`openlore peer add` affordance (the add affordance is shown ONLY for `NetworkUnfollowed`). The
copy is NEUTRAL, never pejorative (no "ex-peer"/"abandoned"/"stale"/judgement — a render-gold
blocklist pins it). The `SubscribedPeer → render_following_indicator()` and `NetworkUnfollowed
→ render_follow_guidance(...)` arms are REUSED VERBATIM (byte-stable, no-regression).

### D4 — Independent per-read graceful degrade; NO new fault seam token

Each of the three LOCAL reads degrades INDEPENDENTLY via `unwrap_or_default()` → an EMPTY set
for the failed read only (mirroring `read_local_active_set` and slice-17's per-count `.ok()`
independence). A failed own read → no `You` arm (those authors fall through to the next
precedence step); a failed cached read → no `UnsubscribedCache` arm; a failed active read →
the slice-16 all-`NetworkUnfollowed` degrade. The worst case (all three fail) is exactly the
slice-08 status quo. No crash / 5xx / blank / leak.

**NO new `#[cfg(debug_assertions)]` fault seam token is added.** The slice-16
`OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ` seam already proves the degrade BRANCH SHAPE
(`unwrap_or_default()` on a `Result<_, StoreReadError>`) that the two new reads reuse
identically; a per-read fault is an in-memory test double over the `StoreReadPort` (a fake
returning `Err` for one read), needing no compiled-in env seam. Keeping the seam surface at
ONE token honors the "each token a distinct literal at its cfg-gated site" discipline without
widening the release-forbidden surface. (DISTILL/DELIVER MAY add per-read seam tokens IF the
acceptance harness cannot inject a per-read fault through a fake `StoreReadPort`; if so, each
new token MUST be a distinct literal at a `#[cfg(debug_assertions)]` site AND added to
`VIEWER_FAIL_SEAM_TOKENS` in `xtask`. The DESIGN default is NO new seam.)

## Alternatives Considered

### A1 — One combined read instead of two (REJECTED)

A single read returning both sets (e.g. a `UNION ALL` of `claims` + `peer_claims` projecting a
`source` discriminator, or a struct of two `HashSet`s). REJECTED because (a) it would be a
CROSS-STORE literal mentioning BOTH `claims` AND `peer_claims` in one SQL string — the exact
shape the `no_cross_table_join_elides_author` xtask rule guards (it would PASS only by
also projecting `author_did`, which it does, but it needlessly courts the rule and conflates
two independent reads); (b) it would couple the two arms' degrade — a single combined read
failing drops BOTH `You` and `UnsubscribedCache`, violating the independent-degrade contract
(C-8 / WD-FS-4); (c) two single-table reads are each trivially below the anti-merging rule
(neither names both tables), so they pass check-arch BY CONSTRUCTION with the simplest possible
proof. The two-reads cost is two `SELECT DISTINCT` per render (each batch-once, invariant to
result count) — negligible against the network index round-trip already in the request.

### A2 — A per-result presence query (N+1) (REJECTED)

Query own/cached presence per result author. REJECTED: the classic N+1 (C-4 / R-FS-2); the
established discipline (slice-12 `counter_presence_for`, slice-15/16 batch-once) is to read the
presence SET ONCE per render and classify each row in-memory. The two new reads are whole-store
distinct projections read once; classification is `HashSet::contains` per row.

### A3 — A held operator-identity surface for `You` instead of own-claim DID-set membership (REJECTED — the precedent slice's deferral blocker)

Resolve `You` against a held operator identity (the signing key's DID, an identity store, a
`/me` surface). REJECTED and EXPLICITLY OUT OF SCOPE: this was the precedent slice's deferral
rationale — giving the read-only viewer an identity/key surface breaks the CARDINAL read-only /
no-key invariant (the viewer holds `StoreReadPort` + `IndexQueryPort` only, no key). `You` is
resolved PURELY from the presence of the result author's bare DID in the operator's own
`claims` table — a read-only LOCAL fact, no identity surface, no key. The viewer stays keyless.

### A4 — A new `AuthorRelationship` variant or a `/search`-specific relationship type (REJECTED)

REJECTED: the enum is ALREADY four-variant (`You | SubscribedPeer | UnsubscribedCache |
NetworkUnfollowed`); the two target arms exist; the render `@match` already has the empty arms.
A new variant would ripple through every match site for zero benefit. This slice REUSES the
existing type (no new variant, no widening).

## Consequences

### Positive

- Completes the `/search` follow-state ADT to its full four arms; the `openlore peer add`
  affordance is now shown ONLY where it is actionable (a genuinely-new author) — 0% re-offered
  to the operator's own claims OR her soft-removed peers' cached claims (the J-005c accuracy
  fix, on top of slice-16's 0% to followed peers).
- Two single-table reads pass the `no_cross_table_join_elides_author` anti-merging rule BY
  CONSTRUCTION (neither names both `claims` AND `peer_claims`; each projects `author_did`
  anyway) — the simplest possible structural proof.
- The viewer stays keyless and read-only (CARDINAL): both new reads are read-only
  `StoreReadPort` methods (no mutation method added to the trait); `You` needs no identity
  surface; both new indicators are render-only TEXT.
- Independent per-read degrade keeps each arm's failure isolated; worst case = slice-08 status
  quo; no new release-forbidden seam surface (D4).
- The render is now a TOTAL `match` over all four variants — the compiler enforces
  exhaustiveness; no empty arm remains.
- Additive: the slice-16 `SubscribedPeer`/`NetworkUnfollowed` rendering + the original ranking/
  attribution/`[verified]`/confidence stay byte-stable (the two new arms only ADD).

### Negative / accepted trade-offs

- Two whole-store `SELECT DISTINCT` reads per `/search` render (in addition to the slice-16
  active-set read). Accepted: each is batch-once, invariant to result count, negligible against
  the network round-trip; both `author_did` columns are already indexed (slice-01/03 schema).
  A future large-own-claim-count store could project these incrementally, but at dogfood scale
  the distinct projection is immaterial.
- The precedence resolution now reads three sets where slice-16 read one — three independent
  degrade paths to keep correct. Mitigated: the degrade is the SAME `unwrap_or_default()` branch
  shape per read (D4), pinned behaviorally per-read.

## Earned Trust (probe contract)

The two new reads are read-only `StoreReadPort` methods over the SAME shared DuckDB connection
the viewer already probes at startup (ADR-030 store-readability probe: a `COUNT(*)` sentinel
read). No NEW external dependency, adapter, or substrate boundary is introduced — the reads ride
the already-probed connection. The substrate "lie" each new read must survive (a mid-request
read failure) is exercised BEHAVIORALLY via a fake `StoreReadPort` returning `Err` for the
target read (D4), proving the production `unwrap_or_default()` degrade branch runs per-read
without crash/5xx/leak. No new probe seam is warranted; the existing store-readability probe +
the per-read degrade test together discharge the Earned-Trust obligation for these reads.

## Enforcement

- **TYPE**: the two new methods are on the read-only `StoreReadPort` (no mutation method on the
  trait — a `Box<dyn StoreReadPort>` stays structurally incapable of mutating, I-VIEW-1); the
  render is a total `match` over the four-variant `AuthorRelationship` (compiler-enforced
  exhaustiveness).
- **STRUCTURAL**: `cargo xtask check-arch` — the `no_cross_table_join_elides_author` rule
  (each new read is single-table, passes by construction); the viewer-capability rule
  (read-only, no signing/identity/PDS); workspace stays 21. No new seam token, so
  `VIEWER_FAIL_SEAM_TOKENS` is UNCHANGED (D4).
- **BEHAVIORAL**: acceptance through the REAL `openlore ui` subprocess — four-arm resolution
  from three batch reads (own → `You`, active → `SubscribedPeer`, cached-inactive →
  `UnsubscribedCache`, else → `NetworkUnfollowed`); the active-and-cached precedence edge; the
  fragment-strip edge; the per-read independent degrade; the no-regression byte-stability of the
  slice-16 arms; the neutral-copy blocklist gold; the no-N+1 (each set read at most once,
  invariant to result count).
