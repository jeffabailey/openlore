# Requirements: bluesky-claim-review-app

## Business context

Today only a CLI operator with a keychain key, a PLC-published `#org.openlore.application`
method, and an app password can publish OpenLore claims. Claims about other developers can
be scraped and signed by third parties (J-004), but the developers themselves have no
lightweight, consent-first way to say "this is how I build". This feature opens OpenLore to
any Bluesky user. They sign in with their handle, prove their GitHub account is theirs,
privately review philosophy suggestions inferred from their public repos, and publish what
they approve as **self-attested** `org.openlore.claim` records in **their own PDS**. They
can then share a profile of those claims on Bluesky.

- Job: **J-009** (new; see `jtbd-job-stories.md`). Related: J-001, J-004b/c/e, J-007. Later: J-010.
- Brownfield: reuses the `scrape person` pipeline (`crates/cli/src/verbs/scrape_person.rs`),
  real-github-signal-detection, the J-004 signal→predicate mapping, the philosophy
  vocabulary (J-002e), and the claim lexicon (`lexicons/org/openlore/claim.json`, ADR-070
  basis points).

## Personas

| ID | Persona | Characteristics | Used in |
|----|---------|-----------------|---------|
| P-003 (primary) | **Priya Raman**, Rust developer | `@priyaraman.bsky.social`, `did:plc:7x3kq2mzv5rj4w6hbn2tqclp`, PDS on bsky.social. GitHub `priyaraman`: `tidepool` (Rust, Cargo.lock, CI matrix, semver tags), `quill-docs`. Never used the OpenLore CLI. | All stories |
| P-003 edge | **Dmitri Volkov** | `@dmitri.volkov.dev`, self-hosted PDS `pds.volkov.dev`, GitHub `dvolkov` | Non-bsky.social PDS; cross-user privacy |
| P-003 edge | **Aisha Bello** | `@aishabello.bsky.social`, GitHub `aishab` with only forks | Empty states |
| Adversarial | **Sam Ortega** | `@samortega.bsky.social`, `did:plc:q9rt5wz2b8kd3m1xv7pn4ahe`; tries to claim `BurntSushi` | Ownership proof |
| P-001 (reader) | **Maria Santos**, OpenLore node operator | Pulls Priya's claims via `peer pull` and the viewer | US-BRA-009 |

## Decisions (locked)

| ID | Decision | Source |
|----|----------|--------|
| D-1 | v1 inference source is **GitHub only**, reusing the shipped scrape and signal-detection pipeline. Bluesky-post inference comes later. | wizard |
| D-2 | The app is a **hosted web app at a public HTTPS origin**, which ATProto OAuth requires (client metadata URL). Where and how it is hosted is DESIGN's call (OD-BRA-2). | wizard |
| D-3 | **Ownership proof = the signed-in user's DID appears exactly in their GitHub profile bio.** No handle (handles change) and no gist in v1; those are later alternatives. The check runs against the **signed-in** DID, so a GitHub account verified by DID A cannot be used by DID B. A DID (~32 chars) fits the 160-char bio. | user, 2026-10-03 |
| D-4 | **Declines are private.** They are kept as app-side state only, never written to the user's PDS or anywhere public. There is no public counter-claim or rejection record. They are used only to suppress re-suggestion. | user, 2026-10-03 |
| D-5 | **Provenance of app-approved claims = the PDS repo commit signature.** There is no app-level `#org.openlore.application` signature and no PLC document change for Bluesky users. **Consequence for DESIGN (not an open question):** the existing verification in `crates/claim-domain/src/verify.rs` expects the app-level signature, and `peer pull` and the indexer verify against the PLC `#org.openlore.application` key (I-AV-1). Readers, verifiers, the indexer and the viewer must **accept and distinguish** a second provenance mode, **"self-attested (repo-signed)"**, and must never render such claims as unverified or reject them. | user, 2026-10-03 |
| D-6 | Approved claims are written to the **user's own PDS** (the `#atproto_pds` service in their DID document), never to `openlore.jeffbailey.us`. | wizard |
| D-7 | **The human approves every write.** No suggestion is ever auto-published. This extends I-SCR-1 / J-004c. | existing invariant |
| D-8 | Suggestions default to confidence **0.25** (J-004 mapping). The owner may edit 0.00–1.00. The wire format is **integer basis points** (ADR-070). Buckets are display-only (WD-10). | existing |
| D-9 | Changing your mind about a published claim = **soft retraction** (RC-02). The app never hard-deletes a PDS record. | existing RC-02 |
| D-10 | v1 claim subject is **repo-level**, `github:<login>/<repo>`, for repos owned by the verified login, with predicate `embodiesPhilosophy` (matching the shipped scraper convention). Person-level self-claims come later (OD-BRA-6). | PO decision |
| D-11 | The **share post** (`app.bsky.feed.post` linking to the profile page) is **in v1 (Release 1), strictly opt-in**. It is previewed and editable, sent only on explicit confirm, and declining has no side effect. It reflects approved, published claims only. | user, 2026-10-03 |
| D-12 | **Ownership is re-verified before every scrape**, including the first scan and every rescan. If the DID is missing or changed: no scrape, no new suggestions, the link is marked **unverified**, and the user sees how to restore it and re-verify. **Approved claims are kept untouched** in the user's PDS. **Still-pending suggestions are hidden (not deleted) until re-verified.** They are private anyway, and hiding them prevents approving claims about an account whose ownership is currently unproven. Re-verifying makes them visible again and re-enables scraping. | user, 2026-10-03 (pending-handling: PO decision) |

## Cross-cutting invariants (testable)

| ID | Invariant | Verified by |
|----|-----------|-------------|
| **I-BRA-1** | **Pending suggestions are private until approved.** For any suggestion not yet approved: (a) no PDS write, (b) no share post includes it, (c) no OpenLore AppView/search, feed or profile exposure, and (d) no other user, signed in or anonymous, can see it. Only the authenticated owner (session DID == suggestion owner DID) can read their queue. | AC-003.4, AC-003.5, AC-007.2, AC-008.3, @property in `.feature` |
| **I-BRA-2** | **Declines are private.** A decline writes nothing to any PDS or public surface. It is readable only by the owner and used only for suppression. | AC-006.2, AC-006.3 |
| **I-BRA-3** | **Every PDS write traces to an explicit owner confirm** (publish, retract, or share post) made after a preview. | AC-004.1, AC-008.2, AC-011.1 |
| **I-BRA-4** | **No scrape without current proof.** The signed-in DID must be present in the GitHub bio, checked immediately before each scrape (D-3, D-12). | AC-002.*, AC-003.1, AC-010.* |
| **I-BRA-5** | **Self-attested provenance is first-class.** App-approved claims are accepted by OpenLore's read/verify path and labelled "self-attested", never "unverified" or rejected. | AC-004.3, AC-009.* |
| **I-BRA-6** | **Share is opt-in and approved-only.** No post without preview and confirm. The post text and link reference only published, non-retracted claims. | AC-008.* |
| **I-BRA-7** | **Writes go only to the user's own PDS**, resolved from their DID document. | AC-004.2 |
| **I-BRA-8** | **The app never modifies or deletes published claims** except through an owner-confirmed soft retraction. | AC-010.3, AC-011.*, AC-012.3 |

## Functional requirements

| ID | Requirement | Story |
|----|-------------|-------|
| FR-BRA-1 | The user signs in with a Bluesky handle via ATProto OAuth. The app learns the user's DID and PDS. | US-BRA-001 |
| FR-BRA-2 | The app shows the exact signed-in DID with a copy affordance and instructions, then verifies that the public GitHub bio contains it exactly. | US-BRA-002 |
| FR-BRA-3 | After verification (re-checked first), the app scans the verified login's owned public repos with the existing pipeline and produces private suggestions. Each one carries subject, philosophy, confidence, evidence URLs and the producing signal. | US-BRA-003 |
| FR-BRA-4 | The owner approves a suggestion through an exact-record preview. The app writes an `org.openlore.claim` to the owner's PDS and shows the record URI. | US-BRA-004 |
| FR-BRA-5 | The owner can edit confidence and swap the philosophy before approving. | US-BRA-005 |
| FR-BRA-6 | The owner can decline privately, and can undo a decline. | US-BRA-006 |
| FR-BRA-7 | A public profile page lists the owner's published, non-retracted claims with self-attested labels. | US-BRA-007 |
| FR-BRA-8 | An opt-in, previewed, editable share post links to the profile. | US-BRA-008 |
| FR-BRA-9 | OpenLore viewer, search and peer pull recognise and label self-attested claims. | US-BRA-009 |
| FR-BRA-10 | A rescan re-verifies ownership and offers only new suggestions. | US-BRA-010 |
| FR-BRA-11 | The owner can retract a claim published via the app (soft retraction). | US-BRA-011 |
| FR-BRA-12 | The owner can disconnect and have all app-side private state purged. | US-BRA-012 |

## Non-functional requirements

| ID | Category | Requirement (measurable) |
|----|----------|--------------------------|
| NFR-BRA-1 | Privacy | I-BRA-1/2 hold for 100% of pending and declined items. An automated cross-user access test returns "not found or denied" for another DID's queue. |
| NFR-BRA-2 | Security | Only HTTPS. Session tokens are never exposed to page scripts or logs. The app holds no long-lived credential beyond what OAuth issues (mechanism is DESIGN). |
| NFR-BRA-3 | Least privilege | The authorization request asks only for identity, `org.openlore.claim` record writes, and post creation (OD-BRA-7 on granularity). |
| NFR-BRA-4 | Performance | Every action gives visible feedback within 100 ms. The scan of ≤10 owned repos completes in ≤60 s at p90, or shows per-repo progress. Publish confirmation appears in ≤3 s at p90 (PDS permitting). |
| NFR-BRA-5 | Reliability | A failed PDS write leaves the suggestion pending with no partial record. A session expiry preserves pending and declined state. |
| NFR-BRA-6 | Accessibility | WCAG 2.2 AA: full keyboard triage (A/E/N/J/K), visible focus, 4.5:1 contrast, labelled inputs, and errors that name the field and the fix. |
| NFR-BRA-7 | Honesty | The preview contains the literal "not as truth" (consistent with I-7). The publish confirmation names the retract path (consistent with I-8). Display buckets are never persisted (I-6 / WD-10). |
| NFR-BRA-8 | Public data only | Scans read only public GitHub data (I-SCR-2). A banner states it. |
| NFR-BRA-9 | Availability | A target of 99% monthly for sign-in and review. When the app is down, already-published claims are unaffected because they live in users' PDSes. |

## Business rules

- BR-1: A suggestion key is `(owner DID, subject, predicate, object)`. A declined key is never
  re-offered to that DID (D-4). DESIGN decides whether materially new evidence may re-offer
  a key (OD-BRA-5).
- BR-2: An approved key is not re-offered. A rescan shows it as "already published".
- BR-3: Only repos **owned** by the verified login are scanned. Forks and archived repos are
  skipped (existing `select_person_repos` rule).
- BR-4: The confidence entered in the UI must be in 0.00–1.00 with 2-decimal precision. It is
  stored as `round(value × 10000)`.
- BR-5: A philosophy swap may pick any vocabulary entry. An unknown object is never rejected
  (J-002e advisory rule), but v1 UI offers the known vocabulary.

## Assumptions (to validate)

1. Bluesky-hosted and self-hosted PDSes accept `org.openlore.claim` records (an unknown
   lexicon) via the standard record-create path (OD-BRA-9).
2. ATProto OAuth (PAR + DPoP, client metadata at a public URL) is available on bsky.social
   and on current self-hosted PDS versions.
3. Server-side GitHub access needs an app-level token or user GitHub OAuth to avoid the
   60 req/h unauthenticated limit for a multi-user service (OD-BRA-3).
4. Users will tolerate editing their GitHub bio once (a Keybase-style proof habit).
5. Server-side aggregate counters for KPIs are acceptable for a hosted app, even though the
   CLI is telemetry-free (OD-BRA-11).

## Alternatives considered (DISCUSS level)

| Alternative | Why not chosen for v1 |
|-------------|-----------------------|
| Users install the CLI and use app passwords | This is today's path. The cost is prohibitive for ordinary Bluesky users (J-009 push). |
| The app writes claims to OpenLore's own PDS on the user's behalf | Breaks data sovereignty and makes the claims not self-attested (D-6). |
| Publish declines as counter-claims | Exposes what a person declined. The user rejected this (D-4). |
| Add an app key to the user's PLC doc | Bluesky users can't easily do this. The user chose repo-commit provenance (D-5). |
| Gist, handle-in-bio or GitHub OAuth proof | DID-in-bio is the simplest stable proof (D-3). The others are kept as later options. |

## Open questions for DESIGN

Resolved and removed: rejection visibility (D-4), the signing tension (D-5), ownership
re-check (D-12), share scope (D-11).

| ID | Risk | Question | PO recommendation |
|----|------|----------|-------------------|
| OD-BRA-1 | **HIGH** | How is "self-attested (repo-signed)" encoded and recognised? Options include `author` without a key fragment, an absent `signature`, or a new optional field. How do `verify.rs`, `peer pull`, the indexer (I-AV-1 verified-before-index) and the viewer distinguish it? What CID and integrity check applies without an app signature? (D-5 consequence: *how*, not *whether*.) | Forward-compatible optional marker. The verify path returns a provenance enum. Integrity comes from the repo commit and the record CID. |
| OD-BRA-2 | **HIGH** | Hosting topology and OAuth client type: confidential or public client, token storage, and co-location with the existing PDS host (`deploy/`) versus the J-007 serverless `atproto/` path. | Confidential client on a public origin you control. Reuse the existing ops discipline. |
| OD-BRA-3 | **HIGH** | Server-side GitHub access and rate limits for many users: app token, user GitHub OAuth, queueing. | App token plus per-DID scan quotas. |
| OD-BRA-4 | **HIGH** | Composition root: invariant I-3 says the `cli` crate is the only one. A hosted app needs its own composition root that reuses the scraper and claim cores, which needs an ADR exception. | New binary or composition root via an ADR. Keep the pure cores shared. |
| OD-BRA-5 | MED | Private state store, retention, suggestion-key shape, and whether new evidence may re-offer a declined key. | Retain until disconnect. Never re-offer a declined key in v1. |
| OD-BRA-6 | MED | Person-level self-claims (subject = DID) versus repo-level, and predicate naming. | Repo-level in v1 (D-10). Person-level later via J-004e. |
| OD-BRA-7 | MED | OAuth scope granularity (granular `repo:` collection scopes versus broad), and whether post permission is requested incrementally at share time. | Granular scopes. Request post permission incrementally if supported. |
| OD-BRA-8 | MED | Profile page host and URL scheme, its relation to the J-007 philosophy card, and whether it reads live from the PDS or from a cache. | `${app_origin}/@<handle>` resolved via DID. Read from the PDS. |
| OD-BRA-9 | MED | Do third-party PDSes accept and serve `org.openlore.claim` records (lexicon validation mode)? | Spike in DESIGN against bsky.social and the OpenLore PDS. |
| OD-BRA-10 | LOW | Bio matching rule: exact DID token match (not a substring of a longer token). What if the bio contains several DIDs? | Exact, whitespace or punctuation delimited. Several DIDs are allowed; only the signed-in one counts. |
| OD-BRA-11 | LOW | Privacy-respecting KPI instrumentation (aggregate counters, no content) for a project with a telemetry-free CLI ethos. | Aggregate event counts only, no PII, documented. |
| OD-BRA-12 | LOW | Abuse controls: per-DID rate limits on scans and publishes. | Yes. Values are DEVOPS's call. |

## Risks

| Risk | Prob | Impact | Mitigation |
|------|------|--------|------------|
| Third-party PDS rejects or strips unknown-lexicon records | M | H | Spike early (OD-BRA-9). The walking skeleton proves it. |
| Verify-path change regresses existing app-signed verification | M | H | D-5 is additive. Existing app-signed tests must stay green (guardrail). |
| GitHub rate limits make scans slow or failing | H | M | OD-BRA-3. Per-repo progress. Partial results kept. |
| Users rubber-stamp suggestions | M | M | Default 0.25, evidence shown, KPI-BRA-3 watched |
| Composition-root invariant conflict | H | M | OD-BRA-4 ADR |
| Hosting adds an operational burden (a central service) contrary to the local-first ethos | M | M | Published data lives in users' PDSes. The app is a convenience surface, not an authority. |

## Glossary

See `shared-artifacts-registry.md` → "CLI and UI vocabulary" (suggestion, approve, not me,
retract, self-attested).
