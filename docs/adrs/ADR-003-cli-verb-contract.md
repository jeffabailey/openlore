# ADR-003: CLI Verb Contract — Two Prompts, Two Verbs, One Combined Session

- **Status**: Accepted
- **Date**: 2026-05-25
- **Deciders**: Morgan (nw-solution-architect), per CLI verb shape lock from Luna (nw-product-owner)
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

The `claim add` interactive flow MUST present sign and publish as two distinct
user-confirmed beats (per US-001..US-003 and the journey's
confidence-building-with-explicit-trust-buffer arc). Sign-and-persist MUST be
atomic on disk (US-002 AC). Publish MUST be retryable without re-signing
(US-003 AC). The standalone `claim publish <cid>` verb is the building block
for retry and scripting (US-003 Example 2).

The `alternatives-considered.md` Choice 3 closes the high-level question
(separate verbs with chained interactive prompts, NOT `claim add --publish`)
but flags a **residual tension** for DESIGN: may the chained-prompt path and
the standalone `claim publish` path share an internal sign+publish atomic
transaction? Resolved here.

## Decision

**The user-observable contract is fixed: two prompts, two verbs, one combined
session for ergonomics.** Internal code sharing between the chained-prompt path
and the standalone `claim publish` verb IS permitted, subject to the following
invariants:

- The sign step and the publish step are TWO DISTINCT user-visible
  confirmations in the chained path. There is no fused `--publish` flag on
  `claim add`.
- The sign step persists to local storage and fires `sign_success_at` BEFORE
  the publish prompt is rendered. Even if the user dies between prompts (Ctrl-C,
  power loss), the signed claim survives and is republishable via `claim
  publish <cid>`.
- The publish step is idempotent on `cid`; re-running it with an already-
  published CID is a no-op exit-zero (US-003 Example 3).
- The standalone `claim publish <cid>` verb reads from the local store and
  performs the same publish step the chained path performs. There is no
  separate publish code path with different semantics.
- The retract verb (`claim retract <cid>`) composes a NEW counter-claim that
  references the original CID (per ADR-008); it does NOT delete or mutate any
  prior signed record.

### Verb surface for slice-01

| Verb | Purpose | Idempotent? |
|---|---|---|
| `openlore init` | Bootstrap identity + DuckDB schema. | Yes (re-run = "already initialized") |
| `openlore claim add <flags>` | Compose, preview ("not as truth"), sign on Enter, prompt to publish. | Compose+sign are deterministic; CID re-derives identically. |
| `openlore claim publish <cid>` | Publish (or re-publish) an already-signed local claim. | Yes (rkey collision on PDS = no-op) |
| `openlore claim retract <cid>` | Compose+sign+publish a counter-claim referencing `<cid>`. | The counter-claim itself has a new CID; double-retract creates a second counter. |
| `openlore graph query --subject <uri>` | List local claims by subject. Local-only default; footer announces `--federated` lands in slice-03. | Pure read. |

Open question deferred to a later slice's CLI verb shape: the anxiety/habit
scenarios in `gherkin-scenarios-expanded.md` reference `claim status`,
`claim counter`, `graph contrib`, `--from-url`, and `--corrects/--supersedes`
flags that are NOT in scope for slice-01. DISTILL flagged each one; this ADR
does not commit to those verbs.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **`claim add --publish` (fused)** | Collapses two emotional beats into one keystroke; defeats the journey's trust buffer. Rejected in alternatives-considered.md Choice 3. |
| **No standalone `claim publish` verb** (only the chained flow) | Breaks the US-003 retry contract: publish failures must be retryable from CLI without re-signing. |
| **Separate `claim sign` verb in slice-01** (instead of folding sign into `claim add`) | Possible but no user observed today needs sign-without-prompt-to-publish; would split the journey's natural flow. Re-open if a batch scripting use case emerges (slice-02 scrapers may want it; not slice-01). |
| **Internal sign+publish as one DB transaction in the chained path** | Considered; rejected because it couples sign atomicity to publish atomicity. The publish step crosses a network boundary; coupling local-write durability to a network operation forces awkward semantics (rollback on publish fail would un-sign a claim the user already saw a CID for). Two separate atomic steps with shared adapter code is cleaner. |

## Consequences

### Positive

- The journey's emotional arc holds across the implementation.
- Failure recovery is natural: any signed-but-unpublished claim is republishable
  via the standalone verb.
- Scripting (slice-02 scrapers in the future) gets the standalone verb for free.
- Internal code reuse: both paths call the same `publish_signed_claim(cid)`
  function inside the `cli` driver; the only difference is the chained path
  prompts first.

### Negative

- The CLI surface is slightly wider than a fused-verb approach (3 user verbs
  for the slice-01 happy path: `init`, `claim add`, optionally `claim publish`).
  Acceptable for a CLI-comfortable persona.
- DISTILL must verify the two-prompt observable contract against every
  acceptance test, even ones that drive the standalone verb. **Action item for
  DISTILL**: every test driving `claim add` MUST assert the two-prompt pattern
  is visible; tests driving `claim publish <cid>` MUST assert no compose
  preview re-renders.

### Earned Trust (per principle 12)

The CLI driver MUST expose a `probe()` for its own internal invariants:

1. After `claim add` reaches sign-success, the local store contains the signed
   claim regardless of any subsequent step (assert via a fault-injected test
   that kills the process between sign and publish prompts; the signed file
   must still be there on next start).
2. `claim publish <cid>` against an already-published CID exits 0 with the
   existing at-uri (idempotency probe).
3. The compose preview output contains the literal text "not as truth" — a
   string-match probe runnable in CI on every release (this is a content-
   frozen AC per US-001).
