# Distribution and Runtime Contract — openlore-foundation (slice-01)

- **Wave**: DEVOPS
- **Date**: 2026-05-25
- **Architect**: Apex
- **Related**: proposed ADR-011 (release matrix and channels), ADR-012 (supply-chain policy)

## 1. Distribution channels

### 1.1 Primary: `cargo install openlore`

- Source: **crates.io** (`https://crates.io/crates/openlore`).
- The published crate is the `cli` workspace member only; workspace
  internal crates (`claim-domain`, `lexicon`, `ports`, `adapter-*`) have
  `publish = false` in their `Cargo.toml`.
- Audience: Rust users who already have a toolchain. Compiles from source
  against their Rust toolchain → naturally yields a binary correct for
  their platform.
- Pros: simplest install for the persona (P-001 = senior engineer, likely
  has Rust installed); doesn't require us to operate a CDN.
- Cons: compile time (~5-10 min on first install for a workspace this size).

### 1.2 Secondary: GitHub Releases binaries

- Source: GitHub Releases page for the repo, populated by `release.yml`
  workflow on `v*` tags (per `ci-cd-pipeline.md` §7).
- Audience: users who don't want to wait for cargo to compile, or who
  don't have a Rust toolchain at all.
- Artifacts per release (4 binaries):
  - `openlore-{version}-aarch64-apple-darwin.tar.gz`
  - `openlore-{version}-x86_64-apple-darwin.tar.gz`
  - `openlore-{version}-x86_64-unknown-linux-gnu.tar.gz`
  - `openlore-{version}-aarch64-unknown-linux-gnu.tar.gz`
- Companion files per binary:
  - `.sha256` — hash of the tarball.
  - `.sig` — sigstore/cosign signature of the tarball (proposed ADR-012).
- Release-wide companion files:
  - `sbom.cdx.json` — CycloneDX SBOM (proposed ADR-012).
  - `CHANGELOG.md` excerpt for this version.
  - `provenance.intoto.jsonl` — SLSA build provenance, if reachable via GitHub OIDC and the `sigstore/cosign-installer` + `actions/attest-build-provenance` actions (SLSA L3 target; L2 minimum acceptable for slice-01).

### 1.3 Tertiary (future, NOT implemented in slice-01)

| Channel | Status | Trigger to implement |
|---|---|---|
| Homebrew tap | ~~Reserved~~ → **Accepted (ADR-061, 2026-07-12)** | Post-slice-05, when there are enough non-Rust users to justify the maintenance — **trigger fired; see ADR-061 + `docs/feature/homebrew-binary-distribution/`. Note: gated on the not-yet-existing `release.yml` prerequisite** |
| AUR package | Reserved | Community submission acceptable; maintainership remains community's |
| Nix flake | Reserved | Post-slice-03 if a user contributes one (Nix users tend to self-serve) |
| `cargo binstall` support | Easy add | Acceptable to do in slice-01 — add the required `[package.metadata.binstall]` block to `Cargo.toml`. DELIVER's call. |

## 2. Runtime contract — paths

OpenLore honors **XDG Base Directory Specification** via the `directories`
crate (already in `technology-stack.md`). DELIVER MUST use `directories::ProjectDirs::from("org", "openlore", "openlore")` and consume only the paths it returns; no hand-rolled `$HOME/.openlore` style lookups.

### 2.1 Config

Resolved via `ProjectDirs::config_dir()`:

| Platform | Path |
|---|---|
| Linux | `~/.config/openlore/` |
| macOS | `~/Library/Application Support/org.openlore.openlore/` (XDG-honoring on macOS varies — see §2.5) |
| Windows (deferred) | `%APPDATA%\openlore\openlore\config\` |

Files:
- `config.toml` — top-level config (`[telemetry]` section per `observability.md` §6.1; future `[pds]` and `[author]` sections per `data-models.md`).
- `identity.toml` — per Morgan's architecture-design.md §7 deployment diagram. Holds the DID, the PDS endpoint URL, and the keychain key fragment id. NOT the key itself (the key is in the OS keychain).

### 2.2 Data

Resolved via `ProjectDirs::data_dir()`:

| Platform | Path |
|---|---|
| Linux | `~/.local/share/openlore/` |
| macOS | `~/Library/Application Support/org.openlore.openlore/` (data + config share the dir on macOS; subdirs disambiguate) |
| Windows (deferred) | `%APPDATA%\openlore\openlore\data\` |

Files / subdirs:
- `openlore.duckdb` — the embedded DB (per `data-models.md` §DuckDB schema).
- `claims/<cid>.json` — per-claim signed artifacts (per `data-models.md` §On-disk artifact format).
- `surveys/` — KPI-3 and KPI-6 survey responses (per `kpi-instrumentation.md` §4 + §7).
- `telemetry-buffer/` — opt-in telemetry buffer (per `observability.md` §6).

### 2.3 Cache

Resolved via `ProjectDirs::cache_dir()`. Slice-01 has no caching needs but
the dir is reserved.

### 2.4 Logs

Convention question: XDG does not specify a logs dir. We use
`$XDG_STATE_HOME/openlore/logs/` if `XDG_STATE_HOME` is set, else
`$XDG_DATA_HOME/openlore/logs/` (per `observability.md` §3.3). On macOS,
`~/Library/Logs/org.openlore.openlore/`. DELIVER resolves via
`directories::ProjectDirs::data_local_dir()` with subdir `logs/`.

### 2.5 macOS XDG note

`directories` crate on macOS by default uses the macOS-native dirs
(`~/Library/Application Support/...`) NOT the XDG paths. Users coming from
Linux may have `XDG_CONFIG_HOME=~/.config` set explicitly; the
`directories` crate does NOT honor that on macOS.

**Decision**: accept the `directories` crate's default behavior. The user
who really wants XDG on macOS can symlink. Document the actual paths in
the `--help` output and in the project README.

### 2.6 Windows fallback (deferred)

Per task spec: document but mark out-of-scope. The `directories` crate
handles Windows path resolution correctly if Windows is ever a target.
Until then: the binary fails fast at startup on Windows with `health.startup.refused{reason: StorageFsyncUnreliable, detail: "Windows is not yet a supported platform for slice-01"}` — implemented by an explicit `#[cfg(windows)] panic!(...)` or equivalent guard in the composition root.

## 3. Versioning

- **Semantic Versioning** (`MAJOR.MINOR.PATCH`).
- Slice-01 releases as `0.1.x`. The `0.x` MAJOR communicates "API is not
  stable" — appropriate while sibling slices are still being designed.
- Git tags: `v0.1.0`, `v0.1.1`, etc.
- Bump policy:
  - PATCH for bug fixes, doc, internal refactors.
  - MINOR for new user-visible features or new CLI verbs.
  - MAJOR (when we get to `1.0.0`) for breaking changes to: the Lexicon
    field shape, the signed-claim CID computation (per ADR-006 — CID
    changes ARE wire-breaks), the CLI verb contract (per ADR-003).

The CID is the wire-stable identifier. Any change that alters the CID of
a previously-published claim is a MAJOR break. The pure-core property
tests + gold fixtures (per ADR-006 Earned Trust) prevent accidental MAJOR
breaks.

## 4. Release workflow narrative

Per `ci-cd-pipeline.md` §7. Summary in user-facing terms:

1. Developer creates a release branch `release/v0.1.x` (optional — solo
   dev may tag directly on `main` if no parallel work is pending).
2. Developer updates `CHANGELOG.md` and bumps version in workspace
   `Cargo.toml`.
3. Developer creates and pushes tag: `git tag v0.1.0 -s && git push origin v0.1.0`. (`-s` for signed tag per `cicd-and-deployment` §Branch Protection Rules.)
4. `.github/workflows/release.yml` triggers:
   a. Re-runs commit + acceptance stages on the tagged ref.
   b. Runs full mutation test sweep.
   c. Runs full 8-cell substrate matrix.
   d. Runs Pact contracts against real `bsky.social` (manual approval gate
      — solo dev clicks Approve in the Actions UI).
   e. Builds the 4-platform binary matrix.
   f. Signs each binary with cosign (proposed ADR-012).
   g. Generates SBOM via `cargo cyclonedx` or `syft`.
   h. Uploads tarballs, sigs, sums, SBOM, CHANGELOG excerpt to GitHub
      Releases.
   i. Runs `cargo publish --package openlore` to push to crates.io.
5. Developer announces release (out-of-pipeline).

## 5. Install paths for the user

### macOS Apple Silicon
```
$ cargo install openlore             # from crates.io
OR
$ curl -L https://github.com/.../releases/download/v0.1.0/openlore-0.1.0-aarch64-apple-darwin.tar.gz | tar xz
$ mv openlore /usr/local/bin/        # or any $PATH dir
$ openlore --version
openlore 0.1.0
$ openlore init                      # first-run wires identity + storage
```

### Linux x86_64
```
$ cargo install openlore
OR
$ curl -L https://github.com/.../releases/download/v0.1.0/openlore-0.1.0-x86_64-unknown-linux-gnu.tar.gz | tar xz
$ mv openlore ~/.local/bin/
$ openlore init
```

### Verifying a binary

```
$ cosign verify-blob \
    --certificate openlore-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sig.cert \
    --signature openlore-0.1.0-x86_64-unknown-linux-gnu.tar.gz.sig \
    openlore-0.1.0-x86_64-unknown-linux-gnu.tar.gz
```

Documented in the GitHub Releases description for each release.

## 6. Update strategy

Per Morgan architecture-design.md §7: the user re-runs `cargo install`
(which respects `--force`) OR downloads a fresh binary. There is no
auto-updater; OpenLore does NOT phone home to check for updates.

The embedded DuckDB schema migration is forward-only and idempotent (per
`data-models.md` §Migration policy + ADR-001). On startup with a NEWER
schema in the DB file than the binary knows about, the storage adapter
probe REFUSES (`StorageSchemaMismatch`) — telling the user to upgrade
the binary, not silently corrupt their data.

## 7. Uninstall

The user removes:
1. The binary: `rm $(which openlore)` (or `cargo uninstall openlore`).
2. The data: `rm -rf $XDG_DATA_HOME/openlore` (or the platform-specific
   equivalent — `openlore --help` documents the path).
3. The config: `rm -rf $XDG_CONFIG_HOME/openlore`.
4. The OS-keychain entry: `openlore identity remove-key` (a verb that
   DELIVER ships; flagged in `wave-decisions.md` D-D5 because it's not in
   Morgan's cli component diagram but is required for a clean uninstall).

The PDS records remain on the user's PDS (out of our control; the user
can `bsky` or equivalent to clean them up — or leave them as auditable
history per ADR-008).

## 8. References

- `platform-design.md`
- `ci-cd-pipeline.md` §7 (release workflow)
- `substrate-matrix.md` (the 8-cell gate that gates the release)
- Morgan: `architecture-design.md` §7 (deployment architecture)
- Proposed ADR-011 (release matrix and channels), ADR-012 (supply-chain policy)
