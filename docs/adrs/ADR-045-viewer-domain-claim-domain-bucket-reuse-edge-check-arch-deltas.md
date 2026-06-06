# ADR-045: The `viewer-domain → claim-domain` Bucket-Reuse Dependency Edge (Dev-Dep → Regular), and the slice-10 `check-arch` Enforcement Deltas — No Capability Change, No New Crate (21 Members)

- **Status**: Accepted (slice-10 viewer-graph-traversal, DESIGN 2026-06-06). Resolves the bucket-reuse dep edge + the architecture-enforcement deltas.
- **Date**: 2026-06-06
- **Deciders**: Morgan (nw-solution-architect), for viewer-graph-traversal (slice-10).
- **Feature**: viewer-graph-traversal (slice-10)
- **Extends**: ADR-009 (hexagonal dep invariants + `check-arch`), ADR-022 (pure cores), ADR-029 (`viewer-domain` pure-core allowlist), ADR-037 (the slice-08 `viewer-domain → appview-domain` pure→pure allowlist edge), ADR-041 (the slice-09 `viewer-domain → scoring` pure→pure allowlist edge), WD-10 (the display-only confidence bucket in `claim-domain`).
- **Resolves**: the bucket-reuse dependency edge + the `xtask check-arch` deltas for slice-10.

## Context

ADR-043 reuses `claim_domain::confidence_bucket(f64) -> ConfidenceBucket` (the
WD-10 display-only bucket SSOT: Speculative/Weighted/WellEvidenced/Triangulated)
to render the per-claim bucket on each traversal edge — instead of duplicating the
thresholds in `viewer-domain`. But `viewer-domain` currently lists `claim-domain`
ONLY as a `[dev-dependencies]` entry (the `/score` render tests build an
`AttributedClaim` feed). The PRODUCTION renderer calling `confidence_bucket`
requires `claim-domain` as a regular `[dependencies]` edge.

`viewer-domain` is a PURE core (ADR-029): the `xtask check-arch` pure-core no-I/O
arm enforces it transitively depends on no `tokio`/`reqwest`/`duckdb`/`keyring`/
`atrium-*`. Any new `[dependencies]` edge must be confirmed pure. `claim-domain`
is the DEEPEST pure core (the `confidence_bucket` module is "no I/O, no serde";
its deps are pure `serde`/`unicode-normalization`). It is ALREADY transitively
present in `viewer-domain`'s graph: `viewer-domain → scoring → claim-domain` and
`viewer-domain → appview-domain → claim-domain`. So the direct edge adds NO new
transitive reachability — it only makes an already-present pure crate a DIRECT,
production dependency.

## Decision

**Promote `claim-domain` from a `[dev-dependencies]` to a regular `[dependencies]`
edge of `viewer-domain`, and ADD `viewer-domain → claim-domain` to the `xtask
check-arch` pure-core dependency allowlist (a pure→pure edge — the SAME shape as
the slice-08 `viewer-domain → appview-domain` and slice-09 `viewer-domain →
scoring` edges). The viewer capability rule and the anti-merging SQL rule are
UNCHANGED. No new crate; workspace stays 21 members.**

### The edge

`claim-domain` is a pure core (no I/O; its only deps are pure `serde` +
allowlisted `unicode-normalization`, ADR-009/WD-35). The edge
`viewer-domain → claim-domain` is PURE→PURE — no I/O enters `viewer-domain`
through it — exactly like the two existing pure→pure edges
(`appview-domain`, `scoring`), both of which ALREADY pull `claim-domain`
transitively. Making it a direct dependency is the canonical way to reuse the
one bucket SSOT (WD-10 / D-12) without a second threshold table.

### `check-arch` enforcement deltas (for software-crafter — DELIVER)

```markdown
Style: Hexagonal + Modular Monolith (UNCHANGED, ADR-009). Language: Rust
(functional, ADR-007 — pure cores: viewer-domain + claim-domain + scoring + appview-domain).
slice-10 deltas only:

  - cargo xtask check-arch:
      * ADD `viewer-domain -> claim-domain` to the pure-core dependency allowlist
        (a pure -> pure edge — claim-domain is the DEEPEST pure core; it is ALREADY
        transitively present via scoring + appview-domain, so this adds no new
        reachability). SAME shape as the slice-08 `viewer-domain -> appview-domain`
        (ADR-037) and slice-09 `viewer-domain -> scoring` (ADR-041) allowlist edges.
        Concretely: confirm the pure-core no-I/O arm for viewer-domain still PASSES
        with claim-domain a DIRECT dep (claim-domain's transitive deps are pure —
        serde + allowlisted unicode-normalization; no tokio/reqwest/duckdb/keyring/atrium-*).
      * NO capability-rule change: the two new StoreReadPort reads
        (query_project_survey / query_philosophy_survey) are read-only (methods on the
        port that already has NO mutation method, ADR-030); the viewer capability
        boundary (VIEWER_FORBIDDEN_DEPS) is UNCHANGED — claim-domain is a pure core,
        not a signing/identity/PDS/indexer surface, so it is NOT a forbidden dep.
        `adapter-http-viewer` MAY link claim-domain (pure) exactly as it MAY link
        viewer-domain / appview-domain / scoring / scraper-domain.
      * CONFIRM the anti-merging SQL rule (`no_cross_table_join_elides_author`) stays
        GREEN over the two new adapter-duckdb survey SELECTs — each names `claims` +
        `peer_claims` AND projects `author_did` (UNION ALL, no merge JOIN). (The rule
        already scans all of adapter-duckdb/src; the two new literals must pass it.)
      * The `only cli may link adapter-http-viewer` rule + the `no adapter depends on
        adapter` rule are UNCHANGED (no new adapter edge introduced).

  - cargo xtask check-probes: UNCHANGED — no new adapter/port with a probe; the two
    reads run over the existing probed StoreReadPort connection (ADR-028: store
    readable via count_claims + loopback bind). wire -> probe -> use holds unchanged.
  - cargo deny: NO new external dependency (claim-domain / ports / maud are all
    in-workspace; promoting a dev-dep to a regular dep adds no crate to the graph).
  - mutation testing (nightly): extend to viewer-domain render_project_*/render_philosophy_*
    + the TraversalView projection (group_project/group_philosophy) + the href_* helpers
    (anti-merging two-rows, verbatim confidence, bucket projected-not-recomputed,
    non-Option author/cid presence, percent-encoded-href round-trip, page-embeds-fragment).

Rules to enforce (slice-10):
- viewer-domain MAY depend on claim-domain (pure) and MUST NOT depend on duckdb/tokio/
  reqwest/std::fs/std::net/SystemTime or any adapter crate (existing pure-core no-I/O arm).
- StoreReadPort gains query_project_survey + query_philosophy_survey (read-only — NO
  mutation method added to the port).
- The two adapter-duckdb survey SELECTs are claims UNION ALL peer_claims, project
  author_did explicitly, and contain NO merge/average JOIN (anti-merging SQL rule green).
- GET /project + GET /philosophy persist nothing; render no sign/write/follow control;
  every cross-link is render-only navigation TEXT (<a href>, no executable control).
- render_project_page EMBEDS render_project_fragment; render_philosophy_page EMBEDS
  render_philosophy_fragment (page = chrome + fragment; parity by construction).
- Each survey edge carries a non-Option author_did + cid (anti-merging + no-invented-edge).
- Claim-controlled subject/object URIs are percent-ENCODED into hrefs (ADR-044 §security).
- ViewerServer::bind still refuses non-loopback (UNCHANGED, ADR-028).
- No new crate; workspace stays 21 members.
```

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **Inline the bucket thresholds in `viewer-domain`** | No new dep edge. | **Rejected (single SSOT, WD-10 / D-12).** The four thresholds (`0.3`/`0.7`/`0.9`) already live in `claim_domain::confidence_bucket`; a second copy is a drift hazard (a future threshold change would have to update two places — the exact bug WD-10 centralizes against). Reuse the one site. |
| **Move `confidence_bucket` into `ports`** (already a viewer-domain dep) | No allowlist change at all. | **Rejected (wrong home).** The bucket is a `claim-domain` confidence concern (it operates on a confidence `f64`, sibling to the `Confidence` smart constructor); `ports` is the port-trait crate, not the home for domain value logic. Promoting the EXISTING dev-dep to a regular dep is the minimal, correctly-homed change. |
| **Render the slice-04 scoring `WeightBucket` instead** (already a viewer-domain dep) | No new edge. | **Rejected (J-002c / ADR-043).** The scoring bucket is a per-pairing WEIGHT bucket, not a per-claim confidence bucket — wrong semantics for a survey edge, and it would require a weight recompute (forbidden, WD-GT-7). |
| **A new crate for the shared bucket** | Clean isolation. | **Rejected (no new crate; 21 members).** The slice's hard constraint is no new crate. The bucket already has a correct home (`claim-domain`); a new crate would be needless and break the 21-member count. |

## Consequences

### Positive
- One display-only-bucket SSOT (`claim_domain::confidence_bucket`) reused by both
  the CLI and the viewer — no drift, no second threshold table.
- The dep delta is minimal + already-precedented: ONE pure-core allowlist edge,
  the SAME shape as the slice-08/09 `viewer-domain → {appview-domain, scoring}`
  edges; the edge adds no new transitive reachability (claim-domain is already
  pulled via both).
- NO capability change, NO probe change, NO `cargo deny` change, NO new crate. The
  viewer still holds no key, binds loopback-only, persists nothing.

### Negative
- `viewer-domain` gains a direct `[dependencies]` edge (promoted from dev-dep) and
  the allowlist gains one entry. Accepted: a pure→pure edge to the deepest pure
  core, confirmed by the no-I/O arm; trivially the smallest change that reuses the
  bucket SSOT.

## Revisit Trigger
- If a future slice needs a NON-pure helper from `claim-domain` in `viewer-domain`
  (none exists today) → re-examine the edge against the pure-core no-I/O arm; the
  bucket reuse itself is pure and stable.
- A tightening of `check-arch` to deny-by-default for pure-core deps → these
  allowlist entries (`appview-domain`, `scoring`, `claim-domain`) keep the
  viewer-domain edges explicitly permitted (the allowlist is the audit record).
