<!-- markdownlint-disable MD024 -->
# Feature Delta: viewer-graph-traversal

> Wave: **DISCUSS** (lean mode + ask-intelligent)
> Feature type: User-facing (two new READ-ONLY browser views + cross-links on the `openlore ui` viewer)
> Walking skeleton: N/A — brownfield DELTA (no Feature-0 skeleton; the viewer already runs). Thinnest end-to-end thread = US-GT-002 (the philosophy page).
> UX depth: Lightweight (server-rendered maud HTML + htmx progressive enhancement — inherits slices 06/07/08/09)
> JTBD: YES — every story traces to **J-002** / sub-job **J-002b** (`docs/product/jobs.yaml`); no new job, no new sub-job created
> Brownfield DELTA on: `openlore-scoring-graph` (slice-04, the grounding journey + traversal contract), `htmx-scraper-viewer` (slice-06), `viewer-htmx-swaps` (slice-07), `viewer-network-search` (slice-08), `viewer-contributor-scoring` (slice-09)
> Date: 2026-06-06 · Owner: Luna (nw-product-owner)

This file is the canonical DISCUSS-wave delta for `viewer-graph-traversal`
(slice-10): a **graph-traversal / entity-navigation** surface added to the
read-only `openlore ui` viewer. Two new routes — a **project (subject) page**
(`GET /project?subject=…`) and a **philosophy (object) page**
(`GET /philosophy?object=…`) — turn the viewer from a set of *flat lists* into a
*navigable graph*. Each row on `/project`, `/philosophy`, `/claims`, `/score`,
and `/search` becomes a clickable **edge** that traverses to the next entity. It
is the **browser realization of J-002b** ("traverse contributor↔project↔philosophy
edges") — the one unshipped J-002 sub-job, and the headline "aha" of the
explore-the-graph journey (the **Orienting → Connecting** transition).

This is a DELTA. It REUSES the slice-04 LOCAL graph-query read path + the
slice-04 scoring buckets (display-only, no recompute), the slice-06/07
page=chrome+fragment render pattern, the slice-09 `/score` contributor route
(every contributor edge links to it — built, not rebuilt), and the slice-08/09
read-only `StoreReadPort` + `Shape` fork. It adds exactly TWO new read
capabilities — a *project-survey* read and a *philosophy-survey* read over the
LOCAL store (claims ∪ peer_claims, never merged) — plus the cross-link wiring.
Tier-1 content is inlined here (lean); SSOT lives under `docs/product/`; the
per-journey + registry artifacts live under `discuss/`.

---

## Wave: DISCUSS / [REF] Persona ID

**P-001 Senior Engineer Solo Builder** ("Maria", the node operator) — the SAME
persona as slices 06/07/08/09 (`docs/product/personas/senior-engineer-solo-builder.yaml`).
She lives in a terminal but runs `openlore ui` to GLANCE at her store in a
browser (slice-06), navigate it without reloads (slice-07), discover the network
(slice-08), and read a contributor's transparent score (slice-09). slice-10
extends that same read-only viewer with **traversal**: from any claim, project,
philosophy, or contributor she is looking at, she can follow an edge to the next
related entity — and surface the *non-obvious connection* (a contributor who
spans two projects she is evaluating) that she could never get from `gh search`
plus skimming READMEs.

slice-04 framed **P-002 Researcher/Tech Lead** as primary for the CLI
graph-explorer surface (`graph query … --traverse`); the BROWSER viewer's
operator, though, is **P-001** (the viewer is her surface, slices 06–09). She
wears the **graph-explorer hat** (`docs/product/personas/researcher-tech-lead.yaml`,
`hats[].id: graph-explorer`) at her own loopback viewer. UX guardrails inherited:
read-only, never silently mutate, attribution always visible / no "merged
consensus" framing, confidence display must never read as "the system thinks this
is true."

---

## Wave: DISCUSS / [REF] JTBD One-Liner

> **J-002b** (sub-job of **J-002**): *When I want to find the non-obvious
> connection — who spans the projects I'm evaluating — I want to traverse the
> graph's edges, where each edge is one signed claim, so I can surface an
> alignment I could never get from `gh search` plus skimming READMEs.*

slice-10 is the **browser UI** for J-002b. J-002 is validated in slice-04
(opportunity score 14, `walking_skeleton_for: openlore-scoring-graph`); J-002b is
its load-bearing, **as-yet-unshipped-in-the-browser** sub-job. The CLI shipped
`graph query … --traverse` in slice-04; the contributor *score* dimension shipped
in the browser as `/score` in slice-09; **traversal in the browser is the gap
this slice fills.** No new job, no new sub-job. Every story below traces to
J-002b, with the J-002a/J-002c boundaries explicit:

| Sub-job | Name | In this slice? | Stories |
|---|---|---|---|
| **J-002b** | Traverse contributor↔project↔philosophy edges | **YES — headline** | US-GT-002, US-GT-003, US-GT-004 |
| J-002a | Query the graph by dimension (subject / object / contributor) | **PARTIAL — the project/philosophy pages ARE the subject/object query-by-dimension surfaces in the browser; they are the *entry node* of every traversal.** The contributor dimension already shipped as `/score` (slice-09). | US-GT-002, US-GT-003 (entry nodes) |
| J-002c | See transparent, auditable adherence weighting | **NO (boundary) — out of scope.** This slice shows VERBATIM confidence + the slice-04 display-only bucket on each edge row, and LINKS to `/score` for the full weighted breakdown; it adds NO new weighting surface and recomputes NO weight. | (link-out only) |

**J-002a/J-002c boundary, stated explicitly:** the project/philosophy pages are
*surveys* (every attributed claim about an entity, grouped, no merge) — they are
J-002a orienting surfaces whose VALUE is that each row is a J-002b traversal edge.
They are NOT a new weighting surface (J-002c stays at `/score`, slice-09) and NOT
a new authoring/network surface (J-001/J-005 are elsewhere). A reader who wants
"why does this project rank?" clicks through a contributor edge to `/score`.

---

## Wave: DISCUSS / [REF] JTBD Scope / Contradiction Gate

| Check | Result |
|---|---|
| Does every story trace to a job? | YES — all 4 trace to J-002b (US-GT-001 is `@infrastructure` enabling it). |
| New job or sub-job needed? | NO — J-002b already exists (slice-04). This is a new SURFACE for it. |
| Contradiction with a shipped invariant? | NONE. Read-only (KPI-VIEW-2), anti-merging (KPI-GRAPH-2 / KPI-FED-1), local-first (KPI-5 / KPI-GRAPH-6), no-invented-edges (slice-04 traversal contract) all carried forward unchanged. |
| Boundary overlap with J-002a/J-002c? | RESOLVED above: J-002a entry-node reused; J-002c link-out only, no new weighting surface. |
| Boundary overlap with J-005 (network discovery, `/search`)? | NONE — traversal reads the **LOCAL** store only; `/search` reads the network indexer. Distinct corpora, distinct routes. A `/search` row may cross-link INTO `/project`/`/philosophy` (local) but traversal never reaches the network. |
| Walking skeleton as Feature-0? | NO — brownfield; the viewer already runs. (Per the task's explicit instruction.) |

**Verdict: PASS.** No new job, no contradiction, boundaries explicit.

---

## Wave: DISCUSS / [REF] Scope Assessment (Elephant Carpaccio)

| Signal | This slice |
|---|---|
| User stories | 4 (1 `@infrastructure` + 3 value-producing) — under 7 ✔ |
| Bounded contexts / modules | viewer-domain (pure) + adapter-http-viewer (effect) + adapter-duckdb (read impl) + ports + cli + xtask — the SAME 6-touchpoint set as slices 06–09, an established pattern ✔ |
| New crates | 0 (workspace stays 21 members) ✔ |
| Integration points | 2 new read methods on `StoreReadPort` (project-survey, philosophy-survey) + cross-link href wiring on existing renderers — bounded ✔ |
| Estimated effort | ≤ 1 day ✔ |
| Independent user outcomes | 1 (traverse the local graph in the browser) ✔ |

**Scope Assessment: PASS — 4 stories, 6 touchpoints (no new crate), estimated ≤ 1 day.**
Right-sized; no split needed. (A thinner split was considered — ship only
`/philosophy` first — but `/project` and `/philosophy` are symmetric surfaces over
the same new read pattern and the same render pattern, so splitting them would
double the integration-test scaffolding cost for a fraction of a day's saving.
The cross-link story US-GT-004 is the *connective tissue* that makes traversal an
actual journey rather than two isolated pages, so it ships in the same slice.)

---

## Wave: DISCUSS / [REF] Locked Decisions

See `discuss/wave-decisions.md` for full rationale. Summary (WD-GT-*):

| # | Decision | Status |
|---|---|---|
| WD-GT-1 | Brownfield DELTA on slices 04/06/07/08/09. Two new GET routes: `GET /project?subject=<uri>` (the project survey) and `GET /philosophy?object=<uri>` (the philosophy survey). The contributor dimension is NOT a new route — every contributor edge links to the slice-09 `/score?contributor=<did>`. | LOCKED |
| WD-GT-2 | Persona = P-001 (Maria, the node operator) wearing the slice-04 graph-explorer hat — the viewer's operator. | LOCKED |
| WD-GT-3 | Viewer stays **READ-ONLY**: traversal is a READ; no new write/sign/follow route; no key in the process. Inherits I-VIEW-1/2/3 / KPI-VIEW-2 / KPI-HX-G3. The three-layer enforcement (StoreReadPort no-mutation type + xtask viewer capability rule + behavioral gold) carries verbatim. | LOCKED |
| WD-GT-4 | **LOCAL-only / offline.** Both routes read the LOCAL DuckDB store (claims ∪ peer_claims) — NO network seam (distinct from `/search`, which hits the indexer, and `/scrape`, which hits GitHub). Inherits KPI-5 / KPI-GRAPH-6 / KPI-VIEW-5. Both routes work network-down. | LOCKED |
| WD-GT-5 | **Anti-merging in surveys.** A project/philosophy survey is an AGGREGATE VIEW that NEVER merges authors: two claims on the same (subject, object) by two authors render as two attributed rows; no average, no "consensus" row. Inherits KPI-GRAPH-2 / KPI-FED-1 / I-FED-1. Every edge carries its `author_did` (non-Option, load-bearing) and `cid`. | LOCKED |
| WD-GT-6 | **No invented edges.** Every displayed edge maps to exactly ONE signed claim (its `cid`). Traversal walks existing claims only; it fabricates no relationship. Inherits the slice-04 traversal contract. An empty survey renders the entity with "no claims" — never a fabricated edge. | LOCKED |
| WD-GT-7 | **Verbatim confidence + display-only bucket (no recompute).** Each edge row shows the claim's numeric confidence VERBATIM (`0.90`, never `0.9`/`90%`) and the slice-04 display-only bucket (speculative / weighted / well-evidenced / triangulated). The viewer recomputes NO weight; the full transparent weighted breakdown stays at `/score` (slice-09, J-002c). Inherits KPI-4 / FR-VIEW-8 / WD-10. | LOCKED |
| WD-GT-8 | **Progressive enhancement + offline chrome.** Both routes serve a full page without `HX-Request` and a fragment of the same results region with it (the slice-07 `Shape` fork; page = chrome + fragment, parity by construction). Cross-links are plain `<a href>` that ALSO work as htmx `hx-get` swaps where they target an in-page panel; a no-JS click is a full navigation to the target page. htmx stays the vendored, SHA-256-pinned local asset. Inherits I-HX-1..5 / KPI-HX-G1 / KPI-HX-G2. | LOCKED |
| WD-GT-9 | **Zero new persisted types; loopback-only bind unchanged (127.0.0.1).** Surveys are computed per-query, never persisted. Inherits BR-VIEW-2 / I-VIEW-1 / I-VIEW-4. | LOCKED |
| WD-GT-10 | **Out of scope (explicit):** NO authoring/sign/counter affordance; NO network on these routes; NO new weighting surface (link to `/score`); NO bounded-depth multi-hop tree render in THIS slice — a survey is depth-1 (the entity + its direct edges); each edge is a LINK that the operator clicks to traverse the next hop (the browser's back/forward IS the traversal stack). Multi-hop auto-expansion (the slice-04 CLI `--depth K` tree) is deferred. | LOCKED |

---

## Wave: DISCUSS / [REF] Inherited Invariants (I-GT-* inheriting I-VIEW-* / I-HX-* / GRAPH-* / FED-*)

These are binding inputs to DESIGN; they are NOT relitigated here.

| ID | Inherits | Carries into slice-10 as |
|---|---|---|
| I-GT-1 | I-VIEW-1/2/3 (slice-06) / KPI-VIEW-2 | Read-only preserved: traversal is a READ; the viewer signs/writes/persists/follows nothing, holds no signing key. The two new survey reads add no mutation method to `StoreReadPort`. |
| I-GT-2 | KPI-5 / KPI-GRAPH-6 (slice-04) / KPI-VIEW-5 (slice-06) | Local-first / offline: `/project` and `/philosophy` read the LOCAL store only; both render fully with the network disabled; neither route has a network seam (distinct from `/search` and `/scrape`). |
| I-GT-3 | KPI-GRAPH-2 (slice-04) / KPI-FED-1/2 (slice-03) / I-FED-1 | Anti-merging in surveys: every survey decomposes to per-author, per-cid rows; identical-content-different-author = two rows; zero faceless consensus rows. `author_did` + `cid` non-Option on every edge (load-bearing). |
| I-GT-4 | slice-04 traversal contract (jobs.yaml J-002b rationale) | No invented edges: every displayed edge maps to exactly one signed claim (its cid). An empty survey renders "no claims," never a fabricated edge. |
| I-GT-5 | KPI-4 / FR-VIEW-8 (slice-06) / WD-10 (slice-01) | Verbatim confidence (`0.90`) + display-only bucket; no viewer recompute of weight (J-002c stays at `/score`); confidence never reads as "the system thinks this is true." |
| I-GT-6 | I-HX-1..5 / KPI-HX-G1 (slice-07) | Progressive enhancement: full page without `HX-Request`, fragment of the same results region with it; page = chrome + fragment; the two shapes agree by construction (the full page embeds the fragment fn). A cross-link works as a full navigation without JS. |
| I-GT-7 | I-HX-2 / KPI-HX-G2 (slice-07) | Offline / no-CDN: htmx is the vendored, SHA-256-pinned local asset at `/static/htmx.min.js`; zero off-host references. (Both new routes need NO network at all — even stronger than `/search`.) |
| I-GT-8 | I-VIEW-4 (slice-06) / KPI-HX-G3 | Loopback-only bind unchanged (127.0.0.1); zero new persisted types (surveys computed per query). |

---

## Wave: DISCUSS / [REF] Story Map and Slicing

One journey: **traverse-the-graph-in-the-browser** — the arc: open a flat list →
click an entity to land on its survey page (Orienting) → spot a contributor who
spans two projects, or a project that embodies a philosophy I care about → click
that edge to traverse to the next entity (Connecting, the aha) → arrive at a
defensible understanding I can cite. Visual journey + shared-artifacts registry
under `discuss/`.

Emotional arc: **flat-list-curiosity → orienting → the-aha (Connecting) →
defensibly-connected** — entry curious-but-skeptical (Maria has flat lists of
claims but cannot *see the connections*; anxious the local graph is too sparse to
be useful), through orienting (a project/philosophy page surveys every attributed
claim about an entity — she sees who claims what), to the Connecting peak (she
clicks a contributor edge and discovers that *Rachel* embodies dependency-pinning
on BOTH cargo and nixpkgs — the non-obvious span), to defensibly-connected (she
can cite the exact signed claims, each attributed, each with verbatim confidence).

### Backbone

| Activity 1: Land on an entity | Activity 2: Survey its edges | Activity 3: Traverse to the next entity | Activity 4: Ground the finding |
|---|---|---|---|
| Click a project from a claim row → `/project` (US-GT-002) | See every philosophy it embodies + every contributor who claimed it, attributed, no merge (US-GT-002) | Click a philosophy edge → `/philosophy` (US-GT-003); click a contributor edge → `/score` (slice-09, reused) | Each edge shows author DID + cid + verbatim confidence; cross-links wire all surfaces (US-GT-004) |
| Click a philosophy from a claim row → `/philosophy` (US-GT-003) | See every project that embodies it + every contributor, attributed (US-GT-003) | Click a project edge → `/project`; click a contributor edge → `/score` | Sparse survey renders "no claims" honestly (US-GT-002/003) |

### Walking Skeleton (thinnest end-to-end thread)

US-GT-001 (the infra read capability) + **US-GT-002** (the project page: land on a
project, see its philosophies + contributors as attributed edges, each a link).
That alone is a complete traverse-one-hop thread. US-GT-003 (philosophy page) is
the symmetric second surface; US-GT-004 (cross-links) is the connective tissue
that closes the loop into a navigable journey.

### Release 1 (this slice — the whole thing; it is ≤ 1 day)

US-GT-001, US-GT-002, US-GT-003, US-GT-004. Sliced as ONE release because the
four stories are one coherent surface and splitting them would cost more
integration scaffolding than it saves (see Scope Assessment).

### Priority Rationale

1. **US-GT-001** (`@infrastructure`) — the project-survey + philosophy-survey
   read capability. P1: everything else needs it. Thinnest enabling read.
2. **US-GT-002** (project page) — P1: the walking-skeleton thread; the
   highest-traffic entry node (Maria starts from a project she is evaluating).
   Targets KPI-GRAPH-1 (non-obvious connection surfaced).
3. **US-GT-003** (philosophy page) — P1: symmetric surface; the orienting entry
   for "who embodies the value I care about." Targets KPI-GRAPH-1.
4. **US-GT-004** (cross-links) — P1: without it the pages are reachable only by
   hand-typed URLs; the cross-links are what make traversal a *journey*. Targets
   KPI-GRAPH-1 + KPI-GRAPH-5 (referenced justification).

All four are Must-Have for the slice to deliver J-002b in the browser; there is no
Should/Could tail in a ≤1-day slice.

---

## Wave: DISCUSS / [REF] Route Table (fits the existing viewer)

| Route | Method | Slice | Local/Network | New this slice? |
|---|---|---|---|---|
| `/` | GET | 06 | local | no |
| `/static/htmx.min.js` | GET | 07 | local | no |
| `/claims`, `/claims/{cid}` | GET | 06/07 | local | no (rows become clickable — US-GT-004) |
| `/peer-claims` | GET | 06/07 | local | no (rows become clickable — US-GT-004) |
| `/score?contributor=<did>` | GET | 09 | local | no (the contributor traversal TARGET — reused) |
| `/search` | GET | 08 | network | no (rows cross-link INTO local `/project`/`/philosophy` — US-GT-004) |
| `/scrape` | GET/POST | 06/07 | network | no |
| **`/project?subject=<uri>`** | **GET** | **10** | **local** | **YES (US-GT-002)** |
| **`/philosophy?object=<uri>`** | **GET** | **10** | **local** | **YES (US-GT-003)** |

Both new routes follow the slice-09 `/score` shape exactly: parse a single query
param, read the LOCAL store via a new read method, project into a pure
`viewer-domain` view-model ADT, render — forking by `Shape` (fragment vs full
page). A bare `/project` / `/philosophy` with no param renders an empty Form-style
guidance state; an unknown / claim-less entity renders the guided "no claims"
state (never a crash, never a fabricated edge).

---

## Wave: DISCUSS / [REF] System Constraints (cross-cutting)

- **Read-only, three-layer enforced** (StoreReadPort no-mutation type + xtask
  viewer capability rule + behavioral gold). No write/sign/follow route added.
- **No new crate; workspace stays 21 members.** Extend viewer-domain (pure) +
  adapter-http-viewer (effect) + adapter-duckdb (read impl) + ports + cli + xtask.
- **Functional paradigm (ADR-007):** pure survey-render + view-model ADTs in
  viewer-domain; the effect shell does read → (no decide step beyond grouping) →
  render. Grouping/anti-merging happens in Rust (pure), NEVER in SQL (the
  slice-03/04/05 anti-merging discipline — a survey read is `UNION ALL` claims ∪
  peer_claims with NO merge JOIN; the per-author grouping is a pure transform).
- **Loopback-only (127.0.0.1); LOCAL store only; offline.**
- **Verbatim confidence; no weight recompute** (single `render_confidence` site
  reused).

---

## Wave: DISCUSS / [REF] User Stories

> Story IDs `US-GT-00N`. Every story has a `job_id`. Every non-`@infrastructure`
> story has an Elevator Pitch (Before / After / Decision-enabled). AC name the
> driving port — the HTTP route — port-to-port. Domain examples use real data
> (Maria, Rachel, cargo, nixpkgs, dependency-pinning), never `user123`.

---

### US-GT-001: Local project-survey and philosophy-survey read capability

- **job_id:** `infrastructure-only`
- **infrastructure_rationale:** This story adds the two LOCAL read methods
  (`query_project_survey(subject)`, `query_philosophy_survey(object)`) to
  `StoreReadPort` + the adapter-duckdb impl that US-GT-002/003 render. It produces
  no user-visible output on its own (no route renders from it in isolation); it is
  the enabling read capability. The slice still contains ≥ 1 non-infra story
  (US-GT-002, US-GT-003, US-GT-004), so the slice has release value.
- **`@infrastructure`** (no Elevator Pitch required per Dimension 0 slice-level rule).

#### Problem

The viewer's `StoreReadPort` can list claims (by page), get one claim by cid,
list peer claims, and read a contributor's scoring feed (slice-09). It CANNOT
read "every attributed claim about a given subject (project)" or "every attributed
claim about a given object (philosophy)" — the two survey reads the project and
philosophy pages need. Without them, traversal has no data source.

#### Solution

Add two read-only methods to `StoreReadPort` and implement them in
`adapter-duckdb` over the shared read handle:

- `query_project_survey(subject: &Subject) -> Result<SurveyFeed, StoreReadError>`
  — every claim (own ∪ peer, `UNION ALL`, no merge) whose `subject == subject`.
- `query_philosophy_survey(object: &Object) -> Result<SurveyFeed, StoreReadError>`
  — every claim (own ∪ peer, `UNION ALL`, no merge) whose `object == object`.

Each `SurveyFeed` row carries `author_did` (non-Option), `cid`, `subject`,
`predicate`, `object`, `confidence` (numeric, verbatim), `composed_at`, and the
peer/own origin — exactly the fields the pure grouping + render need. The
grouping into "philosophies embodied" + "contributors who claimed" happens in the
PURE `viewer-domain` core (Rust), never in SQL (anti-merging discipline).

#### Acceptance Criteria

- [ ] `StoreReadPort` exposes `query_project_survey` and `query_philosophy_survey`,
      both returning a read-only `SurveyFeed`; NEITHER adds any mutation/sign/write
      method to the port (xtask viewer capability rule + the StoreReadPort
      no-mutation type both stay green).
- [ ] The adapter-duckdb impl reads `claims UNION ALL peer_claims` filtered by the
      survey key, with NO merge/average JOIN (the `xtask check-arch`
      no-author-eliding SQL rule stays green; aggregation is in Rust).
- [ ] Each returned row carries a non-Option `author_did` and `cid`.
- [ ] A subject/object with zero matching claims returns an empty `SurveyFeed`
      (Ok, not Err) — the render layer turns this into the guided "no claims" state.

#### Technical Notes

- Mirrors the slice-09 `query_contributor_scoring_feed` read shape
  (`UNION ALL`, read-only, no merge). DESIGN owns the exact SQL + whether a single
  parametrized read backs both surveys.
- DESIGN owns the storage shape (recursive query vs flat survey) — the product
  contract is "every matching attributed claim, no merge, no invented edge."

---

### US-GT-002: Land on a project and see the philosophies it embodies + who claimed them

- **job_id:** `J-002b` (entry node also realizes the subject dimension of J-002a)

#### Elevator Pitch

- **Before:** Maria is evaluating `github:rust-lang/cargo` for her team. To see
  *what philosophies it embodies and who backs them*, she scrolls a flat
  `/claims` list and mentally filters by subject — or drops to the CLI
  `graph query --subject`.
- **After:** Maria clicks `github:rust-lang/cargo` on any claim row and lands on
  `GET /project?subject=github:rust-lang/cargo` — a page that surveys every
  philosophy the project embodies (with verbatim confidence + bucket) and every
  contributor who claimed it, each one an attributed, clickable edge.
- **Decision enabled:** "Does cargo embody the values I build by, and who else
  backs that?" — she sees `dependency-pinning (0.90, triangulated)` claimed by
  `did:plc:rachel-test` and her own DID, and decides cargo is aligned — citing the
  exact signed claims.

#### Problem

Maria (P-001, graph-explorer hat) is choosing a dependency for a side project and
treats philosophical alignment as a first-class engineering concern. She has the
claims in her local store but can only see them as a flat list. She cannot, in the
browser, ask "for THIS project, what does it embody and who says so?" — the
orienting question that starts every traversal.

#### Who

- P-001 Senior Engineer Solo Builder | evaluating a project in her loopback
  `openlore ui` | motivated to make a defensible, attributed tech choice.

#### Solution

A read-only `GET /project?subject=<uri>` route. It reads the project survey from
the LOCAL store (US-GT-001), groups the attributed claims in the pure core into
(a) **philosophies embodied** — each `object` with its claiming rows, each row
showing `author_did` + verbatim `confidence` + display-only bucket + `cid` — and
(b) **contributors who claimed** — each distinct `author_did` who claimed
anything about this project, as a link to `/score?contributor=<did>`. No merge: two
authors claiming the same philosophy render as two rows. Forks by `Shape`
(fragment vs full page).

#### Domain Examples

##### 1: Happy path — cargo embodies dependency-pinning, two authors
Maria's local store has her own claim and Rachel's pulled peer claim that
`github:rust-lang/cargo` `embodiesPhilosophy` `org.openlore.philosophy.dependency-pinning`
(conf 0.90 and 0.88). She opens `GET /project?subject=github:rust-lang/cargo`. The
page lists `dependency-pinning` under "Philosophies embodied," with TWO attributed
rows — `did:plc:maria-test` (0.90, triangulated) and `did:plc:rachel-test` (0.88,
well-evidenced) — each with its cid, never averaged. Under "Contributors," both
DIDs appear as links to `/score`.

##### 2: Edge case — a project with one sparse claim
`github:smol-rs/smol` has exactly one own claim: `embodiesPhilosophy`
`org.openlore.philosophy.memory-safety` (conf 0.25, speculative). Maria opens
`GET /project?subject=github:smol-rs/smol`. The page shows the single philosophy
row with `(0.25, speculative)` and one contributor (herself). The page does not
manufacture confidence or invent a second edge.

##### 3: Error/boundary — a project with no claims
Maria hand-types `GET /project?subject=github:nonexistent/repo`. The survey read
returns empty. The page renders the guided "No claims about this project in your
local graph" state — naming the queried subject, suggesting `openlore graph query`
or `openlore scrape` in the CLI — never a crash, never a fabricated edge, exit 200.

#### UAT Scenarios (BDD)

##### Scenario: A project page surveys every philosophy it embodies, attributed, no merge
```gherkin
Given Maria's local store has her own claim (conf 0.90) and Rachel's pulled peer
  claim (conf 0.88) that github:rust-lang/cargo embodies org.openlore.philosophy.dependency-pinning
When Maria requests `GET /project?subject=github:rust-lang/cargo` on her openlore ui viewer
Then the response groups claims under "Philosophies embodied"
And dependency-pinning shows TWO attributed rows, one per author DID, never averaged
And each row shows its numeric confidence verbatim (0.90, 0.88) with the display-only bucket
And each row names its claim cid
```

##### Scenario: A project page lists contributors as traversal links to their score
```gherkin
Given github:rust-lang/cargo has claims by did:plc:maria-test and did:plc:rachel-test in the local store
When Maria requests `GET /project?subject=github:rust-lang/cargo`
Then the response lists both DIDs under "Contributors who claimed"
And each contributor is a link to `/score?contributor=<did>`
And no contributor row merges the two authors into a single aggregate
```

##### Scenario: A claim-less project renders the guided no-claims state, not a crash
```gherkin
Given there are zero claims about github:nonexistent/repo in the local store
When Maria requests `GET /project?subject=github:nonexistent/repo`
Then the response is 200 and names the queried subject
And it states there are no claims about this project in the local graph
And it suggests a CLI next step (graph query / scrape)
And it fabricates no edge
```

##### Scenario: The project page renders fully with the network disabled
```gherkin
Given the network is disabled
And github:rust-lang/cargo has claims in the local store
When Maria requests `GET /project?subject=github:rust-lang/cargo`
Then the response renders the full survey from the local store
And no network call is made (distinct from /search and /scrape)
```

##### Scenario: htmx request returns the results fragment; no-JS returns the full page
```gherkin
Given github:rust-lang/cargo has claims in the local store
When Maria requests `GET /project?subject=github:rust-lang/cargo` WITH an HX-Request header
Then the response is the project-survey results fragment only (no chrome)
When Maria requests the same route WITHOUT an HX-Request header
Then the response is the complete full page embedding that same fragment
```

#### Acceptance Criteria

- [ ] `GET /project?subject=<uri>` groups attributed claims under "Philosophies
      embodied," one row per (object, author) pair — never merged.
- [ ] Each philosophy row shows `author_did`, verbatim numeric confidence, the
      display-only bucket, and the `cid`.
- [ ] Each distinct contributor is listed as a link to `/score?contributor=<did>`.
- [ ] A claim-less subject renders the guided no-claims state (200, names the
      subject, no fabricated edge).
- [ ] The route renders fully network-disabled (local store only, no network call).
- [ ] `HX-Request` returns the results fragment; absence returns the full page
      embedding the same fragment (parity).

#### Outcome KPIs

- **Who**: P-001 operators exploring a project in the browser viewer.
- **Does what**: surface ≥ 1 attributed philosophy-edge for a project they are
  evaluating without dropping to the CLI.
- **By how much**: contributes to KPI-GRAPH-1 (≥ 60% of explorer sessions surface
  a non-obvious connection within 30 days), now reachable from the browser.
- **Measured by**: per-feature GREEN via the UAT scenarios above; cohort via the
  KPI-GRAPH-1 day-30 think-aloud (DEVOPS telemetry endpoint, YELLOW).
- **Baseline**: 0 — no browser project-survey surface existed before slice-10.

#### Technical Notes

- Reuses the slice-09 `/score` route as the contributor traversal target (built,
  not rebuilt).
- Confidence + bucket reuse the single `render_confidence` / bucket site
  (FR-VIEW-8 / WD-10) — no new formatting path, no recompute.
- DESIGN owns whether the contributor link carries the bare DID or the
  app-identity DID the `/score` resolver expects (mirror the slice-08 resolver).

---

### US-GT-003: Land on a philosophy and see the projects that embody it + who claimed them

- **job_id:** `J-002b` (entry node also realizes the object dimension of J-002a)

#### Elevator Pitch

- **Before:** Maria cares about `reproducible-builds` but cannot, in the browser,
  ask "which projects in my graph embody this, and who backs each?" — she scrolls
  a flat `/claims` list or drops to `graph query --object`.
- **After:** Maria clicks the `reproducible-builds` philosophy on any claim row and
  lands on `GET /philosophy?object=org.openlore.philosophy.reproducible-builds` — a
  page that surveys every project that embodies it and every contributor who
  claimed it, each an attributed, clickable edge.
- **Decision enabled:** "Which of the projects I'm weighing actually embody the
  value I care about, and is it well-backed or speculative?" — she sees `nixpkgs`
  and `bazel` both claimed for reproducible-builds, spots `did:plc:rachel-test`
  backing both, and traverses to Rachel's score.

#### Problem

The object (philosophy) dimension is the most natural *orienting* entry for a
value-driven choice ("I care about X — who embodies it?"). The viewer has no
browser surface for it; the operator can only see philosophies scattered across a
flat claim list.

#### Who

- P-001 Senior Engineer Solo Builder | orienting a decision around a philosophy in
  her loopback `openlore ui` | motivated to find aligned projects + the people
  behind them.

#### Solution

A read-only `GET /philosophy?object=<uri>` route. It reads the philosophy survey
from the LOCAL store (US-GT-001), groups the attributed claims in the pure core
into (a) **projects that embody it** — each `subject` with its claiming rows,
attributed, with verbatim confidence + bucket + cid; each project a link to
`/project?subject=<uri>` — and (b) **contributors who claimed it** — each distinct
`author_did` as a link to `/score?contributor=<did>`. No merge. Forks by `Shape`.

#### Domain Examples

##### 1: Happy path — reproducible-builds embodied by two projects, a shared contributor
Maria's local store has Rachel's peer claims that BOTH `github:NixOS/nixpkgs` and
`github:bazelbuild/bazel` `embodiesPhilosophy`
`org.openlore.philosophy.reproducible-builds` (conf 0.92, 0.85). She opens
`GET /philosophy?object=org.openlore.philosophy.reproducible-builds`. The page
lists both projects under "Projects that embody this," each attributed to Rachel
with verbatim confidence; under "Contributors," `did:plc:rachel-test` appears once
as a link to `/score`. Maria notices Rachel spans both — the non-obvious
connection.

##### 2: Edge case — same philosophy, two authors, same project, no merge
`github:NixOS/nixpkgs` is claimed for `reproducible-builds` by BOTH
`did:plc:maria-test` (0.92) and `did:plc:tobias-test` (0.70). The philosophy page
shows nixpkgs with TWO attributed rows — one per author — never averaged into one
"nixpkgs: 0.81" row.

##### 3: Error/boundary — a philosophy with no claims
Maria hand-types `GET /philosophy?object=org.openlore.philosophy.actor-model` and
her local graph has zero claims for it. The page renders the guided "No claims for
this philosophy in your local graph" state — naming the queried object, suggesting
a CLI next step — exit 200, no fabricated edge.

#### UAT Scenarios (BDD)

##### Scenario: A philosophy page surveys every project that embodies it, attributed, no merge
```gherkin
Given Rachel's pulled peer claims assert github:NixOS/nixpkgs (conf 0.92) and
  github:bazelbuild/bazel (conf 0.85) embody org.openlore.philosophy.reproducible-builds
When Maria requests `GET /philosophy?object=org.openlore.philosophy.reproducible-builds`
Then the response lists both projects under "Projects that embody this"
And each project shows its attributed author DID, verbatim confidence, bucket, and cid
And each project is a link to `/project?subject=<uri>`
```

##### Scenario: Two authors claiming one project for a philosophy render as two rows
```gherkin
Given github:NixOS/nixpkgs is claimed for reproducible-builds by did:plc:maria-test (0.92)
  and did:plc:tobias-test (0.70) in the local store
When Maria requests `GET /philosophy?object=org.openlore.philosophy.reproducible-builds`
Then nixpkgs shows two attributed rows, one per author DID
And no row averages the two confidences into a single nixpkgs entry
```

##### Scenario: A shared contributor across projects is a single traversal link
```gherkin
Given did:plc:rachel-test claims both nixpkgs and bazel for reproducible-builds
When Maria requests `GET /philosophy?object=org.openlore.philosophy.reproducible-builds`
Then did:plc:rachel-test appears once under "Contributors who claimed it"
And it is a link to `/score?contributor=did:plc:rachel-test`
```

##### Scenario: A claim-less philosophy renders the guided no-claims state
```gherkin
Given there are zero claims for org.openlore.philosophy.actor-model in the local store
When Maria requests `GET /philosophy?object=org.openlore.philosophy.actor-model`
Then the response is 200 and names the queried object
And it states there are no claims for this philosophy in the local graph
And it fabricates no edge
```

##### Scenario: The philosophy page renders fully with the network disabled
```gherkin
Given the network is disabled
And org.openlore.philosophy.reproducible-builds has claims in the local store
When Maria requests `GET /philosophy?object=org.openlore.philosophy.reproducible-builds`
Then the response renders the full survey from the local store
And no network call is made
```

#### Acceptance Criteria

- [ ] `GET /philosophy?object=<uri>` groups attributed claims under "Projects that
      embody this," one row per (subject, author) pair — never merged.
- [ ] Each project row shows `author_did`, verbatim confidence, bucket, and `cid`,
      and is a link to `/project?subject=<uri>`.
- [ ] Each distinct contributor is a link to `/score?contributor=<did>`; a
      contributor spanning multiple projects appears once.
- [ ] A claim-less object renders the guided no-claims state (200, names the
      object, no fabricated edge).
- [ ] The route renders fully network-disabled.
- [ ] `HX-Request` returns the results fragment; absence returns the full page
      embedding the same fragment.

#### Outcome KPIs

- **Who**: P-001 operators orienting a decision around a philosophy in the browser.
- **Does what**: surface ≥ 1 attributed project-edge (and the contributors behind
  it) for a philosophy they care about, without the CLI.
- **By how much**: contributes to KPI-GRAPH-1 (non-obvious connection) reachable
  from the browser; the cross-project shared-contributor span is the canonical
  "aha."
- **Measured by**: per-feature GREEN via the UAT scenarios; cohort via KPI-GRAPH-1
  day-30 study (DEVOPS, YELLOW).
- **Baseline**: 0 — no browser philosophy-survey surface existed before slice-10.

#### Technical Notes

- Symmetric to US-GT-002 (same read pattern, same render pattern, mirrored key).
- Reuses the slice-09 `/score` route as the contributor traversal target.

---

### US-GT-004: Make every entity clickable so traversal is one journey

- **job_id:** `J-002b`

#### Elevator Pitch

- **Before:** The project and philosophy pages exist, but Maria can only reach
  them by hand-typing URLs — the claim rows on `/claims`, `/peer-claims`,
  `/score`, and `/search` are inert text. There is no *traversal*; there are two
  islands.
- **After:** Every subject, object, and contributor on every existing surface
  becomes a clickable edge: a project link to `/project`, a philosophy link to
  `/philosophy`, a contributor link to `/score`. Maria clicks her way from a claim
  to a project to a philosophy to a contributor — following the graph.
- **Decision enabled:** "Starting from this one claim, who else in my graph spans
  the values I care about?" — the Connecting step; she traverses the next ring of
  related entities by clicking, with the browser's back/forward as her traversal
  stack.

#### Problem

Without cross-links, US-GT-002/003 are reachable only by typed URL, and the
existing flat lists stay flat. Traversal — the J-002b "follow the edge to the next
entity" — is precisely the *clicking*. The connective tissue is what turns two
survey pages + four list pages into one navigable graph.

#### Who

- P-001 Senior Engineer Solo Builder | already on any viewer surface | motivated
  to follow the non-obvious connection without re-typing queries.

#### Solution

Make the subject, object, and contributor cells on the existing renderers
(`/claims` rows, `/claims/{cid}` detail, `/peer-claims` rows, `/score` breakdown
rows, `/search` result rows) render as links: subject → `/project?subject=<uri>`,
object → `/philosophy?object=<uri>`, contributor (author DID) →
`/score?contributor=<did>`. Plain `<a href>` so a no-JS click is a full
navigation; where the target is an in-page panel the link MAY carry `hx-get` for
an in-place swap (progressive enhancement, never required). This is render-only
wiring in the pure `viewer-domain` core — no new route, no new data, no write
surface. Reuses verbatim confidence formatting; adds no recompute.

#### Domain Examples

##### 1: Happy path — traverse claim → project → philosophy → contributor
On `/claims`, Maria sees a row: `github:rust-lang/cargo` · `dependency-pinning` ·
`did:plc:rachel-test` · 0.88. She clicks `github:rust-lang/cargo` → lands on
`/project?subject=github:rust-lang/cargo`. There she clicks
`reproducible-builds` → lands on `/philosophy?object=…`. There she clicks
`did:plc:rachel-test` → lands on `/score?contributor=did:plc:rachel-test`. Four
hops, all by clicking, browser-back unwinds the path.

##### 2: Edge case — a peer claim row on /peer-claims is equally clickable
On `/peer-claims`, Rachel's claim row's subject `github:NixOS/nixpkgs` is a link to
`/project?subject=github:NixOS/nixpkgs` — peer-origin rows traverse identically to
own rows, attribution preserved (the project page shows Rachel's DID).

##### 3: Boundary — a /search (network) row cross-links into LOCAL traversal
On `/search`, a network result for `github:denoland/deno` ·
`dependency-pinning` lets Maria click `dependency-pinning` to land on the LOCAL
`/philosophy?object=org.openlore.philosophy.dependency-pinning`. Traversal stays
local even when the entry point was a network search; no traversal route ever
reaches the network.

#### UAT Scenarios (BDD)

##### Scenario: A subject cell on /claims is a link to the project page
```gherkin
Given a claim row on /claims has subject github:rust-lang/cargo
When Maria views `GET /claims` on her openlore ui viewer
Then the subject cell renders as a link to `/project?subject=github:rust-lang/cargo`
And a no-JS click navigates to the full project page
```

##### Scenario: An object cell links to the philosophy page across all list surfaces
```gherkin
Given claim rows on /claims, /peer-claims, and /score show org.openlore.philosophy.dependency-pinning
When Maria views each of those surfaces
Then every object cell renders as a link to `/philosophy?object=org.openlore.philosophy.dependency-pinning`
```

##### Scenario: A contributor cell links to that contributor's score (reusing slice-09)
```gherkin
Given a claim row shows author did:plc:rachel-test
When Maria views the row on /claims or /peer-claims
Then the contributor cell renders as a link to `/score?contributor=did:plc:rachel-test`
```

##### Scenario: Cross-links add no write surface and stay read-only
```gherkin
Given the cross-links are wired on every list surface
When the route inventory and key-access audit run against the real openlore ui
Then no new write/sign/follow route exists
And no signing key is read in the viewer process
And the bind stays loopback-only
```

##### Scenario: A network /search row cross-links into local traversal without a network call
```gherkin
Given a /search network result shows object org.openlore.philosophy.dependency-pinning
When Maria clicks that object link
Then she lands on `/philosophy?object=org.openlore.philosophy.dependency-pinning`
And that page reads the LOCAL store only, with no network call
```

#### Acceptance Criteria

- [ ] Subject cells on `/claims`, `/claims/{cid}`, `/peer-claims`, `/score`,
      `/search` render as links to `/project?subject=<uri>`.
- [ ] Object cells on the same surfaces render as links to `/philosophy?object=<uri>`.
- [ ] Contributor (author DID) cells render as links to `/score?contributor=<did>`.
- [ ] No new write/sign/follow route is added; no key read; loopback-only bind
      (the slice-06 three-layer read-only gold tests stay green).
- [ ] A no-JS click is a full navigation to the target page (progressive
      enhancement; htmx swap optional, never required).
- [ ] Cross-linking from a network `/search` row lands on a LOCAL traversal page
      with no network call.

#### Outcome KPIs

- **Who**: P-001 operators on any viewer surface.
- **Does what**: traverse ≥ 2 hops (claim → entity → related entity) by clicking,
  without re-typing a query or dropping to the CLI.
- **By how much**: contributes to KPI-GRAPH-1 (non-obvious connection surfaced)
  and KPI-GRAPH-5 (a query result cited when justifying a real choice) on the
  browser surface.
- **Measured by**: per-feature GREEN via the UAT scenarios; cohort via the
  KPI-GRAPH-1/5 day-30 study (DEVOPS, YELLOW).
- **Baseline**: 0 — viewer rows were inert text before slice-10.

#### Technical Notes

- Render-only wiring in pure `viewer-domain`; no new route, no new read, no write.
- Confidence/bucket formatting unchanged (single site reused).
- DESIGN owns the exact href construction + whether contributor links carry the
  bare vs app-identity DID (mirror the slice-08 `/search` resolver).

---

## Wave: DISCUSS / [REF] Out of Scope (explicit)

- **NO authoring / sign / counter affordance** on any traversal surface (read-only,
  KPI-VIEW-2). Authoring stays in the CLI (J-001).
- **NO network on the traversal routes** (`/project`, `/philosophy` are LOCAL-only;
  distinct from `/search` → indexer and `/scrape` → GitHub). KPI-5 / KPI-GRAPH-6.
- **NO new weighting surface** (J-002c). Edge rows show verbatim confidence + the
  slice-04 display-only bucket; the full transparent weighted breakdown stays at
  `/score` (slice-09). The viewer recomputes no weight.
- **NO follow execution** from a contributor edge — the edge LINKS to `/score`;
  following a peer stays a deliberate CLI `peer add` action (J-003/J-005).
- **NO multi-hop auto-expanded tree** in this slice — a survey is depth-1 (the
  entity + its direct edges); each edge is a LINK the operator clicks to traverse
  the next hop. The slice-04 CLI `--depth K` tree render is deferred. Browser
  back/forward IS the traversal stack.
- **NO new crate; NO write port; NO key in the process.**

---

## Wave: DISCUSS / [REF] Outcome KPIs (feature-level)

slice-10 mints **NO new KPI ID** — it REALIZES the inherited KPI-GRAPH-* /
KPI-VIEW-* / KPI-HX-* / KPI-4 / KPI-5 on a new browser surface (per the
slice-08/09 precedent of not duplicating a per-feature `outcome-kpis.md` for a
DELTA that adds a surface, not an outcome). Per-story KPIs are inlined above.
Realization summary:

| KPI | Type | Realized on `/project`+`/philosophy`+cross-links as |
|---|---|---|
| KPI-GRAPH-1 (non-obvious connection surfaced) | north-star | Now reachable from the browser by traversal; the cross-project shared-contributor span is the canonical aha (US-GT-002/003/004). Cohort YELLOW pending day-30 study. |
| KPI-GRAPH-2 (anti-merging in aggregates) | guardrail | MET by construction — surveys decompose to per-author, per-cid rows; grouping in Rust, never SQL; identical-content-two-authors = two rows (WD-GT-5 / I-GT-3). Release-blocking. |
| KPI-GRAPH-5 (referenced justification) | leading | A traversal path is citable (attributed edges with cids); cohort via day-30 survey (YELLOW). |
| KPI-GRAPH-6 / KPI-5 (local-first) | guardrail | MET — both routes read LOCAL only, render network-down (WD-GT-4 / I-GT-2). Release-blocking. |
| KPI-VIEW-2 / KPI-HX-G3 (read-only / no new write surface) | guardrail | MET — no write/sign/follow route, no key, loopback-only (WD-GT-3 / I-GT-1; three-layer enforced). Release-blocking. |
| KPI-HX-G1 (no-JS no-regression) | guardrail | MET — both routes serve a full page without HX-Request; cross-links are plain `<a href>` working without JS (WD-GT-8 / I-GT-6). Release-blocking. |
| KPI-HX-G2 (offline / no-CDN chrome) | guardrail | MET — vendored htmx; both routes need NO network at all (WD-GT-8 / I-GT-7). Release-blocking. |
| KPI-4 (verbatim, no silent normalization) | guardrail | MET — verbatim confidence (`0.90`) reused single site; no weight recompute (WD-GT-7 / I-GT-5). Release-blocking. |

**Disprover / kill criterion** (inherited from KPI-GRAPH-1): if < 20% of browser
explorer sessions traverse to a non-obvious connection at day-30, re-investigate
the cross-link affordance discoverability before any deeper traversal investment.

---

## Wave: DISCUSS / [REF] WS / Progressive-Enhancement Strategy

- **WS (working-software) strategy:** Brownfield DELTA on a running viewer — no
  Feature-0 walking skeleton. The thinnest end-to-end thread is US-GT-001 (the
  read) + US-GT-002 (the project page), demonstrable as "click a project on
  `/claims` → land on its survey → click a contributor → land on `/score`."
  US-GT-003 + US-GT-004 complete the journey and ship in the same ≤1-day slice.
- **Progressive enhancement:** every new route serves a complete full page without
  `HX-Request` (no-JS / bookmark / direct URL) and a fragment of the same results
  region with it (the slice-07 `Shape` fork; page = chrome + fragment, parity by
  construction). Cross-links are plain `<a href>` — a no-JS click is a full
  navigation; an htmx swap is a nicety, never a requirement. htmx stays the
  vendored, SHA-256-pinned local asset; both traversal routes need no network at
  all (offline-stronger than `/search`).

---

## Wave: DISCUSS / Definition of Ready

| DoR Item | Status | Evidence |
|---|---|---|
| 1. Problem statement clear, domain language | PASS | Each story opens from Maria's pain (flat lists, cannot see connections) in domain language; no "implement X." |
| 2. User/persona with specific characteristics | PASS | P-001 (Maria, node operator, graph-explorer hat) with characteristics from the persona SSOT; P-002 boundary noted. |
| 3. 3+ domain examples with real data | PASS | Each value story has 3 examples (happy / edge / error) with real data (cargo, nixpkgs, bazel, dependency-pinning, reproducible-builds, did:plc:rachel-test/maria-test/tobias-test) — no generic data. |
| 4. UAT in Given/When/Then (3-7 scenarios) | PASS | US-GT-002: 5; US-GT-003: 5; US-GT-004: 5. All name the driving HTTP route (port-to-port). US-GT-001 is `@infrastructure` (4 AC, no UAT required). |
| 5. AC derived from UAT | PASS | Each story's AC checklist maps 1:1 to its scenarios. |
| 6. Right-sized (1-3 days, 3-7 scenarios) | PASS | 4 stories, ≤ 1 day, ≤ 5 scenarios each; Scope Assessment PASS (no new crate, 6 touchpoints). |
| 7. Technical notes: constraints/dependencies | PASS | Each story has Technical Notes; System Constraints section captures cross-cutting (read-only, no-crate, functional paradigm, local-only, verbatim). |
| 8. Dependencies resolved or tracked | PASS | Depends on slice-04 (traversal contract, scoring buckets), slice-06/07 (render pattern), slice-09 (`/score` target) — all SHIPPED. No open dependency. |
| 9. Outcome KPIs defined with measurable targets | PASS | Per-story KPIs (Who/Does-what/By-how-much/Measured-by/Baseline) + feature-level realization table of inherited KPIs with disprover. |
| JTBD traceability (Decision 1) | PASS | Every story has `job_id`: US-GT-001 `infrastructure-only` + rationale; US-GT-002/003/004 = `J-002b`. |
| Elevator Pitch (non-infra) | PASS | US-GT-002/003/004 each have Before/After/Decision-enabled referencing a real HTTP entry point + concrete output. |
| Dimension 0 slice-level | PASS | The slice contains ≥ 1 non-`@infrastructure` user-visible story (3 of them). |

### DoR Status: PASSED (pending peer review)

---

## Wave: DISCUSS / Risks

| Risk | Prob | Impact | Mitigation |
|---|---|---|---|
| The local graph is too sparse for traversal to surface a connection (the J-002 anxiety) | Med | Med | Honest "no claims" / sparse rendering (WD-GT-6); the survey renders sparse AS sparse, never fabricates an edge — treated as a lead, not a conclusion (inherits KPI-GRAPH-4 discipline). Not blocking. |
| Cross-link href ambiguity (bare DID vs app-identity DID for `/score`) | Low | Low | Flagged to DESIGN (mirror the slice-08 `/search` resolver); a known, bounded decision. |
| Subject/object URIs need percent-encoding in hrefs | Low | Low | Reuse the existing `percent_decode_form` + standard href encoding; bounded. DESIGN owns. |

---

## Wave: DISTILL

> Wave: **DISTILL** (lean mode) · Owner: Quinn (nw-acceptance-designer) · Date: 2026-06-06
> Per ADR-025 DISTILL is the CANONICAL author of ALL acceptance tests, scaffolded
> RED (each body `todo!(...)` → panics → classifies RED / MISSING_FUNCTIONALITY).
> DELIVER unskips them (it does not re-author ATs in RED). Brownfield DELTA: mirrors
> the slice-06/07/08/09 viewer acceptance corpus EXACTLY (same `ViewerServer`
> subprocess harness, `Shape` discriminators, production peer seeding path,
> `capture_store_row_count_universe` / `assert_store_read_only` gold). No new test
> framework.

### [REF] Reconciliation result

**Reconciliation passed — 0 contradictions.** Read DISCUSS `discuss/wave-decisions.md`
(WD-GT-1..10) against the DESIGN artifacts (`architecture-design.md` §7 resolved
questions + `component-boundaries.md` + `data-models.md` + ADR-042/043/044/045).
Every DISCUSS lock is carried verbatim by DESIGN: two routes + contributor→/score
(WD-GT-1 ↔ ADR-044), read-only 3-layer (WD-GT-3 ↔ I-GT-1), local-only/offline
(WD-GT-4 ↔ I-GT-2), anti-merging (WD-GT-5 ↔ I-GT-3 + SurveyRow non-Option
author_did), no-invented-edges (WD-GT-6 ↔ I-GT-4 + non-Option cid), verbatim+bucket
no-recompute (WD-GT-7 ↔ I-GT-5 + claim_domain::confidence_bucket reuse), Shape parity
(WD-GT-8 ↔ I-GT-6), depth-1 (WD-GT-10 ↔ ADR-043). The three DISCUSS open questions
(Q1 bare-DID, Q2 percent-encode, Q3 two methods) are RESOLVED by DESIGN §7 — these
are resolutions, not contradictions. No DEVOPS dir for this slice (graceful
degradation: WARN; the `openlore ui` driving port is already in the project
Infrastructure Policy, so no env-matrix gap). No DESIGN `wave-decisions.md` file
(decisions live in the four ADRs, all read).

### [REF] Lang + Infrastructure Policy + Port bootstrap

- `[lang-mode] rust` (workspace `Cargo.toml`).
- `[policy-mode] inherit` — `docs/architecture/atdd-infrastructure-policy.md` PRESENT;
  the `openlore` CLI `ui` verb driving port (`GET /` `/claims` `/peer-claims`
  `/scrape` `/search` `/score` `/static/htmx.min.js`) is already recorded. slice-10
  adds `/project` + `/philosophy` to that SAME long-running-subprocess driving port —
  no new port class, no new mechanism, no policy row appended (the StoreReadPort
  driven-internal real-DuckDB mechanism is unchanged; the two new survey reads are
  methods on the already-recorded read-only port).
- `[port-mode] inherit` — state-delta port present at `tests/common/state_delta.rs`
  (re-exported as `support::state_delta`; reused by the read-only gold).

### [REF] Scenario list with tags

Naming `GT-N` (mirrors slice-09 `C-N`). Driving port (port-to-port): the two HTTP
routes `GET /project?subject=<uri>` + `GET /philosophy?object=<uri>` over the REAL
`openlore ui` subprocess. Layer-3/5 subprocess + real-I/O, EXAMPLE-only (Mandate
9/11 — sad paths enumerated, never PBT-generated at this layer).

**`tests/acceptance/viewer_graph_traversal.rs`** (Tier A story scenarios — 14):

| Scenario | US | Invariant(s) | Tags |
|---|---|---|---|
| `open_a_project_survey_with_htmx_returns_only_the_traversal_fragment` **(WS)** | US-GT-002 | I-GT-3/5/6 | `@walking_skeleton @driving_port @real-io @htmx-fragment @happy` |
| `the_project_survey_full_page_and_fragment_render_the_same_region` | US-GT-002 | I-GT-6 | `@no-js @full-page @parity @happy` |
| `a_project_survey_renders_two_authors_on_one_philosophy_as_two_rows` | US-GT-002 | I-GT-3/4/5 | `@anti-merging @kpi-graph-2 @boundary` |
| `a_project_survey_lists_contributors_as_links_to_their_score` | US-GT-002 | I-GT-3 | `@crosslink @kpi-graph-1 @happy` |
| `a_claim_less_project_renders_the_guided_no_claims_state_not_a_crash` | US-GT-002 | I-GT-4 | `@no-claims @empty-state @error` |
| `the_project_survey_renders_fully_with_the_network_disabled` | US-GT-002 | I-GT-2 | `@offline @local-first @kpi-5 @happy` |
| `open_a_philosophy_survey_with_htmx_returns_only_the_traversal_fragment` **(WS)** | US-GT-003 | I-GT-3/5/6 | `@walking_skeleton @driving_port @real-io @htmx-fragment @happy` |
| `the_philosophy_survey_full_page_and_fragment_render_the_same_region` | US-GT-003 | I-GT-6 | `@no-js @full-page @parity @happy` |
| `a_philosophy_survey_renders_two_authors_on_one_project_as_two_rows` | US-GT-003 | I-GT-3/4 | `@anti-merging @crosslink @kpi-graph-2 @boundary` |
| `a_shared_contributor_across_projects_is_a_single_traversal_link` | US-GT-003 | I-GT-3 | `@crosslink @kpi-graph-1 @happy` |
| `a_claim_less_philosophy_renders_the_guided_no_claims_state` | US-GT-003 | I-GT-4 | `@no-claims @empty-state @error` |
| `survey_cells_render_as_traversal_links_to_the_next_entity` | US-GT-004 | I-GT-6 | `@crosslink @kpi-graph-1 @kpi-graph-5 @happy` |
| `traversal_cross_links_are_plain_anchors_navigable_without_js` | US-GT-004 | I-GT-6 | `@crosslink @no-js @happy` |
| `a_claim_controlled_uri_is_percent_encoded_and_cannot_inject_the_href` | US-GT-004 | ADR-044 §security | `@security @injection-boundary @adr-044 @error` |

**`tests/acceptance/viewer_graph_traversal_invariants.rs`** (GOLD guardrails — 5):

| Scenario | Invariant | Tags |
|---|---|---|
| `every_traversal_route_leaves_the_store_read_only` | I-GT-1 / WD-GT-3 (read-only, Mandate 8 state-delta) | `@property @read-only @i-gt-1 @gold` |
| `no_traversal_response_adds_a_write_or_sign_control` | I-GT-1 / WD-GT-3 (no write/sign; cross-links render-only) | `@property @read-only @i-gt-1 @gold` |
| `the_traversal_page_chrome_stays_offline_no_cdn` | I-GT-7 / KPI-HX-G2 (only local htmx asset) | `@property @offline @no-cdn @i-gt-7 @gold` |
| `the_traversal_surface_works_fully_offline` | I-GT-2 / KPI-5 (LOCAL read, no outbound edge) | `@property @offline @local-first @i-gt-2 @gold` |
| `no_traversal_href_lets_a_claim_controlled_uri_inject` | ADR-044 §security (injection boundary, both shapes) | `@property @security @injection-boundary @i-gt-3 @i-gt-4 @gold` |

**Error/edge ratio**: 8 of 19 scenarios are error/edge/security/offline boundary
(GT-5, GT-11 no-claims; GT-3, GT-9 anti-merging boundary; GT-14 + GT-INV-Security
injection; GT-6 + GT-INV-Offline offline) = **42%** (≥ 40% target met). Two walking
skeletons (one per new route — the symmetric project + philosophy threads).

### [REF] WS strategy

Per the Architecture of Reference (driving = real adapter) + the project
Infrastructure Policy: the walking skeletons drive the REAL `openlore ui` subprocess
(`ViewerServer::start`) over a REAL DuckDB store seeded through the PRODUCTION peer
path (`seed_*_survey_trail` → `peer add` + `peer pull`), asserting on rendered HTML
(Pillar 3). Two WS — `open_a_project_survey_with_htmx_returns_only_the_traversal_fragment`
(US-GT-002) and `open_a_philosophy_survey_with_htmx_returns_only_the_traversal_fragment`
(US-GT-003) — each the thinnest end-to-end thread for its route (viewer → LOCAL
survey read → pure group → HTML fragment). Tagged `@walking_skeleton @driving_port
@real-io`. No Tier B state-machine PBT: the journey is depth-1 (each edge is a
discrete LINK click = a fresh GET, NOT a chained in-process state machine — browser
back/forward IS the traversal stack, WD-GT-10), so the Mandate-10 "≥3 chained
scenarios + domain-rich input" trigger does not fire; Tier A example coverage
suffices.

### [REF] Adapter / driving-port coverage

| Driving port (route) | Scenario(s) | Real-I/O |
|---|---|---|
| `GET /project?subject=<uri>` | GT-1..6, GT-12, GT-INV-* | YES — REAL `openlore ui` subprocess + HTTP |
| `GET /philosophy?object=<uri>` | GT-7..11, GT-13/14, GT-INV-* | YES — REAL `openlore ui` subprocess + HTTP |

Driven port: `StoreReadPort` (read-only) — its two new survey reads
(`query_project_survey` / `query_philosophy_survey`) are exercised through the REAL
DuckDB via the driving routes (real-I/O; the store is seeded by the production
`claim add` / `peer add` / `peer pull` verbs, never hand-inserted). NO new driven
adapter, NO new external integration → NO contract-test annotation required (the
offline-STRONGER property: both routes have no outbound edge — I-GT-2 / I-GT-7).

### [REF] Scaffold file list (Mandate 7 — `// SCAFFOLD: true`)

- `tests/acceptance/viewer_graph_traversal.rs` — 14 story scenarios (`todo!()` bodies).
- `tests/acceptance/viewer_graph_traversal_invariants.rs` — 5 GOLD guardrails
  (`todo!()` bodies; the read-only gold reuses the inherited universe-bound
  `assert_store_read_only`).
- `tests/acceptance/support/mod.rs` — NEW slice-10 seams (compile; stubbed `todo!()`
  where they need DELIVER render/seed knowledge): consts
  `TRAVERSAL_PROJECT_*` / `TRAVERSAL_PHILOSOPHY_*` / `TRAVERSAL_AUTHOR_*` /
  `TRAVERSAL_RESULTS_ID` / `TRAVERSAL_INJECTION_SUBJECT(_ENCODED)` /
  `TRAVERSAL_PROJECT_{NIXPKGS,BAZEL}_ENCODED`; seeders `seed_project_survey_trail`,
  `seed_philosophy_survey_trail`, `seed_two_author_same_edge`,
  `seed_injection_uri_subject` (all drive the EXISTING production
  `seed_peer_authored_graph` / `seed_own_plus_peer_graph` peer path); asserts
  `assert_traversal_html_groups_attributed_and_verbatim`,
  `assert_traversal_html_names_cids`,
  `assert_traversal_html_contributors_link_to_score`,
  `assert_traversal_html_crosslink_is_plain_anchor`,
  `assert_traversal_href_percent_encoded`,
  `assert_traversal_html_renders_no_claims`,
  `assert_traversal_html_has_no_write_or_sign_control` (this last fully materialized,
  mirrors the slice-09 no-write scan).
- `crates/cli/Cargo.toml` — two new `[[test]]` targets registered so `cargo build -p
  cli --tests` compiles them.

**Build/RED confirmation**: `cargo build -p cli --tests` COMPILES both new binaries
(only pre-existing shared unused-import warnings). Spot-run confirms RED: each GT-*
scenario panics at a `todo!(...)` (MISSING_FUNCTIONALITY), never an ImportError /
collection error / BROKEN. The 19 slice-10 scenarios stay RED until DELIVER's
per-scenario RED→GREEN→COMMIT cycles.

### [REF] Test placement

`tests/acceptance/viewer_graph_traversal{,_invariants}.rs` — the EXACT directory +
`{feature}` + `{feature}_invariants` split the slice-06/07/08/09 viewer acceptance
files use (precedent: `viewer_contributor_scoring.rs` +
`viewer_contributor_scoring_invariants.rs`). Shared harness in
`tests/acceptance/support/mod.rs` (one harness for the whole acceptance corpus, the
established brownfield convention).

### [REF] Pre-requisites (DESIGN driving ports + DEVOPS env the scenarios depend on)

- DESIGN driving ports: `GET /project?subject` + `GET /philosophy?object` (ADR-044),
  forking by `Shape` after the synchronous store-read match (component-boundaries.md
  §adapter-http-viewer). The pure `TraversalView` group + render + `href_*` helpers
  (ADR-043 + data-models.md §2) are the DELIVER GREEN target.
- DEVOPS: none new — the `openlore ui` subprocess driving port + REAL DuckDB store
  are the inherited slice-06..09 environment (Infrastructure Policy `## Driving` +
  `## Driven internal (real)`). Build-before-run: `cargo build` the `openlore` bin.

### CM-A/B/C mandate-compliance evidence

- **CM-A (hexagonal boundary)**: every scenario enters through the REAL `openlore ui`
  HTTP routes (driving ports) via `ViewerServer::get`/`get_htmx`; ZERO scenario calls
  `viewer-domain` `render_*`/`group_*` or `StoreReadPort` impls directly. The only
  `use` is `support::*` (the subprocess harness).
- **CM-B (business language)**: scenario names + Gherkin doc-comments speak the domain
  (project / philosophy / contributor / claim / embodies / traverse / survey),
  never HTTP/SQL/DuckDB/maud. Technical detail lives inside the step bodies + support
  helpers only.
- **CM-C (complete user journeys)**: each scenario is a full traverse-one-hop journey
  with observable value (land on an entity → see its attributed edges → follow a
  cross-link), asserted on rendered HTML the operator's browser shows — not isolated
  technical operations.

---

## Changelog

- 2026-06-06 — Quinn — DISTILL wave for `viewer-graph-traversal` (slice-10):
  reconciliation PASSED (0 contradictions). Authored 19 RED acceptance scaffolds (14
  Tier-A story + 5 GOLD invariants) for the two new driving ports
  (`GET /project?subject`, `GET /philosophy?object`) mirroring the slice-09 viewer
  corpus: two walking skeletons (project + philosophy fragment threads), anti-merging
  two-authors-two-rows, verbatim confidence + display-only bucket + cid, contributor
  →/score (bare DID) + subject→/project + object→/philosophy cross-links, guided
  NoClaims, network-disabled render, and the ADR-044 §security injection boundary (a
  claim-controlled URI percent-encoded into the href). New support seams stubbed
  (`seed_*_survey_trail` / `seed_two_author_same_edge` / `seed_injection_uri_subject`
  + `assert_traversal_*`). `cargo build -p cli --tests` compiles; all 19 classify RED
  (`todo!()` panic). `[lang-mode] rust` / `[policy-mode] inherit` / `[port-mode]
  inherit`.
- 2026-06-06 — Luna — Initial DISCUSS delta for `viewer-graph-traversal` (slice-10):
  browser traversal for J-002b. Two LOCAL read-only routes (`/project`,
  `/philosophy`) + cross-link wiring on existing surfaces. No new job, no new
  sub-job, no new crate, no new KPI ID (realizes inherited KPI-GRAPH-*/VIEW-*/HX-*
  on a new surface). J-002a entry-node reused; J-002c link-out only (no new
  weighting surface). Scope PASS (4 stories, ≤1 day). DoR PASSED pending review.
