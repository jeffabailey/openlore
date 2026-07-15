# ADR-062: Self-Hosted Serverless Instance = Opaque, Content-Addressed HTTP Blob Transport

- **Status**: Proposed
- **Date**: 2026-07-15
- **Deciders**: Morgan (nw-solution-architect), per D-1..D-9 locks from Luna
  (nw-product-owner) for `serverless-philosophy-federation` (DISCUSS), and SPIKE-00
  (`docs/feature/serverless-philosophy-federation/spike/findings.md`, verdict WORKS-conditionally).
- **Feature**: serverless-philosophy-federation (slices 01-05)
- **Extends**: ADR-023 (self-hostable indexer; signing-incapable-by-construction) — this ADR is
  the ADDITIVE, self-hosted-serverless realization of the hosted mode ADR-023 deliberately
  DEFERRED. ADR-009 (hexagonal modular monolith), ADR-007 (functional Rust), ADR-027 (configurable
  URL). Inherits the J-003 (`openlore-federated-read`) verify-before-trust + anti-merging discipline
  (ADR-016 / KPI-FED-6) for cross-instance pull.
- **Resolves**: OD-SF-1 (Rust↔Worker boundary + record storage), OD-SF-3 (cross-instance pull
  transport), and the storage-medium / CID-conformance questions handed to DESIGN.
- **References**: ADR-006 (CID / canonical dag-cbor — a latent conformance gap is documented below,
  NOT modified here), ADR-023 (reconciled, NOT modified), ADR-027 (configurable URL, reused).

## Context

`serverless-philosophy-federation` lets a user deploy their OWN Cloudflare Worker instance
(a NEW `atproto/` TS deployment target, D-2), `push` locally-signed `org.openlore.claim` records
to it, serve a public read-only philosophy card, `pull` the instance back into local DuckDB, and
`pull` from OTHER users' instances (J-003 federated read, transport delta). The load-bearing
guardrail is **round-trip CID integrity** (KPI-SF-1, North Star): a claim pushed then pulled back
MUST recompute to an identical CID, or the whole mirror/card thesis collapses.

SPIKE-00 tested the single riskiest assumption — does a signed claim's content-addressed CID
survive a round-trip through a Cloudflare Worker? — against the REAL `crates/claim-domain` with the
5 gold fixtures on local `wrangler dev` (workerd). The **binary finding**:

> The CID survives **iff** the Worker is an OPAQUE, CID-addressed byte store (stores/returns the
> record bytes VERBATIM, never re-encodes). If the Worker re-canonicalizes the record (the ATProto
> server-assigned-CID `putRecord` model), the CID does **NOT** survive.

Root cause (confirmed against the real core): `claim-domain` (`ciborium`) emits RFC 8949
shortest-form floats (`0.0 → f90000`, `0.5 → f93800`, `1.0 → f93c00`, float16), while IPLD
DAG-CBOR mandates strict float64. So openlore's canonical bytes advertise codec `0x71` (dag-cbor)
but are NOT dag-cbor-conformant for shortest-float values. **Any "round" confidence a user types
(`0.0`/`0.25`/`0.5`/`0.75`/`1.0`) makes a standard JS `@ipld/dag-cbor` PDS compute a DIFFERENT CID
than the Rust core.** The 4 matching gold fixtures matched only by luck (their decimals are not
f16-representable). This is a latent ADR-006 conformance gap; the opaque transport SIDESTEPS it.

The consequence for DESIGN: **the CID must be minted by exactly ONE canonicalizer — the Rust
`claim-domain` core.** Any second CID computer (a JS/Worker IPLD library) is a permanent
cross-language federation hazard.

ADR-023 rejected a *central or community-operated* hosted service on sovereignty grounds but
explicitly deferred an ADDITIVE hosted mode ("the CLI already talks to a configurable URL
(ADR-027), so this is additive"). This feature's hosting model is **SELF-HOSTED SERVERLESS**: each
user deploys and OWNS their instance; federation is peer-to-peer between users' own instances; no
shared operator is in the trust path (D-1/D-3/D-4). The owner and the operator are the same person,
so the central-authority failure mode ADR-023 rejected does not arise.

## Decision

**A user's serverless instance is an OPAQUE, CONTENT-ADDRESSED HTTP BLOB STORE. It stores and
returns raw openlore record bytes VERBATIM, keyed by the CLI-minted CID; it computes no CID, runs
no IPLD/CBOR library, and canonicalizes nothing. The CID is minted ONLY by
`claim-domain::compute_cid`; the pull side (own and cross-instance) recomputes the CID and verifies
the signature LOCALLY in Rust. The ATProto server-assigned-CID `putRecord` model is REJECTED.**

### 1. The opaque transport contract (driving ports, Worker/TS side)

| Method | Semantics |
|---|---|
| `PUT /records/:cid` | Store the request body (a lexicon-JSON signed record) VERBATIM under key `:cid`. The Worker NEVER parses it for CID purposes, NEVER re-encodes, NEVER derives a CID. Idempotent: re-PUT of an existing `:cid` is a no-op success. |
| `GET /records/:cid` | Return the exact bytes stored under `:cid` (or 404). Byte-identical to what was PUT. |
| `GET /manifest` | Return the instance's index: an ordered list of stored CIDs + a per-CID DISPLAY projection (`author_did`, `subject`, `predicate`, `object`, `confidence`, `composed_at`) written by the CLI on push. Advisory-for-display only; trust is always re-established by the puller in Rust. Enables enumeration for `publish pull`, cross-instance `peer pull`, and the card. |
| `GET /` | The public read-only philosophy card (HTML), rendered from `/manifest`. Signing-incapable; renders ONLY pushed claims, each attributed to its `author_did`; no consensus/merged row (anti-merging, D-7). |

The Worker parses only JSON (the manifest for display); it NEVER touches the record blob's
semantics. "Zero Lexicon/CBOR knowledge on the CID path" is preserved by construction.

### 2. Storage medium — a single Durable Object per instance ("repo" object)

The instance stores record blobs + the manifest in a **single Durable Object** (transactional,
strongly consistent). Rationale: each user owns exactly ONE instance (single-tenant), so a single
DO models "my repo, which I own" (D-3); strong read-after-write protects the KPI-SF-1 round-trip
(push-then-immediately-pull must see the record) and the interrupted-push resume (US-SF-003) via an
atomic manifest append; blobs are ~1 KB, far under the DO 128 KB/key limit. See Alternatives for KV
and R2.

**Concurrency**: the DO is strongly consistent and single-threaded per request, so manifest writes
serialize. Concurrent pushes of the SAME claim (identical CID) are idempotent by construction
(content-addressed key; re-PUT is a no-op) and are assumed rare — the happy path is a single author
from a single machine (D-3). The idempotency key + interrupted-resume marker for bulk push is a
DELIVER concern (Q-SF-D3); the DO's atomic manifest makes it tractable.

**Operating cost** (the "each user owns their instance" model must be affordable): a typical
personal instance (~100 claims, ~1 KB each, low card traffic) sits within Cloudflare's free /
near-free tier — Workers requests + Durable Object storage + egress for a single-author low-traffic
card are pennies-per-month order. Cost scales with card audience + claim count, not with a central
operator (there is none). A per-user cost/monitoring note belongs in the instance setup docs
(DEVOPS); it is not a slice-01 blocker.

**Availability / latency**: publish is ADDITIVE and never on the authoring hot path — a Worker
cold-start, transient outage, or high latency makes `publish push`/`pull` fail NON-fatally (exits
non-zero, local store untouched); compose/sign/`graph query` are structurally independent of the
instance (KPI-SF-5). A Worker outage cannot corrupt the local source of truth. Real-time publish
confirmation is not a requirement; round-trip CID integrity (KPI-SF-1) is latency-insensitive.

### 3. The record blob format — verbatim lexicon-JSON signed record

The pushed blob is the lexicon-JSON signed record (the exact `{subject, predicate, object,
evidence, confidence, author, composedAt, references, reason, signature:{kid, alg, sig}}` shape the
J-003 `parse_signed_claim` already consumes), stored VERBATIM and keyed by the Rust-minted CID.
Round-trip integrity holds because (a) the Worker returns the bytes verbatim — unlike a real PDS it
never reserializes — and (b) Rust is the SOLE canonicalizer: on pull the CLI re-parses the JSON,
re-canonicalizes in Rust, recomputes the CID, and byte-matches it against the key (exactly J-003's
WD-24 discipline). A `confidence` of `0.0`/`0.5`/`1.0` (f16-representable) crossing the boundary is
the regression guard that keeps the rejected `putRecord`/re-encode model from creeping back.

### 4. OD-SF-3 — cross-instance pull (req 4, J-003 transport delta)

Standard ATProto DID-document resolution is REUSED unchanged: a peer's DID document
`serviceEndpoint` resolves to their Cloudflare instance URL (`IdentityPort::resolve_peer`,
unchanged). The RECORD transport is the byte-preserving opaque read (`GET /manifest` +
`GET /records/:cid`), NOT XRPC `listRecords`. The Rust side decodes the lexicon JSON (reusing the
J-003 parse), recomputes the CID, and verifies the signature LOCALLY — reusing the J-003 verb-level
verification / per-author attribution / anti-merging VERBATIM (peer claims land in `peer_claims`,
never merged). The spike's "req-4 falls out for free" holds ONLY under a byte-preserving read; the
opaque read IS that transport.

### 5. CID-conformance — leave the core as-is (documented revisit trigger)

The opaque transport does NOT require the core to be dag-cbor-conformant, so the ciborium-vs-DAG-CBOR
shortest-float nonconformance (ADR-006 latent gap) is LEFT UNCHANGED for now. Fixing it would change
the CID of every claim with an f16-representable confidence — a wire break for every shipped gold CID
and every already-published claim. Revisit trigger below.

### 6. Composition-root + capability boundary (Earned Trust)

`crates/cli` stays the ONLY Rust composition root (ADR-009 / I-3). The write-capable `PublishPort`
(minting + `PUT /records/:cid`) is wired ONLY in the `openlore publish` root. Cross-instance pull
and the card read through a READ-ONLY instance surface that exposes NO write method — so a peer's
instance can never be written to from the pull path (the write-incapable extension of the ADR-023
signing-incapable boundary). "Wire then probe then use" applies: the publish adapter's `probe()`
round-trips a canary CID against the configured instance and refuses to start
(`health.startup.refused`) if the byte round-trip or reachability fails.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **ATProto `putRecord`, server-assigned CID (a thin standard PDS)** | REJECTED. SPIKE-00 proved the CID diverges for f16-representable confidence (`0.0`/`0.5`/`1.0`) because a JS `@ipld/dag-cbor` PDS re-canonicalizes to strict float64 while the Rust core emits shortest-form float16. Would require reimplementing ciborium's shortest-float encoding in the Worker — a permanent cross-language federation hazard. Breaks KPI-SF-1 (the North Star). |
| **OD-SF-3 option (a): Worker also exposes an XRPC `listRecords`/`getRecord` read surface wrapping the opaque blobs** | Considered — would make J-003 peer-pull LITERALLY unchanged (existing `PdsPort` XRPC path works verbatim, echoing the stored CID key, deriving nothing). Rejected as the default because it gives the Worker an asymmetric surface (opaque write + XRPC read) and leaks ATProto-view knowledge into the "dumb store". Option (b) keeps the Worker minimal + symmetric + opaque-pure and puts the (small) transport variance in Rust where the single canonicalizer/verifier already lives. Documented as the fallback if literal-zero-Rust-change J-003 reuse is later preferred. |
| **Storage medium: Workers KV** | Rejected for slice-01. Simplest + globally edge-cached (good for the card), but eventually-consistent LIST/read risks flaking the KPI-SF-1 push-then-pull round-trip CI test, and the single mutable manifest key hits KV's ~1 write/sec/key limit under bulk push (US-SF-003). Immutable content-addressed blobs tolerate eventual consistency; the manifest does not. |
| **Storage medium: R2 object storage** | Rejected for slice-01 (overkill for ~1 KB blobs; weaker list-after-write than a DO). Retained as the escalation path if evidence attachments make blobs large (revisit trigger). |
| **Record blob = raw canonical dag-cbor bytes (SPIKE-00's literal probe artifact)** | Considered — makes the CID a direct `multihash(stored bytes)` identity (the strongest round-trip form). Rejected as the default because it needs a NEW pure CBOR-decode path in `claim-domain` (inverse of `canonicalize`) for pull-reconstruction and a CBOR-aware card, whereas the lexicon-JSON blob reuses the shipped J-003 `parse_signed_claim` verbatim (zero new decode) and keeps the card CBOR-free. Both honor the opaque invariant; JSON maximizes reuse. |
| **Reuse Rust crates compiled to WASM inside the Worker (OD-SF-1 option c)** | Rejected. Heaviest option; the opaque store needs zero domain logic on the Worker, so a WASM `claim-domain` on the Worker would be a second CID computer — the exact hazard the single-canonicalizer invariant forbids. |
| **Fix ADR-006 dag-cbor float conformance now (strict float64)** | Rejected for this feature. CID-changing wire break for every existing claim with an f16-representable confidence; the opaque transport does not need it. Deferred to a revisit trigger (standard-PDS interop JTBD). |

## Consequences

### Positive

- **Data sovereignty preserved (D-1/D-3/D-4)**: each user owns their instance; no shared operator
  in the trust path. This is precisely the ADDITIVE hosted mode ADR-023 said "costs nothing" to
  defer — realized without re-creating the central-authority failure mode.
- **KPI-SF-1 (North Star) protected by construction**: a single canonicalizer (Rust) + a
  byte-verbatim store + recompute-and-match-on-pull make round-trip CID integrity a structural
  property, not a policy. The `0.0`/`0.5`/`1.0` regression guard fences off the rejected re-encode
  model.
- **Maximum reuse**: `claim-domain` (CID/verify), `adapter-atproto-did` (DID resolution), and the
  J-003 verb-level verification/attribution/anti-merging are REUSED unchanged; the JSON blob reuses
  the shipped `parse_signed_claim`. The Worker is a dumb, language-agnostic store.
- **The card is signing-incapable + write-incapable by construction** (D-7), mirroring the ADR-023
  indexer boundary and extending it: even the pull path holds no write capability toward a peer.
- **Local-first preserved (KPI-SF-5)**: publish is purely additive; compose/sign/`graph query`
  never depend on the instance being reachable; the local DuckDB stays canonical.

### Negative

- **A new TS deployment target (`atproto/`) to build, test, deploy, and document** — a second
  language + toolchain (wrangler/workerd) in an otherwise all-Rust workspace. Mitigation: the Worker
  is deliberately dumb (store + manifest + card, no domain logic); its correctness surface is small
  and its riskiest property (byte round-trip) is CI-guarded.
- **No ATProto `putRecord` wire-compat + no server-side field indexing** — a generic ATProto client
  cannot write to, or index, an openlore instance. Accepted: the CID *is* the contract and must be
  minted by exactly one canonicalizer; server-side indexing is the indexer's job (ADR-023), not the
  personal instance's.
- **The manifest is a single serialization point** (one DO). Fine for a single-author personal
  instance (low write volume); would need revisiting for a multi-tenant offering (out of scope).
- **The latent ADR-006 dag-cbor float nonconformance remains** — sidestepped, not fixed. Documented
  with a revisit trigger so future standard-PDS interop is a conscious, migrated decision.

### Earned Trust

Per principle 12, every dependency the instance transport does not probe is an act of faith. The
opaque transport's load-bearing invariants are enforced at three semantically orthogonal layers
(mirroring ADR-009 / ADR-023):

| Layer | What it checks | Tool |
|---|---|---|
| **Subtype / type** | Cross-instance pull + the card read through a READ-ONLY instance port (`get_record`/`list_manifest`) that exposes NO `put_record`/write method — the type system makes writing-to-a-peer un-callable from the pull path. The write-capable `PublishPort` is a distinct trait. | Rust trait split (`PublishPort` write vs read-only instance port) |
| **Structural / arch** | `xtask check-arch` gains `publish_write_capability_isolated`: only the `openlore publish` composition root may wire the write-capable `PublishPort`; the peer-pull/card read surfaces MUST NOT depend on it. No JS IPLD/CBOR/multiformats dependency may appear on the `atproto/` Worker's CID path (a dependency-manifest guard). | `xtask check-arch` (new rule) + `atproto/` dependency lint |
| **Behavioral / probe** | (a) `adapter-publish-http.probe()` round-trips a CANARY claim at confidence `0.0`/`0.5`/`1.0` against the configured instance and refuses to start (`health.startup.refused{reason: publish.cid_roundtrip_failed}`) on any byte/CID divergence — the SPIKE-00 regression guard as a startup gate; (b) a CI round-trip smoke test (push→pull→assert CID equality, KPI-SF-1) exercises the f16-representable confidence values that would catch a re-encode regression. | composition-root probe (ADR-009 mechanism) + CI round-trip test |

## Revisit Trigger

- **Standard-PDS interop becomes a JTBD** (a non-openlore ATProto client must compute/verify
  openlore CIDs, or openlore must publish to a generic PDS with server-assigned CIDs). Then fix the
  ADR-006 dag-cbor float nonconformance (emit strict float64) as a coordinated, migrated CID change
  — at which point the opaque transport and a standard PDS could co-exist. Until then: opaque only.
- **Record blobs grow large** (e.g. embedded evidence attachments exceed the DO per-key budget) →
  move blob storage to R2, keep the manifest in the DO. Re-evaluate the medium, not the contract.
- **A genuine multi-tenant hosted offering is pursued** — auth, rate-limiting, per-tenant isolation,
  and an operational SLA become first-class. This is ADR-023's THIRD revisit trigger and remains OUT
  of scope; the single-owner opaque instance is not a multi-tenant SaaS.
- **The card needs higher-trust identity binding to Bluesky** (beyond a manual profile-link paste)
  → add `did:web`/handle bidirectional verification (OD-SF-4 alternative), currently deferred.
