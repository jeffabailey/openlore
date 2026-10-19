<!-- markdownlint-disable MD024 -->
# User Stories: review-app-repos-and-accounts

> Every story has a `job_id` from `docs/product/jobs.yaml`. Personas come from
> `docs/feature/bluesky-claim-review-app/discuss/requirements.md` (P-003 Priya Raman, Dmitri
> Volkov, Aisha Bello, adversarial Sam Ortega). No new persona and no new job.
> Journey: `journey-choose-repos-and-accounts.yaml`. Story IDs: US-RRA-NNN.
> Every AC names the UAT scenario that tests it ("Tested by").

## System Constraints (cross-cutting, apply to every story)

- **I-BRA-1 Pending is private.** All suggestions, owned or imported, are visible only to the
  signed-in owner ("Private: only you can see these."). No PDS write, share post, profile,
  search or feed exposure.
- **I-BRA-3 / D-7 Explicit approval.** Nothing is written to a PDS without the owner's explicit
  confirm after a preview.
- **NFR-BRA-8 Public data only.** Scans, repo lists and repo checks read only public GitHub
  data. No GitHub sign-in is added by this feature. R1-R3 publish owned repos only;
  administered repos (US-RRA-007) are deferred (D-RRA-12, user 2026-10-19).
- **Scan admission is unchanged** (`review_domain::budget`): 6 scans per DID per day, 2 scans
  at once app-wide, a scan pauses below 300 remaining GitHub requests and resumes on its own.
  One scan is one admission however many accounts and repos it covers.
- **BR-1 A declined key is never re-offered** to that DID, for owned and imported repos alike.
- **Bio-check rule (D-3, D-12, I-BRA-4).** A GitHub account counts as the owner's only while
  her signed-in DID is in its public bio. It is checked again before a scan reads that
  account's repos. A check **fails** only when the bio was read and does not contain the DID; a
  check GitHub could not answer is not a failure (see the unfinished-check rule and AC-004.8).
  An account whose check fails is marked unverified, its repos are not read, and its pending
  suggestions are hidden (not deleted) until it is verified again.
- **C-GDL-1.** Only the DID owner creates a link. Linking several accounts is private app state;
  it never publishes or retracts a `sameAs` link record. "Publish this link" stays a separate,
  existing action.
- **BR-RRA-1 Unsigned suggestions never reach the publish path.** A suggestion whose repo is
  not owned by a currently verified account of the signed-in DID (BR-RRA-2) is never previewed,
  approved, written to a PDS, quoted in a share post or shown on a profile. This holds for every
  request, not only for the buttons on the page.
  *Note (ADR-089 §2):* "currently verified" means a listed account (verified or not) whose bio,
  read in this request, holds the signer DID — the fresh read is the verification; the stored
  status never decides. A publish through an account stored as unverified marks it verified.
- **BR-RRA-2 "Currently verified owner" is read fresh, in the request.** At preview and again
  at confirm, the app reads the repo's current owner from GitHub and checks that owner
  account's bio for the signed-in DID, both within that request. Publishing proceeds only when
  the owner is an account on the owner's list and its bio holds her DID. A stored status or flag
  is never the proof. If the bio check fails, the account is marked unverified (as in a scan).
  Delivered in R2 (US-RRA-010), the first release where a second account makes more repos
  publishable.
- **An unfinished check is not a failed check.** If GitHub cannot be reached or rate-limits a
  bio check (in a scan, at preview or at confirm), the account keeps its status and its
  suggestions stay shown; its repos are not read in that scan, nothing is published, and the
  owner is told nothing was lost and to try again later.
- **BR-RRA-3 One total per-scan repo limit.** The ticked repos across all accounts (saved or
  default ticks) plus the imported repos may total at most `${scan_repo_cap}`. The total is
  checked at every Save and every import; the refusal names the limit, the current total and
  how many to untick or remove. Default ticks fill only the room left: an account without a
  saved choice gets its best repos ticked (up to 5), in the order accounts were added, until
  the limit is reached, and its list says how many were ticked because of the limit. At scan
  start the read set is checked again; if it still exceeds the limit (for example after the
  value is lowered), the scan is refused before admission with the totals and the
  fix. The limit is 10 (D-RRA-16, user 2026-10-19), the size NFR-BRA-4 already budgets
  for (≤10 repos in ≤60 s p90). Every surface prints the same number.
- **BR-RRA-4 A scan uses what was in force when it started.** Choices, accounts and imports
  saved during a running scan apply from the next scan. Removing an account or an imported
  repo during a scan stops reads of it at once, and its pending suggestions are deleted at once;
  the running scan cannot add new ones for it (D-RRA-17; user decision OQ-D4, DESIGN 2026-10-19).
- **D-RRA-17 Remove deletes pending.** Removing a GitHub account or an imported repo deletes its
  pending suggestions. Published claims stay in the PDS, untouched. The private records of
  declined keys (and, for accounts, published keys) are kept so BR-1 and BR-2 still hold; they
  go only with "Disconnect and forget me".
- **BR-RRA-5 One list per repo.** A repo is either on an account's owned list or on the
  imported list, never both. When an account on the owner's list owns an imported repo (after
  a verify, or seen by a scan), the repo moves to that account's owned list and keeps its
  suggestions. The move is written into that account's saved choice (created from its current
  ticks if it had none), with the repo ticked. It always fits under BR-RRA-3: the repo already
  counted toward the limit as an imported repo, so the total does not change.
- **Every refusal names the field or cause and the next step**, and says "Nothing was
  published" when a publish is refused.
- Technology choices (store shape, routes, caching) are DESIGN's.

## Non-Functional Requirements

| ID | Category | Requirement |
|---|---|---|
| NFR-RRA-1 | Responsiveness | Save choice, Add repo, Remove and Verify give visible feedback within 100 ms (inherits NFR-BRA-4). |
| NFR-RRA-2 | Performance | The repo list renders within 3 s at p90 for an account with up to 100 owned repos, or shows a loading state. |
| NFR-RRA-3 | Performance | A scan of `${scan_repo_cap}` repos completes within 60 s at p90 or shows per-repo progress (inherits NFR-BRA-4). |
| NFR-RRA-4 | GitHub budget | A scan costs at most one profile read per linked account plus the per-repo harvest (≈10 requests per repo). A publish request costs at most one repo read plus one profile read. Opening the repo list costs one listing per verified account. |
| NFR-RRA-5 | Accessibility | WCAG 2.2 AA: every new control is keyboard reachable with a visible focus and a label naming its repo or account. |
| NFR-RRA-6 | Privacy of third parties | Owners of imported repos are never contacted or notified, and no public trace of an import exists. Imported suggestions are visible only to the importer, who is never shown as a publisher of anything about that repo. |

## Impacted Journeys

| Journey | Steps changed | How |
|---|---|---|
| `docs/product/journeys/review-and-publish-my-claims.yaml` (SSOT, created by this wave from the bluesky-claim-review-app journey) | 3 Prove GitHub, 4 Scan, 5 Review, 6 Publish; new steps 9-12 | Several accounts, repo choice, imported repos, fresh ownership check at publish |
| `docs/feature/bluesky-claim-review-app/discuss/journey-review-and-publish-my-claims.yaml` | none (not edited) | Superseded for steps 3-6 by the SSOT journey |
| `docs/product/journeys/scrape-propose-sign.yaml` (CLI) | none | Unchanged; the review app reuses its derivation |

## Changed Assumption

`docs/feature/bluesky-claim-review-app/discuss/requirements.md`, BR-3, today reads:

> BR-3: Only repos **owned** by the verified login are scanned. Forks and archived repos are
> skipped (existing `select_person_repos` rule).

This feature widens it:

- Owned repos of **every** currently verified login of the signed-in DID can be scanned, and
  the owner **chooses** which ones (top 5 pre-ticked). Forks and archived owned repos are still
  not offered on the owned list.
- **Any public repo** the owner names by owner/name is scanned too, as **unsigned** suggestions
  that are never publishable (BR-RRA-1).
- What may be **published** stays narrow: only repos owned by a currently verified login
  (BR-RRA-2), plus administered repos once US-RRA-007 lands.

Related widenings: I-BRA-4 ("No scrape without current proof.") now applies per account and to
owned repos only; imported repos need no proof because they can never be published. The review
app's "one GitHub link per DID" private state (bluesky-claim-review-app `data-models.md`)
becomes several per DID, as A-4 of github-did-identity-link already allows.

---

## US-RRA-001: Choose which of my owned repos a scan reads

- **job_id**: J-009 (sub-job J-009c)
- **Release**: R1 | **MoSCoW**: Must | **Priority**: P1 | **Estimate**: 2 days

### Elevator Pitch

- **Before**: Priya's scan reads only the 5 most-starred of her 7 repos, so `ferrous-notes`, where her newest work lives, is never read and the abandoned `driftwood` always is.
- **After**: Priya presses "Choose repos" on her queue and sees her 7 owned repos ranked by stars with the top 5 ticked; she unticks `driftwood`, ticks `ferrous-notes`, presses "Save choice" and sees "Saved. Your next scan reads 5 repos."
- **Decision enabled**: Priya decides which of her repos speak for how she builds.

### Problem

Priya Raman is a Rust developer signed in to the live review app with `github.com/priyaraman`
verified. She owns 7 public repos besides 1 fork and 1 archived repo. The scan always reads the
top 5 by stars, so her newest project `ferrous-notes` (3 stars) never produces suggestions and
the abandoned `driftwood` does. She has no way to change that.

### Who

- Bluesky developer | verified, returning to the queue | wants suggestions about the repos that show how she builds now.

### Solution

A repo list for each verified account, reached from the queue. It lists every owned repo that is
not a fork or archived, ranked as today (stars, last push, name), with today's top 5 ticked
until she saves her own choice. She ticks, unticks and saves. The list says how many forks and
archived repos were skipped.

### Domain Examples

#### 1: Happy path

Priya opens "Choose repos". tidepool ★412, quill-docs ★88, kelp ★51, harbor-cli ★37 and
driftwood ★12 are ticked; ferrous-notes ★3 and sandbar ★0 are not; "Skipped: 1 fork, 1 archived
repo." She unticks driftwood, ticks ferrous-notes, saves: "Saved. Your next scan reads 5 repos."

#### 2: Edge: only forks

Aisha Bello (`aishab`) owns 3 forks and nothing else. "Choose repos" says "No repos to choose
on github.com/aishab: forks and archived repos are skipped (3 forks). You can name a public repo
you don't own instead." There is no Save button.

#### 3: Boundary: over the limit

Dmitri Volkov (`dvolkov`) owns 12 public repos. With a limit of 10 he ticks 11. Save is refused:
"You can choose up to 10 repos per scan in total. You have 11 chosen. Untick 1 to save."

#### 4: Error: GitHub rate-limited

GitHub is rate-limiting the app when Priya opens the list: "GitHub is rate-limiting us right now,
so we can't list your repos. Your saved choice is unchanged. Try again in a few minutes."

### UAT Scenarios (BDD)

#### Scenario: First visit shows today's top 5 already ticked

Given Priya is verified as github.com/priyaraman with 7 owned public repos, 1 fork and 1 archived repo
And she has never saved a repo choice
When she opens "Choose repos"
Then she sees the 7 repos ranked by stars
And tidepool, quill-docs, kelp, harbor-cli and driftwood are ticked
And she sees "Skipped: 1 fork, 1 archived repo."

#### Scenario: Priya saves her own choice

Given Priya sees her repo list with the top 5 ticked
When she unticks driftwood, ticks ferrous-notes and presses "Save choice"
Then she sees "Saved. Your next scan reads 5 repos."
And when she opens "Choose repos" again, ferrous-notes is ticked and driftwood is not

#### Scenario: Choosing more than the limit is refused with the fix

Given Dmitri is verified as github.com/dvolkov with 12 owned public repos
And the per-scan limit is 10 repos
When he ticks 11 repos and presses "Save choice"
Then he sees "You can choose up to 10 repos per scan in total. You have 11 chosen. Untick 1 to save."
And his saved choice is unchanged

#### Scenario: An owner with only forks learns why nothing is listed

Given Aisha is verified as github.com/aishab, which owns 3 forks and no other repo
When she opens "Choose repos"
Then she sees that forks and archived repos are skipped and that 3 forks were skipped
And she is offered naming a public repo she doesn't own

#### Scenario: A failed listing leaves the saved choice alone

Given Priya saved a choice of 5 repos
And GitHub is rate-limiting the app
When she opens "Choose repos"
Then she sees that GitHub is rate-limiting the app, that her saved choice is unchanged, and to try again in a few minutes
And her next scan still reads her 5 saved repos

#### Scenario: Priya chooses and saves with the keyboard only

Given Priya uses only the keyboard
When she tabs through "Choose repos", toggles ferrous-notes with the space bar and activates "Save choice"
Then every box and the button receive visible focus in order
And each box is announced with its repo name, for example "priyaraman/ferrous-notes"
And her choice is saved

#### Scenario: An account that needs re-verifying shows no repo list

Given Priya's DID is no longer in the bio of github.com/priyaraman
When she opens "Choose repos"
Then she sees no repo list for priyaraman
And she sees "Your DID is no longer in the bio of github.com/priyaraman. Put it back, then verify on the GitHub step." with a link to the GitHub step

### Acceptance Criteria

- [ ] AC-001.1 For an account that has no saved choice (and no BR-RRA-5 move, which creates one), the ticked repos are today's default (top 5 by stars, then last push, then name), limited by the room left under BR-RRA-3. Tested by: "First visit shows today's top 5", US-RRA-003 "A new account's default ticks fit the limit".
- [ ] AC-001.2 Saving stores exactly the ticked repos; reopening shows them. Tested by: "Priya saves her own choice".
- [ ] AC-001.3 A Save that would make the total across accounts and imports exceed `${scan_repo_cap}` is refused with the limit, the total and how many to untick; nothing is saved (BR-RRA-3). Tested by: "Choosing more than the limit is refused".
- [ ] AC-001.4 Forks and archived repos are never offered; the list states how many of each were skipped. Tested by: "First visit…", "An owner with only forks…".
- [ ] AC-001.5 A listing failure shows the cause and the next step and leaves the saved choice unchanged. Tested by: "A failed listing leaves the saved choice alone".
- [ ] AC-001.6 The list is fully operable by keyboard with labelled boxes (NFR-RRA-5). Tested by: "Priya chooses and saves with the keyboard only".
- [ ] AC-001.7 The list is shown only for accounts verified for the signed-in DID; an account needing re-verify shows the fix and a link instead. Tested by: "An account that needs re-verifying shows no repo list".

### Outcome KPIs

- **Who**: verified owners with more than 5 owned non-fork repos | **Does what**: save a choice that differs from the default | **By how much**: ≥25% within 30 days (hypothesis, see wave-decisions) | **Measured by**: `repos.choice.changed_from_default` / `repos.choice.viewed` (no repo names) | **Baseline**: 0% (no choice possible).

### Technical Notes

- Reuses the `scraper_domain::select_person_repos` ranking for the default (no second ranking rule).
- A rename of the GitHub account must keep the saved choice (identify accounts by their immutable GitHub id; C-GDL-3 precedent). Store shape is DESIGN's.
- The choice is private app state and must be purged by "Disconnect and forget me" (US-BRA-012).
- Listing freshness and cost: OQ-7, NFR-RRA-2/4.

---

## US-RRA-002: A scan reads exactly the repos I chose, and remembers them

- **job_id**: J-009 (sub-job J-009c)
- **Release**: R1 | **MoSCoW**: Must | **Priority**: P1 | **Estimate**: 2 days

### Elevator Pitch

- **Before**: every scan picks its own top 5, so Priya can't control which repos produced her suggestions.
- **After**: Priya presses "Scan again" on her queue and the summary reads "Ownership re-checked ✓ · 5 repos read · 3 new suggestions"; her new `ferrous-notes` suggestions appear as cards, and next week's scan reads the same 5 without asking.
- **Decision enabled**: Priya decides whether her saved choice still fits, or whether to change it before the next scan.

### Problem

Priya saved her repo choice (US-RRA-001), but that is worth nothing unless the scan reads exactly
those repos, every time, and tells her when one of them can no longer be read.

### Who

- Bluesky developer | verified, with a saved choice | rescans weekly and wants predictable results.

### Solution

The scan reads the saved choice of each verified account, or, when none is saved, its top 5,
limited by the room left under BR-RRA-3.
Repos that appeared since the choice was saved are shown unticked and marked new; they are not
read until ticked. Chosen repos that became forks, archived, deleted or transferred are skipped
and named in the summary. A scan with nothing to read is refused before it uses one of the day's
scans. Unticking a repo keeps its pending suggestions. A scan uses the choice in force when it
started (BR-RRA-4).

### Domain Examples

#### 1: Happy path

Priya's choice is tidepool, quill-docs, kelp, harbor-cli and ferrous-notes. "Scan again" reads
those 5: "Ownership re-checked ✓ · 5 repos read · 3 new suggestions". driftwood is not read.

#### 2: Edge: a new repo since the choice

Priya created `priyaraman/sandpiper` yesterday. "Choose repos" shows it unticked with "new since
your last choice". The scan does not read it.

#### 3: Edge: a chosen repo was archived

Priya archived `kelp`. The next scan reads 4 repos and says "Skipped priyaraman/kelp: archived
since you chose it. Untick it, or unarchive it on GitHub."

#### 4: Error: nothing to read

Priya unticked every repo and has no imported repo. "Scan again" says "Choose at least one repo,
or name a public repo, before scanning." No scan starts.

### UAT Scenarios (BDD)

#### Scenario: The scan reads only the repos Priya chose

Given Priya's saved choice is tidepool, quill-docs, kelp, harbor-cli and ferrous-notes
When she presses "Scan again"
Then exactly those 5 repos are read and driftwood is not
And the summary says "5 repos read"

#### Scenario: A scan keeps the choice it started with, and later scans use the new one

Given Priya's saved choice is tidepool, quill-docs, kelp, harbor-cli and ferrous-notes
And a scan with that choice is running
When she unticks quill-docs and saves while it runs
Then the running scan, including any part that resumes on its own after a pause, reads the 5 repos it started with
And her next scan reads the 4 repos she saved, without her opening "Choose repos" again

#### Scenario: A repo created after the choice is not read until ticked

Given Priya created priyaraman/sandpiper after saving her choice
When she opens "Choose repos"
Then sandpiper is listed unticked and marked "new since your last choice"
And her next scan does not read sandpiper

#### Scenario: A chosen repo that can no longer be read is skipped and named

Given Priya chose priyaraman/kelp and then archived it on GitHub
When she presses "Scan again"
Then kelp is not read
And the summary says "Skipped priyaraman/kelp: archived since you chose it. Untick it, or unarchive it on GitHub."

#### Scenario: A scan with nothing to read does not start

Given Priya unticked every repo and has no imported repo
When she presses "Scan again"
Then she sees "Choose at least one repo, or name a public repo, before scanning."
And no scan starts and her count of scans today is unchanged

#### Scenario: An owner who never chose keeps today's behaviour

Given Dmitri is verified as github.com/dvolkov and never opened "Choose repos"
When he presses "Scan again"
Then his top 5 owned repos by stars are read, as before this feature

#### Scenario: Unticking a repo keeps its pending suggestions

Given Priya has 2 pending suggestions for priyaraman/driftwood and 1 declined one
When she unticks driftwood, saves, and scans again
Then driftwood is not read
And the 2 pending suggestions are still in her queue and the declined one stays declined

### Acceptance Criteria

- [ ] AC-002.1 The repos read by a scan equal the saved choice of each verified account (or, when none is saved, its top 5, limited by the room left under BR-RRA-3), never more. Tested by: "The scan reads only…", "An owner who never chose…".
- [ ] AC-002.2 A scan reads the choice in force when it started, including after an automatic resume; a saved choice applies to every later scan until changed. Tested by: "A scan keeps the choice it started with…".
- [ ] AC-002.3 Repos first seen after the choice was saved are listed unticked and marked new and are not read until ticked. Tested by: "A repo created after the choice…".
- [ ] AC-002.4 A chosen repo that is now a fork, archived, missing or owned by someone else is not read and is named in the summary with the reason and the fix. Tested by: "A chosen repo that can no longer be read…".
- [ ] AC-002.5 With nothing chosen and nothing imported, "Scan again" is refused with the fix and no scan admission is spent. Tested by: "A scan with nothing to read does not start".
- [ ] AC-002.6 Unticking stops future reads only; pending suggestions stay pending and declined keys stay declined. Tested by: "Unticking a repo keeps its pending suggestions".

### Outcome KPIs

- **Who**: verified owners with a saved choice | **Does what**: act on (approve, edit or decline) ≥1 suggestion flagged `outside_default` (defined in `outcome-kpis.md`) | **By how much**: ≥30% within 30 days (hypothesis) | **Measured by**: `suggestion.acted.outside_default` | **Baseline**: 0%.

### Technical Notes

- The scan task calls `select_person_repos(&repos, DEFAULT_PERSON_REPO_COUNT)` in `crates/openlore-review-app/src/scan.rs`; the saved choice replaces that call when present.
- The bio re-check before reading owned repos is unchanged (I-BRA-4).
- The summary keeps "Ownership re-checked" whenever at least one account is checked (see US-RRA-005 for imports-only scans).
- Depends on US-RRA-001.

---

## US-RRA-003: Verify a second GitHub account on the same sign-in

- **job_id**: J-009 (sub-job J-009b)
- **Release**: R2 | **MoSCoW**: Must | **Priority**: P2 | **Estimate**: 2 days

### Elevator Pitch

- **Before**: the GitHub step holds one account, so Priya's work account `priya-tidal` can't be added without replacing `priyaraman`.
- **After**: on the GitHub step Priya presses "Add another GitHub account", puts her DID in the `priya-tidal` bio, presses Verify and sees "Verified: github.com/priya-tidal belongs to @priyaraman.bsky.social", with both accounts listed as verified.
- **Decision enabled**: Priya decides which of her GitHub accounts the app may read for her.

### Problem

Priya has a personal account `priyaraman` and a work account `priya-tidal` that owns
`priya-tidal/ingest-bench`. Both are her work. The review app keeps one GitHub link per Bluesky
sign-in, so she must choose one and lose the other's suggestions.

### Who

- Bluesky developer | already verified once | has a second GitHub account she owns.

### Solution

The GitHub step lists every account linked to this sign-in with its status and offers "Add
another GitHub account" using the same DID-in-bio check. Adding never changes the accounts
already listed and publishes nothing. Up to 3 accounts per sign-in (D-RRA-15, user 2026-10-19).

### Domain Examples

#### 1: Happy path

Priya adds `priya-tidal`, whose bio holds `did:plc:7x3kq2mzv5rj4w6hbn2tqclp`: "Verified:
github.com/priya-tidal belongs to @priyaraman.bsky.social". Both accounts show as verified.

#### 2: Error: DID not in the second bio

Priya adds `priya-tidal` before editing its bio and sees the existing "We couldn't find your DID
in the bio of github.com/priya-tidal. Add your DID to the bio of your own GitHub account, then
press Verify again." `priyaraman` stays verified.

#### 3: Adversarial: someone else's account

Sam Ortega adds `dvolkov`, already verified for `@dmitri.volkov.dev`, and is refused. He adds
`BurntSushi`; its bio does not hold Sam's DID, so it is refused like example 2.

#### 4: Boundary: a fourth account

Dmitri has 3 verified accounts and adds a fourth: "You can link up to 3 GitHub accounts. Remove
one on this page to add another."

### UAT Scenarios (BDD)

#### Scenario: Priya verifies her work account without losing her personal one

Given Priya is verified as github.com/priyaraman
And the bio of github.com/priya-tidal contains "did:plc:7x3kq2mzv5rj4w6hbn2tqclp"
When she adds and verifies GitHub username "priya-tidal"
Then she sees "Verified: github.com/priya-tidal belongs to @priyaraman.bsky.social"
And her account list shows priyaraman and priya-tidal, both verified
And no record is written to her PDS

#### Scenario: A second account without the DID is refused and the first stays verified

Given Priya is verified as github.com/priyaraman
And the bio of github.com/priya-tidal does not contain her DID
When she adds and verifies GitHub username "priya-tidal"
Then she sees that her DID was not found in the bio of github.com/priya-tidal and to add it, then press Verify again
And github.com/priyaraman is still verified

#### Scenario: An account verified for another Bluesky sign-in cannot be added

Given github.com/dvolkov is verified for @dmitri.volkov.dev
When Sam Ortega, signed in as @samortega.bsky.social, adds GitHub username "dvolkov"
Then he sees "github.com/dvolkov is already verified for another Bluesky account. Each GitHub account can belong to one Bluesky account only. If it is yours, remove it from that Bluesky account first."
And nothing is added to Sam's list

#### Scenario: Adding an account already on the list changes nothing

Given Priya's list already holds github.com/priya-tidal, verified
When she adds GitHub username "priya-tidal" again
Then she sees "github.com/priya-tidal is already on your list."
And her list is unchanged

#### Scenario: A fourth account is refused with the fix

Given Dmitri has 3 verified GitHub accounts on his list
When he adds a fourth GitHub username
Then he sees "You can link up to 3 GitHub accounts. Remove one on this page to add another."
And his list is unchanged

#### Scenario: GitHub being unreachable loses nothing

Given Priya is verified as github.com/priyaraman
And GitHub cannot be reached
When she adds and verifies GitHub username "priya-tidal"
Then she sees "GitHub could not be reached. Please wait a few minutes, then press Verify again. Nothing was lost."
And her list still holds only priyaraman, verified

#### Scenario: A new account's default ticks fit the limit

Given Priya has 8 repos chosen on priyaraman, no imported repo, and the per-scan limit is 10
And github.com/priya-tidal owns 4 public repos
When she verifies priya-tidal and opens "Choose repos"
Then only the 2 best priya-tidal repos are ticked
And she sees "2 of 4 ticked: your limit of 10 repos is reached. Untick others to choose more."
And her next scan reads 10 repos

### Acceptance Criteria

- [ ] AC-003.1 A second account is verified by the same DID-in-bio check against the signed-in DID; success lists both as verified. Tested by: "Priya verifies her work account…".
- [ ] AC-003.2 A failed check of a new account never changes accounts already listed. Tested by: "A second account without the DID…", "GitHub being unreachable…".
- [ ] AC-003.3 An account verified for another DID is refused with the reason and the fix. Tested by: "An account verified for another Bluesky sign-in…".
- [ ] AC-003.4 Adding an account already on the list says so and changes nothing. Tested by: "Adding an account already on the list…".
- [ ] AC-003.5 Adding past the account limit is refused with the limit and the fix. Tested by: "A fourth account is refused…".
- [ ] AC-003.6 Adding an account writes nothing to any PDS and publishes no link (C-GDL-1). Tested by: "Priya verifies her work account…".
- [ ] AC-003.7 A new account's default ticks fill only the room left under the limit, and its list says so (BR-RRA-3). Tested by: "A new account's default ticks fit the limit".

### Outcome KPIs

- **Who**: owners who start "Add another GitHub account" | **Does what**: finish with the account verified | **By how much**: ≥70% (same bar as the first-account proof in bluesky-claim-review-app) | **Measured by**: `account.add.started` / `account.add.verified` | **Baseline**: 0% (not possible today).

### Technical Notes

- The private store holds one link per DID today; it must hold several (OQ-4). github-did-identity-link A-4 allows several accounts per DID.
- The verify-attempt budget (20 per DID per hour) is shared across accounts.
- BR-RRA-5: verifying an account that owns an imported repo moves that repo to its owned list (tested in US-RRA-006).

---

## US-RRA-004: One scan reads every verified account's chosen repos

- **job_id**: J-009 (sub-job J-009c)
- **Release**: R2 | **MoSCoW**: Must | **Priority**: P2 | **Estimate**: 2.5 days

### Elevator Pitch

- **Before**: a scan reads one GitHub account, so `priya-tidal/ingest-bench` never produces a suggestion.
- **After**: Priya presses "Scan again"; the summary reads "Ownership re-checked: priyaraman ✓ · priya-tidal ✓ · 7 repos read · 6 new suggestions", and the `ingest-bench` card says "from github.com/priya-tidal".
- **Decision enabled**: Priya decides which suggestions to approve knowing which account each one speaks for.

### Problem

Once Priya has two verified accounts, she expects one scan to cover both and to see which account
each suggestion came from. If one account fails its bio check, she must not lose the other's scan.

### Who

- Bluesky developer | two verified accounts | wants one scan and clear provenance per card.

### Solution

One scan re-checks every linked account's bio, then reads the chosen repos of each account that
passes. An account fails only when its bio was read and does not contain the DID; an account
GitHub could not answer is not failed (AC-004.8). Accounts that fail are skipped, named in the
summary with the fix, marked unverified, and their pending suggestions hidden until re-verified. Every card names its account. Each account
keeps its own repo choice; the total limit spans all of them (BR-RRA-3).

### Domain Examples

#### 1: Happy path

Priya chose 5 repos on `priyaraman` and 2 on `priya-tidal`. "Scan again" reads 7 repos; the
`ingest-bench` card says "from github.com/priya-tidal".

#### 2: Edge: one account fails

Priya removed her DID from the `priya-tidal` bio. The scan reads her 5 `priyaraman` repos only and
says "priya-tidal ✗: your DID is not in its bio, so its repos were not read and its suggestions
are hidden. Put your DID back, then verify."

#### 3: Boundary: total over the limit

Priya has 8 repos chosen on `priyaraman` and ticks 3 on `priya-tidal`: "You can choose up to 10
repos per scan in total. You have 11 chosen. Untick 1 to save."

#### 4: Error: rate limit mid-scan

GitHub's budget runs low after 4 repos. The scan pauses, keeps the 4 repos' suggestions, and
resumes on its own with the same accounts and choices.

### UAT Scenarios (BDD)

#### Scenario: One scan covers both accounts and each card names its account

Given Priya chose 5 repos on priyaraman and 2 on priya-tidal, and both bios contain her DID
When she presses "Scan again"
Then 7 repos are read under one of her 6 scans today
And the summary says "Ownership re-checked: priyaraman ✓ · priya-tidal ✓"
And the card for priya-tidal/ingest-bench says "from github.com/priya-tidal"

#### Scenario: An account that fails its check is skipped and named, the other is scanned

Given the bio of github.com/priya-tidal no longer contains Priya's DID
When she presses "Scan again"
Then no priya-tidal repo is read
And her 5 chosen priyaraman repos are read
And the summary names priya-tidal, says its suggestions are hidden, and says to put the DID back and verify

#### Scenario: Hidden suggestions come back after re-verifying

Given priya-tidal failed its check and its 2 pending suggestions are hidden
When Priya puts her DID back in the priya-tidal bio and verifies it
Then the 2 suggestions are shown in her queue again

#### Scenario: GitHub being unreachable during a scan hides nothing

Given Priya has 2 pending suggestions from priya-tidal
And GitHub cannot be reached when the scan checks the priya-tidal bio
When she presses "Scan again"
Then no priya-tidal repo is read in this scan
And priya-tidal keeps its verified status and its 2 suggestions stay shown
And the summary says "priya-tidal: GitHub could not be reached, so its repos were not read. Nothing was lost. Scan again later."

#### Scenario: Two accounts together cannot exceed the limit, each keeping its own choice

Given Priya has 8 repos chosen on priyaraman and the per-scan limit is 10
When she ticks 3 repos on priya-tidal and presses "Save choice"
Then she sees "You can choose up to 10 repos per scan in total. You have 11 chosen. Untick 1 to save."
And neither account's saved choice changes, and each is still shown under its own account heading

#### Scenario: When every account fails, nothing is read

Given neither bio contains Priya's DID and she has no imported repo
When she presses "Scan again"
Then no repo is read
And the summary names both accounts and how to fix them
And her published claims are unchanged

#### Scenario: A paused scan resumes with the same accounts and choices

Given a scan of Priya's 7 chosen repos pauses for GitHub's budget after 4 repos
When the budget refills and the scan resumes on its own
Then the 4 repos' suggestions are still in her queue
And the remaining 3 chosen repos are read, and no other repo

### Acceptance Criteria

- [ ] AC-004.1 One scan uses one scan admission and covers every linked account. Tested by: "One scan covers both accounts…".
- [ ] AC-004.2 Each account's bio is checked in that scan before any of its repos is read; an account that fails has none of its repos read. Tested by: "An account that fails its check…", "When every account fails…".
- [ ] AC-004.3 A failing account is marked unverified, named in the summary with the fix, and its pending suggestions are hidden, not deleted; other accounts are unaffected. Tested by: "An account that fails its check…".
- [ ] AC-004.4 Re-verifying an account shows its hidden suggestions again. Tested by: "Hidden suggestions come back…".
- [ ] AC-004.5 Every suggestion card names the GitHub account it came from. Tested by: "One scan covers both accounts…".
- [ ] AC-004.6 Each account has its own choice; the limit applies to the total (BR-RRA-3). Tested by: "Two accounts together cannot exceed the limit…", US-RRA-003 "A new account's default ticks fit the limit".
- [ ] AC-004.7 A rate-limit pause keeps what was found and resumes with the same accounts and choices. Tested by: "A paused scan resumes…".
- [ ] AC-004.8 A bio check that cannot finish is not a failed check: no unverified mark, no hiding, the account's repos are not read this scan, and the summary says nothing was lost. Tested by: "GitHub being unreachable during a scan hides nothing".

### Outcome KPIs

- **Who**: owners with two or more verified accounts | **Does what**: act on ≥1 suggestion from a second account | **By how much**: ≥50% within 30 days (hypothesis) | **Measured by**: `suggestion.acted.outside_default` restricted to non-first accounts | **Baseline**: 0%.

### Technical Notes

- Today `run_scan` re-verifies one `GithubLink`; it must cover every linked account (OQ-4, OQ-5).
- Pending visibility is derived today from the single link's status; it must be derived per source account (OQ-5).
- GitHub cost per NFR-RRA-4; the floor and pause rules are unchanged.
- Depends on US-RRA-003. Works with or without R1 (default per account: top 5, limited by the room left under BR-RRA-3).

---

## US-RRA-008: Stop scanning a GitHub account I no longer want read

- **job_id**: J-009 (sub-job J-009b)
- **Release**: R2 | **MoSCoW**: Should | **Priority**: P2 | **Estimate**: 1 day

### Elevator Pitch

- **Before**: once an account is linked, Priya cannot take it off without disconnecting everything.
- **After**: on the GitHub step Priya presses "Remove" next to `github.com/priya-tidal`, confirms, and sees "github.com/priya-tidal removed. Its 2 pending suggestions were deleted. Your published claims are unchanged."
- **Decision enabled**: Priya decides which accounts stay part of her scans.

### Problem

Priya left her job and no longer wants `priya-tidal` scanned. She wants it off her list, with its
pending suggestions gone, without losing her published claims or having declined suggestions come
back if she re-adds it.

### Who

- Bluesky developer | two linked accounts | wants to drop one.

### Solution

"Remove" per account, with a confirm that says what will be deleted. Removing stops scans of that
account and deletes its pending suggestions (D-RRA-17). It keeps the private records of declined
keys, so BR-1 still holds, and of published ones, so BR-2 still holds. It writes nothing to any
PDS and retracts no claim and no published link. Re-adding and re-verifying starts fresh: the next
scan offers that account's suggestions again as new, except declined and published keys.

### Domain Examples

#### 1: Happy path

Priya removes `priya-tidal`. Its 2 pending suggestions are deleted; her published claim about
`priya-tidal/ingest-bench` stays in her PDS; her published `sameAs` link to `priya-tidal` stays
until she retracts it with "Retract this link".

#### 2: Edge: re-added

A month later Priya re-adds and verifies `priya-tidal` and scans. The 2 earlier suggestions come
back as new suggestions; the one she declined is not offered; `ingest-bench`'s published one shows
as "already published".

#### 3: Edge: removed during a scan

Priya removes `priya-tidal` while a scan runs. Its remaining repos are not read; the pending
suggestions the scan found for it are deleted when the scan ends.

### UAT Scenarios (BDD)

#### Scenario: Priya removes an account she no longer wants scanned

Given Priya's list holds priyaraman and priya-tidal, and priya-tidal has 2 pending suggestions
And she published one claim about priya-tidal/ingest-bench and a link to github.com/priya-tidal
When she presses "Remove" next to priya-tidal and confirms
Then she sees "github.com/priya-tidal removed. Its 2 pending suggestions were deleted. Your published claims are unchanged."
And her published claim and published link are unchanged and nothing is written to her PDS

#### Scenario: Cancelling the removal changes nothing

Given Priya pressed "Remove" next to priya-tidal
And the confirm says "This deletes its 2 pending suggestions. Your published claims stay in your repo."
When she cancels the confirm
Then priya-tidal is still on her list, verified, and its 2 suggestions are shown

#### Scenario: Re-adding offers suggestions again, but never declined or published ones

Given Priya removed priya-tidal while it had 2 pending suggestions, 1 declined one and 1 published claim
When she adds and verifies priya-tidal again and presses "Scan again"
Then the 2 earlier suggestions are offered again as new suggestions
And the declined one is not offered
And the published one is counted as "already published"

#### Scenario: Removing an account during a scan stops its reads

Given a scan of Priya's priyaraman and priya-tidal repos is running and has read 1 of 2 priya-tidal repos
When she removes priya-tidal and confirms
Then the second priya-tidal repo is not read
And after the scan ends, no priya-tidal pending suggestion exists

### Acceptance Criteria

- [ ] AC-008.1 Remove needs a confirm that states how many pending suggestions will be deleted; cancelling changes nothing. Tested by: "Priya removes…", "Cancelling the removal…".
- [ ] AC-008.2 Removing stops scans of the account and deletes its pending suggestions; it writes nothing to any PDS and retracts no claim or link. Tested by: "Priya removes…".
- [ ] AC-008.3 Removing keeps the private records of the account's declined and published keys: after re-adding and re-verifying, a scan offers its other suggestions as new, never re-offers a declined key (BR-1), and counts published keys as already published (BR-2). Tested by: "Re-adding offers suggestions again…".
- [ ] AC-008.4 Removing during a scan stops further reads of that account; the pending suggestions that scan found for it are deleted at once, and the scan cannot add new ones for it (BR-RRA-4; OQ-D4). Tested by: "Removing an account during a scan…".

### Outcome KPIs

- **Who**: owners who remove an account | **Does what**: keep using the app (scan again within 30 days) | **By how much**: ≥60% (hypothesis: removal is tidying, not leaving) | **Measured by**: `account.removed` followed by `scan.completed` | **Baseline**: n/a (only full disconnect exists).

### Technical Notes

- Delete on remove (D-RRA-17, user 2026-10-19). Declined and published records stay until "Disconnect and forget me" (US-BRA-012) purges everything.
- Depends on US-RRA-003.

---

## US-RRA-005: Get private suggestions for any public repo I name

- **job_id**: J-004 (sub-job J-004a)
- **Release**: R3 | **MoSCoW**: Should | **Priority**: P3 | **Estimate**: 2.5 days

### Elevator Pitch

- **Before**: Priya can only see suggestions for repos her verified accounts own, so she can't see what OpenLore would say about `rust-lang/cargo` or her team's `tidal-labs/streamline`.
- **After**: Priya types `rust-lang/cargo` under "Repos you don't own", presses "Add repo", presses "Scan again", and sees 4 private cards for `rust-lang/cargo`, each saying "Not yours to publish".
- **Decision enabled**: Priya decides whether a repo she admires or works on embodies the philosophies she cares about, without any claim going out under her name.

### Problem

Priya works every day on `tidal-labs/streamline`, owned by her employer's org, and admires
`rust-lang/cargo`. The review app reads only repos her accounts own. She wants to name any public
repo and see suggestions for it, knowing they stay private and unsigned.

### Who

- Bluesky developer | signed in, verified or not | curious about public repos she does not own.
- Owners and maintainers of the named repos | not users of the app | must never be contacted, and no public trace of the import may exist (NFR-RRA-6).

### Solution

A "Repos you don't own" list on the queue. She names a public repo by owner/name; the app checks
that it exists and is public, and refuses repos one of her verified accounts owns. Forks and
archived repos may be named. Named repos are read by her next scan without any bio check; their
suggestions go to her private queue marked "Not yours to publish" (US-RRA-006). Any signed-in
person may do this, with no GitHub account verified, because nothing imported can be published.

### Domain Examples

#### 1: Happy path

Priya names `rust-lang/cargo`. After "Scan again", 4 cards for `rust-lang/cargo` appear, each
"Not yours to publish".

#### 2: Error: wrong shape, missing or private

`cargo`: "Enter the repo as owner/name, for example rust-lang/cargo." `tidal-labs/secret-roadmap`
(private): "We couldn't find a public repo at github.com/tidal-labs/secret-roadmap. Check the
spelling. Private repos can't be read."

#### 3: Edge: it's hers

`priyaraman/sandbar`: "github.com/priyaraman/sandbar is yours. Tick it in your repo list
instead."

#### 4: Edge: no verified account

Dmitri has not verified `dvolkov`. He names `rust-lang/cargo` and scans; the summary says "No
GitHub account to re-check · 1 repo read · 4 new suggestions".

#### 5: Boundary: over the limit

Priya has 10 repos chosen and names an eleventh: "You can choose up to 10 repos per scan in
total. You have 10 chosen. Untick or remove 1 repo before adding another."

### UAT Scenarios (BDD)

#### Scenario Outline: Priya names a public repo she doesn't own and gets private suggestions

Given Priya is signed in and <repo> is a public repo none of her accounts owns
When she adds "<repo>" and presses "Scan again"
Then <repo> is listed under "Repos you don't own"
And her private queue shows its suggestions, each marked "Not yours to publish"

Examples:

| repo | kind |
|---|---|
| rust-lang/cargo | ordinary repo |
| aishab/ripgrep | a fork |

#### Scenario: A repo name in the wrong shape is refused with an example

When Priya adds "cargo"
Then she sees "Enter the repo as owner/name, for example rust-lang/cargo." next to the field
And nothing is added

#### Scenario: A missing or private repo is refused

When Priya adds "tidal-labs/secret-roadmap", which is private
Then she sees "We couldn't find a public repo at github.com/tidal-labs/secret-roadmap. Check the spelling. Private repos can't be read."
And nothing is added

#### Scenario: A repo her verified account owns is pointed to the repo list

Given Priya is verified as github.com/priyaraman
When she adds "priyaraman/sandbar"
Then she sees "github.com/priyaraman/sandbar is yours. Tick it in your repo list instead."
And sandbar is not listed under "Repos you don't own"

#### Scenario: Someone with no verified GitHub account can still look at a repo

Given Dmitri is signed in, has no verified GitHub account, and has run 2 scans today
When he adds "rust-lang/cargo" and presses "Scan again"
Then the summary says "No GitHub account to re-check · 1 repo read"
And his private queue shows suggestions for rust-lang/cargo marked "Not yours to publish"
And his count of scans today is 3

#### Scenario: Naming a repo past the limit is refused with the fix

Given Priya has 10 repos chosen and the per-scan limit is 10
When she adds "rust-lang/cargo"
Then she sees "You can choose up to 10 repos per scan in total. You have 10 chosen. Untick or remove 1 repo before adding another."
And nothing is added

#### Scenario: GitHub being unreachable adds nothing and loses nothing

Given GitHub cannot be reached
When Priya adds "rust-lang/cargo"
Then she sees "GitHub could not be reached, so we couldn't check github.com/rust-lang/cargo. Please try again in a few minutes. Nothing was lost."
And her list of repos she doesn't own is unchanged

### Acceptance Criteria

- [ ] AC-005.1 A repo is imported only if given as owner/name, existing and public (forks and archived included); otherwise the message names the cause and the fix and nothing is added. Tested by: the Scenario Outline, "wrong shape", "missing or private".
- [ ] AC-005.2 A repo owned by a verified account of the signed-in DID is not imported; the message points to the repo list. Tested by: "A repo her verified account owns…".
- [ ] AC-005.3 Imported repos are read by later scans with no bio check and count toward the total limit; naming past it is refused with the fix. Tested by: the Scenario Outline, "Naming a repo past the limit…".
- [ ] AC-005.4 Imported suggestions are private and carry "Not yours to publish". Tested by: the Scenario Outline.
- [ ] AC-005.5 Any signed-in DID may import; such a scan counts as one of the day's 6 scans, and a scan with no account says "No GitHub account to re-check". Tested by: "Someone with no verified GitHub account…".
- [ ] AC-005.6 If GitHub cannot be read while checking a named repo, nothing is added and the message gives the next step. Tested by: "GitHub being unreachable…".

### Outcome KPIs

- **Who**: owners who import ≥1 repo | **Does what**: act on ≥1 suggestion from an imported repo | **By how much**: ≥50% within 30 days (hypothesis) | **Measured by**: `import.added`, `import.refused.<code>`, `suggestion.acted.outside_default` on imported suggestions | **Baseline**: 0%.

### Technical Notes

- Reuses the shipped harvest and `derive_candidates` per repo; only the repo list source differs.
- GitHub answers 404 for both missing and private repos, so one message covers both (NFR-BRA-8).
- Owner-only-signing OOS-D13 option B ("import any public repo into private suggestions, marked not yours to publish") was offered as a later slice; this story realizes it.
- The imported repo list is private app state, purged by US-BRA-012.
- **Ships only together with US-RRA-006 and US-RRA-009, after US-RRA-010 (R2).**

---

## US-RRA-009: Stop reading a repo I don't own

- **job_id**: J-004 (sub-job J-004a)
- **Release**: R3 | **MoSCoW**: Should | **Priority**: P3 | **Estimate**: 1 day

### Elevator Pitch

- **Before**: once Priya names `rust-lang/cargo`, it would be read by every scan with no way out.
- **After**: Priya presses "Remove" next to `rust-lang/cargo` and sees "rust-lang/cargo removed. Its 3 pending suggestions left your queue."
- **Decision enabled**: Priya decides which outside repos stay worth a place in her scan limit.

### Problem

Priya named `rust-lang/cargo` out of curiosity. She has seen enough and wants the slot back for
`tidal-labs/streamline`.

### Who

- Bluesky developer | has imported repos | wants to tidy them.

### Solution

"Remove" per imported repo. It stops future reads, deletes its pending suggestions (D-RRA-17),
keeps the private record of its declined keys (BR-1), and frees room under the limit. Removing
during a scan stops reads of it at once (BR-RRA-4). Imported repos are never published, so there
is no published record to keep.

### Domain Examples

#### 1: Happy path

Priya removes `rust-lang/cargo`; its 3 pending suggestions are deleted.

#### 2: Edge: re-added

Priya names `rust-lang/cargo` again; the suggestion she declined earlier is not offered.

#### 3: Edge: at the limit

Priya was at 10 of 10. After removing `rust-lang/cargo`, naming `tidal-labs/streamline` works.

### UAT Scenarios (BDD)

#### Scenario: Removing an imported repo stops reading it

Given Priya imported rust-lang/cargo and has 3 pending suggestions for it
When she presses "Remove" next to rust-lang/cargo
Then she sees "rust-lang/cargo removed. Its 3 pending suggestions were deleted."
And her queue holds no rust-lang/cargo suggestion
And her next scan does not read rust-lang/cargo

#### Scenario: Re-adding offers suggestions again, but never declined ones

Given Priya had 3 pending and 1 declined suggestion for rust-lang/cargo and then removed the repo
When she adds "rust-lang/cargo" again and scans
Then the 3 earlier suggestions are offered again as new suggestions
And the declined suggestion is not offered

#### Scenario: Removing frees room under the limit

Given Priya has 10 of 10 repos chosen, including rust-lang/cargo
When she removes rust-lang/cargo and adds "tidal-labs/streamline"
Then tidal-labs/streamline is listed under "Repos you don't own"

#### Scenario: Removing an imported repo during a scan stops its read

Given a scan is running that has not yet read rust-lang/cargo
When Priya removes rust-lang/cargo
Then the scan does not read rust-lang/cargo

### Acceptance Criteria

- [ ] AC-009.1 Removing stops future reads and deletes the repo's pending suggestions. Tested by: "Removing an imported repo stops reading it".
- [ ] AC-009.2 An imported repo has no published keys (BR-RRA-1: its suggestions never reach publish), so removing it keeps only the private record of its declined keys; a repo that has moved to an account's owned list (BR-RRA-5) is no longer imported and follows the account rules (US-RRA-008). If the repo is added again, a scan offers its other suggestions as new and never re-offers a declined key (BR-1). Tested by: "Re-adding offers suggestions again, but never declined ones".
- [ ] AC-009.3 Removing frees room under the total limit at once. Tested by: "Removing frees room…".
- [ ] AC-009.4 Removing during a scan stops reads of that repo (BR-RRA-4). Tested by: "Removing an imported repo during a scan…".

### Outcome KPIs

- **Who**: owners who import repos | **Does what**: keep at least one imported repo for 14 days or longer | **By how much**: ≥40% (hypothesis: imports are kept, not tried once) | **Measured by**: `import.added` / `import.removed` age buckets (no repo names) | **Baseline**: n/a.

### Technical Notes

- Delete on remove (D-RRA-17, user 2026-10-19); declined records stay until "Disconnect and forget me". Depends on US-RRA-005.

---

## US-RRA-006: Suggestions about repos I don't own can never be published

- **job_id**: J-009 (sub-job J-009d)
- **Release**: R3 | **MoSCoW**: Must | **Priority**: P3 | **Estimate**: 2.5 days

### Elevator Pitch

- **Before**: nothing stops a suggestion about someone else's repo from reaching "Publish to my repo", which would put a claim about `rust-lang/cargo` in Priya's repo as if she owned it.
- **After**: Priya's `rust-lang/cargo` card on her queue reads "Not yours to publish. Only an owner of github.com/rust-lang can publish claims about this repo." and offers only Decline.
- **Decision enabled**: Priya decides whether to import a repo she admires, knowing nothing about it can be published under her name.

### Problem

Importing any public repo (US-RRA-005) creates suggestions about repos the owner does not own. The
user's rule: anyone may import and get unsigned suggestions, but only an owner (or admin) of a
repo may publish claims about it. Sam Ortega would try to publish claims about
`BurntSushi/ripgrep` by sending the publish request without the page.

### Who

- Bluesky developer exploring repos she doesn't own | and an adversarial user forging requests.

### Solution

Imported cards show "Not yours to publish" and who can publish, with no Select, Approve or Edit
control; Decline works as usual. Every preview and confirm applies BR-RRA-2 and refuses
otherwise, whatever the request. Imported suggestions never appear on the profile or in a share
post.

### Domain Examples

#### 1: Happy path (the refusal is visible)

Priya's `rust-lang/cargo` card says "Not yours to publish. Only an owner of github.com/rust-lang
can publish claims about this repo. This suggestion stays private." It offers Decline only.

#### 2: Adversarial: forged request

Sam Ortega imported `BurntSushi/ripgrep` and sends a publish request directly. Nothing is written
to his PDS; he sees "github.com/BurntSushi/ripgrep is not owned by an account you verified, so
nothing was published. Decline this suggestion, or verify the GitHub account that owns the repo."

#### 3: Error: GitHub unreachable at confirm

GitHub is down when Priya confirms a claim about `priyaraman/tidepool`: "GitHub could not be
reached, so we couldn't check who owns github.com/priyaraman/tidepool. Nothing was published.
Please try again in a few minutes." The suggestion stays pending.

### UAT Scenarios (BDD)

#### Scenario: An imported suggestion offers no way to publish and says who can

Given Priya has a pending suggestion for rust-lang/cargo, a repo she imported
When she opens her queue
Then its card says "Not yours to publish. Only an owner of github.com/rust-lang can publish claims about this repo."
And it has no Select, Approve or Edit control, only Decline

#### Scenario: A forged publish request for a repo he doesn't own writes nothing

Given Sam Ortega imported BurntSushi/ripgrep and has a pending suggestion for it
When a publish request for that suggestion reaches the app without the page
Then no record is written to Sam's PDS
And Sam sees "github.com/BurntSushi/ripgrep is not owned by an account you verified, so nothing was published. Decline this suggestion, or verify the GitHub account that owns the repo."

#### Scenario: Profile and share preview never show imported suggestions

Given Priya has 2 published claims and 4 pending suggestions for rust-lang/cargo
When she opens her profile and the share preview
Then neither mentions rust-lang/cargo

#### Scenario: GitHub being unreachable at confirm publishes nothing

Given Priya previewed a claim about priyaraman/tidepool
And GitHub cannot be reached when she confirms
When she presses "Publish to my repo"
Then nothing is written to her PDS
And she sees that GitHub could not be reached, nothing was published, and to try again in a few minutes
And the suggestion is still pending

@property
#### Scenario: No request publishes a suggestion about a repo the owner doesn't own

Given any signed-in DID and any suggestion whose repo no currently verified account of that DID owns
When any preview, approve, approve-selected, edit-preview, retry or confirm request names that suggestion
Then no record is written to any PDS
And the suggestion stays pending or declined as it was

#### Scenario: An imported repo moves to the owned list when its owner account is verified

Given Priya imported priya-tidal/tidal-sdk-notes and has a pending suggestion for it
And priya-tidal has no saved choice and room is left under the limit
When she verifies github.com/priya-tidal
Then tidal-sdk-notes is no longer under "Repos you don't own"
And it is ticked under "github.com/priya-tidal" in that account's saved choice
And its suggestion shows Approve, and publishing it writes the record after the ownership check passes

#### Scenario: A scan moves an imported repo that one of her accounts now owns

Given Priya imported rust-lang/cargo-plugin-demo and has 1 pending suggestion for it
And the repo was later transferred to priya-tidal, which is on her list and verified
When she presses "Scan again"
Then the repo appears as priya-tidal/cargo-plugin-demo under "github.com/priya-tidal" and no longer under "Repos you don't own"
And it is ticked in priya-tidal's saved choice (created from priya-tidal's current ticks if it had none)
And her total of repos toward the limit is unchanged
And its pending suggestion is kept and shows Approve

### Acceptance Criteria

- [ ] AC-006.1 Imported cards show "Not yours to publish", name who can publish, and offer Decline only. Tested by: "An imported suggestion offers no way to publish…".
- [ ] AC-006.2 For every request path, a suggestion failing BR-RRA-2 causes zero PDS writes, and the refusal names the repo, says nothing was published and gives the next step. Tested by: "A forged publish request…", the @property scenario.
- [ ] AC-006.3 Imported suggestions never appear on the profile or in a share post. Tested by: "Profile and share preview never show…".
- [ ] AC-006.4 If GitHub cannot be read at confirm, nothing is written, the suggestion stays pending, and the message says to try again in a few minutes. Tested by: "GitHub being unreachable at confirm…" (preview: US-RRA-010).
- [ ] AC-006.5 When an account on the list owns an imported repo (at verify or seen by a scan), the repo moves to that account's owned list per BR-RRA-5, its suggestions are kept and become publishable subject to BR-RRA-2. Tested by: "An imported repo moves to the owned list…", "A scan moves an imported repo…".

### Outcome KPIs

- **Who**: all users | **Does what**: PDS writes of suggestions about repos no verified account of the writer owns | **By how much**: 0 (guardrail KPI-RRA-4) | **Measured by**: CI property over every publish request path plus a production audit (every PDS write has a same-request ownership pass) | **Baseline**: n/a.

### Technical Notes

- Today's publish path re-checks nothing about the repo (owner-only-signing code reading). OOS-D13 option A (accepted, not yet delivered) adds a publish-time authority check; DESIGN should build one check that serves both (OQ-6).
- Freshness follows C-GDL-10 (same request, no cache). Cost per NFR-RRA-4.
- Depends on US-RRA-005 for imported suggestions.

---

## US-RRA-010: Publishing follows who owns the repo right now

- **job_id**: J-009 (sub-job J-009d)
- **Release**: R2 | **MoSCoW**: Must | **Priority**: P2 | **Estimate**: 1.5 days

### Elevator Pitch

- **Before**: a suggestion found while Priya owned `kelp` stays publishable after she transfers `kelp` to `tidal-labs`, and a suggestion from `priya-tidal` stays publishable after she removes her DID from that bio.
- **After**: when Priya opens the preview for `kelp` she sees "github.com/priyaraman/kelp is no longer owned by an account you verified, so nothing was published. If you own it under another account, verify that account; otherwise decline this suggestion."
- **Decision enabled**: Priya decides what to publish knowing the app checks ownership as it is today, not as it was at scan time.

### Problem

Ownership changes: repos are transferred and bios are edited. With several accounts (R2), more
repos become publishable, so publishing must follow ownership as it is at the moment of
publishing (BR-RRA-2), not as it was at scan time.

### Who

- Bluesky developer | publishes days after a scan | whose repos or accounts changed meanwhile.

### Solution

The BR-RRA-2 check at preview and confirm, for every owned suggestion, from R2 on. (The BR-RRA-5
move of imported repos ships with imports, in US-RRA-006.)

### Domain Examples

#### 1: Transferred away

Priya transferred `priyaraman/kelp` to `tidal-labs` after her scan. Preview is refused with the
message above.

#### 2: GitHub down at preview

GitHub cannot be reached when Priya opens a preview for `priyaraman/tidepool`: "GitHub could not
be reached, so we couldn't check who owns github.com/priyaraman/tidepool. Nothing was published.
Please try again in a few minutes."

#### 3: Bio edited between preview and confirm

Priya previews a claim about `priya-tidal/ingest-bench`, then removes her DID from the
`priya-tidal` bio before confirming. Confirm is refused: "Your DID is no longer in the bio of
github.com/priya-tidal, so nothing was published. Put your DID back, verify the account, then
publish again."

### UAT Scenarios (BDD)

#### Scenario: A repo transferred away after the scan can no longer be published

Given Priya has a pending suggestion for priyaraman/kelp
And kelp was transferred to tidal-labs after her scan
When she opens its preview
Then she sees "github.com/priyaraman/kelp is no longer owned by an account you verified, so nothing was published. If you own it under another account, verify that account; otherwise decline this suggestion."
And nothing is written to her PDS

#### Scenario: GitHub being unreachable at preview shows no preview and publishes nothing

Given Priya has a pending suggestion for priyaraman/tidepool
And GitHub cannot be reached
When she opens its preview
Then she sees "GitHub could not be reached, so we couldn't check who owns github.com/priyaraman/tidepool. Nothing was published. Please try again in a few minutes."
And nothing is written to her PDS, the suggestion stays pending and priyaraman keeps its verified status

#### Scenario: A bio edited between preview and confirm stops the publish

Given Priya previewed a claim about priya-tidal/ingest-bench
And she removed her DID from the bio of github.com/priya-tidal before confirming
When she presses "Publish to my repo"
Then nothing is written to her PDS
And she sees "Your DID is no longer in the bio of github.com/priya-tidal, so nothing was published. Put your DID back, verify the account, then publish again."
And priya-tidal is marked as needing re-verify

### Acceptance Criteria

- [ ] AC-010.1 Preview and confirm each apply BR-RRA-2 in the request; a repo transferred away is refused with the reason and next step. Tested by: "A repo transferred away…".
- [ ] AC-010.2 A bio that no longer holds the DID at confirm refuses the publish and marks the account unverified. Tested by: "A bio edited between preview and confirm…".
- [ ] AC-010.3 If GitHub cannot be read at preview, no preview is shown, nothing is written, and the account keeps its status (an unfinished check is not a failed check). Tested by: "GitHub being unreachable at preview…".

### Outcome KPIs

- **Who**: all users | **Does what**: publishes of a repo not owned by a verified account at confirm time | **By how much**: 0 (guardrail, part of KPI-RRA-4) | **Measured by**: same-request ownership audit | **Baseline**: unknown today (no re-check exists).

### Technical Notes

- Ships in R2 with the second account; needs no imports. In R3, US-RRA-006 reuses the same check for imported suggestions.
- OQ-6 (shared check with owner-only-signing). Depends on the live publish path only.

---

## US-RRA-007: Publish suggestions for a repo I administer but don't own (separable)

- **job_id**: J-009 (sub-job J-009d)
- **Release**: R4 (backlog) | **MoSCoW**: Won't (this feature) | **Priority**: P4 | **Estimate**: 2-3 days once a proof method is chosen
- **Status**: DEFERRED (user, 2026-10-19). Kept in the backlog; no proof method is chosen. R1-R3 ship with owned-repo publishing only.

### Elevator Pitch

- **Before**: Priya administers `tidal-labs/streamline` but her accounts don't own it, so its suggestions stay "Not yours to publish" forever.
- **After**: once the app has proof that Priya administers `tidal-labs/streamline`, its cards on her queue say "You administer this repo" and show Approve; "Publish to my repo" writes the claim after a fresh admin check.
- **Decision enabled**: Priya decides which claims about her team's repo to publish as its administrator.

### Problem

Priya administers the org repo `tidal-labs/streamline`. The user's rule allows owners **or
admins** to publish claims about a repo. GitHub's public data shows who owns a repo but not who
administers it, so the app cannot prove it today.

### Who

- Bluesky developer | administers an org repo she doesn't own | wants to publish about it.

### Solution

A second publish eligibility, "administered", proven fresh at preview and confirm by a mechanism
to be chosen later (deferred, D-RRA-12). Without proof the repo stays "Not yours to publish". The label says
"administered", never "owner".

### Domain Examples

#### 1: Happy path

The app holds a current proof that Priya administers `tidal-labs/streamline`. Its
`semantic-versioning` card says "You administer this repo" and shows Approve. She publishes at
confidence 0.40.

#### 2: Edge: no proof

Priya imported `tidal-labs/streamline` but no proof exists: its cards stay "Not yours to publish".

#### 3: Error: proof lost before confirm

Priya's admin role was removed between preview and confirm: "We couldn't confirm that you
administer github.com/tidal-labs/streamline, so nothing was published. Ask an owner of the repo
to restore your admin rights, then publish again, or decline this suggestion."

### UAT Scenarios (BDD)

#### Scenario: An administered repo's suggestion can be published and is labelled administered

Given Priya imported tidal-labs/streamline and the app holds a current proof that she administers it
When she opens its suggestion
Then the card says "You administer this repo", never "owner", and shows Approve
And publishing writes the claim to her PDS after a fresh admin check

#### Scenario: Without proof an org repo stays unpublishable

Given Priya imported tidal-labs/streamline and the app holds no proof that she administers it
When she opens her queue
Then its suggestions say "Not yours to publish"

#### Scenario: Admin rights lost before confirm stop the publish

Given Priya previewed a claim about tidal-labs/streamline as its administrator
And her admin rights were removed before she confirmed
When she presses "Publish to my repo"
Then nothing is written to her PDS
And she sees "We couldn't confirm that you administer github.com/tidal-labs/streamline, so nothing was published. Ask an owner of the repo to restore your admin rights, then publish again, or decline this suggestion."

#### Scenario: Declining to grant extra access keeps the repo unpublishable

Given proving admin rights needs a permission Priya has not granted
When she is asked for it and declines
Then nothing changes on GitHub or in her PDS
And tidal-labs/streamline suggestions stay "Not yours to publish"

### Acceptance Criteria

- [ ] AC-007.1 A repo is publishable as "administered" only with an admin proof read fresh at preview and confirm. Tested by: "An administered repo's suggestion can be published…".
- [ ] AC-007.2 Without a proof, or when it cannot be read, the repo stays "Not yours to publish" and nothing is written; a refusal gives the next step. Tested by: "Without proof…", "Admin rights lost…".
- [ ] AC-007.3 "Administered" and "owner" use different labels. Tested by: "An administered repo's suggestion can be published…".
- [ ] AC-007.4 Any extra access the proof needs is opt-in; declining it leaves the repo unpublishable and changes nothing. Tested by: "Declining to grant extra access…".

### Outcome KPIs

- **Who**: owners who imported an org repo they administer | **Does what**: publish ≥1 claim about it | **By how much**: ≥1 dogfood claim confirmed by a second reader, then ≥20% within 60 days (hypothesis) | **Measured by**: `publish.administered` | **Baseline**: 0.

### Technical Notes

- Proof options, none chosen (D-RRA-12): (1) GitHub sign-in reading `permissions.admin` (exact, signing-time only, new token storage, leaves NFR-BRA-8); (2) the `.github/openlore-authority` listing accepted for the CLI (owner-only-signing OOS-D5 A, label `repo-listed`, residual risk R-1); (3) keep deferred.
- Depends on US-RRA-006/010 (one eligibility check to extend) and a future choice of proof method.
