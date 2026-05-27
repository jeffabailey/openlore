# Alternatives Considered — openlore-federated-read (slice-03)

- **Wave**: DISCUSS, ask-intelligent expansion (fired trigger: cross-context complexity — slice spans CLI, ports, DuckDB, Lexicon, ATProto adapter)
- **Date**: 2026-05-27
- **Owner**: Luna (nw-product-owner)
- **Purpose**: explicitly document the rejected alternatives for the
  three biggest design choices in slice-03 so DESIGN does not relitigate
  them in flight.

The three choices documented here:

1. **Counter-claim verb shape** — `claim counter <cid>` sugar vs `claim add --counters <cid>` flag.
2. **Peer storage layout** — single DuckDB with new tables vs separate DB file vs new store crate.
3. **Pull mechanism** — pull-on-demand vs auto-pull vs push subscription.

---

## Choice 1: Counter-claim verb shape

### Options considered

| Option | Form | Pro | Con |
|---|---|---|---|
| A | `openlore claim counter <target_cid> --reason "..." [other flags]` (sugar verb) | Discoverable; matches user mental model "I'm countering THIS claim"; reads naturally in scripts and help text | Adds a new verb to the ADR-003 verb list (needs ADR-013 amendment); slight verb-count growth |
| B | `openlore claim add --counters <target_cid> --reason "..." [other flags]` (flag-augmented existing verb) | Reuses existing `claim add` verb; ADR-003 stays unamended | Less discoverable (`--counters` is buried in flags); breaks the conceptual symmetry with `claim retract` (which IS a verb, per slice-01 DD-9); reads poorly in scripts ("`claim add --counters`" is awkward) |
| C | `openlore claim oppose <target_cid> ...` or `openlore disagree <target_cid> ...` (more emotionally-charged verb) | Stronger emotional framing; closer to natural language | "Oppose"/"disagree" carry combative connotations contrary to the J-001/J-003 framing of "structured public stake, not a fight"; harder to internationalize |

### Chosen: Option A — `openlore claim counter <target_cid>`

### Rationale

- Discoverable: the tip line in `openlore graph query --federated` output names a specific copy-pasteable command. Verb form is easier to surface than flag form ("`openlore claim counter bafy...`" is more obvious than "`openlore claim add --counters bafy...`").
- Symmetric with `claim retract <cid>` (which is also a verb, per slice-01 DD-9). Both verbs construct claims with `references[]` entries; both reuse the slice-01 publish pipeline. The user learns one pattern.
- Reads cleanly in scripts and acceptance tests.
- The verb-count cost (one new verb) is acceptable; the ADR-013 amendment is small (3 new verbs: `peer add`, `peer pull`, `peer remove`, AND `claim counter`, AND a new `--federated` flag on `graph query`).

### Trade-offs / future revisit

- A future slice may want to add `claim corrects <cid>` (for typo fixes), `claim supersedes <cid>` (for full replacement), and `claim refines <cid>` (for elaborations) as additional sugar verbs. The Lexicon already supports these via the same `references[]` mechanism (ADR-008's `ReferenceType` enum has all four variants). Slice-03 ships only `Counters`; later slices add the remaining sugar verbs as the JTBD validates each.
- If user testing reveals the verb is rarely used (KPI-FED-3 < 10%), reconsider whether the verb should be more discoverable (e.g., promoted in the federated query footer as a first-class CTA, not just a tip).

---

## Choice 2: Peer storage layout

### Options considered

| Option | Form | Pro | Con |
|---|---|---|---|
| A | Single DuckDB file, two new tables: `peer_subscriptions` + `peer_claims` alongside existing `author_claims` | Single file; one migration; one backup target; reuses existing `adapter-duckdb` | Risk: cross-table queries may JOIN in a way that elides author DID (the anti-merging invariant must be enforced by check-arch); single file means peer purge requires DELETE WHERE author_did = X (slower than dropping a partition) |
| B | Two DuckDB files: `openlore.duckdb` (author) + `peer_claims.duckdb` (peer) | Cleaner separation; peer purge is dropping rows from a separate DB; no risk of accidental JOIN | Two files to back up; two migrations to keep in sync; introduces a 2-DB transaction concern if a query needs both (it does, for federated query) |
| C | New crate `crates/adapter-peer-store` with its own backing (DuckDB or sled or sqlite) | Explicit boundary; can swap backend without touching author storage | Premature; slice-03 has no evidence that peer storage needs a different backend; adds a new adapter to maintain |

### Chosen: Option A — single DuckDB file, two new tables

### Rationale

- Simplicity wins for slice-03: the federation thesis is what's being tested, not the storage layer's flexibility. Validating the wire contract with the simplest storage that holds the invariants is the right tradeoff.
- The anti-merging invariant is enforceable via `xtask check-arch` (a clippy lint or schema test asserting "no query may JOIN author_claims and peer_claims in a way that elides the author_did column"). DESIGN owns the exact enforcement mechanism.
- Peer purge as `DELETE WHERE author_did = X` on `peer_claims` is fine at slice-03 scale (peer claim counts in the low thousands, not millions). Performance becomes a concern if a user follows many peers with many claims each; revisit at slice-04 or beyond.
- Reuses `adapter-duckdb` and `adapter-duckdb::probe()` infrastructure; no new adapter to write, test, or maintain.

### Trade-offs / future revisit

- If slice-04 scoring-graph introduces graph-traversal queries that span author_claims + peer_claims, the single-DB layout makes those queries natural. If those queries surface performance hot spots, the storage layout becomes a revisit candidate.
- If a user reports that their peer_claims has grown unwieldy (e.g., >100k rows from many peers), revisit whether peer_claims should be partitioned per-peer-DID for faster purge.

---

## Choice 3: Pull mechanism

### Options considered

| Option | Form | Pro | Con |
|---|---|---|---|
| A | Pull-on-demand: user explicitly runs `openlore peer pull` | Simple; user-controlled; predictable (no surprise network activity); aligns with CLI-first paradigm | Requires user to remember to pull; staleness window between subscribe and first pull |
| B | Auto-pull on subscribe: `openlore peer add <did>` immediately runs the equivalent of `peer pull` for that DID | Lower friction at first use; user immediately sees the peer's claims | Subscribe action now has variable runtime (depends on peer's claim count); failure modes for subscribe are larger surface |
| C | Push subscription via ATProto Firehose or similar streaming protocol | Real-time freshness; matches social-media expectations | Requires a long-running process (daemon), violates the CLI-first paradigm; adds substantial protocol complexity (ATProto firehose semantics, reconnection logic, ordering guarantees) unrelated to the J-003 hypothesis under test |

### Chosen: Option A — pull-on-demand

### Rationale

- Brief explicitly says "start with pull on demand" — re-affirmed here as the right call for the JTBD validation.
- Simplicity: pull-on-demand is one verb, one network operation, one set of failure modes. Slice-03's goal is to validate the federation contract, not to engineer a freshness experience.
- Predictability: user knows exactly when network activity happens. This matches the CLI-first paradigm of slice-01 (compose-and-sign is offline; publish is the network step).
- Auto-pull on subscribe (Option B) was rejected because it conflates two operations with different failure surfaces. If a peer's PDS is slow or down, subscribe should still succeed; the user can pull later.
- Push subscription (Option C) was rejected because it introduces a daemon (violates CLI-first per slice-01 constraints) and protocol complexity (firehose semantics) that has nothing to do with the federation contract being validated. Push is post-MVP at the earliest.

### Trade-offs / future revisit

- Staleness window: between subscribe and first pull, the user has zero peer claims. The CLI mitigates this with explicit "next pull: on-demand" hint and a copy-pasteable suggestion in the peer-add output.
- Background pull / scheduled pull (e.g., `openlore peer pull --daemon` or a cron-friendly mode) is a slice-04 candidate IF dogfeed reveals user fatigue around manual pulls.
- The pull mechanism informs DEVOPS instrumentation: KPI-FED-5 (end-to-end latency) measures from `peer pull` start to first `graph query --federated` result.

---

## Cross-cutting trade-off summary

| Tradeoff | Slice-03 default | Revisit when |
|---|---|---|
| Verb count growth | Add 1 verb (`claim counter`) | If verb count exceeds ~12 (currently 8 after slice-03) |
| Schema simplicity vs separation | Single DB, two new tables | If peer_claims grows beyond ~100k rows OR if slice-04 graph queries reveal hot spots |
| Pull pattern | Pull-on-demand | If KPI-FED-5 reveals friction OR users explicitly request scheduled pull |

These tradeoffs are LOCKED for slice-03 design. DESIGN may revisit them if
they encounter evidence that contradicts the rationale above; otherwise the
choices are inputs, not open questions.

---

## Changelog

- 2026-05-27 — Luna — initial write under ask-intelligent expansion (cross-context complexity trigger; slice spans CLI + ports + DuckDB + Lexicon + ATProto adapter).
