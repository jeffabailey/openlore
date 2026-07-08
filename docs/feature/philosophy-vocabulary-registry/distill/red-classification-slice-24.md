<!-- markdownlint-disable MD013 -->
# RED Classification — slice-24 (philosophy-mint)

> DISTILL Pre-DELIVER fail-for-the-right-reason gate (nw-distill §"Pre-DELIVER
> fail-for-the-right-reason gate"). Every slice-24 acceptance scenario was run
> once against the CURRENT (unimplemented) production code and classified.
> DELIVER reads this file at the RED-phase entry gate (ADR-025 D2) to confirm RED
> is genuine.
>
> Owner: Quinn (nw-acceptance-designer) · 2026-07-08 · Rust / cucumber-free
> subprocess acceptance shape (mirrors slice-23 `philosophy_show.rs`).
> Scope: US-PV-003 (AC-003.1..4) — the `openlore philosophy add` compose → SIGN →
> persist verb ONLY (job_id J-001). This is the FIRST philosophy slice that
> WRITES and SIGNS. Slices 22 (seed + list) and 23 (show) are SHIPPED (read-only);
> slice-28 (scraper) is SHIPPED. Compose-advisory / alias / viewer are OUT.

## How the run was performed

```
cargo build --bin openlore                                 # build-before-run (the AT spawns the real bin)
cargo test -p cli --test philosophy_add --no-run           # COMPILE gate (BROKEN check)
cargo test -p cli --test philosophy_add -- --test-threads=1
```

The acceptance target COMPILES green (`--no-run` → `Finished`; warnings are all
from the shared `support` harness — unused imports/variables — none from
`philosophy_add.rs`). It spawns the real `openlore` bin via the existing
`run_openlore` / `run_openlore_with_stdin` support harness and imports only that
harness (`mod support; use support::*`) plus `std::path` — NO new production
symbol, NO typed deserialization into a `lexicon` struct (the persisted-artifact
assertions read the JSON as TEXT). Therefore every acceptance failure is a
RUNTIME assertion, not a compile/import error → RED, never BROKEN.

A `[[test]]` target `philosophy_add` was added to `crates/cli/Cargo.toml`
(mirroring the `philosophy_show` entry) so the workspace-root
`tests/acceptance/philosophy_add.rs` is discoverable — the only build-config
change. No new crate; the workspace stays at 21 members.

## What is missing today (the RED cause)

- **No `philosophy add` subcommand.** Slice-22 shipped the `philosophy` parent
  verb (with `list`) and slice-23 added `show`, so `openlore philosophy add …` is
  rejected by clap at the SUBcommand level: `error: unrecognized subcommand
  'add'` → **exit 2** (observed `left: 2, right: 0`).
  - **PA-1 / PA-2 / PA-5** assert `status == 0` FIRST → all fail on that
    assertion (exit 2) → MISSING_FUNCTIONALITY (no `add` verb; no compose / sign /
    persist; no printed object id; no signed artifact).
  - **PA-3** (seed-collision sad path) asserts `status != 0` FIRST — that PASSES
    (exit 2 is non-zero) and the no-panic-marker guard PASSES (clap's usage text
    carries no panic / backtrace markers), but the next assertion — the combined
    output must NAME the collision (`memory-safety`) + hint `--alias` — FAILS,
    because clap emitted only `unrecognized subcommand 'add'`. So PA-3 is RED
    because the seed-collision guard + guidance does not exist yet →
    MISSING_FUNCTIONALITY.
  - **PA-4** (empty-description invalid path) asserts `status != 0` FIRST (PASSES,
    exit 2) and the no-panic guard PASSES, but the next assertion — the error must
    NAME the `description` field — FAILS, because clap's `unrecognized subcommand`
    text does not mention `description`. So PA-4 is RED because the
    `validate_philosophy_json` empty-description rejection does not exist yet →
    MISSING_FUNCTIONALITY.

## Classification key

- **RED (MISSING_FUNCTIONALITY, assertion)** ✅ — the assertion fires because the
  `philosophy add` verb (compose / sign / persist, its seed-collision guard, and
  its empty-description rejection) is unimplemented (clap exit 2). Correct RED.
- **BROKEN / SETUP / IMPORT** ❌ — would block handoff. **NONE remain.**

## Tally

| File | Scenario | AC | Line | Classification | Why |
|---|---|---|---|---|---|
| `philosophy_add.rs` | PA-1 `philosophy_add_mints_signs_and_persists_a_new_record_printing_the_object_id` (WS) | AC-003.1 / .2 | 185 | RED ✅ | `unrecognized subcommand 'add'` → exit 2; `status == 0` assertion fails; no verb, no object id, no signed artifact |
| | PA-2 `philosophy_add_with_empty_stdin_cancels_cleanly_without_writing` | AC-003.2 | 236 | RED ✅ | same — exit 2 (not the clean-cancel 0); no compose preview / no local-first cancel path |
| | PA-3 `philosophy_add_refuses_a_name_that_collides_with_a_seed` | AC-003.3 | 320 | RED ✅ | exit 2 is non-zero (status guard passes) + no-panic passes, but the collision guidance naming `memory-safety` + hinting `--alias` is absent → guidance-substring assertion fails |
| | PA-4 `philosophy_add_empty_description_is_rejected_with_a_named_field_error` | AC-003.4 | 403 | RED ✅ | exit 2 is non-zero + no-panic passes, but the named-field error mentioning `description` is absent → field-name assertion fails |
| | PA-5 `philosophy_add_records_the_author_did_in_the_signed_artifact` | AC-003.2 | 437 | RED ✅ | same as PA-1 — exit 2; `status == 0` assertion fails; no signed artifact to carry the author DID |

### Numeric summary (slice-24 scenarios only; excludes the 2 pre-existing `support::state_delta` framework self-tests bundled in the acceptance binary)

| Classification | Count |
|---|---|
| RED — MISSING_FUNCTIONALITY (assertion, clap exit 2 / absent guidance / absent field error) | 5 |
| GREEN-today (no-regression guardrail) | 0 |
| **BROKEN / SETUP / IMPORT** | **0** |
| **Total slice-24 tests** | **5** |

RED total = **5**, all assertion-RED. Zero GREEN-today, zero BROKEN. Observed
runner output: `running 7 tests … 2 passed; 5 failed` — the 2 passes are the
`support::state_delta::tests::*` framework self-tests (present in every
acceptance binary), NOT slice-24 scenarios; all five `philosophy_add_*` functions
appear in the `failures:` list.

## Gate verdict

**PASS.** Every failing test fails for the RIGHT reason (MISSING_FUNCTIONALITY —
the `philosophy add` verb, its compose / sign / persist path, its seed-collision
guard, and its empty-description rejection do not exist yet; clap rejects `add`
with exit 2). Zero tests are in category 2 (IMPORT_ERROR / FIXTURE_BROKEN /
SETUP_FAILURE) or category 3 (WRONG_ASSERTION / internal-struct coupling — every
assertion scans the OBSERVABLE CLI stdout / stderr / exit code, or reads the
on-disk signed artifact as TEXT, never a `lexicon` struct field). Handoff to
DELIVER is UNBLOCKED for slice-24.

## Error/edge ratio note

5 scenarios: PA-1 (WS happy mint) + PA-5 (author-DID happy) = 2 happy; PA-2
(local-first cancel, edge) + PA-3 (seed-collision, error) + PA-4
(empty-description, error) = 3 non-pure-happy = **60%** (≥40% target). AC-003.3
(seed collision) and AC-003.4 (invalid record) — the two named failure modes —
are covered explicitly by PA-3 / PA-4, example-based per Mandate 11 (no PBT at
layer 3).

## Outcomes-registry note

Skipped — `docs/product/outcomes/registry.yaml` does not exist and the prior
philosophy slices (22 seed+list, 23 show) registered no OUT-N rows. Following
that precedent, no outcome is registered for the `philosophy add` operation. If
the registry is later adopted for this feature, register `philosophy add` as a
`kind: operation` row at that time.

## DELIVER pointers (from the observed RED)

1. Add an `Add { name, description, aliases, see_also }` variant to the
   `philosophy` subcommand enum + a `verbs/philosophy_add.rs` (mirroring
   `verbs/claim_add.rs`'s two-prompt compose → sign → persist structure). Compose
   an `org.openlore.philosophy` record from the flags; validate it via
   `lexicon::validate_philosophy_json`; render a compose preview; block at the
   sign prompt (EOF = clean cancel, no side effect — KPI-5 local-first); on Enter,
   sign via `claim_domain::{canonicalize, compute_cid, sign}` (ADR-006, no new
   signing model) and persist a signed `<cid>.json` under `<root>/philosophies/`
   (tmp + fsync + rename) plus a `philosophies` table row (schema_v4). Print the
   derived object id `org.openlore.philosophy.<normalize(name)>`. Publish
   deferrable (second prompt, `n` declines). Turns PA-1 / PA-2 / PA-5 GREEN.
2. Pre-check the requested name against the seed set (`lexicon::find` /
   `object_id`) BEFORE signing: on a collision with a seed (e.g. `memory-safety`),
   exit NON-ZERO with a plain message that NAMES the collision and says it already
   exists + hints reuse or `--alias` onto the existing philosophy — e.g.
   `philosophy 'memory-safety' already exists; use it directly or add \`--alias\`
   onto it`. No silent duplicate id, no panic. Turns PA-3 GREEN.
3. Reject an EMPTY (or missing) `--description` with a named-field error via
   `validate_philosophy_json` — the error must mention the `description` field and
   must NOT panic / leak a backtrace (AC-003.4). Note: the current
   `validate_philosophy_json` Gate 1 only checks field PRESENCE
   (`contains_key`), so an empty string `""` passes today — DELIVER must add an
   empty/blank check (or route the CLI to reject empty before compose). Turns
   PA-4 GREEN.
4. Schema_v4: extend the idempotent `CREATE TABLE IF NOT EXISTS` +
   `schema_version` migration-runner (adapter-duckdb `schema.rs` / `schema_v3.rs`)
   with a `philosophies` table (`cid PK | object_id UNIQUE | name | description |
   author_did | composed_at | artifact_path`). The `object_id UNIQUE` constraint
   plus the seed pre-check (pointer 2) together enforce no-duplicate-id.
5. Record the author DID on the signed record/row so the artifact carries it
   (PA-5 reads it back from the on-disk JSON — no minted-philosophy read surface
   exists yet at slice-24). Reuse the existing identity/signing wiring; no new
   signing model.
6. `xtask check-arch` stays 21 members / no new crate (one CLI verb + one DuckDB
   schema migration over the existing `lexicon` + `claim_domain` cores).

## Upstream gaps for DELIVER to resolve

- **AC-003.4 needs an emptiness check the pure core does not yet have.**
  `validate_philosophy_json` validates field PRESENCE, not non-emptiness — an
  empty `--description ""` currently satisfies the validator. DELIVER must decide
  WHERE the empty-description rejection lives (extend `validate_philosophy_json`
  to reject blank required strings, OR add a CLI-layer pre-check like
  `claim_add`'s confidence-range guard). PA-4 asserts only the observable (named
  `description` error, no panic, no persist), so either placement satisfies it —
  but the design does not currently name the mechanism. Recommend extending the
  pure validator so the invariant holds for every caller (scraper mint too), not
  just the CLI.
- **Author-DID location on the signed philosophy artifact is inferred, not
  specified.** Architecture §4.5 names an `author_did` COLUMN on the
  `philosophies` table, but does not pin whether the signed `<cid>.json` artifact
  embeds the DID (as `SignedClaim` does via its `UnsignedClaim`). PA-5 asserts the
  DID appears in the artifact JSON (the port-observable, since no minted-record
  read surface exists yet). If DELIVER stores the DID only in the DuckDB row and
  NOT in the artifact, PA-5 will need re-pointing at a future `philosophy show
  <minted>` read surface — which does not exist at slice-24. Recommend embedding
  the author DID in the signed artifact (mirroring the claim signing envelope) so
  the record is self-describing and portable.
