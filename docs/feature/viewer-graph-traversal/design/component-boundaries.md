# Component Boundaries — viewer-graph-traversal (slice-10)

> Companion to architecture-design.md. The 6 existing touchpoints, the additions
> to each, and the cross-component invariants. **No new crate; 21 members.**

## Touchpoint map (the established slice-06..09 set)

```
adapter-http-viewer  (EFFECT, driving)  ── reads ──▶  ports::StoreReadPort  ◀── impl ──  adapter-duckdb (EFFECT, driven)
        │  groups in core + renders                                                              │ LOCAL SELECT
        ▼                                                                                         ▼
viewer-domain (PURE)  ── calls ──▶  claim-domain::confidence_bucket (PURE, REUSED)        LOCAL DuckDB store
        │                                                                                  (claims ∪ peer_claims)
        └── also already depends on ──▶  scoring, appview-domain (PURE, unchanged)
```

Dependency direction is inward (ADR-009). No adapter depends on another adapter.
Only `cli` links the adapters (composition root, unchanged).

---

## `crates/ports` — the read contract (PURE)

**Adds** to `store_read.rs`:

- `SurveyRow` — the boundary DTO for one attributed survey row. A FLAT DTO
  (mirrors `ClaimRow`/`PeerClaimRow`), carrying the load-bearing attribution:

  ```text
  pub struct SurveyRow {
      pub author_did: String,   // NON-Option (anti-merging, I-GT-3) — bare or fragmented as stored
      pub cid: String,          // NON-Option (no invented edges, I-GT-4) — every edge = one signed claim
      pub subject: String,
      pub predicate: String,
      pub object: String,
      pub confidence: f64,      // the stored DOUBLE — rendered VERBATIM (I-GT-5)
      pub origin: PeerOrigin,   // REUSED — Own rows vs Known peer (author_did + fetched_from_pds)
      pub composed_at: DateTime<Utc>,
  }
  ```

  > `origin` reuses the existing `PeerOrigin` ADT so a survey row knows whether it
  > is the operator's own claim or a peer's, with the peer attribution preserved
  > (the project/philosophy page shows Rachel's DID for a peer-origin edge). For an
  > OWN row the origin is `Own`-equivalent (DELIVER picks the representation; the
  > simplest is `PeerOrigin::Known` for peer rows and a dedicated own marker — see
  > data-models.md §SurveyRow origin).

- Two read methods on `StoreReadPort` (NO mutation method added):

  ```text
  /// Every attributed claim whose subject == `subject` (own ∪ LOCAL peer,
  /// UNION ALL, NO merge JOIN — anti-merging in pure Rust). Read-only. LOCAL
  /// only (no network). Empty Vec for a subject with no claims (Ok, not Err).
  fn query_project_survey(&self, subject: &str) -> Result<Vec<SurveyRow>, StoreReadError>;

  /// Every attributed claim whose object == `object` (own ∪ LOCAL peer, UNION
  /// ALL, NO merge JOIN). Read-only. LOCAL only. Empty Vec for an unknown object.
  fn query_philosophy_survey(&self, object: &str) -> Result<Vec<SurveyRow>, StoreReadError>;
  ```

**Boundary:** `ports` stays PURE (`async-trait` only; no I/O crate). The two
methods return owned value types; no lifetimes leak the connection.

---

## `crates/adapter-duckdb` — the read impls (EFFECT, driven)

**Adds** two impls to `store_read.rs::DuckDbStoreReadAdapter`, each the
read-only sibling of `query_contributor_scoring_feed` with the survey key swapped:

```text
// query_project_survey — filter on subject. UNION ALL claims + peer_claims,
// explicit author_did projection, NO merge/average JOIN (anti-merging SQL rule).
SELECT author_did, cid, subject, predicate, object, confidence, composed_at, fetched_from_pds, source_table FROM (
  SELECT c.author_did, c.cid, c.subject, c.predicate, c.object, c.confidence, c.composed_at,
         '' AS fetched_from_pds, 'Own' AS source_table
    FROM claims c WHERE c.subject = ?
  UNION ALL
  SELECT pc.author_did, pc.cid, pc.subject, pc.predicate, pc.object, pc.confidence, pc.composed_at,
         pc.fetched_from_pds, 'Peer' AS source_table
    FROM peer_claims pc WHERE pc.subject = ?
) ORDER BY object, source_table, cid
```

`query_philosophy_survey` is identical with `WHERE … object = ?` and
`ORDER BY subject, source_table, cid`. Both run over the SAME shared
`Arc<Mutex<Connection>>` (BR-VIEW-4) via the existing `lock_conn`; both map
errors to `StoreReadError` (never panic).

**Boundary / invariants:**
- The SQL names BOTH `claims` and `peer_claims` AND projects `author_did` → the
  `xtask` `no_cross_table_join_elides_author` rule stays GREEN.
- NO `GROUP BY` / `AVG` / `COUNT` over authors — the per-claim rows ARE the
  output; grouping is the pure core's job.
- LOCAL only — no network crate is reachable from this method.

---

## `crates/viewer-domain` — the view-model + render + grouping (PURE)

**Adds:**

- `TraversalView` ADT (the pure render input — total match):

  ```text
  pub enum TraversalView {
      Found { entity: String, groups: Vec<EdgeGroup>, contributors: Vec<String> },
      NoClaims { entity: String },     // guided "no claims" — names the entity, no fabricated edge
  }
  pub struct EdgeGroup { pub key: String, pub edges: Vec<EdgeRow> }   // key = the OTHER dimension
  pub struct EdgeRow { pub author_did: String, pub confidence: f64, pub cid: String }
  ```

- `TraversalView::group_project(entity, rows)` / `group_philosophy(entity, rows)`
  — PURE total functions that decompose `Vec<SurveyRow>` into attributed
  per-`(key, author, cid)` rows. For `/project`: group by `object`
  (philosophies embodied). For `/philosophy`: group by `subject` (projects that
  embody). `contributors` = distinct `author_did`s, order-preserved, deduped (a
  contributor spanning groups appears ONCE). NO averaging, NO merge.

- `render_project_fragment(&TraversalView) -> Markup` / `render_project_page(&TraversalView) -> String`
  and the philosophy mirror. The page EMBEDS the SAME fragment fn (parity). Each
  `EdgeRow` renders attributed: `author_did` (a link to `/score` — bare DID), the
  verbatim `render_confidence(confidence)`, the REUSED
  `claim_domain::confidence_bucket(confidence)` label, and the `cid`. Each group
  `key` is a traversal `<a href>` (philosophy → `/philosophy?object=…`; project →
  `/project?subject=…`). `NoClaims` renders the guided state naming the entity + a
  CLI next-step hint.

- Cross-link href helpers (the single source of truth for traversal hrefs):

  ```text
  pub fn href_project(subject: &str)  -> String   // "/project?subject=" + encode_query_component(subject)
  pub fn href_philosophy(object: &str) -> String  // "/philosophy?object=" + encode_query_component(object)
  pub fn href_score(author_did: &str)  -> String   // "/score?contributor=" + encode_query_component(bare(author_did))
  fn encode_query_component(value: &str) -> String  // percent-encode reserved/unsafe bytes (security; ADR-044)
  ```

- Cross-link wiring on the EXISTING renderers (US-GT-004): the subject cell →
  `href_project`, the object cell → `href_philosophy`, the author/contributor cell
  → `href_score`, on `render_claim_row`, `render_claim_fields` (detail),
  `render_peer_claim_row`, the `/score` breakdown rows, and the `/search`
  result rows. Render-only `<a href>` (no executable control).

**Boundary / invariants:**
- PURE — no I/O. New `[dependencies]` edge to `claim-domain` (promoted from
  dev-dep) is PURE→PURE (ADR-045).
- `render_confidence` reused verbatim (single site, I-GT-5). The bucket is the
  REUSED `claim_domain::confidence_bucket` (no viewer recompute, no second
  threshold table).
- Every `EdgeRow` carries `author_did` + `cid` (anti-merging + no-invented-edge,
  by construction).
- NO weight recompute, NO `/score`-style breakdown here — the contributor link is
  the J-002c boundary (link-out only, WD-GT-7).

---

## `crates/adapter-http-viewer` — the route handlers (EFFECT, driving)

**Adds** two handlers + two route-table arms (mirrors `score_page`):

```text
// route(): synchronous arms, after the async /search fork
PROJECT_URL    => Ok(project_page(store.as_ref(), query.as_deref(), shape)),
PHILOSOPHY_URL => Ok(philosophy_page(store.as_ref(), query.as_deref(), shape)),

fn project_page(store, query, shape) -> Response {
    let view = match query_param(query, "subject").filter(|v| !v.is_empty()) {
        None => TraversalView::NoClaims { entity: String::new() },  // bare /project -> guidance state
        Some(subject) => match store.query_project_survey(&subject) {
            Ok(rows) if rows.is_empty() => TraversalView::NoClaims { entity: subject },
            Ok(rows) => TraversalView::group_project(subject, rows),  // PURE grouping
            Err(_)   => TraversalView::NoClaims { entity: subject },  // degrade, never a stack trace
        },
    };
    match shape {
        Shape::Fragment => html_ok(render_project_fragment(&view).into_string()),
        Shape::FullPage => html_ok(render_project_page(&view)),
    }
}
```

`philosophy_page` mirrors it with `?object` + `query_philosophy_survey` +
`group_philosophy` + the philosophy renderers. Both reuse the existing
`query_param` + `percent_decode_form` (inbound decode) and the `Shape` fork.

**Boundary / invariants:**
- Read → group (pure) → render sandwich (ADR-007). NO `.await` (LOCAL, sync).
- The `ViewerServer` needs NO new field — both routes read the store it ALREADY
  holds (mirrors `/score`; NOT `/search`'s `IndexQueryPort` wiring).
- Persists nothing; renders no write/sign/follow control; loopback-only.

---

## `crates/claim-domain` — REUSED (PURE, deepest core)

No code change. `confidence_bucket(f64) -> ConfidenceBucket` (the WD-10
display-only bucket: Speculative/Weighted/WellEvidenced/Triangulated) is REUSED
by `viewer-domain`. The only change is the dependency *edge* (dev → regular),
adjudicated in ADR-045.

---

## `crates/xtask` — enforcement (tooling)

ONE change: add `viewer-domain → claim-domain` to the pure-core allowlist (so the
new `[dependencies]` edge passes the no-I/O arm). The viewer capability rule
(`VIEWER_FORBIDDEN_DEPS`) and the anti-merging SQL rule are UNCHANGED — they
already cover the new reads (claim-domain is not a forbidden dep; the survey
SELECTs project `author_did`). See ADR-045 for the exact delta.

---

## Cross-component invariants (DELIVER must keep green)

1. **Read-only (I-GT-1):** `StoreReadPort` has no mutation method; the two new
   reads are SELECTs; capability rule unchanged; behavioral read-only gold green.
2. **Anti-merging (I-GT-3):** `SurveyRow.author_did` non-Option; grouping in pure
   Rust; the survey SQL projects `author_did`; two-authors → two `EdgeRow`s.
3. **No invented edges (I-GT-4):** `SurveyRow.cid` + `EdgeRow.cid` non-Option;
   empty survey → `NoClaims` (no fabricated edge).
4. **LOCAL/offline (I-GT-2):** the reads touch only `claims`/`peer_claims`; the
   handlers hold only `StoreReadPort`; network-disabled scenario passes.
5. **Verbatim + bucket reuse (I-GT-5):** one `render_confidence` site; bucket via
   REUSED `claim_domain::confidence_bucket`; no weight recompute.
6. **Parity (I-GT-6):** page embeds the SAME fragment fn.
7. **Security (ADR-044):** claim-controlled URIs percent-encoded into hrefs.
8. **No new crate; 21 members.**
