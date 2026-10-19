# ADR-089: Review-App Publish Permit — A Fresh Repo-Owner and Bio Read in the Request Mints a `SigningPermit` from the Shared `claim_domain::authority` Gate; a `PublishPlan` Cannot Exist Without One (BR-RRA-1)

- **Status**: Accepted (2026-10-19, approved by the user in chat). Covers D-RRA-D4, D5 and D9 (OQ-6)
  (`docs/feature/review-app-repos-and-accounts/design/wave-decisions.md`).
- **Date**: 2026-10-19
- **Deciders**: Morgan (nw-solution-architect), propose mode; Jeff Bailey (to approve)
- **Feature**: review-app-repos-and-accounts (DESIGN, scope: application)
- **Companion**: ADR-088 (store v2)
- **Builds on**: ADR-086 §4 (one pure gate `claim_domain::authority::may_sign` →
  `SigningPermit`; the review app's `PublishPlan` execution takes a permit built in the publish
  request), ADR-087 §2 (owner tier of a user-owned repo: `owner.id` from `GET /repos/{o}/{r}`),
  ADR-085 C-GDL-10 (fresh = same request, no cache), ADR-076 (bio matcher, `VerifiedOwnership`)
- **Delivers first**: the owner tier of the ADR-086/087 gate, for the review app only.
  owner-only-signing later adds the repo-listed tier, local checks and the CLI call sites to the
  same module.

## Context

BR-RRA-1: a suggestion whose repo is not owned by a currently verified account of the signed-in
DID is never previewed, approved, written to a PDS, quoted in a share post or shown on a profile,
whatever request is sent. BR-RRA-2: at preview and again at confirm the app reads the repo's
current owner and that owner account's bio in the request; a stored status is never proof.

Today `routes/review.rs::owner_can_publish` checks only that the suggestion is pending and the
single `github_links` row says `verified` (a stored flag). `PublishPlan` is built by
`publish_plan(owner_did, draft, composed_at)`, so any route can build and preview one. There is
no repo-metadata read on `GithubPort`. `claim_domain::authority` (ADR-086) is accepted but not
delivered. R3 adds imported repos, the first suggestions that must never publish.

## Decision

1. **New `GithubPort::read_repo(owner, name) → RepoFacts`**: exactly one `GET /repos/{o}/{r}`,
   following at most one same-host redirect (renamed or transferred repo, ADR-087 A-10). Facts:
   `repo_id`, `full_name` (final body), `owner_id`, `owner_login`, `owner_kind` (`User` |
   `Organization`), `fork`, `archived`. 404 = not found or private (one meaning, NFR-BRA-8).
   owner-only-signing's `read_repo_authority` becomes `read_repo` plus the authority-file read.
   `OwnedRepo` (the listing row) gains `repo_id` and `owner_id`.
2. **Shared pure gate, owner tier only.** `claim_domain::authority` is created with the subset
   the review app needs: `may_sign` for a `github:<owner>/<repo>` assertion takes the signer DID,
   the subject, the GitHub ids the signer has listed, the fresh `RepoFacts` and the fresh profile
   of the repo's owner, and returns `SigningPermit { signer, subject, basis: Owner(github_id) }`
   or a `SigningRefusal`: `NotOwnedByListedAccount`, `OrgOwned`, `RepoGone`,
   `BioNoLongerHolds(github_id)` (disproves: the account is marked unverified, AC-010.2),
   `Unfinished` (rate limit or outage: nothing changes, AC-010.3). `SigningPermit` has private
   fields, no serde derive and no public constructor: only `may_sign` mints it. The bio rule is
   the existing exact-token matcher (`claim_domain::identity_link::bio_verdict`). Freshness is the
   caller's duty and is enforced structurally (§4).
3. **A `PublishPlan` needs a permit.** `publish_plan`, `publish_edited_plan` and `restore_plan`
   take `&SigningPermit` instead of `owner_did`; the plan's author is the permit's signer and the
   draft's subject must equal the permit's subject, else `PlanError::NotPermitted`.
   `executor::execute_publish` takes a `PublishPlan`, so preview, approve, approve-selected,
   edit-preview, retry and confirm all need a permit minted in that request. No plan value
   survives a request: the stored plan is restored only with a new permit at confirm.
   Retraction, share and identity-link plans are unchanged (non-weight kinds or already-published
   claims; ADR-086 §2).
4. **The route mints the permit from its own reads.** `owner_can_publish` becomes
   `permit_for(app, session, suggestion)`: one `read_repo`, then (only if the owner id is on the
   owner's list) one `read_person_by_id(owner_id)`, then `may_sign`. A per-request memo shares
   reads between suggestions of the same repo or account in a batch (≤ 25, `MAX_SELECTED`);
   nothing is cached across requests. The stored `status` decides only whether a card is shown.
5. **Visibility (OQ-5)** is one pure function over the pending rows, the account list and the
   import list: an `account` row is shown iff its account is listed and `verified`; an `imported`
   row is shown iff its repo is on the import list. Imported cards render "Not yours to publish"
   with Decline only. Eligibility to publish is decided by §2-4 alone, never by the stored source
   tag.
6. **Enforcement, three layers.**
   - Type: §2-3 (private constructor; plan constructors and the executor require the permit).
   - Structural (`xtask check-arch`): `review_publish_plan_needs_permit` (in
     `openlore-review-app` and `review-domain`, `publish_plan`/`publish_edited_plan`/
     `restore_plan` are called only with a `SigningPermit` argument and `create_claim_record` only
     in `executor.rs`); `authority_evidence_only_from_adapters` (ADR-086 name and one allowlist
     shared with owner-only-signing: construction only in `adapter-github`, `adapter-duckdb`
     and `test-support`/`#[cfg(test)]`; `ports` defines, never constructs);
     `signing_permit_not_serializable` (no `Serialize`/`Deserialize` on `SigningPermit`);
     `review_accounts_write_only_in_github_route` (the publish path holds only a narrow
     `AccountStatusWrite` to mark an account unverified).
   - **Honest limit**: the permit is non-representable without `may_sign`, but the freshness of
     its inputs is not a type fact: `RepoFacts` and `GithubAccountProfile` are plain port values
     and Rust cannot limit their construction to one other crate. Freshness rests on the lint
     and the behavioral property below. Residual risk: a lint gap; mitigated by the property and
     review.
   - Identity-link writes stay under their own fresh-proof rules
     (`identity_link_compose_only_via_proven_link`, `link_preview_proves_from_a_fresh_read`).
   - Behavioral: the @property AT over every request path (preview, approve, approve-selected,
     edit-preview, retry, confirm, confirm-selected) with generated suggestions and generated
     GitHub answers: zero `create_claim_record` calls unless the fresh owner id is listed and its
     bio holds the DID; plus the production audit log `ownership.publish_check.passed` in every
     request that writes a claim (KPI-RRA-4).

## Alternatives Considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **A. Review-domain-local `publish_eligibility` + `PublishPermit`, replaced later by `authority`** | Smallest change; but two gates with two vocabularies until owner-only-signing lands, and the swap touches the same call sites twice. | Rejected (viable fallback) |
| **B. Owner tier of `claim_domain::authority` now (this decision)** | One rule and one permit type from the start, matching ADR-086 §4; owner-only-signing extends rather than replaces. Slightly earlier commitment to the module's shape. | Chosen |
| **C. Wait for owner-only-signing** | R2/R3 would ship without BR-RRA-2; R3 imports would have no guarantee. | Rejected |
| **Boolean check before execute (today's `owner_can_publish`)** | A forgotten call on one route reopens the hole; nothing makes it non-representable. | Rejected |
| **Trust the scan-time owner or the stored status** | Transfers and bio edits after the scan would still publish (C-GDL-10). | Rejected |
| **Store an `unsigned` flag and refuse flagged rows** | Tag-based: wrong after BR-RRA-5 moves and after transfers; still needs the fresh read. | Rejected; the tag drives UI only |

## Consequences

- **Positive**: an unsigned or no-longer-owned suggestion cannot be previewed or written through
  any route, by construction; one gate shared with owner-only-signing; rename- and
  transfer-safe (ids, not logins).
- **Negative**: each preview and confirm costs up to two GitHub requests (NFR-RRA-4); a batch of
  25 distinct repos costs up to 25 + 3. GitHub outages block publishing (by design: nothing is
  published on an unfinished check). Existing review-app publish tests need a `FakeGithub` repo
  and profile fixture.
- **Relationship to owner-only-signing**: ADR-086 reads "owner" for a reader only when the author
  has a *published, verified* identity link; this gate accepts a *private* listed account with a
  fresh bio. Both are the same rule at different freshness: composer = same-request observation,
  reader = local check. Claims published from the review app read `owner` to other readers only
  after the author publishes the link (unchanged ADR-086 behaviour). Recorded in this feature's
  `upstream-changes.md`.
