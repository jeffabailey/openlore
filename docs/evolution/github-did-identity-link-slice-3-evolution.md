<!-- markdownlint-disable MD013 -->
# Evolution: github-did-identity-link (slice 3: review app publish and retract)

> The THIRD and last of three slices of `github-did-identity-link`. Slices 1 and
> 2 are in `github-did-identity-link-slice-1-evolution.md` and
> `github-did-identity-link-slice-2-evolution.md`. Paradigm: functional Rust
> (ADR-007). Companion design: ADR-084 (§4 review-app guard), ADR-085, ADR-008
> (retraction shape), ADR-073/074 (sessions), ADR-086/087 (owner-only-signing).
> Slice-3 rulings: `deliver/upstream-issues.md` U-21..U-44.

## Summary

Slices 1 and 2 gave the CLI a way to publish, re-check and read GitHub links.
Slice 3 brings the same to the review app. A signed-in owner whose GitHub step
is verified gets "Publish this link" on `/github`. The preview reads GitHub
itself and shows `<did> sameAs github:<login>`, the profile URL, the id URI, and
that the record is public and that retracting withdraws it without erasing it.
The confirm reads GitHub again and writes exactly the previewed link: a
self-attested record, create-only, keyed by its CID. If the fresh read
disproves the link, or GitHub cannot be read, nothing is written and no plan is
kept. A failed write keeps the plan for one Retry. A write the PDS may have
stored says the link "may have been saved"; a definite refusal says "Nothing was
published." "Retract this link" on `/github` retracts an Active link through the
shipped `/retract` path, and the link record stays. Links stay off the public
profile and the share draft.

### What shipped (one paragraph)

`review-domain` gained guarded plan constructors: `publish_plan`,
`publish_edited_plan` and `restore_plan` refuse reserved-predicate records
(ADR-084 §4, U-24). It also gained a separate `LinkPlan` (`link_plan`,
`restore_link_plan`, `link_preview`, and the pure confirm decision), keyed by the
record's own triple and stored under kind `publish` with no review-store
migration (U-21 B). `published_claims` drops link records, so the profile and
the share draft never list one (U-27). `prove_ownership` now delegates its
account rule to claim-domain's `account_named_by` (U-25). The new
`routes/github_link.rs` holds the preview, confirm and Retry routes, behind
same-origin and signed-in guards. It spends the verify budget, and check-arch
keeps it away from `GithubLinkPort`, `restore_plan`, `owner_can_publish`,
`mark_published` and `change_state`. Link refusals get their own exhaustive
wording (`LinkRefusal`, U-31). A link retraction has the ADR-008 shape: empty
subject, predicate and object, plus the `retracts` reference. A record that
carries a `retracts` reference never projects as a link (U-39).

### Wave timeline

- DISCUSS: none (as slices 1 and 2).
- DESIGN / DISTILL: shared with slice 1 (2026-10-14, `a6735f7`). RED scenarios
  RA-9..RA-14 and the RA-5 rows were added for the roadmap review (`5708242`,
  `e536e06`). RA-4b and RA-12b were added after the re-review (`49406a0`).
- DELIVER: 2026-10-17. Roadmap `dd0fa59`, user rulings `596e97e` (U-21, U-27),
  revision `2678b56`, closure `0346107` (U-36: B-1 closure table, C-1..C-9).
  Phase 03 approved with 9 steps, all COMMIT/PASS. `des-verify-integrity`: all
  26 steps (slices 1 to 3) have complete traces.

## DELIVER steps (5-phase TDD, functional crafter)

- **03-01** `2669607`: claim plans refuse the reserved link predicate. A
  separate link plan restores only as the owner's own link (U-21 B, U-24).
- **03-02** `0b9400f`: the profile and the share draft never list a link record
  (U-27).
- **03-03** `8c141fd`: refactor only (RED approved-skip, green before and
  after). The ownership proof uses the identity-link account rule (U-25).
- **03-04** `9c15e62`: a verified owner previews "Publish this link" from a
  fresh read (RA-1a, RA-7, RA-9, RA-10, RA-10d, RA-11a/b, RA-14). Deviations
  recorded in U-37.
- **03-05** `02fe564`: confirming re-reads GitHub and publishes exactly the
  previewed link (RA-1b, RA-6, RA-13, RR-2a/b). Addendum `9c46265` (U-38):
  sessions expire by the app's clock, so RA-8 runs.
- **03-06** `d194031`: a failed link write keeps the plan for one Retry, and the
  Retry posts to the link confirm (RA-12 rows 1 and 2, U-34).
- **03-07** `365ffb0`: a confirm whose fresh read disproves the link says the
  proof is gone and keeps no plan (RA-2).
- **03-08** `639a5b0`: a confirm that cannot read GitHub says why and publishes
  nothing (RA-5, four rows).
- **03-09** `5485cde`: Priya retracts her published link from the GitHub step
  without erasing it (RA-4). U-39 (link retraction shape) and U-40 (the fake PDS
  fails requests on open connections too) were ruled during the step. Its GREEN
  failed twice on PR-3, a gap in the fake, before it passed.

DES phase records: `1e1a06b`, `39c428c`, `aa69cd9`, `adb0d0d`, `bac70ac`,
`bf5e0b7`, `f0c1c23`, `7522f8c`, `5cbcbe9`.

## Phase-3 refactor

- `65e34cb`: one claim-fields view, one retraction test, one plan key.
- Carried from U-37: `b9c9a9f` gives a preview that could not be saved its own
  wording (it used to read "GitHub could not be reached"). `c4a3f8f` asserts that
  a stored link plan never appears in the review queue (C-6). `aeb103c`: the
  retract preview names the claim being retracted.

## Phase-4 adversarial review

**Iteration 1: REJECTED (U-41).** Findings:

- **F-1 (blocker)**: the 03-03 move dropped the converse half of
  `only_github_username_shaped_text_is_a_login`. Fixed in `b3f67e3`: the login
  rule is pinned by its literal spelling again.
- **F-2/F-3 (high)**: the C-6 test re-composed the queue instead of reading the
  one `queue_page` renders (`50fde80`). The session tests read the production
  lifetime constant; the 30 days are now pinned as 2 592 000 seconds (`89caa18`).
- **F-4..F-6 (medium)**: a refused verify or link preview lost the "Retract this
  link" offer (`adbc648`). A reference-only link retraction reached the score
  feed (`f45c0ee`). The ingest client's repo listing had no timeout on a silent
  PDS (`949e43b`).
- **F-7..F-14 (low)**: the xtask link executor is held to the fresh-read rule
  (`b561199`). The build profile follows `debug_assertions` (`2280e15`). A web
  session expires 30 days from sign-in, not after 30 idle days (`d15adbf`,
  `6841a0a`). The keep-plan universe is every `LinkNotPublished` variant
  (`da584c5`). F-10 was kept on purpose: auth-request expiry stays on the DB
  clock. F-11 went to the backlog. F-13 was explained by U-40.
- **F-12** went to the user, who ruled (**U-42**) that after a failed write the
  page says the link "may have been saved" and that Retry will confirm. RED
  `dd1d678`, fix `a2b7315`.

**Re-review: REJECTED (U-43).**

- **N1 (high)**: `a2b7315` sent every `RepoWriteError` to "may have been saved",
  including a definite `400` and an unreachable PDS. Orchestrator ruling: "may
  have been saved" only after a 5xx answer or a transport failure once the
  request was sent. A 4xx other than "already exists", or a failure to connect,
  keeps "Nothing was published." RED `49406a0` (RA-12b, RA-4b). The fake now
  refuses the connection for an unreachable host (`37439c5`). Fix `7193a02`.
- **N2 (medium)**: no guard kept the retract offer after a refused POST. RA-4b
  now pins it (`49406a0`).
- **N3..N8 (low)**: ADR-074 wording (`c013c2c`), the variant-count pin and an
  overstated comment (`15f019f`), a check-arch scan that passed silently on a
  missing file (`af2f1a0`), a view-level `github_page` that passed no links
  (`bc504d6`), the score-filter doc (`ca783d5`), and guidance when Retry cannot
  be offered (`234aba9`).

**Final re-review (U-44).** N1..N8 are fixed on the link path. NF-2/NF-3 (a
stale comment and a stale doc) were fixed in `ba3531e`. **NF-1 is out of
scope**: the claim publish, retract and share pages still say "Nothing was
published." after a lost answer or a 5xx refusal. That behaviour predates slice
3, and U-42/U-43 cover links only, so NF-1 goes to the user as a follow-up.
NF-4/NF-5 went to the backlog.

**Rulings this slice:**

- User: U-21 → **B** (link plans under kind `publish`, no migration;
  conditions F-02, F-03, F-08). U-27 → **hide** links on the profile and in the
  share draft for now. U-39: a link's retraction is reference-only (ADR-008
  shape). U-42: "may have been saved" wording.
- Orchestrator: U-38 (sessions use the app's clock; the debug seam covers 30
  days; release builds refuse it). U-40 (fix the fake, not production pooling).
  U-43 (which failures may have been stored). U-44 (NF-1 out of scope).

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED after 2 iterations (U-36 closure table, C-1..C-9): 9 steps, phase 03 only |
| DES integrity | `des-verify-integrity`: all 26 steps (slices 1 to 3) have complete traces; 9/9 slice-3 steps COMMIT/PASS |
| Phase-3 refactor | `65e34cb`, plus the U-37 carry-overs `b9c9a9f`, `c4a3f8f`, `aeb103c` |
| Phase-4 adversarial review | Iteration 1 rejected (F-1 blocker), revised. Re-review rejected (N1 high, N2), fixed. Final re-review: N1..N8 fixed; NF-1 ruled out of scope (U-44) and sent to the user; NF-2/NF-3 fixed |
| Phase-5 mutation (per-feature, gate ≥80%) | **182 mutants** over the slice-3 production diff `4164329..6e11436^`: **103 caught, 4 missed, 75 unviable** (`Default::default()` for types with no `Default`). **96.3% of viable mutants caught**. Gate PASSED. The 4 survivors were refusal log labels, now pinned by `6e11436` |
| Mutation re-run on the N1-fix files | TODO: result of the re-run on the N1-fix files (in progress) |
| check-arch | OK, with the link-route rules (U-26, 03-05) and a missing source file now an error (`af2f1a0`) |

### Mutation tooling note

As in slices 1 and 2, cargo-mutants 25.3.1 ignores tests that live in another
package, so the slice-3 diff was run with the manual runner: each mutant's diff
is applied in a worktree and the relevant tests are run. The 75 unviable mutants
replace a return value with `Default::default()` for types that have no
`Default`, so they do not compile.

## Deploys

- Review app deployed at `bf5e0b7` (steps 03-01..03-06), digest
  `sha256:7a97a1f9...`. Post-deploy checks passed. This respected U-35: no
  deploy between 03-04 and 03-05.
- The slice-3 tip `ba3531e` is pushed. Its review-app deploy runs after CI.
  TODO: record the review-app digest for `ba3531e` once deployed.

## Load-bearing invariants

- **Every publish is proven in its own request**: the preview and the confirm
  each read GitHub (2 reads per publish, both from the verify budget). The proof
  is the bio, never the `github_links` row.
- **Link plans are kept apart from claim plans**: `restore_plan` refuses
  reserved-predicate records, so a link plan confirmed through `/review/publish`
  is used up and writes nothing (RA-14). The link confirm never touches
  suggestion bookkeeping.
- **Only a write that may have been stored says "may have been saved"**: a 5xx
  answer, or a transport failure after the request was sent. Every other link
  refusal says "Nothing was published." Retry rewrites the same CID-keyed
  record, so exactly one link remains.
- **No plan survives a failed proof or read**: a disproving or unreadable
  confirm keeps no plan. Only a failed write keeps one, for one Retry.
- **Links stay off the profile and the share draft**: one choke point,
  `published_claims`. `live_published_claim` keeps them so that `/retract` works.
- **Retraction never erases, and is not itself a link**: a link retraction is
  reference-only. A record that carries `retracts` never projects as a link, and
  the scorer drops a reference-only link retraction.

## Deviations: planned vs shipped

- No `PlanKind::IdentityLink` and no review-store v2 migration. Link plans use
  kind `publish` (U-21 B).
- No `github.link.publish.ok/refused` counters. The app logs
  `github.link.published` and `github.link.refused` events, labels only; the
  counters are deferred to a KPI pass (U-29).
- An already-published Active link still gets the publish offer (U-23). Readers
  fold the duplicate into one line.
- 03-04 deviations accepted in U-37. A disproving preview does not mark the
  stored link unverified.
- Link retraction reuses `/retract`, counts `retract.posted` and attempts a
  no-op suggestion state change (U-28).
- Web sessions now take the app's clock at the edge (U-38). Production behaviour
  is unchanged.

## Follow-ups

- **U-44 NF-1**: the claim publish, retract and share pages still say "Nothing
  was published." after a lost answer or a 5xx refusal. For the user to decide.
- **F-6**: the review app's ingest client is unguarded; the decision on a
  guarded constructor is still open.
- **NF-4/NF-5**: the fake PDS listener rebind (and a unit-test fixture) can race
  a parallel test for the released port.
- **F-11**: the fake has no posture for a connection refused at connect time
  beyond closing the listener (partly done in `37439c5`).
- **U-20**: `peer add` after a plain `peer remove` does not re-follow.
- Carried from slice 2: F3, F4, F5, U-7 (widen the search label), U-10
  (`scrape person` refresh). Owner-only-signing: link execution takes the exempt
  IdentityLink `SigningPermit` when ADR-086/087 lands (U-33).

## Lessons

1. Moving a property can drop half of it. F-1 was the converse of a login rule,
   lost in a refactor-only step. When a test moves, check both directions of
   every property it pinned.
2. "Nothing was published" is a claim about the remote side. After a 5xx answer
   the app cannot know, so the wording has to follow what the PDS may have done
   (U-42, U-43). The same gap is still open on the claim pages (NF-1).
3. Test fakes must fail the way real hosts fail. Two regressions (PR-3, U-40;
   then N1's refused-connect case) came from a fake that checked reachability
   only when it accepted a connection.
4. A shape copied with `..original.clone()` carried the link triple into its
   retraction (U-39). Build records that carry a reserved predicate from an
   explicit shape, never from a spread.

## Try it

On the review app, signed in, with the GitHub step verified:

1. Open `/github` and choose "Publish this link". The preview shows the record
   and writes nothing.
2. Confirm. GitHub is read again and the page shows the link's `at://` address.
3. Back on `/github`, choose "Retract this link" and confirm. A retraction is
   added and the link record stays.

```sh
openlore peer pull                 # a reader pulls the app-published link
openlore identity list             # shows it as the owner's unchecked claim
openlore identity verify --github <login>   # this reader checks it
```

## Lasting artifacts

- `docs/architecture/github-did-identity-link/architecture-design.md`
- `docs/scenarios/github-did-identity-link/walking-skeleton.md`
- ADR-084, ADR-085, the ADR-076 amendments and the ADR-074 session wording in `docs/adrs/`
- Slice-3 rulings: `docs/feature/github-did-identity-link/deliver/upstream-issues.md` (U-21..U-44)

## Commit trail

Roadmap `dd0fa59`, `596e97e`, `5708242`, `e536e06`, `2678b56`, `0346107`; steps
`2669607`, `0b9400f`, `8c141fd`, `9c15e62`, `39c5e0b`, `02fe564`, `5bbe46a`,
`9c46265`, `d194031`, `365ffb0`, `639a5b0`, `3ee3251`, `5845e9e`, `5485cde`;
refactor `65e34cb`, `b9c9a9f`, `c4a3f8f`, `aeb103c`; review `81f7de3`,
`b3f67e3`, `50fde80`, `89caa18`, `adbc648`, `f45c0ee`, `949e43b`, `b561199`,
`2280e15`, `d15adbf`, `6841a0a`, `da584c5`; U-42 `78147ba`, `dd1d678`,
`a2b7315`; re-review `49406a0`, `04cf0d2`, `37439c5`, `7193a02`, `234aba9`,
`15f019f`, `bc504d6`, `c013c2c`, `af2f1a0`, `ca783d5`; mutation survivors
`6e11436`; final rulings `ba3531e`. All on `main` (trunk-based, no PR). Pushed
through `ba3531e`.
