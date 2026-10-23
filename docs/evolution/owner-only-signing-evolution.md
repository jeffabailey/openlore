<!-- markdownlint-disable MD013 -->
# Evolution: owner-only-signing

> Slices 1 to 5 of `owner-only-signing`, all 25 roadmap steps. The feature
> directory `docs/feature/owner-only-signing/` stays in place as delivery
> history. Paradigm: functional Rust (ADR-007). Decisions: ADR-086 (subject
> authority model, attested versus third-party claims) and ADR-087 (authority
> proofs: linked GitHub owner and in-repo authority file). It builds on
> github-did-identity-link (ADR-084/085) and the review-app publish permit
> (ADR-089). Job: J-011 "Speak for my own accounts and repos". DELIVER
> rulings: `deliver/upstream-issues.md` (U-OOS-D1..D35).

## Summary

The user asked that anyone can import claims about any GitHub account or
repo, but only the account's owner, or a repo's owner or admin, can sign them.
Before this feature, any OpenLore user could sign and publish a claim about
anyone's `github:` account or repo, and every claim counted toward weight.
Now:

- **Anyone imports, only the owner signs.** `scrape` and `infer` still give
  everyone unsigned candidates about any public `github:` subject. Signing a
  `github:` assertion needs a `SigningPermit`, which only
  `claim_domain::authority` can mint.
- **Who may sign.** For a person, the signer's DID must be linked to that
  GitHub account (ADR-084 link, verified bio). For a repo, the signer must
  be linked to the user who owns it, **or** be listed one DID per line in the
  repo's `.github/openlore-authority` file. The file is the only route for
  organization repos. This is broader than GitHub's "admin" role (OOS-D5).
- **Signing stays offline.** The gate reads the latest local check. A check
  older than 7 days is refused, and the refusal names
  `openlore authority verify --subject <s>` (an accepted I-9 exception,
  ADR-086 §5). Online verbs (`scrape … --sign`) refresh first. If GitHub
  cannot answer, they fall back to the latest check, just as offline.
- **Counter-claims, retractions and identity links are exempt.** They need no
  authority and are never weighted.
- **Readers say whose word a claim is.** Every `github:` claim is labelled
  `owner`, `repo-listed`, `third-party` or `authority not checked`, from
  the reader's own local checks. Authority is reader-local and never
  federated. Only attested assertions move the weight.

### What shipped (one paragraph)

The pure core is `claim-domain::authority`: `requirement`, `classify`, and
the gate `may_sign_from_checks` / `may_sign_repo_as_served`, plus
`republish_permit` and the four labels. Every composer (`claim add`,
`infer people --sign`, `scrape github/person --sign`, `sign batch`) signs
through `sign_permitted` under a permit. So does `claim publish` of a claim
never published before. `GithubPort::read_repo_authority` reads the repo and
its authority file as one observation, and a recorded-fixture contract test
covers it. Store **v8** adds insert-only `repo_authority_checks`, and store
**v9** adds insert-only `imported_candidates`. `openlore authority verify`
re-checks person and repo authority from public data. With `--subject`, it
checks a repo that no claim names yet (D12). `score_attested` is the only
scoring entry for the CLI and the viewer. The share draft in the review app
re-reads authority before it quotes a claim (P-OOS-2). The new `candidates list`
reads imports offline. check-arch gained `repo_authority_checks_insert_only`,
`authority_readers_hold_no_write_handle`,
`imported_candidates_not_in_scoring_or_inference`,
`scoring_callers_use_score_attested` and `sign_only_with_permit`.
`adapter-github` joined `INDEXER_FORBIDDEN_DEPS`, so the indexer gains no
GitHub capability (C-OOS-8).

### Reader surfaces

- **`graph query`:** each `github:` claim gets a `whose word: <label>` line of
  its own, so DD-FED-10 parsers keep working. Pre-feature claims are judged by
  the same rule (RD-8).
- **`graph query --weighted`:** counts only attested assertions. It ends with
  a footer: `N not counted: <counts per reason>. Re-check with
  \`openlore authority verify\`.`
- **`--include-unattested`:** adds a per-author section and leaves the
  weighted rows unchanged. It requires `--weighted` (P-OOS-1).
- **Viewer and `infer people`:** the viewer's project page and `/search`
  page, and `infer people`, show the same labels and weights as the
  terminal.

## Wave timeline

- DESIGN (2026-10-14): OOS-D1..D15, A-1..A-14, C-OOS-1..9. ADR-086/087 were
  approved by the user as proposed, including A-11 (persist imported
  candidates) and the I-9 exception. Review iteration 1 rejected the design (1
  critical, 8 high). All findings were fixed. The upstream changes are in
  `upstream-changes.md` (UC-1..UC-21); ADR-022's default was narrowed (UC-11).
- DISCUSS (2026-10-20, written in parallel with DISTILL): stories S-OOS-1..9.
  The user added J-011 to `docs/product/jobs.yaml`.
- DISTILL (2026-10-20): walking skeleton WS-OOS-1 and CLI, viewer and
  review-app suites. Four-reviewer gate: rejected, revised, then approved in
  iteration 2. The user accepted rulings P-OOS-1..3 and acknowledged the
  Mandate-8 waiver.
- DELIVER (2026-10-20 to 2026-10-21): the roadmap of 25 steps (6 + 11 + 5 +
  1 + 2) was approved after two revisions. All 25 steps reached COMMIT PASS
  in `execution-log.json`, and `des-verify-integrity` passes. 02-06 had a
  failed RED. 02-07 and 02-09 were each blocked once on a later step
  (U-OOS-D14, D15), then went green. Then came refactors, the review, one
  revision pass and mutation.

## DELIVER steps

- **Slice 1, person gate** (01-01..01-06): walking skeleton, `claim add` signs
  only about your own account (`f3b23a9`); weightless kinds follow references
  (`3130ca0`); refusals say why and who can sign (`f52c227`); the 7-day window
  (`beb8bec`); inferred adherences only about your own account (`bbc97da`);
  signing from an import re-checks your own link (`3bdf647`).
- **Slice 2, repo authority** (02-01..02-11): v8 checks (`454eb11`); repo plus
  authority file as one observation (`14eeb08`); `authority verify` (`abdb6fd`);
  records nothing when GitHub cannot answer (`f721e9b`); every sign site
  through `sign_permitted` (`4a9162e`); shipped suites seed authority
  (`b913611`); `claim add` repo gate (`9a3ed7a`); renamed and missing repos
  refused (`d1d9996`); `scrape --sign` observes and gates (`19a1f28`); org repos
  only through an exact listing (`3987e21`); `claim publish` permit (`5efc74c`).
- **Slice 3, readers** (03-01..03-05): `score_attested` (`bcdc168`); `whose
  word` labels (`beee8bb`); the not-counted footer (`611272a`); viewer and
  inferred people (`de516d2`); network search (`c60b5bd`).
- **Slice 4, review app** (04-01): share draft quotes only claims whose owner
  tier holds now (`6bd6d0d`).
- **Slice 5, imported candidates** (05-01..05-02): store v9 and `candidates
  list` (`9b8941d`); a re-import lists each candidate once (`6b61920`).

## Key decisions

- **P-OOS-1:** `--include-unattested` without `--weighted` is a usage mistake.
  It exits 2 with "--include-unattested requires --weighted" and prints no
  results.
- **P-OOS-2:** in the review app, "Attested" means the owner tier holds now.
  The share request re-reads the bio and each quoted repo's owner, as publish
  does (ADR-089 §4). Any other claim is listed as "not counted: authority not
  proven".
- **P-OOS-3:** `candidates list` shows each (subject, predicate, object) once,
  with the latest import's confidence, evidence and time. The table stays
  insert-only, and the de-duplication is in the read query.
- **Mandate-8 waiver** (acknowledged by the user, 2026-10-20): the secondary
  guardrails read store tables directly (GDL D-e precedent). They are
  fail-closed and never the only evidence for a Then.
- **J-011** was added by the user. The J-004 anxiety text now says anyone may
  import, but only the owner or a listed maintainer signs.
- **D8, one reason rule** (`fef514d`): the gate and the readers now share one
  person-link rule. If a link's latest check is not verified, it reads
  `LinkNotVerified` whatever login it saw. `LoginNotCurrent` needs a verified
  check under another login. `reader_link_standing` is gone.
- **D27, "not checked" wording** (`fef514d`, `5ce6cd0`, `fa778b8`): a reader
  with no identity link for the author at all has not checked, so the claim
  reads `authority not checked`. It reads `third-party` only when the
  author's links name other accounts. The viewer's `/search` now labels these
  rows too.
- **Withdrawn claims keep their weight** (ADR-022, WD-85; `49b2a30`, D25). The
  reviewer said a withdrawn third-party claim should leave the not-counted
  footer. The premise was rejected: a withdrawn claim is shown, never silently
  subtracted, and a retraction acts only through the lineage. `3d80b2d` was
  reverted for weights. Its malformed-subject footer line was kept.
- **D33:** labels carry no check age, and age never changes a label.

## Review

The adversarial review returned **NEEDS_REVISION**. One revision pass followed
(`2534501` to `fa778b8`, dispositions `d0de5f6`, `cf9994f`):

- Fixed D8, D25 and D27 (above).
- Fixed D17's follow-on (`7c46e7e`): every stored-claim read finds its
  artifact by CID under the store.
- Fixed D19's oracle (`26075a7`).
- Added adapter tests for imported candidates (`0b4f7d7`, D29).
- Strengthened vacuous and tautological oracles (`4c996a8`).
- Added the publish rename fix (`74c0ac9`) and usage errors instead of crashes
  (`2534501`).
- Logged D30 to D35 as review findings (below).

The refactors before the review (`07f7c72`, `c7f3140`, `e6cf7c0`) gave the
readers one evidence path, and composers one sign-time gate (D9, D20).

## Quality evidence

- **Tests:** cli 1183 passed, review app 293, unit 847.
- **Steps:** 25/25 COMMIT PASS, and `des-verify-integrity` passes.
- **Mutation** (per-feature, gate 80%, `deliver/mutation/mutation-report.md`;
  the scope is the diff from `6fc64b9` to `cf9994f`):
  - The baseline was **76.1% (134/176, WARN)**. `authority.rs` alone was
    72.6%.
  - All 37 survivors were real gaps, and none was equivalent. Crate-level
    tests in `bf63865` killed them: **100% (176/176)**.
  - No production behaviour changed.
  - `xtask/check_arch.rs` and `cli/src/render/*.rs` were optional and not
    measured: the baseline timed out on xtask, and an environment failure
    unrelated to this feature affected `appview_search`.

## Behaviour changes users will notice

- **Weighted views count fewer claims.** Only attested `github:` assertions
  and assertions needing no authority carry weight, so cross-author
  triangulation becomes mostly the owner plus listed maintainers. The footer
  says what was left out and how to re-check.
- **Signing refuses third-party subjects.** `claim add`, `scrape … --sign`,
  `infer people --sign` and `sign batch` refuse a `github:` assertion about
  someone else's account or repo. The refusal names who can sign. A malformed
  `github:` subject is refused rather than signed ungated (D11).
- **New footer line.** The weighted footer discloses `N malformed GitHub
  subject`. When there are none, the footer is byte-identical.
- **Usage errors instead of crashes.** A bare `openlore search` with no
  dimension, and `graph query` explorer flag combinations the dispatch does
  not serve, used to reach `todo!()`. Both now exit 2 with a message naming
  the flags to use.
- **Renamed or transferred repos.** `claim publish` refuses a never-published
  repo claim under a name GitHub no longer serves. Already-published claims
  still re-publish idempotently.
- **Peer artifacts must live under `peer_claims/`.** Stored-claim reads find
  artifacts by plain CID under `claims/` or `peer_claims/`. A traversal CID,
  or a peer path outside `peer_claims/`, is refused before any join. A moved
  or restored store still reads its claims.
- **Store upgrade.** The store migrates v7 to v8 to v9 once, forward only.
- **The review app's share draft quotes fewer claims.** A claim about a repo
  the user no longer owns is listed as not counted.

## Deferred

- **Resolved after finalize:** U-OOS-D30 in 5ada65b (`publish push` takes a
  permit per claim, re-checked like `claim publish`; user decision), U-OOS-D31
  in 28d8613, U-OOS-D32 in a321e0f, U-OOS-D34 in 89213d5. The entries below
  for them are kept as first recorded.
- **Resolved: U-OOS-D35** in 4757ebb (user decision 2026-10-23: record
  them). `scrape person` now keeps each repo's candidates as `scrape github`
  does: unsigned, listed once, never weighed. This finishes the behaviour half
  of D29.
- **U-OOS-D30, `openlore publish push` has no permit:** `publish push` sends
  every signed claim in your own store to an instance, with no
  `SigningPermit` or fresh authority check (`crates/cli/src/verbs/publish.rs`).
  This predates the feature, which gates only `claim publish`. **It needs
  the user's DESIGN decision.**
- **U-OOS-D31:** a `SigningPermit` does not bind a CID. A republish permit for
  claim A would pass for claim B on the same subject. No current caller does
  this.
- **U-OOS-D32:** a `retracts` reference gives an assertion Exempt precedence,
  and `sign batch` does not check that a candidate withdraws only its
  author's own claim.
- **U-OOS-D34:** the share preview writes `PublishCheckPassed/Refused` audit
  lines, but a preview should not record publish events.
- **U-OOS-D35:** `scrape person` does not record imported candidates (the
  behaviour half of D29). This needs a DISTILL row.
- **U-OOS-D22 / D33:** labels show no check age. D33 accepted this; drop age
  from the 03-02 description.
- **U-OOS-D13:** for DESIGN, name the I-9 offline walking-skeleton test
  (`walking_skeleton_sign_writes_atomic_local_file_with_no_network_call`,
  non-`github:` subject) in the brief's I-9 row.
- Also open: U-OOS-D2 (the `infer people` listing ignores the 7-day window;
  signing enforces it). U-OOS-D3 (`infer people --sign` does not refresh the
  signer's link; this needs a GitHub fake in the infer suites). D26 (the
  philosophy page shows no labels).

## Lessons

1. One judge, one evidence path. The gate and the readers had drifted into
   two person-link rules (D8). Merging them removed a whole class of
   label-versus-permit disagreements.
2. Check a reviewer's premise against the ADRs before you act on it. The
   withdrawn-claim change (`3d80b2d`) contradicted ADR-022 and had to be
   reverted (`49b2a30`).
3. Per-crate mutation sees only per-crate tests. Most of the 37 survivors were
   covered by acceptance tests in another package. Each crate now pins its
   own contract.
4. A chicken-and-egg gate needs an entry point that does not depend on
   itself. `authority verify --subject` had to check a repo that no claim
   names yet (D12).

## Try it

1. `openlore claim add` about another GitHub user's account: it is refused,
   and the refusal names who can sign.
2. `openlore authority verify --subject github:<you>/<repo>`, then sign a claim
   about that repo.
3. `openlore graph query --weighted --explain`: look for `whose word:` lines
   and the not-counted footer.
4. `openlore scrape github <owner>/<repo>`, then `openlore candidates list`.

## Lasting artifacts

Following the review-app-repos-and-accounts precedent, nothing was copied to
`docs/architecture/`, `docs/scenarios/` or `docs/ux/`. The ADRs already live
in `docs/adrs/`, and the rest stays in delivery history.

- `docs/adrs/ADR-086-owner-only-signing-subject-authority-model-attested-vs-third-party-claims.md`
- `docs/adrs/ADR-087-subject-authority-proofs-linked-github-owner-and-in-repo-authority-file.md`
- `docs/feature/owner-only-signing/design/architecture-design.md` and
  `upstream-changes.md`
- `docs/feature/owner-only-signing/distill/walking-skeleton.md` and
  `test-scenarios.md`
- `docs/feature/owner-only-signing/deliver/upstream-issues.md` and
  `deliver/mutation/mutation-report.md`

## Commit trail

Branch `oos-deliver`, 71 commits on top of `6fc64b9` before this document:

- **Roadmap:** `b43c521` to `18d3384`.
- **Steps:** `f3b23a9` to `6b61920`, with DISTILL gap rows interleaved
  (`4c5e7db`, `e65d144`, `d0f30eb`).
- **Refactors:** `07f7c72`, `c7f3140`, `e6cf7c0`.
- **Review revision:** `74c0ac9`, `2534501`, `3d80b2d`, `7c46e7e`, `fef514d`,
  `5ce6cd0`, `4c996a8`, `0b4f7d7`, `26075a7`, `52eabfc`, `0b74a42`, `fa778b8`,
  `49b2a30`.
- **Mutation:** `bf63865`, `937042a`.

Trunk-based, no PR. Not pushed.
