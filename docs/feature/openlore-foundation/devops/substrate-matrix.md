# Substrate Gold-Test Matrix — openlore-foundation (slice-01)

- **Wave**: DEVOPS
- **Date**: 2026-05-25
- **Architect**: Apex
- **Inherits from**: ADR-009 §Architecture Enforcement layer (c) (behavioral probe check); Morgan's architecture-design.md §7 ("gold-test substrate matrix")

The substrate matrix is the behavioral leg of ADR-009's three-layer probe
enforcement. Where ADR-009 layer (a) is the compiler and layer (b) is the
AST walker, **the matrix exercises every adapter `probe()` against real
catalogued substrate-lie scenarios** — fsync on tmpfs, DrvFs paths, TLS
misconfiguration, etc.

A `probe()` that passes only on native-FS macOS is a CI failure.

## 1. Substrate axes

Per task spec §Phase 5, the axes are:

| Axis | Values for slice-01 |
|---|---|
| OS | `macos-latest`, `ubuntu-latest` |
| Arch | `x86_64`, `aarch64` |
| Rust toolchain | `MSRV` (the rust-toolchain.toml-pinned version), `stable` |
| DuckDB | the version pinned by `Cargo.lock` (single value — DuckDB upgrade is a deliberate change, exercised in nightly with a "next version" pre-flight) |
| ATProto PDS implementation | `bsky.social` (reference impl) AND `wiremock`-stubbed (deterministic fixture) |

Slice-01 gating cardinality = 2 OS × 2 arch × 2 Rust × 1 DuckDB × 1 PDS-fixture
= **8 cells** for release-tag gating. PDS-real (`bsky.social`) is run once
at release time, NOT per-cell (rate limits).

## 2. The 8-cell release matrix

| Cell | OS | Arch | Rust | DuckDB | Notes |
|---|---|---|---|---|---|
| C1 | macos-latest | aarch64 | MSRV | pinned | Apple Silicon, MSRV |
| C2 | macos-latest | aarch64 | stable | pinned | Apple Silicon, stable |
| C3 | macos-latest | x86_64 | MSRV | pinned | Intel Mac, MSRV |
| C4 | macos-latest | x86_64 | stable | pinned | Intel Mac, stable |
| C5 | ubuntu-latest | x86_64 | MSRV | pinned | mainstream Linux, MSRV |
| C6 | ubuntu-latest | x86_64 | stable | pinned | mainstream Linux, stable |
| C7 | ubuntu-22.04-arm | aarch64 | MSRV | pinned | ARM Linux, MSRV |
| C8 | ubuntu-22.04-arm | aarch64 | stable | pinned | ARM Linux, stable |

GitHub Actions matrix expression (illustrative for DELIVER; not the YAML):

```
strategy:
  fail-fast: false
  matrix:
    include:
      - os: macos-latest    arch: aarch64  rust: <MSRV>
      - os: macos-latest    arch: aarch64  rust: stable
      - os: macos-13        arch: x86_64   rust: <MSRV>
      - os: macos-13        arch: x86_64   rust: stable
      - os: ubuntu-latest   arch: x86_64   rust: <MSRV>
      - os: ubuntu-latest   arch: x86_64   rust: stable
      - os: ubuntu-22.04-arm arch: aarch64 rust: <MSRV>
      - os: ubuntu-22.04-arm arch: aarch64 rust: stable
```

(Note: `macos-13` runner provides x86_64 since `macos-latest` is
aarch64-only post-2025. `ubuntu-22.04-arm` is GitHub's ARM Linux runner.
DELIVER may adjust runner labels if GitHub updates them.)

## 3. The 4-cell per-PR subset

To keep PR wall-clock fast (per `ci-cd-pipeline.md` §3.8 target ~5 min for
commit stage, ~15 min including acceptance), the per-PR run uses:

| Cell | Rationale for inclusion in PR |
|---|---|
| C4 (macos-latest x86_64 stable) | Most-likely dev laptop config |
| C6 (ubuntu-latest x86_64 stable) | Most-likely Linux user config |
| C2 (macos-latest aarch64 stable) | Apple Silicon coverage |
| C5 (ubuntu-latest x86_64 MSRV) | MSRV regression detection (one MSRV cell sufficient on PR) |

The remaining 4 cells run nightly (per `ci-cd-pipeline.md` §6) and on every
release tag.

## 4. Substrate-lie scenarios catalogued

Per ADR-009 §Architecture Enforcement layer (c), every adapter's `probe()`
MUST exercise at least one of these. The catalogue:

### 4.1 Storage substrate lies (probed by `adapter-duckdb`)

| Scenario | Lie | Probe must detect |
|---|---|---|
| `fsync` on tmpfs | tmpfs returns success from fsync but doesn't persist on power loss | Probe writes a sentinel, fsyncs, and... well, we can't simulate power loss in CI. Instead: probe DETECTS tmpfs by checking the filesystem type at the storage path (`statfs` on Linux, `getmntinfo` on macOS) and emits a WARNING via `tracing::warn!("storage.fsync.tmpfs_detected", path)`. The probe does NOT refuse — tmpfs may be intentional for CI; instead the warning surfaces it. |
| `fsync` on overlayfs (Docker overlay2) | overlayfs can lie about durability | Detect overlayfs via mount type; emit `tracing::warn!("storage.fsync.overlayfs_detected", path)` |
| WSL2 DrvFs (`/mnt/c/...`) | DrvFs has known fsync semantics issues | Probe detects DrvFs by path-prefix heuristic + `statfs` magic; REFUSES to start with `health.startup.refused{reason: StorageFsyncUnreliable, detail: "WSL2 DrvFs detected at <path>; move OpenLore data to ~/.local/share/openlore on the Linux fs"}` |
| Read-after-write inconsistency | filesystem returns stale data after fsync | Probe writes sentinel, fsyncs, opens a fresh file handle, reads sentinel back, asserts equality. On mismatch: REFUSE. |
| Schema-version mismatch | binary is older than DB schema | Probe queries `schema_version` table; if max version > binary's known max, REFUSE with `StorageSchemaMismatch`. |

CI matrix coverage:
- Native FS on macOS (C1-C4): exercises path heuristics return "ok".
- Native FS on Ubuntu (C5-C8): same.
- **Additional dedicated job**: a `substrate-tmpfs-linux` job runs on Ubuntu cells C5+C6, mounts a tmpfs at the storage path, asserts the WARNING fires.
- **Additional dedicated job**: a `substrate-overlayfs-linux` job runs on Ubuntu cells C5+C6 inside a Docker container, asserts the WARNING fires.
- WSL2 DrvFs: NO CI runner available (no WSL2 runner). Coverage is via unit test that mocks `statfs` to return DrvFs magic; asserts REFUSE.

### 4.2 Identity substrate lies (probed by `adapter-atproto-did`)

| Scenario | Lie | Probe must detect |
|---|---|---|
| Keychain returns success but stores nothing (Linux Secret Service not running) | observed in headless CI envs | Probe writes a sentinel key, reads it back, asserts equality. On mismatch: REFUSE with `IdentityKeychainUnreachable`. |
| WSL2 fallback file with perms != 0600 | file readable by other users on shared host | Probe checks `stat` mode; if not exactly `0600`: REFUSE with `IdentityKeyPermsUnsafe`. |
| DID document missing the OpenLore verification method | the user re-ran `openlore init` but the PDS update silently failed | Probe resolves DID doc, walks `verificationMethod` array, asserts the OpenLore key fragment is present and matches the local key. On mismatch: REFUSE with `IdentityDidDocumentMismatch`. |

CI matrix coverage:
- macOS Keychain (C1-C4): real Keychain on the runner (CI uses a temporary keychain unlocked via `security` command — DELIVER scripts this).
- Linux Secret Service (C5-C8): runner needs `gnome-keyring-daemon` started; DELIVER scripts. On runners where Secret Service is not available, the WSL2-fallback codepath (file) is exercised instead, with explicit perms assertions.

### 4.3 PDS substrate lies (probed by `adapter-atproto-pds`)

| Scenario | Lie | Probe must detect |
|---|---|---|
| TLS handshake against the configured PDS fails | certificate expired, hostname mismatch, intermediate CA missing | Probe initiates a TLS connection; on failure: REFUSE with `PdsTlsHandshakeFailed{detail}`. |
| `describeServer.did` returns a DID the user did NOT configure for | user pointed at wrong PDS, or PDS rebrand | Probe calls `com.atproto.server.describeServer`, asserts `did` equals user's configured PDS DID. On mismatch: REFUSE with `PdsDidMismatch`. |
| PDS silently overwrites on rkey collision | violates ATProto spec; some self-hosted PDS impls reportedly do this | Probe writes a sentinel record at `org.openlore.diagnostic` collection with a fixed rkey, fetches it, writes a DIFFERENT body at the same rkey, fetches again. If the second fetch returns the SECOND body (overwrite occurred): REFUSE with `PdsIdempotencyViolation`. If the second write returned 409/conflict: probe passes (idempotency honored). |

CI matrix coverage:
- `wiremock`-stubbed PDS (every cell C1-C8): exercises happy path + the rkey-collision detection assertion against a stub that intentionally overwrites — asserts the probe correctly REFUSES.
- `bsky.social` real PDS (release-tag only, once per release, not per-cell): asserts probe passes against the reference implementation.

### 4.4 Lexicon substrate lies (probed by `lexicon` module)

| Scenario | Lie | Probe must detect |
|---|---|---|
| Lexicon JSON file edited locally to invalid schema | user fiddled or merge conflict | Validate every `lexicons/org/openlore/*.json` at module-load; on schema-of-schemas validation failure: REFUSE with `LexiconInvalid`. |
| serde round-trip not byte-equal | indicates a serde-derive bug or hand-rolled deviation | Probe loads each Lexicon JSON, deserializes via serde, re-serializes, asserts byte-equality. On mismatch: REFUSE with `LexiconSerdeRoundTripFailed`. |

CI matrix coverage:
- All 8 cells (this is a pure-Rust probe; substrate-independent in practice; but running on all cells is cheap).

## 5. Gating policy

### Release tag (push: `v*`)

ALL 8 cells must pass. ANY cell failure blocks the release.

The release workflow's `release-gate` job has `needs: [substrate-matrix-all-8]`. The matrix job uses `fail-fast: false` so partial failures are visible (rather than canceling sibling cells the moment one fails — diagnostic value).

### Per-PR

4-cell subset (C2, C4, C5, C6) must pass. PR cannot merge with a red cell.

### Nightly

All 8 cells PLUS the `substrate-tmpfs-linux` and `substrate-overlayfs-linux` dedicated jobs. Failures open a GitHub issue auto-labeled `substrate-regression` but do NOT page anyone (solo dev; user reads the issue tracker).

### Manual (`workflow_dispatch`)

Developer can trigger the full 8-cell matrix from any branch via `gh workflow run substrate.yml --ref my-branch`. Useful for "I changed an adapter; let me see if it still passes the full matrix before opening a PR".

## 6. Out-of-matrix concerns explicitly deferred

- **Windows**: zero cells. Out-of-scope per `distribution.md`.
- **musl Linux** (Alpine, etc.): zero cells. Deferred per `ci-cd-pipeline.md` §7.2.
- **NetBSD/FreeBSD/OpenBSD**: zero cells. No keyring backend support; deferred indefinitely.
- **WSL2**: zero CI cells (no GitHub runner); handled by unit-test mock per §4.1. Real-WSL2 testing happens manually by the user if they're on Windows; that user is currently out-of-scope per the previous bullet.
- **DuckDB next version (forward-compat probe)**: a single cell in the nightly workflow runs against `DuckDB next` (the `main` branch of the Rust client) with `continue-on-error: true`. Provides early warning of breaking changes; never blocks merges or releases.

## 7. Why not a bigger matrix?

Considered and rejected:

| Expansion | Rejected because |
|---|---|
| Adding `windows-latest` | Out-of-scope; Windows users do not exist for slice-01 per persona definition |
| Adding `proton-pds` / `roomy.host` / other PDS impls | None has a stable URL we can rely on yet; revisit at slice-03 when federation introduces real cross-PDS testing |
| Adding multiple DuckDB versions (1.0, 1.1, 1.2) | DuckDB upgrade is a deliberate change, not a per-PR variation. The "next version" probe in §6 covers forward-warning. |
| Adding glibc-version axis (2.31, 2.35, 2.39) | Static-linking decision (in `distribution.md`) for the release binaries reduces this risk to the build-time runner only; PR matrix doesn't need to vary |

## 8. References

- `platform-design.md`
- `ci-cd-pipeline.md` §6 (mutation), §7.1 (release gates), §9 (gate-enforcement summary)
- `distribution.md`
- Morgan: `architecture-design.md` §7 (CI/CD expectations), §9 (Earned Trust summary table); ADR-009 §Architecture Enforcement layer (c)
- `component-boundaries.md` — per-adapter probe-responsibility tables (the spec the gold tests verify)
