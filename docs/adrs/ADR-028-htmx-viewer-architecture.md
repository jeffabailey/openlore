# ADR-028: `openlore ui` Viewer — Verb Shape, Pure/Effect Split, and Read-Only Web Surface

- **Status**: Proposed
- **Date**: 2026-05-31
- **Deciders**: Morgan (nw-solution-architect), per OD-VIEW-5 / OD-VIEW-7 + I-VIEW-1..6 for htmx-scraper-viewer (slice-06)
- **Feature**: htmx-scraper-viewer (slice-06)
- **Extends**: ADR-007 (functional-Rust paradigm: pure core / effect shell), ADR-009 (hexagonal composition root, WIRE→PROBE→USE), ADR-023 (composition-root capability boundaries), ADR-027 (the slice-05 hyper serving + soft-probe + degradation precedent the viewer mirrors).
- **Resolves**: OD-VIEW-5 (binary shape), OD-VIEW-7 (binding + auth). Companion ADRs: ADR-029 (maud templating + pure-core allowlist), ADR-030 (the read-only store-read port seam).

## Context

slice-06 adds a **read-only htmx viewer** for the node operator (Maria) on **localhost**.
It serves the operator's OWN node store (`claims` from slice-01, `peer_claims` from
slice-03) and a **live, ephemeral** scrape-proposal view that reuses the slice-02
`GithubPort` propose pipeline. The hard invariants (requirements.md §Inherited
Invariants):

- **I-VIEW-1 / I-VIEW-3 (read-only + human gate)**: no route writes or signs; signing
  stays exclusively in the CLI (inherits I-SCR-1 from slice-02).
- **I-VIEW-2 (no key in web process)**: the viewer process never loads or holds the
  signing key.
- **I-VIEW-4 (localhost only)**: the viewer binds a loopback address only.
- **I-VIEW-5 (derived-from display-only)**: derived-from renders only on live-scrape
  candidates, never on persisted claims (inherits WD-62).
- **I-VIEW-6 (local-first / offline)**: the store views work fully offline against the
  local DuckDB (inherits slice-01 KPI-5).

The open questions DESIGN must resolve:

- **OD-VIEW-5 (binary shape)**: standalone binary, fold into `openlore-indexer`, or a
  new verb on the main `openlore` CLI?
- **OD-VIEW-7 (binding + auth)**: bind address, port, conflict handling, and whether a
  localhost-only personal dashboard needs auth.

The forces:

- **Cohesion with the store + scraper (the operator's own node)**: the viewer serves
  the operator's OWN store (`claims` + `peer_claims`) and reuses the CLI's existing
  `GithubPort` for live scrape. Both capabilities already live in/around the `cli`
  composition root (the `scrape github` verb wires `GithubPort`; the `claim`/`graph`
  verbs wire `StoragePort` over the same DuckDB file). Cohesion lives in/around `cli`.
- **No second store, no second composition root (BR-VIEW-4)**: the viewer must read the
  SAME local DuckDB the CLI writes, with no second store and no separate schema.
- **The human gate is structural, not conventional**: I-SCR-1 has been enforced since
  slice-02 by the `adapter-github` adapter holding NO storage/identity/PDS reference.
  The viewer must extend that discipline: the web process must be incapable of signing
  by construction, not merely by not calling a sign function.
- **Personal-dashboard threat model**: the viewer is reachable only from the operator's
  own machine; it is not a public surface.

## Decision

**Add a NEW `openlore ui` verb on the main `openlore` CLI binary (OD-VIEW-5). It binds
`127.0.0.1` ONLY, with no authentication (OD-VIEW-7). It is structured as a
pure-domain + effect-adapter split per ADR-007: a new pure `viewer-domain` crate
(view-models + `render_*` functions producing maud markup) and a new effect
`adapter-http-viewer` crate (hyper server, route table, wiring of a READ-ONLY
store-read port + the existing `GithubPort`). The web process holds NO signing key and
links NO write/sign path — the read-only store-read port (ADR-030) is the structural
guarantee.**

### Why a new `openlore ui` verb (not a standalone binary, not folded into the indexer)

| Factor | `openlore ui` verb — **CHOSEN** | Standalone `openlore-viewer` binary | Fold into `openlore-indexer` |
|---|---|---|---|
| **Cohesion (own store + own scraper)** | The viewer serves the operator's OWN store + reuses the CLI's `GithubPort`. Both already live in/around the `cli` root. A verb keeps the wiring next to the adapters it reuses. | A separate binary would duplicate the DuckDB-open + `GithubPort::from_env` wiring the CLI already owns. | The indexer serves the NETWORK index (`indexed_claims`), a DIFFERENT corpus — it holds no `claims`/`peer_claims` store and no `GithubPort` (ADR-023 / I-AV-5). Wrong home. |
| **No second store (BR-VIEW-4)** | Reuses `DuckDbStorageAdapter::open` over the SAME file the CLI verbs use; zero new schema, zero second store. | Same store is reachable, but the open + path resolution (`OpenLorePaths`) is CLI-owned; a second binary re-implements it. | The indexer deliberately holds NO local store (I-AV-5 / ADR-023); folding the viewer in would breach that boundary. |
| **Runtime reuse** | Reuses the established CLI tokio current-thread runtime builder (`verbs::claim_publish::build_tokio_runtime`), the same one `scrape github` already uses to drive `GithubPort`. | Re-creates the runtime shape. | The indexer owns a multi-thread runtime for a different workload. |
| **Capability boundary (ADR-023)** | `cli` already links `adapter-duckdb` + `adapter-github`. Adding `adapter-http-viewer` to `cli` keeps the two composition roots disjoint (`xtask check-arch::check_indexer_capability_boundary` stays green: the viewer's HTTP server is NOT the indexer's `adapter-xrpc-query-server`). | A third composition root would need its own capability-boundary rule. | Breaches the indexer's signing-incapable + store-less boundary. |
| **Operator mental model** | `openlore ui --port 8788` is discoverable next to the verbs the operator already runs (`openlore claim add`, `openlore scrape github`). | A second binary is a second thing to install/find. | Confusing: "why does the network indexer show my local claims?" |

### Pure/effect split (ADR-007)

The viewer is split along the pure-core / effect-shell line exactly as slice-05 split
`appview-domain` (pure) from `adapter-xrpc-query-server` (effect):

- **`viewer-domain` (PURE, new crate)**: view-model ADTs (`ClaimRowView`,
  `ClaimDetailView`, `PeerClaimRowView`, `CandidateRowView`, `PageView<T>`, and the
  empty/error view states) + total `render_*` functions producing `maud::Markup` →
  `String` HTML. No I/O, no DuckDB, no network. Joins the `xtask check-arch` pure-core
  allowlist with `maud` whitelisted (ADR-029).
- **`adapter-http-viewer` (EFFECT, new crate)**: the hyper 1.x server + route table; it
  holds a READ-ONLY store-read port (ADR-030) + the existing `GithubPort`, calls
  `viewer-domain` to render, and exposes a real (non-stub) `probe()` per the
  `xtask check-probes` gate. Holds NO signing key, NO `IdentityPort`, NO `PdsPort`, NO
  write-capable `StoragePort`.
- **`cli` (EXTEND)**: the `ui` verb parses `--port`, resolves `OpenLorePaths`, opens the
  DuckDB read handle, builds the read-only store-read port + the `GithubPort`, binds the
  loopback listener, walks the viewer probe, and runs the serve loop on the reused
  current-thread runtime.

### Binding + auth (OD-VIEW-7)

- **Bind `127.0.0.1` ONLY**. The listener is constructed from a hard-coded loopback
  address; there is NO `--host` override in this slice. A non-loopback bind is a later
  slice with its own ADR (and its own threat model — auth, TLS, origin checks). This is
  stated as a **security boundary**: the viewer is not reachable off-host.
- **No authentication**. The viewer is a personal, loopback-exclusive dashboard. Anyone
  who can reach `127.0.0.1` already has a local account on the operator's machine and
  could read the DuckDB file directly; adding auth would protect nothing the OS does not
  already protect, while adding a credential-storage surface the slice deliberately
  avoids (no key, no secret, no session store).
- **Default port `8788`** (the value the user stories already show), overridable via
  `--port`. A bind conflict (port in use) is a clean, plain-language startup refusal
  naming the port and suggesting `--port` (NFR-VIEW-6), reusing the established
  `health.startup.refused` shape (ADR-009) — never a raw stack trace.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Standalone `openlore-viewer` binary** | Rejected (OD-VIEW-5). Would duplicate the CLI-owned DuckDB-open + path resolution + `GithubPort::from_env` wiring and add a third composition root needing its own capability-boundary rule, for no cohesion gain — the viewer serves the operator's OWN store + reuses the CLI's scraper, both of which live in/around `cli`. |
| **Fold the viewer into `openlore-indexer`** | Hard reject (OD-VIEW-5 / ADR-023 / I-AV-5). The indexer serves the NETWORK index (`indexed_claims`), holds NO local `claims`/`peer_claims` store, NO `GithubPort`, and is signing-incapable + store-less by design. The viewer serves a different corpus (the operator's own store) and would breach the indexer's capability boundary. |
| **Give the web process the full `StoragePort`** | Hard reject (I-VIEW-1 / I-VIEW-2). `StoragePort` carries `write_signed_claim` / `record_publication`; handing it to the web process would make a write/sign path reachable from a route by construction. The read-only store-read port (ADR-030) is the structural defense. |
| **Bind `0.0.0.0` (LAN-reachable) with optional auth** | Rejected (OD-VIEW-7 / I-VIEW-4). A non-loopback bind makes the operator's full store readable off-host and demands auth + TLS + origin checks — a materially larger threat model. Out of scope for slice-06; a future slice + its own ADR. |
| **Add token/password auth to the localhost dashboard** | Rejected (OD-VIEW-7). On a loopback-only surface, auth protects nothing the OS file permissions do not already protect (a local user can read the DuckDB file directly) while adding a credential-storage surface the no-key/no-secret slice avoids. |
| **htmx + a JSON API (SPA-style)** | Rejected for the walking skeleton. The stories want server-rendered HTML the operator opens in a browser; a JSON API + client framework adds a build toolchain and a second representation for no slice-06 outcome. Server-rendered maud HTML is the simplest correct surface; htmx progressive enhancement (partial swaps for pagination) is an additive renderer concern within the same route table. |

## Consequences

### Positive

- The viewer's wiring lives next to the adapters it reuses (`adapter-duckdb` read handle
  + `adapter-github`), with zero second store and zero second composition root; the two
  existing composition roots (`cli`, `openlore-indexer`) stay disjoint.
- The read-only + no-key guarantees are STRUCTURAL: the web process links a read-only
  store-read port (ADR-030) and no `IdentityPort`/`PdsPort`/write `StoragePort`, so a
  route CANNOT write or sign — `xtask check-arch` + the probe enforce it, not convention.
- Loopback-only + no-auth keeps the slice's attack surface minimal and matches the
  personal-dashboard threat model; the security boundary is explicit and testable
  (bind address is loopback; no external interface bound).
- Reuses the proven slice-05 hyper skeleton + slice-02 runtime builder + slice-01/03
  DuckDB store — near-zero new infrastructure risk.

### Negative

- **A non-loopback / multi-user deployment is a future slice** (its own ADR + threat
  model). Accepted: the slice-06 outcome is a personal localhost dashboard; remote
  access is explicitly out of scope.
- **Two new production crates** (`viewer-domain`, `adapter-http-viewer`): 19 → 21.
  Accepted: it mirrors the proven slice-05 pure-domain + effect-adapter split and keeps
  the pure rendering core testable in isolation (no server needed to test `render_*`).
- **The CLI binary grows the hyper + maud dep surface** (already transitively present:
  `hyper`/`hyper-util`/`http-body-util` via the workspace; `maud` is the one new MIT
  dep). Accepted and bounded; documented in technology-stack.md.

### Earned Trust

The viewer's effect shell depends on the local DuckDB (read), the loopback TCP bind, and
(for `/scrape`) the network via `GithubPort`. Per principle 12, `adapter-http-viewer`
ships a REAL `probe()` (non-stub; satisfies `xtask check-probes`) run by the composition
root BEFORE the serve loop accepts traffic (WIRE→PROBE→USE). The probe exercises the
load-bearing slice-06 invariants as self-probes — see ADR-030 §Earned Trust for the
read-only + offline + capability checks.

**External integration note (handoff to platform-architect)**: the `/scrape` route's
dependency on the GitHub public API (via the existing `GithubPort` / `adapter-github`)
is the one external boundary in slice-06. It is already covered by slice-02's
`adapter-github` probe + contract-test candidacy (ADR-019). The viewer adds NO new
external integration — it reuses the slice-02 boundary verbatim.

## Revisit Trigger

- A non-loopback / multi-user deployment becomes a JTBD: a new slice + ADR adds bind
  configuration, auth, TLS, and origin/CSRF defenses (the threat model this slice
  deliberately scopes out).
- The viewer needs to mutate state (e.g. an in-browser sign flow): would breach
  I-VIEW-1/I-SCR-1 and require revisiting the human-gate decision (currently CLI-only).
- The pure rendering core grows enough that a templating engine with runtime I/O is
  considered: would require de-allowlisting maud (ADR-029) and re-running the pure-core
  adjudication.
