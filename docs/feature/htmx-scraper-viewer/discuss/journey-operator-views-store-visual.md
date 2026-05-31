# Journey (Visual): operator-views-store

> **Persona**: the OpenLore **node operator** (e.g. Maria Santos), on **localhost**, on
> their own machine. **Read-only personal dashboard.** Signing stays in the CLI.
> **DELTA** on slices 01/02/03. Lightweight journey: happy path + key error paths.
>
> Two journeys below:
> **A — Inspect my store** (PRIMARY, Job 1, offline-capable).
> **B — Browse scrape proposals** (SECONDARY, Job 2, network-backed).

---

## Emotional arc (Problem Relief + Confidence Building)

```
  Journey A (store):   blind/uneasy  ->  curious  ->  scanning  ->  grounded/in control
  Journey B (scrape):  squinting     ->  hopeful  ->  scanning  ->  calm/decided
                        (start)        (middle)     (middle)        (end)
```

No jarring transitions. Empty states and errors guide rather than block. The dominant
reassurance throughout: **nothing here can write, sign, or expose my key.**

---

## Journey A — Inspect my store (PRIMARY)

```
[Trigger: operator wants to            [Start viewer]        [Open /claims in browser]
 know "what does my node hold?"]   -->  $ openlore viewer  -->  GET /claims
   Feels: blind / uneasy                Feels: curious          Feels: scanning
   Artifact: ${duckdb_store}            Artifact: localhost URL Artifact: ${claim_row}[]
        |
        v
[Read the list of my signed claims]  -->  [Open one claim's detail]  -->  [See peer_claims]
   Sees: subject/predicate/object,        GET /claims/{cid}              GET /peer-claims
         confidence, CID, author          Sees: full evidence[]          Sees: ${peer_claim_row}[]
   Feels: grounded                        Feels: confident               + peer_origin
   Artifact: ${rendered_page}             Artifact: ${claim_row}         Feels: in control
```

### Step A1: Start the viewer

```
+-- $ openlore viewer ------------------------------------------------+
| OpenLore viewer (read-only) listening on http://127.0.0.1:8788      |
| Store: ~/.openlore/store.duckdb  (offline OK)                       |
| Open http://127.0.0.1:8788/claims in your browser.                  |
| Read-only: no signing, no writes. Stop with Ctrl-C.                 |
+---------------------------------------------------------------------+
   Emotional: curious, reassured ("read-only" stated up front)
   ${duckdb_store} bound; no key loaded.
```

### Step A2: View my signed claims  (GET /claims)

```
+================ OpenLore — My Store (read-only) =====================+
|  [ My Claims ]   Peer Claims   |   Live Scrape          127.0.0.1   |
+---------------------------------------------------------------------+
|  My signed claims                                312 total          |
|  -----------------------------------------------------------------  |
|  subject            predicate          object        conf    cid    |
|  rust-lang/rust     is-maintained-by   The Rust...   0.90   bafy..1  |
|  tokio-rs/tokio     has-license        MIT           0.95   bafy..2  |
|  serde-rs/serde     depends-on         syn           0.70   bafy..3  |
|  ...                                                                |
|  [ Showing 1-50 of 312 ]                    [ Next > ]   (page 1)    |
+---------------------------------------------------------------------+
   ${claim_row}[] from ${duckdb_store}.claims
   Emotional: blind -> scanning. "I can finally SEE my store."
   Note: confidence shown as stored numeric (0.90). NO derived-from here
         (not persisted — WD-62).
```

### Step A3: View one claim's detail  (GET /claims/{cid})

```
+================ Claim detail (read-only) ===========================+
|  < back to My Claims                                                |
|  subject     rust-lang/rust                                         |
|  predicate   is-maintained-by                                       |
|  object      The Rust Project                                       |
|  confidence  0.90                                                   |
|  author      did:plc:maria...                                       |
|  composed    2026-04-18T09:12:03Z                                   |
|  cid         bafyrei...1                                            |
|  evidence    - https://github.com/rust-lang/rust (repo)            |
|              - https://www.rust-lang.org/governance (gov page)     |
+---------------------------------------------------------------------+
   ${claim_row} (full, incl evidence[]) from ${duckdb_store}.claims
   Emotional: confident. Every field legible; matches mental model.
```

### Step A4: View federated peer claims  (GET /peer-claims)

```
+================ Peer Claims (federated, read-only) =================+
|  My Claims   [ Peer Claims ]   |   Live Scrape          127.0.0.1   |
+---------------------------------------------------------------------+
|  Federated peer claims                          1,840 total         |
|  -----------------------------------------------------------------  |
|  subject          predicate       object     conf   peer      cid   |
|  axum/axum         has-license     MIT        0.88  peer-A   bafy..7 |
|  hyperium/hyper    depends-on      tokio      0.66  peer-B   bafy..8 |
|  ...                                                                |
|  [ Showing 1-50 of 1840 ]                   [ Next > ]   (page 1)    |
+---------------------------------------------------------------------+
   ${peer_claim_row}[] from ${duckdb_store}.peer_claims (peer_origin shown)
   Emotional: in control. "Mine vs federated" is unambiguous.
```

### Journey A — key error / edge states

```
+-- Empty store (first run, no claims signed yet) --------------------+
| You have not signed any claims yet.                                 |
| Claims you sign with the CLI will appear here.                      |
| (This view is read-only — sign claims from the CLI.)                |
+---------------------------------------------------------------------+
   Empty state guides, does not dead-end. Emotional: oriented, not lost.

+-- Store unreadable (locked / missing file) ------------------------+
| Could not open your store at ~/.openlore/store.duckdb.              |
| Is another OpenLore process using it? Check the path and retry.     |
+---------------------------------------------------------------------+
   Error states what happened + what to do. No raw stack trace.
```

---

## Journey B — Browse scrape proposals (SECONDARY)

```
[Open Live Scrape]      [Enter a target]            [See proposed candidates]
 GET /scrape       -->   submit ${scrape_target} -->  rendered candidate list
 Feels: hopeful          Feels: engaged              Feels: scanning -> decided
 Artifact: form          ${scrape_target}            ${candidate_claim}[] (ephemeral)
```

### Step B1: Live Scrape form  (GET /scrape)

```
+================ Live Scrape (read-only proposals) ==================+
|  My Claims   Peer Claims   |   [ Live Scrape ]          127.0.0.1   |
+---------------------------------------------------------------------+
|  Browse scrape proposals (nothing is signed or saved)               |
|  Target  [ tokio-rs/tokio_________________ ]   [ Propose ]          |
|  This runs a live harvest. Candidates are shown for review only.    |
|  To sign any candidate, use the CLI.                                |
+---------------------------------------------------------------------+
   Emotional: hopeful; reassured ("nothing is signed or saved").
   Requires network (same as CLI scraper). ${scrape_target} = operator input.
```

### Step B2: Proposed candidates  (POST/GET result)

```
+================ Proposals for tokio-rs/tokio (read-only) ===========+
|  7 candidate claims proposed — NONE are signed or saved.            |
|  -----------------------------------------------------------------  |
|  subject        predicate     object   conf   derived-from         |
|  tokio-rs/tokio  has-license   MIT      0.95  LICENSE @ HEAD        |
|  tokio-rs/tokio  written-in    Rust     0.99  Cargo.toml           |
|  tokio-rs/tokio  depends-on    mio      0.80  Cargo.toml [deps]     |
|  ...                                                                |
|  -----------------------------------------------------------------  |
|  To sign any of these: run  `openlore scrape github tokio-rs/tokio  |
|  --sign`  in your terminal.  (No sign button here by design.)       |
+---------------------------------------------------------------------+
   ${candidate_claim}[] live from slice-02 propose step (in-memory, ephemeral).
   ${derived_from} shown DISPLAY-ONLY (WD-62). NO sign control. NO key in process.
   Emotional: scanning -> calm/decided. "I can triage, then sign in the CLI."
```

### Journey B — key error / edge states

```
+-- Target produced no candidates -----------------------------------+
| No candidate claims could be derived from "some-org/empty-repo".    |
| Try a different target, or check the repo has license/manifest data.|
+---------------------------------------------------------------------+

+-- Network unavailable (live harvest needs network) ----------------+
| Could not reach GitHub to harvest "tokio-rs/tokio".                 |
| The Live Scrape view needs a network connection. Your store view    |
| (My Claims / Peer Claims) still works offline.                      |
+---------------------------------------------------------------------+
   Error distinguishes the network-bound Job 2 from offline-capable Job 1.
```

---

## UX heuristics applied (web surface)

- **Visibility of system status** (Nielsen 1): row counts, page indicator, "read-only"
  banner, explicit network-needed note on Live Scrape.
- **Match real world** (Nielsen 2): domain vocabulary only — subject/predicate/object,
  confidence, CID, peer, derived-from. No internal jargon.
- **Error prevention** (Nielsen 5) + **read-only guardrail**: there is simply **no**
  write/sign affordance to misuse; signing is constrained to the CLI.
- **Help with errors** (Nielsen 9): every error states what happened + next step; no raw
  traces; network error clarifies which view still works offline.
- **Empty states** guide (emotional-design): first-run "no claims yet" points to the CLI.
- **Tables for structured comparison** (web data-display): claims/peer-claims/candidates
  are tabular with sortable potential (sort = later release, OD-VIEW-4 pagination).
- **Aesthetic/minimalist** (Nielsen 8): read-only dashboard shows only what supports the
  "what do I hold / what could I add" questions.
