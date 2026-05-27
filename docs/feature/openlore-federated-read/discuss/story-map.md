# Story Map: openlore-federated-read (slice-03)

- **Wave**: DISCUSS
- **Date**: 2026-05-27
- **Owner**: Luna (nw-product-owner)

## User: P-002 Researcher / Tech Lead (federation-reader hat)

Secondary persona: P-001 Senior Engineer Solo Builder (wears the federation-reader hat too).

## Goal

Read another developer's signed claims with full per-claim attribution
preserved, and (when warranted) publish a counter-claim that stands as a
structured public disagreement rather than a reply-guy thread.

## Backbone

| Discover & Subscribe | Pull & Verify | Read Federated Graph | Counter (Optional) | Revoke (Optional) |
|------|------|------|------|------|
| Add peer DID | Pull peer claims | Query --federated | Identify target CID | Soft-remove subscription |
| (Idempotent re-subscribe) | (Reject bad signatures) | (Anti-merging guarantee in output) | Compose counter (--reason) | Hard purge with confirmation |
| | (Reject bad CIDs) | | Sign + publish counter | |
| | | | Observe counter-relationship | |

---

## Walking Skeleton (slice-03 walking skeleton)

The minimum slice that exercises the federation contract end-to-end:

1. **Discover & Subscribe** — `openlore peer add did:plc:other`
2. **Pull & Verify** — `openlore peer pull` ingests their claims into `peer_claims`, recomputing CIDs locally
3. **Read Federated Graph** — `openlore graph query --subject X --federated` returns BOTH my claim and theirs with explicit author attribution

This skeleton validates:

- The federation wire contract (claim Lexicon survives a round-trip across PDSes)
- Canonicalization determinism across implementations (same Rust codebase today, but the test is structurally cross-implementation)
- The anti-merging guarantee (J-003a load-bearing invariant)
- Local store separation (`author_claims` vs `peer_claims`)

It does NOT include counter-claim authoring or peer-remove — those are
deliberately separate releases. The walking skeleton is the thinnest possible
proof that the federation thesis holds.

---

## Release 1 — Walking Skeleton (target outcome: federation contract validated)

| Story | Target outcome | KPI |
|---|---|---|
| US-FED-001 | Subscribe to a peer DID; subscription persists; idempotent | KPI-FED-1 (attribution fidelity, baseline) |
| US-FED-002 | Pull peer claims with signature + CID verification | KPI-FED-1 (attribution fidelity), KPI-FED-2 (zero merge) |
| US-FED-003 | Read federated graph with per-author attribution | KPI-FED-2 (zero merge), KPI-FED-1 |
| US-FED-006 | Bootstrap peer_subscriptions and peer_claims schema (`@infrastructure`) | supports KPI-FED-1, KPI-FED-2 |

**Rationale**: this is the minimum bundle that disproves the federation hypothesis if it fails. Without ANY of these four stories, the walking skeleton is not end-to-end.

**Demo gate (Phase 3.5)**: User runs `openlore peer add did:plc:fixture-peer`, then `openlore peer pull`, then `openlore graph query --subject X --federated`. Output shows their own claim AND the fixture peer's claim, each under their own author DID, no merged row.

---

## Release 2 — Counter-claim authoring (target outcome: J-003b validated)

| Story | Target outcome | KPI |
|---|---|---|
| US-FED-004 | Author and publish a counter-claim referencing a peer's claim | KPI-FED-3 (counter-claim reachability) |

**Rationale**: counter-claim authoring extends the federation contract but does NOT validate it. It validates the J-003b hypothesis that disagreement-as-structured-artifact will change how engineers disagree. Sequenced AFTER the walking skeleton because if Release 1 fails the whole feature dies and Release 2 effort is wasted.

**Demo gate**: Maria sees Rachel's claim about `cargo` in a federated query, runs `openlore claim counter <cid> --reason ...`, sees the compose preview with "not as truth" and "counter-claims coexist, never overwrite", confirms sign and publish, then re-runs the federated query and sees both claims with bidirectional `counters` / `countered-by` annotations.

---

## Release 3 — Subscription revocability (target outcome: J-003c validated)

| Story | Target outcome | KPI |
|---|---|---|
| US-FED-005 | Remove a peer subscription cleanly with optional `--purge` of cached claims | KPI-FED-4 (revocation cleanliness) |

**Rationale**: subscription revocation is the J-003c anxiety mitigation. It can ship LAST because the journey is usable without it for the first few weeks of dogfooding — the worst case is "I have an unwanted subscription I cannot delete yet." That is a survivable defect; the worst case for Release 1 (anti-merging broken) is unsurvivable. Hence the priority order.

**Demo gate**: Maria subscribes to a peer, pulls claims, then runs `openlore peer remove <did> --purge`, confirms the prompt, and a subsequent federated query returns zero claims from that peer.

---

## Priority Rationale

Priority order: **Release 1 (Walking Skeleton) > Release 2 (Counter-claim) > Release 3 (Revoke)**.

The ordering is set by outcome impact and risk-of-failure consequence, NOT by feature volume or implementation order:

1. **Release 1 first** because if it fails, the federation hypothesis is disproven and the whole sibling feature collapses. Validating the wire contract and anti-merging guarantee is the riskiest assumption (per `nw-user-story-mapping` SKILL's "Riskiest Assumption First" rule). All four Release-1 stories are tightly coupled — none of them ship usable value alone, but all four together produce a complete end-to-end demo of J-003 (anti-merging + attribution preservation).

2. **Release 2 second** because counter-claim authoring (J-003b) is the highest-value behavior change after the walking skeleton — it activates the "disagreement as structured artifact" thesis that the umbrella product depends on. It also benefits from being tested on top of a stable walking skeleton; if Release 1 has a latent canonicalization bug, that bug surfaces during Release 1 rather than corrupting counter-claim data in Release 2.

3. **Release 3 third** because subscription revocation (J-003c) is an anxiety mitigation, not a primary outcome. The journey is usable without it for the first few weeks of dogfooding; the worst-case ("an unwanted subscription I cannot delete yet") is survivable. By contrast the worst case for Release 1 (anti-merging broken) is unsurvivable. Dependency-wise, US-FED-005 depends on US-FED-001 (subscriptions exist) and US-FED-002 (peer_claims exist to purge), so it cannot ship before Release 1 regardless of priority.

This ordering preserves the carpaccio principle: each release is independently
demo-able and delivers a verifiable working behavior. Release 1 alone is a
shippable end-to-end slice. Release 2 adds disagreement authoring. Release 3
adds the revocability safety valve.

---

## What is NOT in scope (explicitly deferred)

These were considered and deferred to later sibling features, NOT just later
releases of this feature:

| Out-of-scope | Why deferred | Future home |
|---|---|---|
| Trust weighting / scoring | Subscribe/unsubscribe is binary in slice-03; weighting is a separate algorithmic concern with its own JTBD | `openlore-scoring-graph` (slice-04) |
| Real-time push subscriptions | Pull-on-demand is simpler and validates the contract; push adds protocol complexity unrelated to the J-003 hypothesis | `openlore-realtime-feed` (post-slice-05) |
| Spam claim detection | Requires a corpus of spam claims, which does not yet exist | Post-MVP |
| Notification mechanism (e.g., "you have unread counter-claims") | Polling at `peer pull` is sufficient for slice-03; notifications are a separate UX surface | Slice-04 or post-MVP |
| Multi-peer simultaneous counter-claim review UI | TUI is the slice-03 interface; richer review surfaces deferred | Post-MVP web UI |
| User-configurable evidence-threshold filters in queries | Deferred until weighting (slice-04) gives the filter a meaningful basis | Slice-04 |
