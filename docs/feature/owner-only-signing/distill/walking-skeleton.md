# Walking Skeleton — owner-only-signing

DESIGN slice 1 is the thin slice: `claim-domain::authority` + the person gate in `claim add` /
`infer people --sign`, with counter and retract still allowed. DISTILL names ONE scenario as the
feature's end-to-end proof.

## The one scenario

`tests/acceptance/owner_only_signing.rs::only_jeff_can_sign_what_is_said_about_jeffs_github_account`
(WS-OOS-1, `#[ignore = "DISTILL owner-only-signing WS-OOS-1: pending DELIVER (enable FIRST)"]`).

```gherkin
@walking_skeleton @driving_port @driving_adapter @real-io @security @kpi-oos-1 @oos-d4 @c-oos-1 @contract-shape:bounded-change
Scenario Outline: Only Jeff can sign what is said about Jeff's GitHub account
  Given Jeff has publicly linked his GitHub account "jeffbailey" to his identity
  When Jeff signs and publishes "<account> adheres to memory-safety"
  Then <outcome>
  Examples:
    | account    | outcome                                                                                       |
    | jeffbailey | the claim is signed and reaches his published repo                                            |
    | rachelk    | nothing is signed or published, and he is told rachelk can sign it after linking her account |
```

The exemption half of slice 1 (counter and retract still allowed) is the next scenario, PS-3,
chained on the same Given — kept out of the skeleton so it has one When.

## Strategy (Architecture of Reference + project policy)

Real: the `openlore` binary as a subprocess (driving adapter — clap parsing, the composition
root, `claim add`), `claim-domain` (the new `authority` gate), the shared sign path, DuckDB
(`identity_link_checks` v7 written by the real `identity link`), the single publish path.
Fake (driven-external only): `FakeGithubAccounts` (the profile/bio the link proof reads),
`FakePds` (Jeff's published repo, output-capturing), `FakeIdentity`. Environment: clean only; the
stale-check / offline / proof-gone environments are focused scenarios (PS-2, PS-4, PS-8, RA-10)
rather than skeleton variants.

## Litmus (Mandate 5)

Title is the user's goal; Givens and Whens are user actions (link my account, sign a claim); the
Thens are what Jeff sees — his published repo holds his claim, or a refusal that names who can
sign. A non-technical stakeholder says "yes — I can vouch for myself, and the tool tells me who
to ask when I try to speak for someone else."

## End-to-end path

`openlore identity link github jeffbailey --yes` (bio read → verified check v7 → link published)
→ `openlore claim add --subject github:<login> …` → `authority::requirement` (Person) →
`classify` from the latest local link check → `may_sign` (≤ 7 days) → `sign_permitted` → local
store + claim file → `publish_signed_claim(&permit)` → `FakePds` record. Row 2 stops at
`may_sign` with a `SigningRefusal` rendered as the hand-off; nothing is written.

## RED today (pre-DELIVER, 2026-10-20)

Run with `cargo test -p cli --test owner_only_signing -- --ignored only_jeff_can_sign`: row 1
(Jeff's own account) passes — every Given works on shipped code; row 2 fails with
`[rachelk] a refused signature exits non-zero` because `claim add` signs and publishes
`github:rachelk` today. MISSING_FUNCTIONALITY ✅ (see `red-classification.md`).
