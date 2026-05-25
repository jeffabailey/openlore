# Alternatives Considered — openlore-foundation (slice-01)

> Wave: DISCUSS, ask-intelligent expansion (fired trigger: cross-context complexity)
> Date: 2026-05-25
> Owner: Luna (nw-product-owner)
> Purpose: document the rejected alternatives for the three biggest design choices
> made in slice-01, so DESIGN inherits the reasoning rather than re-arguing it.

This file covers three choices:

1. Local storage engine for slice-01 (DuckDB vs Kùzu vs SurrealDB).
2. Identity model for OpenLore claims (reuse-DID-with-derived-key vs mint-fresh-DID).
3. CLI verb shape for the compose/sign/publish flow.

Each section follows the same structure: chosen option, alternatives weighed,
evaluation criteria, per-alternative rejection rationale, and the conditions that
would re-open the decision.

---

## Choice 1: Local storage engine

**Chosen for slice-01**: **DuckDB**.

Locked by WD-8. Explicitly re-opened in slice-04 (`openlore-scoring-graph`) where
graph traversal becomes the dominant workload.

### Alternatives weighed

| Option | One-line summary |
|---|---|
| DuckDB (chosen) | Embedded columnar SQL engine; Rust client mature; portable single-file DB. |
| Kùzu | Embedded native graph database; Cypher-like query language; designed for graph traversal. |
| SurrealDB | Multi-model (document + graph + key-value); server or embedded; richer query model. |

### Evaluation criteria

- **Portability** — single-file, OS-agnostic, no daemon.
- **Embeddedness** — runs in-process; no separate server to install or maintain.
- **Rust client maturity** — usable stable bindings, active maintenance, low surprise.
- **Graph-query support** — native traversal vs SQL-with-joins vs Cypher.
- **Learning curve for the user** — slice-01 user (P-001) is comfortable with SQL.
- **License** — permissive (MIT / Apache-2.0 / BSD), no copyleft hazard.

### Why each alternative was rejected for slice-01

- **Kùzu rejected (for slice-01)**: slice-01 needs only indexed key/predicate lookup
  (single-subject query); the graph-traversal advantage is unused. Adopting Kùzu now
  pays a learning-curve and Rust-client-maturity cost for capability we do not yet
  need. **Re-opens in slice-04** when triangulation weighting requires multi-hop
  contributor↔project↔philosophy traversal.
- **SurrealDB rejected (for slice-01)**: multi-model flexibility is genuine, but the
  embedded story is younger than DuckDB's and the surface area is wider than the
  walking skeleton needs. The "one binary, one file, SQL" simplicity of DuckDB wins
  on the "kill or validate the thesis fast" axis that defines walking-skeleton
  selection.

### What would change the decision in the future

- **For slice-04 (scoring-graph) re-opening**: if benchmark on a representative
  graph (~10k claims, ~5 hop traversals) shows DuckDB recursive CTE latency exceeds
  the per-query budget set in slice-04 KPIs, switch to Kùzu for the graph workload
  and keep DuckDB for tabular/log workloads (or migrate fully to Kùzu).
- **If we hit a portability blocker on a target platform** (e.g. a Windows
  edge-case in the DuckDB Rust client), revisit SurrealDB which has different
  packaging trade-offs.
- **If a new Rust-native embedded graph engine matures** to the point where its
  bindings are as stable as DuckDB's, re-evaluate.

---

## Choice 2: Identity model for OpenLore claims

**Chosen**: **reuse the user's existing ATProto DID with a per-application derived key.**

Locked by WD-12 (OD-4).

### Alternatives weighed

| Option | One-line summary |
|---|---|
| Reuse existing ATProto DID + per-app derived key (chosen) | OpenLore claims are signed by a key derived from the user's main DID. The derived key can be revoked independently of the parent identity. |
| Mint a fresh DID per OpenLore install | Each OpenLore install creates its own `did:plc` (or similar) used only for OpenLore claims. |

### Evaluation criteria

- **Revocability blast radius** — if a key is compromised, what gets nuked?
- **User friction** — how many setup steps and ceremonies the user must perform.
- **Key-management complexity** — how many secrets the user has to track.
- **Claim portability across devices** — can the same identity sign claims from
  multiple machines without manual key migration?
- **Recovery story** — what happens when the user loses a device or key?

### Why each alternative was rejected

- **Mint a fresh DID rejected**: a fresh DID forces the user to set up and maintain
  a second identity that is also less recognizable to peers. Peers reading a claim
  signed by `did:plc:openlore-jeff-laptop-7a3f` cannot trivially correlate it to
  the same person they follow on Bluesky. It also doubles the recovery story: lose
  the OpenLore-only DID and the OpenLore reputation is unrecoverable, even when
  the main ATProto identity is intact. The derived-key approach gives us
  independent revocability (compromise of the OpenLore key does NOT compromise the
  main ATProto identity) WITHOUT splitting the user's social-graph identity.

### What would change the decision in the future

- **Regulatory or jurisdictional reasons** that make signing application data with
  a person's primary identity a liability (e.g. claims become subject to subpoena
  in a way that an ATProto handle would not).
- **A widely-adopted ATProto convention** that standardizes per-app DIDs and gives
  them peer-discoverability features the bare DID currently lacks.
- **A demonstrated cryptographic weakness** in the derivation scheme chosen by
  DESIGN that makes derived keys less trustworthy than freshly minted ones.

---

## Choice 3: CLI verb shape — compose / sign / publish

**Chosen** (read from the journey YAML and US-001..US-003): **separate verbs with a
chained interactive prompt convenience.**

Specifically:

- `openlore claim add ...` opens an interactive flow: it composes, shows the
  preview ("not as truth"), prompts for Enter to sign, then prompts for Y to publish.
- `openlore claim publish <cid>` is its own standalone verb, available for retries
  (US-003 Example 2) and for users who want to defer publication.
- There is **no** `--publish` flag on `claim add` that fuses sign-and-publish into
  a single atomic action.

So sign and publish are conceptually separate steps; the interactive `claim add`
flow merely chains them within one session for ergonomics.

### Alternatives weighed

| Option | One-line summary |
|---|---|
| Separate verbs, chained prompts (chosen) | `claim add` composes + signs + offers publish; `claim publish <cid>` exists standalone. |
| Combined verb with flag: `claim add --publish` | One command does compose + sign + publish atomically; no separate publish verb needed for the common path. |

### Evaluation criteria

- **Reviewable preview moment** — is there an explicit pause where the user reads
  the composed record before anything is signed or published?
- **Atomicity** — does a single keystroke commit across the local-store /
  federated-PDS boundary?
- **Error recovery** — when publication fails, can the user retry without re-signing?
- **Scripting ergonomics** — can the user automate the common path without losing
  the preview gate?
- **The "Not as truth" preview gate** — is the framing moment psychologically
  load-bearing for the J-001 anxiety force?

### Why the alternative was rejected

- **`claim add --publish` (combined verb) rejected**: it collapses the two
  emotional beats of the journey YAML — "I see what I am signing" and "I am
  crossing the federated boundary" — into one keystroke. The middle step
  (local-only persistence, journey step 2) is described as the load-bearing trust
  buffer; collapsing it defeats the entire emotional arc. It also makes
  publication retry harder: a failed `--publish` would either leave a signed-but-
  unpublished claim (creating the same retry surface anyway) or roll the
  signature back (breaking the round-trip identity guarantee for sign).

### What would change the decision in the future

- If the J-001 KPI signals show the chained-prompt friction is itself a barrier
  ("users get to the publish prompt and bail out" — KPI-6 day-30 signal), a
  scripting-only `--auto-publish` flag could be added later. This would be additive
  and would never replace the interactive default.
- If a future slice (e.g. scrapers in slice-02) needs batch publish, the standalone
  `openlore claim publish <cid>` already provides the building block; the chained
  interactive flow remains the canonical user-driven entry point.

### Residual tension flagged for DESIGN

The locked WD/OD set does **not** explicitly settle the wire-level question of
*whether `claim add` and `claim publish` share an internal sign-then-publish code
path that DESIGN can collapse into a single sign+publish atomic transaction at the
storage layer*. From the requirements side:

- Sign and persist (US-002) MUST be atomic on disk.
- Publish (US-003) MUST be retryable without re-signing.
- The chained prompts in `claim add` MUST present sign and publish as two distinct
  user-confirmed beats.

DESIGN is free to share code internally between the chained-prompt path and the
standalone `claim publish` path, but the user-observable contract is fixed: two
prompts, two verbs, one combined session for ergonomics. If DESIGN finds an
implementation that violates the chained-prompt observable contract while sharing
a transaction, **come back to product-owner before shipping** — the journey YAML
emotional arc, not the implementation efficiency, is the authority here.

---

## Changelog

- 2026-05-25 — Luna — initial write under ask-intelligent expansion (cross-context complexity trigger).
