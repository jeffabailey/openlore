<!-- markdownlint-disable MD013 -->
# Evolution: github-did-identity-link (slice 2: readers)

> The SECOND of three slices of `github-did-identity-link`. Slice 3 (review app
> "Publish this link") is still to come, so the feature workspace
> `docs/feature/github-did-identity-link/` stays live. Slice 1 is in
> `github-did-identity-link-slice-1-evolution.md`. Paradigm: functional Rust
> (ADR-007). Companion design: ADR-084, ADR-085 (reader semantics), ADR-086/087
> (owner-only-signing). Slice-2 rulings: `deliver/upstream-issues.md` U-1..U-20.

## Summary

Slice 1 let a DID owner publish a GitHub link and re-check it. Slice 2 makes every
reader show those links honestly and keeps them out of scoring. `identity list`
folds the lineage, so retracted and superseded links (the owner's or a peer's)
are gone; `--did` narrows to one person; duplicate records for one person and
account read as one line naming every record ID; each line gives its check age,
and a verified line past 30 days suggests a re-check. Scoring drops every
identity-link row before triangulation. The person views (`infer people`,
`scrape person`, `scrape github <user>`) show link lines offline, from this
reader's last checks. `scrape github <user>` also refreshes that login's link
checks from the profile it already fetched, with no second GET. Network search
labels a link row `authority not checked here`, still listed and attributed to
its author. A removed peer's links are hidden from every identity reader.

### What shipped (one paragraph)

One listed-link read (`identity_reads`) now backs `identity list` and the person
views, fed by a single candidate source, `LINK_CANDIDATES_SQL`, which folds owner
and peer retractions and supersessions and excludes soft-removed peers. The
scorer drops identity-link rows in `scoring::score` (U-3; it moves with
`score_attested` when owner-only-signing lands). The person-scrape refresh keeps
the already-fetched `/users/{user}` body, parses it strictly with
`parse_account_profile`, and decides through `check_link` gated on the account
number (U-4, U-5); `GithubPort::resolved_account` is keyed by user so one user's
cached profile never serves another. The login a re-check reads and a person
row's key are claim-domain rules, not verb code (review M2).

### Wave timeline

- DISCUSS: none (as slice 1).
- DESIGN / DISTILL: shared with slice 1 (2026-10-14, `a6735f7`). Slice-2
  scenarios IR-1..IR-19 were held `#[ignore]` as `deferred_to_later_slices`.
- DELIVER: 2026-10-16 to 2026-10-17. Roadmap `d4a811f`, revised `d2672a2`
  (review F1–F16) and `6268450` (round 2, N1–N7), phase 02 approved: 8 steps.
  All 8 COMMIT/PASS; `des-verify-integrity`: all 17 steps (slices 1 and 2) complete.

## DELIVER steps (5-phase TDD, functional crafter)

- **02-01** `c30ed35`: a retracted or superseded link leaves `identity list`;
  peers' links read as claims until this reader checks them; a peer-pulled
  self-attested link keeps its record ID (IR-19 round-trip guard, U-11).
- **02-02** `3b8e515`: `identity list --did` narrows the list to one person's links.
- **02-03** `f10e455`: duplicate link records for one person and account read as
  one line naming every record ID, in record-ID order (U-9, U-13).
- **02-04** `b87eb42`: each link line gives its check age; a verified line past
  30 days adds the re-check hint ("proof gone" lines do not, U-8).
- **02-05** `5a91964`: weighted results and the score page drop every identity
  link, so a link never changes a score (CORE-6/6x, IR-14/15).
- **02-06** `5cefa24`: `infer people` shows each person's published links
  offline, matched by account number, else last-seen login (U-6); a rename flag
  still never becomes a link. `scrape person` shows the lines without refreshing (U-10, U-14).
- **02-07** `34fe0e9`: `scrape github <user>` re-checks that login's published
  links on the one profile read and shows a lost proof; an account-number
  mismatch records nothing and leaves it to `identity verify` (U-4, U-5, U-15).
- **02-08** `e1f561a`: network search labels link rows `authority not checked
  here` (link rows only, U-7), still listed and attributed to the author.

Test pins after the steps: `0987739` (record-ID line shape, links-only search
label), `b577f6b` (a non-link `github:` row stays unlabelled).

DES phase records: `53f7805`, `be2f4f5`, `23247e8`, `2e1f0d1`, `393eea5`,
`9a598f7`, `a232bef`, `c8ee1f5`.

## Phase-3 refactor

- `c07bfe9`: one listed-link read shared by `identity list` and the person views.

## Phase-4 adversarial review

**Iteration 1: REJECTED (U-17).** Findings that held:

- **B1**: `identity list --did` changed a link's standing (narrowing ran before
  the lineage fold).
- **B2**: a peer's own retraction or supersession was ignored, so the withdrawn
  link still listed.
- **B3**: the U-5 "profile decides" property was circular: its oracle reused the
  code under test.
- **M2**: domain rules (which login a re-check reads, a person row's key) lived
  in verbs.
- H2 (no end-to-end refresh fail-closed scenario) turned out not to be a real gap.

One revision pass. RED scenarios and hardening first (`daed351`: B1, B2, H1, H2,
M1, L1), then the fixes: `7e33808` (B2), `b70ff22` (B1), `d88adfc` (B3, a
literal oracle), `622faed` (M2).

**Iteration 2 of 2: fixes verified, REJECTED only on mandate-9 contract-shape
tags** (adapter and claim-domain tests, "Outcome anchor" lines). Overruled
(U-18): mandate 9 is not enforced in this repo (no checker, no `Outcome anchor`
anywhere, slice 1's accepted tests carry none). The allow-list diff (G1) was
checked: three `#[ignore]` deletions under `tests/acceptance`. F2 (stale
"ignored" comments) fixed in `212c377`.

**User rulings (2026-10-17):**

- **U-16 out of scope**: the graph walk treats a `sameAs` row as an ordinary
  traversal edge; traversal is unchanged.
- **U-19 hide removed peers' links** (re-review F1): a soft-removed peer's links
  are not candidates for any reader (list, verify, person views, refresh). RED
  `39095c4` (IR-20, IR-20b); fix `c9c9377`, one clause in `LINK_CANDIDATES_SQL`,
  the single candidate source.

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED after two revision rounds: 8 steps, phase 02 only |
| DES integrity | `des-verify-integrity`: all 17 steps complete; 8/8 slice-2 steps COMMIT/PASS |
| Phase-3 refactor | `c07bfe9` |
| Phase-4 adversarial review | Iteration 1 rejected (B1, B2, B3, M2), fixed. Iteration 2 found the fixes correct; rejection on mandate-9 tags overruled (U-18). Addendum U-19 delivered |
| Phase-5 mutation (per-feature, gate ≥80%) | **140 mutants** over the slice-2 diff: **99 viable, 99 caught (100%)**, 41 unviable. Gate PASSED |
| check-arch | OK |

### Mutation tooling note

As in slice 1, cargo-mutants 25.3.1 ignores cross-package test scoping, so the
whole slice-2 diff was run with the manual runner (each mutant's diff applied in
a worktree, the relevant `cli` tests run). U-19's SQL clause is not mutable by
cargo-mutants; the orchestrator instead checked that the real-DuckDB tests in
`adapter-duckdb/tests/link_candidates.rs` fail against the pre-fix query.

## Load-bearing invariants

- **Display-only, now enforced**: `score(F) == score(F \ identity-link rows)`;
  the scorer drops them before triangulation. No subject merging.
- **One candidate source**: every identity reader takes links from
  `LINK_CANDIDATES_SQL`, which folds owner and peer retraction/supersession and
  excludes removed peers. Fixing a reader rule there fixes every reader.
- **Narrowing never changes standing**: `--did` and `--github` filter after the
  lineage fold.
- **One profile read**: the person-scrape refresh parses the already-fetched
  body strictly; never a second GET, never the lenient person parser.
- **Id-keyed refresh**: a refresh records only when the profile's account number
  matches the link's; otherwise `identity verify` decides.
- **Offline person views**: link lines come from this reader's last checks; only
  `scrape github <user>` and `identity verify` write checks.

## Deviations: planned vs shipped

- The scorer drop sits in `scoring::score`, not `authority::kind` /
  `score_attested` as ADR-085 §6 sketches (neither exists yet, U-3).
- Duplicates read as one line naming every record ID, not the newest
  `composedAt` pick of GDL-D9 (U-9).
- The search label covers link rows only, not every `github:` row (U-7).
- `scrape person` shows link lines but does not refresh (U-10).
- 02-03 and 02-06 touched files outside their lists (U-13, U-14); the 02-07
  move into `identity_reads` is partial (U-15). All read-only, accepted.

## Known deferrals and follow-ups

- **F1** resolved by U-19.
- **F3**: `login_to_read` has no `now` cap (callers pass SQL-capped checks).
- **F4**: `--github` may also select a link whose last-seen login was reused.
- **F5**: a CID in both `claims` and `peer_claims` yields duplicate referrer rows
  (standings unaffected).
- **U-7**: widen the search label to every `github:` row.
- **U-10**: `scrape person` does not refresh checks.
- **U-20**: `peer add` after a plain `peer remove` returns `AlreadyExisted` and
  leaves `removed_at` set, so the peer stays unfollowed and its links hidden
  (peer-feature follow-up). `infer people` is not pinned for removed peers by an
  acceptance test (same candidate read).
- Contract-shape (mandate-9) tags, if that mandate is ever enforced here.
- Slice 3: review app "Publish this link"; `self_attested_link_record` still has
  no production caller.

## Lessons

1. Filter after folding. B1 came from narrowing before the lineage fold; any
   reader that both narrows and folds needs a test that narrowing leaves the
   standing unchanged.
2. A property whose oracle calls the code under test proves nothing (B3). Write
   the oracle as literal expectations.
3. A single candidate source paid off: U-19 was one SQL clause for every reader.
4. Some fixes are out of the mutation tool's reach (SQL text); check them by
   running their tests against the pre-fix code.

## Try it

```sh
openlore identity list                     # lineage folded; record IDs, check age, re-check hint
openlore identity list --did <did>         # one person's links
openlore infer people                      # person blocks show their link lines, offline
openlore scrape github <user>              # also refreshes that login's link checks, one profile read
openlore search --subject <did>            # network link rows: "authority not checked here"
```

## Lasting artifacts

- `docs/architecture/github-did-identity-link/architecture-design.md`
- `docs/scenarios/github-did-identity-link/walking-skeleton.md`
- ADR-084, ADR-085 and the ADR-076 amendments in `docs/adrs/`
- Slice-2 rulings: `docs/feature/github-did-identity-link/deliver/upstream-issues.md`

## Commit trail

Roadmap `d4a811f`, `53242f3`, `c424e31`, `d2672a2`, `ecfe881`, `f2724ad`,
`6268450`, `b577f6b`; steps `c30ed35`, `3b8e515`, `f10e455`, `b87eb42`,
`5a91964`, `5cefa24`, `34fe0e9`, `e1f561a`; pins `0987739`; refactor `c07bfe9`;
review RED `daed351`, fixes `7e33808`, `b70ff22`, `d88adfc`, `622faed`; rulings
`ba0ef89`, `99326a5`, `212c377`, `8fb06ac`; U-19 `39095c4`, `c9c9377`,
`04e28bf`. All on `main` (trunk-based, no PR). Not yet pushed or deployed.
