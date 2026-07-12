# ADR-061: Homebrew Tap = In-Repo, Prebuilt-Tarball, sha256-Verified, Release-Time Auto-Bumped (openlore CLI only)

- **Status**: Accepted (2026-07-12) — promotes the "Homebrew tap: Reserved/deferred" item in **ADR-011** (deferred list) and `distribution.md` §1.3 from *Reserved* to *Accepted*; the "post-slice-05" deferral trigger has fired (24 features shipped).
- **Date**: 2026-07-12
- **Deciders**: Morgan (nw-solution-architect), DESIGN wave (application/components scope, PROPOSE mode)
- **Feature**: homebrew-binary-distribution
- **Amends**: ADR-011 (release matrix and channels) — Homebrew tap deferred row. **References**: ADR-012 (supply-chain policy) for the sha256 ↔ cosign relationship.

## Context

OpenLore ships two install channels today (ADR-011): `cargo install openlore`
(crates.io; requires a Rust toolchain + ~5-10 min compile) and raw GitHub-Release
tarballs (`curl | tar xz` + hand-verified `.sha256`). Both are friction for the
macOS / Homebrew-Linux user who has `brew` but not (or not willingly) a Rust
toolchain, and for whom a "just let me try it" moment should cost seconds, not a
compile or a manual checksum.

ADR-011 reserved a Homebrew tap "post-slice-05, when there are enough non-Rust
users to justify the maintenance". That trigger has fired. DISCUSS locked the
shape (feature-delta D-1..D-7): a tap hosted **in this repository**
(`Formula/openlore.rb`), installing a **prebuilt** `openlore` CLI binary from an
existing GitHub-Release tarball (never build-from-source), sha256-verified by
brew, registering no service/updater and never phoning home, kept in sync with
each `v*` release automatically.

**Critical prerequisite reality.** DISCUSS's Pre-requisites section assumed the
GitHub-Release 4-tarball + `.sha256` pipeline "already ships". It does **not**:
there is no `.github/workflows/release.yml` (only `ci.yml` + `nightly.yml`;
`nightly.yml` states "release.yml lands when the first vX.Y.Z tag is cut"), there
are no git tags, and no GitHub Releases. **ADR-011 does lock the output
contract** — 4 tarballs `openlore-{version}-{triple}.tar.gz` for the triples
`aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-gnu`,
`aarch64-unknown-linux-gnu`, each with a `.sha256` companion (+ cosign `.sig` +
SBOM per ADR-012). This feature therefore designs the formula + autobump against
ADR-011's **locked contract**, and treats **"release.yml producing that 4-platform
matrix"** as an explicit **blocking external prerequisite** — a separate future
DEVOPS feature (native 4-platform builds + cosign + SBOM), NOT part of this
feature. See `design/upstream-changes.md` and the Open Questions below.

## Decision

### D-1 (tap scope + shape)

- A single new artifact `Formula/openlore.rb` in the **in-repo tap**
  (`github.com/jeffabailey/openlore`). It installs the single prebuilt `openlore`
  CLI binary (`bin.install "openlore"`) — never `openlore-indexer`.
- **No build-from-source**: no `depends_on "rust"`/`"cargo"`, no `cargo build`.
  Homebrew never compiles openlore (feature D-2/D-3).
- **Single multi-platform formula** (resolves **OD-HB-3**): `on_macos`/`on_linux`
  × `on_arm`/`on_intel` `url` + `sha256` blocks, one per shipped triple, pointing
  at the ADR-011 release tarballs. NOT per-platform `bottle do` blocks on ghcr
  (rejected: needs a ghcr bottle-hosting pipeline this lean channel does not
  want; the GH-Release tarballs already exist).

### D-2 (verification)

- Each per-triple `sha256` field **equals the published `.sha256` companion** for
  that tarball (ADR-011 / `distribution.md` §1.2). brew verifies the download
  before install; a mismatch aborts, installing nothing. The formula never
  carries a hand-typed hash that can diverge from the pipeline artifact.
- cosign `.sig` verification stays the separate manual `verify-blob` path
  (`distribution.md` §5, ADR-012); brew's channel trust is sha256. The sha256 is
  the *download-integrity* control; cosign is the *provenance* control — they are
  complementary layers, not substitutes.

### D-3 (no auto-update / no phone-home guardrail)

- The formula registers no `brew services` entry, no launchd/systemd unit, and no
  background update checker. OpenLore does not phone home. Upgrades are only an
  explicit `brew upgrade openlore` (feature D-5, `distribution.md` §6, persona
  P-001 guardrail).

### D-4 (tap resolution — resolves OD-HB-1)

- Homebrew's default resolution maps `jeffabailey/openlore` →
  `github.com/jeffabailey/homebrew-openlore`, which is **not** the in-repo tap.
  To keep the tap truly in-repo (D-1) without a second repo, the user runs a
  one-time explicit-URL tap, then installs normally:
  ```
  brew tap jeffabailey/openlore https://github.com/jeffabailey/openlore
  brew install jeffabailey/openlore/openlore
  ```
  This is documented verbatim in the README. `brew upgrade openlore` then works
  with no further tap step. (Rejected: a direct `brew install <url-to-formula.rb>`
  — installs but breaks `brew upgrade`, defeating US-HB-002; a `homebrew-openlore`
  mirror repo — violates the in-repo D-1 and adds a second repo to keep in sync.)

### D-5 (release-time auto-bump — resolves OD-HB-2, the headline)

- On each `v*` release, `Formula/openlore.rb`'s `version` + 4 `sha256` are
  regenerated from the published `.sha256` companions and **committed directly to
  `main`** (trunk-based, no PR — house rule) by a **job inside the future
  `release.yml`**, sequenced strictly **after** the artifact-upload job via an
  explicit `needs:` dependency (the ordering guard against R-1: never reference an
  unpublished tarball).
- (Rejected: `brew bump-formula-pr` — opens a PR, which **conflicts with the
  trunk-based/no-PR house rule**; a separate event-triggered `formula-bump.yml` on
  `release: published` — weaker ordering guarantee than an in-DAG `needs:` edge,
  and a second workflow to maintain. An in-`release.yml` job with a `needs:` edge
  on upload is the strongest, simplest ordering guarantee.)
- Because `release.yml` does not yet exist, the autobump is specified as a job
  that **EXTENDS the future `release.yml`**, not a standalone pipeline. This is a
  design-time contract against ADR-011's locked outputs; it cannot execute until
  the prerequisite lands (see Open Questions).

### D-6 (Earned-Trust probe = per-triple brew-install smoke test)

- The autobump's correctness is proven, not assumed: a **per-triple `brew install`
  + `openlore --version` smoke test** runs after the bump on real macOS + Linux
  runners and **blocks the release on failure**. This is the Earned-Trust probe
  for the formula adapter — it empirically demonstrates each of the 4 `url`+`sha256`
  pairs actually installs on the real platform where it will run (exercising the
  real GitHub-Releases download + brew checksum path, catching an
  eventually-consistent/partially-uploaded asset "lie"). "Wire (bump) → probe
  (smoke test) → use (release proceeds)"; a failed probe refuses to ship.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **Build-from-source formula** (`depends_on "rust"` + `cargo build`) | Re-imposes the ~5-10 min compile + toolchain this channel exists to remove; contradicts the whole J-006 value. Reject. |
| **Per-platform `bottle do` blocks on ghcr** (OD-HB-3 alt) | Needs a ghcr bottle build/host pipeline; the ADR-011 GH-Release tarballs already exist and are reused directly via `url`+`sha256`. Reject (future optimization). |
| **`brew bump-formula-pr`** (OD-HB-2 alt b) | Opens a PR — directly conflicts with the trunk-based/no-PR house rule. Reject. |
| **Separate `formula-bump.yml` on `release: published`** (OD-HB-2 alt c) | Event-driven ordering is weaker than an in-`release.yml` `needs:` DAG edge (R-1 race window) and adds a second workflow to maintain. Reject in favor of the in-DAG job. |
| **Bare `brew install jeffabailey/openlore/openlore` with no tap step** (OD-HB-1 default resolution) | Resolves to a non-existent `homebrew-openlore` repo → "tap not found". Reject; document the one-time explicit-URL tap instead. |
| **`homebrew-openlore` mirror repo** (OD-HB-1 alt) | Violates the locked in-repo D-1 and adds a second repo to keep byte-synced with each release. Reject. |
| **Publish to `homebrew-core`** | Carries review latency + maintenance obligations a lean custom tap avoids; out of scope (feature Out of Scope). Reject. |
| **cosign `.sig` verification inside the formula** | brew's native trust is sha256 (D-2); cosign stays the documented manual provenance path (ADR-012). A future nicety, not this feature. Reject. |

## Consequences

### Positive

- A third, lowest-friction install channel for non-Rust users: one `brew install`
  (verified, no compile, seconds) + one `brew upgrade`, honoring every P-001
  guardrail (no service, no phone-home, explicit upgrades).
- Zero new binary production: reuses the ADR-011 4-tarball + `.sha256` matrix
  wholesale (feature D-7). No new Rust code, no CDN, no cross-compile.
- The autobump makes formula freshness a machine invariant (KPI-HB-3), not a
  manual chore; the smoke-test probe makes a bad bump a release blocker, not a
  user-facing breakage.

### Negative

- **Hard dependency on a not-yet-existing `release.yml`.** slice-01's dogfood
  install cannot execute until `release.yml` + one real tagged release exist
  (see Open Questions / `upstream-changes.md`). This feature is *designable* now
  against ADR-011's locked contract but *not shippable* until the prerequisite
  lands.
- The one-time explicit-URL `brew tap` (D-4) is one extra command vs a bare
  install; documented in the README (R-2). Revisit a mirror only if this proves to
  be real friction.
- The formula is Ruby + the autobump is YAML/shell — outside the Rust `xtask`
  arch-enforcement surface; enforced instead by Homebrew's own linters (below).

## Architecture Enforcement

Architecture rules without enforcement erode (principle 11). Language-appropriate
tooling for a Ruby/Homebrew + GitHub-Actions surface:

- **`brew audit --strict --online Formula/openlore.rb`** and **`brew style
  Formula/openlore.rb`** — Homebrew's own linters (the ArchUnit/import-linter
  equivalent for a formula): validate DSL correctness, url/sha256 presence per
  block, and no forbidden stanzas. Run in the smoke-test job.
- **Freshness assertion** (KPI-HB-3): a shell gate asserting the formula's
  `version` == `${tag#v}`, run in `release.yml` after the bump. A lagging formula
  fails the release.
- **Per-triple `brew install` + `openlore --version` smoke test** (D-6) on real
  macOS + Linux runners — the behavioral gate that the 4 `url`+`sha256` pairs
  actually resolve and install. Blocks the release on any failure.
- **`bin.install "openlore"`-only assertion**: the smoke test greps the install
  manifest to confirm `openlore-indexer` is never installed (D-3 mechanically
  enforced).

## Earned Trust

*Every dependency you don't probe is an act of faith.* The formula depends on an
external substrate — GitHub Releases serving the exact tarball whose sha256 the
formula claims. That dependency is **probed, not trusted**: the per-triple
`brew install` smoke test (D-6) downloads the real asset on the real OS/arch and
lets brew verify the checksum, on every release, before the release is allowed to
complete. The ordering guard (bump `needs:` upload) closes the "reference an
unpublished tarball" window. Absence of a green smoke test is itself the signal
that the channel is broken — the release refuses to finish, exactly as an adapter
that fails `probe()` refuses to start (`health.startup.refused` analog).

## Revisit Trigger

- The one-time explicit-URL `brew tap` proves to be real friction → revisit a
  `homebrew-openlore` mirror (OD-HB-1 alt b).
- Enough demand for reproducible per-platform bottles → revisit ghcr `bottle do`
  (OD-HB-3 alt).
- A 5th target triple is added to ADR-011 → add the corresponding `on_*` block +
  a smoke-test matrix cell.
- `release.yml` gains cosign-in-channel support brew can consume natively →
  revisit adding provenance verification beyond sha256.
