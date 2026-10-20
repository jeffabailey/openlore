<!-- markdownlint-disable MD024 -->
# User Stories: owner-only-signing (derived)

> **Derived after the fact, not discovered.** This feature had no DISCUSS wave and no user
> interviews. DESIGN (`design/wave-decisions.md`, 2026-10-14) worked from the user's one-paragraph
> requirement and recorded its reading as assumptions A-1..A-14, which the user accepted. DISTILL
> (`distill/test-scenarios.md` §3, 2026-10-20) then derived stories S-OOS-1..9 from DESIGN. This
> file restates those nine stories in the house LeanUX shape (job_id, persona, Elevator Pitch,
> AC). IDs and titles match test-scenarios.md §3 exactly. It adds no requirement that DESIGN does
> not already hold.
>
> - **Requirement (verbatim)**: "it should be possible for someone to import claims into the
>   database without being the owner of an account but only the owner of the account can sign
>   those claims. If possible, also make it so only an owner or admin of a repo can sign claims
>   about the repo but let anyone import a repo and generate unsigned claims."
> - **UAT scenarios**: the Gherkin lives in the `tests/acceptance/owner_only_signing*.rs` doc
>   comments (the SSOT) and is extracted in `distill/test-scenarios.md` §6. Each AC below names
>   the scenario IDs that test it, so the Gherkin is not repeated here.
> - **Personas**: P-001 Senior Engineer Solo Builder (`docs/product/personas/senior-engineer-solo-builder.yaml`),
>   played by Jeff Bailey (`github:jeffbailey`). The other people
>   named are peers: Rachel K (`rachelk`), Tobias W (`tobiasw`), Sven, and the organization
>   `acme-tools`. In the review app (S-OOS-8) the persona is P-003 Priya Raman from
>   `docs/feature/bluesky-claim-review-app/discuss/requirements.md`.
> - **Prerequisite for every story**: github-did-identity-link (GDL) slice 1 (identity link and
>   v7 link checks, ADR-084/085).
> - **Job fit**: no job in `docs/product/jobs.yaml` is about the subject's consent ("only the
>   owner speaks for an account or repo"). Each story uses the closest existing job. See
>   "Proposed job (not added)" at the end.

## System Constraints (cross-cutting, apply to every story)

- **C-OOS-1** No OpenLore composer signs or publishes a `github:` assertion without a signing
  permit from an attested authority check no older than 7 days. Online verbs refresh the check in
  the same invocation.
- **C-OOS-2** Retractions and counter-claims are never refused for lack of authority.
- **C-OOS-3** Authority proofs use only public GitHub data, and any reader can re-check them.
- **C-OOS-4** Authority is decided by each reader on their own machine, from current GitHub data.
  It is never federated. Stored checks are insert-only.
- **C-OOS-5** Nothing is deleted, hidden on the wire, retracted or re-signed on anyone's behalf.
- **C-OOS-6** Scoring counts only assertions that are either not about `github:` or attested.
  Counters, retractions and identity links are never counted. Unattested assertions are shown
  separately, per author, and never merged.
- **C-OOS-7** Anyone may still import unsigned candidates. Imported candidates are never
  federated and never feed inference or scoring.
- **C-OOS-8** The network indexer gains no GitHub capability.
- **C-OOS-9** Signing never calls the network. Accepted exception to I-9: a `github:` assertion
  without a local authority check from the last 7 days is refused when offline.

Scope (A-1, OOS-D1): only subjects `github:<login>` and `github:<owner>/<repo>`. Philosophy
records and non-GitHub subjects are not gated.

---

## S-OOS-1: Only the owner of a GitHub account signs about it; others get the hand-off

- **job_id**: J-001 (Author a signed philosophical claim)
- **Persona**: P-001 (Jeff), with peers Rachel and Tobias as subjects
- **Slice**: 1 (walking skeleton) | **Estimate**: 2 days
- **From**: A-1, A-2, OOS-D4, C-OOS-1, KPI-OOS-1

### Elevator Pitch

- **Before**: Jeff can run `openlore claim add` with subject `github:rachelk`, sign it with his key and publish it. Readers see a signed claim about Rachel that she never agreed to.
- **After**: Jeff runs `openlore claim add` about `github:rachelk` and sees that nothing was signed or published, plus the hand-off: "can be signed by: rachelk (after `openlore identity link github rachelk`)". The same command about `github:jeffbailey`, which he has linked, signs and publishes.
- **Decision enabled**: Jeff decides whether to sign a claim about his own account, or to pass the candidate to the person it is about.

### Problem

Jeff is a senior engineer who writes signed claims from his terminal. Today any DID can sign
anything about any GitHub account, so a claim about `rachelk` can look like Rachel's own word.
The user's rule is that only the owner of an account signs claims about it. Jeff needs the tool
to refuse claims about other people's accounts and to tell him who can sign them instead.

### Who

- Senior engineer, CLI user, has published an identity link to `github:jeffbailey` | wants his
  signed claims to carry weight because they are his own.

### Domain Examples

#### 1: Happy path

Jeff linked `jeffbailey`. `openlore claim add` "jeffbailey adheres to memory-safety" is signed
and reaches his published repo (WS-OOS-1, row 1).

#### 2: Error: someone else's account

Jeff, linked as `jeffbailey`, signs "tobiasw adheres to memory-safety". He is told "you have no
identity link to tobiasw" and that tobiasw can sign it after linking. Nothing is signed, published
or recorded (PS-2, row 3).

#### 3: Edge: renamed account

Jeff renamed his GitHub account to `jeff-b` and re-checked his link. A claim about `jeff-b` signs.
A claim about `jeffbailey` is refused because that is no longer the login of his linked account
(PS-8).

#### 4: Inference

`openlore infer people` proposes memory-safety for both `jeffbailey` and `BurntSushi`. Jeff can
sign the first. The second says "can be signed by BurntSushi after `openlore identity link github
BurntSushi`" (PS-5, PS-6).

### Acceptance Criteria

- [ ] AC-1.1 Jeff signs and publishes a claim about the account he has linked and verified. Tested by: WS-OOS-1 row 1, PS-7.
- [ ] AC-1.2 A claim about any other account is refused before signing. Nothing is signed, published or stored, and the message names the account's owner and the `openlore identity link github <login>` command. Tested by: WS-OOS-1 row 2, PS-2, RA-2 (person row).
- [ ] AC-1.3 With no link, or a link that no longer verifies, even a claim about his own login is refused with the reason. Tested by: PS-2 rows 1-2.
- [ ] AC-1.4 `infer people --sign` signs only the candidate about Jeff. Listing inferred people says who can sign each candidate and writes nothing. Tested by: PS-5, PS-6.
- [ ] AC-1.5 A bio that names a second identity does not block the linked owner. Tested by: PS-7.
- [ ] AC-1.6 After a rename, only the current login can be signed. Tested by: PS-8.
- [ ] AC-1.7 The person gate follows the ADR-087 rule table for every input. Tested by: CORE-OOS-2, CORE-OOS-3, CORE-OOS-4.

### Outcome KPIs

- **Who**: every OpenLore composer | **Does what**: signs or publishes a `github:` person assertion without a permit | **By how much**: 0 occurrences (guardrail KPI-OOS-1) | **Measured by**: CI property plus the ATs above | **Baseline**: unrestricted today.

### Dependencies

- GDL slice 1 (published identity link and v7 link checks). Without it nobody can sign about a person.

---

## S-OOS-2: Anyone can always dispute; the author can always withdraw

- **job_id**: J-003 (sub-job J-003b, counter-claim as first-class disagreement)
- **Persona**: P-001 (Jeff), disputing Rachel
- **Slice**: 1 | **Estimate**: 1 day
- **From**: OOS-D11, C-OOS-2, KPI-OOS-3

### Elevator Pitch

- **Before**: counters and retractions copy the original claim's subject, so a gate on `github:` subjects would block Jeff from disputing Rachel's claim about her own account and from withdrawing his own old claims.
- **After**: Jeff runs `openlore claim counter <cid>` against "rachelk adheres to memory-safety" and sees the dispute signed and published. `openlore claim retract <cid>` on his own claim about `jeffbailey` succeeds even after his bio stopped naming him.
- **Decision enabled**: Jeff decides to disagree in public, or to take back his own word, whoever owns the subject.

### Problem

Owner-only signing must not make owners impossible to dispute or make a claim impossible to
withdraw. ADR-015 says disagreement is a counter-claim. ADR-008 says the author may always retract.

### Who

- Senior engineer | reads peers' claims | disagrees with some, and changes his own mind.

### Domain Examples

#### 1: Dispute an owner

Rachel published "rachelk adheres to memory-safety". Jeff pulled it and disputes it. The counter
is signed and published (PS-3, row 1).

#### 2: Withdraw after losing authority

Jeff published about `jeffbailey`, then his bio stopped naming his DID. He withdraws his claim, and
the retraction is signed and published (PS-3, row 2).

#### 3: Spoofed dispute does not gain weight

A claim marked as disputing a claim nobody has is still a dispute. It needs no authority and
never adds weight (CORE-OOS-1, see S-OOS-7 RD-4).

### Acceptance Criteria

- [ ] AC-2.1 A counter-claim against any claim, including an owner's, is signed and published whatever authority Jeff holds. Tested by: PS-3 row 1.
- [ ] AC-2.2 Jeff can always retract his own claim, even after losing authority over its subject. Tested by: PS-3 row 2.
- [ ] AC-2.3 Only disputes, withdrawals and identity links are exempt. A malformed `github:` subject is never exempt. Tested by: CORE-OOS-1, CORE-OOS-2.

### Outcome KPIs

- **Who**: any author | **Does what**: has a retraction or counter refused for authority | **By how much**: 0 (guardrail KPI-OOS-3) | **Measured by**: PS-3 | **Baseline**: 0 today, which must hold.

### Dependencies

- GDL slice 1. S-OOS-1 (the gate this story exempts from).

---

## S-OOS-3: Signing works offline from a check ≤ 7 days old, never otherwise

- **job_id**: J-001 (functional: "persist locally first; publishing is a separate step")
- **Persona**: P-001 (Jeff, on a train with no network)
- **Slice**: 1 (person checks) and 2 (repo checks) | **Estimate**: 1.5 days
- **From**: OOS-D7, C-OOS-9, I-9 exception, A-6

### Elevator Pitch

- **Before**: signing never needs the network (I-9), but under owner-only signing nothing would say how old an authority check may be when Jeff signs offline.
- **After**: offline on 3 October, Jeff runs `openlore claim add` about `github:jeffbailey` and it is signed and kept locally from his 1 October check. On 8 October at 09:01 the same command signs nothing and tells him to run `openlore authority verify --subject github:jeffbailey`.
- **Decision enabled**: Jeff decides whether to sign now from a recent check or to re-check first when he is back online.

### Problem

Jeff signs claims offline and publishes later. The user accepted a narrow exception to I-9: a
`github:` claim can be signed offline only from a local check at most 7 days old. GitHub is never
called at signing time.

### Who

- Senior engineer | often offline | expects local-first signing to keep working.

### Domain Examples

#### 1: Within the window

Link checked 1 October 09:00; Jeff signs on 3 October and on 8 October at 09:00 (exactly 7 days).
Both are kept locally (PS-4 rows 1-2).

#### 2: Boundary: one minute late

On 8 October at 09:01 nothing is signed, and the message names `openlore authority verify
--subject github:jeffbailey` (PS-4 row 3).

#### 3: Error: never checked, or check dated in the future

Jeff linked `jeffbailey` but `jeffbailey/tidepool` was never observed, so the repo claim is refused
because authority has not been checked (RA-10 row 4). A check dated after now counts as missing
(PS-4 row 4).

#### 4: Repo not found

`acme-tools/vanished` was recorded as not found. Offline, a claim about it is refused because it
was not found on GitHub (RA-15).

### Acceptance Criteria

- [ ] AC-3.1 Offline, a person or repo claim signs when every check it needs is attested and between 0 and 7 days old. GitHub is not asked anything. Tested by: PS-4 rows 1-2, RA-10 rows 1-2.
- [ ] AC-3.2 Older than 7 days, dated in the future, or never checked: nothing is signed, and the message names the `openlore authority verify` command. Tested by: PS-4 rows 3-4, RA-10 rows 3-4.
- [ ] AC-3.3 A repo recorded as not found cannot be signed. Tested by: RA-15.
- [ ] AC-3.4 The permit exists exactly when the claim is exempt or freshly attested, with edges at 0, 7 days, 7 days + 1 minute and -1 minute. Tested by: CORE-OOS-4, CORE-OOS-4x.

### Outcome KPIs

- **Who**: Jeff signing offline | **Does what**: signs a `github:` claim from a check older than 7 days | **By how much**: 0 (part of KPI-OOS-1) | **Measured by**: PS-4, RA-10, CORE-OOS-4 | **Baseline**: no window today.

### Dependencies

- GDL slice 1 (v7 link checks). Repo rows need slice 2 (v8 `repo_authority_checks`). The DELIVER note U-OOS-2 (WS-10 subject) is tracked in `distill/wave-decisions.md`.

---

## S-OOS-4: Only the repo's owner or a DID its authority file lists signs about the repo; anyone imports

- **job_id**: J-004 (sub-job J-004c, "the human always signs, the scraper never asserts")
- **Persona**: P-001 (Jeff), importing his own repo, Rachel's repo and an org repo
- **Slice**: 2 | **Estimate**: 3 days
- **From**: A-3, A-12, OOS-D5, ADR-087 §2-3, KPI-OOS-4

### Elevator Pitch

- **Before**: Jeff can run `openlore scrape github rachelk/kelp --sign 1` and publish a signed claim about Rachel's repo as if it were the owner's word.
- **After**: `openlore scrape github rachelk/kelp --sign 1` still lists every candidate, signs nothing, and says the repo "is not owned by your linked GitHub account" and can be signed by rachelk after linking, or by a DID in `.github/openlore-authority`, using `openlore scrape github rachelk/kelp --sign N`. `openlore scrape github jeffbailey/tidepool --sign 1` signs and publishes.
- **Decision enabled**: Jeff decides which repos he can speak for, and for an org repo he decides whether to ask its maintainers to list his DID.

### Problem

The user asked that only an owner or admin of a repo sign claims about it, while anyone may still
import a repo and see unsigned candidates. GitHub's admin role is not public, so DESIGN uses two
public proofs. The owner tier: a personal repo whose owner account is the one Jeff has linked. The
repo-listed tier: the repo's maintainers list Jeff's DID in `.github/openlore-authority` on the
default branch. Org repos can only use the second.

### Who

- Senior engineer | maintains personal repos and contributes to org repos | wants to sign about
  repos he is responsible for and only those.

### Domain Examples

#### 1: Own repo

Jeff linked `jeffbailey`, which owns `jeffbailey/tidepool`. Importing and signing candidate 1
publishes it, and one authority observation is recorded: user-owned, no file (RA-1).

#### 2: Org repo with an authority file

`acme-tools/anvil`'s file lists Jeff's DID exactly, so his claim is signed. If the file lists the
DID in capitals, lists `did:plc:test-jeffrey`, comments it out, or lists it only on another
branch, the claim is refused. A symlink, submodule, directory, a file over 4 KiB, or a file that
is not plain text is refused as not a valid authority file. A file of exactly 4 KiB is accepted
(RA-3).

#### 3: Someone else's repo

`scrape github rachelk/kelp` lists the candidates. Signing is refused with the hand-off (RA-2).

#### 4: Renamed repo, stale link

After renaming `tidepool` to `tidepool-rs`, only the new name signs (RA-8). If his link was last
checked on 1 October, `--sign` on 21 October re-reads his profile first and refuses if the bio no
longer names him (RA-13).

### Acceptance Criteria

- [ ] AC-4.1 Jeff signs about a personal repo owned by his linked, verified account. Tested by: RA-1.
- [ ] AC-4.2 Any public repo or person can be imported. Candidates are always listed, and signing what Jeff cannot attest is refused with the reason, who can sign, and the command they would run. Tested by: RA-2.
- [ ] AC-4.3 An org repo is signed only by a DID that its default-branch `.github/openlore-authority` lists as an exact token. Every malformed or look-alike file is refused. Tested by: RA-3, CORE-OOS-3.
- [ ] AC-4.4 After a repo rename, only the name GitHub serves now can be signed. Tested by: RA-8.
- [ ] AC-4.5 `scrape ... --sign` re-checks the signer's own link before signing. Tested by: RA-13.

### Outcome KPIs

- **Who**: repo maintainers using the CLI | **Does what**: publish at least 1 `repo-listed` claim on an org repo and at least 1 `owner` claim, each confirmed by a second reader | **By how much**: ≥ 1 of each (leading KPI-OOS-4) | **Measured by**: `openlore authority verify` on a second machine | **Baseline**: 0.

### Dependencies

- GDL slice 1 (owner tier). The repo-listed tier works without a link. Residual risk R-1: anyone with push access can list a DID (A-3).

---

## S-OOS-5: Anyone re-checks authority from public data; failures change nothing

- **job_id**: J-003 (sub-job J-003a, "see who is making the claim and weigh their authority myself")
- **Persona**: P-001 (Jeff, reading Rachel's pulled claims)
- **Slice**: 2 | **Estimate**: 2 days
- **From**: OOS-D6, ADR-087 §4-6, C-OOS-3, C-OOS-4

### Elevator Pitch

- **Before**: Jeff pulls Rachel's claims about `acme-tools/anvil` and `jeffbailey/tidepool` and cannot tell whether either is the owner's word or an outsider's opinion.
- **After**: Jeff runs `openlore authority verify` and sees Rachel's anvil claim labelled "repo-listed" and her tidepool claim "third-party". When GitHub is down he sees "GitHub could not be asked", and nothing in his store changes.
- **Decision enabled**: Jeff decides how much weight to give each peer's claim, from proof he checked himself.

### Problem

Authority must be something every reader can re-check from public GitHub data, not something the
signer asserts. Repos change hands, so the label has to reflect current ownership. A failed or
partial check must never leave a half-written result.

### Who

- Senior engineer | subscribes to peers | wants to verify, not trust.

### Domain Examples

#### 1: Re-check labels claims

Rachel is listed in anvil's file. After `openlore authority verify`, her anvil claim reads
"repo-listed" and her tidepool claim reads "third-party". Exactly one observation of each repo is
recorded, from one repo read and one file read each (RA-4).

#### 2: Transfer

Jeff transferred `tidepool` to `acme-tools`. After a re-check his earlier claim reads
"third-party", and it is not retracted, deleted or re-signed (RA-7).

#### 3: GitHub fails

GitHub is rate-limiting, unreachable, or redirects the repo read to another site. Jeff is told
GitHub could not be asked, and nothing is recorded (RA-5). If the budget runs out mid-import, the
candidates are still listed and signing waits for the reset (RA-6).

#### 4: Nothing to check

With no `github:` claims in his store, `openlore authority verify` says there is nothing to
re-check and asks GitHub nothing (RA-14).

### Acceptance Criteria

- [ ] AC-5.1 `openlore authority verify` labels each pulled `github:` repo claim from one repo read and one authority-file read per repo, and records one observation per repo. Tested by: RA-4.
- [ ] AC-5.2 A listed identity stays listed whoever observed the repo last. Tested by: RA-9.
- [ ] AC-5.3 Losing ownership changes the label only. Nothing is withdrawn or re-signed. Tested by: RA-7.
- [ ] AC-5.4 Busy, unreachable, cross-host redirect, or exhausted budget: nothing is recorded or changed, and the message says GitHub could not be asked and when to retry. Tested by: RA-5, RA-6.
- [ ] AC-5.5 With no `github:` claims stored, no GitHub request is made. Tested by: RA-14.

### Outcome KPIs

- **Who**: readers running `authority verify` | **Does what**: get a label from public data alone, with no token required | **By how much**: 100% of checked repo claims labelled `owner`, `repo-listed`, `third-party` or `not found` (feeds KPI-OOS-4's second-reader confirmation) | **Measured by**: RA-4, RA-9 | **Baseline**: no labels today.

### Dependencies

- Slice 2 adapter work (`read_repo_authority`, v8 `repo_authority_checks`). GDL slice 1 for the owner tier.

---

## S-OOS-6: A never-published claim publishes only with a permit

- **job_id**: J-001 (functional: "publishing to ATProto is a separate, deferrable step")
- **Persona**: P-001 (Jeff, with a store from before this feature)
- **Slice**: 2 | **Estimate**: 1.5 days
- **From**: OOS-D8, ADR-086 §4, OOS-D3

### Elevator Pitch

- **Before**: Jeff's store holds a claim about `BurntSushi/ripgrep` that he signed but never published, from before owner-only signing. `openlore claim publish <cid>` would push it out.
- **After**: `openlore claim publish <cid>` refuses it. Nothing reaches his published repo, and he is told the authority of `BurntSushi/ripgrep` has not been checked, with the `openlore authority verify` command. A claim he already published re-publishes unchanged.
- **Decision enabled**: Jeff decides whether to check authority and publish, or to leave an old claim private.

### Problem

`claim publish` is a sign site too: it puts a record attributed to Jeff's DID on the network. A
claim signed before the gate existed must not slip out through it. A claim already published
must not be blocked or rewritten, because that changes nothing on the network.

### Who

- Senior engineer | upgrading an existing store | has signed but unpublished claims.

### Domain Examples

#### 1: Pre-feature unpublished claim

Jeff's unpublished claim about `BurntSushi/ripgrep` is refused at publish (RA-11a).

#### 2: Own repo, publish later

Tidepool was re-checked on 1 October. Jeff signed without publishing and publishes on 8 October.
It reaches his published repo (RA-11b row 1).

#### 3: Transferred before publishing

He signed without publishing, transferred tidepool and re-checked. Publish is refused with "is
owned by the organization", and nothing is published (RA-11b row 2). If it had already been
published, publishing again succeeds and changes nothing (row 3).

#### 4: Upgrade keeps everything

Opening a store made before identity links and owner-only signing records both upgrades once and
leaves every claim, peer claim and contributor unchanged (RA-12).

### Acceptance Criteria

- [ ] AC-6.1 Publishing a claim with `published_at` empty requires a permit, as signing does. Tested by: RA-11a, RA-11b rows 1-2.
- [ ] AC-6.2 Publishing a claim that is already published needs no authority and leaves the published repo unchanged. Tested by: RA-11b row 3.
- [ ] AC-6.3 The v8 upgrade runs once and changes no existing row. Tested by: RA-12.

### Outcome KPIs

- **Who**: users with pre-feature stores | **Does what**: publish a never-published `github:` claim without a permit | **By how much**: 0 (part of KPI-OOS-1) | **Measured by**: RA-11a/b | **Baseline**: unrestricted today.

### Dependencies

- S-OOS-4 and S-OOS-5 (repo checks), v8 migration.

---

## S-OOS-7: Readers label every `github:` assertion and weigh only attested ones

- **job_id**: J-002 (sub-job J-002c, "transparent, auditable adherence weighting")
- **Persona**: P-001 (Jeff, choosing between repos on memory-safety)
- **Slice**: 3 | **Estimate**: 3 days
- **From**: OOS-D9, OOS-D10, C-OOS-5, C-OOS-6, C-OOS-8, KPI-OOS-2, UC-7/11/16/20

### Elevator Pitch

- **Before**: `openlore graph query --weighted` counts Rachel's claim about `jeffbailey/tidepool` as much as Jeff's own, and counter-claims even add weight (A-13).
- **After**: `openlore graph query memory-safety --weighted --explain` weights tidepool from Jeff's own claim only and says "2 claims not counted: 1 third-party, 1 authority not checked", with the re-check command. `--include-unattested` lists Rachel's left-out claims under her name in a separate section, and the weighted rows stay the same.
- **Decision enabled**: Jeff decides which repos really embody memory-safety, knowing whose word each weight rests on.

### Problem

If anyone's claim about any repo counts, owner-only signing means nothing to readers, and
claims signed by other clients bypass it entirely. Readers must label every `github:` assertion
with whose word it is, count only attested ones, and still show the rest instead of hiding them.

### Who

- Senior engineer | uses the weighted view and the viewer to pick dependencies | wants
  transparent, reproducible weights.

### Domain Examples

#### 1: Labels

Jeff's tidepool claim reads "owner", Rachel's anvil claim "repo-listed", Rachel's tidepool claim
"third-party", and her kelp claim "authority not checked". None is hidden (RD-1).

#### 2: Weight only moves for attested claims

A dispute, a spoofed dispute, a claim and its withdrawal, an identity link, a claim not listed in
the file, or a foreign withdrawal of Jeff's claim: none of them changes anvil's weight (RD-4).

#### 3: Network search

Sven's claim about `rust-lang/regex` reads "authority not checked here". Priya's claim about anvil
reads "repo-listed" once Jeff has checked anvil himself (RD-7).

#### 4: Old claims, same rule

Claims made before this feature, such as Jeff's about `BurntSushi/ripgrep`, are judged by the same
rule with no cut-off date and read "third-party" (RD-8). After Jeff renames his account, his claim
about the old login reads "third-party" and stays published (RD-10).

### Acceptance Criteria

- [ ] AC-7.1 Every `github:` assertion in `graph query`, `infer people`, scrape views, the viewer and `search` carries one label: `owner`, `repo-listed`, `third-party` or `authority not checked`. Nothing is hidden. Tested by: RD-1, RD-7, RD-8, RD-9, RD-10.
- [ ] AC-7.2 The weighted view counts only attested assertions and assertions not about `github:`, and states what it left out with the re-check command. Tested by: RD-2.
- [ ] AC-7.3 `--include-unattested` adds a per-author section and leaves the weighted rows unchanged. Tested by: RD-3.
- [ ] AC-7.4 Counters (including spoofed ones), retractions, identity links and unattested assertions never change the weight. Tested by: RD-4, CORE-OOS-5.
- [ ] AC-7.5 A dispute is shown as a dispute, with no authority label. Tested by: RD-5.
- [ ] AC-7.6 The viewer weighs and labels exactly as the terminal does. Tested by: RD-6.

### Outcome KPIs

- **Who**: every reader (CLI and viewer) | **Does what**: sees the weight change when a non-weight kind or unattested assertion is added | **By how much**: 0 (guardrail KPI-OOS-2, consistent with KPI-GDL-2) | **Measured by**: CORE-OOS-5 property, RD-4, RD-6 | **Baseline**: counters add weight today (A-13).

### Size note

This story has 10 scenarios, more than the 7 the DoR allows. If DELIVER finds it over 3 days, split
it without renumbering: labels (RD-1, RD-5, RD-7, RD-8, RD-9, RD-10) first, then weighting (RD-2,
RD-3, RD-4, RD-6).

### Dependencies

- S-OOS-5 (stored checks to read). Indexer unchanged (C-OOS-8).

---

## S-OOS-8: The review app's share draft quotes only claims whose authority holds

- **job_id**: J-009 (sub-job J-009f, "show my approved profile and share it on Bluesky, opt-in only")
- **Persona**: P-003 Priya Raman (review app user, `github.com/priyaraman`)
- **Slice**: 4 (reduced to the share-draft rule) | **Estimate**: 1 day
- **From**: UC-21, ADR-086 §8, DWD-OOS-13

### Elevator Pitch

- **Before**: Priya published "tidepool is test-driven" and "kelp pins its dependencies", then transferred kelp to `tidal-labs`. The share preview would still boast about kelp.
- **After**: Priya opens the share preview in the review app and sees a draft that names test-driven and not dependency-pinning. The page lists kelp's claim as "not counted: authority not proven", and nothing has been posted.
- **Decision enabled**: Priya decides whether to post a share that speaks only for repos she still owns.

### Problem

The share post is a public statement in Priya's name. After a transfer it must not quote a claim
about a repo she no longer owns. The review app keeps no CLI check store, so "attested" here means
the owner tier holds now, read fresh when the share preview is built (proposal U-OOS-4, awaiting a
DESIGN ruling).

### Who

- Bluesky developer using the review app | shares her approved profile.

### Domain Examples

#### 1: Transfer after publish

Kelp went to `tidal-labs`. The draft quotes test-driven only, and kelp's claim reads "not
counted: authority not proven" (SA-1).

#### 2: Still owns both

Without the transfer, both claims are quoted, as today.

#### 3: Publish refused already (shipped)

Publishing a new claim about a repo Priya does not own is already refused at preview and confirm.
That shipped with review-app-repos-and-accounts (ADR-089, `review_app_repos_and_accounts_several.rs`,
`review_app_publish_permit_alone.rs`) and is not re-specified here.

### Acceptance Criteria

- [ ] AC-8.1 The share draft names only claims about repos whose owner tier holds when the preview is built. Tested by: SA-1.
- [ ] AC-8.2 Left-out claims are listed as "not counted: authority not proven", and nothing is posted without Priya's confirm. Tested by: SA-1.
- [ ] AC-8.3 (Already shipped, ADR-089) Publish-time owner check in the review app. Tested by: the review-app-repos-and-accounts publish suites listed above.

### Outcome KPIs

- **Who**: review-app users who share | **Does what**: post a share quoting a claim about a repo they no longer own | **By how much**: 0 | **Measured by**: SA-1 | **Baseline**: possible today after a transfer.

### Dependencies

- ADR-089 (shipped). U-OOS-4 ruling (DESIGN). The identity-link prompt is GDL slice 3, already shipped and not part of this story.

---

## S-OOS-9: Imported candidates are kept locally, never federated, never weighed

- **job_id**: J-004 (sub-job J-004b, "derive editable candidate claims from harvested signals")
- **Persona**: P-001 (Jeff, importing repos he does not own)
- **Slice**: 5 | **Estimate**: 1.5 days
- **From**: A-11, OOS-D12 option B, C-OOS-7

### Elevator Pitch

- **Before**: `openlore scrape github acme-tools/anvil` prints candidates and forgets them, so "import into the database" is not literally true and Jeff must re-scrape to see them again.
- **After**: with the network off, Jeff runs `openlore candidates list --subject github:acme-tools/anvil` and sees anvil's dependency-pinning candidate, and nothing about kelp. With nothing imported it says there are no imported candidates.
- **Decision enabled**: Jeff decides which imported repos are worth handing to their owners or maintainers to sign.

### Problem

The user said anyone may import claims into the database without owning the account. Today
candidates are not stored. Storing them must not turn them into opinions that federate, feed
inference, or move a weight.

### Who

- Senior engineer | imports repos he admires or depends on | reviews them later, offline.

### Domain Examples

#### 1: Review offline

Jeff imported `acme-tools/anvil` and `rachelk/kelp`. Offline, `candidates list --subject
github:acme-tools/anvil` shows only anvil's candidate, and GitHub is not asked (IC-1).

#### 2: Stays local

Importing anvil adds to his imported candidates. Nothing is signed or published, and his weighted
view of dependency-pinning is unchanged (IC-2).

#### 3: Empty and repeated

With nothing imported, the list says so and the command succeeds (IC-3). Importing anvil on 1
and 3 October lists each candidate once (IC-4, pending U-OOS-6).

### Acceptance Criteria

- [ ] AC-9.1 `openlore candidates list [--subject]` works offline and filters by subject. Tested by: IC-1.
- [ ] AC-9.2 Importing stores candidates locally only. Nothing is signed, published, fed to inference or weighted. Tested by: IC-2.
- [ ] AC-9.3 An empty list says so and exits successfully. Tested by: IC-3.
- [ ] AC-9.4 Re-importing the same repo lists each candidate once. Tested by: IC-4.

### Outcome KPIs

- **Who**: CLI users who import | **Does what**: see their weighted views change because of an import | **By how much**: 0 (C-OOS-7, KPI-OOS-2) | **Measured by**: IC-2 | **Baseline**: n/a (nothing stored today).

### Dependencies

- v8 `imported_candidates` table. Ruling U-OOS-6 (IC-4 behaviour). Independent of slices 3-4.

---

## Job mapping summary

| Story | job_id | Fit |
|---|---|---|
| S-OOS-1 | J-001 | Signing claims about yourself, so your signature means something |
| S-OOS-2 | J-003 (J-003b) | Counter-claims stay first-class |
| S-OOS-3 | J-001 | Local-first signing, publishing deferrable |
| S-OOS-4 | J-004 (J-004c) | Scraper candidates, human signs, now only the right human |
| S-OOS-5 | J-003 (J-003a) | Weigh a peer's authority yourself |
| S-OOS-6 | J-001 | Publishing as a separate, deferrable step |
| S-OOS-7 | J-002 (J-002c) | Transparent weighting |
| S-OOS-8 | J-009 (J-009f) | Opt-in share of the approved profile |
| S-OOS-9 | J-004 (J-004b) | Candidates from harvested signals |

## Proposed job (not added to jobs.yaml)

No existing job states the subject's side: "When someone publishes claims about my GitHub account
or repo, I want only me (or maintainers I list) to be able to sign them, so a signed claim about
my work is my word and everything else is clearly someone's opinion." J-004's anxiety mitigation
("the contributor is the subject of a claim, not a controller of it") is the opposite of what this
feature now does for signing. Proposal: add **J-011 Speak for my own accounts and repos** (or a
sub-job of J-001) and revise the J-004 anxiety text. Until a human decides, the stories above keep
the closest existing jobs.
