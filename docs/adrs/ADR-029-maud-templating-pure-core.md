# ADR-029: `maud` as the Viewer Templating Engine + Pure-Core Allowlist Addition

- **Status**: Accepted (shipped — slice-06 htmx-scraper-viewer, DELIVER close 2026-05-31)
- **Date**: 2026-05-31
- **Deciders**: Morgan (nw-solution-architect), per OD-VIEW-1 for htmx-scraper-viewer (slice-06)
- **Feature**: htmx-scraper-viewer (slice-06)
- **Extends**: ADR-007 (pure core / effect shell), ADR-015 / WD-35 (the pure-core allowlist mechanism), the slice-02 precedent that whitelisted `serde_yaml_ng` for `scraper-domain` in `xtask check-arch::PURE_CORE_ALLOWED_CRATES`.
- **Resolves**: OD-VIEW-1 (templating / HTML rendering approach).

## Context

The viewer renders persisted claims, peer claims, claim detail, and live-scrape
candidates as HTML (FR-VIEW-2..5, FR-VIEW-7, FR-VIEW-8). Per ADR-028 the rendering
lives in the PURE `viewer-domain` crate (view-model → HTML, no I/O). DESIGN must choose
the HTML-rendering approach (OD-VIEW-1).

The forces:

- **Pure-core fit (ADR-007)**: rendering is a pure transformation `view-model → HTML
  string`. The renderer must do NO I/O (no filesystem template loading at runtime, no
  network), so it can live in the pure-core crate and be unit-tested with zero
  substrate (the slice-05 `appview-domain` precedent: pure functions, fixture inputs).
- **Type-checked correctness (NFR-VIEW-8 accessibility + FR-VIEW-8 confidence fidelity)**:
  the markup must be checked at compile time so a malformed tag or a misplaced field is
  a build error, not a runtime surprise; confidence must render as the stored numeric
  verbatim (FR-VIEW-8), so the renderer must interpolate values without silent
  reformatting.
- **Minimal, well-licensed dependency surface (open-source-first)**: the workspace
  prefers small, MIT/Apache pure crates; `deny.toml` bans `axum`/`actix-web` (heavy web
  frameworks). The templating choice must be a small pure dependency.
- **No runtime template files**: a runtime template directory would add a filesystem
  I/O dependency to the pure core (breaking ADR-007) and a deployment artifact to ship.

## Decision

**Use `maud` (a compile-time HTML macro) as the viewer's templating engine, in the PURE
`viewer-domain` crate. `maud` produces type-checked `Markup` at compile time with no
runtime I/O. Add `maud` to the `xtask check-arch` pure-core allowlist
(`PURE_CORE_ALLOWED_CRATES`) and register `viewer-domain` as a pure-core crate subject
to the I/O ban list — mirroring exactly how slice-02 whitelisted `serde_yaml_ng` for
`scraper-domain`.**

### Why `maud`

| Factor | `maud` (compile-time macro) — **CHOSEN** | `askama` (compile-time, file templates) | `tera` / `handlebars` (runtime templates) | hand-rolled `format!`/string concat |
|---|---|---|---|---|
| **Pure core fit (ADR-007)** | Markup is built in-code by a macro; no runtime template loading, no filesystem I/O. Lives cleanly in the pure `viewer-domain` crate. | Templates are external `.html` files compiled in via a build step, but the rendering call is pure; acceptable, but ships template files + a `build.rs` step. | Loads + parses templates at RUNTIME (filesystem I/O) — would force I/O into the pure core, breaking ADR-007. | Pure, but no structural safety (see below). |
| **Compile-time type check** | The `html! { }` macro is type-checked: a malformed tag or a bad interpolation is a compile error. Values are escaped by default; numeric confidence interpolates verbatim. | Compile-time checked against the template file. | Runtime-checked — template errors surface as runtime failures. | None — a missing `</td>` or an unescaped value is a silent bug. |
| **Dependency weight + license** | Single small crate, MIT, no async runtime, no I/O, mature + maintained. | Pulls a template-compiler build dependency. | Heavier; runtime parser; more transitive deps. | Zero deps, but the cost moves to fragile hand-written escaping. |
| **Accessibility discipline (NFR-VIEW-8)** | Semantic HTML is written directly in Rust with the structure visible at the call site; labeled inputs / table semantics are explicit and reviewable. | Same, in template files. | Same, in template files. | Easy to get wrong; no structure enforcement. |
| **Auto-escaping (display fidelity)** | Escapes interpolated strings by default (XSS-safe for the rendered subject/object/DID values); numeric values render verbatim — so confidence `0.90` is shown as the stored numeric (FR-VIEW-8) without reformatting. | Auto-escapes. | Auto-escapes. | Manual escaping — error-prone. |

`maud` is the only candidate that is BOTH compile-time-checked AND has zero runtime I/O,
which is exactly what lets the renderer live in the pure core. `askama` is close but
ships external template files + a build step for no slice-06 benefit; runtime engines
break the pure-core constraint outright.

### Pure-core allowlist addition (the WD-35 / ADR-015 mechanism)

`xtask check-arch` enforces a deny-by-ban-list on pure-core crates: a pure crate must
not transitively reach `tokio`/`reqwest`/`duckdb`/`keyring`/`atrium-*`. Non-I/O crates
are permitted by default, but each is recorded EXPLICITLY in `PURE_CORE_ALLOWED_CRATES`
as an audited adjudication (the slice-02 `serde_yaml_ng` precedent). This ADR:

1. Adds `"maud"` to `PURE_CORE_ALLOWED_CRATES` with the adjudication: *compile-time HTML
   macro; no I/O, no async runtime; MIT.*
2. Registers `viewer-domain` in `check_workspace` as a pure-core crate subject to the
   I/O ban list (a new `check_pure_core_no_io(workspace, "viewer-domain", ...)` arm),
   so a future contributor cannot sneak `duckdb`/`reqwest`/`tokio` into the renderer.

The effect crate `adapter-http-viewer` (which DOES carry hyper/tokio) is NOT pure-core
and is governed instead by the adapter invariants (no adapter depends on another
adapter; only a composition root links adapters).

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **`askama` (compile-time, external template files)** | Rejected (OD-VIEW-1). Compile-time-checked like maud, but ships external `.html` template files + a `build.rs` compile step — an extra build artifact + indirection for no slice-06 benefit. maud keeps the markup in-code, visible at the call site, with the same compile-time safety. |
| **`tera` / `handlebars` (runtime template engines)** | Hard reject (ADR-007). They load + parse templates at RUNTIME (filesystem I/O), which would force I/O into the pure `viewer-domain` core, breaking the pure/effect split, and would defer template errors to runtime. |
| **Hand-rolled `format!` / string concatenation** | Rejected. Zero deps but no structural safety: a malformed tag or an unescaped interpolated value (XSS on the DID/subject/object strings) is a silent bug, and confidence formatting fidelity (FR-VIEW-8) is unguarded. maud's auto-escaping + compile-time checking eliminate a class of rendering bugs for one small MIT dependency. |
| **`axum`/`actix-web` built-in view layers** | Hard reject. Both frameworks are banned by `deny.toml`; the HTTP layer is hyper-hand-rolled per ADR-028. Templating is independent of the HTTP framework regardless. |

## Consequences

### Positive

- The renderer is a pure, compile-time-checked transformation testable with zero
  substrate (fixture view-model in → asserted HTML out), exactly like the slice-05
  `appview-domain` renderer tests.
- One small MIT dependency; no runtime template files to ship; no `build.rs` step.
- Auto-escaping makes the rendered DID/subject/object/evidence strings XSS-safe by
  default while numeric confidence renders verbatim (FR-VIEW-8).
- `xtask check-arch` keeps `viewer-domain` honest: the I/O ban list applies, with `maud`
  the single audited exception — a future contributor cannot pull DuckDB/network into
  the renderer.

### Negative

- **maud markup is Rust-macro syntax, not HTML files** — a contributor unfamiliar with
  maud has a small learning curve. Accepted: the macro reads close to HTML and the
  compile-time errors are guiding; the slice-05 precedent shows pure renderers are
  maintainable.
- **A new entry in `PURE_CORE_ALLOWED_CRATES`** widens the pure-core allowlist by one.
  Accepted and audited here (MIT, no I/O); the explicit-allowlist mechanism is precisely
  the WD-35 control that makes this safe and reviewable.

## Revisit Trigger

- The viewer grows a need for runtime-pluggable themes/templates (operator-customizable
  HTML): would require a runtime template engine and moving rendering OUT of the pure
  core into the effect shell — a different ADR.
- A maud security advisory or maintenance lapse: re-adjudicate the allowlist entry and
  consider `askama`.
