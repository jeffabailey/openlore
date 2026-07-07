<!-- markdownlint-disable MD013 -->
# Evolution: scraper-seeded-vocabulary (slice-28 — the scraper's proposed philosophy objects are validated against the seeded vocabulary as the single source)

> Part of the `philosophy-vocabulary-registry` feature (slices 22–28). Builds on
> slice-22 (the seeded vocabulary + `lexicon::philosophy::find`). Paradigm:
> functional Rust (ADR-007). Companion design: ADR-059 §5 (slice-28 row).

## Summary

slice-28 ships US-PV-007 ("Scraper proposes seeded philosophies"). `load_mapping`
(the scraper's signal→predicate mapping loader) now validates every entry's object
against the seeded vocabulary via `lexicon::philosophy::find`, rejecting any object
that is not a seeded philosophy with `MappingError::UnknownPhilosophy { object }`
(the Display names the offender). The seeded vocabulary is now the **single source
of truth** for what the scraper can propose — a drift string like
`org.openlore.philosophy.mystery` is impossible to ship: the enforcement runs in
the production scrape path (`verbs/scrape_github.rs` loads `EMBEDDED_MAPPING_YAML`
at startup, before the network harvest), so a drifted SSOT edit fails the scraper
immediately with a named error (KPI-PV-6: 0 orphan philosophy strings). No new
crate — `scraper-domain` gained a pure→pure `lexicon` dependency (acyclic);
workspace stays 21 members.

### What shipped (one paragraph)

`crates/scraper-domain/src/mapping.rs::load_mapping` gained a per-entry vocabulary
gate: after resolving each `signal → SignalKind`, it calls
`lexicon::philosophy::find(&entry.object)` and returns
`Err(MappingError::UnknownPhilosophy { object })` when the object does not resolve
to a seed. `lexicon` was promoted from a dev-dependency to a real dependency of
`scraper-domain` (both pure; no I/O crate pulled in — check-arch stays green). The
shipped SSOT mapping (5 objects: dependency-pinning, documentation-first,
test-driven, semantic-versioning, memory-safety — all already seeded) is
**unchanged**: this slice adds the enforcement, not a data fix (no `mystery` drift
existed).

### Wave timeline

- DISCUSS / DESIGN — feature level: `feature-delta.md` US-PV-007 (AC-007.1/007.2,
  KPI-PV-6) + ADR-059 §5 (slice-28 row).
- DISTILL — commit `08412e0`: RED scaffold in `mapping.rs` (`philosophy_vocabulary_tests`)
  + `distill/red-classification-slice-28.md`. Gate PASS — 1 genuine RED
  (`load_mapping_rejects_object_not_in_seeded_vocabulary`) + 1 GREEN-today guardrail,
  0 BROKEN.
- DELIVER — this slice.

## DELIVER steps (5-phase TDD, functional crafter, DES-traced; integrity exit 0)

- **01-01** `eb10075` — the vocabulary gate in `load_mapping` +
  `MappingError::UnknownPhilosophy` + `lexicon` promoted to `[dependencies]`.
  Greened the drift-rejection RED; guardrail + 9 slice-02 tests stayed green.
- **Phase-3 refactor** `0204157` — L2 extract `parse_entry(EntryDto) -> Result<MappingEntry, MappingError>`,
  so `load_mapping` reads as a clean `from_str → map(parse_entry) → collect` pipeline.
- **Phase-5 mutation** `ee6346a` — killed the one survivor (a `Display → default`
  mutant on the error message) by adding
  `unknown_philosophy_display_names_the_offending_object`, taking the pure-core
  kill rate to 100%.

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED — automated quality gate PASS (1 step / 2 production files = ratio 0.5, 1 genuine RED owned + 1 guardrail) |
| DISTILL RED | genuine MISSING_FUNCTIONALITY (drift object accepted by `load_mapping` today), 0 BROKEN — gate PASS |
| Integrity | `verify_deliver_integrity` exit 0 — 1 step complete DES traces |
| Phase-3 refactor | L2 extract `parse_entry` (behavior-preserving) |
| Phase-4 adversarial review | APPROVED — 0 defects, 0 testing theater; **production-seam enforcement verified** (scrape startup, before harvest; error propagates, no panic) |
| Phase-5 mutation | pure-core `mapping.rs` = **100% (9/9 viable, 4 unviable no-Default substitutions, 0 survivors)** — gate ≥80% PASSED |
| check-arch | OK — 21 workspace members; `scraper-domain → lexicon` is a clean pure→pure edge |

Tests green: `scraper-domain` 12 (drift-rejection RED now green + 2 guardrails + 9
slice-02); `lexicon` 41; `philosophy_vocabulary` 8/8 (no-regression).

## Field note — the scraper's real-repo signal detection is still a walking skeleton

This slice makes the scraper's *proposed objects* provably seeded. Separately, a
live scrape of a **real** GitHub repo currently yields **0 signals**: the harvest
(`adapter-github::harvest_repo`) reads a `signals[]` array from the
`GET /repos/{owner}/{repo}` response body, which only the in-process `FakeGithub`
test double provides — the real GitHub API returns no such field, and the logic to
DERIVE signals from real repo metadata (Cargo.lock, test ratio, docs, semver tags,
language) was never implemented (slice-02 walking skeleton; `lib.rs:41` "live paths
land Phase 03/04"). Verified this session: `openlore scrape github BurntSushi/ripgrep`
→ "0 signals". Injecting the signals a real detector *would* produce (via the
`OPENLORE_GITHUB_API_BASE` seam) drives the full pipeline end-to-end — 5 philosophy
candidates derived, and `--sign` persists a chosen one (confirmed via `graph query`:
`embodiesPhilosophy → org.openlore.philosophy.memory-safety`, cid `bafyrei…`). So
the inference→sign→persist path works and its proposable objects are now
seed-validated; the missing piece for real-repo use is signal DETECTION, a
follow-up beyond US-PV-007's scope.

## KPI

KPI-PV-6 (No drift — scraper proposes only seeded objects, 0 orphan strings): MET —
`load_mapping` rejects any non-seeded object at scraper startup, in the production path.

## Feature status

Shipped in this feature: slice-22 (list + seeds), slice-23 (show), slice-28
(scraper seed-validation). Still OUT/unbuilt: slice-24 (`philosophy add` mint),
slice-25 (claim-compose advisory), slice-26 (alias triangulation), slice-27
(`/philosophies` viewer surface).

## Commit trail

`08412e0` (DISTILL RED), `eb10075` (01-01), `0204157` (Phase-3 refactor),
`ee6346a` (Phase-5 mutation). All on `main` (trunk-based, no PR).
