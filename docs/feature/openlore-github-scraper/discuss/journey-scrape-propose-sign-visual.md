# Visual Journey — scrape-propose-sign

- **Feature**: openlore-github-scraper (slice-02)
- **Wave**: DISCUSS
- **Date**: 2026-05-28
- **Owner**: Luna (nw-product-owner)
- **Persona**: P-002 Researcher / Tech Lead (contributor-evaluator hat) — primary; P-001 (Solo Builder) also wears this hat
- **Job**: J-004 (sub-jobs J-004a harvest, J-004b derive candidates, J-004c human-always-signs)
- **Structured schema**: `docs/product/journeys/scrape-propose-sign.yaml`

This document is the human-readable companion to the YAML schema. It captures
the visual flow, the emotional arc, and the per-step TUI mockups in one place
so the reviewer and DESIGN wave can read the journey without context-switching.

## Flow at a glance

```
        Trigger: Maria is evaluating rust-lang/cargo's maintainers
                 through a philosophy lens, and writing each
                 evidence-backed claim by hand is slow
                              |
                              v
+------------+   +------------+   +------------+   +------------+
| Step 1     |-->| Step 2     |-->| Step 3     |-->| Step 4     |
| scrape     |   | review     |   | select +   |   | (each) sign|
| github     |   | candidate  |   | edit one   |   | + publish  |
| <target>   |   | list       |   | candidate  |   | (slice-01) |
+------------+   +------------+   +------------+   +------------+
  Curious-but-    In-control      Authoring        Confident-
  skeptical       (nothing        (this is MY      authorship
                  signed yet)     reasoning)
```

## Emotional arc — skeptical-to-confident-authorship (with a hard human-gate buffer)

The load-bearing emotional moment is **step 2 -> step 3**. Step 1 harvests
PUBLIC GitHub signals and step 2 renders them as *candidate* claims with a
conservative confidence (0.25, "speculative") and the exact signal that
produced each. The whole psychological contract of this feature is: **the tool
proposes, the human disposes.** Nothing is signed, persisted-as-a-claim, or
published until the user explicitly carries a candidate into the slice-01
compose-sign-publish pipeline (step 3->4).

Two anxieties drive this journey and the design answers each at a specific step:

- **"Is this a surveillance tool / will it auto-publish junk about a person?"**
  Answered at step 1 (banner: only public data; contributor is the SUBJECT of
  a claim, not a controller) and at step 2 (NOTHING is signed; every candidate
  is editable; confidence starts speculative).
- **"Will the tool put words in my mouth?"** Answered at step 3->4: the
  candidate flows into the SAME `claim add` compose preview the user already
  knows from slice-01, complete with the literal "not as truth" framing and
  the human signing gesture. The scraper never signs.

Half the sessions this feature must support will end at step 2 (user scans the
candidates, decides none are worth signing, walks away). That is fine and
expected — the harvest still lowered the cost of *deciding there was nothing to
claim*. Step 3->4 is the value-capture, not the only valid exit.

## Step 1 — scrape github <target>

```
$ openlore scrape github rust-lang/cargo

OpenLore GitHub scraper
  Only PUBLIC GitHub data is read. The target is the SUBJECT of any claim
  you may later sign — never a controller of it. Nothing is published here.

Resolving target rust-lang/cargo ... ok  (repository)
  default branch   : master
  auth             : authenticated (GITHUB_TOKEN found; 4982/5000 rate budget)

Harvesting public signals ...
  README.md                  ... 1 signal
  Cargo.lock committed       ... 1 signal
  docs/ directory present    ... 1 signal
  test/source file ratio 0.61... 1 signal
  CHANGELOG.md + semver tags ... 1 signal

Harvested 5 signals in 2.1s. Deriving candidate claims (next step) ...
```

**Why the public-data banner is load-bearing**: it front-loads the J-004
anxiety answer (no surveillance). The user reads, at the moment of invocation,
that the tool only ever touches public data and that nothing is published.

**Feels**: entry Curious-but-skeptical -> exit Curious-and-reassured.

## Step 2 — review candidate list

```
Candidate claims for subject github:rust-lang/cargo
(5 derived — NOTHING is signed or published; you choose what to sign)
=====================================================================

  [1] embodiesPhilosophy  org.openlore.philosophy.dependency-pinning
      from signal : Cargo.lock committed at repo root (exact-version pins)
      evidence    : https://github.com/rust-lang/cargo/blob/master/Cargo.lock
      confidence  : 0.25 (speculative)   <- conservative default; you edit

  [2] embodiesPhilosophy  org.openlore.philosophy.documentation-first
      from signal : docs/ dir + README 412 lines + high doc-comment density
      evidence    : https://github.com/rust-lang/cargo/tree/master/src/doc
      confidence  : 0.25 (speculative)

  [3] embodiesPhilosophy  org.openlore.philosophy.test-driven
      from signal : test/source file ratio 0.61; CI test matrix present
      evidence    : https://github.com/rust-lang/cargo/tree/master/tests
      confidence  : 0.25 (speculative)

  [4] embodiesPhilosophy  org.openlore.philosophy.semantic-versioning
      from signal : CHANGELOG.md + tags follow semver
      evidence    : https://github.com/rust-lang/cargo/blob/master/CHANGELOG.md
      confidence  : 0.25 (speculative)

  [5] embodiesPhilosophy  org.openlore.philosophy.memory-safety
      from signal : primary language Rust; zero unsafe blocks in src/
      evidence    : https://github.com/rust-lang/cargo
      confidence  : 0.25 (speculative)

These are PROPOSALS derived from public signals. None is a claim until YOU
sign it. Select one to review and sign: `openlore scrape github rust-lang/cargo
--sign 1`  (or --sign 1,3,4 for several).  No selection = nothing happens.
```

**Why every candidate names its source signal**: auditability. The user can
see exactly WHY the tool proposed each predicate and reject any derivation they
disagree with. The mapping (signal -> predicate) is small and defensible by
design.

**Why confidence starts at 0.25 (speculative)**: the scraper has weak evidence
— a single public signal. Starting low forces the human to consciously RAISE
the confidence if they believe the claim is well-supported, rather than the
tool over-asserting on the user's behalf.

**Feels**: entry Curious-and-reassured -> exit In-control.

## Step 3 — select + edit one candidate

```
$ openlore scrape github rust-lang/cargo --sign 1

Editing candidate [1] before signing (slice-01 compose pipeline) ...
Press Enter to accept each field, or type a new value.

  subject     [github:rust-lang/cargo] :
  predicate   [embodiesPhilosophy] :
  object      [org.openlore.philosophy.dependency-pinning] :
  evidence    [https://github.com/rust-lang/cargo/blob/master/Cargo.lock] :
  confidence  [0.25] : 0.55
  reason / note (optional) : Cargo.lock is committed deliberately; this is a
    repo policy, not an accident.
```

**Why the candidate becomes editable here**: this is the J-004c moment — the
human takes ownership. The pre-filled values are a strong starting point, but
the user can change the predicate, swap the evidence URL, and (here) raise
confidence from speculative 0.25 to weighted 0.55 because they have personal
knowledge that Cargo.lock is a deliberate policy.

**Feels**: entry In-control -> exit Authoring (this is MY reasoning now).

## Step 4 — sign + publish (slice-01 pipeline, unchanged)

```
Compose preview (claim is asserted by you, not as truth)
  subject:    github:rust-lang/cargo
  predicate:  embodiesPhilosophy
  object:     org.openlore.philosophy.dependency-pinning
  evidence:   https://github.com/rust-lang/cargo/blob/master/Cargo.lock
  confidence: 0.55 (weighted)
  reason:     Cargo.lock is committed deliberately; this is a repo policy ...
  derived-from: openlore-github-scraper (signal: Cargo.lock committed)
  author:     did:plc:maria-test
  composedAt: 2026-05-28T11:04:12Z

Press Enter to sign locally (or Ctrl-C to cancel):
Computing claim CID bafyrei...cargo-pin
Written to local store: ~/.local/share/openlore/claims/bafyrei...cargo-pin.json

Publish to your PDS now? (y/N): y
Published. at-uri: at://did:plc:maria-test/org.openlore.claim/bafyrei...cargo-pin
(retract later with `openlore claim retract bafyrei...cargo-pin`)
```

**Why this step is byte-identical to slice-01**: WD-22-style single-publish-path
reuse. The counter-claim slice (slice-03) reused the slice-01 publish pipeline;
the scraper does the same. There is NO scraper-specific publish code path. The
only addition is an optional `derived-from` provenance line so a reader can see
the claim originated from a scraper run (the field is informational; it does NOT
change the signed payload's confidence or change how the claim federates).

**Feels**: entry Authoring -> exit Confident-authorship (I signed exactly what I
reviewed; the tool never spoke for me).

## Shared artifacts highlighted

| Artifact | First appears | Reused at | Risk |
|---|---|---|---|
| `github_target` | step 1 | steps 2, 3 | HIGH — drift would attribute claims to the wrong subject |
| `harvested_signal` | step 1 | step 2 (each candidate names its signal) | HIGH — a candidate without a traceable signal is unauditable |
| `candidate_claim` | step 2 | step 3 (editable), step 4 (signed) | HIGH — a candidate must round-trip its fields unchanged into compose unless the user edits |
| `signal_predicate_mapping` | step 2 (applied) | jobs.yaml (SSOT) | MEDIUM — mapping drift changes which predicates are proposed |
| `confidence` (default 0.25) | step 2 | step 3 (editable), step 4 (signed) | HIGH — must never auto-inflate; human-only raise |
| `claim_cid` | step 4 (computed at sign) | publish, future query | HIGH — drift = federation thesis broken |

Full registry: `shared-artifacts-registry.md` (this directory).

## Human-gate guarantee (cross-cutting)

This is the single most load-bearing invariant of the whole feature. It is
called out separately because it spans the slice — not a step-local concern.

- **At harvest (step 1)**: only PUBLIC GitHub data is read. The target is the
  SUBJECT of a possible claim, never a controller; no private data, no
  surveillance affordance.
- **At derivation (step 2)**: candidates are PROPOSALS. They are never written
  to `author_claims`, never signed, never published. Confidence starts
  speculative (0.25).
- **At sign (step 3->4)**: a candidate becomes a claim ONLY by passing through
  the SAME slice-01 compose-sign-publish pipeline, with the human's explicit
  signing gesture. The scraper has no signing key and no publish path.
- **At test time (acceptance suite)**: a dedicated test
  `scraper_never_persists_unsigned` asserts that running `scrape github`
  WITHOUT `--sign` produces zero rows in `author_claims` and makes zero PDS
  writes.

## Failure scenarios summary

| Step | Mode | User-visible behavior |
|---|---|---|
| 1 | Target does not exist (404) | No candidates; non-zero exit; "repository/user not found" error names the target |
| 1 | Rate limit exhausted (unauthenticated) | Partial-or-no harvest; message suggests setting GITHUB_TOKEN; exits non-zero |
| 1 | Target private / no access | "target is not public; the scraper only reads public data"; exits non-zero |
| 1 | Network offline | Harvest cannot run; clear "scrape requires network" message; exits non-zero |
| 2 | Zero signals matched the mapping | "No candidate claims could be derived from public signals for <target>." Exit 0 (not an error; just nothing to propose) |
| 3 | `--sign N` where N out of range | "candidate N does not exist; valid range 1..M"; no claim written |
| 3 | User edits confidence out of [0,1] | Re-prompt with the [0.0,1.0] constraint; no claim written until valid |
| 4 | PDS unreachable at publish | Local claim file intact; retry hint with `openlore claim publish <cid>` (slice-01 behavior, unchanged) |
