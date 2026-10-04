<!-- markdownlint-disable MD024 -->
# User Stories: bluesky-claim-review-app

> Every story has a `job_id` tracing to `docs/product/jobs.yaml` (J-009 and its sub-jobs),
> or `infrastructure-only` with a rationale. Personas are listed in `requirements.md`.
> AC IDs (AC-NNN.n) are consolidated in `acceptance-criteria.md`.

## System Constraints (cross-cutting, apply to every story)

- **I-BRA-1 Pending is private.** No PDS write, share post, AppView/search, feed or profile
  exposure for unapproved suggestions. Only the authenticated owner sees their queue.
- **I-BRA-2 Declines are private** (D-4). They are never written anywhere public.
- **I-BRA-3** Every PDS write follows an explicit owner confirm after a preview.
- **I-BRA-4** No scrape without the signed-in DID in the GitHub bio, re-checked before every
  scrape (D-3, D-12).
- **I-BRA-5** App-approved claims are self-attested (repo-signed) and accepted by OpenLore
  readers, never shown as unverified (D-5).
- **I-BRA-6** The share post is opt-in, previewed, and references approved claims only (D-11).
- **I-BRA-7** Writes go only to the user's own PDS. **I-BRA-8** The app never deletes or
  modifies published claims except through an owner-confirmed soft retraction.
- Confidence is integer basis points on the wire (ADR-070). Buckets are display-only (WD-10).
- Web, WCAG 2.2 AA. Technology choices are deferred to DESIGN (OD-BRA-1..12).

---

## US-BRA-000: The review app is reachable at a public address Bluesky can trust (@infrastructure)

- **job_id**: infrastructure-only
- **infrastructure_rationale**: ATProto OAuth identifies a client by a publicly fetchable
  client-metadata URL on an HTTPS origin. Today the viewer binds loopback only (I-VIEW-4),
  so no Bluesky user can sign in. This story produces no user decision on its own. It
  enables US-BRA-001 in the same slice, which contains four user-visible stories.
- **Release**: Walking Skeleton | **MoSCoW**: Must | **Priority**: P1

### Problem

Priya cannot use any OpenLore web surface today. The viewer only listens on
`127.0.0.1`, and her PDS will refuse to authorize an app whose metadata it cannot fetch over
public HTTPS.

### Who

- Maintainer/operator (Jeff) | deploying the app | wants Bluesky users to reach it safely.

### Solution

A publicly reachable HTTPS origin (`${app_origin}`) that serves the landing page and the
OAuth client metadata document, deployed by the existing ops discipline.

### Domain Examples

#### 1: Happy path

Priya's PDS (bsky.social) fetches `${app_origin}`'s client metadata and shows "OpenLore
review" on its consent screen.

#### 2: Edge: self-hosted PDS

Dmitri's PDS at `pds.volkov.dev` fetches the same metadata and shows the same app name.

#### 3: Error: metadata unreachable

During a deploy, the metadata URL returns 503. Priya's sign-in shows "OpenLore review is
temporarily unavailable. Nothing changed." instead of a raw PDS error.

### UAT Scenarios (BDD)

#### Scenario: Bluesky can identify the review app

Given the review app is deployed at its public address
When Priya's PDS looks up the app's published client details
Then it finds the app name "OpenLore review" and its address over HTTPS

#### Scenario: The landing page is reachable over HTTPS only

Given the review app is deployed
When Priya opens its address over plain HTTP
Then she is moved to the HTTPS address

#### Scenario: Unavailability is explained, not raw

Given the app's client details cannot be fetched
When Priya tries to sign in
Then she sees that the app is temporarily unavailable and nothing changed

### Acceptance Criteria

- [ ] AC-000.1 Client metadata is served at a public HTTPS URL on `${app_origin}` and names the app "OpenLore review".
- [ ] AC-000.2 HTTP requests redirect to HTTPS.
- [ ] AC-000.3 When metadata is unavailable, sign-in shows a plain-language "temporarily unavailable, nothing changed" message.

### Outcome KPIs

- **Who**: Bluesky users attempting sign-in | **Does what**: reach the PDS consent screen | **By how much**: ≥99% of attempts | **Measured by**: aggregate sign-in funnel counters (OD-BRA-11) | **Baseline**: 0% (no public origin exists).

### Technical Notes

- Hosting and topology: OD-BRA-2. Composition root: OD-BRA-4 (I-3 exception).
- `${app_origin}` is the single source for the client id, the profile URL and the share link (registry).
- Depends on: the domain and DNS under `jeffbailey.us` (exists, Cloudflare-served).

---

## US-BRA-001: Sign in with my Bluesky handle

- **job_id**: J-009 (sub-job J-009a)
- **Release**: Walking Skeleton | **MoSCoW**: Must | **Priority**: P1

### Elevator Pitch

- **Before**: Priya can only appear in OpenLore by installing the CLI, creating a keychain key and pasting an app password, so she never does.
- **After**: Priya opens `${app_origin}`, types `priyaraman.bsky.social`, authorizes at her own PDS, and sees "Signed in as @priyaraman.bsky.social".
- **Decision enabled**: Priya decides whether to continue into the GitHub proof, knowing exactly what the app may and may never do.

### Problem

Priya Raman is a Rust developer who lives on Bluesky and has never touched the OpenLore
CLI. Participating today means a CLI install, key management and an app password. She finds
that far too heavy for "let me see what it says about me", so she doesn't participate.

### Who

- Bluesky developer | first visit, on her laptop browser | curious but wary about granting access.

### Solution

"Sign in with Bluesky" using her handle via ATProto OAuth. Her PDS shows the consent screen.
The app learns her DID and PDS. The landing page lists what the app will never do.

### Domain Examples

#### 1: Happy path

Priya enters `priyaraman.bsky.social` and authorizes at bsky.social. She lands on "Signed
in as @priyaraman.bsky.social" with her DID `did:plc:7x3kq2mzv5rj4w6hbn2tqclp` available to
the next step.

#### 2: Edge: custom-domain handle, self-hosted PDS

Dmitri enters `dmitri.volkov.dev`. He is sent to `pds.volkov.dev` to authorize, and returns
signed in.

#### 3: Error: unknown handle or cancel

Priya mistypes `priyaramen.bsky.social` and sees "We couldn't find that Bluesky handle.
Check the spelling." Later she presses Cancel at her PDS and sees "No access granted.
Nothing changed."

### UAT Scenarios (BDD)

#### Scenario: Priya signs in with her handle

Given Priya's handle "priyaraman.bsky.social" belongs to did:plc:7x3kq2mzv5rj4w6hbn2tqclp
When she signs in with Bluesky and authorizes the app at her PDS
Then she sees "Signed in as @priyaraman.bsky.social"

#### Scenario: Priya sees what the app will never do before signing in

Given Priya opens the review app for the first time
Then she sees that it never publishes unapproved suggestions, never shows her pending or declined suggestions to anyone, and never posts unless she presses Post

#### Scenario: Dmitri signs in through his self-hosted PDS

Given Dmitri's handle "dmitri.volkov.dev" is hosted at pds.volkov.dev
When he signs in with Bluesky
Then he authorizes at pds.volkov.dev and returns signed in as @dmitri.volkov.dev

#### Scenario: A mistyped handle gets a helpful message

Given no account has the handle "priyaramen.bsky.social"
When Priya tries to sign in with it
Then she sees "We couldn't find that Bluesky handle. Check the spelling."

#### Scenario: Cancelling authorization changes nothing

Given Priya is on her PDS's authorization screen
When she cancels
Then she is back on the start page with "No access granted. Nothing changed."

### Acceptance Criteria

- [ ] AC-001.1 Sign-in with a valid handle ends on a page showing "Signed in as @<handle>".
- [ ] AC-001.2 The landing page shows the three "never" commitments before any input.
- [ ] AC-001.3 Sign-in works for PDSes other than bsky.social (self-hosted).
- [ ] AC-001.4 An unresolvable handle shows a plain-language message naming the fix.
- [ ] AC-001.5 Cancel or deny at the PDS returns to start with "No access granted. Nothing changed." and no session.
- [ ] AC-001.6 The signed-in DID equals the DID the handle resolved to. On mismatch, sign-in is refused.
- [ ] AC-001.7 Sign out ends the session.

### Outcome KPIs

- **Who**: first-time Bluesky visitors | **Does what**: complete sign-in after starting it | **By how much**: ≥90% | **Measured by**: KPI-BRA-9 funnel counters | **Baseline**: 0 (no path).

### Technical Notes

- OAuth flow and client type: OD-BRA-2. Scopes: OD-BRA-7 (NFR-BRA-3 least privilege).
- Depends on: US-BRA-000.

---

## US-BRA-002: Prove my GitHub account is mine with my DID in my bio

- **job_id**: J-009 (sub-job J-009b)
- **Release**: Walking Skeleton | **MoSCoW**: Must | **Priority**: P1

### Elevator Pitch

- **Before**: Nothing stops anyone from asking OpenLore about any GitHub account and presenting the results as their own.
- **After**: Priya copies `did:plc:7x3kq2mzv5rj4w6hbn2tqclp` from `${app_origin}/github` into her GitHub bio, presses Verify, and sees "Verified: github.com/priyaraman belongs to @priyaraman.bsky.social".
- **Decision enabled**: Priya decides to let the app read her public GitHub work, and trusts that nobody else can claim it.

### Problem

Priya wants suggestions about her own repos, and she wants nobody else able to claim them in
her name. Sam Ortega could otherwise type `BurntSushi` and publish flattering claims about
repos he didn't build.

### Who

- Signed-in Bluesky developer | linking her own GitHub account | wants a simple, stable proof.

### Solution

Show the exact signed-in DID with a Copy button, where to paste it (GitHub → Settings →
Public profile → Bio), and why. On Verify, fetch the public GitHub profile and check that the
bio contains the signed-in DID **exactly**. Record the link for that DID only. Failures give
specific, actionable messages and allow retry.

### Domain Examples

#### 1: Happy path

Priya's bio reads "Rust, tide models. did:plc:7x3kq2mzv5rj4w6hbn2tqclp". She verifies
`priyaraman` and sees the success message.

#### 2: Edge: different DID in the bio

Dmitri's GitHub `dvolkov` bio holds his old DID `did:plc:ab12cd34ef56gh78ij90klmn`. He sees
"github.com/dvolkov's bio contains a different DID (did:plc:ab12…). It must match the
account you're signed in with."

#### 3: Error: someone else's account / rate-limited

Sam verifies `BurntSushi` and sees "We couldn't find did:plc:q9rt…ahe in github.com/BurntSushi's
bio." No scan starts. Separately, with GitHub rate-limiting, Priya sees "GitHub is
rate-limiting us. Try again in 4 minutes. Nothing was lost."

### UAT Scenarios (BDD)

#### Scenario: Priya proves github.com/priyaraman is hers

Given Priya is signed in as did:plc:7x3kq2mzv5rj4w6hbn2tqclp
And her GitHub bio contains "did:plc:7x3kq2mzv5rj4w6hbn2tqclp"
When she verifies GitHub username "priyaraman"
Then she sees "Verified: github.com/priyaraman belongs to @priyaraman.bsky.social"

#### Scenario: Priya gets the exact DID to copy and where to put it

Given Priya is signed in and has not linked GitHub
When she opens the GitHub step
Then she sees her exact DID "did:plc:7x3kq2mzv5rj4w6hbn2tqclp" with a copy action and instructions to add it to her GitHub bio

#### Scenario: Sam cannot claim someone else's account

Given Sam is signed in as did:plc:q9rt5wz2b8kd3m1xv7pn4ahe
And github.com/BurntSushi's bio does not contain Sam's DID
When Sam verifies GitHub username "BurntSushi"
Then he sees that his DID was not found in that bio and how to add it to his own
And no scan starts and no suggestions are created

#### Scenario: A different DID in the bio is explained

Given Dmitri's GitHub bio contains did:plc:ab12cd34ef56gh78ij90klmn but he is signed in with another DID
When he verifies "dvolkov"
Then he sees that the bio holds a different DID that must match his signed-in account

#### Scenario: A rate limit is explained and retry works

Given GitHub is rate-limiting the app for 4 minutes
When Priya verifies
Then she sees a rate-limit message with the wait time
And after the wait, pressing Verify again succeeds

#### Scenario: A verified link belongs to one DID only

Given github.com/priyaraman is verified for did:plc:7x3kq2mzv5rj4w6hbn2tqclp
When Sam, signed in as did:plc:q9rt5wz2b8kd3m1xv7pn4ahe, verifies "priyaraman"
Then the verification fails for Sam
And Priya's link is unaffected

### Acceptance Criteria

- [ ] AC-002.1 The GitHub step displays the signed-in DID exactly, with a copy action, bio location instructions and a one-line reason.
- [ ] AC-002.2 Verify passes only if the public bio contains the signed-in DID as an exact token (OD-BRA-10).
- [ ] AC-002.3 Failure messages distinguish: user not found, DID not found, different DID found, rate-limited (with wait). Each names the fix and allows retry.
- [ ] AC-002.4 No scan runs and no suggestion is created while unverified.
- [ ] AC-002.5 A link is recorded per signed-in DID. Verification by DID A never authorizes DID B.
- [ ] AC-002.6 The handle is never accepted as proof in v1 (DID only).

### Outcome KPIs

- **Who**: signed-in users starting GitHub proof | **Does what**: verify successfully | **By how much**: ≥70% within one session. 0 scans of unverified accounts (KPI-BRA-5) | **Measured by**: funnel counters plus a scan-gate audit | **Baseline**: no proof step exists.

### Technical Notes

- Public GitHub profile read (I-SCR-2). Rate limits: OD-BRA-3. Store the GitHub numeric id (renames, as in OD-CPI-1).
- Gist, handle and OAuth proofs are later alternatives (story map "Later").
- Depends on: US-BRA-001.

---

## US-BRA-003: See my private suggestion queue from my GitHub repos

- **job_id**: J-009 (sub-job J-009c)
- **Release**: Walking Skeleton | **MoSCoW**: Must | **Priority**: P1

### Elevator Pitch

- **Before**: Priya has no idea what OpenLore's scrapers would conclude about her repos, and would hear it, if ever, from someone else's published claim.
- **After**: After verifying, Priya opens `${app_origin}/review` and sees cards like "priyaraman/tidepool embodies dependency-pinning · 0.25 (speculative) · Why: Cargo.lock is committed · evidence link", under "Private: only you can see these".
- **Decision enabled**: Priya decides, card by card, which suggestions are true of her work before anything is public.

### Problem

Priya wants to see what her public work suggests about how she builds, but she is anxious
that a machine's guesses could become public statements about her.

### Who

- Verified Bluesky developer | reviewing her own work | needs evidence and privacy.

### Solution

Immediately before scanning, re-check ownership (D-12). Then scan the verified login's owned
public repos with the existing scrape pipeline and signal detection, and present suggestions
as private cards. Each card shows subject, philosophy, confidence plus bucket, the signal
("Why"), and evidence links. Only the owner can read the queue.

### Domain Examples

#### 1: Happy path

The `tidepool` scan finds 4 signals (Cargo.lock, CI test matrix, semver tags with
CHANGELOG, Rust without unsafe) and `quill-docs` 1 (docs-first). Priya sees 5 cards at 0.25
each.

#### 2: Edge: nothing to suggest

Aisha's GitHub `aishab` has only forks. She sees "We didn't find suggestions in your owned,
public repos. Forks and archived repos are skipped," with a link to what signals are.

#### 3: Error: someone else asks for Priya's queue

Dmitri, signed in, opens Priya's queue address and sees "not found". The page reveals
nothing about Priya's suggestions.

### UAT Scenarios (BDD)

#### Scenario: Priya sees evidence-backed suggestions from her repos

Given Priya has verified github.com/priyaraman
And priyaraman/tidepool commits Cargo.lock
When the scan finishes
Then she sees "priyaraman/tidepool embodies dependency-pinning" at 0.25 (speculative) with the Cargo.lock evidence link and "Why: Cargo.lock is committed"

#### Scenario: Ownership is re-checked before the scan

Given Priya verified github.com/priyaraman and then removed her DID from the bio
When the scan is about to start
Then no scan runs and she sees how to restore her DID and re-verify

#### Scenario: Only Priya can see her queue

Given Priya has 5 pending suggestions
When Dmitri, signed in as @dmitri.volkov.dev, requests Priya's queue
Then he sees nothing of Priya's suggestions
And an anonymous visitor sees nothing either

#### Scenario: Pending suggestions are never exposed

Given Priya has 5 pending suggestions
Then none appears in her PDS, on her profile page, in OpenLore search, in any feed, or in any post

#### Scenario: An empty result guides Aisha

Given Aisha's GitHub has only forked repos
When her scan finishes
Then she sees that no suggestions were found and why forks are skipped

### Acceptance Criteria

- [ ] AC-003.1 Ownership is re-verified immediately before each scan. On failure the scan does not run.
- [ ] AC-003.2 Cards show subject, philosophy, confidence (numeric and bucket), signal and evidence URLs from the shipped derivation.
- [ ] AC-003.3 The queue page always states "Private: only you can see these".
- [ ] AC-003.4 Only the session owner's DID can read its queue. Other DIDs and anonymous visitors get no content (not-found).
- [ ] AC-003.5 Pending suggestions produce no PDS write, no post, and no profile, search, AppView or feed exposure.
- [ ] AC-003.6 An empty result shows guidance (owned public repos only, forks and archived skipped).
- [ ] AC-003.7 Scan progress is shown per repo. A rate limit mid-scan keeps partial results and offers resume.
- [ ] AC-003.8 Cards are operable by keyboard (A/E/N, J/K) with visible focus.

### Outcome KPIs

- **Who**: verified users | **Does what**: reach a non-empty queue | **By how much**: ≥80% of verified users with ≥1 owned public repo | **Measured by**: aggregate counters | **Baseline**: n/a. Guardrail KPI-BRA-4 = 0 exposures of pending items.

### Technical Notes

- Reuse `scrape person` per-repo beats and the J-004 mapping (I-SCR-4/5). Server-side reuse needs OD-BRA-4.
- Private state store: OD-BRA-5. GitHub limits: OD-BRA-3.
- Depends on: US-BRA-002. The scraper pipeline exists.

---

## US-BRA-004: Approve a suggestion into my own PDS, accepted by OpenLore as self-attested

- **job_id**: J-009 (sub-job J-009d)
- **Release**: Walking Skeleton | **MoSCoW**: Must | **Priority**: P1

### Elevator Pitch

- **Before**: Priya cannot publish an OpenLore claim at all without CLI keys, and nothing she agrees with lives in her own repo.
- **After**: On a card in `${app_origin}/review`, Priya presses Approve, sees the exact record preview ("at://did:plc:7x3k…/org.openlore.claim/…, confidence 0.25 (stored as 2500), self-attested"), presses "Publish to my repo", and sees the published `at://` URI with a Retract option.
- **Decision enabled**: Priya decides to put this claim on the public record, in her own repo, knowing exactly what was written and how to retract it.

### Problem

Priya agrees that `tidepool` pins its dependencies and wants that on record as her
reasoning, in her own repository. She is wary of a write she can't see in advance.

### Who

- Verified Bluesky developer | first publish | needs to see the exact record and its destination.

### Solution

Approve opens a preview of the exact record: destination repo and host, subject, predicate,
object, confidence (decimal and stored basis points), evidence, provenance "self-attested",
and the literal "not as truth". Confirm writes an `org.openlore.claim` to the user's own PDS.
OpenLore's read/verify path accepts it as self-attested (D-5).

### Domain Examples

#### 1: Happy path

Priya approves "priyaraman/tidepool embodies dependency-pinning" at 0.25 and confirms. Her
PDS now holds the record at `at://did:plc:7x3kq2mzv5rj4w6hbn2tqclp/org.openlore.claim/3l2x…`
with confidence 2500. OpenLore labels it self-attested.

#### 2: Edge: self-hosted PDS

Dmitri approves a claim, and it is written to `pds.volkov.dev`, not to bsky.social and not
to openlore.jeffbailey.us.

#### 3: Error: PDS unreachable

Priya confirms while bsky.social is unreachable. She sees "We couldn't reach your PDS.
Nothing was published." with Retry. The card stays pending.

### UAT Scenarios (BDD)

#### Scenario: Priya publishes a claim into her own PDS

Given Priya has the pending suggestion "priyaraman/tidepool embodies dependency-pinning" at 0.25
When she previews it and confirms "Publish to my repo"
Then her PDS holds an org.openlore.claim with subject github:priyaraman/tidepool, object org.openlore.philosophy.dependency-pinning, and confidence 2500
And she sees the record's at:// address and a way to retract it

#### Scenario: The preview shows exactly what will be written

Given Priya presses Approve on a suggestion
Then she sees the destination (her repo on her PDS), every field to be written, confidence as 0.25 and as stored 2500, provenance "self-attested", and the words "not as truth"
And nothing is written until she confirms

#### Scenario: OpenLore accepts the app-approved claim as self-attested

Given Priya published a claim through the app
When OpenLore's claim read/verify path reads it from her PDS
Then the claim is accepted, not rejected
And its provenance is shown as "self-attested", not "unverified"

#### Scenario: Backing out writes nothing

Given Priya is on the preview
When she presses Back
Then nothing is written and the suggestion is still pending

#### Scenario: A failed publish leaves the suggestion pending

Given Priya's PDS is unreachable
When she confirms publishing
Then she sees "We couldn't reach your PDS. Nothing was published." with a retry
And the suggestion is still pending with no partial record

### Acceptance Criteria

- [ ] AC-004.1 A record is written only after explicit confirm on the preview (I-BRA-3).
- [ ] AC-004.2 The record is written to the PDS named in the user's DID document (I-BRA-7).
- [ ] AC-004.3 **A claim approved via the app is accepted by OpenLore's read/verify path and shown with self-attested provenance**, never rejected or labelled unverified (D-5, I-BRA-5).
- [ ] AC-004.4 Previewed fields equal the written record field-for-field. Confidence is stored as round(value × 10000). No bucket is stored.
- [ ] AC-004.5 The preview contains "not as truth". The confirmation shows the `at://` URI and a retract path.
- [ ] AC-004.6 Back writes nothing. A failed write leaves the suggestion pending with no partial record and offers retry.
- [ ] AC-004.7 An approved suggestion leaves the queue and is counted as approved.

### Outcome KPIs

- **Who**: verified users with ≥1 suggestion | **Does what**: publish ≥1 claim in their first session | **By how much**: ≥50% (KPI-BRA-1). Median sign-in→publish ≤5 min (KPI-BRA-2) | **Measured by**: aggregate counters | **Baseline**: 0 (no path for Bluesky users).

### Technical Notes

- **D-5 consequence**: `crates/claim-domain/src/verify.rs` (app-level signature) must gain the self-attested provenance mode. The encoding is OD-BRA-1 (HIGH). Existing app-signed verification must not regress.
- Lexicon: `signature` is optional at the wire level. `author` currently documents a key fragment (OD-BRA-1). Third-party PDS acceptance: OD-BRA-9.
- Depends on: US-BRA-003, the OD-BRA-1 resolution, and the verify path change.

---

## US-BRA-005: Edit confidence or swap the philosophy before approving

- **job_id**: J-009 (sub-job J-009d)
- **Release**: R1 | **MoSCoW**: Must | **Priority**: P2

### Elevator Pitch

- **Before**: Priya can only accept a machine's guess as-is, at 0.25 and with the machine's chosen philosophy.
- **After**: On a card in `${app_origin}/review`, Priya presses Edit, picks "memory-safety" from the vocabulary list, sets confidence 0.70 (shown "well-evidenced"), and the preview shows exactly those values.
- **Decision enabled**: Priya decides how strongly, and under which philosophy, she stands behind each claim.

### Problem

The scanner suggested "dependency-pinning" for a signal that, to Priya, is really about
memory safety, and 0.25 undersells what she knows about her own repo.

### Who

- Verified developer | refining a suggestion | knows her work better than the scanner does.

### Solution

An Edit mode on each card. A philosophy picker draws from the shared vocabulary (J-002e).
The confidence input takes 0.00–1.00 and shows its bucket live. Evidence is kept and can be
extended. The preview reflects the edits.

### Domain Examples

#### 1: Happy path

Priya swaps tidepool's suggestion to memory-safety at 0.70. The preview shows 0.70 (stored
7000). She publishes.

#### 2: Edge: boundary values

Priya sets 1.00. The preview shows "triangulated" and stored 10000. 0.00 is allowed and shows
"speculative".

#### 3: Error: invalid confidence

Priya types 1.5. The field shows "Enter a number from 0.00 to 1.00" when she leaves it, and
Approve is disabled until she fixes it.

### UAT Scenarios (BDD)

#### Scenario: Priya publishes her edited claim

Given Priya edits the tidepool suggestion to philosophy memory-safety and confidence 0.70
When she previews and confirms
Then her PDS holds the claim with object org.openlore.philosophy.memory-safety and confidence 7000

#### Scenario: The bucket shows as a label but is never stored

Given Priya sets confidence 0.70
Then she sees "well-evidenced"
And the published record contains only the number 7000

#### Scenario: An out-of-range confidence is caught with guidance

Given Priya enters 1.5
When she leaves the field
Then she sees "Enter a number from 0.00 to 1.00" and cannot approve until fixed

#### Scenario: Cancelling an edit restores the suggestion

Given Priya changed the philosophy in Edit
When she presses Cancel
Then the card shows the original suggestion unchanged

### Acceptance Criteria

- [ ] AC-005.1 The philosophy can be swapped to any vocabulary entry. Confidence is editable from 0.00 to 1.00 in steps of 0.01.
- [ ] AC-005.2 The preview and the written record reflect the edits. Stored confidence = round(value × 10000).
- [ ] AC-005.3 The bucket label is displayed but never written (WD-10).
- [ ] AC-005.4 Invalid confidence shows inline guidance when the field loses focus and blocks approval.
- [ ] AC-005.5 Cancel restores the original suggestion.

### Outcome KPIs

- **Who**: publishers | **Does what**: edit before approving | **By how much**: ≥20% of approvals edited (KPI-BRA-3) | **Measured by**: counters (edited flag at approval) | **Baseline**: 0.

### Technical Notes

- Vocabulary source: the J-002e seeds. An unknown object is never rejected (advisory).
- Depends on: US-BRA-004.

---

## US-BRA-006: Decline a suggestion privately so it never comes back

- **job_id**: J-009 (sub-job J-009e)
- **Release**: R1 | **MoSCoW**: Must | **Priority**: P2

### Elevator Pitch

- **Before**: A wrong suggestion would either sit in Priya's queue forever or, under a public-counter model, announce to the world what she disagreed with.
- **After**: On a card in `${app_origin}/review`, Priya presses "Not me" and sees "Declined. Private: never published, won't be suggested again. [Undo]".
- **Decision enabled**: Priya decides to dismiss a wrong suggestion without any public trace and without being nagged by it again.

### Problem

The scanner says `quill-docs` embodies "documentation-first", but it's a docs site for a
client and not how Priya builds. She wants it gone, quietly and permanently.

### Who

- Verified developer | triaging suggestions | values privacy of her disagreements.

### Solution

"Not me" records a private decline keyed to her DID (D-4). It writes nothing to any PDS or
public surface, suppresses the suggestion on future scans, and offers Undo.

### Domain Examples

#### 1: Happy path

Priya declines "quill-docs embodies documentation-first". The queue drops to 4 pending and 1
declined. Her PDS is unchanged.

#### 2: Edge: Undo

Priya declines by mistake (keyboard N) and presses Undo within the toast. The card returns
as pending.

#### 3: Error/boundary: rescan does not resurrect it

A week later the scan again detects the docs-first signal on `quill-docs`. The suggestion is
not offered.

### UAT Scenarios (BDD)

#### Scenario: Declining writes nothing public

Given Priya has the pending suggestion "priyaraman/quill-docs embodies documentation-first"
When she chooses "Not me"
Then no record of any kind is written to her PDS
And she sees "Declined. Private: never published, won't be suggested again."

#### Scenario: A declined suggestion is not offered again

Given Priya declined "priyaraman/quill-docs embodies documentation-first"
When her repos are scanned again and the same signal is found
Then that suggestion is not offered

#### Scenario: Nobody else can see Priya's declines

Given Priya declined 1 suggestion
When Dmitri or an anonymous visitor looks at Priya's profile, OpenLore search, or her PDS records
Then there is no trace of the decline

#### Scenario: Undo restores the suggestion

Given Priya just declined a suggestion
When she presses Undo
Then the suggestion is pending again

### Acceptance Criteria

- [ ] AC-006.1 "Not me" removes the card from pending and shows the private-decline confirmation with Undo.
- [ ] AC-006.2 **No record is written to the user's PDS on decline** (no counter-claim, no rejection record, nothing).
- [ ] AC-006.3 Declines are readable only by the owner DID and never appear on the profile, search, AppView, feeds or posts.
- [ ] AC-006.4 A declined key `(DID, subject, predicate, object)` is never re-offered on a later scan.
- [ ] AC-006.5 Undo returns the suggestion to pending.

### Outcome KPIs

- **Who**: reviewers | **Does what**: re-see a declined suggestion | **By how much**: 0 occurrences (KPI-BRA-4b). ≥1 decline in ≥30% of sessions with ≥4 suggestions (KPI-BRA-3 non-rubber-stamp signal) | **Measured by**: counters plus a suppression audit | **Baseline**: n/a.

### Technical Notes

- Private store and key shape: OD-BRA-5. This contrasts with the public counter-claim (J-003b): a decline is NOT a counter-claim.
- Depends on: US-BRA-003.

---

## US-BRA-007: My public profile shows only claims I approved

- **job_id**: J-009 (sub-job J-009f)
- **Release**: R1 | **MoSCoW**: Must | **Priority**: P2

### Elevator Pitch

- **Before**: Priya's published claims exist only as raw records in her PDS, and there's no human-readable page to point anyone to.
- **After**: Priya opens `${app_origin}/@priyaraman.bsky.social` and sees "How I build: self-attested", listing memory-safety 0.70 (tidepool) and test-driven 0.40, each labelled [self-attested].
- **Decision enabled**: Priya decides whether her published profile represents her well enough to share.

### Problem

After publishing, Priya wants one clean page that shows how she builds, in her words, and
nothing she hasn't approved.

### Who

- Developer who has published ≥1 claim | about to share | wants accuracy and no leaks.

### Solution

A public profile page per user, listing that user's published, non-retracted claims (from
their PDS) with confidence, subject, evidence and the self-attested label. Pending and
declined items never appear.

### Domain Examples

#### 1: Happy path

Priya has 2 published, 3 pending and 1 declined. Her profile shows exactly the 2 published.

#### 2: Edge: none published yet

Aisha opens her profile and sees "Aisha hasn't published any claims yet." To Aisha herself,
it adds "Review suggestions →".

#### 3: Error: PDS temporarily unreachable

Priya's PDS is down. The profile shows "We can't reach this person's PDS right now," not
stale claims presented as current.

### UAT Scenarios (BDD)

#### Scenario: The profile shows only published claims

Given Priya has 2 published claims, 3 pending suggestions and 1 declined suggestion
When anyone opens Priya's profile page
Then they see exactly the 2 published claims, each labelled self-attested

#### Scenario: Pending and declined items never appear

Given Priya has a pending suggestion "priyaraman/tidepool embodies semantic-versioning"
Then her profile page does not mention semantic-versioning

#### Scenario: An empty profile is honest

Given Aisha has published nothing
When a visitor opens her profile
Then they see that she hasn't published any claims yet

#### Scenario: An unreachable PDS is stated

Given Priya's PDS is unreachable
When a visitor opens her profile
Then they see that her PDS can't be reached right now

### Acceptance Criteria

- [ ] AC-007.1 The profile lists the user's published, non-retracted `org.openlore.claim` records with the self-attested label.
- [ ] AC-007.2 Pending and declined suggestions never appear on the profile (I-BRA-1/2).
- [ ] AC-007.3 The empty state is explicit. The owner sees a call to review.
- [ ] AC-007.4 An unreachable PDS is stated plainly.
- [ ] AC-007.5 Confidence is shown as the stored value / 10000 with its bucket label.

### Outcome KPIs

- **Who**: publishers | **Does what**: open their own profile after first publish | **By how much**: ≥60% | **Measured by**: counters | **Baseline**: 0.

### Technical Notes

- URL scheme, host and live-vs-cache: OD-BRA-8. Relation to the J-007 card: OD-BRA-8.
- Depends on: US-BRA-004.

---

## US-BRA-008: Share my profile on Bluesky — only if I choose to

- **job_id**: J-009 (sub-job J-009f)
- **Release**: R1 | **MoSCoW**: Must (D-11) | **Priority**: P2

### Elevator Pitch

- **Before**: Priya's followers have no idea her profile exists, and she worries an app with post permission might post for her.
- **After**: On `${app_origin}/@priyaraman.bsky.social`, Priya presses "Share on Bluesky…", edits the previewed text ("How I build, self-attested on OpenLore: memory-safety, test-driven" plus the profile link), presses "Post to Bluesky", and sees "Posted. View on Bluesky →".
- **Decision enabled**: Priya decides whether, and with what words, to tell her followers how she builds.

### Problem

Priya is proud of her profile and wants her followers to see it, but only on her terms. No
auto-posts, and no leaking of what she hasn't approved.

### Who

- Developer with ≥1 published claim | on her profile | wants control over public posts.

### Solution

An opt-in share flow. It shows an editable post preview whose generated text names only
published, non-retracted claims and links to the profile. A post is created only on explicit
confirm. "Don't post" has no side effects.

### Domain Examples

#### 1: Happy path

Priya, with memory-safety and test-driven published, edits the text to add "Feedback
welcome!" and posts. The post links to her profile.

#### 2: Edge: declining

Dmitri opens the preview and presses "Don't post". Nothing is posted and nothing changes.

#### 3: Error: post fails

Priya's PDS rejects the post (session expired). She sees "Your post wasn't published. Sign in
again to retry." Her profile is unaffected.

### UAT Scenarios (BDD)

#### Scenario: Priya previews and confirms her post

Given Priya has published memory-safety and test-driven
When she opens "Share on Bluesky…"
Then she sees an editable post that names memory-safety and test-driven and links to her profile
And when she presses "Post to Bluesky" the post appears on her Bluesky account

#### Scenario: Nothing is posted without consent

Given Priya opens the share preview
When she presses "Don't post" or leaves the page
Then nothing is posted to Bluesky and nothing else changes

#### Scenario: The post never mentions unapproved items

Given Priya has 2 published claims, 3 pending suggestions and 1 declined suggestion
When she opens the share preview
Then the generated text names only the 2 published philosophies

#### Scenario: Sharing is unavailable with nothing published

Given Aisha has no published claims
When she views her profile
Then no share action is offered, with a hint to review suggestions first

#### Scenario: A failed post is explained and retryable

Given Priya's post is refused by her PDS
Then she sees that the post wasn't published and how to retry
And her profile is unchanged

### Acceptance Criteria

- [ ] AC-008.1 Share shows an editable preview containing the profile link before anything is posted.
- [ ] AC-008.2 A post is created only on explicit "Post to Bluesky" (I-BRA-3/6). Never automatically, never on publish.
- [ ] AC-008.3 Generated text references only published, non-retracted claims, never pending or declined ones.
- [ ] AC-008.4 "Don't post" or navigating away creates no post and changes no state.
- [ ] AC-008.5 The share action is unavailable when there are 0 published claims.
- [ ] AC-008.6 Failure states the reason and the retry path. The profile is unaffected.

### Outcome KPIs

- **Who**: users with ≥1 published claim | **Does what**: share a post | **By how much**: ≥25% (KPI-BRA-7). 0 posts without a confirm event (guardrail) | **Measured by**: counters (confirm-event to post ratio = 1) | **Baseline**: 0.

### Technical Notes

- Post permission scope: OD-BRA-7 (incremental if supported).
- Depends on: US-BRA-007.

---

## US-BRA-009: OpenLore readers recognise my self-attested claims

- **job_id**: J-009 (sub-job J-009d)
- **Release**: R2 | **MoSCoW**: Should | **Priority**: P3

### Elevator Pitch

- **Before**: A claim without an app-level signature would be rejected by `peer pull` and the indexer, or shown as unverified in the viewer, so Priya's claims would be second-class.
- **After**: Maria runs `openlore peer add did:plc:7x3kq2mzv5rj4w6hbn2tqclp` then `openlore peer pull`, opens the viewer at `/peer-claims`, and sees Priya's tidepool claim with a "self-attested" marker. `openlore search --object memory-safety` lists it the same way.
- **Decision enabled**: Maria decides how much weight to give Priya's claim, knowing its provenance is self-attested rather than app-signed.

### Problem

Maria Santos, an OpenLore node operator, follows developers she respects. Without this
story, Priya's claims either vanish at verification or look suspicious, which undermines
both of them.

### Who

- P-001 node operator (reader) | using peer pull, the viewer and search | weighs provenance.

### Solution

All OpenLore read surfaces (peer pull, indexer and search, viewer) accept self-attested
(repo-signed) claims and display a distinct, neutral "self-attested" provenance marker.
App-signed claims keep their existing behaviour.

### Domain Examples

#### 1: Happy path

Maria pulls Priya. The memory-safety claim is stored and shown as "self-attested".

#### 2: Edge: mixed provenance

Jeff's app-signed claim and Priya's self-attested claim about the same philosophy appear side
by side, each attributed, with their different provenance markers.

#### 3: Error: tampered record

A record whose content doesn't match its repo commit is rejected with "integrity check
failed", same as today's tampered-claim handling.

### UAT Scenarios (BDD)

#### Scenario: Maria pulls Priya's self-attested claims

Given Priya published "priyaraman/tidepool embodies memory-safety" via the review app
When Maria adds Priya as a peer and pulls
Then the claim is in Maria's store and shown as "self-attested"

#### Scenario: Search shows self-attested claims as verified-by-repo

Given the indexer has seen Priya's claim
When Maria searches by philosophy memory-safety
Then Priya's claim is listed with author did:plc:7x3kq2mzv5rj4w6hbn2tqclp and marked "self-attested", not "unverified"

#### Scenario: App-signed claims are unchanged

Given Jeff's app-signed claim exists
When it is pulled and viewed
Then it shows its existing verified marker exactly as before

#### Scenario: A tampered self-attested record is refused

Given a self-attested record whose content fails its integrity check
When it is pulled
Then it is refused with an integrity-check message

### Acceptance Criteria

- [ ] AC-009.1 Peer pull accepts self-attested claims and stores them with provenance = self-attested.
- [ ] AC-009.2 The indexer and search include them (verified by the D-5 mode), each attributed and marked "self-attested".
- [ ] AC-009.3 The viewer labels them "self-attested". They are never "unverified".
- [ ] AC-009.4 Existing app-signed verification and its markers are unchanged (regression guardrail).
- [ ] AC-009.5 Integrity failures are refused as today.

### Outcome KPIs

- **Who**: OpenLore readers | **Does what**: see app-approved claims accepted and labelled | **By how much**: 100% accepted, 0 shown unverified (KPI-BRA-6) | **Measured by**: acceptance tests plus an indexer ingest audit | **Baseline**: 0% (today they would be rejected).

### Technical Notes

- Touches `claim-domain` verify, `peer pull`, the indexer ingest (I-AV-1) and the viewer. Encoding: OD-BRA-1.
- Depends on: US-BRA-004.

---

## US-BRA-010: Rescan to get only new suggestions, with ownership re-checked

- **job_id**: J-009 (sub-job J-009c)
- **Release**: R2 | **MoSCoW**: Should | **Priority**: P3

### Elevator Pitch

- **Before**: Returning a month later, Priya would have to wade through everything again, including things she already published or declined.
- **After**: Priya returns to `${app_origin}/review`, presses "Scan again", and sees "Ownership re-checked ✓ · 2 new suggestions · 3 already published · 1 declined (hidden)".
- **Decision enabled**: Priya decides on only what's new since her last visit.

### Problem

Priya's repos evolve. She added a CHANGELOG to a new crate, `estuary`. She wants just the new
suggestions, and she expects the app to confirm her GitHub is still hers before reading it.

### Who

- Returning verified developer | weeks after first use | low patience for repeats.

### Solution

A rescan re-verifies ownership first (D-12). On pass, it scans and offers only keys not
already published or declined. On fail, it does not scan, marks the link unverified, hides
pending suggestions until re-verified, and leaves published claims untouched.

### Domain Examples

#### 1: Happy path

Priya rescans. `estuary` yields 2 new suggestions, and the 3 published and 1 declined are not
re-offered.

#### 2: Edge: DID removed from bio

Priya cleaned her bio and removed the DID. The rescan doesn't run. She sees "Your DID is no
longer in github.com/priyaraman's bio, so we didn't scan. Your published claims are
untouched. Pending suggestions are hidden until you re-verify."

#### 3: Error→recovery: re-verify restores

Priya re-adds the DID and presses Verify. Her 2 hidden pending suggestions reappear and
"Scan again" works.

### UAT Scenarios (BDD)

#### Scenario: Rescan offers only new suggestions

Given Priya has 3 published claims and 1 declined suggestion
And she added a CHANGELOG with semver tags to priyaraman/estuary
When she scans again
Then she sees only the new estuary suggestions

#### Scenario: Ownership is re-checked before every scrape

Given Priya's link was verified last month
When she scans again
Then the app confirms her DID is still in her GitHub bio before reading any repo

#### Scenario: A failed re-check blocks the scan and keeps published claims untouched

Given Priya removed her DID from her GitHub bio
And she has 3 published claims and 2 pending suggestions
When she scans again
Then no scan runs and no new suggestions are created
And her link shows as unverified with instructions to restore the DID and re-verify
And her 3 published claims in her PDS are unchanged
And her 2 pending suggestions are hidden, not deleted

#### Scenario: Re-verifying restores scanning and pending suggestions

Given Priya's link is unverified and 2 pending suggestions are hidden
When she restores her DID in the bio and verifies
Then her 2 pending suggestions are visible again
And scanning works again

### Acceptance Criteria

- [ ] AC-010.1 Ownership re-verification runs before every scrape. On failure no scrape runs, no suggestion is created, and the link is marked unverified with an actionable message.
- [ ] AC-010.2 Pending suggestions are hidden (not deleted) while unverified and cannot be approved.
- [ ] AC-010.3 **Published claims are not modified or deleted** on verification failure (I-BRA-8).
- [ ] AC-010.4 Re-verifying restores scanning and un-hides pending suggestions.
- [ ] AC-010.5 A rescan never re-offers a published or declined key. The summary shows new, published and declined counts.

### Outcome KPIs

- **Who**: returning users (≥7 days after first visit) | **Does what**: rescan and act on ≥1 new suggestion | **By how much**: ≥30% of returners (KPI-BRA-8) | **Measured by**: counters | **Baseline**: n/a.

### Technical Notes

- Suppression key and "material new evidence" rule: OD-BRA-5.
- Depends on: US-BRA-003, US-BRA-006.

---

## US-BRA-011: Retract a claim I published through the app

- **job_id**: J-009 (sub-job J-009d)
- **Release**: R2 | **MoSCoW**: Should | **Priority**: P3

### Elevator Pitch

- **Before**: Priya can't undo a published claim from the app, and deleting the record elsewhere would silently break readers who already pulled it.
- **After**: On her profile, Priya presses Retract on "tidepool embodies test-driven", sees the preview ("A public retraction referencing this claim will be added to your repo. The original stays readable, marked retracted."), confirms, and sees it marked "retracted".
- **Decision enabled**: Priya decides to withdraw a claim she no longer stands behind, openly and reversibly in spirit.

### Problem

Priya removed the CI test matrix from tidepool. "test-driven" at 0.40 no longer reflects the
repo, and she wants to withdraw it honestly.

### Who

- Publisher | weeks after publishing | wants a clean, honest correction.

### Solution

Retract follows RC-02: a previewed, confirmed retraction claim referencing the original's
CID, written to her PDS. The profile and post generation exclude retracted claims. The app
never hard-deletes.

### Domain Examples

#### 1: Happy path

Priya retracts test-driven. Her profile no longer lists it, and readers see it as retracted.

#### 2: Edge: cancel

Priya opens the retract preview and cancels. Nothing is written.

#### 3: Error: PDS write fails

The retraction fails to write. The claim remains active and Priya sees a retry.

### UAT Scenarios (BDD)

#### Scenario: Priya retracts a published claim

Given Priya published "priyaraman/tidepool embodies test-driven"
When she confirms Retract on its preview
Then a retraction referencing that claim is added to her PDS
And her profile no longer lists test-driven

#### Scenario: Cancelling a retraction writes nothing

Given Priya opened the retract preview
When she cancels
Then nothing is written

#### Scenario: The app never deletes the original

Given Priya retracted a claim
Then the original record is still in her PDS, marked retracted for readers

### Acceptance Criteria

- [ ] AC-011.1 Retraction is written only after preview and confirm (I-BRA-3).
- [ ] AC-011.2 The retraction references the original claim's CID (RC-02). The original is never deleted.
- [ ] AC-011.3 Retracted claims are excluded from the profile listing and share text.
- [ ] AC-011.4 A failed write leaves the claim active and offers retry.

### Outcome KPIs

- **Who**: publishers | **Does what**: report feeling safe to publish because retraction exists | **By how much**: retraction available on 100% of app-published claims. Retract-without-error rate ≥99% | **Measured by**: counters | **Baseline**: n/a.

### Technical Notes

- Reuses the existing retraction semantics (ADR-008 references, RC-02). Encoding under D-5 provenance: OD-BRA-1.
- Depends on: US-BRA-004.

---

## US-BRA-012: Disconnect and have the app forget me

- **job_id**: J-009 (sub-job J-009a)
- **Release**: R2 | **MoSCoW**: Should | **Priority**: P3

### Elevator Pitch

- **Before**: Priya can't tell what the app keeps about her (pending suggestions, declines, GitHub link) or how to remove it.
- **After**: In `${app_origin}/settings`, Priya presses "Disconnect and forget me", reads "This deletes your pending suggestions, declines and GitHub link from OpenLore review. Claims you published stay in your own repo.", confirms, and is signed out.
- **Decision enabled**: Priya decides to leave without residue, knowing her published claims remain hers to manage.

### Problem

Priya is done with the app and wants everything it holds about her gone. She does not want
it touching her published claims.

### Who

- Any signed-in user | leaving | wants a clean exit.

### Solution

A confirmed "Disconnect and forget me" purges all app-side private state for her DID
(suggestions, declines, ownership link, session) and ends the session. It does not touch PDS
records.

### Domain Examples

#### 1: Happy path

Priya confirms. On her next sign-in she starts from the GitHub proof step with an empty
queue.

#### 2: Edge: cancel

She opens the dialog and cancels. Nothing changes.

#### 3: Boundary: published claims survive

After forgetting, her 3 published claims are still in her PDS and on her profile.

### UAT Scenarios (BDD)

#### Scenario: Forget me removes everything the app holds

Given Priya has 2 pending suggestions, 1 decline and a verified GitHub link
When she confirms "Disconnect and forget me"
Then she is signed out
And on her next sign-in she has no pending suggestions, no declines and no GitHub link

#### Scenario: Published claims are untouched

Given Priya has 3 published claims
When she confirms "Disconnect and forget me"
Then all 3 remain in her PDS

#### Scenario: Cancel keeps everything

Given Priya opened the forget-me confirmation
When she cancels
Then nothing is removed

### Acceptance Criteria

- [ ] AC-012.1 The confirmation states exactly what is deleted and what is kept.
- [ ] AC-012.2 On confirm, all app-side state for the DID is purged and the session ends.
- [ ] AC-012.3 No PDS record is modified or deleted (I-BRA-8).
- [ ] AC-012.4 Cancel changes nothing.

### Outcome KPIs

- **Who**: users who disconnect | **Does what**: leave zero app-side residue | **By how much**: 100% purge verified | **Measured by**: post-purge store audit test | **Baseline**: n/a.

### Technical Notes

- Retention and store: OD-BRA-5. Token revocation at the PDS, if supported: OD-BRA-2.
- Depends on: US-BRA-001..006.
