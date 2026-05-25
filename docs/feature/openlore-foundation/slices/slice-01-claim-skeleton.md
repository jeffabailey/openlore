# Slice 01 — Claim Skeleton (Walking Skeleton)

**Feature**: openlore-foundation
**Slice priority**: P0 (this slice IS the walking skeleton)
**Status**: in DISCUSS
**Effort estimate**: 3-5 days of focused work
**Primary persona**: P-001 Senior Engineer Solo Builder
**Primary job**: J-001 Author a signed philosophical claim

## Hypothesis

> A senior engineer can author a single philosophical claim about a project,
> sign it with their existing ATProto DID, persist it locally to DuckDB,
> publish it to their PDS as an `org.openlore.claim` record, and read it back
> through a local graph query — in under 2 minutes once tooling is installed,
> and feeling at every step that they have framed a claim, not asserted a truth.

## Disproves if it fails

- The ATProto-Lexicon-as-claim-container approach is unworkable for application data.
- Canonical CIDs over our claim records are non-deterministic across machines/re-runs.
- The "claims not truth" UX framing is impossible to land in a CLI flow.
- DuckDB is the wrong local store for this shape of data.

If any of these fail, the entire OpenLore architecture pivots BEFORE we invest in
scrapers, federation, scoring, or AppView.

## In scope

- Define `org.openlore.claim` Lexicon (subject, predicate, object, evidence[], confidence, author, composedAt, signature).
- CLI: `openlore claim add` (with all fields as flags + an interactive `--edit` mode).
- CLI: `openlore graph query --subject <uri>` (local-only, no `--federated`).
- Sign with the user's existing ATProto identity (see RC-03; default: reuse existing DID).
- Persist to DuckDB at `~/.local/share/openlore/openlore.duckdb` (XDG-respecting).
- Publish to the author's PDS.
- Hint at retraction at publication time (the command exists but its full implementation is in scope; retract via counter-claim per RC-02 proposed default).

## Out of scope (explicit deferrals)

- Scrapers of any kind → slice-02.
- Federated read (`--federated` flag) → slice-03.
- Trust weighting / triangulation → slice-04.
- AppView, web UI → slice-05.
- A controlled philosophy lexicon larger than a starter seed (~10 well-known philosophies) → slice-04.
- Multi-DID identity management beyond reusing the one ATProto identity already in the user's environment.

## Slice composition (gate check)

| Story | Type | Job link | Elevator pitch present? |
|---|---|---|---|
| US-001 Author a single signed claim from the CLI | user story | J-001 | yes |
| US-002 Sign and persist a claim locally before any publication | user story | J-001 | yes |
| US-003 Publish a signed claim to the author's PDS | user story | J-001 | yes |
| US-004 Read back local claims by subject | user story | J-001 | yes |
| US-005 Bootstrap a starter philosophy seed lexicon | technical task | J-001 (enables) | n/a — tech task, links to US-001 |

**Slice composition rule**: 4 user-visible stories, 1 supporting technical task. Slice
contains user-observable behavior — passes the "no slice is 100% @infrastructure" gate.

## Walking skeleton task list (one task per Activity 1 sub-step)

| # | Task | Demonstrates |
|---|---|---|
| WS-1 | `openlore claim add ...` composes a record and shows it before signing | CLI compose + "not as truth" framing |
| WS-2 | `<Enter>` signs and writes to DuckDB at `~/.local/share/openlore/openlore.duckdb` | Signing + DuckDB persistence |
| WS-3 | `<Y>` publishes the record to the user's PDS as `org.openlore.claim` | ATProto Lexicon + publication |
| WS-4 | `openlore graph query --subject <subject>` reads it back from DuckDB | Local query + round-trip identity |
| WS-5 | `openlore claim retract <cid>` issues a counter-claim referencing the original | Retraction model (RC-02 default) |

## Taste tests (carpaccio quality gates)

| Test | Pass/Fail | Note |
|---|---|---|
| End-to-end demoable in a single session? | PASS | 2-minute demo once installed |
| Delivers value to a real user (not just infrastructure)? | PASS | Author produces and reads their own claim |
| Independently shippable (no future-slice dependency leaking in)? | PASS | No dependency on slice-02/03/04/05 |
| Has a named learning hypothesis? | PASS | See "Disproves if it fails" |
| Slice is ≤ ~1 week of focused work? | PASS | 3-5 days estimated |

## Open red cards (must resolve before DESIGN)

- **RC-01**: Confidence semantics (numeric vs bucketed vs both). Proposed default: numeric `[0.0, 1.0]` + display-only buckets. Confirm with user.
- **RC-02**: Retraction model. Proposed default: soft-retract + counter-claim-only; never hard-delete. Confirm with user.
- **RC-03**: Signing identity. Proposed default: reuse existing ATProto DID with per-app key derivation. Confirm with user.

## Hand-off targets

- DESIGN (solution-architect): full slice artifacts in `docs/feature/openlore-foundation/discuss/`.
- DEVOPS (platform-architect): outcome KPIs in `outcome-kpis.md`.
- DISTILL (acceptance-designer): journey YAML with embedded Gherkin + this slice brief.
