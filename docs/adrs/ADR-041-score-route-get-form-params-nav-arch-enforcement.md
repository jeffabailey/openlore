# ADR-041: The `GET /score` Route — A GET Contributor Form, Always-Weighted+Breakdown Params, an Optional Author-Row Link, and the `check-arch` Enforcement Deltas

- **Status**: Accepted (slice-09 viewer-contributor-scoring, DESIGN 2026-06-05). Resolves OD-CS-1 + OD-CS-4 + OD-CS-5 + the arch-enforcement deltas.
- **Date**: 2026-06-05
- **Deciders**: Morgan (nw-solution-architect), resolving OD-CS-1 + OD-CS-4 + OD-CS-5 for viewer-contributor-scoring (slice-09).
- **Feature**: viewer-contributor-scoring (slice-09)
- **Extends**: ADR-028 (loopback-only viewer bind), ADR-030 (the read-only store port), ADR-031 (the vendored htmx asset), ADR-032/033 (the page=chrome+fragment split + the `Shape` fork in the effect shell), ADR-038 (the slice-08 `GET /search` GET-form + nav + capability-wiring precedent), ADR-039/040 (the read seam + the `ScoreState` projection this route drives).
- **Resolves**: OD-CS-1 (route shape) + OD-CS-4 (entry points) + OD-CS-5 (param surface).

## Context

US-CS-002 needs a real, user-invocable entry point that scores a contributor over the
local graph and renders the transparent weight + breakdown (ADR-040), with the
slice-07 progressive-enhancement contract (full page without `HX-Request`, fragment
with it) and the slice-06 invariants (loopback-only, read-only, no key, offline
chrome). Three open decisions shape the route surface:

- **OD-CS-1** route shape: `GET /score?contributor=<did>` vs a `/contributor/{did}`
  detail page vs a nav tab.
- **OD-CS-4** entry points: link `/score` from `/claims`//peer-claims author rows, or
  reach it only by the form/URL.
- **OD-CS-5** params: expose the slice-04 `--depth`/`--weighted` toggles, or always
  render the weighted+breakdown view.

The slice-08 `/search` route (ADR-038) already set the precedent: a GET form route,
forked by `Shape::from_request` read ONCE in `route`, threading an optional
capability into `ViewerServer`, reusing the vendored htmx chrome + the
page=chrome+fragment split.

## Decision

**`GET /score?contributor=<did>` — its OWN GET-form route, ALWAYS rendering the
weighted+breakdown view (no opaque mode, no depth toggle), reachable directly by
form/URL with an OPTIONAL render-only "score" link added from author rows. The
`ViewerServer` threads an `Option<SharedStore>`-based score handler via the existing
read-only store (no new wiring beyond the store it already holds); the `Shape` fork
and the vendored chrome are reused unchanged.**

### Route + form (OD-CS-1, OD-CS-5)

- **Route**: `GET /score` — its OWN route (the contributor-scoring corpus is the
  LOCAL graph + the pure scorer, distinct from `/claims` list reads, `/scrape`
  GitHub, `/search` network). A GET form (not a `/contributor/{did}` path-param page)
  so a score is bookmarkable/shareable as a URL and the no-JS path is a plain
  navigation — consistent with slice-07 `hx-push-url` + slice-08 `/search`.
- **Form**: `<form method="get" action="/score">` with ONE labeled `contributor`
  input; enhanced with `hx-get="/score"` + `hx-target="#score-results"` +
  `hx-swap="innerHTML"` (+ `hx-push-url="true"` so the address bar shows the real
  URL). The no-JS path is a plain `GET /score?contributor=<did>`.
- **Params (OD-CS-5)**: ALWAYS weighted + breakdown — the explain/weighted output is
  the DEFAULT; there is NO opaque (number-only) mode and NO `--weighted` toggle
  (transparency is non-optional here; J-002c). Depth uses the slice-04 default —
  `--depth` is a traversal concern, OUT of scope for the contributor-score view
  (the contributor score is a feed+score, not a graph walk). A bare `GET /score`
  (no `contributor`) renders `ScoreState::Form`.
- **No dimension toggle**: the ONLY dimension is `contributor` (the route name says
  so); there is no object/subject selector (unlike `/search`).

### Entry points (OD-CS-4)

- `/score` is reachable DIRECTLY by the form/URL — that is the contract.
- DESIGN ADDS (optional nicety): a render-only "score" link from author-DID rows on
  `/claims` and `/peer-claims` — an `<a href="/score?contributor=<did>">` (the
  natural "I see this DID; what does it add up to?" jump). It is GUIDANCE/navigation
  TEXT, never an executable control, and the link is OPTIONAL (the form/URL is the
  contract; DELIVER may defer the link). A nav link to `/score` MAY also be added
  alongside My Claims / Peer Claims / Search.

### Handler shape (effect shell — `adapter-http-viewer`)

```text
// route(): GET /score forks here (the Shape is read ONCE in route, ADR-033)
if path == SCORE_URL {
    return Ok(score_page(store.as_ref(), query.as_deref(), shape));   // sync local read + pure score
}

fn score_page(store: &dyn StoreReadPort, query: Option<&str>, shape: Shape) -> Response {
    let state = match parse_contributor(query) {                 // pure: ?contributor=<did> -> Option<did>
        None => ScoreState::Form,                                // bare /score
        Some(did) => match store.query_contributor_scoring_feed(&Did(did.clone())) {  // EFFECT: local read (ADR-039)
            Ok(feed) if feed.is_empty() => ScoreState::NoClaims { contributor: did },
            Ok(feed) => {
                let view = scoring::score(&feed, &ScoringConfig::DEFAULT);  // PURE compute (reused slice-04 core)
                ScoreState::Scored { contributor: did, view }
            }
            Err(_) => ScoreState::NoClaims { contributor: did },  // degrade to the guided state, never a stack trace
        },
    };
    match shape {                                                // Shape fork at the render call ONLY (ADR-033)
        Shape::Fragment => html_ok(render_score_results_fragment(&state).into_string()),
        Shape::FullPage => html_ok(render_score_page(&state)),
    }
}
```

`/score` is GET-only and synchronous (a LOCAL read + pure compute — NO `.await`,
unlike `/search`); it forks AFTER the synchronous store-read match in `route`. The
handler persists NOTHING and renders NO write/sign control. `parse_contributor`
reuses the existing `query_param` + `percent_decode_form` helpers (a DID may carry
`%` escapes). The `ViewerServer` already holds the read-only store (slice-06) — the
score route needs NO new wiring (no new `Option<Shared*>` field, unlike `/search`'s
`IndexQueryPort`): it reads the store it already holds. DELIVER decides whether to
gate `/score` behind a flag like `/scrape`/`/search` are, or always serve it (the
read is always available — recommend always serve).

### Earned Trust (principle 12; ADR-009)

The `/score` path adds NO new outbound dependency edge — it reads the LOCAL store the
viewer ALREADY probes. The existing `ViewerServer::probe` (ADR-028) is UNCHANGED:
store readable (sentinel `count_claims`) + loopback bind. The new
`query_contributor_scoring_feed` read runs over the SAME probed connection, so it is
covered by the existing store-readability probe — the composition-root invariant
**wire → probe → use** holds with no new probe. `viewer-domain` + `scoring` are PURE
(no `probe()`); their Earned-Trust analog is property + mutation testing of the
projection + the reused pure scorer (the running-sum==weight, sparse-renders-sparse,
verbatim-confidence, no-merge properties). An "environment lies" check: a feed read
that fails (poisoned lock / read error) degrades to the guided `NoClaims` state — no
crash, no stack trace, no hang — exactly the slice-06/08 degradation discipline.

## `check-arch` enforcement deltas (for software-crafter — DELIVER)

```markdown
Style: Hexagonal + Modular Monolith (UNCHANGED). Language: Rust (functional, ADR-007 —
pure cores: viewer-domain + scoring). slice-09 deltas only:

  - cargo xtask check-arch:
      * ADD `viewer-domain -> scoring` to the pure-core dependency allowlist (a pure -> pure
        edge — `scoring` is the slice-04 pure core, ADR-022; never reverses). This is the
        SAME shape as the slice-08 `viewer-domain -> appview-domain` allowlist edge (ADR-037/038).
      * CONFIRM the pure-core no-I/O arm still PASSES for viewer-domain WITH the new scoring edge
        (scoring's only non-pure-core deps are `ports` + `claim-domain` + pure `chrono`/`serde` —
        no I/O enters viewer-domain via this edge).
      * NO capability-rule change: the new `StoreReadPort::query_contributor_scoring_feed` read is
        read-only (a method on the port that already has NO mutation method, ADR-030); the viewer
        capability boundary (VIEWER_FORBIDDEN_DEPS) is UNCHANGED — `scoring` is a pure core, not a
        signing/identity/PDS/indexer surface, so it is NOT a forbidden dep. `adapter-http-viewer`
        MAY link `scoring` (pure) exactly as it MAY link `viewer-domain`/`appview-domain`/`scraper-domain`.
      * adapter-http-viewer gains a build dep on `scoring` (it calls `scoring::score` in the /score
        handler) — a pure crate; no capability breach (the viewer still holds no key, no write surface).

  - cargo xtask check-probes: UNCHANGED — no new adapter/port with a probe; the read runs over the
    existing probed StoreReadPort connection (ADR-028).
  - cargo deny: no new external dependency (scoring/ports/claim-domain/maud are all in-workspace).
  - mutation testing (nightly): extend to viewer-domain render_score_* + the WeightedView projection
    (running-sum == displayed weight, [SPARSE] projection, verbatim confidence/weight, no-merge rows,
    NoClaims no-leak).

Rules to enforce (slice-09 additions):
- viewer-domain MAY depend on scoring (pure) and MUST NOT depend on duckdb/tokio/reqwest/std::fs/
  std::net/SystemTime or any adapter crate (the existing pure-core no-I/O arm covers this).
- StoreReadPort gains query_contributor_scoring_feed (read-only — NO mutation method added to the port).
- GET /score persists nothing; renders no sign/write control; the author-row "score" link + nav link
  are render-only navigation TEXT (no executable control).
- render_score_page EMBEDS render_score_results_fragment (page = chrome + fragment; parity by construction).
- The breakdown sums to the displayed weight (projected from the SAME WeightedPairing — Gate 2 carried).
- [SPARSE] + the honesty counts are PROJECTED from the pure core's WeightBucket + counts (no viewer recompute).
- ViewerServer::bind still refuses non-loopback (UNCHANGED, ADR-028).
```

## Alternatives Considered

| Decision | Option | Evaluation | Rejected because |
|----------|--------|-----------|------------------|
| **OD-CS-1 route** | `/contributor/{did}` path-param detail page | RESTful path. | **Rejected.** A path-param page needs DID URL-encoding in the path (a DID carries `:`/`#`), is not a form the operator types into, and diverges from the slice-08 `/search` GET-form precedent. A GET query form is bookmarkable, typed, and reuses the exact slice-07/08 shape-fork + `hx-push-url` machinery. |
| **OD-CS-1 route** | A nav TAB (no route) | Glanceable. | **Rejected.** A score needs a contributor argument — a bare tab has nothing to score. The route is the contract; a nav LINK to `/score` is added as a nicety (OD-CS-4) but the route, not a tab, is the surface. |
| **OD-CS-5 params** | Expose `--weighted` / opaque toggle | Parity with the slice-04 CLI. | **Rejected (J-002c / I-CS-2).** An opaque (number-only) mode is the forbidden aggregator failure. The view is ALWAYS weighted + breakdown — transparency is non-optional. |
| **OD-CS-5 params** | Expose `--depth` | Parity with traversal. | **Rejected (out of scope).** Depth is a `--traverse` graph-walk concern (slice-04); the contributor score is a feed+score, not a walk. The browser traversal view is explicitly deferred (Out of Scope). Depth uses the slice-04 default, unsurfaced. |
| **OD-CS-4 entry** | Author-row link is the ONLY entry | Fewer surfaces. | **Rejected.** The form/URL is the bookmarkable contract; the link is a nicety that depends on it. The link is OPTIONAL and additive; the route stands alone. |
| **Wiring** | A new `Option<SharedScoring>` field on `ViewerServer` (mirror `/search`) | Symmetric with slice-08. | **Rejected (simplest-solution).** Unlike `/search`'s `IndexQueryPort`, `/score` needs NO new capability — it reads the store the viewer ALREADY holds and calls a PURE function. No new `ViewerServer` field; the handler takes the existing `store`. |

## Consequences

### Positive
- The route is bookmarkable/shareable + no-JS-navigable (GET form), and reuses the
  slice-07/08 `Shape` fork + page=chrome+fragment + vendored chrome unchanged
  (I-CS-7/I-CS-8).
- Transparency is non-optional by construction: there is no opaque param/mode to
  select (OD-CS-5) — the only render path is weighted + breakdown (ADR-040).
- No new wiring + no new capability: `/score` reads the already-held read-only store
  and calls the pure scorer; the `ViewerServer` is unchanged beyond the route + the
  build dep on `scoring`. The viewer holds no key, binds loopback-only, persists
  nothing (I-CS-1/I-CS-4/I-CS-9).
- The `check-arch` delta is minimal + already-precedented: ONE pure-core allowlist
  edge (`viewer-domain → scoring`), the SAME shape as slice-08's
  `viewer-domain → appview-domain`; NO capability-rule change.

### Negative
- A new GET route + handler in `adapter-http-viewer` + the optional author-row link
  edits in `viewer-domain`. Accepted: the symmetric counterpart to the slice-08
  `/search` handler, reusing the route table, the `Shape` fork, and the chrome.

## Revisit Trigger
- The author-row "score" link or a `/score` nav link proves load-bearing in dogfood →
  promote it from optional to part of the contract (a DISTILL scenario).
- A future need to gate `/score` (e.g. a store-less viewer) → add an `Option`-gated
  handler like `/scrape`/`/search`; the route + ADT stay total.
