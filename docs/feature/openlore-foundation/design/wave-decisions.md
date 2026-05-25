# Wave Decisions — DESIGN — openlore-foundation

- **Wave**: DESIGN
- **Date**: 2026-05-25
- **Architect**: Morgan (nw-solution-architect)
- **Feature**: openlore-foundation (slice-01 walking skeleton)
- **Inherits**: WD-1..WD-13 from DISCUSS (`docs/feature/openlore-foundation/feature-delta.md`)

This file mirrors the DISCUSS-wave decisions style. Each row records a
DESIGN-wave decision (D-prefix), its rationale, and its status. Decisions
that point at an ADR are binding for DELIVER unless explicitly re-opened.

## Locked decisions

| # | Decision | Rationale | Status | ADR / Document |
|---|---|---|---|---|
| D-1 | Architecture style = Hexagonal (Ports + Adapters) + Modular Monolith, single Rust workspace, single binary. | Quality-attribute drivers (claim integrity, local-first, testability, federation interop) all point at port/adapter isolation with a pure core. Conway's Law: solo dev = single deployable. Microservices rejected. | LOCKED | ADR-009 |
| D-2 | Local storage = DuckDB via the official `duckdb` Rust crate. | WD-8 from DISCUSS. Re-evaluated in slice-04 for graph workloads. | LOCKED (slice-01) | ADR-001 |
| D-3 | Identity = existing ATProto DID + per-application Ed25519 derived key registered as a separate verification method on the user's DID document. | WD-12 from DISCUSS. Independent revocability; no fresh DID minted; preserves social-graph identity. | LOCKED | ADR-002 |
| D-4 | CLI verb contract = two prompts (Enter to sign, Y to publish), two verbs (`claim add`, `claim publish`), one combined session for ergonomics. Internal code reuse permitted; observable two-prompt contract is fixed. | Locked CLI shape from `alternatives-considered.md` Choice 3; residual tension resolved here. | LOCKED | ADR-003 |
| D-5 | Async runtime = `tokio` (current-thread flavor for the CLI); TLS = `rustls` with `webpki-roots`. | `atrium-api` requires tokio; rustls avoids OpenSSL system dep; portable static binary. | LOCKED | ADR-004 |
| D-6 | Lexicon namespace = `org.openlore.*`. slice-01 ships `org.openlore.claim` and `org.openlore.philosophy`; reservations documented for sibling features. | Reverse-DNS ATProto convention; user-proposed namespace honored; namespace evolution policy = backward-compatible field changes only. | LOCKED | ADR-005 |
| D-7 | Claim addressing = IPLD CIDv1 with codec `dag-cbor` (0x71), hash `sha2-256` (0x12), encoding `base32` lowercase. Canonical CBOR per RFC 8949 §4.2.1. Sign the unsigned-claim CID; final claim CID computed over the signed payload's canonical CBOR. | Wire-stable, language-agnostic, ATProto/IPLD-native, byte-deterministic across re-runs and machines. | LOCKED | ADR-006 |
| D-8 | Retraction model = counter-claim of type `retracts` referencing the original CID via the unified `references[]` Lexicon field. No hard-delete CLI verb. Correction and counter-claim share the same mechanism (different `type` values). | WD-11 from DISCUSS. One architectural mechanism for 4 reference types. | LOCKED | ADR-008 |
| D-9 | Composition root invariant = "wire then probe then use". Every adapter `probe()` MUST run at startup; on any refusal, the binary emits a structured `health.startup.refused` event and exits 2. | Earned Trust (principle 12); turns silent runtime failures into structured startup refusals. | LOCKED | ADR-009 |
| D-10 | Probe contract enforcement = three orthogonal layers — (a) subtype (trait method required), (b) structural (`xtask check-probes` AST walker over `impl Port for Adapter` blocks, plus `scripts/check-probes.sh` pre-commit), (c) behavioral (CI gold-test runner exercising catalogued substrate lies — fsync on tmpfs/overlayfs, TLS misconfiguration, PDS rkey-collision idempotency). | Per Earned Trust principle 12; a single-layer bypass is caught by at least one of the other two. `import-linter` rejected (Python-only and import-graph only). | LOCKED | ADR-009 |
| D-11 | Architecture enforcement tooling = `cargo-deny` (licenses + bans) + `cargo xtask check-arch` (custom dependency-graph parser over `cargo metadata`) + `cargo xtask check-probes` (AST walker via the `syn` crate). | Rust has no mature `import-linter` equivalent; `xtask`-based custom checks fill the gap; runs as first CI gate. | LOCKED | ADR-009 |
| D-12 | Confidence buckets are display-only and never persisted. The on-disk JSON, the DB rows, and the PDS record contain only the numeric `[0.0, 1.0]` value. A unit test asserts no `speculative|weighted|well-evidenced|triangulated` string appears in persisted confidence columns. | WD-10 from DISCUSS, codified into a data-model invariant. | LOCKED | data-models.md |

## Proposed (awaiting user confirmation)

| # | Decision | Reason it is proposed not locked | Default if no decision |
|---|---|---|---|
| D-13 | Development paradigm = functional-leaning Rust (pure core + effect shell); DELIVER routes to `@nw-functional-software-crafter`. | Paradigm choice was deferred to DESIGN by the orchestrator's brief; auto-mode default applied; appending to project `CLAUDE.md` is the heavier change that requires user permission. | Proceed as Proposed; DELIVER may pick either crafter and inherit the documented rationale. ADR-007 stays in "Proposed" until user confirms. The project `CLAUDE.md` is NOT modified yet. |

## Open questions (handed to DELIVER)

These are deliberately deferred to DELIVER (software-crafter) and are tracked
as crate-level concerns, not architectural decisions:

1. Exact PATCH-level crate versions and any transitive-conflict resolutions
   in the `atrium-api` / `reqwest` / `rustls` graph. (technology-stack.md)
2. Exact `ProbeOutcome` ADT shape and error enum field structures.
   (ports trait sketches in component-boundaries.md)
3. Exact `clap` subcommand grouping and flag naming beyond the locked verbs
   in ADR-003. (cli component section of architecture-design.md)
4. DuckDB migration tool choice (`refinery` vs `sqlx-migrate` vs hand-written
   SQL). (data-models.md)
5. Whether `did-key`, `did-plc-resolver`, or atrium's built-in DID resolution
   is sufficient for the identity adapter. (technology-stack.md identity table)
6. Exact `lexicons/org/openlore/*.json` field-order (display only; canonical
   CID form is CBOR per ADR-006). (data-models.md)

## Out of scope for this DESIGN (explicit deferrals)

- Sibling features (slices 02-05): scrapers, federated read, scoring,
  AppView. Each gets its own DISCUSS + DESIGN wave under its own feature
  directory. This DESIGN reserves `org.openlore.*` NSIDs for them
  informationally only (ADR-005 table).
- CLI verbs mentioned in `gherkin-scenarios-expanded.md` but not in
  slice-01 (`claim status`, `claim counter`, `graph contrib`, `--from-url`,
  `--corrects`/`--supersedes` flags). Flagged for DISTILL as
  `# DISTILL: confirm command name` comments in the scenarios; will resolve
  in the slice that introduces them.
- Stress / residuality analysis (no `--residuality` flag passed).
- Observability dashboards / aggregation (DEVOPS responsibility; slice-01
  ships local emission only).

## Handoff summary

| Recipient | Reads | Produces |
|---|---|---|
| DISTILL (acceptance-designer) | architecture-design.md (cli component, two-prompt contract); component-boundaries.md (DISTILL annotation); data-models.md; ADR-003 + ADR-008; this file. | Executable acceptance tests; resolve `# DISTILL:` flagged scenarios against the locked CLI verb shape. |
| DEVOPS (platform-architect) | architecture-design.md (deployment, integration, contract test annotation); component-boundaries.md (DEVOPS annotation); outcome-kpis.md (already DEVOPS's read); this file. | CI/CD pipeline; observability for `health.startup.refused`; Pact contract tests for ATProto PDS; substrate gold-test matrix. |
| DELIVER (software-crafter — functional or OOP) | every design artifact + every ADR; the open-questions list above. | Source code; the `xtask` checks; the probe implementations; the architecture-test crates. |

## Changelog

- 2026-05-25 — Morgan — initial DESIGN-wave decisions for slice-01. All
  decisions D-1..D-12 locked. D-13 (paradigm) Proposed; awaits user
  confirmation. CLAUDE.md not modified.
