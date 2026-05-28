# ADR-017: CLI Verb Contract Amendment — `scrape github` Sugar Verb + `--sign` Continuation

- **Status**: Accepted
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), per WD-50 lock from Luna (nw-product-owner) for openlore-github-scraper
- **Feature**: openlore-github-scraper (slice-02)
- **Amends**: ADR-003 (CLI Verb Contract) + ADR-013 (slice-03 verb amendment). Both remain in force; this ADR EXTENDS the verb surface. The two-prompt invariant, single-publish-path invariant, "not as truth" content-frozen invariant, retract-hint invariant, and idempotency rules from ADR-003 ALL carry into the new verb unchanged.

## Context

slice-02 introduces one new behavior surface: GitHub-signal harvest ->
auditable candidate-claim proposal -> human review/edit/sign via the EXISTING
slice-01 pipeline. ADR-003 fixed the slice-01 verb surface; ADR-013 added the
slice-03 peer/counter verbs. Slice-02 grows the surface by exactly one sugar
verb (`scrape github`) plus one continuation flag (`--sign`).

DISCUSS locked the verb SHAPE (WD-50; OD-SCR-1 default). What DESIGN owns:

1. The exact verb grammar consistency (`<noun> <verb>` / sugar-verb alignment).
2. The `--sign N[,N...]` flag shape (single + batch).
3. The two-prompt contract's interaction with the new verb (it INHERITS it via
   the reused slice-01 pipeline).
4. Idempotency + exit-code semantics.
5. The human-gate guarantee at the verb layer (scrape-without-`--sign` writes
   nothing).

## Decision

**The slice-02 verb surface adds one sugar verb and one continuation flag, both
governed by the same ADR-003 invariants.**

### Verb surface for slice-02 (added to slice-01 + slice-03 surface)

| Verb | Purpose | Network? | Interactive prompts | Idempotent? |
|------|---------|----------|---------------------|-------------|
| `openlore scrape github <target>` (no `--sign`) | Harvest a PUBLIC GitHub target's signals and render an auditable, numbered candidate-claim list. PROPOSES only — writes NOTHING (zero `author_claims` rows, zero PDS calls, zero claim files). | Yes (harvest requires network; refuses offline) | None — non-interactive (prints banner + candidate list) | Yes — re-running re-harvests + re-renders; no side effects to be idempotent about |
| `openlore scrape github <target> --sign N` | Carry candidate N into the slice-01 compose-sign-publish pipeline: pre-fill the editable compose fields, render the compose preview ("not as truth"), the human edits + signs, then the publish prompt. | Harvest is network; sign is offline; publish is network (same as `claim add`) | Yes — INHERITS ADR-003's two-prompt contract (compose preview -> Enter to sign -> Y to publish) | Sign is deterministic; publish is rkey-idempotent (same as ADR-003) |
| `openlore scrape github <target> --sign N,M,...` | Batch continuation: walk each selected candidate through its OWN compose preview + individual signing gesture, with "(k of M signed)" progress. NO "sign all without review" affordance. | Same as single `--sign` | Yes — one two-prompt sequence PER candidate; a single candidate may be skipped without aborting the rest | Each sign deterministic; each publish rkey-idempotent |

### Verb grammar consistency

- `scrape github <target>` follows the sugar-verb pattern established by
  slice-03's `peer pull` / `claim counter` (a `<verb> <object>` shape that
  reads as a natural-language action). `github` is the source qualifier; future
  slices may add `scrape <source>` siblings (Mastodon, blogs — deferred).
- `--sign N[,N...]` is a CONTINUATION flag on `scrape github`, not a separate
  verb, because it modifies what the harvest+propose surface does with a
  selection — it does not change the observable contract of "harvest a public
  target, propose candidates."
- The verb keeps `claim add` focused on hand-authoring (the rejected
  alternative `claim add --from-github` would have polluted it).

### Inheritance from ADR-003 / ADR-013

The following invariants apply to `scrape github --sign` WITHOUT restatement:

1. **Two-prompt contract**: `--sign` uses the same compose-preview-then-publish-
   prompt sequence as `claim add`. The compose preview MUST contain the literal
   "not as truth" (content-frozen; I-7).
2. **Single publish path**: `--sign` MUST call into `VerbClaimAdd` +
   `VerbClaimPublish` internals as functions; NO parallel publish code path
   (preserves WD-22 + WD-66). `cli::CandidatePrefill` is the only bridge from a
   `CandidateClaim` to the slice-01 pipeline.
3. **Idempotency on `cid`**: re-publishing an already-published signed-from-
   scraper claim's CID is a no-op exit-zero.
4. **Sign-success-before-publish-prompt**: kill the process between the two
   prompts and the signed claim file MUST survive on disk.
5. **`--no-tty` honors the framing literals**: scripting mode pre-confirms both
   prompts but still renders "not as truth" to stdout.
6. **Retract hint** (I-8): the publish-success message mentions
   `openlore claim retract <cid>`.

### Human-gate guarantee at the verb layer (slice-02 NEW)

- `scrape github <target>` WITHOUT `--sign` performs ZERO writes: no
  `author_claims` rows, no PDS calls, no `claims/<cid>.json` files. It is a
  read-and-propose verb only. (KPI-SCR-2; `scraper_never_persists_unsigned`.)
- The candidate list is NEVER auto-signed. A candidate becomes a claim ONLY by
  the human passing it through the slice-01 sign pipeline via `--sign`.
- The public-data-only banner is printed BEFORE any harvest begins.

### Exit-code semantics (slice-02 verb)

| Invocation | Exit 0 | Exit 1 | Exit 2 |
|------------|--------|--------|--------|
| `scrape github <target>` (no `--sign`) | Candidates rendered (may be zero — "no candidates derived" is exit 0) | Target not found / not public / offline / rate-limit exhausted (no partial list) | Probe gauntlet refused at startup (any adapter, incl. `adapter-github`) |
| `scrape github <target> --sign N` | Sign + publish succeeded (or publish declined, local sign retained) | Out-of-range index (pre-compose); PDS unreachable at publish (local file preserved); harvest failure | Probe gauntlet refused |
| `scrape github <target> --sign N,M,...` | All selected signed (or individually skipped) | Invalid selection list (duplicate / out-of-range, pre-compose); any harvest failure | Probe gauntlet refused |

### Output line conventions (slice-02)

- The verb MUST emit the public-data-only banner before harvest.
- The candidate list footer MUST state that nothing is a claim until the user
  signs it, AND name the `--sign` continuation as a copy-pasteable tip.
- A signed-from-scraper claim MUST show a display-only `derived-from` line in
  the compose preview + publish-success output (WD-62; NOT a signed-payload
  field).
- The publish-success message MUST mention `claim retract` (I-8).
- Batch mode MUST emit a "(k of M signed)" progress line and a final
  "N signed, M skipped" summary.

## Alternatives Considered

| Option | Rejection rationale |
|--------|---------------------|
| **`claim add --from-github <target>` flag instead of a `scrape github` sub-verb** | Locked rejected by WD-50. Pollutes `claim add` (currently a focused hand-authoring verb) with a harvest concern + an external-system dependency; less discoverable than a dedicated verb; breaks symmetry with the slice-03 sugar verbs. The one-verb cost is paid for by discoverability + separation of concerns. |
| **Auto-sign the top candidate (a `--auto` convenience)** | Hard reject — violates the human-gate (WD-49), the single most load-bearing invariant of the slice. The scraper has NO signing key by construction; auto-sign would collapse the trust model. |
| **A separate `scrape` + `sign` two-verb flow (no `--sign` continuation)** | Considered; rejected because it would force a re-harvest between verbs (the candidate list is in-memory only) OR require persisting candidates (which WD-55 forbids). The `--sign` continuation keeps harvest + sign in one invocation without persisting an unsigned candidate. |
| **`scrape github --sign all`** | Rejected — there is NO "sign all without review" affordance (US-SCR-005 AC). Batch is explicit indices, each individually previewed + signed. |
| **Reusing the slice-01 `claim publish` standalone verb as the scraper's publish step via a new internal path** | Already covered — `--sign` reuses `VerbClaimPublish` internals (the SAME path the standalone verb uses). No new path is created (WD-66). |

## Consequences

### Positive

- Verb count grows by exactly one (sugar verb); sits well below the ~12 informal
  cap.
- The new verb inherits ADR-003's two-prompt + single-publish-path invariants;
  DISTILL writes new acceptance tests on the same observable-contract template.
- `claim add` stays focused on hand-authoring; the scraper is a discoverable,
  separately-documented surface.
- The human-gate is enforced at the verb layer (scrape-without-`--sign` writes
  nothing) AND at the architecture layer (the scraper has no signing key).

### Negative

- `--sign N,M,...` introduces a small new selection-parsing surface (duplicate /
  out-of-range validation). **Mitigation**: validation happens pre-compose with
  a clear error naming the offending indices; covered by a cli probe.
- The compose preview reached from a candidate now carries an extra display-only
  `derived-from` line. **Mitigation**: it is display-only (WD-62), never in the
  signed payload, so CID stability is unaffected.

## Earned Trust

The CLI driver MUST extend its `probe()` set to cover the new verb without
weakening the slice-01/03 contracts:

1. `scrape github <target>` WITHOUT `--sign` writes ZERO `author_claims` rows,
   makes ZERO PDS calls, writes ZERO claim files (the human-gate storage probe;
   KPI-SCR-2 / `scraper_never_persists_unsigned`).
2. `scrape github <target>` prints the public-data banner BEFORE any harvest.
3. The compose preview reached from a candidate contains the literal "not as
   truth" (I-7) — a string-match probe runnable in CI on every release.
4. The publish-success message reached from a sign-from-scraper claim mentions
   `claim retract` (I-8).
5. An out-of-range / duplicate `--sign` index is rejected BEFORE any compose
   begins, naming the offending indices.
6. `--sign N` with no edits produces a signed claim whose fields equal the
   candidate's proposed values byte-for-byte, confidence 0.25 (no auto-inflation;
   `candidate_confidence_no_autoinflate`).
7. Offline `scrape` exits non-zero with a "requires network" message and renders
   no partial list.

## Revisit Trigger

- A future slice adds a second scrape source (`scrape mastodon`, `scrape blog`)
  — would extend this same sugar-verb template.
- A scripting need (slice-04 batch operations) requires a non-interactive
  bulk-sign mode — would be an ADR amendment (must NOT weaken the per-claim
  human-gate; would require a different trust justification).
- KPI-SCR-1 shows the `--sign` continuation is rarely used (users prefer
  harvest-then-hand-author) — reconsider the candidate->compose pre-fill UX.
