# Feature Delta — philosophy-vocabulary-registry

> DISCUSS wave output. Density: **lean** (Tier-1 [REF] only). Make *philosophy* a
> first-class, discoverable, **seeded-but-open** shared vocabulary so users
> classify projects and contributors against a common set instead of inventing
> free-form `org.openlore.philosophy.*` strings that never triangulate.

---

## Wave: DISCUSS / [REF] Persona

**P-001 — Senior Engineer Solo Builder** (primary): classifies projects/contributors
by philosophy and needs a *shared* vocabulary so his claims connect to others' in
the graph. **P-002 — Researcher / Tech Lead** (secondary, graph-explorer hat):
relies on the same shared objects for triangulation when exploring the graph.

## Wave: DISCUSS / [REF] JTBD one-liner

**The load-bearing gap** (spans J-001 authoring, J-002 exploration, J-004 contributor lens):
> When I classify a project's (or a contributor's) philosophy, I want to pick from
> and discover a *shared* vocabulary of well-defined philosophies — and extend it
> when mine is missing — so my classification triangulates with everyone else's
> instead of stranding on a string only I use.

Traceability: primary **`job_id: J-002`** (discovery/triangulation depends on shared
objects) with **J-001** (the vocabulary is the claim `object` at authoring time) and
**J-004** (a contributor's philosophy profile only aggregates if the objects match).
Today the philosophy *record* is a RED scaffold (`lexicon::validate_philosophy_json`
panics; no seeds; no `openlore philosophy` verb), so classification works mechanically
but does not federate — the whole point of OpenLore.

- **Functional**: discover, reuse, and extend a shared set of named philosophies.
- **Emotional**: confident my classification *connects* ("I'm speaking the same language").
- **Social**: a good citizen of a shared graph, not a coiner of private strings.

## Wave: DISCUSS / [REF] Locked decisions

- **[D1] Philosophy is a first-class SIGNED RECORD.** Implement the
  `org.openlore.philosophy` record already defined in
  `lexicons/org/openlore/philosophy.json` (`name`, `description`, optional
  `aliases`, `seeAlso`) — completing the `validate_philosophy_json` RED scaffold.
  A philosophy is authored/signed like a claim (no truth-arbiter), local-first, offline.
- **[D2] Seeded but OPEN (federated, no gatekeeper).** Ship ~10 curated
  well-known seed philosophy records for discovery + triangulation, AND let anyone
  mint a new philosophy record (`openlore philosophy add`). No central registry
  owns the namespace; new philosophies federate like claims/peers.
- **[D3] Advisory at compose time, never enforcing.** `claim add` SUGGESTS known
  philosophies for `--object` and can flag an unknown object as "not a known
  philosophy (it will still be signed)", but NEVER rejects it — preserves "claims,
  not truth" and the right to classify with a novel philosophy.
- **[D4] Aliases power triangulation.** `aliases` on a philosophy record let
  `mem-safety` resolve to the same philosophy as `memory-safety` in exploration/
  scoring, so near-synonyms connect instead of fragmenting the graph.
- **[D5] Authoring stays CLI; the viewer stays read-only.** The philosophy verbs
  (`list`/`show`/`add`) are CLI (hold the key). Any viewer surface is a read-only
  browse of the vocabulary (I-VIEW-1/3 preserved — no key in the web process).
- **[D6] The seeds are the single source for the scraper.** The scraper's
  signal→predicate mapping (slice-02) references the SEEDED philosophy records
  rather than a hardcoded literal list, so the proposed objects are always in the
  shared vocabulary (no drift, e.g. today's stray `org.openlore.philosophy.mystery`).

## Wave: DISCUSS / [REF] User stories

Each story maps to a carpaccio slice (see Story map). Every story `job_id` noted.

### US-PV-001 — Discover the shared philosophy vocabulary  ·  `job_id: J-002`
As P-001 about to classify a project, I want to list the known philosophies so I
pick a shared object instead of guessing a string.
#### Elevator Pitch
Before: there is no list of philosophies — I invent `org.openlore.philosophy.<something>` and hope others used the same string.
After: run `openlore philosophy list` → sees the ~10 seeded philosophies (id, name, one-line description), e.g. `memory-safety — Programs cannot corrupt memory…`.
Decision enabled: I pick an existing philosophy's exact object for my claim, so it triangulates.
#### Acceptance Criteria
- **AC-001.1** GIVEN an initialized store, WHEN I run `openlore philosophy list`, THEN it prints each seeded philosophy's stable object id (`org.openlore.philosophy.<name>`), name, and one-line description, one per block/line, greppable.
- **AC-001.2** GIVEN the seed set, THEN it contains at least the ~10 well-known philosophies (incl. memory-safety, type-safety, test-driven, documentation-first, dependency-pinning, semantic-versioning) — each a valid record (name + description present).
- **AC-001.3** GIVEN `--json`, THEN the list is emitted as JSON (opt-in; text default per P-001 ux_guardrails).
- **AC-001.4** GIVEN the store, THEN listing is LOCAL/offline (reads seeded records on disk; no network).

### US-PV-002 — Inspect one philosophy  ·  `job_id: J-002`
As P-002 deciding whether a philosophy fits, I want its full definition + aliases + see-also.
#### Elevator Pitch
Before: I can't tell what `memory-safety` officially means or what near-names it subsumes.
After: run `openlore philosophy show memory-safety` → sees the name, full description, `aliases: [mem-safety, memory-safe]`, and `seeAlso` links.
Decision enabled: I confirm this is the right philosophy (and which alias strings triangulate) before I classify.
#### Acceptance Criteria
- **AC-002.1** GIVEN a known philosophy, WHEN `openlore philosophy show <name-or-object>`, THEN it prints name, description, aliases, seeAlso verbatim from the signed record.
- **AC-002.2** GIVEN an UNKNOWN name, THEN it exits non-zero with a plain "no such philosophy; try `philosophy list` or `philosophy add`" (never a stack trace).

### US-PV-003 — Mint a new philosophy  ·  `job_id: J-001`
As P-001 with a philosophy the seeds lack, I want to add it to the vocabulary — federated, no gatekeeper.
#### Elevator Pitch
Before: an unlisted philosophy has no shared definition; I just type a novel object string with no record behind it.
After: run `openlore philosophy add --name event-sourcing --description "State is an append-only log of events." [--alias es]` → it signs + persists an `org.openlore.philosophy` record and prints its object id.
Decision enabled: I classify with `event-sourcing` knowing it now has a shared, signed, discoverable definition others can adopt.
#### Acceptance Criteria
- **AC-003.1** GIVEN name + description, WHEN `openlore philosophy add`, THEN a SIGNED `org.openlore.philosophy` record is composed (name/description/aliases/seeAlso), validated by `validate_philosophy_json`, and persisted locally; the object id is printed.
- **AC-003.2** GIVEN the compose flow, THEN it mirrors `claim add` (sign prompt; local-first; publish deferrable) — no new signing model; author DID recorded.
- **AC-003.3** GIVEN a name that collides with a seed, THEN it is refused with guidance (use the existing one, or `--alias` onto it) — no silent duplicate id.
- **AC-003.4** GIVEN the invalid-record path (missing description), THEN `validate_philosophy_json` rejects with a named-field error (no panic — completes the RED scaffold).

### US-PV-004 — Compose a claim against the vocabulary (advisory)  ·  `job_id: J-001`
As P-001 authoring a claim, I want `--object` to suggest known philosophies and flag an unknown one, without ever blocking me.
#### Elevator Pitch
Before: `claim add --object <anything>` silently accepts any string; no hint whether it's a real shared philosophy.
After: run `openlore claim add … --object org.openlore.philosophy.mem-safety` → the compose preview notes `↳ resolves to memory-safety (alias)` OR `⚠ not a known philosophy — will be signed as-is`.
Decision enabled: I correct a typo/alias to the shared object (triangulate), or knowingly proceed with a novel one.
#### Acceptance Criteria
- **AC-004.1** GIVEN a known/alias object, WHEN composing a claim, THEN the preview shows it resolves to the canonical philosophy (advisory line).
- **AC-004.2** GIVEN an unknown object, THEN the preview shows a non-blocking warning; the claim STILL signs unchanged if confirmed (D3 — never rejects, "claims not truth").
- **AC-004.3** GIVEN the resolution, THEN it is LOCAL/offline and does NOT change the signed payload (the object bytes the user typed are what get signed — advisory is display-only).

### US-PV-005 — Triangulate via aliases in exploration  ·  `job_id: J-002 / J-004`
As P-002 exploring the graph, I want `mem-safety` and `memory-safety` to count as the same philosophy so near-synonyms connect.
#### Elevator Pitch
Before: a claim on `mem-safety` and one on `memory-safety` are two disconnected objects; the triangulation the whole product promises fails.
After: run `openlore graph query --object org.openlore.philosophy.memory-safety` → results INCLUDE claims made against its aliases (`mem-safety`), grouped under the canonical philosophy.
Decision enabled: I trust the philosophy view aggregates all the evidence, not just the exact-string subset.
#### Acceptance Criteria
- **AC-005.1** GIVEN a philosophy with aliases, WHEN I query/score by its canonical object, THEN claims authored against any alias are included, attributed to their authors (anti-merging preserved), grouped under the canonical philosophy.
- **AC-005.2** GIVEN alias resolution, THEN it is a DERIVED read-time view — it NEVER rewrites the stored claim objects (the signed bytes are immutable; resolution is display/aggregation only).

### US-PV-006 — Browse philosophies in the viewer (read-only)  ·  `job_id: J-002`
As P-001/P-002 in the viewer, I want a read-only philosophies surface to discover the vocabulary where I'm already looking.
#### Elevator Pitch
Before: the vocabulary is CLI-only; in the browser I can't see what philosophies exist.
After: open `http://127.0.0.1:8788/philosophies` → sees the list of philosophies (name + description), each linking to its `/philosophy?object=…` traversal.
Decision enabled: I discover a philosophy and jump straight into who embodies it.
#### Acceptance Criteria
- **AC-006.1** GIVEN the viewer, WHEN I GET `/philosophies`, THEN it renders the vocabulary read-only (name + description + a link to the existing `/philosophy?object=` surface), no authoring control (I-VIEW-1/3), offline.
- **AC-006.2** GIVEN the persistent nav (slice-21), THEN `/philosophies` is reachable as a surface (added to `LANDING_HUB_SURFACES`).

### US-PV-007 — Scraper proposes seeded philosophies  ·  `job_id: J-004`
As P-001 running the scraper, I want its proposed objects to come from the seeded vocabulary so proposals are always shared philosophies.
#### Elevator Pitch
Before: the scraper hardcodes ~5 philosophy strings (and a stray `mystery`) with no link to a shared record.
After: run `openlore scrape github:rust-lang/rust` → the proposed candidate objects are seeded philosophy ids (each `philosophy show`-able), no orphan strings.
Decision enabled: every scraper-proposed claim I sign is already in the shared vocabulary.
#### Acceptance Criteria
- **AC-007.1** GIVEN the seeded vocabulary, THEN the scraper's signal→predicate mapping references seeded philosophy records (single source), and every proposed object is a known philosophy (`philosophy show` resolves it).
- **AC-007.2** GIVEN a signal with no seeded philosophy, THEN the mapping is explicit (either a seeded object or clearly absent) — no drift string like `org.openlore.philosophy.mystery`.

## Wave: DISCUSS / [REF] Outcome KPIs

| KPI | Target | Measurement |
|-----|--------|-------------|
| KPI-PV-1 — Vocabulary exists | ≥10 seeded, valid (name+description) philosophy records | `philosophy list` count; `validate_philosophy_json` passes each. |
| KPI-PV-2 — Discoverable | `philosophy list` + `show` return the seeds LOCAL/offline | AT over the two verbs. |
| KPI-PV-3 — Open extension | `philosophy add` mints a signed record; `validate_philosophy_json` rejects invalid | AT: add + re-list; reject-arm AT. |
| KPI-PV-4 — Advisory compose | `claim add` flags unknown vs resolves alias, NEVER blocks | AT: known/alias/unknown compose paths; signed bytes unchanged. |
| KPI-PV-5 — Triangulation | alias-object claims aggregate under the canonical philosophy in graph/score | AT: seed a claim on an alias, query canonical, see it grouped. |
| KPI-PV-6 — No drift | scraper proposes only seeded objects (0 orphan philosophy strings) | AT: scrape → every proposed object `philosophy show`-resolves. |

## Wave: DISCUSS / [REF] Definition of Done

1. All US-PV-001..007 ACs pass as acceptance tests (paradigm-appropriate).
2. `validate_philosophy_json` implemented (RED scaffold completed) with accept + reject arms tested.
3. ≥10 seeded philosophy records shipped, each valid + `show`-able + `list`-ed.
4. `openlore philosophy list|show|add` verbs live; `add` signs a record via the existing signing model.
5. `claim add` advisory resolution (known/alias/unknown) with the signed payload byte-unchanged.
6. Alias triangulation in graph/score is a read-time derivation (stored objects immutable).
7. Read-only `/philosophies` viewer surface (+ nav entry), no authoring control, offline.
8. Scraper mapping sourced from the seeds (no orphan objects).
9. `xtask check-arch` OK (workspace member count updated only if a new crate is genuinely needed — prefer NOT).

## Wave: DISCUSS / [REF] Out of scope

- A central/authoritative philosophy taxonomy or moderation (federation, no gatekeeper — D2).
- Rejecting/blocking unknown claim objects (advisory only — D3).
- Philosophy *hierarchies*/ontology (parent/child) beyond flat `seeAlso` links.
- Editing/merging others' philosophy records (they're signed; counter/alias, don't overwrite).
- Authoring philosophies from the viewer (stays CLI, read-only viewer — D5).
- Auto-classifying projects (the scraper proposes; the human signs — J-004c invariant).

## Wave: DISCUSS / [REF] WS strategy

**Strategy B (extend existing) — no walking-skeleton-as-Feature-0.** Brownfield: the
claim record + signing (slice-01), scraper (slice-02), scoring/graph (slice-04), and
the `philosophy.json` Lexicon + `validate_philosophy_json` RED scaffold already exist.
The feature's own thin foundation is **slice-22 (seed + list)** — the smallest
end-to-end vocabulary that delivers discovery.

## Wave: DISCUSS / [REF] Driving ports

- CLI: new `openlore philosophy {list|show|add}` verb; extended `claim add` (advisory); `graph query`/`score` (alias triangulation); `scrape` (seeded objects).
- Pure core: `lexicon`/`claim-domain` philosophy record type + `validate_philosophy_json` + alias resolution (pure).
- HTTP: read-only `/philosophies` viewer surface (adapter-http-viewer + viewer-domain).

## Wave: DISCUSS / [REF] Pre-requisites

- slice-01 `openlore-foundation` (claim record + signing model + local store) — SHIPPED.
- slice-02 `openlore-github-scraper` (signal→predicate mapping) — SHIPPED.
- slice-04 `openlore-scoring-graph` (graph query/score over objects) — SHIPPED.
- slice-21 `viewer-persistent-left-nav` (`LANDING_HUB_SURFACES` + nav for the new surface) — SHIPPED.
- Seams: `lexicons/org/openlore/philosophy.json` (record schema) + `lexicon::validate_philosophy_json` (RED scaffold to complete).

## Wave: DISCUSS / [REF] Scope assessment

**OVERSIZED for one slice** (7 stories, ≥4 modules: lexicon/claim-domain, cli, viewer, scraper; multiple independent user outcomes that ship separately). **Split into 7 thin carpaccio slices** (below), each ≤1 day, each shippable + dogfoodable alone. Recommended first ship: **slice-22 (seed + list)** — the discovery walking skeleton.

## Wave: DISCUSS / [REF] Story map

Activity — **Classify against a shared philosophy vocabulary**:

```
slice-22 philosophy-registry-seed-and-list   US-PV-001  (+validate_philosophy_json, seeds)  ← thin foundation, ship first
slice-23 philosophy-show                      US-PV-002
slice-24 philosophy-mint                       US-PV-003  (openlore philosophy add — signed, open)
slice-25 claim-compose-suggests-philosophy     US-PV-004  (advisory resolution at claim add)
slice-26 philosophy-alias-triangulation        US-PV-005  (mem-safety ~ memory-safety in graph/score)
slice-27 viewer-philosophies-surface           US-PV-006  (read-only /philosophies + nav entry)
slice-28 scraper-uses-seeded-philosophies      US-PV-007  (single source; no orphan objects)
```

Prioritization: **slice-22 first** (highest learning leverage — does a seeded vocabulary actually make classification discoverable?), then 24 (open extension proves the federated model) and 25 (compose-suggest closes the authoring loop) for the core value; 26 (triangulation) is the payoff; 23/27/28 are ergonomics/reach. Briefs: `docs/feature/philosophy-vocabulary-registry/slices/slice-22..28-*.md`.

## Wave: DISCUSS / [REF] Definition of Ready (9/9)

1. **User need clear** ✓ — a shared, discoverable, extensible philosophy vocabulary (the code gap is verified: `validate_philosophy_json` panics, no seeds, no verb).
2. **Job traceability** ✓ — stories trace to J-001/J-002/J-004 (existing validated jobs).
3. **Elevator pitches** ✓ — every story has Before/After/Decision with a real CLI/HTTP entry point + observable output.
4. **ACs testable** ✓ — each AC is a command/route with observable stdout/HTML (list output, signed record, advisory line, grouped query, /philosophies page).
5. **KPIs measurable** ✓ — KPI-PV-1..6 numeric/binary with AT methods.
6. **Scope bounded** ✓ — 7 thin slices, explicit out-of-scope; slice-22 first.
7. **Dependencies satisfied** ✓ — prereq slices SHIPPED; the philosophy Lexicon + scaffold exist.
8. **Invariants named** ✓ — signed records/no-arbiter, local-first/offline, advisory-not-enforcing, read-only viewer, anti-merging in triangulation.
9. **Sizing** ✓ — each slice ≤1 day with a learning hypothesis (per brief).

## Wave: DISCUSS / [REF] Wave decisions summary

### Key Decisions
- [D1] Philosophy = first-class signed record (complete the `validate_philosophy_json` scaffold).
- [D2] Seeded but OPEN — curated seeds + anyone mints; federated, no gatekeeper.
- [D3] Advisory at compose, never enforcing (claims-not-truth).
- [D4] Aliases power read-time triangulation (stored objects immutable).
- [D5] Authoring stays CLI; viewer read-only.
- [D6] Seeds are the scraper's single source (no drift).

### Requirements Summary
- Primary need: a shared, discoverable, extensible philosophy vocabulary so project/contributor classification federates (J-002/J-001/J-004).
- Walking skeleton: slice-22 (seed + list) — the thin discovery foundation.
- Feature type: cross-cutting (lexicon/claim-domain + cli + viewer + scraper).

### Constraints Established
- Complete `validate_philosophy_json` (no panic); ship ≥10 seeds.
- Advisory-only compose; signed bytes never altered by resolution.
- Alias triangulation is read-time derivation; anti-merging preserved.
- Prefer NO new crate (extend lexicon/claim-domain + cli + viewer); check-arch stays 21 unless a seed data crate is genuinely warranted.

### Upstream Changes
- None contradicting DISCOVER (none exists). jobs.yaml J-002 gains a "shared-vocabulary" sub-aspect (recorded via a J-002 sub-job on SSOT update); no re-scoring.

---

## Wave: DISTILL

> Scope: **slice-22 only** (the seed + list discovery walking skeleton — US-PV-001,
> AC-001.1..4, + the `validate_philosophy_json` accept/reject arms that complete the RED
> scaffold). Slices 23–28 (show / mint / compose-advisory / alias / viewer / scraper) are
> DISTILLed later. Density: **lean** (Tier-1 [REF] only). `[lang-mode] rust` ·
> `[policy-mode] inherit` · `[port-mode] inherit`.

### [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| DISCUSS#D1 | Philosophy is a first-class record; complete the `validate_philosophy_json` RED scaffold with accept + reject arms | ADR-059 D1/D2 | Layer-2 in-crate tests pin the accept arm + the named-field reject arm (missing `description` → `LexiconError::MissingField`), reusing the existing error enum — no parallel type |
| DISCUSS#D2 | Seeded but open: ship ≥10 curated well-known seed records for discovery | ADR-059 D3 | PV-2/PV-4 assert ≥10 distinct seed object ids, each a valid record (name + description present); embedded constants (no signer, no store) |
| DISCUSS#US-PV-001 | `openlore philosophy list` prints each seed's object id + name + one-line description, greppable, LOCAL/offline | ADR-059 D7 | PV-1 (WS) + PV-3/PV-5/PV-6 exercise the real CLI driving adapter via subprocess; offline listing asserted with the network disabled |
| DESIGN(ADR-059)#D1 | Object id is DERIVED `org.openlore.philosophy.<normalize(name)>`, backward-compatible with slice-01 claim `object` bytes | ADR-059 D1 | PV-3 pins the exact dotted-lowercase id form + guards against a `:`-separated / CamelCase drift that would strand the claim-graph join |

### [REF] Reconciliation result

Wave-Decision Reconciliation HARD GATE: **PASS — 0 contradictions.** DISCUSS (D1–D6:
signed record `{name, description, aliases, seeAlso}`, ≥10 embedded seeds, offline `list`)
is consistent with DESIGN/ADR-059 (D1 reconciles the struct to that shape, D3 embedded
seeds, D7 offline list verb). The current stale scaffold struct (`{id, label, description}`)
is exactly the RED scaffold ADR-059 D1 mandates completing — an expected RED cause, not a
cross-wave contradiction. No per-feature `discuss|design|devops/wave-decisions.md` files
exist; reconciliation used feature-delta DISCUSS vs ADR-059 DESIGN.

### [REF] Scenario list with tags

Tier A only (Gojko-style, production composition root via the real `openlore` bin). Tier B
(state-machine PBT) is correctly SKIPPED per Mandate 10: slice-22 is a config/CLI-shaped
single-shot read-only list verb (1 journey, no ≥3-scenario chained state mutation).

| # | Scenario (test fn) | AC | Tags |
|---|---|---|---|
| PV-1 | `philosophy_list_prints_each_seed_object_id_name_and_description` | AC-001.1 | `@us-pv-001 @driving_port @real-io @walking_skeleton @j-002 @kpi-pv-2 @happy` |
| PV-2 | `the_seed_set_contains_at_least_ten_well_known_philosophies` | AC-001.2 | `@us-pv-001 @driving_port @real-io @j-002 @kpi-pv-1 @happy` |
| PV-3 | `each_seed_object_id_matches_the_slice_one_claim_object_bytes` | AC-001.1 / ADR-059 D1 | `@us-pv-001 @driving_port @real-io @j-002 @backward-compat @edge` |
| PV-4 | `philosophy_list_json_emits_each_record_with_name_and_description` | AC-001.3 | `@us-pv-001 @driving_port @real-io @j-002 @json @happy` |
| PV-5 | `philosophy_list_defaults_to_human_text_not_json` | AC-001.3 | `@us-pv-001 @driving_port @real-io @j-002 @text-default @edge` |
| PV-6 | `philosophy_list_succeeds_with_the_network_disabled` | AC-001.4 | `@us-pv-001 @driving_port @real-io @j-002 @local-first @i-9 @edge` |
| LX-accept | `validates_well_formed_philosophy_record` (lexicon in-crate) | AC-001.2 / DoD-2 | layer-2 accept arm |
| LX-reject | `rejects_missing_description_with_named_field_error` (lexicon in-crate) | AC-003.4 / DoD-2 | layer-2 named-field reject arm |

### [REF] Walking-skeleton designation

**PV-1** is the slice-22 walking skeleton (`@walking_skeleton @driving_port @real-io`): the
thin end-to-end discovery path — embedded seed constants → CLI driving adapter (`openlore
philosophy list`) → user-visible greppable stdout (object id + name + description). Litmus:
a non-technical stakeholder confirms "yes — the user needs to SEE the shared vocabulary so
they can copy an exact object into a claim." No store write, no network.

### [REF] Adapter coverage

| Adapter | @real-io scenario | Covered by |
|---------|-------------------|------------|
| CLI driving adapter (`openlore philosophy list`) | YES | PV-1..PV-6 (real `openlore` bin via `run_openlore` / `run_openlore_network_disabled` subprocess) |
| `lexicon` embedded seed set (`include_str!`, ADR-059 D3) | YES | PV-1/PV-2/PV-3/PV-6 (real embedded constants; no signer, no store, no network — offline by construction) |

No DRIVEN external / non-deterministic adapter is exercised by slice-22 (read-only, offline;
the fake PDS is asserted UNUSED in PV-6 via `assert_no_pds_call_was_made`). Signing/DuckDB
persistence (minted records) belong to slice-24 — OUT of scope.

### [REF] Test placement + scaffolds

- `tests/acceptance/philosophy_vocabulary.rs` — Tier A subprocess acceptance (layer 3),
  registered as `[[test]] name = "philosophy_vocabulary"` in `crates/cli/Cargo.toml`
  (precedent: every sibling acceptance target, e.g. `graph_query_explore`, is a `cli`
  `[[test]]` over `../../tests/acceptance/*.rs`).
- `crates/lexicon/src/lib.rs` — `#[cfg(test)] mod philosophy_validator_tests` (layer-2 pure
  accept/reject arms; precedent: `claim.rs mod tests`).
- **No new production scaffold created** — `lexicon::validate_philosophy_json` +
  `philosophy::Philosophy` already exist as the RED scaffold (`SCAFFOLD: true` in lib.rs)
  left by slice-01; DISTILL only adds RED tests against them (Mandate 7 satisfied by the
  existing scaffold — imports resolve, methods panic = RED not BROKEN).

### [REF] RED gate

Pre-DELIVER fail-for-the-right-reason gate: **PASS.** 8 tests, all RED for the right reason
(6 acceptance assertion-RED on `unrecognized subcommand 'philosophy'` exit 2; 2 validator
scaffold-panic RED). Zero BROKEN, zero GREEN-today. Full classification:
`docs/feature/philosophy-vocabulary-registry/distill/red-classification.md`.

### [REF] Pre-requisites

- DESIGN driving port: `openlore philosophy list` CLI verb (ADR-059 D7) — the entry point
  PV-1..6 enter through. RED today (verb absent).
- Embedded seed set (ADR-059 D3) + reconciled `Philosophy` struct (ADR-059 D1) + real
  `validate_philosophy_json` (ADR-059 D2) — the DELIVER work that turns all 8 tests GREEN.
- Support harness: `run_openlore`, `run_openlore_network_disabled`, `TestEnv::initialized`,
  `assert_no_pds_call_was_made` (all present in `tests/acceptance/support/mod.rs`).
