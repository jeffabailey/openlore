# Data Models — viewer-graph-traversal (slice-10)

> Companion to architecture-design.md / component-boundaries.md. The boundary
> DTO, the pure view-model ADTs, the survey SQL shape, and the href encoding.
> **Zero new persisted type** (surveys are computed per query, WD-GT-9).

## 1. Read-side boundary DTO — `ports::SurveyRow`

The flat row the two survey reads return (one per matching signed claim).
Mirrors `ClaimRow`/`PeerClaimRow`/`AttributedClaim` (FLAT DTO, not `SignedClaim`).

| Field | Type | Source | Load-bearing? |
|---|---|---|---|
| `author_did` | `String` (NON-Option) | `claims.author_did` / `peer_claims.author_did` | **YES** — anti-merging (I-GT-3); never elided |
| `cid` | `String` (NON-Option) | `claims.cid` / `peer_claims.cid` | **YES** — every edge maps to one signed claim (I-GT-4) |
| `subject` | `String` | `.subject` | the project key |
| `predicate` | `String` | `.predicate` | (e.g. `embodiesPhilosophy`) — shown, not grouped on |
| `object` | `String` | `.object` | the philosophy key |
| `confidence` | `f64` | `.confidence` (DOUBLE) | rendered VERBATIM (I-GT-5); never rounded/recomputed |
| `origin` | `PeerOrigin` (REUSED) | `'Own'` vs (`author_did`, `fetched_from_pds`) | "mine vs peer" attribution (BR-VIEW-5 carried) |
| `composed_at` | `DateTime<Utc>` | `.composed_at` | ordering/tiebreak only (not displayed on the edge) |

### `origin` representation (DELIVER picks; recommended)

The existing `PeerOrigin` ADT is `Known { author_did, fetched_from_pds } |
Unknown`. A survey spans BOTH own and peer rows, so DESIGN recommends the
adapter set `origin` from the `source_table` discriminant:

- own row (`source_table = 'Own'`) → an `Own`-equivalent marker (DELIVER may add
  a `PeerOrigin::Own` arm OR carry a separate `is_own: bool`; the simplest is to
  reuse `Known { author_did, fetched_from_pds: "" }` for peers and a dedicated
  own marker — the renderer only needs "is this mine or a peer's, and which peer").
- peer row (`source_table = 'Peer'`) → `Known { author_did, fetched_from_pds }`.

> The renderer's contract is only: show the `author_did` VERBATIM (never elided)
> and let the operator distinguish own from peer. The exact `origin` shape is a
> DELIVER detail; the load-bearing fact (non-Option `author_did` + `cid`) is fixed
> here.

## 2. Pure view-model ADTs — `viewer_domain`

```text
/// The pure render input for a project/philosophy survey (depth-1: the entity +
/// its direct attributed edges). Total match in the renderer.
pub enum TraversalView {
    /// ≥1 claim about the entity: the grouped, attributed edges + the distinct
    /// contributors. Grouping is in PURE Rust (anti-merging), never SQL.
    Found {
        entity: String,            // the queried subject (project) or object (philosophy)
        groups: Vec<EdgeGroup>,    // keyed by the OTHER dimension
        contributors: Vec<String>, // distinct author_did, order-preserved, deduped (spanning author appears ONCE)
    },
    /// Zero claims (or bare route / read error): the guided "no claims" state
    /// naming the entity — NEVER a fabricated edge (I-GT-4).
    NoClaims { entity: String },
}

/// One group of attributed edges sharing the OTHER-dimension key.
/// /project: key = object (a philosophy embodied). /philosophy: key = subject (a project).
pub struct EdgeGroup {
    pub key: String,           // a traversal target: <a href> to /philosophy or /project
    pub edges: Vec<EdgeRow>,   // one row per (author, cid) — NEVER averaged
}

/// One attributed traversal edge = one signed claim.
pub struct EdgeRow {
    pub author_did: String,    // non-Option attribution; a link to /score (bare DID)
    pub confidence: f64,       // VERBATIM (render_confidence); + display-only bucket
    pub cid: String,           // non-Option; every edge maps to exactly one claim
}
```

### Grouping rules (pure, anti-merging)

- `group_project(entity, rows)`: group `rows` by `object`; within a group, one
  `EdgeRow` per `(author_did, cid)`; two authors on the same object → two rows.
  `contributors` = distinct `author_did` across all rows.
- `group_philosophy(entity, rows)`: group by `subject`; same per-row rule.
- Empty `rows` → `NoClaims { entity }`.
- NO average, NO "consensus" row, NO dedup that collapses authors. A contributor
  spanning multiple groups is deduped ONLY in `contributors` (appears once there),
  never in the per-group edges.

### Worked examples (from the journey)

**US-GT-002 Ex 1 — cargo, two authors:**
`query_project_survey("github:rust-lang/cargo")` → 2 rows (maria 0.90, rachel
0.88) on object `…dependency-pinning`. `group_project` →
`Found { entity: "github:rust-lang/cargo", groups: [EdgeGroup { key:
"…dependency-pinning", edges: [{maria, 0.90, cid1}, {rachel, 0.88, cid2}] }],
contributors: ["did:plc:maria-test", "did:plc:rachel-test"] }`. Renders two
attributed rows (0.90 triangulated; 0.88 well-evidenced), never averaged; both
DIDs link to `/score`.

**US-GT-003 Ex 2 — nixpkgs, two authors, one project, no merge:**
`query_philosophy_survey("…reproducible-builds")` includes nixpkgs by maria
(0.92) and tobias (0.70). `group_philosophy` → group key `github:NixOS/nixpkgs`
with TWO `EdgeRow`s — never one `0.81` row.

**US-GT-002 Ex 3 / US-GT-003 Ex 3 — no claims:**
empty `rows` → `NoClaims { entity }` → guided "No claims about this project/for
this philosophy in your local graph" naming the entity, suggesting a CLI step.
Exit 200, no fabricated edge.

## 3. Display-only bucket (REUSED, not redefined)

`claim_domain::confidence_bucket(f64) -> ConfidenceBucket` (WD-10 thresholds):

| Range | Bucket | Display label |
|---|---|---|
| `[0.0, 0.3)` | `Speculative` | speculative |
| `[0.3, 0.7)` | `Weighted` | weighted |
| `[0.7, 0.9)` | `WellEvidenced` | well-evidenced |
| `[0.9, 1.0]` | `Triangulated` | triangulated |

The viewer PROJECTS this; it recomputes NO bucket and NO threshold (one SSOT).
`0.90 → triangulated`, `0.88 → well-evidenced`, `0.25 → speculative` (the journey
mockup values). This is DISTINCT from the slice-04 scoring `WeightBucket`
(Strong/Moderate/Sparse, used on `/score`); the traversal edge shows the
per-claim CONFIDENCE bucket, never a weight bucket (J-002c boundary).

## 4. Survey SQL shape (LOCAL, read-only, anti-merging)

```sql
-- query_project_survey(subject): one row per signed claim about the subject.
SELECT author_did, cid, subject, predicate, object, confidence, composed_at, fetched_from_pds, source_table
FROM (
  SELECT c.author_did, c.cid, c.subject, c.predicate, c.object, c.confidence, c.composed_at,
         '' AS fetched_from_pds, 'Own' AS source_table
    FROM claims c WHERE c.subject = ?
  UNION ALL
  SELECT pc.author_did, pc.cid, pc.subject, pc.predicate, pc.object, pc.confidence, pc.composed_at,
         pc.fetched_from_pds, 'Peer' AS source_table
    FROM peer_claims pc WHERE pc.subject = ?
) ORDER BY object, source_table, cid;
```

`query_philosophy_survey(object)`: identical with `WHERE … object = ?` and
`ORDER BY subject, source_table, cid`. Both name `claims` + `peer_claims` AND
project `author_did` (anti-merging SQL rule green); neither uses
`GROUP BY`/`AVG`/`COUNT` over authors (grouping is the pure core's job). LOCAL
only; empty result → empty `Vec` (Ok, not Err).

## 5. Href encoding (security boundary — ADR-044)

Subject/object are **claim-controlled** strings (they originate from signed
claims, which a peer may author). On the way OUT they are percent-encoded into
the href query component so a hostile URI (`a&b`, `x"><script>`, `…#frag`) cannot
break the attribute or inject markup:

```text
href_project(subject)  = "/project?subject="     + encode_query_component(subject)
href_philosophy(object) = "/philosophy?object="   + encode_query_component(object)
href_score(author_did)  = "/score?contributor="   + encode_query_component(bare_did(author_did))
```

`encode_query_component` percent-encodes everything outside the unreserved set
(`A–Z a–z 0–9 - _ . ~`), so `/`, `:`, `#`, `&`, `<`, `>`, `"`, space all become
`%XX`. The inbound side (`query_param` → `percent_decode_form`) DECODES, so the
round-trip is exact: `github:rust-lang/cargo` → `github%3Arust-lang%2Fcargo` →
(decoded) `github:rust-lang/cargo` matches the stored `subject` verbatim.

> maud auto-escapes attribute *text*, which already blocks `"`/`<` breakout in the
> rendered HTML; explicit percent-encoding is **defense-in-depth** AND the correct
> way to carry `/`,`:`,`#` as a single query value (so traversal continuity — the
> linked subject resolves to the SAME `/project` survey key — holds for every URI).

## 6. Persistence

NONE. Surveys + `TraversalView` are computed per query and never written
(WD-GT-9 / I-GT-8). No schema change, no migration, no new table/column.
