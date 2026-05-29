# ADR-027: `openlore search` Verb + CLI→Indexer HTTP/XRPC Transport + Graceful Local-Only Degradation

- **Status**: Proposed
- **Date**: 2026-05-28
- **Deciders**: Morgan (nw-solution-architect), per WD-106/WD-109/WD-110 + OD-AV-2/OD-AV-3/OD-AV-5/OD-AV-6 for openlore-appview-search (slice-05)
- **Feature**: openlore-appview-search (slice-05)
- **Extends**: ADR-003 (CLI verb contract), ADR-013 (the `--federated` flag precedent), ADR-020 (the `graph query` explorer-flag precedent). Reuses the slice-03 `peer add` path verbatim (WD-110 / I-FED-5).
- **Resolves**: OD-AV-2 (CLI→indexer transport), OD-AV-3 (graceful degradation mechanism), OD-AV-5 (discovery surface grammar), OD-AV-6 (share-link resolver).

## Context

slice-05 needs a user-facing discovery surface and a way for the `openlore` CLI to
reach the `openlore-indexer`'s query API. The decisions:

- **OD-AV-5 (grammar)**: a new top-level `openlore search` verb vs a `--network`
  flag on the existing `openlore graph query`.
- **OD-AV-2 (transport)**: how the CLI reaches the indexer (in-process? local HTTP?
  ATProto XRPC?).
- **OD-AV-3 (graceful degradation)**: the mechanism by which `search` degrades when
  the index is unreachable (the requirement is LOCKED by WD-106 / KPI-5; the
  mechanism is DESIGN's).
- **OD-AV-6 (share resolver)**: how a `--share` link resolves (CLI re-run vs a web
  AppView — a full web UI is OUT of scope).

The forces:

- **Corpus clarity (WD-109 / the journey)**: search runs against the NETWORK index,
  a clearly distinct corpus from the LOCAL graph that `graph query` runs against.
  The user must never be confused about which corpus they are querying.
- **Transport must not couple to deployment (ADR-023)**: the CLI talks to a URL;
  whether that URL is `localhost` (self-hosted) or remote (future hosted) is config,
  not a code change. This is what makes the ADR-023 hosted-mode revisit purely
  additive.
- **ATProto-stack consistency**: the project already speaks XRPC (ATProto) and uses
  `reqwest` (rustls). The query API should fit that idiom.
- **Local-first guardrail (KPI-5, cardinal)**: `search` must NEVER block, hang, or
  fatally error when the index is unreachable; it degrades to a clear local-only
  message; the local `graph query` is untouched.
- **Scope (WD-100, the umbrella's easiest-to-scope-creep slice)**: a full web
  AppView is OUT; the share link is held to a stable, query-encoding,
  attribution-preserving contract.

## Decision

**Add a NEW top-level verb `openlore search` (OD-AV-5) that queries the network
index over an HTTP transport carrying an XRPC-style query method
(`org.openlore.appview.searchClaims`) — the CLI calls the indexer at a CONFIGURED
URL (default `http://127.0.0.1:<port>` for the self-hosted case; OD-AV-2). When the
indexer is unreachable, `search` degrades gracefully to a clear local-only message
that points to `openlore graph query` and exits without a fatal error (OD-AV-3).
The `--share` link encodes the QUERY (not a snapshot) and resolves by CLI re-run
(OD-AV-6); a web resolver is OUT of scope.**

### Why a NEW `openlore search` verb (not a `--network` flag on `graph query`)

| Factor | New `search` verb — **CHOSEN** | `--network` flag on `graph query` |
|---|---|---|
| **Corpus clarity (WD-109)** | A distinct verb makes the corpus boundary unambiguous: `graph query` = LOCAL graph (own + subscribed peers); `search` = NETWORK index. The user is never confused about what they are querying or whether bytes cross the wire. | A flag on `graph query` blurs the boundary: the same verb sometimes hits the local store offline and sometimes hits a network service. The local-first mental model ("`graph query` is always local/offline") is load-bearing and a flag would erode it. |
| **Local-first mental model** | `graph query` stays unambiguously local + offline (no network); `search` is the one verb that talks to the network. Clean separation of "offline-always" from "network-discovery". | A `--network` flag means `graph query` SOMETIMES does network I/O — surprising for a verb users learned as offline (the ADR-016 "predictability" principle). |
| **Grammar consistency** | A new verb for a new corpus mirrors how slice-03 added `peer` verbs for the new federation concern (not flags on `claim`). The dimensions (`--object`/`--contributor`/`--subject`) mirror slice-04 for habit-continuity, but on a new verb. | Reuses the learned `graph query` surface, but at the cost of corpus ambiguity — the wrong trade for the load-bearing distinction. |
| **Degradation clarity** | `search` degrading to "index unavailable, local-only" and POINTING to `graph query` is a clean two-verb story (one network, one local). | A degrading flag would have `graph query --network` silently behave like `graph query` (no flag) — a confusing no-op-on-failure. |

The `search` verb's grammar mirrors slice-04's explorer dimensions for
habit-continuity (the user already learned `--object`/`--contributor`/`--subject`),
plus `--show <cid>` (US-AV-004) and `--share` (US-AV-006):

```
openlore search --object <philosophy>      # headline: search by philosophy at network scale
openlore search --contributor <did|handle> # one author's whole network reasoning trail
openlore search --subject <project>         # what the network says about a project
openlore search --object ... --show <cid>   # inspect a result: signature + CID verification lines
openlore search --object ... --share        # emit a stable query-encoding shareable link
```

### Why HTTP transport carrying an XRPC-style query method (OD-AV-2)

| Factor | HTTP + XRPC-style query method — **CHOSEN** | In-process / local IPC alternatives |
|---|---|---|
| **Deployment-independence (ADR-023)** | The CLI talks to a configured URL. `localhost` (self-hosted) today; a remote host (future hosted mode) is a config change, NOT a code change. Makes the ADR-023 hosted revisit purely additive. | In-process would force the indexer INTO the CLI (rejected by ADR-023); a unix-socket IPC would couple to the self-hosted-same-machine case and block the future remote case. |
| **ATProto-stack consistency** | The project speaks XRPC and uses `reqwest` (rustls). An XRPC-style query method (`org.openlore.appview.searchClaims`, a `query` lexicon) over HTTP fits the idiom; the indexer's query surface is an XRPC endpoint, the CLI a `reqwest` client. | A bespoke protocol re-invents what XRPC already gives (typed query lexicon, JSON responses). |
| **Reuse** | Reuses the workspace `reqwest` (rustls) (zero new transport dependency — same as slice-02's `adapter-github`); the response shape reuses the slice-03 signed-claim JSON. | n/a. |
| **Probe + degradation** | An HTTP client is trivially probe-able (reachability) and degrade-able (connection refused → local-only). | A socket/IPC has the same probe-ability but worse deployment-independence. |

The query method (XRPC `query` lexicon `org.openlore.appview.searchClaims`):

```
GET /xrpc/org.openlore.appview.searchClaims
  ?dimension=object|contributor|subject
  &value=<philosophy URI | did | project URI>
  [&cid=<cid for --show>]
Response (200): { results: [ { author_did, cid, subject, predicate, object,
                               confidence, evidence[], composed_at,
                               verified_against, references[] }, ... ],
                  distinct_author_count, total_claims }
  -- Every result carries author_did (anti-merging, WD-103). NO merged/consensus
     object in the response shape. Grouping is the CLI renderer's job.
Response (404 dimension empty): { results: [], suggestion?: <near-match> }   -- exit 0 at CLI
```

The CLI→indexer transport is itself a driven port (`IndexQueryPort`) with an HTTP
adapter (`HttpIndexQueryAdapter`) in the CLI's effect shell — so the CLI's
composition root wires + probes it like any adapter (ADR-009), and a test double
(`FakeIndexQuery`) keeps acceptance hermetic.

### Graceful local-only degradation (OD-AV-3; KPI-5 cardinal)

The LOCKED requirement (WD-106): `search` must never block the local-first flows.
The mechanism:

- The CLI's `HttpIndexQueryAdapter` treats indexer-unreachable (connection refused,
  timeout, DNS failure) as a NON-FATAL `IndexQueryError::Unreachable`, distinct from
  a query error.
- `VerbSearch` catches `Unreachable` and prints a clear, non-fatal message:
  `"Network index unavailable. Showing LOCAL results only (own + subscribed peers).
  Run \`openlore graph query --object ...\` for the local graph."` then EITHER
  (DELIVER's call, US-AV-002 Example 3) prints the pointer OR delegates to the
  EXISTING local `graph query` path for the same dimension — both are acceptable;
  the contract is "clear local-only message, no hang, no fatal error, exit
  non-fatally".
- The local `graph query` verb is COMPLETELY UNTOUCHED — it links no indexer code,
  needs no indexer, and works exactly as slice-04 shipped it. `search` is the ONLY
  verb that talks to the network; its failure can never affect a local verb.
- The composition-root probe for `HttpIndexQueryAdapter` is SKIPPED-OR-SOFT at CLI
  startup (like ADR-009's `--offline` PDS probe skip): the CLI must START without a
  reachable indexer (otherwise an unreachable indexer would block `claim add`). The
  indexer reachability is checked at `search`-time, not at every CLI startup — a
  per-verb soft-fail, NOT a global hard-fail (exactly the ADR-016 per-peer-soft /
  global-hard distinction).

### The `--share` link: query-encoding, CLI-re-run resolver (OD-AV-6; WD-110)

- `--share` emits a stable link encoding the QUERY (dimension + value), e.g.
  `openlore://search?object=org.openlore.philosophy.reproducible-builds`. It
  encodes NO results, NO snapshot, NO merged view (WD-110 / anti-merging across the
  share boundary).
- The resolver is CLI re-run: opening the link runs the encoded query against the
  index, re-composing per-author-attributed verified results — so the link always
  resolves to CURRENT results and never a stale/attribution-losing snapshot
  (US-AV-006 Example 4).
- The link SCHEME (`openlore://` deep link vs an `https://` URL) is DELIVER's call
  within this contract; the walking skeleton ships the `openlore://`-style query
  string + a CLI handler that parses it back into a `search` invocation.
- A web AppView resolver is OUT of scope (WD-100 scope-creep line; OD-AV-6). The
  link is a stable, query-encoding, attribution-preserving artifact resolvable by
  the CLI; a presentational web layer is a future slice.

### The discovery→federation funnel reuses `peer add` verbatim (WD-110)

A `search` result that includes an unfollowed author ends with a render-only
affordance `"Follow this author: \`openlore peer add <did>\`"`. The affordance
PRINTS the existing slice-03 command; it does not execute it (no auto-follow). The
funnel reuses the slice-03 `peer add`/`peer pull`/`peer remove` path with NO
parallel subscription state (I-FED-5). This is a renderer concern, not a new verb.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **`--network` flag on `graph query`** | Rejected (OD-AV-5). Blurs the LOCAL-vs-NETWORK corpus boundary (WD-109) and erodes the load-bearing "`graph query` is always local/offline" mental model — `graph query --network` would sometimes cross the wire, surprising for a verb learned as offline. A distinct `search` verb makes the corpus + the network-vs-local boundary unambiguous and keeps degradation clean (one network verb, one local verb). |
| **In-process indexer (no transport)** | Rejected (folds into ADR-023's rejection of an in-process indexer). Would drag network-ingest deps into the local-first CLI and couple the corpus to one process. |
| **Unix-domain-socket / local IPC transport** | Rejected. Couples to the self-hosted-same-machine case and blocks the future remote/hosted deployment (ADR-023 revisit) without a code change. HTTP-to-a-configured-URL is deployment-independent. |
| **A custom binary protocol over TCP** | Rejected. Re-invents what XRPC-over-HTTP already provides (typed query lexicon, JSON responses, `reqwest` reuse). No benefit at walking-skeleton scale. |
| **Hard-fail `search` (exit non-zero, no local fallback) when the index is unreachable** | Hard reject — violates KPI-5 (cardinal, release-blocking). `search` MUST degrade gracefully to a clear local-only message and never block the local-first flows. |
| **Probe the indexer at EVERY CLI startup (so all verbs fail fast if the indexer is down)** | Rejected. Would make an unreachable indexer block `claim add` / `graph query` — exactly the KPI-5 regression the slice must avoid. The indexer reachability is a per-`search`-verb soft check (the ADR-016 per-peer-soft / global-hard distinction), not a global startup gate. |
| **A web AppView resolver for `--share` in slice-05** | Rejected/deferred (OD-AV-6 / WD-100 scope line). A full web UI is the umbrella-identified scope-creep risk. The link is held to a stable, query-encoding, CLI-resolvable contract; a web layer is a future slice. |
| **`--share` encodes a frozen result snapshot** | Hard reject (WD-110 / WD-103). A snapshot loses attribution + goes stale. The link encodes the QUERY so it re-composes current per-author-attributed verified results. |

## Consequences

### Positive

- The corpus boundary (LOCAL `graph query` vs NETWORK `search`) is unambiguous; the
  local-first mental model survives the network-service shift.
- HTTP-to-a-configured-URL makes the ADR-023 hosted-deployment revisit purely
  additive (the CLI talks to a URL; localhost or remote is config).
- Graceful degradation makes KPI-5 a structural property: `search` is the only verb
  that talks to the network, its failure is a per-verb soft-fail, and the local
  verbs link no indexer code.
- Zero new transport dependency (reuses workspace `reqwest`/rustls); the query API
  is XRPC-idiomatic (ATProto-stack consistency); the funnel + share reuse existing
  surfaces (`peer add`, the slice-04 dimension grammar).
- The `IndexQueryPort` driven-port abstraction keeps acceptance hermetic
  (`FakeIndexQuery`) and the CLI composition-root probe-able (ADR-009).

### Negative

- **`search` requires a reachable indexer to return network results** (otherwise
  local-only). Accepted: this IS the graceful-degradation contract; the CLI's core
  value needs no indexer. Mitigation: a clear local-only message + the pointer to
  `graph query`.
- **A new XRPC query lexicon (`org.openlore.appview.searchClaims`)** to define +
  version. Mitigation: it is a READ query (no signed payload, no CID stability
  concern); it lives under the existing `org.openlore.*` namespace (ADR-005).
- **The `openlore://` deep-link scheme registration** is a desktop-integration
  concern for a future web resolver. Accepted: the walking skeleton ships the link
  as a copy-pasteable query string resolvable by `openlore search` re-run; OS
  scheme registration is a future concern (OD-AV-6 web layer deferred).

### Earned Trust

The CLI→indexer transport is a network dependency the CLI MUST probe AND degrade
from (principle 12). The `HttpIndexQueryAdapter` (driven port `IndexQueryPort`)
ships a `probe()` within the 250ms budget (ADR-009 I-4/I-5) that exercises the
catalogued substrate-lie scenario:

1. **Reachability + response-shape**: against a fixture indexer, the probe confirms
   the query method returns the expected XRPC response shape with per-result
   `author_did` present (the anti-merging-across-the-transport check — a response
   that dropped `author_did` is a contract violation caught at probe time).
2. **Unreachable degrades, does NOT refuse**: the probe asserts that an UNREACHABLE
   indexer yields `IndexQueryError::Unreachable` (a soft, non-fatal outcome), NOT a
   startup refusal — because the CLI MUST start without a reachable indexer (KPI-5).
   This is the inverted-probe case: the probe verifies that the FAILURE mode is
   graceful, not that the dependency is up. (A CLI that hard-refused on an
   unreachable indexer would be the KPI-5 regression; the probe catches it.)
3. **The local-first gold test**: `local_first_preserved` (release gate) — with the
   indexer down (and network disabled), `claim add` / `claim publish` (offline) /
   `graph query` ALL succeed, and `search` prints the local-only message without a
   fatal error. This is the slice-05 "what if the network service is down?" check;
   the design makes the network surface non-load-bearing for authoring.

**External integration note (handoff to platform-architect)**: the CLI→indexer
HTTP/XRPC boundary AND the indexer→network-author DID-document/PDS read boundary
(ADR-024/026) are the two cross-process / external boundaries in slice-05. Both are
candidates for consumer-driven contract tests (see component-boundaries.md handoff).

## Revisit Trigger

- A hosted/community indexer is offered (ADR-023 revisit). The CLI already talks to
  a configured URL; add auth (an API token header) to the `IndexQueryPort` — an
  additive transport concern.
- A web AppView resolver for `--share` becomes a JTBD (KPI-AV-6 shows shared links
  are heavily used and a web render would amplify them). Add the web resolver — the
  link already encodes the query, so the web layer is additive.
- The `--share` link scheme needs OS-level registration (deep-link handling).
  Define the `openlore://` scheme registration per-platform (a packaging/ADR-011
  concern).
- Interactive follow (a confirm-prompt that EXECUTES `peer add` from a result)
  becomes a JTBD (US-AV-005 Technical Notes flags this as a possible later
  release). Add an interactive affordance — the funnel already reuses `peer add`.
