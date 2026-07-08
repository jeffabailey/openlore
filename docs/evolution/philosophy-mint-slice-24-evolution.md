<!-- markdownlint-disable MD013 -->
# Evolution: philosophy-mint (slice-24 — `openlore philosophy add`: compose → sign → persist a new philosophy record)

> Part of the `philosophy-vocabulary-registry` feature (slices 22–28). Builds on
> slice-22 (pure lexicon vocabulary core + `philosophy list`) and slice-23
> (`philosophy show`). Paradigm: functional Rust (ADR-007). Companion design:
> ADR-059 §4.5 (minted storage) + §5 (slice-24 row). First slice in the feature
> that WRITES and SIGNS.

## Summary

slice-24 ships US-PV-003 ("Mint a new philosophy"): `openlore philosophy add
--name <n> --description <d> [--alias <a>…] [--see-also <url>…]` composes, SIGNS,
and persists a new `org.openlore.philosophy` record — federated, open, no
gatekeeper — mirroring `claim add`'s two-prompt, local-first flow. It prints the
derived object id (`org.openlore.philosophy.<normalize(name)>`) and the written
artifact path. No new crate — workspace stays 21 members.

### What shipped (one paragraph)

The pure `lexicon::validate_philosophy_json` gained Gate 1b: a present-but-blank
(empty or whitespace-only) required string (name, description) is rejected with
the reused `LexiconError::MissingField` (no parallel error type) — placed in the
PURE validator so the scraper mint path inherits it too (AC-003.4). A new
idempotent, forward-only `schema_v4` migration (mirrors `schema_v3` + the
`schema_version` guard) creates the `philosophies` table (`cid PK | object_id
UNIQUE | name | description | author_did | composed_at | artifact_path`). A new
`SignedPhilosophy` boundary type (embedding `author_did` so the record is
self-describing) rides the EXISTING `StoragePort` — no new port trait (ADR-059
§3) — via `write_signed_philosophy`, which writes the signed `philosophies/<cid>.json`
artifact (atomic tmp+fsync+rename) and the DB row in one transaction; a duplicate
`object_id` surfaces a typed `StorageError` (no panic). The CLI adds
`PhilosophyCommand::Add { … }` + `verbs/philosophy_add.rs` mirroring `claim_add`:
validate-before-prompt → seed-collision pre-check → compose preview → sign prompt
→ (EOF = clean cancel, no write) → `compute_cid` + `IdentityPort::sign` (ADR-006
reused verbatim, no new signing model) → persist. Dispatched AFTER
`Wiring::production` (needs store + signer, unlike the offline `list`/`show`).

### Wave timeline

- DISCUSS / DESIGN — covered at the feature level: `feature-delta.md` US-PV-003
  (AC-003.1..4) + ADR-059 §4.5 + §5 (slice-24 row). No per-slice DISCUSS/DESIGN —
  the settled feature design already specs the mint slice.
- DISTILL — 2026-07-08, commit `0d55c6f`: RED scaffold `tests/acceptance/philosophy_add.rs`
  (PA-1..5) + `distill/red-classification-slice-24.md`. Gate PASS (5/5 genuine
  RED — clap `unrecognized subcommand 'add'`, 0 BROKEN); 60% error/edge.
- DELIVER — 2026-07-08 (this slice).

## DELIVER steps (5-phase TDD, functional crafter, DES-traced)

- **01-01** `ed2d77c` — pure `validate_philosophy_json` blank-required-string gate
  (Gate 1b, reuses `MissingField`) + proptest over the blank equivalence class
  (both fields) + a no-regression property. Greened **PA-4**.
- **02-01** `9b14730` — `schema_v4` `philosophies` table + `SignedPhilosophy`
  (author_did embedded) on the existing StoragePort + `write_signed_philosophy`
  (atomic artifact + row, one tx; duplicate object_id → typed `WriteFailed`);
  probe asserts v4. Real-temp-store integration tests (write, duplicate, v4).
- **03-01** `613c8d6` — `PhilosophyCommand::Add` + `verbs/philosophy_add.rs`
  (validate → collision-check → preview → sign prompt → EOF-cancel/sign+persist),
  prints object id + path; `paths.philosophies_dir()` + `render_compose_preview`.
  Greened **PA-1** (walking skeleton), **PA-2** (local-first cancel), **PA-5**
  (author DID in the signed artifact).
- **03-02** `7617ece` — seed-collision pre-check (`lexicon::philosophy::find`)
  before preview/sign/persist → non-zero exit + plain guidance (names the
  collision, "exists", hints `--alias`), no write, no panic. Greened **PA-3**.

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — automated quality gate PASS (decomposition ratio 0.5 steps/production-files, PA-1..5 each owned once, valid DAG 01-01 → 02-01 → 03-01 → 03-02, IDs valid, DISTILL linkage present) |
| DISTILL RED | 5/5 genuine MISSING_FUNCTIONALITY (clap `unrecognized subcommand 'add'`), 0 BROKEN — gate PASS; 60% error/edge |
| Phase-3 refactor (L1-L4) | No changes warranted — code already clean (mirrors `claim_add`; pure validator gate reuses the existing error variant; adversarial review found 0 L1-L2 smells) |
| Phase-4 adversarial review | APPROVED — 0 blockers, 0 defects, 0 testing theater; independently verified local-first ordering (no write before confirm), two-layer seed-collision defense, signing/CID soundness, author-DID artifact embedding, idempotent/forward-only schema_v4, zero panics on error paths, clean pure/effect boundary, genuine reuse (no duplication), test budget respected (6/8) |
| check-arch | OK — 21 workspace members (no new crate); pure-core import ban intact |
| Acceptance | `philosophy_add` 5/5 (PA-1..5) + 2 support self-tests = 7/7 GREEN |
| Regression (lib/unit) | `cargo test --workspace --lib` → all crates GREEN, 0 failed (crafter-verified); `adapter-duckdb` 20 lib + 12 integration green |
| **Phase-5 mutation** | **PASS** — pure-core `crates/lexicon/src/philosophy.rs` (incl. the new blank gate) = **100% viable kill (16/16, 3 unviable Default-substitutions, 0 survivors)**; effect shell reported. Report: `deliver/slice-24/mutation/mutation-report.md` |
| **Full acceptance-subprocess suite** | **GREEN** — `cargo test --workspace` re-run on the uncontended box: every binary `ok`, 0 failed (incl. the viewer subprocess suite). The earlier "hang" was pure foundry CPU starvation, NOT a deadlock — `viewer_counter_claim_list_flags` passes on the clean re-run |

Tests green: `philosophy_add` 7/7; workspace `--lib` sweep 0 failed; `adapter-duckdb`
20 lib + 12 integration; full `cargo test --workspace` re-run 0 failed.

## Deferred gates — CLOSED (2026-07-08, same day)

Both gates were deferred at finalize because an **unrelated `foundry` project
build** (6–10 rustc processes, 45+ min) starved all CPU, so cargo test runs stalled
at 0.00% CPU. Once that build finished, both gates were run to completion on the
uncontended box and **both PASSED**:

1. **Phase-5 mutation** — `cargo mutants -p lexicon --file crates/lexicon/src/philosophy.rs`
   → 19 mutants, **16 caught / 3 unviable / 0 survivors = 100% viable kill**. Report:
   `deliver/slice-24/mutation/mutation-report.md`.
2. **Full `cargo test --workspace`** — every binary `ok`, 0 failed, including the
   full viewer subprocess suite.

The earlier apparent "deadlock" of `viewer_counter_claim_list_flags` was **CPU
starvation, not a hang** — it passes on the clean re-run (as do all viewer tests;
they are simply slow because each spawns the `openlore` UI subprocess). No test is
genuinely deadlocked, and slice-24 did not touch the viewer read path
(`store_read.rs` unchanged; `schema_v4` runs only at write-adapter `open()`, which
the passing `philosophy_add` + adapter integration tests already exercise).

## Deviations: planned vs shipped

- DELIVER workspace lives under `deliver/slice-24/` (its own roadmap +
  execution-log) to avoid clobbering the finalized slice-22/23/28 artifacts — the
  feature spans multiple slices under one feature dir.
- `crates/test-support/src/lib.rs` (not in the roadmap `files_to_modify`) gained a
  matching `write_signed_philosophy` — adding the method to `StoragePort` forces
  all impls to satisfy it; `InMemoryStorage` is an all-`panic!` RED scaffold and
  the mint ATs drive the real DuckDb adapter via subprocess, so a matching
  SCAFFOLD panic method is honest (no fake double).
- Duplicate `object_id` maps to the existing `StorageError::WriteFailed` (roadmap
  allowed reusing a fitting variant) rather than a new variant — zero
  match-breakage; the collision UX comes from the seed pre-check, UNIQUE is
  defense-in-depth.
- Philosophy canonical bytes use deterministic serde serialization (not
  `claim_domain::canonicalize`, which is `UnsignedClaim`-typed); `compute_cid` +
  `IdentityPort::sign` are reused verbatim (ADR-006). slice-24 persists LOCALLY
  only — no philosophy federation/CBOR wire contract owed yet. A dedicated
  canonicalize will be needed if cross-peer philosophy CID stability lands later
  (forward dependency, flagged).
- `// SCAFFOLD: true` left in `tests/acceptance/philosophy_add.rs` — matches repo
  convention (slice-23's shipped `philosophy_show.rs` still carries the marker).
- Outcome registry: skipped — `docs/product/outcomes/registry.yaml` does not exist
  and slices 22/23/28 registered none (precedent).

## KPI / dogfood

`./cli.sh philosophy add --name event-sourcing --description "State is an
append-only log of events." --alias es` → signs + persists an
`org.openlore.philosophy` record, prints `Minted philosophy:
org.openlore.philosophy.event-sourcing` + the written path;
`./cli.sh philosophy add --name memory-safety …` → refused (seed collision, hints
`--alias`), non-zero, no write; `--description ""` → named-field error, no panic.

## Next slices (OUT of scope)

25 claim-compose advisory, 26 alias triangulation, 27 viewer `/philosophies`
surface. (22 seed+list, 23 show, 28 scraper single-source already SHIPPED.)

## Commit trail

`0d55c6f` (DISTILL RED), `ed2d77c` (01-01), `9b14730` (02-01), `613c8d6` (03-01),
`7617ece` (03-02), `eabc9b0` (deliver roadmap + log). All on `main` (trunk-based,
no PR).
