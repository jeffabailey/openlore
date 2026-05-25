# ADR-011: Release Matrix = 4-Platform Binary Set + crates.io; Windows Deferred

- **Status**: Accepted (locked by user 2026-05-25)
- **Date**: 2026-05-25
- **Deciders**: Apex (nw-platform-architect)
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

OpenLore ships as a CLI binary. The persona (P-001 senior engineer solo
builder) is a developer-class user: comfortable with `cargo install`,
comfortable downloading a tarball, comfortable verifying a signature.

Morgan's `architecture-design.md` §7 names a target platform set; this ADR
locks the specific matrix and the channels through which binaries are
distributed, plus the explicit deferral list.

## Decision

### Release matrix (per release tag)

**4 binary artifacts**, one per target triple:

| Target triple | OS | Arch | Linker | Use case |
|---|---|---|---|---|
| `aarch64-apple-darwin` | macOS | aarch64 | system | Apple Silicon |
| `x86_64-apple-darwin` | macOS | x86_64 | system | Intel Mac |
| `x86_64-unknown-linux-gnu` | Linux | x86_64 | glibc | mainstream x86 Linux |
| `aarch64-unknown-linux-gnu` | Linux | aarch64 | glibc | ARM Linux (RPi 4/5, Graviton) |

Each binary is built on its native runner (no cross-compile in slice-01).

### Channels

1. **`cargo install openlore`** — published to crates.io as the workspace's
   `cli` crate (other workspace crates have `publish = false`).
2. **GitHub Releases** — the 4 binaries above, each accompanied by
   `.sha256`, `.sig` (cosign), and a release-wide `sbom.cdx.json` and
   `provenance.intoto.jsonl` (per ADR-012).

### Deferred (NOT in slice-01)

| Item | Defer until |
|---|---|
| `x86_64-pc-windows-msvc` | >= 3 user requests on GitHub issues, OR slice-05 AppView introduces a need we can't avoid |
| `x86_64-unknown-linux-musl` | `keyring` crate's musl-static-link story improves OR WSL2 fallback file mechanism is determined insufficient for musl users |
| Homebrew tap | Post-slice-05 (non-Rust user base exists) |
| AUR / Nix flake | Community contribution acceptable; no commitment from us |
| `cargo binstall` metadata | DELIVER's call; zero-cost add but not blocking |
| Auto-updater | Out of scope for local-first ethos; user updates manually |

### Versioning

- Semantic Versioning (per `distribution.md` §3).
- `0.1.x` series for slice-01 (the `0.x` MAJOR communicates "API not
  stable" while sibling slices iterate the shape).
- `1.0.0` is reserved for the point where Lexicon shape, CID computation,
  and CLI verb contract are all promised stable.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Only `cargo install`, no binaries** | Excludes users without a Rust toolchain. Persona P-001 mostly has Rust but the binary download is the lowest-friction install for a brand-new tool. Reject. |
| **6-platform matrix (add musl + Windows)** | musl: keyring complications (see `ci-cd-pipeline.md` §7.2). Windows: zero users in scope per persona. Reject for slice-01; revisit triggers documented above. |
| **Cross-compile from one runner** | Faster CI but the substrate matrix loses platform coverage (e.g., macOS Keychain cannot be cross-compiled-tested from Linux). Reject — native build per target is worth the runner-minutes. |
| **Single "universal" binary for macOS (lipo)** | Possible, but doubles the macOS binary size for no install-time UX benefit (each user has one arch). Reject. |
| **Ship only via cargo, defer GitHub Releases entirely** | Misses the audience that downloads release tarballs (e.g., shell users, CI consumers). Reject. |

## Consequences

### Positive

- Covers >95% of the addressable persona (macOS + Linux developers).
- Two install paths (cargo + tarball) match the persona's habits.
- Clear deferral list documents the "we considered it, here's why not yet"
  position — future expansion is one ADR amendment per channel added.

### Negative

- No Windows support. Users who try it on Windows get a `health.startup.refused`
  at first run (per `distribution.md` §2.6). May generate GitHub issues —
  that's actually the desired signal mechanism for "should we add Windows".
- musl users get the same Windows-style refusal (WSL2 fallback file
  mechanism activates instead of Secret Service; perms-checked per
  ADR-002).

## Architecture Enforcement

- The release workflow (`release.yml`) is keyed to the 4-triple matrix
  explicitly. Adding a triple is a workflow-file change + a corresponding
  cell in `substrate-matrix.md`.
- The `Cargo.toml`'s `[package.metadata.binstall]` block (if DELIVER ships
  it) MUST point at the 4 triples and no others.
- A unit test in `cli` (cfg(windows)) refuses to compile or runs a panic
  with the documented "Windows not supported in slice-01" message. Prevents
  accidental "looks like it works" claims about Windows.

## Earned Trust

The `build-release` matrix in the release workflow is the proof; each
release ships exactly 4 verifiable binaries. Anyone can rebuild from the
tagged commit using the same toolchain pin and check that the binary
matches by hash (modulo build-time stamps which are pinned out via
`SOURCE_DATE_EPOCH` if DELIVER adopts reproducible builds — flagged for
future).

## Revisit Trigger

- >= 3 GitHub issues request Windows support: add `x86_64-pc-windows-msvc`.
- Reproducible-builds adoption: add `SOURCE_DATE_EPOCH` pin and document
  the rebuild procedure.
- crates.io changes its publish API in a breaking way: revisit (unlikely).
- A target triple's GitHub runner is deprecated: swap to the replacement
  runner; this ADR amended.
