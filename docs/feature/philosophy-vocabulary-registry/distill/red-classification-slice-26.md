<!-- markdownlint-disable MD013 -->
# RED Classification — slice-26 (philosophy-alias-triangulation)

> DISTILL Pre-DELIVER fail-for-the-right-reason gate (nw-distill §"Pre-DELIVER
> fail-for-the-right-reason gate"). Every slice-26 acceptance scenario was run
> once against the CURRENT (unimplemented) production code and classified.
> DELIVER reads this file at the RED-phase entry gate (ADR-025 D2) to confirm RED
> is genuine.
>
> Owner: Quinn (nw-acceptance-designer) · 2026-07-08 · Rust / cucumber-free
> subprocess acceptance shape (mirrors slice-25 `claim_compose_advisory.rs` +
> slice-04 `graph_query_explore.rs`).
> Scope: US-PV-005 (AC-005.1..2) — ALIAS TRIANGULATION at READ time in
> `graph query --object` + `--weighted` (job_id J-002 / J-004; ADR-059 §5 row 26).
> `graph query` already reads own ∪ peer claims by exact object (slice-04,
> SHIPPED). Slice-26 widens the object read to the philosophy's EQUIVALENCE CLASS
> so near-synonyms triangulate, WITHOUT rewriting the stored objects (AC-005.2).
> Slices 22 (seed+list), 23 (show), 24 (mint), 25 (compose advisory), 28 (scraper)
> are SHIPPED. Minted-philosophy aliases (a `philosophies`-table read) are OUT —
> SEED-alias equivalence only.

## Wave-decision reconciliation

The feature uses the single `docs/feature/philosophy-vocabulary-registry/feature-delta.md`
SSOT — there are no separate `discuss/`, `design/`, `devops/` `wave-decisions.md`
files to cross-check. AC-005.1..2 (feature-delta.md US-PV-005, lines 119–120) and
the DESIGN row 26 ("lexicon (`equivalence_class`); `adapter-duckdb::store_read`
(widen `query_philosophy_survey` filter to the class); `scoring` (group under
canonical) | Read-time derivation only; stored objects immutable (AC-005.2);
UNION-ALL still projects `author_did` (anti-merging)") agree with each other and
with the slice brief. **Reconciliation passed — 0 contradictions.**

## How the run was performed

```
cargo build --bin openlore                                                 # build-before-run (the AT spawns the real bin)
cargo test -p cli --test philosophy_alias_triangulation --no-run           # COMPILE gate (BROKEN check)
cargo test -p cli --test philosophy_alias_triangulation -- --test-threads=1
```

The acceptance target COMPILES green (`--no-run` → `Finished`; the 15 warnings
are all from the shared `support` harness — unused imports / unreachable match
arms — NONE from `philosophy_alias_triangulation.rs`). It spawns the real
`openlore` bin via the existing `run_openlore*` support harness and imports only
that harness (`mod support; use support::*`) plus `std::path` — NO new production
symbol, NO typed deserialization into a `claim_domain`/`lexicon` struct (the
persisted-artifact assertions read the JSON as TEXT). Therefore every acceptance
failure is a RUNTIME assertion against the observable CLI surface, not a
compile / import error → RED, never BROKEN.

The triangulation pair is seeded through the PRODUCTION write paths (no new
external fake): the LOCAL user's OWN claim on the alias object via the real
`claim add` verb, and a distinct PEER's claim on the canonical object via the real
`peer add` + `peer pull` verbs against a `PeerPds` double built with the slice-04
`build_verifiable_peer_records_for_triples` seam (REAL Ed25519 + CID recompute —
the production pull verifies it). These are the SAME public support primitives
`seed_federated_graph` uses internally; the memory-safety/mem-safety pair is
composed in the test file because the frozen harness's `FederatedGraphFixture`
variants hard-pin OTHER objects.

A `[[test]]` target `philosophy_alias_triangulation` was added to
`crates/cli/Cargo.toml` (mirroring the `claim_compose_advisory` / `philosophy_add`
entries) so the workspace-root `tests/acceptance/philosophy_alias_triangulation.rs`
is discoverable — the only build-config change. No new crate; the workspace stays
at 21 members.

## What is missing today (the RED cause)

- **The CLI `graph query` object read filters `object` by EXACT string match.**
  `graph query --object <canonical>` already reads own ∪ peer claims (slice-04
  SHIPPED), but the plain dimension read routes through
  `adapter-duckdb::graph_query::query_by_object` (own ∪ peer `UNION ALL WHERE
  object = ?`) and the `--weighted` read through the scoring `ByObject` filter —
  BOTH exact-match on the `object` column. So a claim authored on the ALIAS
  `org.openlore.philosophy.mem-safety` is INVISIBLE to a query for the CANONICAL
  `org.openlore.philosophy.memory-safety`. The pure alias-widening seam
  (`lexicon::equivalence_class` over `seeds()` + each seed's `aliases`) does not
  exist yet.
  - **Observed** `graph query --object …memory-safety` output today: a single
    group `subject: github:denoland/deno` / `author_did: did:plc:rachel-test
    (subscribed peer)` / one cid, footer `1 subject(s), 1 author(s).` — the
    peer's exact canonical match ONLY. The local user's alias-object claim
    (subject `github:rust-lang/rust`, "(you)") is absent.
  - **AT-1** (walking skeleton) asserts the alias-authored claim IS included +
    attributed (local DID present, its subject present, cid_rows == 2). All three
    RED assertions fail on the missing alias claim → MISSING_FUNCTIONALITY. Panic
    fires at the first (local-DID) assertion (line 369).
  - **AT-2** (anti-merging) asserts both authors present on two distinct-cid rows.
    Today only the peer's row exists → the local-author assertion fails (line 437)
    → MISSING_FUNCTIONALITY. (The no-merge scan itself would pass — there is
    nothing to merge yet; the RED driver is the ABSENT second attributed row.)
  - **AT-3** (`--weighted`) asserts the alias claim's project is aggregated under
    the canonical. The observed weighted view ranks ONLY `github:denoland/deno`
    (`weight 0.88 [SPARSE]`, `claims: 1 authors: 1`); `github:rust-lang/rust` is
    absent → the aggregation-inclusion assertion fails (line 506) →
    MISSING_FUNCTIONALITY.
- **AT-4 (immutability, LOAD-BEARING) is GREEN-today** — after a triangulated
  read, the stored alias-object claim's `<cid>.json` still carries
  `org.openlore.philosophy.mem-safety` verbatim and NOT the canonical (claim add
  signs the object verbatim; the read is display-only). This PINS the AC-005.2
  immutability invariant so DELIVER's read-time widening cannot corrupt the signed
  bytes. Passes today; must STAY green.
- **AT-5 (no-regression / singleton) is GREEN-today** — a query for
  `…dependency-pinning` (a DISTINCT equivalence class) returns only its exact match
  and does NOT leak the `memory-safety` class (cid_rows == 1). The exact-match read
  already excludes the other class; DELIVER must keep it so (an over-widening
  `equivalence_class` that crossed classes or returned all objects would red it).

## Classification key

- **RED (MISSING_FUNCTIONALITY, assertion)** ✅ — the assertion fires because
  read-time alias triangulation (the `equivalence_class`-widened object read that
  makes near-synonyms connect + aggregate) is unimplemented. Correct RED.
- **GREEN-today (no-regression / invariant guardrail)** 🟢 — passes against today's
  code and must STAY green after DELIVER (an over-widening or a payload rewrite
  would red it). Not a failure; not a blocker.
- **BROKEN / SETUP / IMPORT** ❌ — would block handoff. **NONE remain.**

## Tally

| File | Scenario | AC | Panic line | Classification | Why |
|---|---|---|---|---|---|
| `philosophy_alias_triangulation.rs` | AT-1 `graph_query_by_canonical_object_includes_alias_authored_claim_attributed` (WS) | AC-005.1 | 369 | RED ✅ | peer exact canonical claim surfaces (green sanity); the local alias-object claim is excluded by today's exact-`object` read → the include/attribute assertion fails |
| | AT-2 `graph_query_triangulated_claims_stay_two_attributed_rows_unmerged` | AC-005.1 anti-merging | 437 | RED ✅ | only the peer's row exists today; the second attributed author row (the alias claim) is absent → the both-authors / two-cid assertion fails |
| | AT-3 `graph_query_weighted_over_canonical_aggregates_alias_authored_claim` | AC-005.1 (--weighted) | 506 | RED ✅ | the weighted view ranks only the canonical claim's project today; the alias claim's project is not aggregated → the aggregation-inclusion assertion fails |
| | AT-4 `alias_triangulation_never_rewrites_the_stored_object_bytes` | AC-005.2 | — | GREEN-today 🟢 | the stored alias object stays `…mem-safety` verbatim after a triangulated read (nothing rewrites it); DELIVER must keep it immutable (read-time-only guard) |
| | AT-5 `graph_query_other_class_object_returns_only_its_exact_matches` | AC-005.1 no-regression | — | GREEN-today 🟢 | a query for a DISTINCT class returns only its exact match; the memory-safety class does not leak (over-widening guard) |

### Numeric summary (slice-26 scenarios only; excludes the 2 pre-existing `support::state_delta` framework self-tests bundled in the acceptance binary)

| Classification | Count |
|---|---|
| RED — MISSING_FUNCTIONALITY (assertion; alias-object claim excluded by today's exact-match read) | 3 |
| GREEN-today (immutability invariant + no-over-widening guardrail) | 2 |
| **BROKEN / SETUP / IMPORT** | **0** |
| **Total slice-26 tests** | **5** |

RED total = **3**, all assertion-RED. Two GREEN-today guardrails (AT-4 immutability,
AT-5 no-regression). Zero BROKEN. Observed runner output:
`test result: FAILED. 4 passed; 3 failed` — the 4 passes are AT-4 + AT-5 plus the 2
`support::state_delta::tests::*` framework self-tests (present in every acceptance
binary), NOT slice-26 RED scenarios; the three RED functions appear in the
`failures:` list at lines 369 / 437 / 506.

## Gate verdict

**PASS.** Every failing test fails for the RIGHT reason (MISSING_FUNCTIONALITY —
read-time alias triangulation: the `equivalence_class`-widened object read that
makes near-synonym claims connect + aggregate under the canonical does not exist
yet; today's CLI `graph query` read filters `object` exactly). Zero tests are in
category 2 (IMPORT_ERROR / FIXTURE_BROKEN / SETUP_FAILURE) or category 3
(WRONG_ASSERTION / internal-struct coupling — every assertion scans the OBSERVABLE
CLI stdout / exit code, or reads the on-disk signed artifact as TEXT, never a
`claim_domain` / `lexicon` struct field). The two GREEN-today scenarios (AT-4
immutability, AT-5 no-over-widening) are intentional invariant guardrails, not
failures. Handoff to DELIVER is UNBLOCKED for slice-26.

## Error/edge ratio note

5 scenarios: AT-1 (WS happy triangulation) = 1 pure-happy; AT-2 (anti-merging
edge) + AT-3 (`--weighted` aggregation edge) + AT-4 (immutability invariant guard)
+ AT-5 (no-over-widening / singleton regression edge) = 4 non-pure-happy =
**80%** (≥40% target). The two hard invariants of the payoff slice (AC-005.1
anti-merging, AC-005.2 immutability) and the over-widening boundary are covered
explicitly by AT-2 / AT-4 / AT-5, example-based per Mandate 11 (no PBT at layer 3 —
the pure `equivalence_class` is property-tested at layer 1 in `crates/lexicon` by
DELIVER: every seed name AND every alias maps to the same class; an unknown /
non-philosophy object maps to the singleton `[object]`; arbitrary input never
panics).

## Outcomes-registry note

Skipped — `docs/product/outcomes/registry.yaml` does not exist and the prior
philosophy slices (22 seed+list, 23 show, 24 mint, 25 compose advisory) registered
no OUT-N rows. Following that precedent, no outcome is registered for the alias-
triangulation read. If the registry is later adopted for this feature, register
`lexicon::equivalence_class` as a `kind: specification` (a pure read-time widening
rule) and the widened object read as a `kind: operation` at that time.

## DELIVER pointers (from the observed RED)

1. **Add a pure `equivalence_class` seam to `crates/lexicon`** (ADR-059 §5 row 26).
   Over `seeds()`: given any object in a seed philosophy's class (the canonical
   object-id OR any of its `aliases`), return ALL object-ids in the class
   (`object_id(name)` + one per alias); given an unknown / non-philosophy object,
   return the SINGLETON `[object]` (the no-op that keeps AT-5 green — no
   regression, no cross-class leak). Reuse `seeds()`, `normalize()`, `object_id()`,
   and the slice-25 `resolve_object_advisory` machinery. Pure + total; property-
   test at layer 1 (every seed name AND every alias resolves to the SAME class;
   an unknown / out-of-namespace object → the singleton; arbitrary input never
   panics).
2. **Widen the CLI object read to the equivalence class** — the read path the
   `graph query --object` / `--weighted` DRIVING PORT actually uses. See the
   upstream gap below: today that is `adapter-duckdb::graph_query::query_by_object`
   (plain `--object`) + the scoring `ByObject` filter (`--weighted`), which filter
   `object = ?` exactly. Widen the filter to `object IN (<equivalence class>)`
   (still a UNION-ALL over own ∪ peer projecting `author_did` per row — anti-
   merging, AT-2; `xtask check-arch::no_cross_table_join_elides_author` still
   holds). Turns AT-1 + AT-2 GREEN.
3. **Group the widened rows under the canonical philosophy in `scoring` / render**
   — the display concern (DESIGN row 26 "scoring (group under canonical)"). Because
   the widened read already returns the alias-object rows tagged with their own
   object, feeding them into `scoring::score` aggregates them automatically; the
   scoring/render change is grouping the result under the canonical, not a second
   read. Turns AT-3 GREEN (the alias claim's project enters the weighted
   aggregation).
4. **Keep it a read-time derivation only.** The widening feeds ONLY the read/
   aggregate; `claim add` / `peer pull` continue to persist `composed.object`
   verbatim (AC-005.2 / AT-4). No write path, no migration, no object rewrite.
   `xtask check-arch` stays 21 members / no new crate (one lexicon fn + a widened
   filter + a grouping tweak).

## Upstream gaps for DELIVER to resolve

- **Read-path discrepancy — the DESIGN row 26 names the VIEWER's survey, but the
  CLI `graph query` driving port uses a DIFFERENT read (FLAG).** DESIGN row 26
  says "widen `adapter-duckdb::store_read`'s `query_philosophy_survey` filter". But
  `query_philosophy_survey` is the VIEWER's `/philosophy` route read
  (`crates/adapter-http-viewer/src/lib.rs`), NOT the CLI's. The CLI `graph query
  --object` routes through `adapter-duckdb::graph_query::query_by_object`
  (`run_object_dimension`), and `--weighted` through the scoring `ByObject` filter
  (`run_weighted_object`) — both filter `object` exactly and both are what these
  ATs drive. **DELIVER must widen the read path the CLI actually uses**, not (only)
  `query_philosophy_survey`. For FULL US-PV-005 coverage the viewer's
  `query_philosophy_survey` almost certainly needs the SAME widening (its
  `/philosophy` route triangulates too), so the cleanest fix is a shared widened
  object filter applied to BOTH the CLI `query_by_object` + scoring reads AND the
  viewer `query_philosophy_survey`, all fed by the one pure `equivalence_class`.
  These ATs pin only the CLI observable; a companion viewer AT (or a note that the
  viewer widening ships in the same slice) closes the gap.
- **Attribution + immutability are observable at the CLI/artifact port (confirmed —
  NOT coupled to internal structs).** Anti-merging (AT-2) is asserted on the
  OBSERVABLE `graph query` stdout — the per-author `author_did:` rows + the count
  of `cid:` field lines + the no-merge footer scan — exactly the slice-04
  `graph_query_explore.rs` style; no `SurveyRow`/`AttributedClaim` field is read.
  Immutability (AT-4, AC-005.2) is asserted on the on-disk signed `<cid>.json` read
  as TEXT (the alias object stays `…mem-safety`, never the canonical) — no serde
  into a `claim_domain` struct. Both survive a DELIVER refactor of the internal
  read/grouping types.
- **`--score` vs `--weighted` (confirmed) — the widening lives in the READ filter,
  not in `scoring`.** AT-3 drives `--weighted` (the CLI's scored view). Because the
  weighted view feeds the SAME object-filtered survey into the pure
  `scoring::score`, widening the READ filter to the equivalence class is sufficient
  to make the alias claim flow into the aggregation — `scoring` needs no
  equivalence knowledge, only the DISPLAY grouping under the canonical. i.e. the
  INCLUSION mechanism is the store read; `scoring`'s change is presentational
  (group-under-canonical), matching DESIGN row 26's split. (`--score` is an alias
  for the same scored surface; if a distinct `--score` flag exists it shares the
  `run_weighted_object` / scoring `ByObject` path and inherits the widening for
  free.)
- **How the two claims are seeded (confirmed).** Own claim on the ALIAS object via
  the real `claim add` verb (author "(you)"); a distinct-author PEER claim on the
  CANONICAL object via the real `peer add` + `peer pull` verbs
  (`build_verifiable_peer_records_for_triples` — REAL Ed25519 + CID recompute).
  This is the SAME production seam `seed_federated_graph` uses; it yields two
  DIFFERENT authors + distinct CIDs (the anti-merging precondition) and a real
  on-disk own artifact (the immutability observable). The frozen `support/mod.rs`
  is NOT modified — the memory-safety/mem-safety pair is composed in the test file
  from the PUBLIC support primitives (the private `FederatedGraphFixture` variants
  hard-pin other objects).
