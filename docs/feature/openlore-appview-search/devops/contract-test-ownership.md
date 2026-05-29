# Contract-Test Ownership — openlore-appview-search (slice-05)

- **Wave**: DEVOPS
- **Date**: 2026-05-28
- **Architect**: Apex (nw-platform-architect)
- **Feature**: openlore-appview-search (sibling slice-05)
- **Inherits**: the slice-01 foundation Pact suite (`contract-pact-pds`), the slice-03 `contract-pact-pds-peer` (D-D12/D-D15), and the slice-02 `contract-pact-github` (D-D22/D-D24) — all carry forward UNCHANGED; slice-05 ADDS two consumer-driven contract suites.

Slice-05 adds the FIRST external/cross-process integration boundaries since slice-01
(PDS/keychain) and slice-02 (GitHub). Per Apex Core Principle 10 (shift-left quality
gates: PR-stage contract tests) + WD-123 + DESIGN §6.2/§6.4, external integrations
are the highest-risk boundary; slice-05 has TWO, and both are annotated here for
consumer-driven contract tests. This doc fixes OWNERSHIP, the Pact-style suites, the
public-endpoint allowlists, and the recorded-fixture discipline — mirroring the
slice-03 `contract-pact-pds-peer` (D-D12 mocked-in-PR / real-in-release-with-approval,
D-D15 fixture regeneration) + the slice-02 `contract-pact-github` public-endpoint
allowlist (D-D22).

## 1. The two boundaries (DESIGN §6.2)

| # | Boundary | Consumer | Provider | What the contract pins | The guardrail it protects |
|---|---|---|---|---|---|
| **B1** | **CLI → Indexer** (`org.openlore.appview.searchClaims` XRPC over HTTP, ADR-027) | the `openlore` CLI (`adapter-index-query` / `HttpIndexQueryAdapter`) | the indexer's query server (`adapter-xrpc-query-server`) | the XRPC query REQUEST shape (dimension/value/cid params) + the RESPONSE shape — and the LOAD-BEARING invariant that **every result element carries `author_did`** and there is NO `consensus`/`merged`/`aggregate` object in the response | **KPI-AV-2 (anti-merging across the transport, I-AV-2)** — a PROVIDER change that dropped per-result `author_did` would silently merge authors away in production; the contract catches it at build time. |
| **B2** | **Indexer → Network Author PDS + PLC Directory** (record enumeration + DID-doc resolution, ADR-024/026) | the indexer (`adapter-atproto-ingest` for `listRecords`; `adapter-atproto-did` resolve-only for the PLC DID-doc) | arbitrary network-author PDSes (`com.atproto.repo.listRecords`) + the PLC directory (`https://plc.directory` DID-document resolution) | the record-enumeration RESPONSE shape (the `org.openlore.claim` record + its signature/CID fields the verify-gate reads) + the PLC DID-document shape (the `verificationMethod` + `publicKeyMultibase` `z6Mk...` the ADR-026 decode reads) | **KPI-AV-3 (verified-before-index, I-AV-1)** — an ATProto/PLC response-shape drift would silently break the ingest verification gate (the record's signature/CID could not be read, or the pubkey could not be decoded); the contract catches it at build time. |

These are consumer-driven contracts: the CONSUMER's expectations are the contract;
the PROVIDER is verified against them. Both are the same Pact discipline as slice-03
(`contract-pact-pds-peer`) + slice-02 (`contract-pact-github`).

## 2. Boundary B1 — CLI ↔ Indexer (the cross-PROCESS, same-org boundary)

### 2.1 Ownership + suite (D-D36)

- **Suite**: `contract-pact-indexer-query` — a NEW Pact sub-job in the existing CI
  acceptance stage (mirrors the slice-03 `contract-pact-pds-peer` sub-job placement).
- **Consumer (owns the contract)**: the `openlore` CLI's `adapter-index-query`. The
  CLI declares the request it sends + the response shape it depends on (every result
  carries `author_did`; the optional `suggestion` on empty; `distinct_author_count`
  + `total_claims`).
- **Provider (verified against the contract)**: the indexer's
  `adapter-xrpc-query-server`. The provider verification asserts the server's actual
  `org.openlore.appview.searchClaims` responses satisfy the consumer contract.
- **Shared DTO source of truth**: the request/response DTOs live in `lexicon` (the
  `org.openlore.appview.searchClaims` query lexicon + `SearchQueryRequest`/
  `SearchQueryResponse`; component-boundaries §`lexicon` grouping note). Both ends
  consume the SAME DTOs, so the contract test guards against DRIFT between the
  hand-written wire handling and the lexicon, NOT against two independently-evolving
  schemas. This is why B1 is a same-org cross-process boundary, not a true
  third-party contract — but it is STILL contract-tested because the CLI must not
  link the server crate (the two ends are compiled into different binaries; a
  provider change can ship without the consumer noticing).

### 2.2 What the contract covers

```
org.openlore.appview.searchClaims (a query lexicon; a READ query; no signed payload)

Request (the consumer sends):
  dimension : "object" | "contributor" | "subject"
  value     : <philosophy URI | did | project URI>
  cid?      : <cid>   (for --show)

Response 200 (the consumer depends on):
  { results: [ { author_did, cid, subject, predicate, object, confidence,
                 composed_at, verified_against, evidence[], references[] }, ... ],
    distinct_author_count, total_claims }
  INVARIANT (LOAD-BEARING, I-AV-2): EVERY element of results carries author_did.
  There is NO consensus/merged/aggregate object anywhere in the response shape.
  Every result carries verified_against (drives the [verified] marker).

Response empty: { results: [], suggestion?: <near-match> }   -- consumer exits 0
```

Pact interactions to pin (the consumer contract):
- happy path — a multi-author result; assert EVERY element carries `author_did`
  (the anti-merging-across-the-transport pin) + `verified_against`.
- the multi-author-same-(subject,object) case — two distinct `author_did`s on the
  same (subject,object) come back as TWO result elements (the contract proves the
  wire never merges them; I-AV-2).
- empty dimension — `{ results: [], suggestion?: ... }` → the consumer exits 0
  (distinct from `--show <cid not present>` which is a usage error, exit non-zero).
- `--show <cid>` — a single-result-by-cid response carrying `verified_against`.
- the NO-merged-object assertion — the consumer contract asserts the response has no
  `consensus`/`merged`/`aggregate` key (a provider that ADDED one would be a
  contract violation, catching a future "the network says X" regression).

### 2.3 Fixture + gating (mirrors D-D12)

- **PR + nightly**: MOCKED. The Pact runs the consumer against a Pact MOCK provider
  AND verifies the indexer's server against the consumer contract IN-PROCESS (an
  in-process test server reusing the chosen HTTP framework's test utilities;
  `technology-stack.md` §test-only). Hermetic — no real indexer process needed; no
  network. < 30 s.
- **Release-tag**: the same contract is re-verified against a REAL `openlore-indexer
  serve` instance bound to localhost (an ephemeral port) — proving the actual built
  binary's server honors the contract end-to-end. Gated by the SAME manual-approval
  environment as the slice-03 real-PDS Pact (D-D12), but NOTE: B1's "real" provider
  is the project's OWN indexer binary on localhost (NOT a third party), so it carries
  NO third-party-rate-limit / external-flakiness concern — it is a local end-to-end
  re-verification, ~30 s, low risk.

### 2.4 Allowlist (B1)

B1's provider is the project's own indexer on localhost — there is NO public-endpoint
allowlist concern (no third-party host is contacted). The localhost bind
(`127.0.0.1:<ephemeral>`) is the only network endpoint, and it is the test's own
process. This is SIMPLER than B2 (and than slice-02's GitHub allowlist): a same-org
localhost boundary needs no egress allowlist.

## 3. Boundary B2 — Indexer → Network Author PDS + PLC Directory (the EXTERNAL, adversarial-input boundary)

### 3.1 Ownership + suite (D-D37)

- **Suite**: `contract-pact-pds-network` — a NEW Pact sub-job in the existing CI
  acceptance stage (mirrors slice-03 `contract-pact-pds-peer` + slice-02
  `contract-pact-github`). It EXTENDS the existing PDS contract surface
  (`contract-pact-pds` / `-peer`) to the NETWORK-author read paths + ADDS the PLC
  DID-document contract.
- **Consumer (owns the contract)**: the indexer — `adapter-atproto-ingest` (the
  `listRecords` enumeration) + `adapter-atproto-did` resolve-only (the PLC DID-doc
  resolution + the `publicKeyMultibase` shape the ADR-026 decode reads).
- **Provider (verified against recorded fixtures)**: arbitrary network-author PDSes
  (`com.atproto.repo.listRecords` for `org.openlore.claim` records) + the PLC
  directory (`https://plc.directory`). Because the providers are third-party + many
  (arbitrary network authors), the contract is verified against RECORDED FIXTURES
  (the hermetic ingest fixtures already model these shapes; DESIGN §6.4), NOT a live
  provider on every CI run.

### 3.2 What the contract covers

Two response shapes the verify-before-index gate (I-AV-1) + the ADR-026 decode depend on:

1. **`com.atproto.repo.listRecords`** (network-author record enumeration; reuses the
   slice-03 `contract-pact-pds-peer` `listRecords` Pact, EXTENDED to network authors):
   - happy path — a page of `org.openlore.claim` records, each carrying the signed
     payload (`author`/`subject`/`object`/`predicate`/`confidence`/`signature`) + the
     published CID the ingest gate recomputes-and-matches (WD-104).
   - pagination cursor + empty result + 404 collection (carried from slice-03).
   - **the adversarial set** (the network-lies pin, the cardinal KPI-AV-3 surface):
     a tampered-signature record, a CID-mismatch record, an unsigned record — the
     contract pins that the verify-before-index gate REJECTS all three (the same
     `at-indexer-rejects-unverified-claim` adversarial fixtures, modelled as Pact
     interactions). This mirrors the slice-03 `at-peer-tampered-signature-rejected`
     adversarial fixture, extended to network ingest.

2. **PLC DID-document resolution** (`https://plc.directory/<did>` → the DID document;
   NEW for slice-05, the ADR-026 production decode):
   - happy path — a `did:plc:*` DID document carrying a `verificationMethod` with a
     `publicKeyMultibase` `z6Mk...` value the `decode_ed25519_multibase` helper reads.
   - the DID-doc shape pin — the `#org.openlore.application` verification method
     locating the key the signature verifies against (`verified_against`, ADR-026).
   - the rejection boundary — an unsupported DID method or key type (NOT `did:plc:*`
     Ed25519 `z6Mk...`) is EXPLICITLY rejected (Q-DELIVER-AV-6; the explicit-rejection
     messages DISTILL asserts) → the record is NOT indexed (`did_unresolvable`).
   - the network-lies-about-a-key pin — a tampered DID-doc (a key that does NOT verify
     the claim's signature) is REJECTED; the indexer never indexes a claim whose
     signature does not verify against the PLC-resolved key (the trust anchor is the
     PLC doc, never the record's forgeable `author` field).

### 3.3 Fixture + gating (mirrors D-D12 + D-D15 + the slice-02 D-D22 allowlist)

- **PR + nightly**: RECORDED FIXTURES (consumer-driven contracts against committed
  recordings). The `listRecords` recordings extend the slice-03 `tests/contracts/pact/`
  set; the PLC DID-doc recordings are NEW (a recorded `did:plc:*` doc with a real
  `z6Mk...` key — the SAME known test keypair as the `adapter-atproto-did` decode gold
  test, `test-support` real-`z6Mk...` fixture). Hermetic — no real PLC/PDS in PR. < 30 s.
- **Release-tag**: the same contracts re-verified against the REAL providers once at
  release, gated by the SAME manual-approval environment as the slice-03 real-bsky
  Pact (D-D12):
  - `listRecords` against `bsky.social` (reuses the slice-03 real-PDS approval gate).
  - the PLC DID-document resolution against the REAL `https://plc.directory` (NEW for
    slice-05) — proving the ADR-026 production decode reads a REAL PLC doc shape, not
    just the recorded fixture. This is the release-time confirmation that KPI-AV-3
    holds against REAL network data (the cardinal slice-05 concern the DV-4 seam left
    open since slice-03).
- **Adversarial-fixture regeneration (D-D38, reuses slice-03 D-D15)**: the tampered/
  CID-mismatch/unsigned ingest fixtures + the real-`z6Mk...` DID-doc fixture are
  regenerated via an `xtask` helper against the live `org.openlore.claim` Lexicon +
  the `decode_ed25519_multibase` contract (e.g. `cargo xtask regenerate-ingest-fixtures`,
  extending the slice-03 `regenerate-peer-fixtures`); an `arch-check`-stage `--check`
  run fails if the committed fixtures drift. DELIVER may defer the regenerator (the
  fixtures work without it; it just risks drift — the slice-03 D-D15 escape hatch).

### 3.4 Public-endpoint allowlist (B2; mirrors slice-02 D-D22)

Slice-02's `contract-pact-github` established a public-endpoint allowlist (the only
external hosts the contract test may contact in the real-provider release variant).
Slice-05 EXTENDS the allowlist with the slice-05 external hosts (D-D39):

| Host | Path scope | Used for | Real-variant gating |
|---|---|---|---|
| `bsky.social` (+ the existing PDS allowlist from slice-01/03) | `com.atproto.repo.listRecords`, `com.atproto.repo.getRecord`, `com.atproto.identity.resolveHandle` | network-author record enumeration (reuses slice-03) | release-tag, manual approval (D-D12) |
| `plc.directory` (NEW) | `GET /<did>` (DID-document resolution) | the ADR-026 production PLC `z6Mk...` pubkey resolution | release-tag, manual approval (D-D12); the NEW slice-05 host |

The allowlist is the ONLY set of external hosts the real-provider contract variant
may contact; any other host is a contract-test failure (the slice-02 D-D22 discipline,
extended). In PR/nightly NO external host is contacted (recorded fixtures only). The
indexer's RUNTIME ingest (not the contract test) contacts whatever sources its
`config.toml` seeds + relay configure — that is operator config (`platform-design.md`
§3.3), distinct from the contract-test allowlist (which governs only what the CI
test may reach).

## 4. The local-first guardrail confirmation (KPI-5) — the contract NEGATIVE

Per DESIGN §6.4 + the DEVOPS handoff: confirm the local-first guarantee at the
contract level. There is NO contract test for a CLI→indexer dependency in the
local-first flows BECAUSE the CLI's compose/sign/local-query path links NO indexer
code and adds NO network dependency (the `xtask check-arch` CLI-dep-graph exclusion,
component-boundaries §`cli`). `search` is the ONLY network verb and degrades
gracefully. The `at-local-first-preserved` release gate (KPI-5; `ci-cd-pipeline.md`
delta §3) is the BEHAVIORAL confirmation: offline compose/sign/graph-query succeed
with the indexer down. This is the "contract" that the network boundary is
NON-LOAD-BEARING for authoring — a structural + behavioral guarantee, not a Pact.

## 5. Summary — the two suites + their gating

| Suite | Boundary | Consumer | Provider | PR/nightly | Release-tag | Allowlist |
|---|---|---|---|---|---|---|
| `contract-pact-indexer-query` | B1 (CLI↔indexer) | `openlore` CLI (`adapter-index-query`) | the indexer (`adapter-xrpc-query-server`) | MOCKED + in-process provider verify (< 30 s) | re-verify vs a real localhost `openlore-indexer serve` (~30 s; no third party) | none (localhost own-binary) |
| `contract-pact-pds-network` | B2 (indexer→PDS/PLC) | the indexer (`adapter-atproto-ingest` + `adapter-atproto-did`) | network-author PDSes + `plc.directory` | RECORDED fixtures (< 30 s) | re-verify vs real `bsky.social` + real `plc.directory` (manual approval, D-D12) | `bsky.social` + `plc.directory` (D-D39) |

Both are blocking-on-PR (mocked/recorded) and gate release. Aggregate added
wall-clock: **< 1 min per PR** (both parallelize within the acceptance stage; both
hermetic in PR). Release-tag adds the manual-approval real-provider re-verification
(~2-3 min, reusing the slice-03 approval gate + the new PLC host).

## 6. References

- `platform-design.md` (sibling) §2, §8 (the two boundaries as risks)
- `ci-cd-pipeline.md` (sibling) §3 (the two Pact sub-jobs), §7 (the adversarial-fixture regenerator)
- `observability.md` (sibling) §2.6 (the `indexer.ingest.rejected{reason}` events the contract's adversarial set drives), §2.8 (the ingest/identity probes)
- `wave-decisions.md` (sibling) — D-D36, D-D37, D-D38, D-D39
- `docs/feature/openlore-appview-search/design/architecture-design.md` §6.2 (the two boundaries), §6.4 (the contract-test recommendation), §6.3 (the probe contracts)
- `docs/feature/openlore-appview-search/design/component-boundaries.md` (the `adapter-atproto-ingest` / `adapter-atproto-did` / `adapter-xrpc-query-server` / `adapter-index-query` contracts; the DEVOPS annotation)
- `docs/feature/openlore-appview-search/design/data-models.md` (the XRPC query DTOs; the `listRecords` + PLC DID-doc shapes)
- slice-03 `ci-cd-pipeline.md` §3.6 (`contract-pact-pds-peer`), slice-03 `wave-decisions.md` D-D15; slice-02 `contract-pact-github` D-D22/D-D24 — the patterns this extends
- ADR-024 (pull ingestion / the network-lies fixtures), ADR-026 (the PLC `z6Mk...` decode), ADR-027 (the CLI↔indexer XRPC contract)
