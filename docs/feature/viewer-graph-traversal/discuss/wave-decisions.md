# Wave Decisions: viewer-graph-traversal (slice-10) — DISCUSS

> Locked decisions (WD-GT-*) with rationale. Summary table lives in
> `../feature-delta.md`. Brownfield DELTA on slices 04/06/07/08/09.

## DIVERGE artifacts

NOT PRESENT for this slice (no `docs/feature/viewer-graph-traversal/diverge/`).
This is a brownfield DELTA whose job (J-002 / sub-job J-002b) was validated in the
slice-04 DIVERGE/DISCUSS. **Risk noted:** no fresh DIVERGE; mitigated because the
grounding job, journey, persona hat, and KPI contracts are all SSOT from
slice-04/09 and read directly (`docs/product/jobs.yaml` J-002b,
`docs/product/journeys/explore-the-graph.yaml`,
`docs/product/personas/researcher-tech-lead.yaml` graph-explorer hat,
`docs/product/kpi-contracts.yaml` KPI-GRAPH-*). JTBD was NOT re-run; the validated
J-002b statement is grounded verbatim.

## Scope Assessment: PASS — 4 stories, 6 touchpoints (no new crate), estimated ≤ 1 day

Run BEFORE journey visualization. Oversized signals checked: stories 4 (≤10 ✓);
bounded contexts/modules 6 — the established viewer touchpoint set (≤3 *new*
modules: the work is concentrated in viewer-domain + adapter-http-viewer, with
thin read additions to adapter-duckdb/ports and wiring in cli/xtask) ✓; integration
points 2 new read methods + cross-link wiring ✓; effort ≤ 1 day ✓; one independent
user outcome (traverse the local graph in the browser) ✓. Right-sized; no split.
A thinner `/philosophy`-only first slice was considered and rejected (the two
survey surfaces are symmetric over one new read + one new render pattern; splitting
doubles integration scaffolding for a fraction of a day; the cross-link story is
the connective tissue that makes traversal a journey and must ship together).

## Locked decisions

### WD-GT-1 — Two new LOCAL GET routes; contributor reuses /score
`GET /project?subject=<uri>` (project survey) and `GET /philosophy?object=<uri>`
(philosophy survey). The contributor dimension is NOT a new route — every
contributor edge links to the slice-09 `/score?contributor=<did>`.
**Rationale:** symmetric subject/object surfaces complete the J-002a dimension
trio in the browser (contributor already shipped). Reusing `/score` avoids a
parallel contributor surface and gives traversal a transparent-weight terminus
for free.

### WD-GT-2 — Persona = P-001 (Maria), graph-explorer hat
**Rationale:** the browser viewer is P-001's surface (slices 06–09); slice-04
framed P-002 primary for the CLI graph-explorer, but the loopback `openlore ui`
operator is Maria wearing the same graph-explorer hat
(`researcher-tech-lead.yaml hats[].graph-explorer`).

### WD-GT-3 — Read-only (three-layer enforced)
Traversal is a READ; no write/sign/follow route; no key in the process.
**Rationale:** carries KPI-VIEW-2 / KPI-HX-G3 verbatim. The StoreReadPort
no-mutation type + xtask viewer capability rule + behavioral gold all stay green;
the two new survey reads add no mutation method.

### WD-GT-4 — LOCAL-only / offline
Both routes read the LOCAL DuckDB store (claims ∪ peer_claims); NO network seam.
**Rationale:** distinct from `/search` (indexer) and `/scrape` (GitHub). Carries
KPI-5 / KPI-GRAPH-6 / KPI-VIEW-5 — both routes render fully network-down. This is
offline-STRONGER than `/search` (which needs the network for the search itself).

### WD-GT-5 — Anti-merging in surveys
A survey is an AGGREGATE VIEW that never merges authors: two claims on one
(subject, object) by two authors = two attributed rows; no average, no consensus
row. Every edge carries `author_did` (non-Option) + `cid`.
**Rationale:** the cardinal OpenLore invariant. Extends KPI-GRAPH-2 / KPI-FED-1/2
/ I-FED-1 onto the new surfaces. Grouping in pure Rust, never SQL (UNION ALL, no
merge JOIN — mirrors slice-09). Release-blocking.

### WD-GT-6 — No invented edges
Every displayed edge maps to exactly one signed claim (its cid); an empty survey
renders "no claims," never a fabricated edge.
**Rationale:** the slice-04 J-002b traversal contract ("traversal invents no
edges; every edge MUST map to exactly one signed claim").

### WD-GT-7 — Verbatim confidence + display-only bucket (no recompute)
Each edge row shows verbatim numeric confidence (`0.90`) + the slice-04
display-only bucket; the viewer recomputes NO weight; the full weighted breakdown
stays at `/score` (J-002c).
**Rationale:** KPI-4 / FR-VIEW-8 / WD-10. Reuse the single `render_confidence` +
bucket site. Keeps J-002c (weighting) a clean boundary out of this slice.

### WD-GT-8 — Progressive enhancement + offline chrome
Both routes serve a full page without `HX-Request` and a fragment of the same
results region with it (slice-07 `Shape` fork; page = chrome + fragment). Cross-
links are plain `<a href>` (no-JS click = full navigation); an htmx swap is
optional. htmx is the vendored, SHA-256-pinned local asset.
**Rationale:** I-HX-1..5 / KPI-HX-G1 / KPI-HX-G2. A swap is a nicety, never a
requirement.

### WD-GT-9 — Zero new persisted types; loopback-only bind
Surveys computed per-query, never persisted; bind stays 127.0.0.1.
**Rationale:** BR-VIEW-2 / I-VIEW-1 / I-VIEW-4.

### WD-GT-10 — Out of scope (explicit)
NO authoring/sign/counter; NO network on these routes; NO new weighting surface
(link to /score); NO follow execution (link only); NO multi-hop auto-expanded tree
(survey is depth-1; each edge is a link; browser back/forward IS the traversal
stack); NO new crate; NO write port; NO key.
**Rationale:** keeps the slice thin (≤1 day) and the boundaries with J-001 (author),
J-002c (weight), J-003/J-005 (follow/network) clean. The slice-04 CLI `--depth K`
tree is a deferred enhancement, not this slice's value.

## Open questions for DESIGN (not blocking DISCUSS)

- **Q1 (low):** Contributor cross-link DID form — bare `did:plc:rachel-test` vs the
  app-identity `…#org.openlore.application` the `/score` resolver expects. Mirror
  the slice-08 `/search` resolver. DESIGN owns.
- **Q2 (low):** Subject/object href percent-encoding for URIs containing `/` and
  `:`. Reuse the existing `percent_decode_form` decode + standard encode. DESIGN
  owns.
- **Q3 (low):** Whether a single parametrized read backs both surveys or two
  methods. DESIGN owns; product contract is "every matching attributed claim, no
  merge, no invented edge."

These are bounded implementation decisions, not product ambiguities — none blocks
the DoR gate. No red cards.
