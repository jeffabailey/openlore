# Journey (visual): traverse-the-graph-in-the-browser — slice-10

> Companion to `journey-traverse-the-graph-in-the-browser.yaml` (structured schema
> with embedded Gherkin per step). Persona: **P-001 Maria** (node operator, slice-04
> graph-explorer hat). Job: **J-002b** — traverse contributor↔project↔philosophy edges.
> READ-ONLY, LOCAL-only, offline. Brownfield DELTA on slices 04/06/07/08/09.

## Emotional arc (mirrors the slice-04 grounding journey: Orienting → Connecting)

```
flat-list-curiosity  ──►  orienting  ──►  THE AHA (Connecting)  ──►  defensibly-connected
   (skeptical:           (a survey         (click an edge →           (cite the exact
    "I have claims        page shows        Rachel spans cargo          signed claims,
    but cannot SEE        who claims        AND nixpkgs — the           each attributed,
    the connections")     what about        non-obvious span)          verbatim confidence)
                          this entity)
```

## Flow (each edge is a CLICK; the browser's back/forward is the traversal stack)

```
 [/claims row]                  Activity 1: LAND ON AN ENTITY
   cargo · dependency-pinning · did:plc:rachel-test · 0.88
        │ click subject
        ▼
 GET /project?subject=github:rust-lang/cargo          Activity 2: SURVEY ITS EDGES
   Philosophies embodied:
     dependency-pinning   did:plc:maria-test   0.90 [triangulated]  cid bafy…1
                          did:plc:rachel-test  0.88 [well-evidenced] cid bafy…2   ◄ no merge
   Contributors who claimed:
     did:plc:maria-test  →/score      did:plc:rachel-test →/score
        │ click object                         │ click contributor
        ▼                                       ▼
 GET /philosophy?object=…dependency-pinning   GET /score?contributor=did:plc:rachel-test
   Projects that embody this:                  (slice-09 — REUSED, not rebuilt:
     github:rust-lang/cargo  →/project          transparent weighted breakdown)
     github:NixOS/nixpkgs    →/project   ◄── Activity 3: TRAVERSE TO NEXT ENTITY
   Contributors:                               THE AHA: Rachel spans cargo AND nixpkgs
     did:plc:rachel-test →/score
        │
        ▼  Activity 4: GROUND THE FINDING — every edge = one signed claim (cid),
           attributed (author_did), verbatim confidence; citable; LOCAL; offline.
```

## TUI / HTML mockups per step

### Step 1 — Land on an entity (cross-link, US-GT-004)

```
+-- GET /claims (rows now clickable) --------------------------------------+
| subject                  predicate          object             author    |
| [github:rust-lang/cargo] embodiesPhilosophy [dependency-      [did:plc:  |
|   ^link →/project           pinning]          rachel-test]      0.88     |
|                             ^link →/philosophy ^link →/score             |
| Every subject/object/contributor cell is a traversal edge (a link).      |
+--------------------------------------------------------------------------+
```

### Step 2 — Survey an entity (project page, US-GT-002)

```
+-- GET /project?subject=${subject} ---------------------------------------+
| Project: ${subject}                                                      |
| Philosophies embodied (LOCAL graph; own ∪ peer; no merge):               |
|   ${object}                                                              |
|     ${author_did} (you)     ${confidence} [${bucket}]   cid ${claim_cid} |
|     ${author_did} (peer)    ${confidence} [${bucket}]   cid ${claim_cid} |
|     ^ two authors → two attributed rows, never averaged                  |
| Contributors who claimed this project:                                   |
|   ${author_did} →/score       ${author_did} →/score                      |
| Each edge is one signed claim. No invented edges. LOCAL · offline.       |
+--------------------------------------------------------------------------+
```

### Step 2b — Survey an entity (philosophy page, US-GT-003)

```
+-- GET /philosophy?object=${object} --------------------------------------+
| Philosophy: ${object}                                                    |
| Projects that embody this (LOCAL; own ∪ peer; no merge):                 |
|   ${subject} →/project   ${author_did} ${confidence} [${bucket}] cid …   |
|   ${subject_2} →/project ${author_did} ${confidence} [${bucket}] cid …   |
| Contributors who claimed it:                                             |
|   ${author_did} →/score   ◄── spans BOTH projects above (the aha)        |
+--------------------------------------------------------------------------+
```

### Step 3 — Traverse to a contributor (reuses slice-09 /score)

```
+-- GET /score?contributor=${author_did}  (slice-09 — REUSED) -------------+
| Transparent weighted adherence breakdown (J-002c — already shipped).     |
| Reached HERE by clicking a contributor edge; this slice adds the LINK,   |
| not the page.                                                            |
+--------------------------------------------------------------------------+
```

### Empty / sparse / boundary states (US-GT-002/003)

```
+-- GET /project?subject=github:nonexistent/repo --------------------------+
| No claims about this project in your local graph.                        |
| Queried: github:nonexistent/repo                                         |
| Next: run `openlore graph query --subject …` or `openlore scrape …`.     |
| (200 OK · no fabricated edge · LOCAL · offline)                          |
+--------------------------------------------------------------------------+
```

## Shared-artifact note

Every `${variable}` above is documented in
`shared-artifacts-registry.md` with its single source of truth. The load-bearing
ones — `author_did`, `claim_cid`, `confidence` — MUST be identical across every
surface they appear on (anti-merging + verbatim confidence are integration
invariants, not per-page choices).

## Progressive enhancement

Each route serves a FULL page without `HX-Request` (no-JS / bookmark / direct URL)
and a FRAGMENT of the same results region with it (slice-07 `Shape` fork; page =
chrome + fragment). Cross-links are plain `<a href>` — a no-JS click is a full
navigation. htmx is the vendored local asset; both traversal routes need NO network.
