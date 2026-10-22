# ADR-086: Owner-Only Signing — Subject Authority for `github:` Subjects, Attested vs Third-Party Claims Derived by Readers, Enforced at Every Composer

- **Status**: Accepted (2026-10-14, user approved as proposed). The Decision text is the
  recommended options of OOS-D1..D3, D6..D11 (`docs/feature/owner-only-signing/design/wave-decisions.md`).
  Depends on ADR-084/085 (github-did-identity-link, Accepted).
- **Date**: 2026-10-14
- **Deciders**: Morgan (nw-solution-architect), propose mode; Jeff Bailey (approved)
- **Feature**: owner-only-signing (DESIGN; no DISCUSS wave; requirements are assumptions A-1..A-13)
- **Companion**: ADR-087 (how authority is proven)
- **Builds on / amends**: ADR-084, ADR-085 (identity link becomes a source of authority, not only a
  display association; see "Relationship to github-did-identity-link"), ADR-008 (retraction),
  ADR-015 (counter-claims: exempt from the gate, and now also excluded from scoring weight),
  ADR-022 (scoring default narrows), ADR-063/064 (`infer people --sign` narrows), ADR-017 (scraper
  `--sign` gated), ADR-071/076 (review app). **Establishes an accepted exception to cross-feature invariant
  I-9** (§5).
- **References**: ADR-007, ADR-009, ADR-023 (indexer capability boundary)

## Context

The user's requirement (verbatim): "it should be possible for someone to import claims into the
database without being the owner of an account but only the owner of the account can sign those
claims. If possible, also make it so only an owner or admin of a repo can sign claims about the repo
but let anyone import a repo and generate unsigned claims."

Today any DID signs any claim about any subject. A claim means "I, the author, assert X"; scoring
triangulates across distinct authors (ADR-022).

**Signing call sites today** (every inline `IdentityPort::sign` in `crates/cli/src/verbs/`):
`claim_add.rs`, `claim_counter.rs`, `claim_retract.rs`, `sign_batch.rs` (shared by
`scrape github --sign`, `scrape person --sign`, `infer people --sign`) and `philosophy_add.rs`
(philosophy records, not claims; out of scope). **Publishing call sites**: the single CLI publish
path `claim_publish::publish_signed_claim` (from `claim add`, the sign batch and `claim publish
<cid>`), and the review app's `executor::execute_publish` (self-attested), gated today by
`routes/review.rs::owner_can_publish` (link verified). The review app's share post
(`review-domain/src/share.rs`) is not a claim but quotes the owner's published claims.

Facts that shape the decision:

1. ATProto cannot stop anyone writing any record to their own PDS. A tool-side refusal binds only
   OpenLore's composers. What peers see is decided by readers.
2. `claim counter` and `claim retract` copy the original claim's subject, predicate and object. A
   subject-based gate with no exemption would stop non-owners from disputing and stop a former
   admin from withdrawing.
3. The scoring feed (`adapter-duckdb/src/graph_query.rs`) carries no `references` column and
   `scoring::score` excludes nothing, so a counter-claim (same subject/predicate/object) appears to
   add weight to the pairing it disputes today, against ADR-015 "shown, never applied". To be
   confirmed with a fixture in DISTILL (UC-20).
4. Cross-feature invariant I-9: compose and sign must succeed with the network disabled.
5. The indexer holds no GitHub capability (ADR-023, C-GDL-8).
6. Already-published third-party claims exist in PDSes and in the live index
   (`https://index.openlore.jeffbailey.us`); their prevalence was not measured. `composedAt` is
   author-asserted, so a date cutoff is forgeable.

## Decision

1. **Scope.** Authority is required only for claims whose subject is in the `github:` scheme:
   `github:<login>` (person) and `github:<owner>/<repo>` (repo). Other subjects are unchanged.
2. **Claim kinds.** A pure `kind(claim)` classifies every claim as `Assertion` or one of three
   **non-weight kinds**: `Retraction` (carries a `retracts` reference), `Counter` (carries a
   `counters` reference), `IdentityLink` (predicate `sameAs`). `supersedes`/`corrects` claims are
   assertions.
   - **Gate**: non-weight kinds need no authority (anyone counters; only the author retracts, per
     ADR-008, enforced by the shipped verbs; identity links follow ADR-084).
   - **Weight**: non-weight kinds **never count in scoring**, for any subject, whoever wrote them
     (counters shown-not-applied per ADR-015; retractions act only through the ADR-060 lineage;
     links dropped per ADR-085 §6). Because a non-weight kind earns nothing in the weight, giving a
     claim a `counters` reference only turns it into a displayed dispute; it cannot buy weight or
     bypass the gate.
3. **The "I assert X" model is kept but split by a reader-side, derived property.** No wire change
   and no new field. For each `github:` assertion a reader computes
   `Authority = Attested(Owner | RepoListed) | Unattested(reason) | NotChecked`
   from the provenance-verified claim and the reader's own local checks (ADR-087). Unattested and
   NotChecked assertions are the third-party layer: shown, labelled, never presented as the
   owner's word.
4. **Composers refuse to sign or publish what the signer cannot attest.**
   - One pure gate, `claim_domain::authority::may_sign`, returns a `SigningPermit` or a
     `SigningRefusal`. `Attested` values and `SigningPermit` have private fields: they are built
     only by `classify`/`may_sign`, from observation values decoded by the check-store and GitHub
     adapters (structural rule `authority_evidence_only_from_adapters`).
     *Note 2026-10-20 (U-OOS-3)*: ADR-089 has since shipped `may_sign` with the owner tier and
     same-request inputs (`signer, subject, listed_ids, RepoRead, OwnerBioRead`). The CLI gate
     from local checks is a second entry point in the same module returning the same
     `SigningPermit`, whose basis gains `RepoListed`; minting stays inside
     `claim_domain::authority`.
   - A new shared CLI helper `sign_permitted(permit, …)` replaces the four inline claim `sign`
     calls; `philosophy_add.rs` keeps its own call (philosophy records; allowlisted by name).
   - The single publish path takes a permit: `publish_signed_claim(…, &SigningPermit)`. `claim
     publish <cid>` obtains one from the stored claim: for a non-weight kind an exempt permit; for
     an assertion, the gate (offline, from checks). "Never published" = `claims.published_at IS
     NULL`; an already-published claim re-published (idempotent 409) needs no new permit.
   - The review app's `PublishPlan` execution takes a permit built in the publish request
     (extends `owner_can_publish`).
   - There is no override flag.
5. **Offline signing, with an accepted exception to I-9.** The gate decides from the **latest local
   authority checks**, which must be `Attested` and no older than the signing window (default
   7 days, A-6). Compose and sign stay network-free: no GitHub call happens at sign time. **But a
   `github:` assertion with a missing or stale check is refused offline**, which narrows I-9 ("compose
   and sign MUST succeed with network disabled") to: *succeeds offline for every claim kind and
   subject except `github:` assertions without a fresh local authority check*. This is recorded as a
   accepted exception in the brief's I-9 row. *Corrected 2026-10-20 (DISTILL U-OOS-2)*: WS-1..WS-10
   (`tests/acceptance/walking_skeleton.rs`) sign `github:rust-lang/rust`, a `github:` repo
   assertion, so WS-10 **is** affected: the slice-01 walking skeleton needs a fixture with
   authority (e.g. a listed DID, or the signer's own repo with a fresh check) or a non-`github:`
   subject. DELIVER decides which, keeping an I-9 test on a non-`github:` subject; the
   offline-with-fresh-check ATs are PS-4 / RA-10. Online verbs (`scrape github … --sign`, `scrape person … --sign`,
   review-app publish) refresh the check in the same invocation or request before gating; offline
   verbs (`claim add`, `infer people --sign`, `claim publish`) refuse with the command to run
   (`openlore authority verify --subject <s>`). *Clarification 2026-10-20 (OOS-D7, within
   option A)*: when an online CLI verb cannot get an answer from GitHub (rate limit, unreachable),
   the refresh records nothing and the gate falls back to the latest local check exactly as
   offline — permit iff `Attested` and ≤ 7 days old, else refuse naming
   `openlore authority verify --subject <s>`.
6. **Authority is current, not historical.** Readers evaluate authority now. A repo transferred
   away, a DID removed from the repo's authority file, or a link whose bio proof is gone makes the
   author's assertions `Unattested` on the next check. Nothing is retracted, superseded or deleted
   on anyone's behalf (CPI D-5, C-GDL-6).
7. **Existing published claims are evaluated by the same rule (grandfathered by label).** No date
   cutoff, no purge, no relabel on the wire.
8. **Reader treatment.**
   - CLI `graph query`, `infer people`, scrape views and the local viewer label every `github:`
     assertion `owner`, `repo-listed`, `third-party` or `authority not checked`, with the check age.
   - **Scoring**: one pure choke point `scoring::score_attested(feed, view)`. Weight-eligible
     rows: `kind = Assertion` ∧ (subject not `github:` ∨ `attested(c, view)`). Contract shape
     *unbounded preservation*: `score_attested(F, V) == score({c ∈ F | weight_eligible(c, V)})`,
     no row altered. The feed gains the claim's reference types so `kind` can be computed. The
     output names how many claims were not counted and why. `--include-unattested` adds unattested
     assertions as a separate per-author section, never mixed into the attested weight; non-weight
     kinds are never weighted.
   - `openlore search` labels every `github:` assertion row from the index "authority not checked
     here" until a local check exists. The indexer is unchanged.
   - The review app's share draft quotes only `Attested` claims of the owner (others are listed as
     "not counted: authority not proven").
9. **Counter-claims and retractions.** Anyone may counter any claim, including an owner's; counters
   stay shown-not-applied. Only the author retracts (ADR-008); a retraction is never refused for
   lack of authority, so a former owner can always withdraw.
10. **Unsigned import is unrestricted.** `scrape github`, `scrape person` and `infer people` keep
    producing unsigned candidates for anyone, for any public subject. When the signer lacks
    authority the candidate list says so per subject and prints the hand-off: who can sign (the
    owner login after `openlore identity link github <login>`, or "a DID listed in the repo's
    `.github/openlore-authority`") and the exact command they would run.

*Amendment 2026-10-21 (user decision D30, U-OOS-D30)*: `openlore publish push` is gated like
`claim publish`. At push time each claim gets a fresh permit from the same stored-check gate
(§4): a claim this store already published re-publishes; exempt kinds (counters, retractions,
identity links, non-`github:` subjects) push; a `github:` assertion needs the person or repo
authority signing it would need now, from local checks only (GitHub is not asked), and a
malformed `github:` subject is refused. A refused claim is not pushed: the push report lists it
as `Not pushed (authority): <subject> — <refusal>`, with the same refusal and hand-off as `claim
publish`, and the push exits 1. Permitted claims push as before; the push path takes the permit
(`sign_only_with_permit` holds `push_one` to a `&SigningPermit`).

## Alternatives Considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **Replace the model: hide or drop every claim whose author lacks authority** | Simplest reading of "only the owner can sign". Loses the opinion layer, silently erases published data from views, hides disputes; not enforceable on the wire anyway. | Rejected |
| **Wire-level `kind: attestation \| opinion` field** | Self-declared, so a reader must still prove authority; Lexicon change, new CIDs, new OAuth consent for the review app. | Rejected |
| **Date cutoff for grandfathering** | `composedAt` is author-asserted; back-dating is trivial. | Rejected |
| **Purge third-party claims from the live index** | Needs indexer GitHub capability; destroys data peers may want as opinions. | Rejected |
| **Live GitHub check at every sign** | Freshest, but breaks I-9 for every `github:` claim and makes `infer people --sign` online. | Rejected (kept for online verbs only) |
| **Reader-only enforcement (no composer gate)** | OpenLore would keep producing claims the user said must not be signed. | Rejected |
| **Gate counters too** | Owners become indisputable; contradicts ADR-015. | Rejected |
| **Exempt kinds counted in weight** (iteration-1 draft) | A `counters` reference would let any author add weight without authority. | Rejected (review C-1) |
| **Scoring counts all assertions, labelled** | No behavior change; a foreign client bypasses the rule for every reader. | Viable — OOS-D9 option B |

## Consequences

- **Positive**: the user's rule holds for every OpenLore composer and the publish path; peers see an
  honest, re-checkable distinction; no Lexicon, CID or OAuth-scope change; dispute and withdrawal
  stay open; already-published data is neither lost nor rewritten; counters stop adding weight.
- **Negative**: authority is reader-local and can be stale or differ between readers. The default
  weighted view shrinks to attested assertions; with owner-only signing most repos will have one or
  a few authors, so ADR-022's cross-author triangulation becomes mostly "owner plus listed DIDs".
  `infer people --sign` works only for the signer's own linked login. Signing about a `github:`
  subject needs a published identity link (person, user-owned repo) or a listing in the repo's
  authority file. I-9 narrows for `github:` assertions without a fresh check.
- **Migration of existing tests**: acceptance files that sign `github:` subjects
  (`scrape_sign.rs`, `scrape_github.rs`, `scrape_person.rs`, `scrape_person_command.rs`,
  `infer_people_sign.rs`, `infer_people_evidence_grows.rs`, `graph_query_explore.rs`,
  `federated_query.rs`, `federation_roundtrip.rs`, `appview_search.rs`, `indexer_ingest.rs`, the
  viewer suites that seed signed `github:` claims, and the review-app publish suites) need an
  authority fixture (seeded fresh checks) or a changed expectation; DISTILL owns the list.
- **Risk**: users who sign today about others' repos lose that ability; dogfood data in the live
  index will mostly read `third-party`. Accepted by the user (2026-10-14).

## Relationship to github-did-identity-link (ADR-084/085)

These changes were applied to ADR-085 and the github-did-identity-link design on 2026-10-14
(`docs/feature/owner-only-signing/design/upstream-changes.md` UC-1..UC-8):

- ADR-085 §6 "display-only association" is extended: a `verified` link is also the source of
  person and user-owned-repo authority (no subject merging; anti-merging unchanged).
- Authority uses each author DID's own `verified` check; a `Contested` id (the bio names several
  DIDs) does not block authority, because the account holder named each DID.
- The ADR-085 §7 per-verb capability table gains rows.
- GDL slice 1 is a prerequisite; GDL slice 3 is a prerequisite for review-app claims to read
  `owner`.

## Earned Trust

- **Derivation properties (not definitional)**: `kind` and `requirement` against a rule table over
  generated claims, including spoofed shapes (a `counters` reference with no reason, a `retracts`
  naming another author's claim, a `sameAs` with author ≠ subject): each is a non-weight kind and
  never raises any weight. `classify` against the ADR-087 rule table over generated observation
  sequences, including observation order (owner verifies, listed DID verifies, owner verifies
  again → the listed DID stays `RepoListed`). `may_sign` refuses whenever the older check is
  outside the window or dated in the future.
- **Structural**: `xtask check-arch` `sign_only_with_permit` (`IdentityPort::sign` only in
  `sign_permitted` and allowlisted `philosophy_add.rs`; `publish_signed_claim` and `PublishPlan`
  execution take a permit), `authority_evidence_only_from_adapters`.
- **Behavioral**: ATs drive every signing and publishing site listed in Context against owner,
  repo-listed, non-owner, stale-check, offline and spoofed-kind cases.
