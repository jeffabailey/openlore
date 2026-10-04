# Journey: Review and publish my claims (visual)

- **Persona:** Priya Raman, Rust developer. Bluesky `@priyaraman.bsky.social`
  (`did:plc:7x3kq2mzv5rj4w6hbn2tqclp`, PDS hosted by bsky.social). GitHub `priyaraman`
  (owns `priyaraman/tidepool`, a Rust crate with `Cargo.lock` committed, CI test matrix and
  semver tags, plus `priyaraman/quill-docs`, a docs-heavy site).
- **Goal:** publish an accurate, consented, self-attested picture of how she builds into
  her own PDS, then (optionally) share it.
- **Arc:** *Curious but wary* → *In control, reassured* → *Recognised, proud*
  (confidence-building pattern).
- **Platform:** hosted web app (browser). `${app_origin}` is the public HTTPS origin
  (DESIGN, OD-BRA-2). Page paths are illustrative.

## Flow

```text
 [1 Arrive]──▶[2 Sign in w/ Bluesky]──▶[3 Authorize at my PDS]──▶[4 Prove GitHub]──▶[5 Scan]
  curious,       hopeful                  cautious → reassured      focused            waiting,
  wary           "just my handle"         "it says what it can do"  "copy, paste, ok"  informed
                                                                                          │
   ┌──────────────────────────────────────────────────────────────────────────────────────┘
   ▼
 [6 Private review queue] ──approve──▶ [7 Preview exact record] ──confirm──▶ [8 Published in MY PDS]
  in control, judging      ──edit────▶  (confidence / philosophy)            recognised, relieved
  "only I can see this"    ──decline─▶ [6b Declined privately — never public, not re-suggested]
                                                                                          │
   ┌──────────────────────────────────────────────────────────────────────────────────────┘
   ▼
 [9 My profile (approved claims only)] ──(opt-in)──▶ [10 Preview share post] ──confirm──▶ [11 Posted]
  proud                                              deliberate                           proud, done
                                         decline ──▶ nothing happens (no side effects)
```

Peak tension is at **step 3** (granting a third party write access) and **step 7** (the
first public write). Both get explicit "here is exactly what will happen" copy and a
reversible path: cancel at consent, or Back and Retract later.

## Step mockups

### Step 1 — Arrive (`${app_origin}/`)

```text
+-- OpenLore · How you build --------------------------------------------------+
|  See what your public GitHub work says about how you build.                   |
|  You choose what gets published. Nothing is shared until you approve it.      |
|                                                                               |
|  What this app will NEVER do:                                                 |
|   - publish a suggestion you haven't approved                                 |
|   - show anyone your pending or declined suggestions                          |
|   - post to Bluesky unless you press "Post"                                   |
|                                                                               |
|  Bluesky handle  [ priyaraman.bsky.social            ]  [ Sign in with Bluesky ]|
+-------------------------------------------------------------------------------+
```

Feels: curious, wary → a little reassured by the "never" list.

### Step 2/3 — Authorize at her own PDS (outside the app)

```text
+-- bsky.social · Authorize application ---------------------------------------+
|  "OpenLore review" (${app_origin}) wants to:                                  |
|   - know your identity  (@priyaraman.bsky.social)                             |
|   - create OpenLore claim records in your repository                          |
|   - create posts — only when you press Post                                   |
|                                       [ Cancel ]   [ Authorize ]              |
+-------------------------------------------------------------------------------+
```

Shown by her PDS, not by us. The listed permissions depend on OD-BRA-7 (scopes). Cancel
returns to Step 1 with "No access granted. Nothing changed."

### Step 4 — Prove your GitHub (`${app_origin}/github`)

```text
+-- Signed in as @priyaraman.bsky.social ------------------------ [Sign out] --+
|  Step 1 of 3 · Prove the GitHub account is yours                              |
|                                                                               |
|  GitHub username  [ priyaraman ]                                              |
|                                                                               |
|  Add this exact text anywhere in your GitHub profile bio:                     |
|    ┌──────────────────────────────────────────┐                              |
|    │ did:plc:7x3kq2mzv5rj4w6hbn2tqclp         │ [ Copy ]                      |
|    └──────────────────────────────────────────┘                              |
|  (github.com → Settings → Public profile → Bio. 32 of 160 characters.)        |
|  Why: so nobody can claim repos that aren't theirs. You can remove it later.  |
|                                                              [ Verify ]       |
+-------------------------------------------------------------------------------+
  ✗ "We couldn't find did:plc:7x3k…clp in github.com/priyaraman's bio.
     Bios can take a minute to update — add it, then press Verify again."
  ✗ "github.com/priyaraman's bio contains a different DID (did:plc:ab12…).
     It must match the account you're signed in with."
  ✗ "GitHub is rate-limiting us. Try again in 4 minutes — nothing was lost."
  ✓ "Verified: github.com/priyaraman belongs to @priyaraman.bsky.social."
```

Feels: focused. A small, concrete task with a copy button. Errors say exactly what to do.

### Step 5 — Scan (`${app_origin}/scan`)

```text
+-- Step 2 of 3 · Reading your public GitHub work ----------------------------+
|  Public data only. Nothing is published.                                      |
|  ✓ Your DID is still in github.com/priyaraman's bio (checked just now)        |
|  ✓ priyaraman/tidepool       4 signals                                        |
|  ✓ priyaraman/quill-docs     1 signal                                         |
|  … priyaraman/dotfiles       skipped (fork)                                   |
|  Found 5 suggestions.                         [ Review suggestions → ]        |
+-------------------------------------------------------------------------------+
```

Ownership is re-checked before every scan (D-12). If the check fails:

```text
|  ✗ Your DID is no longer in github.com/priyaraman's bio, so we didn't scan.  |
|    Your published claims are untouched. Pending suggestions are hidden until  |
|    you re-verify.   Add did:plc:7x3kq2mzv5rj4w6hbn2tqclp [Copy] → [ Verify ]  |
```

### Step 6 — Private review queue (`${app_origin}/review`)

```text
+-- Step 3 of 3 · Your suggestions (private — only you can see these) ------+
|  5 pending · 0 approved · 0 declined                                          |
|                                                                               |
|  ┌ priyaraman/tidepool embodies  dependency-pinning ─────────────────────┐  |
|  │ Confidence 0.25 (speculative)                                           │  |
|  │ Why: Cargo.lock is committed                                            │  |
|  │ Evidence: github.com/priyaraman/tidepool/blob/main/Cargo.lock           │  |
|  │ [ Approve… ]  [ Edit ]  [ Not me ]                                     │  |
|  └─────────────────────────────────────────────────────────────────────────┘  |
|  ┌ priyaraman/tidepool embodies  test-driven ───────────── 0.25 ───────────┐  |
|  ...                                                                          |
|  Keyboard: A approve · E edit · N not me · J/K next/prev                      |
+-------------------------------------------------------------------------------+
```

Feels: in control. "Private — only you can see these" is always visible on this page
(I-BRA-1).

### Step 6 (edit) — Edit before approving

```text
|  ┌ priyaraman/tidepool embodies [ memory-safety        ▼ ] ───────────────┐  |
|  │ Confidence  [ 0.70 ]  → shown as "well-evidenced"                       │  |
|  │ Evidence (kept): …/Cargo.lock   [+ add link]                            │  |
|  │ [ Cancel ]                                   [ Approve… ]               │  |
```

### Step 6b — Not me (decline)

```text
|  Declined "quill-docs embodies documentation-first". Private — never        |
|  published, won't be suggested again.                       [ Undo ]          |
```

### Step 7 — Preview the exact record (`${app_origin}/review/{suggestion}/approve`)

```text
+-- This will be published to YOUR repository -------------------------------+
|  Where:  at://did:plc:7x3kq2mzv5rj4w6hbn2tqclp/org.openlore.claim/…         |
|  Host:   your PDS (bsky.social)                                               |
|                                                                               |
|   subject     github:priyaraman/tidepool                                      |
|   predicate   embodiesPhilosophy                                              |
|   object      org.openlore.philosophy.memory-safety                           |
|   confidence  0.70  (stored as 7000)                                          |
|   evidence    https://github.com/priyaraman/tidepool/blob/main/Cargo.lock     |
|   provenance  self-attested · signed by your repository                       |
|                                                                               |
|  Published as your reasoning, not as truth. Public once published.          |
|  You can retract it later.                                                    |
|                                         [ Back ]   [ Publish to my repo ]     |
+-------------------------------------------------------------------------------+
```

Feels: deliberate. This is the second peak; it shows exactly what will be written.

### Step 8 — Published

```text
|  ✓ Published to your repository.                                            |
|    at://did:plc:7x3kq2mzv5rj4w6hbn2tqclp/org.openlore.claim/3l2x…            |
|    Provenance: self-attested (repo-signed)                                    |
|    Changed your mind? [ Retract ] (adds a public retraction; never deletes)   |
|    4 pending · 1 approved · 0 declined          [ Next suggestion → ]         |
```

### Step 9 — My profile (`${app_origin}/@priyaraman.bsky.social`)

```text
+-- Priya Raman · @priyaraman.bsky.social ------------------------------------+
|  How I build — self-attested                                                  |
|   memory-safety        0.70 well-evidenced   tidepool   [self-attested]       |
|   test-driven          0.40 weighted         tidepool   [self-attested]       |
|  Only claims Priya approved and published appear here.                        |
|                                                [ Share on Bluesky… ]          |
+-------------------------------------------------------------------------------+
```

### Step 10/11 — Preview the share post (opt-in)

```text
+-- Preview your post --------------------------------------------------------+
|  [ How I build, self-attested on OpenLore: memory-safety, test-driven.     ]|
|  [ ${profile_url}                                                          ]|
|  This posts publicly as @priyaraman.bsky.social. Edit the text above.        |
|                                   [ Don't post ]   [ Post to Bluesky ]       |
+-------------------------------------------------------------------------------+
  ✓ Posted. View on Bluesky →      |   Don't post → back to profile, nothing posted.
```

## Emotional annotations summary

| Step | Entry | Exit | Design lever |
|------|-------|------|--------------|
| 1 Arrive | curious, wary | reassured | "Never" list before any input |
| 3 Authorize | cautious (peak 1) | reassured | PDS consent names capabilities; cancel = no change |
| 4 Prove GitHub | slightly burdened | accomplished | Copy button, exact DID, specific errors |
| 5 Scan | waiting | informed | Per-repo progress, "public data only" |
| 6 Review | judging | in control | Private banner, evidence, one decision per card |
| 7 Preview | deliberate (peak 2) | confident | Exact record and location, "claim not truth", retract path |
| 8 Published | relieved | recognised | Record URI, self-attested label |
| 9–11 Profile and share | proud | proud, done | Approved-only profile, previewed opt-in post |

## Error paths (see YAML `failure_modes` per step)

Handle doesn't resolve · OAuth cancelled or denied · PDS unreachable · bio missing DID or
holding a different DID · GitHub rate-limit · user has no public repos or no signals (empty
queue with guidance) · PDS write fails (suggestion stays pending, nothing half-written) ·
session expired mid-review (pending state preserved) · share post fails (profile unaffected).
