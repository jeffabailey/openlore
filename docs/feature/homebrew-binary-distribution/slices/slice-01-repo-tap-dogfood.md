# Slice 01 — In-repo tap formula: dogfood-install the current release on the maintainer's Mac

> Release 1 · Story: US-HB-001 (user-visible) · Job: J-006 · Persona: P-001 (Maria, Homebrew hat)
> Estimate: ~1 day

## Goal

The thinnest thread that delivers the whole install outcome: a hand-authored
`Formula/openlore.rb` in the in-repo tap (D-1) that installs the **prebuilt** `openlore` CLI
binary (D-2, D-3) from the latest published GitHub-Release tarball, sha256-verified by brew (D-4),
registering no service/updater (D-5) — proven end-to-end by `brew install`ing it on the
maintainer's own `aarch64-apple-darwin` Mac and seeing `openlore --version` print the released
version.

## Learning hypothesis

If the maintainer can `brew install` a prebuilt, in-repo tap formula on their own Mac in seconds
(no compile, no Rust), get a sha256-verified `openlore` on PATH, and confirm no service/updater was
added and nothing phones home — then the Homebrew channel honors every P-001 guardrail and the
prebuilt-tarball-reuse approach holds before automation depends on it. Settling **OD-HB-1** (does
the bare `brew install jeffabailey/openlore/openlore` resolve to the in-repo tap, or is a one-time
explicit-URL `brew tap` needed?) against a real `brew install` is the load-bearing learning.

## IN scope

- A new `Formula/openlore.rb` in the in-repo tap, structured as a single multi-platform formula:
  `on_macos`/`on_linux` + `on_arm`/`on_intel`, each branch with the matching release tarball `url`
  + its published `sha256` (OD-HB-3 default shape).
- `bin.install "openlore"` — installs the single extracted CLI binary only; never
  `openlore-indexer` (D-3).
- No `depends_on "rust"`/`"cargo"`, no build step — prebuilt only (D-2).
- Dogfood proof: `brew install` on the maintainer's `aarch64-apple-darwin` Mac; `openlore
  --version` prints the released version; `brew services list` / launchd / systemd show no openlore
  entry (D-5).
- Resolve OD-HB-1: document the exact working `brew install` (and one-time `brew tap …` if needed)
  in the README.

## OUT of scope

- The release-time auto-bump step and per-triple CI smoke test (→ slice 02).
- `openlore-indexer`, Windows, build-from-source, auto-updater / `brew services`, homebrew-core
  submission, cosign `.sig` in the formula, ghcr bottles (all OUT per feature-delta Out of Scope).

## Acceptance criteria (from US-HB-001 UAT)

- [ ] `brew install jeffabailey/openlore/openlore` (with the OD-HB-1 tap step if required) installs
      a working `openlore` on PATH on `aarch64-apple-darwin` with no Rust toolchain and no
      compilation; `openlore --version` prints the released version.
- [ ] The single formula selects the correct tarball for each of the 4 shipped triples via
      `on_macos`/`on_linux` + `on_arm`/`on_intel` (D-2).
- [ ] Only `openlore` is installed; `openlore-indexer` is never referenced (D-3).
- [ ] Each per-triple `sha256` equals the published `.sha256`; a mismatched download aborts the
      install with a checksum error and installs nothing (D-4).
- [ ] No brew service / launchd / systemd unit is registered; no phone-home; upgrade is only an
      explicit `brew upgrade` (D-5).
- [ ] The formula contains no `depends_on "rust"`/`"cargo"` and runs no build step (D-2).

## Dependencies

- A published `v*` GitHub Release with all 4 tarballs + `.sha256` companions (`release.yml`,
  ADR-011, `distribution.md` §1.2) — shipped.
- **OD-HB-1 (settle first)**: confirm whether the bare `brew install` resolves to the in-repo tap
  or needs a one-time `brew tap jeffabailey/openlore https://github.com/jeffabailey/openlore`
  (Risk R-2).

## Estimate

~1 day: the formula is small (4 url/sha256 pairs + `bin.install`); the dogfood install + guardrail
checks are quick; no application code changes.
