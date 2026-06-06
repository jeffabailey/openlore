# ADR-044: The `GET /project` + `GET /philosophy` Routes and the Cross-Link Hrefs — Bare-DID `/score` Targets, Percent-Encoded Claim-Controlled URIs (an Injection Boundary), and Render-Only Navigation

- **Status**: Accepted (slice-10 viewer-graph-traversal, DESIGN 2026-06-06). Resolves WD-GT open-questions Q1 + Q2 + the US-GT-002/003/004 route + cross-link shape.
- **Date**: 2026-06-06
- **Deciders**: Morgan (nw-solution-architect), resolving Q1 + Q2 for viewer-graph-traversal (slice-10).
- **Feature**: viewer-graph-traversal (slice-10)
- **Extends**: ADR-028 (loopback-only bind), ADR-030 (read-only store port), ADR-031 (vendored htmx), ADR-032/033 (page=chrome+fragment + the `Shape` fork), ADR-038 (the slice-08 `/search` GET-form precedent + the handle→DID resolver), ADR-041 (the slice-09 `/score` GET-form route + the bare-`?contributor=` read), ADR-042/043 (the read seams + the `TraversalView` projection this route drives).
- **Resolves**: WD-GT Q1 (contributor DID href form) + WD-GT Q2 (subject/object href percent-encoding) + the US-GT-002/003 route shape + US-GT-004 cross-link wiring.

## Context

US-GT-002/003 need two user-invocable routes; US-GT-004 needs every
subject/object/contributor cell on the existing surfaces to become a traversal
`<a href>`. Two DESIGN-owned decisions shape the hrefs:

- **Q1 (contributor DID form):** does a contributor edge link to `/score` with the
  BARE `did:plc:rachel-test` or the app-identity `…#org.openlore.application` the
  slice-08 `/search` resolver lifts to?
- **Q2 (subject/object percent-encoding):** subject/object URIs contain `/`, `:`,
  `#` — and, because they are CLAIM-CONTROLLED, possibly hostile `&`/`<`/`"`. How
  are they carried in an href without breaking the URL or injecting markup?

The slice-08/09 route precedent is fixed: a GET form route, the `Shape` fork read
ONCE in `route`, the inbound `query_param` → `percent_decode_form` decode, the
page=chrome+fragment split.

## Decision

**Add `GET /project?subject=<uri>` and `GET /philosophy?object=<uri>` as their own
synchronous GET-form routes (mirroring `/score`); wire the cross-links as
render-only `<a href>` in `viewer-domain` via three single-source-of-truth
helpers. Contributor edges link to `/score` with the BARE DID (Q1). Claim-controlled
subject/object/DID values are PERCENT-ENCODED into the href query component (Q2) —
an explicit injection boundary, not just maud auto-escaping.**

### Routes (US-GT-002/003)

```text
pub const PROJECT_URL: &str    = "/project";      // viewer-domain (one source of truth)
pub const PHILOSOPHY_URL: &str = "/philosophy";

// route(): synchronous arms, after the async /search fork (alongside /score, /claims)
PROJECT_URL    => Ok(project_page(store.as_ref(), query.as_deref(), shape)),
PHILOSOPHY_URL => Ok(philosophy_page(store.as_ref(), query.as_deref(), shape)),
```

`project_page` parses `?subject` via the existing `query_param`/`percent_decode_form`,
calls `store.query_project_survey(&subject)` (ADR-042), builds the `TraversalView`
via the PURE `group_project` (ADR-043), and forks by `Shape` at the render call.
`philosophy_page` mirrors it. Both are GET-only, SYNCHRONOUS (LOCAL read + pure
group/render — NO `.await`, unlike `/search`), persist nothing, render no
write/sign/follow control, and need NO new `ViewerServer` field (they read the
store the viewer already holds — like `/score`, not `/search`). A bare
`/project`/`/philosophy` (no param) → the guided no-claims/guidance state (200).

### Q1 — contributor cross-link form: BARE DID

A contributor edge links to `/score?contributor=<bare-did>`, e.g.
`/score?contributor=did:plc:rachel-test`.

**Why bare (not app-identity):** `/score` is LOCAL. Its read,
`query_contributor_scoring_feed` (ADR-039), matches the local feed with
`bare_did(contributor)` + `LIKE '<bare>%'` — it STRIPS any `#fragment` and
prefix-matches. The slice-08 `/search` resolver lifts to
`…#org.openlore.application` ONLY because the *indexer* matches `author_did`
EXACTLY (a different corpus, ADR-038). Traversal targets the LOCAL `/score`, which
expects the bare form; the survey rows already carry the bare `author_did` from
the local store. So the correct, simplest form is to link the stored `author_did`
verbatim, stripping any fragment via the existing `bare_did` helper. (Should a DID
arrive fragmented, `href_score` strips the fragment so the `/score` prefix match
still lands.)

### Q2 — percent-encode claim-controlled URIs (an injection boundary)

The three href helpers are the SINGLE source of truth for traversal hrefs:

```text
pub fn href_project(subject: &str)   -> String { format!("{PROJECT_URL}?subject={}",     encode_query_component(subject)) }
pub fn href_philosophy(object: &str) -> String { format!("{PHILOSOPHY_URL}?object={}",    encode_query_component(object)) }
pub fn href_score(author_did: &str)  -> String { format!("{SCORE_URL}?contributor={}",    encode_query_component(bare_did(author_did))) }

/// Percent-encode every byte outside the unreserved set (A-Z a-z 0-9 - _ . ~).
/// So `/`,`:`,`#`,`&`,`<`,`>`,`"`,space all become %XX. PURE total function — the
/// encode side of the existing percent_decode_form decode (round-trips exactly).
fn encode_query_component(value: &str) -> String;
```

**This is a SECURITY decision, not just a URL-correctness one.** Subject and
object originate from SIGNED CLAIMS, which a PEER may author — they are
**attacker-influenced strings**. Without encoding, a hostile subject like
`x"><script>…` or `a&object=evil` could break out of the `href` attribute /
inject a second query param / inject markup. Encoding every reserved/unsafe byte
makes the value a single, inert query component:

- `github:rust-lang/cargo` → `github%3Arust-lang%2Fcargo`
- `org.openlore.philosophy.reproducible-builds` → unchanged (all unreserved)
- a hostile `x"><script>` → `x%22%3E%3Cscript%3E` (inert)

The inbound side (`query_param` → `percent_decode_form`) DECODES, so the
round-trip is exact and traversal continuity holds (the linked subject resolves to
the SAME `/project` survey key — the journey's `subject`/`object` shared-artifact
consistency check). maud already auto-escapes attribute TEXT (blocking `"`/`<`
breakout in the rendered HTML); explicit percent-encoding is **defense-in-depth**
AND the correct transport for `/`,`:`,`#` as a single query value. Belt and
suspenders, because the input is claim-controlled.

### Cross-link wiring (US-GT-004, render-only)

The subject/object/contributor cells on `render_claim_row`, the `/claims/{cid}`
detail fields, `render_peer_claim_row`, the `/score` breakdown rows, and the
`/search` result rows render via `href_project` / `href_philosophy` / `href_score`
as plain `<a href>` — a no-JS click is a full navigation; an `hx-get` swap is an
OPTIONAL nicety where the target is an in-page panel. No executable control, no
write surface, no new route, no new data. A network `/search` row cross-links INTO
the LOCAL `/project`/`/philosophy` — traversal stays local even when the entry
point was a network search (no traversal route ever reaches the network).

### Earned Trust (principle 12; ADR-009)

The routes add NO new outbound dependency — they read the already-probed LOCAL
store. `ViewerServer::probe` (ADR-028) is UNCHANGED (store readable + loopback);
**wire → probe → use** holds with no new probe. The "environment lies" check: a
survey read that fails degrades to the guided `NoClaims` state — no crash, no
stack trace. The injection boundary (Q2) is the Earned-Trust application to a
hostile INPUT (a claim-controlled URI is a dependency you don't trust): the
`encode_query_component` + the maud auto-escape together demonstrate the route
honors its contract even when a peer's claim lies about being well-formed —
property + mutation tested (a hostile URI round-trips inert; the linked key
matches the stored key).

## Alternatives Considered

| Decision | Option | Evaluation | Rejected because |
|----------|--------|-----------|------------------|
| **Q1** | App-identity `…#org.openlore.application` (mirror `/search`) | Symmetric with slice-08. | **Rejected.** `/score` is LOCAL and prefix-matches via `bare_did` + `LIKE`; lifting to the app identity would mismatch the local read's expectation. The app-identity lift is correct ONLY for the indexer's exact match (`/search`). The local survey rows carry the bare DID; link it verbatim. |
| **Q2** | Rely on maud attribute auto-escaping ALONE | Less code. | **Rejected (security + correctness).** Auto-escaping blocks `"`/`<` breakout but does NOT make `/`,`:`,`#`,`&` a valid single query value — an unencoded `&` in a subject would inject a second param, breaking traversal continuity AND opening a param-smuggling vector. Explicit percent-encoding is required for correctness and is defense-in-depth for injection. |
| **Q2** | A heavy URL-encoding crate (`percent-encoding`/`url`) | Battle-tested. | **Rejected (no new dependency; symmetry).** The codebase already hand-rolls `percent_decode_form` (to avoid a dep edge); the encode side is a tiny pure mirror. A new external crate for one pure function violates the no-new-dependency constraint and the established hand-rolled-decode precedent. |
| **Route shape** | `/project/{subject}` path-param page | RESTful. | **Rejected.** A path-param page needs the URI (with `/`,`:`) encoded INTO the path, is not a typed/bookmarkable form, and diverges from the slice-08/09 `/search`//score GET-query-form precedent (ADR-038/041). A GET query form is bookmarkable, reuses the `Shape` fork + chrome unchanged. |
| **Wiring** | A new `Option<Shared*>` field on `ViewerServer` | Symmetric with `/search`. | **Rejected (simplest-solution).** Both routes read the store the viewer ALREADY holds and call PURE functions — no new capability (the `/score` verdict, ADR-041). No new `ViewerServer` field. |

## Consequences

### Positive
- The routes are bookmarkable/no-JS-navigable GET forms reusing the slice-07/08/09
  `Shape` fork + page=chrome+fragment + vendored chrome unchanged.
- The contributor link lands correctly on the LOCAL `/score` (bare DID matches the
  local prefix read) — the slice-09 terminus is REUSED, not rebuilt.
- Claim-controlled URIs cannot inject markup or smuggle params: percent-encoding
  the href value is an explicit injection boundary (defense-in-depth over maud's
  auto-escape), property + mutation tested.
- The three href helpers are the single source of truth — a mutation to a
  traversal URL has exactly three sites, all pinned.
- No new wiring, no new capability, no network on these routes (LOCAL/offline).

### Negative
- Two new GET routes + handlers in `adapter-http-viewer`, plus cross-link edits to
  five existing renderers in `viewer-domain`. Accepted: the symmetric counterpart
  to the slice-09 `/score` handler; the cross-link wiring is the US-GT-004 value
  (render-only `<a href>`, no new data path).

## Revisit Trigger
- A future surface needs the app-identity DID on a LOCAL link → add a resolver at
  that call site; the bare-DID default for `/score` is unchanged (the local read
  is prefix-based).
- A claim-URI scheme is added that needs different encoding → extend
  `encode_query_component` (the single site); the helpers are unchanged.
