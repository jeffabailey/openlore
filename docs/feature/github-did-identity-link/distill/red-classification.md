# RED classification — github-did-identity-link (pre-DELIVER gate)

Final pass, run **2026-10-14** against `main` (feature unimplemented). Hand-off state and CI
policy: the single statement is in the header of `test-scenarios.md` (D-d).

## Commands and results

| Command | Result |
|---|---|
| `rustfmt --edition 2021 --check` on `identity_link.rs`, `identity_link_readers.rs`, `identity_link_core.rs`, `review_app_identity_link.rs`, `review_app_identity_link_readers.rs`, `crates/test-support/src/fake_github_accounts.rs` (the `mod support` / `review_app` modules are checked through them) | clean |
| `cargo clippy -p cli --test identity_link --test identity_link_readers --test identity_link_core --test review_app_identity_link_readers -- -D warnings` | `Finished`, 0 warnings |
| `cargo clippy -p openlore-review-app --test review_app_identity_link -- -D warnings` | `Finished`, 0 warnings |
| `cargo test -p cli --test identity_link --test identity_link_readers --test identity_link_core --test review_app_identity_link_readers` (default) | ok — identity_link 2 passed (support self-tests) / 33 ignored; core 0 / 10 ignored; readers 4 passed (IR-13, IR-17 + 2 support) / 17 ignored; RR 4 passed (support) / 5 ignored |
| `cargo test -p openlore-review-app --test review_app_identity_link` (default) | ok — 3 passed (RA-3 + 2 support) / 8 ignored |
| the same two commands with `--no-fail-fast -- --include-ignored` | identity_link 32 failed / 3 passed (support ×2, the fixture regenerator, which skips without `OPENLORE_REGEN_PRE_V7`); core 10 failed; readers 17 failed / 4 passed; RR 5 failed / 4 passed; review app 8 failed / 3 passed |
| `OPENLORE_REGEN_PRE_V7=1 cargo test -p cli --test identity_link -- --ignored regenerate` | ok — fixture written from the v6 binary; self-check (schema versions, rows, read-back) passed |

## Classification of every failure

Every GIVEN that uses only shipped verbs succeeded (scrapes, `claim add`, `claim counter`,
`peer add`/`pull`, `infer people`, review-app sign-in and verification, the frozen-store import
with its v6 precondition). No scenario fails in compile, fixture or setup.

| Scenario(s) | Failing step | Cause | Class |
|---|---|---|---|
| WS-GDL-1, IL-2, IL-3, IL-4, IL-6, IL-12, IV-8 | WHEN `identity link` / `verify` (IL-3/IL-6 first hit the pinned exit 1; an unknown verb exits 2) | `unrecognized subcommand 'identity'` | MISSING_FUNCTIONALITY ✅ |
| IL-7 | THEN exit 1 + "not a GitHub username" | same | MISSING_FUNCTIONALITY ✅ |
| IL-9a | THEN stderr names the "repo host" | same | MISSING_FUNCTIONALITY ✅ |
| IL-13a | WHEN the first `identity list` on the frozen v6 store (precondition `schema_version` head 6 passed) | same | MISSING_FUNCTIONALITY ✅ |
| IL-13b | GIVEN the first open (chained) | same | MISSING_FUNCTIONALITY ✅ |
| IL-5, IL-10, IL-11 (its before-view precondition passed), IL-14, IV-1..IV-7, IV-9, IV-10, IF-1a, IF-2, IR-3, IR-4, IR-5, IR-8a/b, IR-9, IR-18 | GIVEN "Jeff linked …" (chained) | same | MISSING_FUNCTIONALITY ✅ |
| IF-1b, IR-6, IR-7, IR-10, IR-11, IR-12, RR-3 | GIVEN "Jeff re-checked …" after a successful `peer pull` | same | MISSING_FUNCTIONALITY ✅ |
| IL-9b | GIVEN "the link is signed locally" (chained on IL-9a) | no link signed | MISSING_FUNCTIONALITY ✅ |
| IR-1, IR-2, IL-8c, RR-1a, RR-1b | `identity list` / `verify` (IL-8c: the counter published, author ≠ subject on the wire, nothing projectable) | same | MISSING_FUNCTIONALITY ✅ |
| IL-8a | THEN `claim add --predicate sameAs` exits 1 | today it signs and publishes | MISSING_FUNCTIONALITY ✅ |
| IL-8b | THEN the inference batch exits 1 | today it signs the edited claim (exit 0) | MISSING_FUNCTIONALITY ✅ |
| IR-14 | THEN `github:rachelk` has no weighted row | `did:plc:rachel-test weight 1.00 [SPARSE]` | MISSING_FUNCTIONALITY ✅ |
| IR-15 | THEN the `/score` region is identical with / without the link | the link is scored | MISSING_FUNCTIONALITY ✅ |
| IR-16 | THEN "authority not checked here" | label absent (the link is listed and attributed) | MISSING_FUNCTIONALITY ✅ |
| RA-1a, RA-1b, RA-2, RA-4..RA-8, RR-2a, RR-2b | WHEN pressing "Publish this link" | `no button or link labelled "Publish this link"` | MISSING_FUNCTIONALITY ✅ |
| CORE-1, 1x, 2, 3, 4, 5, 5b, 7 | `sut_*` binding | `not yet implemented: RED: bind to claim_domain::identity_link::…` | MISSING_FUNCTIONALITY ✅ (Mandate 7 seam; gate DD-GDL-16) |
| CORE-6, CORE-6x | REAL `scoring::score` | a `sameAs` row is ranked as a pairing | MISSING_FUNCTIONALITY ✅ |
| **IR-13**, **IR-17**, **RA-3** | — | pass | ACTIVE regression guards (D-d) |

Result: **0 BROKEN, 0 WRONG_ASSERTION.**

## Slice-2 review RED scenarios (2026-10-17)

Run: `cargo test -p cli --test identity_link_readers -- --include-ignored`, after the slice-2
DELIVER, against `main` at `c07bfe9`. Every GIVEN (link, `peer pull` of the link and of the
retraction/supersession marker, `identity verify`) succeeded; each failure is the reviewer's
failure scenario.

| Scenario | Failing step | Actual failure | Class |
|---|---|---|---|
| IR-18b (B1) | THEN `--did did:plc:test-jeff` shows github:jeffbailey as contested | `expected "github:jeffbailey is contested" naming did:plc:test-jeff` — stdout `did:plc:test-jeff is github:jeffbailey — verified today (record …)` | MISSING_FUNCTIONALITY ✅ |
| IR-8c (B2) | THEN nothing links Rachel to github:rachelk after her retraction was pulled | `did:plc:rachel-test and github:rachelk must not be shown as a link at all` — stdout `did:plc:rachel-test claims github:rachelk — not checked (record …)` | MISSING_FUNCTIONALITY ✅ |
| IR-8d (B2) | THEN the line names only the successor record | `the line names only the new record " (record <successor>)"` — line ends `(records <successor>, <superseded>)` | MISSING_FUNCTIONALITY ✅ |
| IR-12b (H2-1) | — | pass: a re-registered login decides nothing (`check_on_profile` gates on the account number) | ACTIVE regression guard |
| IR-12c (H2-2, U-15 scope) | — | pass | ACTIVE regression guard |
| IR-15 (H1), IR-16 (L1) hardened; `link_checks` (M1) | — | pass | — |

Default run: readers 25 passed / 3 ignored; identity_link 39 passed / 1 ignored.
