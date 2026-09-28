# Walking Skeleton — contributor-philosophy-inference

- **Wave**: DISTILL · **Date**: 2026-09-27 · **Designer**: Quinn (nw-acceptance-designer)
- **Slices**: thin thread through 01 → 02 → 03 (US-CPI-001/002/003) · **Jobs**: J-004 (J-004d, J-004e, J-004c)
- **Executable SSOT**: `tests/acceptance/infer_people_sign.rs` → **WS-CPI-1**
  (`maria_signs_an_evidence_backed_adherence_for_the_person_who_builds_her_signed_repos`)
- **Status**: **IMPLEMENTED** — green since DELIVER 01-02 (`6034659`, 2026-09-28; 01-01
  `25c8c5b` landed the contribution-link half of the thread). Live counterpart: IS-10 on real
  GitHub (`OPENLORE_LIVE_GITHUB=1`). (At DISTILL handoff it was RED, not `#[ignore]`.)
- **Migrated**: 2026-09-28 from `docs/feature/contributor-philosophy-inference/distill/walking-skeleton.md`

## The one scenario

```gherkin
@walking_skeleton @driving_port @driving_adapter @real-io @us-cpi-001 @us-cpi-002 @us-cpi-003 @kpi-cpi-2
Scenario: Maria signs an evidence-backed adherence for the person who builds her signed repos
  Given Maria scraped BurntSushi/ripgrep and rust-lang/regex, and both record BurntSushi as #1
  And she signed "memory-safety" for both repos
  When she runs `openlore infer people --sign 1`, sets confidence 0.45 and signs
  Then a claim "github:BurntSushi adheresToPhilosophy memory-safety" is signed in her name at 0.45
  And its evidence names each supporting repo claim (author + CID) and BurntSushi's commits page per repo
  And nothing else in her store changed and nothing was published
```

## Litmus (Mandate 5)

1. **User goal title** — "sign an evidence-backed adherence for the person who builds her signed repos".
2. **Given/When are user actions** — scrape, sign repo claims, sign the inference.
3. **Then are user observations** — the signed claim's subject/predicate/object/confidence/author and its evidence list (what any peer will read), plus "nothing else changed".
4. **Stakeholder confirmable** — "the claims I signed about repos now let me sign an auditable statement about the *person* who builds them."

## Why this thread

The DISCUSS walking-skeleton strategy is a *brownfield thin thread*: the person thread
(scrape → link → infer → sign) completes at slice-03. The skeleton therefore spans slices
01-03 thinly: DELIVER lands just enough of each (contributors read + link write, a minimal
inference with provenance, the extracted sign batch with prefilled evidence) to go green, then
fleshes each slice out scenario-by-scenario. It carries the two riskiest assumptions at once:
people really are linked across scraped repos (R-1) and provenance survives into the signed,
CID-stable payload without a Lexicon change (D-9, ADR-064).

## End-to-end path

```
openlore scrape github BurntSushi/ripgrep   REAL CLI → FakeGithub GET /repos/…/contributors → REAL contribution_links (DuckDB)
openlore scrape github rust-lang/regex      (same)
openlore claim add … embodiesPhilosophy ×2  REAL claim-domain sign + DuckDB + claims/<cid>.json
openlore infer people --sign 1              REAL: links + StoragePort reads → pure inference → shared sign batch
                                            → canonicalize → CID → sign → claims/<cid>.json (publish declined)
```

REAL: `openlore` binary, `scraper-domain`, `claim-domain`, `adapter-duckdb`, filesystem.
FAKE (driven-external only): `FakeGithub`, `FakePds`, `FakeIdentity`.

## Universe (Mandate 8)

`assert_store_delta` over `{local.claims.row_count, local.claim_files, local.peer_claims.row_count,
local.contribution_links, pds.records.len}`: only the claim count (+1) and claim-file set change;
links, peer claims and the PDS are implicitly unchanged (fail-closed).

## RED at DISTILL handoff (historical — fail-for-the-right-reason)

Run 2026-09-27 (`cargo test -p cli --test infer_people_sign`): every GIVEN succeeds against
today's binary (both scrapes, both `claim add`s); the WHEN fails with

```
error: unrecognized subcommand 'infer'
```

exit 2 → assertion `openlore infer people --sign 1 must sign the selected candidate` fails.
Classification: **MISSING_FUNCTIONALITY** (the verb does not exist) — correct RED, not a
fixture/setup failure. Note the scrapes already exit 0 today because `FakeGithub` serves
`/contributors` with a default the current harvest never requests; link recording is asserted
indirectly through the evidence (the commits URLs only exist if BurntSushi was linked to both
repos).
