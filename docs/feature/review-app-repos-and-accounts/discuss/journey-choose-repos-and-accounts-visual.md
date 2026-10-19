# Journey: Choose the repos and accounts a scan reads

Feature: `review-app-repos-and-accounts`. Extends steps 3-6 of
`docs/feature/bluesky-claim-review-app/discuss/journey-review-and-publish-my-claims-visual.md`.
Structured form, Gherkin and shared artifacts: `journey-choose-repos-and-accounts.yaml`.

Persona: Priya Raman (`@priyaraman.bsky.social`, `did:plc:7x3kq2mzv5rj4w6hbn2tqclp`). GitHub
accounts: `priyaraman` (personal, 7 owned public repos, 1 fork, 1 archived) and `priya-tidal`
(work, owns `priya-tidal/ingest-bench` and `priya-tidal/tidal-sdk-notes`). She also works on
`tidal-labs/streamline` (an org repo she administers but does not own) and admires
`rust-lang/cargo`.

## Flow

```
 signed in, verified (live today)
          |
          v
 [1 Choose repos] ----> [2 Add another account] ----> [3 Add a repo you don't own]
  top 5 pre-ticked        DID in bio, Verify            owner/name, public only
  Save choice             list + Remove                 "can't be published"
          \                     |                              /
           \                    v                             /
            +----------> [4 Scan again] <--------------------+
                          re-check each account's bio
                          read: chosen repos of passing accounts + imported repos
                                |
                                v
                     [5 Review the private queue]
                      owned card: "from github.com/<login>"  [Approve][Edit][Not me]
                      imported card: "Not yours to publish"  [Decline]
                                |
                                v
                     [6 Publish] -- owned now? (fresh GitHub read) --no--> "Nothing was published."
                                |yes
                                v
                       record in Priya's own PDS (unchanged)

  [7 Administered repo] (separable; how admin is proven is open for DESIGN)
```

## Emotional arc

| Step | Entry | Exit | Design lever |
|------|-------|------|--------------|
| 1 Choose repos | Boxed in ("I can't pick my repos") | In control | Today's top 5 already ticked, so doing nothing keeps today's behaviour |
| 2 Add account | Hopeful | Accomplished | Same DID-in-bio step she already knows; the first account never drops |
| 3 Add a repo | Curious | Curious, safe | The "can't be published" rule is stated before she adds anything |
| 4 Scan | Waiting | Informed | Summary names each account's check and how many repos were read |
| 5 Review | Judging | In control, safe | Every card says where it came from; imported cards have no publish control |
| 6 Publish | Deliberate | Confident | A refusal says why and that nothing was published |

Arc: frustrated -> hopeful -> relieved (problem relief). No step moves from a positive state to
an error without a plain explanation and a next step.

## Screens

### 1. Choose the repos a scan reads (first visit)

```
+-- Choose the repos a scan reads ---------------------------------------+
| github.com/priyaraman (verified)                                        |
|  [x] priyaraman/tidepool       ★412   last push 2026-10-11              |
|  [x] priyaraman/quill-docs     ★88    last push 2026-09-30              |
|  [x] priyaraman/kelp           ★51    last push 2026-08-02              |
|  [x] priyaraman/harbor-cli     ★37    last push 2026-10-02              |
|  [x] priyaraman/driftwood      ★12    last push 2025-03-14              |
|  [ ] priyaraman/ferrous-notes  ★3     last push 2026-10-17              |
|  [ ] priyaraman/sandbar        ★0     last push 2026-06-01              |
|  Skipped: 1 fork, 1 archived repo.                                      |
| 5 of 10 repos chosen.                               [ Save choice ]     |
+-------------------------------------------------------------------------+
```

After she unticks `driftwood`, ticks `ferrous-notes` and saves: "Saved. Your next scan reads 5
repos."

### 2. Your GitHub accounts

```
+-- Your GitHub accounts ------------------------------------------------+
| github.com/priyaraman   verified 2026-10-04                  [ Remove ] |
| github.com/priya-tidal  verified just now                    [ Remove ] |
| Add another GitHub account                                              |
|  Your DID: did:plc:7x3kq2mzv5rj4w6hbn2tqclp  [ Copy ]                   |
|  GitHub username [               ]                           [ Verify ] |
+-------------------------------------------------------------------------+
```

### 3. Repos you don't own

```
+-- Repos you don't own -------------------------------------------------+
| Suggestions for these stay private and can't be published: only an    |
| owner of the repo can publish claims about it.                          |
|  rust-lang/cargo          added 2026-10-19              [ Remove ]      |
|  Add a public repo [ owner/name        ]                [ Add repo ]    |
+-------------------------------------------------------------------------+
```

Refusals (each names the field and the fix):

- `cargo`: "Enter the repo as owner/name, for example rust-lang/cargo."
- `tidal-labs/secret-roadmap`: "We couldn't find a public repo at
  github.com/tidal-labs/secret-roadmap. Check the spelling. Private repos can't be read."
- `priyaraman/sandbar`: "github.com/priyaraman/sandbar is yours. Tick it in your repo list
  instead."

### 4. Scan summary

```
| Ownership re-checked: priyaraman ✓ · priya-tidal ✓                        |
| 8 repos read · 7 new suggestions · 1 already published                    |
```

One account failing:

```
| Ownership re-checked: priyaraman ✓ · priya-tidal ✗                        |
| Your DID is not in the bio of github.com/priya-tidal, so its repos were   |
| not read and its suggestions are hidden. Put your DID back, then verify.  |
```

### 5. Queue cards

```
| priya-tidal/ingest-bench embodies test-driven   0.25 (speculative)       |
| from github.com/priya-tidal                                              |
| [ ] Select   [ Approve ] [ Edit ] [ Not me ]                             |

| rust-lang/cargo embodies dependency-pinning     0.25 (speculative)       |
| Not yours to publish. Only an owner of github.com/rust-lang can publish  |
| claims about this repo. This suggestion stays private.                   |
| [ Decline ]                                                              |
```

### 6. Publish refused

```
+-- Not published --------------------------------------------------------+
| github.com/priyaraman/kelp is no longer owned by an account you          |
| verified, so nothing was published. If you own it under another          |
| account, verify that account; otherwise decline this suggestion.         |
|                       [ Back to your suggestions ]                       |
+-------------------------------------------------------------------------+
```

Every refusal names the cause, says nothing was published, and gives the next step.

## Vocabulary added

| Term | Meaning | Never say |
|------|---------|-----------|
| choose repos / repo choice | The owned repos a scan reads, saved per account | "settings", "filter" |
| repo you don't own / imported repo | A public repo added by owner/name; its suggestions are unsigned and private | "claim", "your repo" |
| not yours to publish | The card state of an imported suggestion | "unverified", "rejected" |
| decline (imported card) | The same private decline as "Not me" (BR-1), same N key. Labelled "Decline" because "Not me" reads wrong on someone else's repo. | "report", "counter" |
| account (GitHub account) | One verified GitHub login on this Bluesky sign-in | "identity", "profile" |

"Self-attested" stays reserved for published claims. "Unsigned" is a requirements term only; the
UI says "not yours to publish" and "stays private".
