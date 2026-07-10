# Build optimization notes

Practical guidance for keeping local Rust builds fast in this workspace. The
dominant costs here are **debug info** and **link time**, not compilation of the
pure-core crates.

## What was changed

`Cargo.toml` now tunes the dev profile:

```toml
[profile.dev]
debug = "line-tables-only"     # workspace members

[profile.dev.package."*"]
debug = 2                      # dependencies unchanged
```

**Why.** The default dev profile emits full DWARF (`debug = 2`) for every crate.
That debug info is the biggest per-edit codegen + link cost, and it is paid on
the `openlore` binary *and* each of the **63 `cli` integration-test binaries**
(each statically links duckdb / tokio / atrium). `line-tables-only` keeps the
`file:line` in panic messages and backtraces — everything the TDD loop needs —
at a fraction of the DWARF size, so both codegen and link shrink on every edit to
a workspace crate.

**Scope.** The reduction is applied to workspace members only. Dependencies are
pinned at `debug = 2` (via `package."*"`, which matches all deps but no workspace
member) so the change does not permanently lower dependency debuggability.

**Measured.** A clean incremental relink of the `openlore` binary after editing a
`cli` leaf is ~8s. (The multi-minute compiles seen during development were mostly
*concurrent* cargo runs contending for CPU — see "Don't run builds in parallel"
below — compounded by full debug info.)

### One-time cost

Introducing or changing any `[profile.*]` section re-fingerprints the whole
graph, so the **first** build after this change is a full rebuild (all ~160
crates, including the duckdb C++). That cost has already been paid; subsequent
builds are incremental.

## Workflow tips (no config needed)

These avoid the biggest structural cost — linking 63 fat test binaries.

- **Scope test runs to one target.** `cargo test -p cli --test philosophy_show`
  builds and links *one* integration-test binary. Plain `cargo test -p cli`
  builds all **63**. Only widen when you need the full suite (e.g. pre-commit).
- **Test pure logic at the crate that owns it.** `cargo test -p lexicon` compiles
  no adapters and links no duckdb — sub-second after a warm build. Prefer it for
  domain-core work over an end-to-end acceptance target.
- **Editing a foundational crate cascades.** `lexicon` / `ports` are depended on
  widely, so touching them rebuilds most of the workspace. That is inherent to
  the dependency graph, not the profile. Batch such edits.
- **Don't run builds in parallel.** Two overlapping `cargo` invocations thrash a
  shared CPU and each runs far slower than one at a time. Let one finish.

## Optional further win (not applied)

Setting dependencies to no debug info gives lighter links across the bin + all 63
test binaries:

```toml
[profile.dev.package."*"]
debug = false
```

It is **not** applied here because it forces a one-time rebuild of the expensive
duckdb C++. Adopt it at a moment you are already doing a clean build; the ongoing
link-time win is real (dependency DWARF is the largest single input to each link).

## Deliberately not done

- **Alternate linker (lld / mold).** The stock macOS linker is already Apple's
  fast `ld` (build 1267 / "ld-prime"). lld on Mach-O is finicky and needs a
  `brew install llvm`, for near-zero upside over the default. Skipped.
- **Consolidating the 63 `[[test]]` targets.** A real structural lever, but it
  fights the per-slice acceptance-target convention and is invasive. Left as a
  future consideration.
