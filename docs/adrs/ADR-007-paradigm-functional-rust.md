# ADR-007: Development Paradigm = Functional-Leaning Rust (Pure Core + Effect Shell)

- **Status**: Accepted (locked by user 2026-05-25; `## Development Paradigm` section appended to project CLAUDE.md the same day)
- **Date**: 2026-05-25
- **Deciders**: Morgan (nw-solution-architect) — recommendation; user to confirm
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

Rust is a multi-paradigm language: it supports both OOP-style designs (trait
objects, struct methods, dependency-inversion via `dyn Trait`) and FP-style
designs (pure functions over immutable values, ADTs via enums, composition
over inheritance, effect isolation at the edges).

The OpenLore domain has specific properties that bias the choice:

- **Claims are immutable values.** A signed claim is never mutated; a
  retraction is a NEW claim (per WD-11). This is exactly the shape FP excels at.
- **Canonicalization, signing, CID computation are pure transformations.**
  No I/O, no state, deterministic. Pure functions test trivially and
  property-test perfectly.
- **The "Not as truth" framing benefits from explicit pipeline visibility.**
  A reader of the code should be able to see "compose -> canonicalize -> sign
  -> CID -> persist -> publish" as a sequence of named transformations, not as
  methods scattered across classes.
- **I/O lives at the edges only.** Storage, identity, PDS, CLI prompt — all
  effects; the core is pure.

The DELIVER wave routes to `@nw-software-crafter` for OOP and
`@nw-functional-software-crafter` for FP. The choice locks DELIVER's crafter
assignment.

## Decision

**Use functional-leaning Rust: pure core + effect shell.**

- The `claim-domain` and `lexicon` modules are **pure**: no I/O, no global
  state, no `unsafe`, no interior mutability. Every public function is `fn
  (inputs) -> Result<outputs, DomainError>`.
- Adapters are **effect modules** at the hexagon edges. They implement port
  traits whose method signatures are pure-function-shaped (`fn(...) ->
  Result<T, E>`) but whose bodies perform I/O.
- The `cli` driver is the **composition root**: it wires concrete adapters
  into the pure core's data-flow pipeline.
- ADT discipline: prefer enums over trait-object hierarchies where the variant
  set is known and closed. `ClaimError`, `StorageError`, etc., are `thiserror`
  enums.
- Composition over inheritance (Rust has no inheritance anyway; this is mostly
  a reminder to avoid trait hierarchies emulating it).
- Side-effect honesty: any function that returns `impl Future` or takes a
  `&mut self` participates in the effect shell, not the pure core.

DELIVER wave crafter: `@nw-functional-software-crafter`.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **OOP-style trait-object ports + struct methods** | Idiomatic Rust and would work. Rejected because the domain is so naturally a pipeline of pure transformations that wrapping it in trait objects with `&self` adds ceremony without value. Also: trait objects have runtime overhead and obscure the call graph; static dispatch via generics is more honest about "this code is a pipeline." |
| **Mixed paradigm** (pure core but OOP-style adapter classes) | The Rust ecosystem doesn't have "classes" per se; "OOP-style" in Rust is structs + impl blocks, which is fine. The decision is mostly about the *shape* of how the core thinks (data + transformations vs entities + methods). Mixed is the default if no decision is made; we make an explicit choice to bias toward FP. |
| **Macro-DSL for the pipeline** | Overengineered for slice-01; reconsider only if the pipeline grows to dozens of stages. |

## Consequences

### Positive

- The pure core is trivially testable: every function is `fn(in) -> out`;
  no fixtures, no mocks, no setup. Property tests apply directly.
- Canonicalization/CID/signing are isolated from I/O, making it impossible to
  accidentally compute a CID against a database row instead of canonical CBOR.
- Mutation testing is more effective against pure code (no false positives
  from mutated branches that touch I/O paths only).
- The architectural enforcement rules (ADR-009) are simpler to express:
  "domain has zero imports from std::fs, std::net, std::time" is a one-liner.

### Negative

- Rust FP idioms (Result-chaining, monadic-style pipelines via `?`) are less
  familiar to some Rust developers than OOP idioms. **Mitigation**: solo
  developer is the only audience for slice-01; documentation in CLAUDE.md can
  set the paradigm expectation.
- Some adapters genuinely need state (e.g., a connection pool). The effect
  shell tolerates this; pure-core does not. The boundary must be clear.

### Earned Trust

The architecture enforcement rules (ADR-009) MUST include automated checks for
the pure-core invariant. Specifically, the `claim-domain` and `lexicon` crate
roots MUST be forbidden from depending on:

- `std::fs`, `std::net`, `std::process`, `std::time::SystemTime` (use a
  `Clock` port instead), `std::env`
- Any `tokio` or async-runtime crate
- Any I/O-performing crate (`reqwest`, `duckdb`, `keyring`)

Enforcement tool: **`cargo-deny`** (license + dependency rules) PLUS a custom
`xtask` script that parses `cargo metadata` and asserts `claim-domain` and
`lexicon` have only pure-crate transitive dependencies. Rust has no
import-linter equivalent as mature as Python's, so the `xtask` custom check is
the substitute. CI fails on violation.

## Revisit Trigger

- User declines this paradigm at confirmation — flip to ADR-007-alt (OOP
  variant) and re-route DELIVER to `@nw-software-crafter`.
- A real adoption pain in DELIVER (e.g., the FP idioms genuinely slowing
  shipping) reported in a retro.

## Open Question for User

This ADR is **Proposed**, not Accepted. The user must confirm before:

1. The ADR status flips to Accepted.
2. The project `CLAUDE.md` is appended with the "Development Paradigm" section.
3. DELIVER auto-routes to `@nw-functional-software-crafter`.

Default if no confirmation: **proceed as Proposed**; DELIVER may pick either
crafter and adopt the rationale documented here.
