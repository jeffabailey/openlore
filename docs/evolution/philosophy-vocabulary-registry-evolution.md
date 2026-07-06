# Evolution: philosophy-vocabulary-registry (slice-22 — a discoverable shared philosophy vocabulary: the `org.openlore.philosophy` record reconciled to the ADR-059 schema + `validate_philosophy_json` completed + ≥10 embedded seeds + an offline `openlore philosophy list` verb)

> Feature archive. Authored at finalize (DELIVER close). Source of truth for all
> detail remains the feature workspace `docs/feature/philosophy-vocabulary-registry/`
> (the single-narrative `feature-delta.md` carrying the DISCUSS/DISTILL [REF] sections,
> plus `design/architecture-design.md`, `distill/red-classification.md`, `slices/`, and
> `deliver/` — roadmap.json, execution-log.json, mutation/mutation-report.md) and
> **ADR-059** under `docs/adrs/`; this file is the post-mortem summary. slice-22 is the
> **thin foundation of a 7-slice feature** (US-PV-001..007 → slices 22–28): it ships the
> smallest end-to-end vocabulary that delivers **discovery** — completing the RED scaffold
> `lexicon::validate_philosophy_json`, embedding the seed set, and exposing a read-only
> offline `philosophy list`. It realizes **J-002** ("classify against a *shared* vocabulary
> so my claims triangulate") in its discovery form: a user can now SEE the shared
> vocabulary and copy an exact object id into a claim instead of inventing a private string.

## Summary

`philosophy-vocabulary-registry` (slice-22) turns a stale RED scaffold into a working,
discoverable shared vocabulary along a single thin path: **embedded seed constants →
pure validator + object-id derivation → CLI `philosophy list` driving adapter →
user-visible greppable stdout**. Five pieces landed. (1) The pure
`lexicon::philosophy::Philosophy` record was **reconciled** from the stale
`{id, label, description}` shape to the ADR-059 schema `{name, description, aliases,
seeAlso}` (the `id`/`label` fields dropped; the object id is DERIVED, never stored).
(2) **`validate_philosophy_json`** was implemented as a per-field-gated pure validator
mirroring `validate_claim_json` — required `name`/`description` missing →
`LexiconError::MissingField`; `aliases`/`seeAlso` not array-of-string →
`LexiconError::InvalidType`; serde catch-all → `LexiconError::SchemaMismatch` — **reusing
the existing `LexiconError` enum, no parallel error type**. (3) A pure total **`normalize`
+ `object_id(name) = "org.openlore.philosophy." + normalize(name)`** derivation,
byte-compatible with the slice-01 claim `object` bytes (ADR-059 D1 dotted-lowercase NSID)
so the vocabulary joins the claim graph. (4) **12 unsigned seed records** embedded via
`include_str!("seeds.json")` (the six hard-pinned — memory-safety, type-safety, test-driven,
documentation-first, dependency-pinning, semantic-versioning — plus six more) with a public
`seeds()` accessor. (5) **`openlore philosophy list`**
(`Command::Philosophy(PhilosophyCommand::List{json})`, verb `verbs/philosophy_list.rs`
+ render helper `render/philosophy.rs`), **dispatched BEFORE `Wiring::production` so it is
offline / store-free by construction** — text default, `--json` opt-in JSON array.

The load-bearing thesis: **provenance is a type, not a comment, and discovery is offline by
construction.** The seeds are explicit *unsigned* embedded constants (minting signed records
is slice-24, OUT of scope); the `list` verb holds no key, opens no store, and makes no
network call because it is dispatched before the production wiring is ever constructed. The
object id is a pure derivation of the name, not a stored field — so the vocabulary joins the
claim graph on exactly the bytes slice-01 already signs. The slice ships **NO new crate**
(the record type + validator + seed data live in `lexicon`; the verb in `cli`); the
workspace stays **21 members** (`check-arch` OK).

### What shipped (one paragraph)

The pure `lexicon::philosophy::Philosophy` record now matches the ADR-059 schema
`{name, description, aliases (serde default), see_also (serde rename "seeAlso", default)}`;
**`validate_philosophy_json`** is a per-field-gated pure validator mirroring
`validate_claim_json` (required-field gates → `MissingField`, array-type gates →
`InvalidType`, serde catch-all → `SchemaMismatch`, all on the existing `LexiconError`);
a pure total **`normalize` + `object_id`** derives
`org.openlore.philosophy.<kebab(name)>`, byte-identical to the slice-01 claim object bytes;
**12 unsigned seed records** are embedded via `include_str!("seeds.json")` behind a public
`seeds()` accessor; and **`openlore philosophy list`** (a new `Command::Philosophy`
subcommand, `verbs/philosophy_list.rs` + `render/philosophy.rs`) prints — per seed — the
derived object id + name + one-line description in the **text default**, or a **`--json`
opt-in** array of the full records. The verb is **dispatched before `Wiring::production`**,
so it is offline and store-free by construction (PV-6 asserts success with the network
disabled; the fake PDS is asserted UNUSED). No new crate; the workspace stays **21 members**;
nothing is signed or persisted (minting is slice-24).

### Wave timeline

| Wave    | Date       | Commit  | Owner                                                        |
|---------|------------|---------|-------------------------------------------------------------|
| DISCUSS | 2026-07-05 | 98e8eaa | Luna (nw-product-owner)                                      |
| DESIGN  | 2026-07-05 | 957f2f6 | Morgan (nw-solution-architect) — ADR-059                     |
| DISTILL | 2026-07-05 | 364f650 | Quinn (nw-acceptance-designer) — RED scaffolds (seed + list) |
| DELIVER | 2026-07-05 | (below) | Crafter (nw-functional-software-crafter) + orchestration     |

### Shipping metrics

- **3 roadmap steps** across **2 phases** (all COMMIT/PASS — or APPROVED_SKIP with
  rationale — in `deliver/execution-log.json`). Roadmap **APPROVED** (decomposition ratio
  0.5 steps/production-file, no orphans, all 8 RED tests owned once, linear DAG
  01-01 → 02-01 → 02-02).
- **Acceptance scenarios GREEN**: the `philosophy_vocabulary` acceptance binary **8/8** —
  the 6 PV-* subprocess scenarios (PV-1 the **walking skeleton**: `philosophy list` prints
  each seed's object id + name + description; PV-2 ≥10 seeds; PV-3 backward-compat
  dotted-lowercase NSID matches the slice-01 claim object bytes; PV-4 `--json` array;
  PV-5 text-default; PV-6 offline / network-disabled) + 2 support-framework self-tests.
  Driven through the REAL `openlore` bin via the `run_openlore` /
  `run_openlore_network_disabled` subprocess harness.
- **`lexicon` suite GREEN — 39 passed**: the 2 in-crate validator arms (LX-accept
  `validates_well_formed_philosophy_record`, LX-reject
  `rejects_missing_description_with_named_field_error`) + seed-validity / no-normalize-collision
  + `normalize`/`object_id` property loops.
- **NO new crate (near-all-EXTEND)**: extends `crates/lexicon` (PURE — the reconciled
  record, `validate_philosophy_json`, `normalize`/`object_id`, the embedded `seeds.json` +
  `seeds()` accessor) and `crates/cli` (EFFECT — the `Command::Philosophy` verb,
  `verbs/philosophy_list.rs`, `render/philosophy.rs`, dispatch BEFORE `Wiring::production`).
  Diff is confined to `crates/lexicon/*` + `crates/cli/{lib.rs, render.rs, render/philosophy.rs,
  verbs/mod.rs, verbs/philosophy_list.rs}`. Workspace member count stays **21**;
  `cargo xtask check-arch` OK.
- **Mutation**: the pure core `crates/lexicon/src/philosophy.rs` = **100% (12/12 viable,
  2 unviable)** after killing 2 genuine `normalize` survivors; the effect shell (cli
  render/verb) 10/10 caught via the cross-crate acceptance binary. The per-feature gate
  (≥80% of viable) is **MET**.
- **1 ADR** (ADR-059) Accepted/shipped (slice-22 realizes its D1/D2/D3/D7 decisions).
- DES integrity: **3/3** steps have complete DES traces; integrity verification exit 0.
- Adversarial review (Phase 4): **APPROVED**, **0 blockers, 0 Testing Theater,
  0 test-integrity issues** (one non-blocking test-density note, left as-is — PBT density
  justified).
- Gates: **DoR/roadmap APPROVED** (0 blockers), DISTILL verified genuine **RED 8/8**
  (0 BROKEN), Phase-3 refactor **L1-L4 applied** (4d9da33), review **APPROVED**, mutation
  **pure-core 100% (12/12)**, integrity **3/3** exit 0, `check-arch` **OK (21)**.

## Wave-by-wave changelog

### DISCUSS (2026-07-05, 98e8eaa)

Luna framed a **shared, discoverable, seeded-but-open philosophy vocabulary** so
classification federates instead of stranding on private strings. The load-bearing gap spans
**J-001** (the vocabulary is the claim `object` at authoring), **J-002** (discovery /
triangulation depends on shared objects), and **J-004** (a contributor's philosophy profile
only aggregates if objects match). Six locked decisions: **[D1]** philosophy = first-class
signed record (complete the `validate_philosophy_json` scaffold); **[D2]** seeded but OPEN
(curated seeds + anyone mints; federated, no gatekeeper); **[D3]** advisory at compose, never
enforcing (claims-not-truth); **[D4]** aliases power read-time triangulation (stored objects
immutable); **[D5]** authoring stays CLI, viewer read-only; **[D6]** the seeds are the
scraper's single source (no drift). The 7-story feature (US-PV-001..007) was assessed
**OVERSIZED** and split into **7 thin carpaccio slices** (22–28), each ≤1 day; **slice-22
(seed + list)** was chosen to ship first as the discovery walking skeleton — the highest
learning leverage (does a seeded vocabulary actually make classification discoverable?).
**KPI-PV-1..6** framed. DoR PASS (**9/9**).

### DESIGN (2026-07-05, 957f2f6 — ADR-059)

Morgan formalized the feature as **ADR-059**, casting the registry as a **cross-cutting
extension** of the existing modular monolith (no new subsystem, no new crate). The
slice-22-relevant decisions: **D1** — reconcile the `Philosophy` record to
`{name, description, aliases, seeAlso}`; the object id is DERIVED
`org.openlore.philosophy.<normalize(name)>`, backward-compatible with the slice-01 claim
object bytes. **D2** — `validate_philosophy_json` per-field-gated, mirroring
`validate_claim_json`, reusing `LexiconError`. **D3** — ≥10 embedded unsigned seed constants
via `include_str!("seeds.json")` (mirroring the `PHILOSOPHY_LEXICON_JSON` pattern). **D7** —
the offline `philosophy list` verb. The C4 Context/Container/Component diagrams (the pure
`lexicon` core with the record / validator / `object_id`·`normalize` / seeds / VocabularyIndex
components, plus the effect edges for later slices) are in `design/architecture-design.md`.
A reuse-first table justified every seam as EXTEND-in-an-existing-crate; the trade-off note
recorded that "uniform all-signed" was rejected because the seeds have no signer —
provenance-split (unsigned seeds, signed mints) keeps D2's no-gatekeeper honest. Workspace
stays **21 members**; **no new crate** (principle 8).

### DISTILL (2026-07-05, 364f650 — RED scaffolds)

Quinn authored the slice-22 acceptance corpus (Tier A only — the slice is a config/CLI-shaped
single-shot read-only list verb, so Tier B state-machine PBT is correctly SKIPPED per
Mandate 10):

- **`tests/acceptance/philosophy_vocabulary.rs`** (`PV-` ids PV-1..PV-6, real `openlore` bin
  via `run_openlore` / `run_openlore_network_disabled`): **PV-1** the **walking skeleton**
  (`philosophy_list_prints_each_seed_object_id_name_and_description`), **PV-2**
  (`the_seed_set_contains_at_least_ten_well_known_philosophies`, KPI-PV-1), **PV-3**
  (`each_seed_object_id_matches_the_slice_one_claim_object_bytes` — the backward-compat guard
  against a `:`-separated / CamelCase drift that would strand the claim-graph join), **PV-4**
  (`philosophy_list_json_emits_each_record_with_name_and_description`), **PV-5**
  (`philosophy_list_defaults_to_human_text_not_json`), **PV-6**
  (`philosophy_list_succeeds_with_the_network_disabled` — the fake PDS asserted UNUSED via
  `assert_no_pds_call_was_made`).
- **`crates/lexicon/src/lib.rs`** in-crate (layer-2 validator arms): **LX-accept**
  `validates_well_formed_philosophy_record` + **LX-reject**
  `rejects_missing_description_with_named_field_error` (asserts
  `LexiconError::MissingField{field:"description"}`).

The **Wave-Decision Reconciliation HARD GATE passed** (0 contradictions — the stale
`{id, label, description}` scaffold is exactly the RED cause ADR-059 D1 mandates completing,
not a cross-wave contradiction). The **RED gate PASSED**: 8 tests, all RED for the right
reason — 6 acceptance assertion-RED on `unrecognized subcommand 'philosophy'` (clap exit 2)
+ 2 validator scaffold-panic RED (`panic!("Not yet implemented -- RED scaffold")`); **0
BROKEN, 0 GREEN-today**. Full classification in `distill/red-classification.md`. No new
production scaffold was created — the RED scaffold left by slice-01 already existed; DISTILL
only added tests against it (imports resolve, methods panic = RED not BROKEN).

### DELIVER (2026-07-05)

Executed **3 roadmap steps across 2 phases** via DES-monitored functional-crafter dispatches,
each commit carrying a `Step-ID: NN-NN` trailer; integrity verified exit 0. Per-step SHAs are
in `deliver/execution-log.json`.

- **Phase 01 — pure lexicon vocabulary core**: **01-01 (6406cc2)** reconciled the
  `Philosophy` record to the ADR-059 schema, implemented `validate_philosophy_json`
  (per-field-gated, reusing `LexiconError`), added the pure `normalize`/`object_id`
  derivation, and embedded the **12 seeds** via `include_str!("seeds.json")` behind
  `seeds()`. Greened the 2 in-crate validator arms (LX-accept, LX-reject) and added
  seed-validity / no-normalize-collision + `normalize`/`object_id` **property** tests. No
  signer, no store, no network.
- **Phase 02 — `philosophy list` CLI verb (discovery surface)**: **02-01 (3f00077)** added
  `Command::Philosophy(PhilosophyCommand::List{json})` to the clap enum, created
  `verbs/philosophy_list.rs` (registered in `verbs/mod.rs`) + `render/philosophy.rs`, and
  wired dispatch **BEFORE `Wiring::production`** so the verb is offline/store-free by
  construction. The single text-output implementation greened **PV-1** (walking skeleton),
  **PV-2** (≥10 seeds), **PV-3** (backward-compat dotted-lowercase NSID), **PV-5**
  (text default), **PV-6** (offline / network-disabled, no PDS call). **02-02 (1642a3e)**
  added the **`--json` opt-in** branch (a JSON array of the full records), greening **PV-4**;
  text stays the default and `--json` changes only the emission, not the data source.

The 3-step shape: the **pure core lands first (01-01)** so the vocabulary exists as embedded
constants + a total validator + a deterministic object-id derivation; then a **single text
verb (02-01)** turns five of the six acceptance scenarios green off one implementation
(discovery, count, backward-compat, default, offline all fall out of the one `list` view);
the **`--json` opt-in (02-02)** is a thin serde-serialization addition greened through PV-4.

**Phase-3 refactor — L1-L4 applied (4d9da33)**: collapsed a redundant `OBJECT_ID_PREFIX`
const into the single NSID constant (so the prefix is written once) and fixed a stale doc
comment. All tests green.

**Phase-4 adversarial review — APPROVED**: **0 blockers, 0 Testing Theater, 0 test-integrity
issues**. One non-blocking **test-density note** was raised and **left as-is** — the
property-based-testing density on the pure seams (`validate_philosophy_json`, `normalize`,
`object_id`) is justified by the paradigm mandate (proptest-first for pure seams, ADR-007)
and each property pins a distinct behavior.

**Phase-5 mutation**: the pure core `crates/lexicon/src/philosophy.rs` scored **100%
(12/12 viable, 2 unviable)** after killing **2 genuine `normalize` survivors** (both on
line 128, `ch.is_whitespace() || ch == '_' || ch == '-'`) with an added reference-mapping
property test (bdac50b). The effect shell (cli render/verb) is **10/10 caught** via the
cross-crate acceptance binary. The per-feature gate (≥80% of viable) is **MET**.

**Phase-6 integrity**: all 3 steps have complete DES traces; integrity verification exit 0;
`cargo xtask check-arch` OK (21 workspace members).

## DELIVER-wave decisions

| # | Decision | Why it mattered |
|---|----------|-----------------|
| DV-PV-1 | **Reconcile the stale `Philosophy` record in place to the ADR-059 schema `{name, description, aliases, seeAlso}` — drop the non-schema `id`/`label`; the object id is DERIVED, never stored** (ADR-059 D1). | The scaffold's `{id, label, description}` shape was the RED cause. Storing the id would double-source it and risk drift from the derivation; deriving `object_id(name)` from the record keeps the join to the claim graph a pure function of the name — one source of truth for the bytes slice-01 signs. |
| DV-PV-2 | **Implement `validate_philosophy_json` as a per-field-gated pure validator mirroring `validate_claim_json`, REUSING the existing `LexiconError` — no parallel error type** (ADR-059 D2). | Required `name`/`description` → `MissingField`; array-of-string gates on `aliases`/`seeAlso` → `InvalidType`; serde catch-all → `SchemaMismatch`. Mirroring the claim validator keeps one validation idiom in the crate; reusing `LexiconError` avoids a parallel error taxonomy the callers would have to fork on. |
| DV-PV-3 | **`object_id(name) = "org.openlore.philosophy." + normalize(name)`, byte-compatible with the slice-01 claim object bytes** (ADR-059 D1). | The vocabulary is only useful if a claim authored against a philosophy id joins that philosophy in the graph. Deriving the id as a dotted-lowercase NSID (not `:`-separated / CamelCase) makes the join exact on the bytes the claim already carries — PV-3 guards this against drift. |
| DV-PV-4 | **Ship the 12 seeds as UNSIGNED embedded constants via `include_str!("seeds.json")` + a public `seeds()` accessor — not signed records** (ADR-059 D3). | Minting signed records needs a signer + store (slice-24). The seeds are curated well-known constants; embedding them keeps `list` offline by construction and makes provenance a TYPE (unsigned constant vs signed mint), not a comment. ≥10 satisfies KPI-PV-1. |
| DV-PV-5 | **Dispatch `philosophy list` BEFORE `Wiring::production` is constructed — offline / store-free by construction** (ADR-059 D7). | PV-6 / AC-001.4 require success with the network disabled and no store dependency. Dispatching before the production wiring means the verb *cannot* open a store or make a network call — offline is a structural property of the dispatch order, not a runtime check. The fake PDS is asserted UNUSED. |
| DV-PV-6 | **Text is the DEFAULT; `--json` is strictly opt-in and changes only the emission, not the data source** (AC-001.3, P-001 ux_guardrails). | Greppable human text is the primary discovery affordance (copy an id into a claim); JSON is for tooling. Sharing the seed source between both paths means the two views can never disagree — `--json` re-serializes the same `seeds()` the text path iterates. |
| DV-PV-7 | **NO new crate — the record type + validator + seed data live in `lexicon`; the verb in `cli`** (ADR-059, principle 8). | The vocabulary core is pure data + pure functions (a natural `lexicon` fit); the verb is one CLI dispatch. A seed-data crate would be over-engineering for embedded constants. Workspace stays 21; `check-arch` OK. |
| DV-PV-8 | **Mutation = per-feature: pure core `philosophy.rs` 100% (12/12); the effect-shell cli render/verb 10/10 caught via the acceptance binary (reported, not gated)**. | Follows the established slice-21 DV-NAV-2 pattern — mutate the pure domain core and kill in-crate; treat cross-crate effect-shell mutants as coverage artifacts killed through the real binary. The 2 genuine `normalize` survivors were a real test gap (value-level separator mapping), closed with one reference-mapping property. ≥80%-of-viable gate MET. |
| DV-PV-9 | **Phase-3 refactor: collapse the redundant `OBJECT_ID_PREFIX` const into the single NSID constant; fix a stale doc comment** (4d9da33). | The prefix string was written in two places; a single constant means the NSID form is defined once and the `object_id` derivation and any future consumer read the same source. A cheap L1/L4 that removes a latent divergence. |

## Cardinal release gates + slice-22 invariants (I-PV-1..n)

The cardinal gates realized on the discovery surface — all release-blocking:

1. **Vocabulary exists / valid (CARDINAL, I-PV-1)** — ≥10 seeded records, each a valid
   `{name, description}` record. Three-layer: STRUCTURAL (12 embedded constants) + BEHAVIORAL
   (PV-2 counts ≥10 in output; the in-crate seed-validity test validates every seed through
   `validate_philosophy_json`; KPI-PV-1). DV-PV-4.
2. **Backward-compatible object id (CARDINAL, I-PV-2)** — the derived
   `org.openlore.philosophy.<kebab(name)>` is byte-identical to the slice-01 claim object
   bytes, so the vocabulary joins the claim graph. STRUCTURAL (the pure `object_id`·`normalize`
   derivation, DV-PV-3) + BEHAVIORAL (PV-3 pins the exact dotted-lowercase form). Cardinal.
3. **Offline / local-first (CARDINAL, I-PV-3)** — `philosophy list` reads embedded seeds with
   no store and no network; dispatched before `Wiring::production`. STRUCTURAL (dispatch order,
   DV-PV-5) + BEHAVIORAL (PV-6 succeeds network-disabled; the fake PDS asserted UNUSED). Cardinal.
4. **Validator completes the scaffold, no panic (CARDINAL, I-PV-4)** — `validate_philosophy_json`
   is a total per-field-gated function on `LexiconError` (no `panic!`); the reject arm returns a
   named-field error. STRUCTURAL (per-field gates reusing `LexiconError`, DV-PV-2) + BEHAVIORAL
   (LX-accept + LX-reject `MissingField{description}`). Cardinal.
5. **Text default / JSON opt-in (I-PV-5)** — text is the default view; `--json` is strictly
   opt-in and re-serializes the same seed source. STRUCTURAL (shared `seeds()` source, DV-PV-6)
   + BEHAVIORAL (PV-5 text-default + PV-4 JSON array).
6. **No new crate / workspace stays 21 (I-PV-6)** — the record + validator + seeds in `lexicon`;
   the verb in `cli`; no new crate. STRUCTURAL (`xtask check-arch` reports 21; DV-PV-7).
7. **Provenance is a type (I-PV-7)** — the seeds are explicit UNSIGNED embedded constants;
   signed minting is slice-24. STRUCTURAL (unsigned constants vs signed mints, DV-PV-4).

| # | Invariant | Enforcement |
|---|---|---|
| I-PV-1 | Vocabulary exists / valid (≥10 seeded, each a valid `{name, description}` record). | STRUCTURAL (12 embedded constants) + BEHAVIORAL (PV-2 count; in-crate seed-validity test; KPI-PV-1). Cardinal. |
| I-PV-2 | Backward-compatible object id (`org.openlore.philosophy.<kebab(name)>` = slice-01 claim object bytes). | STRUCTURAL (pure `object_id`·`normalize`, DV-PV-3) + BEHAVIORAL (PV-3). Cardinal. |
| I-PV-3 | Offline / local-first (`list` reads embedded seeds; no store, no network; dispatched before `Wiring::production`). | STRUCTURAL (dispatch order, DV-PV-5) + BEHAVIORAL (PV-6 network-disabled; PDS asserted unused). Cardinal. |
| I-PV-4 | Validator completes the scaffold, no panic (`validate_philosophy_json` total; reject → named field). | STRUCTURAL (per-field gates on `LexiconError`, DV-PV-2) + BEHAVIORAL (LX-accept + LX-reject). Cardinal. |
| I-PV-5 | Text default / JSON opt-in (shared seed source; `--json` re-serializes the same data). | STRUCTURAL (shared `seeds()`, DV-PV-6) + BEHAVIORAL (PV-5 + PV-4). |
| I-PV-6 | No new crate / workspace stays 21. | STRUCTURAL (`xtask check-arch` reports 21; DV-PV-7). |
| I-PV-7 | Provenance is a type (seeds UNSIGNED constants; signed minting is slice-24). | STRUCTURAL (unsigned constants vs signed mints, DV-PV-4). |

slice-22 INHERITS the platform cardinals (claims-not-truth / no-arbiter, local-first / offline,
signed records for minted content, read-only viewer, anti-merging) from the slice-01
foundation; the discovery surface touches none of the write/sign/persist paths — it is a
read-only offline list over embedded constants.

## Quality gates — final report

- **Acceptance / integration**: the `philosophy_vocabulary` acceptance binary **8/8**
  (6 PV-* subprocess scenarios + 2 support-framework self-tests) GREEN, driven through the
  REAL `openlore` bin via `run_openlore` / `run_openlore_network_disabled`. PV-1 is the
  walking skeleton; PV-6 asserts the fake PDS UNUSED with the network disabled.
- **`lexicon` in-crate suite**: **39 passed** — the 2 validator arms (LX-accept, LX-reject)
  + seed-validity / no-normalize-collision + `normalize`/`object_id` property loops.
- **`cargo xtask check-arch`**: **OK (21 workspace members)** — no new crate. `lexicon` purity
  intact (no `std::fs`/`net`/`time` imports; the record / validator / `object_id`·`normalize` /
  seeds are pure; the verb + render live in the `cli` effect shell).
- **Refactor (L1-L4)**: clippy + check-arch clean; **Phase-3 refactor applied** — the
  redundant `OBJECT_ID_PREFIX` const collapsed into the single NSID constant + a stale doc
  comment fixed (4d9da33).
- **Adversarial review (Phase 4)**: **APPROVED**, **0 blockers, 0 Testing Theater,
  0 test-integrity issues**. One non-blocking test-density note on the pure-seam PBT density —
  **left as-is** (justified by the ADR-007 proptest-first mandate; each property pins a
  distinct behavior).
- **DES integrity**: PASS — all 3 steps have complete DES traces (**3/3**); integrity
  verification exit 0.

## Mutation testing — final report

**Tool**: cargo-mutants 25.3.1. **Scope** (feature-scoped, per the slice-21 DV-NAV-2 pattern):
mutate the **pure domain core** `crates/lexicon/src/philosophy.rs`, kill with in-crate feature
tests; treat cross-crate effect-shell mutants (cli render/verb) as coverage artifacts killable
only through the acceptance binary — reported, not gated.

| Mutant surface | Viable | Caught | Missed | Kill rate |
|---|---:|---:|---:|---|
| Pure core `lexicon/src/philosophy.rs` (validator + `normalize`/`object_id` + seed access) | 12 | 12 | 0 | **100% (12/12)** |
| Effect shell `cli` (`render/philosophy.rs`, `verbs/philosophy_list.rs`) | 10 | 10 | 0 | killed via the acceptance binary (reported, not gated) |

**Mutation note (precise)**: the initial pure-core run scored **10/12 (83.3%)** — already
passing — with **2 genuine survivors** in `normalize` (line 128,
`ch.is_whitespace() || ch == '_' || ch == '-'`): one replaced `||` with `&&` (making the
whitespace/underscore conjunct unsatisfiable, so only `'-'` survives as a separator); the
other flipped `==` to `!=` (mis-classifying `'_'`). **Classification: genuine test gap** —
the existing `normalize` tests pinned only *structural* invariants (output charset `[a-z0-9-]`,
no boundary/doubled dash, idempotence, NSID prefix) plus one exact assertion on an
already-kebab input; **none pinned the exact normalized VALUE** for an input containing
whitespace, `'_'`, or other punctuation, so a mutant that mis-classified separators still
produced kebab-shaped output. Closed with one added in-crate **reference-mapping property**
(`normalize_maps_separators_and_punctuation_to_exact_kebab`, bdac50b) pinning exact
`(input → expected)` pairs (`"Memory Safety"`/`"memory_safety"`/`"  Memory   Safety  "` →
`"memory-safety"`; `"test.driven"` → `"testdriven"`; `"C++ style"` → `"c-style"`) — a
hand-rolled table staying within the pure-crate dependency envelope (the documented ADR-059
fallback where a proptest dev-dependency is unavailable). Re-run: **12/12**. The **10 effect-shell
mutants** (function-replacement on `render_philosophy_list` / `render_seed_block` and the
verb `run`'s `(i32, String)` result) were **100% caught via the cross-crate acceptance layer**
(the real `openlore` subprocess exercised by the 8-test `philosophy_vocabulary` suite) —
coverage artifacts that happen to carry no genuine survivor. The per-feature gate
(≥80% of viable) is **MET** (pure core 100%; 0 equivalent mutants; 0 unresolved cross-crate
artifacts). Working tree restored clean; `mutants.out*` scratch dirs removed. Full report:
`deliver/mutation/mutation-report.md`.

## Lessons learned / issues

- **A pure normalizer needs value-level reference tests, not just structural invariants
  (DV-PV-8)**: the `normalize` survivors slipped through every structural property (charset,
  no-doubled-dash, idempotence, prefix) because a mis-classified separator still yields
  kebab-SHAPED output — the shape invariants can't distinguish `"memorysafety"` from
  `"memory-safety"`. **Lesson: when a pure transform maps a rich input space to a constrained
  output space, structural invariants on the output charset/shape are necessary but not
  sufficient — pin a handful of exact `(input → expected)` reference pairs covering each
  separator/punctuation class, or a mutant that mis-routes a class survives with plausible
  output.**
- **Offline-by-construction beats offline-by-check (DV-PV-5)**: dispatching `philosophy list`
  BEFORE `Wiring::production` means the verb *cannot* touch a store or the network — there is
  no handle to misuse. This is stronger than asserting "no network call happened" at runtime.
  **Lesson: when a read-only verb must be offline, place its dispatch before the effectful
  wiring is even constructed — the offline guarantee becomes a structural property of the
  composition root, and PV-6's network-disabled assertion becomes a confirmation of structure
  rather than the only line of defense.**
- **Completing a RED scaffold is reconciliation, not greenfield (DV-PV-1/DV-PV-2)**: the
  stale `{id, label, description}` struct + panicking validator were left deliberately by
  slice-01; DISTILL's reconciliation gate correctly classified them as the RED cause, not a
  contradiction. The DELIVER work was to reshape the existing type and fill the existing
  signature — reusing `LexiconError` and mirroring `validate_claim_json` — not to invent new
  machinery. **Lesson: when a prior slice leaves a typed RED scaffold, treat completion as
  reconciling to the DESIGN schema + filling the frozen signature; reuse the sibling
  validator's idiom and error type so the crate keeps one validation model.**
- **A thin pure core makes most acceptance scenarios confirmatory (DV-PV-6)**: once the seeds
  + `object_id` + the text `list` view existed (01-01 + 02-01), five of six PV scenarios
  (discovery, count, backward-compat, default, offline) fell out of the one implementation;
  only `--json` (PV-4) needed a distinct step. **Lesson: when a slice's value is a read-only
  projection of pure data, get the data + the one primary view right and most scenarios become
  confirmations of the projection — the only genuinely-separate work is an alternate emission
  format.**
- **[PRE-EXISTING REGRESSION — flagged for follow-up, NOT caused by slice-22]**: 2 acceptance
  tests in `tests/acceptance/viewer_graph_traversal.rs`
  (`a_claim_less_philosophy_renders_the_guided_no_claims_state` and
  `a_claim_less_project_renders_the_guided_no_claims_state_not_a_crash`) **fail on `main`**.
  They stem from the **slice-21** (viewer-persistent-left-nav) feature adding a `/project` nav
  href that trips the older **I-GT-4** "no fabricated traversal edge" guard. **slice-22 touched
  ZERO viewer files** — its diff is confined to `crates/lexicon/*` +
  `crates/cli/{lib.rs, render.rs, render/philosophy.rs, verbs/mod.rs, verbs/philosophy_list.rs}`
  — so it is categorically not the cause. **Recorded here as a flagged pre-existing regression
  for a dedicated follow-up fix** (reconcile the slice-21 `/project` nav entry with the I-GT-4
  traversal-edge guard); it was out of scope for slice-22 and ignored per the task boundary.

## Deviations: planned (DESIGN) vs shipped

| # | Planned at DESIGN (ADR-059) | Shipped state | Disposition |
|---|-----------------------------|---------------|-------------|
| 1 | D1: reconcile `Philosophy` to `{name, description, aliases, seeAlso}`; object id DERIVED. | Shipped exactly — `id`/`label` dropped; `object_id(name)` derived, never stored. | Resolved at DELIVER (DV-PV-1). |
| 2 | D2: `validate_philosophy_json` per-field-gated, mirroring `validate_claim_json`, reusing `LexiconError`. | Shipped — required→`MissingField`, array-type→`InvalidType`, serde catch-all→`SchemaMismatch`; no parallel error type. | Resolved at DELIVER (DV-PV-2). |
| 3 | D1: object id byte-compatible with slice-01 claim object bytes. | Shipped — `org.openlore.philosophy.<kebab(name)>`, dotted-lowercase; PV-3 pins the exact form. | Resolved at DELIVER (DV-PV-3). |
| 4 | D3: ≥10 embedded unsigned seed constants via `include_str!`. | Shipped **12** seeds (the six hard-pinned + six more) behind a public `seeds()` accessor. | Resolved at DELIVER (DV-PV-4); exceeds the ≥10 floor. |
| 5 | D7: offline `philosophy list` verb (text default, `--json` opt-in). | Shipped — dispatched before `Wiring::production`; text default; `--json` array. | Resolved at DELIVER (DV-PV-5/6). |
| 6 | DESIGN VocabularyIndex / alias resolution / minting / viewer surface scoped to LATER slices. | NOT built in slice-22 (correctly OUT of scope — slices 24/26/27). | Deferred by plan; no deviation. |
| 7 | No new crate; workspace stays 21. | Shipped exactly — record/validator/seeds in `lexicon`, verb in `cli`; `check-arch` OK (21). | Resolved at DELIVER (DV-PV-7). |
| 8 | Mutation expected strong on the pure core. | Pure core 100% (12/12) after killing 2 genuine `normalize` survivors; effect shell 10/10 via the acceptance binary. ≥80% gate MET. | Recorded; the survivor gap explained + closed (DV-PV-8). |
| 9 | Review expected to pass clean. | Review APPROVED — 0 blockers, 0 Testing Theater, 0 test-integrity issues; one non-blocking density note left as-is. | Confirmed at DELIVER. |

## KPI status

- **J-002** ("classify against a *shared* vocabulary so my claims triangulate" — the
  discovery aspect): realized in its discovery form — a user can list the shared vocabulary
  and copy an exact object id into a claim instead of inventing a private string. (Full
  triangulation via aliases is slice-26.)
- **US-PV-001** (discover the shared philosophy vocabulary): **SHIPPED** — `openlore
  philosophy list` prints each seed's object id + name + description (text) or a `--json`
  array, offline.
- **KPI-PV-1 — Vocabulary exists** (≥10 seeded, valid records): **MET** — 12 seeds, each
  validates through `validate_philosophy_json`, all with distinct object ids (no-collision
  test).
- **KPI-PV-2 — Discoverable** (list returns the seeds LOCAL/offline): partially realized —
  `list` is shipped and offline (PV-6); `show` is slice-23. **KPI-PV-3..6** (open extension /
  advisory compose / triangulation / no-drift) belong to slices 24–28, OUT of scope for
  slice-22.
- **Dogfood moment (same day)**: `./cli.sh philosophy list` shows the vocabulary; copy an id
  into `./cli.sh claim add --object org.openlore.philosophy.memory-safety …`. The offline
  read-only verb emits no telemetry; these KPIs are verified structurally + by the acceptance
  corpus, not by runtime metrics.

## Pointers

- **Feature workspace** (DISCUSS through DELIVER, all detail — PRESERVED):
  `docs/feature/philosophy-vocabulary-registry/` — the single-narrative `feature-delta.md`
  (DISCUSS/DISTILL [REF] sections), `design/architecture-design.md`,
  `distill/red-classification.md`, `slices/`, `deliver/` (roadmap.json, execution-log.json,
  mutation/mutation-report.md).
- **Slice-22 ADR**:
  `docs/adrs/ADR-059-philosophy-vocabulary-registry-record-reconciliation-embedded-seeds-signed-mints-read-time-alias-resolution.md`
- **Architecture design / C4 / component boundaries / per-slice seam map**:
  `docs/feature/philosophy-vocabulary-registry/design/architecture-design.md`
- **DELIVER execution log + roadmap + mutation report**:
  `docs/feature/philosophy-vocabulary-registry/deliver/execution-log.json`,
  `docs/feature/philosophy-vocabulary-registry/deliver/roadmap.json`,
  `docs/feature/philosophy-vocabulary-registry/deliver/mutation/mutation-report.md`
- **RED classification (DISTILL gate)**:
  `docs/feature/philosophy-vocabulary-registry/distill/red-classification.md`
- **Slice brief**:
  `docs/feature/philosophy-vocabulary-registry/slices/slice-22-philosophy-registry-seed-and-list.md`
- **Acceptance corpus (executable SSOT)**: `tests/acceptance/philosophy_vocabulary.rs`
  (PV-1..PV-6, the walking skeleton at PV-1) + the in-crate validator arms in
  `crates/lexicon/src/lib.rs` (LX-accept, LX-reject).
- **Pure vocabulary core (this slice)**: `crates/lexicon/src/philosophy.rs` (the reconciled
  `Philosophy` record, `validate_philosophy_json`, `normalize`/`object_id`),
  `crates/lexicon/src/seeds.json` (the 12 embedded seeds) + `seeds()` accessor,
  `crates/lexicon/src/lib.rs` (the validator arms + property tests).
- **Discovery verb (this slice)**: `crates/cli/src/lib.rs` (`Command::Philosophy`, dispatch
  before `Wiring::production`), `crates/cli/src/verbs/philosophy_list.rs`,
  `crates/cli/src/render/philosophy.rs`, `crates/cli/src/{render.rs, verbs/mod.rs}`.
- **Record schema (Lexicon)**: `lexicons/org/openlore/philosophy.json`.
- **Reference-class slice**: slice-01 `openlore-foundation` (claim record + validation + CLI
  verb — the shape this slice mirrors).
- **Next slices (OUT of scope)**: slice-23 show, slice-24 mint/add, slice-25 compose-advisory,
  slice-26 alias triangulation, slice-27 viewer surface, slice-28 scraper single-source.
- **Prior evolution archive** (the immediately-prior slice-21 — the persistent left nav; the
  source of the flagged `/project` nav pre-existing regression):
  `docs/evolution/viewer-persistent-left-nav-evolution.md`
- **Paradigm**: `docs/adrs/ADR-007-paradigm-functional-rust.md` (functional Rust — pure
  vocabulary core, effect shell at the CLI edge).

## Commit trail

DISCUSS 98e8eaa → DESIGN 957f2f6 (ADR-059) → DISTILL 364f650 (8 RED scaffolds: seed + list) →
roadmap APPROVED (3 steps, ratio 0.5, linear DAG 01-01 → 02-01 → 02-02) →
01-01 6406cc2 (pure lexicon core: record reconcile + `validate_philosophy_json` +
`normalize`/`object_id` + 12 seeds) →
02-01 3f00077 (`philosophy list` offline human-text discovery — PV-1/2/3/5/6) →
02-02 1642a3e (`--json` opt-in array — PV-4) →
4d9da33 (Phase-3 refactor L1-L4: collapse redundant `OBJECT_ID_PREFIX`; fix stale doc) →
bdac50b (mutation-gate: kill the 2 genuine `normalize` survivors with a reference-mapping
property). All on `main` (trunk-based, no PR).
</content>
