<!-- markdownlint-disable MD013 -->
# Evolution: github-did-identity-link (slice 4: the viewer's linked-identity line)

> The FOURTH slice of `github-did-identity-link`, added after the three planned
> slices. Slices 1 to 3 are in `github-did-identity-link-slice-{1,2,3}-evolution.md`.
> The feature directory stays in place, because later slices may follow.
> Paradigm: functional Rust (ADR-007). Companion design:
> `design/slice-4-viewer-linked-identity.md` and `design/wave-decisions.md`
> § "Slice 4" (D-VL-1..11, C-VL-1..3). No new ADR (D-VL-9). Slice-4 ruling:
> `deliver/upstream-issues.md` U-51.

## Summary

Slices 1 to 3 let the CLI and the review app publish, check and read GitHub
links. Slice 4 shows them in the local viewer (`openlore ui`). Four pages that
are about one person now carry a "Linked identities" region: the contributor
page (`/score?contributor=<did>`), the GitHub person page
(`/project?subject=github:<login>`), a claim's detail page (under the Author
row), and each author group in `/search`. Each line is the `identity list` line,
character for character, with the DID and the account as links and the re-check
command in `<code>`. The lines come from the reader's own last checks in the
local store. The viewer never asks GitHub and writes nothing. Retracted,
superseded, third-party, flawed and removed-peer links never show. If a link
read fails, or the startup probe is refused, the region says "Linked identities
could not be read from your local store." and never "not checked". On `/search`,
every link row reads "authority not checked here".

### What shipped (one paragraph)

`read_listed_links` and its lineage fold moved from the CLI into `ports`
(finally `ports::identity_link_listing`), with a typed `LinkReadError`. The CLI
delegates, so `identity list` and the viewer share one read (D-VL-3).
`viewer-domain` gained a pure `linked_identity` module: the `LinkedIdentities`
view-model (`None` / `Lines` / `Unreadable`), the region and line renders, and
the heading, legend and notice constants. The score, traversal, detail and
search renders embed the region inside each results fragment, so the htmx swap
and the full page match. `adapter-http-viewer` holds optional read-only
`LinkReads` (check read, candidate read, lineage read), reads once per request
on the four pages, and has a debug-only fault seam
`OPENLORE_VIEWER_FAIL_IDENTITY_LINKS=checks|candidates|lineage`. `ui.rs` wires
the three handles from the one DuckDB storage after a read-only startup probe
(one `read_listed_links` call, never the check port's insert-and-rollback
`probe()`). A refused probe injects no handles and emits
`viewer.identity_links.probe {refused: true}`; the viewer still serves. check-arch
extends `identity_link_readers_hold_no_write_handle` to the viewer adapter, and
the seam token joins `VIEWER_FAIL_SEAM_TOKENS`. Step 04-09 fixed U-51 in the
shared read: a retraction that repeats the link's evidence no longer shows.

### Wave timeline

- DISCUSS: none. US-VL-1..3 stand in, recorded as UC-11 in
  `upstream-changes.md`.
- DESIGN: 2026-10-19, propose mode. Solution-architect review: iteration 1
  NEEDS_REVISION (S4-1 critical: the check port's `probe()` writes, rolled back),
  iteration 2 APPROVED_WITH_NOTES (N-1 fixed in the text). **Status: PROPOSED.**
  The user said "start the identity-link viewer slice", and the three open
  questions were defaulted: no `/peers` line, no `/identities` page, legend and
  notice wording as proposed. The user has not yet explicitly confirmed these
  defaults.
- DISTILL: 2026-10-19. 21 VL scenarios (VL-1..20, VL-17 split into a/b), the
  VL-2p property and the VL-G guard case, in 19 test fns. Review cycle 1: all
  four reviewers (PO, solution architect, platform architect, Sentinel)
  rejected; every finding was fixed with no waivers. Cycle 2: PO and SA
  approved. The PA residuals (F-5 clock in VL-17b/VL-20, viewer stderr not
  drained) and the Sentinel residuals (F1-F4, F7, F8, N1) were fixed without a
  third review (max two cycles; the user asked to proceed).
- DELIVER: 2026-10-19. Roadmap `56964e0`, phase 04 with 8 steps. Review: round 1
  rejected on F1-F9; round 2 resolved all but F7 (low, a criterion named a
  check-arch test fn), which the orchestrator rephrased under the max-2 rule.
  Step 04-09 was added after approval for U-51 (`3d09da6`) and not re-reviewed.
  All 9 steps COMMIT/PASS. `des-verify-integrity`: all 35 steps (slices 1 to 4)
  have complete DES traces.

## DELIVER steps (5-phase TDD, functional crafter)

- **04-01** `b3eb2f4`: walking skeleton. The read moves into `ports` with a typed
  `LinkReadError`; `/score` shows the contributor's linked identities; no link,
  no region; a pre-v7 store loads every page (VL-1, VL-12, VL-20).
- **04-02** `fea4066`: every standing reads as `identity list` prints it; a
  future-dated check is ignored; the VL-2p parity property binds to the viewer
  render through a `viewer-domain` dev-dependency (VL-2, VL-18, VL-15, VL-2p).
  **Its tests passed on activation**: the 04-01 render already printed every
  standing, so no production change was needed. The RED evidence for this
  behaviour came from mutation (Phase 5), not from a failing test.
- **04-03** `4f72d71`: the GitHub person page shows the DIDs that linked the
  account, by last-seen login in any case; contested reads contested on every
  page; a rename moves the line only on re-check (VL-3, VL-4, VL-17a/b).
- **04-04** `951bdb7`: retracted, superseded, third-party, flawed and
  removed-peer links never show (VL-5, VL-6, VL-7). Test-only step; **RED was
  recorded by mutation**: dropping the lifecycle fold fails VL-6, dropping the
  author=subject or evidence rule fails VL-5, dropping the U-19 filter fails
  VL-7. This RED found U-51.
- **04-05** `5a40c0a`: a claim's detail page shows its author's linked identity
  (VL-10).
- **04-06** `79f2dc4`: search author groups show each author's linked identity;
  link rows read "authority not checked here" (VL-11, VL-19).
- **04-07** `feb73b3`: an unreadable link read or a refused probe shows the
  notice on every page; the seam token is release-guarded (VL-9, VL-14, VL-G).
- **04-08** `a0ac624`: guards over the finished region. No GitHub request, no
  write, and the htmx swap matches the full page (VL-8, VL-13, VL-16). Test-only
  step with a **recorded mutation RED**, as 04-04.
- **04-09** `0ef452c`: U-51. Each link candidate carries its own references, so
  the claim-domain retraction rule decides; no SQL or viewer filter. RED
  `d832f35` (IR-8e), plus the adapter test `link_candidates.rs`.

DES phase records: `539a98e`, `430fab5`, `50fbce2`, `a2dba0f`, `bb3ce47`,
`ff4bea3`.

## Phase-3 refactor

- `c85f2ce` (L1-L4): one render fn per page, with the region as a parameter and
  no test-only wrappers; one adapter selection over a `LinkedPerson`; author
  selection shares `LinkedIdentities::selecting`; the shared read moves to
  `ports::identity_link_listing`; the re-check command and the GitHub prefix are
  named constants. No behaviour change.

## Phase-4 adversarial review

**Result: approved_with_notes.**

- **F1**: no test showed that a page not about a GitHub person carries no
  GitHub-person lines. Added as **VL-21** (`3ccd45a`).
- **F3**: `/score` must select lines by the bare contributor DID (a DID with a
  signing fragment matched nothing). Fixed in `e84a5d8`.
- **F2** (per-candidate N+1 referrer query), **F4** (a nuance in the startup
  probe) and **F5** (optional unit tests) were left as follow-ups.

## Quality gates — final report

| Gate | Result |
|---|---|
| DESIGN review | APPROVED_WITH_NOTES after 2 iterations (S4-1..S4-8, N-1 fixed); status PROPOSED, awaiting the user |
| DISTILL review | Cycle 1: all four rejected, all fixed. Cycle 2: PO and SA approved; PA and Sentinel residuals fixed without a third review |
| Roadmap | APPROVED after 2 rounds (F7 rephrased by the orchestrator); 04-09 added after approval for U-51 |
| DES integrity | `des-verify-integrity`: all 35 steps (slices 1 to 4) have complete traces; 9/9 slice-4 steps COMMIT/PASS |
| Phase-3 refactor | `c85f2ce` (L1-L4) |
| Phase-4 adversarial review | approved_with_notes; F1 (VL-21) and F3 fixed; F2, F4, F5 follow-ups |
| Phase-5 mutation (per-feature, gate ≥80%) | **152 mutants** over `56964e0..3ccd45a`: **65 caught, 0 missed, 87 unviable**. **100% of viable mutants caught.** First pass 63/65 (96.9%); two survivors killed by new unit tests in `7a3a1b9`. Gate PASSED |
| check-arch | OK, with the viewer adapter under `identity_link_readers_hold_no_write_handle` and the new seam token guarded |

### Mutation tooling note

As in slices 1 to 3, cargo-mutants 25.3.1 ignores tests in another package, so
mutants were listed by cargo-mutants and run by the manual runner in scratch
worktrees. The two first-pass survivors were real gaps: `GET /claims/` with an
empty CID must be the route-miss 404 (#25,
`route_answers_an_empty_claim_cid_with_the_route_miss_404`), and only search
states with author groups may ask for linked identities (#135,
`only_search_states_with_author_groups_ask_for_linked_identities`). Mutant #49
replaces the release-only seam function, which the debug build never compiles,
and does not compile in release; it was reclassified as unviable. Most of the
other 86 unviable mutants use `Default::default()` for types with no `Default`.
Report: `deliver/mutation/slice-4-mutation-report.md`.

## Deploys

None. The viewer runs locally (`openlore ui`); there is nothing to deploy.
Not pushed: the slice is committed locally on `main` through `7a3a1b9`.

## Load-bearing invariants

- **One shared read**: `identity list` and the viewer both call
  `read_listed_links`. Retraction, U-19 and U-39/U-51 rules live there and in
  `claim_domain::identity_link`, so the two surfaces cannot drift (D-VL-3).
- **No viewer-side filter**: the viewer adds no rule about what shows. Standings
  are decided over every active link before a page selects its lines (C-VL-3),
  and link rows are selected only through `is_reserved_predicate`.
- **No GitHub request or write from the viewer**: the viewer holds no check
  write or purge handle and no signing surface; its `GithubPort` stays used by
  `POST /scrape` only. The startup probe is read-only (C-VL-1, C-VL-2).
- **The region sits inside every results fragment**, so the htmx swap and the
  full page always carry the same lines (VL-13).
- **Debug-only fault seam with a release guard**:
  `OPENLORE_VIEWER_FAIL_IDENTITY_LINKS` fails a read per request, never the
  startup probe, and is in `VIEWER_FAIL_SEAM_TOKENS`, so an ungated read fails
  check-arch (VL-G).
- **A failed read never reads as "not checked"**: any read error makes the whole
  region `Unreadable`; there are no partial lines.

## Deviations: planned vs shipped

- 04-02 needed no production change; its tests went green on activation (the
  04-01 render already covered every standing).
- 04-09 was not in the reviewed roadmap; it was added for U-51, found by 04-04's
  mutation RED.
- The shared read lives in `ports::identity_link_listing` (the design named
  `ports::identity_link_check`); moved in the L1-L4 refactor.
- VL-21 was added after the adversarial review (F1).

## Follow-ups

- **F2**: one referrer query per `sameAs` candidate (N+1). Batch it into one
  query if a store exceeds about 200 candidates (design cost note). No cache.
- **Open design questions, awaiting the user's explicit confirmation**: (1) a
  line on `/peers`, (2) an `/identities` page mirroring `identity list`, (3) the
  legend and unreadable-notice wording. All three were defaulted (no, no, as
  proposed).
- **Private `bare_did` duplicate**: `adapter-http-viewer` keeps its own private
  `bare_did` (`crates/adapter-http-viewer/src/lib.rs`) next to the shared
  `viewer_domain::common::bare_did`; fold it into the shared one.
- **F4** from the adversarial review: `read_listed_links` returns before the
  check read when no link is stored, so the startup probe exercises the check
  table only when links exist. VL-14 relies on that; note it in its comment.
- **F5** from the adversarial review: `of_author`, `of_person`,
  `github_person` and the Lines render are covered only by acceptance tests;
  unit tests for the pure selectors are optional.
- Known limit (D-VL-6, VL-17): after a rename and reuse of the old login, the
  old login's page shows the first DID's line until the reader re-checks.
- Carried from slice 3: U-44 NF-1, F-6, NF-4/NF-5, F-11, U-20, and the slice-2
  carry-overs.

## Lessons

1. A test that passes on activation is not RED evidence. 04-02's tests were
   green from the start because 04-01 had built the render; mutation supplied
   the RED. For test-only steps (04-04, 04-08), record a mutation RED in the
   step commit.
2. Mutation RED finds bugs outside the slice. 04-04's run showed that the
   slice-2 lineage read passed empty references, so a retraction that repeated
   the link's evidence would show as a link (U-51). Moving a read to share it
   also shares its bugs; fixing it once fixed both surfaces.
3. Compare DIDs in one canonical form. `/score` matched nothing for a DID with
   a signing fragment until it used the bare DID (F3).
4. A read-only surface must use a read-only probe. The check port's `probe()`
   writes and rolls back, which the viewer may not do (S4-1).

## Try it

```sh
openlore identity link --github <login>      # or pull a peer's published link
openlore identity verify --github <login>    # record a check
openlore ui                                  # then open the pages below
```

1. `/score?contributor=<did>`: the "Linked identities" region shows
   `<did> is github:<login> — verified today`.
2. `/project?subject=github:<login>`: the same account's DID line.
3. A claim by that DID at `/claims/<cid>`, and `/search` for it: the line under
   the author; link rows read "authority not checked here".

## Lasting artifacts

- `docs/architecture/github-did-identity-link/architecture-design.md`
- `docs/scenarios/github-did-identity-link/walking-skeleton.md`
- `docs/feature/github-did-identity-link/design/slice-4-viewer-linked-identity.md`
  (stays in the feature directory)
- Slice-4 ruling: `docs/feature/github-did-identity-link/deliver/upstream-issues.md` (U-51)

## Commit trail

Roadmap `56964e0`, `d72a3bc` (U-51), `3d09da6` (04-09); steps `b3eb2f4`,
`fea4066`, `4f72d71`, `951bdb7`, `5a40c0a`, `79f2dc4`, `feb73b3`, `a0ac624`,
`d832f35`, `0ef452c`, `d7f513f`; refactor `c85f2ce`; review `e84a5d8`,
`3ccd45a`; mutation `7a3a1b9`. All on `main` (trunk-based, no PR). Interleaved
`review-app-repos-and-accounts` commits (`602f9be`, `079f571`, `b780d68`) belong
to another feature. Not pushed.
