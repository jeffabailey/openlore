# ADR-012: Supply-Chain Policy = cargo-deny + CycloneDX SBOM + cosign + SLSA L2 (L3 target)

- **Status**: Accepted (locked by user 2026-05-25)
- **Date**: 2026-05-25
- **Deciders**: Apex (nw-platform-architect)
- **Feature**: openlore-foundation (slice-01 walking skeleton)

## Context

OpenLore signs claims for a privacy-conscious senior-engineer audience.
The signing operation itself is the load-bearing trust assumption — if a
shipped binary is compromised, EVERY claim it produces is suspect.

Slice-01 is greenfield and the dependency graph (per
`technology-stack.md`) includes `atrium-api` (pre-1.0; tracking ATProto
Lexicon evolution), `keyring` (platform-specific quirks), and the
`tokio`/`rustls`/`reqwest` ecosystem (industry standard but large
transitive trees).

Common supply-chain attack vectors:
- Typosquatting (e.g., `atrium_api` vs `atrium-api`).
- Compromised maintainer accounts publishing malicious releases.
- Yanked crates with known vulnerabilities silently re-resolved.
- License drift (a dep adds a copyleft transitive that conflicts with
  our redistribution intent).

This ADR locks the policy. The execution is in `ci-cd-pipeline.md` and
`distribution.md`.

## Decision

### Policy = 4 layers

1. **`cargo-deny`** — every CI build runs `cargo deny check`. Enforces:
   - **Licenses**: whitelist (MIT, Apache-2.0, BSD-3-Clause, Unicode-DFS-2016, ISC, Zlib). Any other license fails.
   - **Bans**: hard-deny `openssl`, `openssl-sys` (per ADR-004 rustls
     lock), `serde_cbor` (per `technology-stack.md`), `actix-web`,
     `axum` (no HTTP server in slice-01).
   - **Advisories**: deny RUSTSEC vulnerabilities; warn on unmaintained;
     deny yanked.
   - **Sources**: only crates.io registry permitted; no git deps
     (until/unless a justified exception is added with a comment).

2. **CycloneDX SBOM** — every release tag generates `sbom.cdx.json` via
   `cargo cyclonedx` or `syft` (DELIVER picks). The SBOM is uploaded to
   GitHub Releases alongside the binaries. Consumers can audit the full
   transitive dependency graph.

3. **cosign-signed artifacts** — every release-tag binary tarball is
   signed with sigstore/cosign using GitHub OIDC keyless signing (no
   long-lived signing keys to rotate / steal). `.sig` and certificate
   files accompany each tarball.

4. **SLSA build provenance** — `actions/attest-build-provenance` action
   generates an in-toto attestation per release. Target SLSA L3 via
   GitHub OIDC; L2 acceptable if any GitHub feature gap emerges.

### `Cargo.lock` policy

- **Committed to the repo** (already standard for binaries; reinforcing it
  here as the supply-chain anchor).
- Every release builds against the committed lockfile, NOT a freshly
  resolved one.
- Lockfile-affecting PRs (dep bumps) MUST include a `cargo deny check`
  green run AND a `cargo audit` cross-check.

### Dependency-update workflow

- **Dependabot** enabled with weekly cadence, grouped by major-version
  family (`atrium-*` group, `tokio-*` group, etc.) to limit noise.
- Auto-merge for PATCH-level bumps if all CI gates pass; manual review
  for MINOR and MAJOR.
- Security advisories trigger Dependabot regardless of schedule (the
  default).

### Signing key custody

- **No long-lived signing keys.** Cosign keyless uses ephemeral keys
  bound to GitHub OIDC; the public anchor is sigstore's Rekor transparency
  log. No "the OpenLore signing key was leaked" failure mode is possible
  because no such key exists.
- crates.io publish token: rotated yearly OR on any maintainer change.
  Stored in GitHub repository secrets only. Rotation procedure
  documented in `distribution.md` §1.2.

## Alternatives Considered

| Option | Rejection rationale |
|---|---|
| **No supply-chain controls beyond `cargo` default** | Acceptable for prototypes; not acceptable for code that signs the user's claims. Reject. |
| **GPG-signed release tarballs (PGP)** | Long-lived signing keys = a custody problem we don't want. Cosign keyless is the modern equivalent without that burden. Reject. |
| **Self-hosted Trivy / Grype scanning** | Overlap with cargo-deny + cargo-audit + RUSTSEC; an extra tool with extra config for marginal coverage. Reject for slice-01; revisit if SBOM consumers request specific scanner outputs. |
| **SLSA L4 (two-party review + hermetic builds)** | Requires multiple maintainers and significant infra investment. Solo dev; not reachable today. Aim for L2/L3 which is achievable on GitHub Actions with first-party actions. |
| **Vendored deps (git-submodule the entire dependency graph)** | Defensible but extreme; maintenance burden is disproportionate for slice-01. Reject. |

## Consequences

### Positive

- Four overlapping controls catch different attack surfaces (cargo-deny =
  licenses + bans + advisories; SBOM = consumer visibility; cosign =
  artifact integrity; SLSA = build-process integrity).
- No long-lived signing keys means no custody-leak failure mode.
- The cost is small: 3-4 GitHub Actions adds + a `deny.toml` + a CI step.

### Negative

- cargo-deny advisories block merges; a vulnerability in a transitive
  dep can require a workaround before a fresh release can ship.
  **Mitigation**: cargo-deny supports `[advisories.ignore]` with a
  documented expiration; use sparingly and only with a tracked
  remediation issue.
- SBOM adds ~tens of KB to each release; negligible.
- cosign verification requires the user to install `cosign`; not all
  users will verify. **Mitigation**: ship the verification command in
  the release page so users WHO WANT to verify, can, with one copy-paste.

## Architecture Enforcement

- `cargo deny check` is a blocking CI gate on every PR (per
  `ci-cd-pipeline.md` §3.3).
- A unit test in `xtask` verifies `deny.toml` exists and has non-empty
  `licenses.allow` and `bans.deny` sections (prevents accidental
  emptying of the policy).
- The release workflow's `release-gate` job has explicit dependencies on
  `sbom-generation`, `cosign-sign`, and `attest-provenance` jobs;
  removing any of them is a workflow-file change that requires a
  reviewed PR (which itself runs against this enforcement).

## Earned Trust

The supply-chain policy applies the same Earned Trust pattern as
adapters: the controls are not optional ("we'll add cosign later"), and
their presence is verifiable from the release artifacts (every release
has `.sig`, `.cert`, `sbom.cdx.json`, `provenance.intoto.jsonl` files).
Absence of any of these from a release is itself a signal that something
is wrong.

A user can verify a release end-to-end:
1. `sha256sum -c openlore-0.1.0-*.sha256` — checksum matches.
2. `cosign verify-blob --certificate ... --signature ...` — signature
   anchored to the GitHub Actions workflow OIDC identity.
3. `cyclonedx-cli analyze sbom.cdx.json` — review the dep tree.
4. `cosign verify-attestation --type slsaprovenance ...` — verify the
   build provenance.

## Revisit Trigger

- crates.io adds first-class signing or attestation: re-evaluate whether
  cosign step is still needed (likely keep both for defense in depth).
- A new SLSA level becomes practical on GitHub Actions: bump target.
- A second maintainer joins: revisit SLSA L4 two-party-review feasibility.
- A vulnerability scanner becomes a community ask from SBOM consumers:
  add Trivy or Grype as an additional gate.
