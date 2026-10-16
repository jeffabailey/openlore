# Walking Skeleton — github-did-identity-link

- **Wave**: DISTILL · **Date**: 2026-10-14 · **Designer**: Quinn (nw-acceptance-designer)
- **Slice**: 1 (DESIGN "Slices": pure core + `identity link`/`identity verify` + v7 check store) — also the prerequisite of owner-only-signing slice 1
- **Executable SSOT**: `tests/acceptance/identity_link.rs` → **WS-GDL-1**
  (`jeffs_github_account_is_publicly_linked_to_his_did_and_shows_as_verified`)
- **Status**: `#[ignore]`d at hand-off and RED when run — the CI policy (D-d) is stated once, in `test-scenarios.md` header. DELIVER removes the `#[ignore]` in the same commit as its first green step.
- **Strategy C** (Sentinel B-1): REAL `openlore` binary, `adapter-github` over HTTP, `adapter-duckdb`, filesystem; FAKE (driven-external only) `FakeGithubAccounts`, `FakePds`, `FakeIdentity`, `PeerPds`.

## The one scenario

```gherkin
@walking_skeleton @driving_port @driving_adapter @real-io @adapter-integration
@gdl-d1 @gdl-d2 @gdl-d4 @gdl-d6 @gdl-d10 @kpi-gdl-1 @contract-shape:bounded-change
Scenario: Jeff's GitHub account is publicly linked to his DID and shows as verified
  Given Jeff's GitHub bio for "jeffbailey" names his DID
  When Jeff links github "jeffbailey" and confirms
  Then his published repo holds the link "did:plc:test-jeff sameAs github:jeffbailey" under the record ID he was shown
  And it carries his profile page and his account number as evidence
  And the publish message names the command that would retract it
  And his linked identities show him as github:jeffbailey, verified today
```

## Litmus (Mandate 5)

1. **Outcome title** — "Jeff's GitHub account is publicly linked to his DID and shows as verified".
2. **Given/When are user actions** — he puts his DID in his bio (outside OpenLore), runs the link verb and confirms.
3. **Then are user observations through driving ports** — the wire record on his PDS (subject, predicate, object, evidence, 10000 bp, author; printed CID == record key, re-verified by the real decoder), the retract command he is told, and his own `identity list` line.
4. **Stakeholder confirmable** — "readers can now see that my GitHub work and my OpenLore claims are the same person, and nobody else could have said so" (A-1).

## End-to-end path

```
GitHub bio names did:plc:test-jeff     FakeGithubAccounts (outside OpenLore)
openlore identity link github jeffbailey --yes
   REAL CLI → adapter-github GET /users/jeffbailey (fresh, no cache)
            → claim_domain::identity_link check_link → ProvenLink → LinkDraft → UnsignedClaim
            → canonicalize → CID → sign → claims/<cid>.json + claims row (adapter-duckdb)
            → identity_link_checks INSERT (v7) → single publish path → FakePds createRecord
openlore identity list                 REAL CLI, offline (no PDS wired, GitHub asked nothing)
            → StoragePort + IdentityLinkCheckReadPort → link_view → "did:plc:test-jeff is github:jeffbailey (verified today)"
```

REAL: `openlore` binary, `claim-domain`, `adapter-github` (over real HTTP), `adapter-duckdb`, filesystem.
FAKE (driven-external only): `FakeGithubAccounts`, `FakePds`, `FakeIdentity`.

## Universe (Mandate 8 — secondary guardrail, waiver in wave-decisions.md)

The primary Thens above observe through ports. As a guardrail, `assert_link_store_delta` over `{local.claims.row_count, local.claim_files, local.peer_claims.row_count,
local.contribution_links, pds.records.len, local.identity_link_checks}`: claims +1, one new claim
file, PDS +1, checks = `[(did:plc:test-jeff, 1234567, jeffbailey, verified)]`; peer claims and
contribution links implicitly unchanged (fail-closed).

## Current RED (fail-for-the-right-reason)

Run 2026-10-14 (`cargo test -p cli --test identity_link -- --include-ignored jeffs_github`):

```
assertion `left == right` failed: the link must publish;
--- stderr ---
error: unrecognized subcommand 'identity'
```

Class: **MISSING_FUNCTIONALITY** — the GIVEN (bio on the GitHub double) succeeded; the WHEN reached the real binary, which has no `identity` verb yet.

## What DELIVER needs to land to turn it green (thinnest thread)

1. `claim_domain::identity_link`: matcher (hoisted), `check_link` for the common case (login read, same id), `ProvenLink` → `LinkDraft` → `UnsignedClaim` with the ADR-084 §2 shape, `link_view` for one Verified link.
2. `IdentityLinkCheckReadPort` / `WritePort` + v7 migration + adapter (insert, latest read).
3. `openlore identity link github <login> --yes` — bypasses the shared compose step (`build_unsigned_claim`, which holds the reserved-predicate guard): its `UnsignedClaim` comes only from `LinkDraft`; it signs and goes out through the single publish path, printing `Published claim <cid>.` and `openlore claim retract <cid>`. *(Amended 2026-10-15, DELIVER Q-RM-4: previously "wired through the shared sign batch".)*
4. `openlore identity list` rendering `<did> is github:<login> (verified today)`.

Everything else (rename id read, preview-only path, squatter rules, scorer drop, review app) is
fleshed out scenario by scenario afterwards.

## Hand-off note

See the CI policy (D-d) in `test-scenarios.md`: the skeleton ships `#[ignore]`d; DELIVER un-ignores it with the first green step.
