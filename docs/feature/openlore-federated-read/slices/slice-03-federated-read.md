# Slice 03 — Federated Read (this feature's only slice; walking skeleton)

- **Status**: in DISCUSS (just emitted; awaiting handoff approval to DESIGN)
- **Feature**: openlore-federated-read
- **Slice priority**: P1 (immediately after slice-01 walking skeleton)
- **Effort estimate**: ~10 days at moderate confidence (6 stories: 5 user-visible + 1 infrastructure)
- **Primary persona**: P-002 (researcher-tech-lead, federation-reader hat) + P-001 (author-engineer, reader hat)
- **Primary job**: J-003 (sub-jobs J-003a anti-merging, J-003b counter-claim, J-003c revocability)

## Hypothesis

> A user can subscribe to another developer's DID, pull their `org.openlore.claim`
> records into a separate `peer_claims` store, and read them through
> `openlore graph query --federated` with full per-claim attribution preserved.
> Disagreement is one verb away (`openlore claim counter <peer_cid> --reason ...`)
> and unsubscribe is one verb away (`openlore peer remove <did> [--purge]`).
> Throughout, no claim is ever merged with another author's claims; no peer
> claim ever modifies the user's own claims; nothing is hard-deleted.

## Walking skeleton (minimum slice that proves the federation thesis)

User runs:

```
openlore peer add did:plc:fixture-peer
openlore peer pull
openlore graph query --subject github:rust-lang/cargo --federated
```

Output of step 3 shows BOTH the user's own claim and the fixture peer's
claim, each under their own author DID header, with no merged row.

Stories that make up the walking skeleton: US-FED-001, US-FED-002,
US-FED-003, US-FED-006 (the infrastructure substrate). That's Release 1 of
the story map.

## Disproves if it fails

- Per-claim CID determinism does not hold across implementations (locally-recomputed CID disagrees with peer-published rkey). The OpenLore federation thesis is broken.
- Users find per-author attribution-preserving reads cognitively confusing and would prefer "consensus" merging. This forces a re-design of the trust model.
- The local DuckDB layout (single file, two new tables) cannot support federated queries at usable speed even at slice-03 scale. Storage layout becomes a fresh question.
- KPI-FED-3 < 10%: counter-claim authoring is too friction-heavy or the J-003b behavior change is not happening even with the verb available.

## In scope

- `openlore peer add <did>` — subscribe to a DID's claim stream (US-FED-001)
- `openlore peer pull` — fetch peer claims into peer_claims (US-FED-002)
- `openlore graph query --federated` — federated reads with per-author attribution (US-FED-003)
- `openlore claim counter <target_cid> --reason "..." ...` — counter-claim authoring (US-FED-004)
- `openlore peer remove <did> [--purge]` — soft and hard unsubscribe (US-FED-005)
- Infrastructure: peer_subscriptions + peer_claims schema, Lexicon `reason` field, port surface for peer ops (US-FED-006)

## Out of scope (deferred to later sibling features)

| Item | Why deferred | Future home |
|---|---|---|
| Trust weighting / scoring | Binary subscribe/unsubscribe is enough for slice-03 | openlore-scoring-graph (slice-04) |
| Push subscriptions / firehose | Pull-on-demand suffices for the hypothesis; daemon violates CLI-first | post-MVP |
| Spam detection | Requires corpus that does not yet exist | post-MVP |
| Notifications | Polling via `peer pull` suffices for slice-03 | slice-04 or post-MVP |
| `peer audit <did>` verb | Implied by J-003c but only emerges as needed | possibly slice-04 (see anxiety scenario 3 DISTILL comment) |
| `--yes` flag on `peer remove --purge` | Confirmation is the safety valve for slice-03; scripting need not yet justified | slice-04 |

## Why this is P1 (sequencing rationale per WD-13)

The federation contract — what fields must be wire-stable, what counts as
"the same" claim, how counter-claims reference originals — constrains every
later design choice. Building scrapers (slice-02) without first validating
federation risks serialization rework. The slice-03 walking skeleton is
specifically designed to disprove the federation thesis early so any rework
happens before slice-02 invests in scraper data shapes.

## Hand-off

This slice IS the entirety of `openlore-federated-read` (one slice = one
feature for this sibling). On DISCUSS handoff:

- **DESIGN** owns: ADR-013 amendment for new verbs, new `PeerPort` (or extension of `PdsPort` + `StoragePort`), peer storage schema, Lexicon `reason` field, the `--federated` flag wiring on `graph query`. All ADR-001..012 are inherited unchanged.
- **DEVOPS** owns: instrumentation for KPI-FED-1..6, contract tests for peer ATProto read paths, the adversarial peer fixture for KPI-FED-6.
- **DISTILL** owns: acceptance tests including the `# DISTILL: confirm` resolution checkpoints in `gherkin-scenarios-expanded.md`.
