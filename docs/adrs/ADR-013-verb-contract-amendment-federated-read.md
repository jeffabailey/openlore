# ADR-013: CLI Verb Contract Amendment — Peer Verbs, Counter-Claim Sugar Verb, `--federated` Flag

- **Status**: Accepted
- **Date**: 2026-05-27
- **Deciders**: Morgan (nw-solution-architect), per WD-17/WD-22 locks from Luna (nw-product-owner) for openlore-federated-read
- **Feature**: openlore-federated-read (slice-03)
- **Amends**: ADR-003 (CLI Verb Contract). ADR-003 remains in force; this ADR
  EXTENDS its verb surface. The two-prompt invariant, single-publish-path
  invariant, "not as truth" content-frozen invariant, and idempotency rules
  from ADR-003 ALL carry into the new verbs unchanged.

## Context

slice-03 introduces three new behavior surfaces — peer subscription
management, peer claim pull/storage, and structured disagreement via
counter-claims — and an opt-in switch on the existing read surface
(`graph query`). ADR-003 fixed the slice-01 verb surface to `init | claim
add | claim publish | claim retract | graph query`. Slice-03 grows that
surface by 4 verbs and 1 flag.

DISCUSS locked the verb SHAPE choices (WD-17, WD-22, WD-25 + OD-FED-1
accepted at default). What DESIGN owns is:

1. The exact verb grammar consistency (`noun verb` pattern alignment).
2. Argument and flag shapes per verb.
3. The two-prompt contract's interaction with new verbs (which inherit it,
   which do not).
4. Idempotency contracts for each new verb.
5. Exit-code semantics across the new verb set.

## Decision

**The slice-03 verb surface adds four sub-commands and one read flag, all
governed by the same ADR-003 invariants.**

### Verb surface for slice-03 (added to slice-01 surface)

| Verb | Purpose | Network? | Interactive prompts | Idempotent? |
|---|---|---|---|---|
| `openlore peer add <did>` | Subscribe to a peer's claim stream. Validates the peer's DID resolves and that the peer's PDS exposes `org.openlore.claim`. Persists a subscription record. Does NOT pull claims. | Yes (DID resolution + PDS collection probe) | None — non-interactive (a single confirm prompt fires only if the peer has zero published claims yet, per US-FED-001 Example 4) | Yes — re-running with the same DID prints "already subscribed since `<ts>`" and exits 0 |
| `openlore peer pull` | Fetch every subscribed peer's `org.openlore.claim` records. Per-record signature verify + per-record CID recompute. Verified records land in `peer_claims`; rejected records reported in summary. Pulls ALL subscribed peers (no per-peer filter in slice-03; defer to slice-04). | Yes (one HTTPS connection per peer's PDS) | None | Yes — re-pull skips records already in `peer_claims` by CID; reports "0 new, N already in peer_claims, skipped" |
| `openlore peer remove <did> [--purge]` | Without `--purge`: soft-remove (drop subscription, retain cached claims). With `--purge`: hard-remove (drop subscription AND delete all of that peer's cached claims). | No (local-only) | `--purge` REQUIRES interactive `[y/N]` confirmation prompt; no `--yes` flag (deferred to slice-04 per WD-21) | Yes — removing a not-subscribed peer prints "Not subscribed; nothing to remove" and exits 0 |
| `openlore claim counter <target_cid> --reason "..." [other claim flags]` | Sugar verb: constructs a claim with `references[].type == Counters` pointing at `<target_cid>` and threads it through the slice-01 sign+publish pipeline (`VerbClaimPublish` internals). | Sign offline; publish step is network (same as `claim add`). | Yes — inherits ADR-003's two-prompt contract: compose preview (now with "counters: `<target_cid>` (by `<peer_did>`)" + "counter-claims coexist, never overwrite" lines) -> Enter to sign -> Y to publish | Sign is deterministic; publish is rkey-idempotent (same as ADR-003). Re-running a `counter` against an already-countered target prompts for confirmation (US-FED-004 Example 4). |
| `openlore graph query <flags> --federated` | Flag extension on the existing `graph query` verb. Without `--federated`, behavior is byte-identical to slice-01 (author claims only). With `--federated`, query includes peer claims grouped by author DID with bidirectional `counters`/`countered-by` annotation when counter-relationships exist. | No (local read; pull is a separate verb) | None | Pure read; idempotent. |

### Verb grammar consistency

- `peer <verb>` group (new in slice-03) follows the `<noun> <verb>` pattern
  already established by `claim <verb>` in slice-01. `init` and `graph query`
  remain the exceptions (intransitive and `graph` is the noun for the read
  surface, respectively).
- `claim counter` slots into the existing `claim <verb>` group alongside
  `claim add`, `claim publish`, `claim retract`. Symmetric with `claim
  retract <cid>`: both target an existing CID; both construct a new claim
  with `references[]`; both publish via the same pipeline.
- `--federated` is a flag, NOT a separate verb, because it modifies the
  scope of the existing read surface without changing its observable
  contract: a `graph query` result is still "a list of claims, attributed".

### Inheritance from ADR-003

The following ADR-003 invariants apply to slice-03 verbs without restatement:

1. **Two-prompt contract**: `claim counter` uses the same compose-preview-then-publish-prompt sequence as `claim add` and `claim retract`. The compose preview MUST contain the literal "not as truth" string (content-frozen; inherits I-7 from the SSOT cross-feature invariants).
2. **Single publish path**: `claim counter` MUST call into `VerbClaimPublish` internals; no parallel publish code path is permitted (preserves WD-22 lock).
3. **Idempotency on `cid`**: re-publishing an already-published counter-claim's CID is a no-op exit-zero, same as any other claim.
4. **Sign-success-before-publish-prompt**: kill the process between the two prompts and the signed claim file MUST survive on disk (US-FED-004 inherits the slice-01 fault-tolerance probe).
5. **`--no-tty` honors the framing literals**: scripting mode pre-confirms both prompts but still renders the "not as truth" + "counter-claims coexist, never overwrite" lines to stdout.

### Exit-code semantics (slice-03 verbs)

| Verb | Exit 0 | Exit 1 | Exit 2 |
|---|---|---|---|
| `peer add` | Subscription added OR already subscribed | Resolution failure, self-DID rejection, validation error | Probe gauntlet refused at startup (any adapter) |
| `peer pull` | All peers pulled with zero rejections | Any peer's pull skipped OR any record rejected (signature/CID/schema) | Probe gauntlet refused |
| `peer remove` | Subscription removed (soft or hard), OR was not subscribed | Disk error during purge transaction; user declined `--purge` is exit 0 (not an error) | Probe gauntlet refused |
| `claim counter` | Sign + publish succeeded | Pre-compose validation (missing `--reason`, self-counter, unknown target_cid); PDS unreachable at publish (local file preserved); already-countered without confirmation | Probe gauntlet refused |
| `graph query --federated` | Query rendered (may show zero rows) | Storage read error | Probe gauntlet refused |

### Output line conventions (slice-03)

- All new verbs MUST emit the unsubscribe / next-step hint pattern established by `peer add` ("Tip: ..." + copy-pasteable command).
- `peer pull` MUST emit a per-peer block AND a final summary line "Pulled N new peer claims in Xs. None merged with your own claims." (the no-merge clause is content-frozen).
- `claim counter` publish-success message MUST mention `claim retract` as the way to undo (mirrors US-003 / I-8 for the slice-01 publish path).
- `graph query --federated` MUST emit a footer naming distinct-author-count AND the no-merge guarantee (content-frozen — see ADR-014 invariant I-FED-1).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **`claim add --counters <cid>` flag instead of `claim counter` sub-verb** | Locked rejected by WD-17. Less discoverable; breaks symmetry with `claim retract` which is itself a sub-verb. The verb-count cost (one new verb) is paid for by discoverability. |
| **Auto-pull on `peer add`** | Locked rejected by WD-18. Conflates two operations with different failure surfaces. Pull is explicit. |
| **`--yes` flag on `peer remove --purge` to skip confirmation** | Locked rejected by WD-21 for slice-03. Deferred to slice-04 if scripting need surfaces. The destructive-confirmation safety valve must hold against accidental scripting at this stage of dogfeed maturity. |
| **`peer audit <did>` verb to inspect post-purge residue** | OD-FED-3 deferred to slice-04. The audit capability can be derived from `peer list --include-purged` + `graph query --include-counters` (both deferred too); slice-03 trusts the purge transaction without an audit verb. |
| **Separate `claim oppose <cid>` / `claim disagree <cid>` verbs** | Locked rejected (alternatives-considered.md Choice 1 Option C). Combative connotations conflict with "structured public stake, not a fight" framing. |
| **`graph query --include-peers` instead of `--federated`** | The word "federated" is the load-bearing framing of slice-03's value proposition and matches the journey emotional arc ("Sovereign-confident — I see who said what"). `--include-peers` is technically accurate but emotionally flatter. |

## Consequences

### Positive

- Verb count grows by exactly 4 (8 user verbs after slice-03), which sits well below the ~12 informal cap noted in alternatives-considered.md.
- Counter-claim becomes discoverable via the `graph query --federated` footer tip line; the verb is one copy-paste away from the moment of disagreement.
- All new verbs inherit ADR-003's invariants; DISTILL writes new acceptance tests on the same observable contract template (two-prompt where applicable; idempotency; offline-where-possible).
- No verb introduces a new prompt pattern beyond what slice-01 already validated.

### Negative

- `claim` is now a four-verb noun (`add | publish | retract | counter`); the `--help` output for `openlore claim` becomes longer. **Mitigation**: clap's auto-help groups sub-verbs cleanly; no manual fix needed.
- `peer` is a new top-level noun; users learning slice-03 must remember both verb groups. **Mitigation**: the `peer add` output's Tip line names the second-step command (`peer pull`) verbatim, anchoring the workflow.
- ADR-003's two-prompt contract is now invoked by THREE verbs (`claim add`, `claim retract`, `claim counter`). DISTILL must assert the contract on each. **Mitigation**: the contract is observable identically across all three; one acceptance test template, three instantiations.

### Earned Trust

The CLI driver MUST extend its existing `probe()` set to cover the new
verbs without weakening the slice-01 contracts:

1. After `claim counter` reaches sign-success, the local store contains the counter-claim file regardless of any subsequent step (same as slice-01 sign-survives-kill probe; applies to all three claim-authoring verbs).
2. `claim counter` against an already-published counter-claim CID exits 0 with the existing at-uri (idempotency probe inherited from ADR-003).
3. `claim counter` compose preview output contains the literal text "not as truth" AND the literal text "counter-claims coexist, never overwrite" — two string-match probes runnable in CI on every release.
4. `peer pull` against zero subscribed peers exits 0 with a "no peers subscribed" line and writes nothing.
5. `peer remove <did>` (soft) against a peer with N cached claims leaves `peer_claims` row count unchanged (only `peer_subscriptions` shrinks).
6. `peer remove <did> --purge` against a peer with N cached claims AND M user-authored counter-claims targeting that peer deletes the N peer_claims rows AND leaves the M counter-claims in `author_claims` (WD-25 invariant probe).

## Revisit Trigger

- A future slice's discoverability research shows the `claim counter` verb is rarely used despite the federated-query tip line (KPI-FED-3 < 10%) — reconsider promotion-by-default vs flag-gated render.
- A scripting need (slice-04 batch operations) requires `--yes` on `peer remove --purge`; ADR amendment.
- An additional reference-type sugar verb (`claim corrects | claim supersedes`) materializes — would be a third-amendment ADR following this same template.
