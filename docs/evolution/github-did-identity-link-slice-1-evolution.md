<!-- markdownlint-disable MD013 -->
# Evolution: github-did-identity-link (slice 1: CLI `identity link` / `verify` / `list` / `forget`)

> The FIRST of three slices of `github-did-identity-link`. Slice 2 (readers:
> `identity list` lineage, person views, search, peer pull) and slice 3 (review
> app "Publish this link") are still to come, so the feature workspace
> `docs/feature/github-did-identity-link/` stays live. Paradigm: functional Rust
> (ADR-007). Companion design: ADR-084 (wire contract), ADR-085 (verification
> reuse, reader semantics), ADR-076 amendments; revised by ADR-086/087
> (owner-only-signing).

## Summary

Slice 1 lets a DID owner say, from the CLI, "this GitHub account is me", and lets
any reader re-check that claim. `openlore identity link github <login>` reads the
account's bio, refuses unless it names the caller's exact DID, and publishes an
`org.openlore.claim` with predicate `sameAs` (subject = DID, object =
`github:<login>`, the numeric account pinned in evidence as
`https://api.github.com/user/<id>`). `identity verify` re-checks each Active link
by account number and appends a check. `identity list` shows each link with this
reader's latest verdict. `identity forget --did` erases one DID's local check
history. Linking is opt-in and owner-only: a third-party `sameAs` is an ordinary
claim and never projects as a link. Association is display-only; scoring drops
every `sameAs` row.

### What shipped (one paragraph)

A pure `claim_domain::identity_link` core (the ADR-076 bio matcher hoisted from
`review-domain`, which now delegates; `ProvenLink` with a `NonZeroU64` `user_id`;
the link lifecycle; projection of only the owner's provenance-passed record). The
DuckDB store moved to v7 with an insert-only `identity_link_checks` table behind
three split ports (`IdentityLinkCheckReadPort` / `WritePort` / `PurgePort`), each
verb getting the narrowest handle, plus a `LinkLineageReadPort` so `identity
verify` holds no storage write or signing handle. `adapter-github` gained
`read_person_by_id` (`GET /user/{id}`, one same-host redirect) with recorded
profile fixtures and the DD-GDL-11 contract test. The shared compose path
refuses the reserved `sameAs` predicate and points to `identity link`; `claim
counter` stays allowed (D-b). check-arch gained the GDL-D13 rules.

### Wave timeline

- DISCUSS: none. Requirements were derived from OD-CPI-1, ADR-076 and the claim
  model, recorded as assumptions A-1..A-10.
- DESIGN: 2026-10-14. GDL-D1..D14, ADR-084, ADR-085, all accepted by the user as
  proposed. Revised by owner-only-signing (UC-1..UC-8, ADR-086/087).
- DISTILL: 2026-10-14, commit `a6735f7`. Acceptance targets `identity_link`,
  `identity_link_core`, `identity_link_readers`, review-app targets; walking
  skeleton WS-GDL-1 (strategy C). User rulings D-a, D-b, D-c, D-g.
- DELIVER: 2026-10-15 to 2026-10-16. Roadmap `f00c99f` (9 steps, slice 1 only).
  All 9 steps COMMIT/PASS; `des-verify-integrity` passed.

## DELIVER steps (5-phase TDD, functional crafter)

- **01-01** `01ee0f9`: v7 check store opens the frozen v6 store unchanged and
  refuses to start when it cannot keep checks; claim-domain matcher; an empty
  `identity list` says so.
- **01-02** `1e488e9`: walking skeleton. `identity link github jeffbailey --yes`
  publishes the proven `sameAs` link and `identity list` shows it verified today.
- **01-03** `99a1ed2`: a link is refused unless a fresh bio read names the exact
  DID; the preview writes nothing; bad logins and unreadable GitHub change nothing.
- **01-04** `9e1d3ce`: link lifecycle. An Active link is not linked twice (D-a), a
  retracted one can be re-linked (D-g), a second account links alongside, and a
  link is kept when the repo host is unreachable.
- **01-05** `039cee0` (+ `cebd457`): generic composers refuse `sameAs` and point
  to `identity link`; `claim counter` may dispute a link; only the owner's
  provenance-passed record projects.
- **01-06** `bd2fd6b`: `identity verify` re-checks each Active link with one
  request, keeps earlier checks, shows a lost proof as `proof gone`, and records
  nothing when GitHub fails.
- **01-07** `5ef0abc`: renames and re-registrations are told apart by account
  number; unfollowable GitHub answers change nothing; recorded fixtures pin the
  profile shape.
- **01-08** `8f9cbf0`: re-checks cover followed peers and narrow by `--did` or
  `--github`; only this reader's sole verified check associates; identity verbs
  leak nothing.
- **01-09** `b240674`: `identity forget --did` erases one DID's local check
  history only, through the separate purge port.

DES phase records: `d1cd189`, `307a162`, `43e48e3`, `d76b7d8`, `8f7786a`,
`738a4e4`, `9f442ad`, `bc126cc`, `adb183c`.

## Phase-3 refactor

- `25c2f61`: `identity verify` reads lineage through a read-only
  `LinkLineageReadPort`; it holds no storage or signing handle.
- `b9616a4`: collapsed the once-per-link dedup and the linked-account check.

## Phase-4 adversarial review

**Round 1: REJECTED on D1, a fail-open.** A GitHub profile with no `id` field was
read as account 0, so a malformed answer could pass as a real account. Fixed in:

- `74564ae`: strict `read_account`, a `MalformedObservation` outcome, `NonZeroU64`
  for `ProvenLink.user_id`, and `id_of_uri` rejecting 0 (also D5, D7).
- `0258e38`: check-arch reader method/slot names, glob imports, split check-table
  SQL (D2, D3).
- `49cbf12`: ADR-085 / ADR-076 amendments for the strict account read.
- `292947a`: fail-closed rows for malformed profiles, invalid stored login and
  account 0; IL-12 failure path (D4, D6, D7).

**Re-review (iteration 2 of 2): REJECTED on N1–N11.** Fixed directly, without a
third review round:

- `0ccccbf`: OW-2 and OW-4b assert the scan control state on the unverified queue (N4).
- `4210644`: ADR-085 states the GitHub login rule (N8).
- `e145b0f`: the CORE-2 oracle states the login rule independently; logins span 1–45 chars (N8).
- `ed64b45`: check-arch uses syn-based glob detection and a token walk for opaque macros; `sameAs` rule split (N1–N3).
- `c31579a`: pure ownership refuses account 0; `observation_of` narrowed; a write-only field dropped (N5, N7, N9, N11).
- **N6 accepted as is**: a malformed GitHub answer is worded "GitHub could not be
  reached". It still fails closed; only the wording is coarse.
- **N10 open**: legacy review-app links may carry `github_user_id` 0. The
  production DB has not been checked.

## Quality gates — final report

| Gate | Result |
|---|---|
| Roadmap | APPROVED: 9 steps, slice 1 only; slice-2/3 scenarios stay ignored (`deferred_to_later_slices`) |
| DES integrity | `des-verify-integrity` PASS; 9/9 steps COMMIT/PASS in `execution-log.json` |
| Phase-3 refactor | `25c2f61`, `b9616a4` |
| Phase-4 adversarial review | Round 1 rejected (D1 fail-open), fixed. Re-review rejected (N1–N11), fixed directly except N6 (accepted) and N10 (open) |
| Phase-5 mutation (per-feature, gate ≥80%) | core claim-domain **106/113 viable caught (94%)**; CLI verbs and render **66/70 (94%)**; adapter-github redirects **9/9**; adapter-duckdb **8/8**. Gate PASSED. All 13 survivors then killed by new tests: `f2dd9d4` (acceptance: check age, declined message, verified recheck line), `e987465` and `56a8d47` (claim-domain unit and property tests) |
| check-arch | OK, with the GDL-D13 rules and the review-driven reader/glob/macro hardening |

### Mutation tooling note

cargo-mutants 25.3.1 ignores `--test-package` and `--test-workspace` for tests
that live in another package. The core, redirect and DuckDB code is tested from
the `cli` package, so for those crates cargo-mutants reported misses that no
relevant test had run against. Those groups were run with a manual runner that
applies each mutant's diff in a worktree and runs the relevant `cli` tests.

## Load-bearing invariants

- **Owner-only, opt-in**: a link projects only when author == subject DID and the
  record passed provenance. Nothing infers a link (bio scan, contribution link,
  rename flag).
- **Fail closed on GitHub data**: a missing, zero or malformed account number is
  `MalformedObservation`, never an account. `ProvenLink.user_id` is `NonZeroU64`.
- **Id-keyed**: links pin the numeric account; renames and re-registrations are
  told apart by account number, never by login.
- **Display-only**: `score(F) == score(F \ sameAs rows)`; no subject merging.
- **Append-only checks**: `identity_link_checks` is insert-only; the latest check
  is a read query; only `identity forget --did` purges, through its own port.
- **Narrow capabilities**: `identity verify` holds read-only lineage and the check
  writer; no signing handle.

## Deviations: planned vs shipped

- **Behaviour change**: the scraper's `PersonSubject::parse` now rejects logins
  over 39 chars, because it shares `is_github_login` with the identity-link core.
- The ADR-085 sketch `Disproved { reason, at }` is left as is (not revised to
  match the shipped code).
- Mutation testing for cross-package-tested crates used a manual runner (above).

## Known deferrals

- `identity list` does not fold the retraction lineage (slice 2).
- `self_attested_link_record` has no production caller (slice 3).
- The review-app changes (strict `read_account`) are not yet deployed.
- N10: legacy review-app links with `github_user_id` 0 are unchecked in the
  production DB.

## Lessons

1. The D1 fail-open was caught only by adversarial review. Parsers of external
   identity data must default to strict: a missing field is an error, not a zero.
2. Check that cargo-mutants' test scoping reaches the tests that cover the code
   before trusting "missed" counts.
3. Deleting a mutation output dir mid-run lost a group's results. Leave output
   dirs alone until the run ends.

## Try it

```sh
openlore identity link github <login>          # preview, then asks; only `y` publishes
openlore identity link github <login> --yes    # bio must name your DID
openlore identity list                          # link + latest verdict and age
openlore identity verify --github <login>       # re-check; appends a check
openlore identity forget --did <did>            # erase that DID's check history
```

## Lasting artifacts

- `docs/architecture/github-did-identity-link/architecture-design.md`
- `docs/scenarios/github-did-identity-link/walking-skeleton.md`
- ADR-084, ADR-085 and the ADR-076 amendments in `docs/adrs/`

## Commit trail

`a6735f7` (DISTILL), `f00c99f` (roadmap), steps `01ee0f9`, `1e488e9`,
`99a1ed2`, `9e1d3ce`, `039cee0`, `cebd457`, `bd2fd6b`, `5ef0abc`, `8f9cbf0`,
`b240674`; refactor `25c2f61`, `b9616a4`; review round 1 `74564ae`, `0258e38`,
`49cbf12`, `292947a`; re-review `0ccccbf`, `4210644`, `e145b0f`, `ed64b45`,
`c31579a`; mutation survivors `f2dd9d4`, `e987465`, `56a8d47`. All on `main`
(trunk-based, no PR).
