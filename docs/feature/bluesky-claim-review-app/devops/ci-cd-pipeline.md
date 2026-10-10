# CI/CD Pipeline: bluesky-claim-review-app (DEVOPS)

> Extends the existing workflows; replaces none. The user's host is slow, so CI is the
> authoritative gate and runs the full suite on every push to main. Deploys are laptop-initiated
> (ADR-068/075). CI builds, scans, signs and publishes the image, and never deploys.

## 1. Pipeline overview

```text
push to main
  ci.yml
   ├─ commit stage (parallel): fmt │ clippy │ cargo-deny │ check-arch (+xtask tests) │ check-probes
   ├─ acceptance stage: test (nextest --workspace, builds openlore, openlore-indexer, openlore-review-app)
   ├─ review-app-build   (needs: test; main push only; ubuntu-24.04-arm; container rust:1-bookworm)
   └─ review-app-image   (needs: review-app-build; ubuntu-24.04-arm)
        docker build (COPY binary into distroless) -> smoke -> trivy -> push GHCR
        -> cosign sign (keyless) -> SBOM attestation -> SLSA provenance
  deploy-pds-check.yml (paths: deploy/**): tofu fmt/validate │ check-plan gate │ + review-app host-file checks
nightly.yml (08:00 UTC): mutants claim-domain (advisory, unchanged) │ + live-contract-smoke (read-only)
laptop: deploy.sh deploy <sha>  (verifies CI green + cosign, then SSM Run Command; infrastructure-integration §7.2)
```

Wall-clock targets: commit stage about 5 min (unchanged); acceptance about 10-12 min (the new
crates add tests); image path about 8 min warm and about 20 min cold (DuckDB's bundled C++ build).

## 2. Commit-stage gates for the new crates

All run in the existing jobs. No new job is needed, because the rules live in `xtask` and
`deny.toml`, which DELIVER edits.

| Gate | Where | What it enforces (component-boundaries §5) | Type |
|---|---|---|---|
| `COMPOSITION_ROOTS += openlore-review-app` | `check-arch` | ADR-072 third root | Blocking |
| `check_pure_core_no_io("review-domain")`, with `unicode-segmentation` allowlisted | `check-arch` | Pure core | Blocking |
| `check_review_app_capability_boundary` | `check-arch` | Disjoint dependency sets: the app never reaches `adapter-duckdb`, `adapter-atproto-pds`, `adapter-publish-http`, `adapter-http-viewer`, `adapter-xrpc-query-server` or `adapter-index-*`; `cli` and the indexer never reach `adapter-atproto-oauth`, `adapter-review-store` or the app | Blocking |
| `review_app_holds_no_signing_identity` | `check-arch` | No `IdentityPort` or keychain constructor in `openlore-review-app/src` | Blocking |
| `user_repo_write_is_create_only` | `check-arch` | No `deleteRecord`, `putRecord` or `applyWrites` in `adapter-atproto-oauth/src` (I-BRA-8) | Blocking |
| `review_store_owner_scoped_sql` | `check-arch` | `owner_did` in every owner-table SQL; `DELETE` only in purge and expiry; `kpi_counters` has no `owner_did` (I-BRA-1, OD-BRA-11) | Blocking |
| **NEW `review_app_log_field_allowlist`** (DEVOPS addition) | `check-arch` | Source scan of `openlore-review-app` and the two new adapters: every `tracing` event uses only allowlisted field names (`observability-design.md` §3). Forbidden names are `token`, `access_token`, `refresh_token`, `bio`, `subject`, `object`, `evidence`, `text`, `handle`, `did`, `cookie`, `code` and `jwk`, plus `?`/`%` debug captures of the forbidden types. | Blocking |
| `check-probes`: non-stub `probe()` within 250 ms for `adapter-atproto-oauth` and `adapter-review-store` | `check-probes` | Earned Trust | Blocking |
| `cargo deny check` (advisories, licenses, bans, sources) | `deny` | The `atrium-*`, `jose-*`, `hickory-*`, `chacha20poly1305`, `p256` and `dashmap` licenses are on the allowlist; `openssl-sys` stays banned, which catches an accidental `atrium-oauth` default feature | Blocking |
| **hickory >= 0.26** | `deny.toml` `[bans] deny` | Add `{ crate = "hickory-resolver:<0.26", reason = "RUSTSEC-2026-0119 (SPIKE-2)" }` and the same for `hickory-proto`. The advisory DB also catches it; the explicit ban documents the floor and survives an advisory being withdrawn. | Blocking |
| `atrium-oauth` exact pin | `Cargo.toml` `=0.1.7` | API churn risk (ADR-073). Reviewed on bump. | Convention (reviewed) |
| fmt, clippy `-D warnings` | existing jobs | Workspace-wide, so the new crates are included automatically | Blocking |

Acceptance stage (`test` job):

- Add `--bin openlore-review-app` to the "Build binaries the acceptance tests spawn" step if the
  DISTILL acceptance tests spawn the binary. Keep `release.yml`'s `verify` job in lock-step, as
  its comment requires.
- Privacy guardrails run here on every push: the I-BRA-1 cross-owner `@property`, plan→record
  equality (AC-004.4), the create-only fake (`FakeUserRepoWrite` panics on a non-create), the
  disconnect purge audit (AC-012.2), and the AC-009.4 regression (every app-signed test stays
  green). These are the CI half of KPI-BRA-4/4b/5/6.
- The recorded-fixture contract tests for the PDS XRPC, OAuth authorization server, PLC and
  GitHub (architecture-design §11) run here, hermetic, with no network.

Per-feature mutation testing (project CLAUDE.md): during DELIVER, `cargo mutants` scoped to
the modified files of `review-domain` and the new adapters, with a kill-rate gate of >= 80%. The
nightly `claim-domain` advisory job is unchanged.

## 3. Image build and publish (new jobs in `ci.yml`)

Trigger: `push` to `main` only (`if: github.event_name == 'push' && github.ref == 'refs/heads/main'`).
They are not path-filtered: every green main commit gets an image, so any sha is deployable.
They run after `test` succeeds, so an image never exists for a red commit.

Concurrency: `ci.yml` cancels in-flight runs for the same ref. A superseded main commit may
therefore lack an image. `deploy.sh` refuses a sha without an image, so deploy the newer sha.

### 3.1 `review-app-build`

- `runs-on: ubuntu-24.04-arm` (the native arm64 precedent from `release.yml`), with
  `container: rust:1-bookworm`, pinned by digest.
- **Why a bookworm container:** binaries built on ubuntu-24.04 link against glibc 2.39 symbols,
  but `distroless/cc-debian12` ships glibc 2.36, so the binary would fail at load time. Building
  on bookworm makes the glibc match. Building in a job container rather than in the Dockerfile
  keeps `Swatinem/rust-cache` working, which matters for DuckDB's bundled C++ compile.
- Steps:
  1. checkout;
  2. rust-cache (key `review-app-arm64`);
  3. `cargo build --release --locked -p openlore-review-app`;
  4. `strip`;
  5. upload the artifact `openlore-review-app-aarch64` (1-day retention).

### 3.2 `review-app-image`

- `runs-on: ubuntu-24.04-arm`.
- `permissions: contents: read, packages: write, id-token: write, attestations: write`, at job
  scope only. The workflow default stays read.
- Dockerfile `crates/openlore-review-app/Dockerfile`. It is a runtime-only stage with no
  compiler:

  ```dockerfile
  FROM gcr.io/distroless/cc-debian12:nonroot@sha256:<pinned>
  COPY --chmod=0555 openlore-review-app /openlore-review-app
  USER 65532:65532
  EXPOSE 8080
  ENTRYPOINT ["/openlore-review-app"]
  CMD ["serve"]
  ```

  Labels: `org.opencontainers.image.source=https://github.com/jeffabailey/openlore`, `revision=<sha>`.
- Steps:
  1. Download the artifact. Run `docker/setup-buildx-action`. Build `--platform linux/arm64 --load`.
  2. **Smoke** (blocking):
     - `docker run --rm img --version` proves dynamic linking works on distroless;
     - `docker run --rm --read-only -v $RUNNER_TEMP/data:/data img probe --self-test` runs every
       hard probe arm with throwaway generated keys, a temporary DuckDB on a real (ext4) bind
       mount and no network. That covers the AEAD canary, the cross-owner canary, JWK
       sign/verify and the JWKS shape. `probe --self-test` is a DELIVER deliverable.
  3. **Scan** (blocking): `aquasecurity/trivy-action`, pinned by sha, with
     `severity: CRITICAL,HIGH`, `ignore-unfixed: true`, `exit-code: 1`. It covers the OS packages
     in distroless plus the Rust binary (trivy reads the Cargo audit data when it is embedded;
     `cargo deny` remains the primary SCA).
  4. **Push** to GHCR with `docker/login-action` using `GITHUB_TOKEN`. Tags: `sha-<40-hex>` and
     `main`. Record the digest from the push output.
  5. **Sign:** `cosign sign --yes ghcr.io/jeffabailey/openlore-review-app@<digest>` (keyless,
     OIDC, as ADR-012 does for tarballs).
  6. **SBOM:**
     - `cargo cyclonedx` for `crates/openlore-review-app` (the release.yml pattern), then
       `cosign attest --type cyclonedx`;
     - also `syft` on the image for the distroless layer, attested the same way.
  7. **Provenance:** `actions/attest-build-provenance@v2` with `subject-name` and
     `subject-digest`, and `push-to-registry: true` (SLSA L2 or better, consistent with ADR-012).
  8. Job summary: digest, tags, and the exact `deploy.sh deploy <sha>` command.

**GHCR visibility (operator, once):** the first push creates a **private** package. Set it to
public in the package settings (the repo is public, so the source is already public). Public
pulls need no credentials on the host. The alternative, a host pull token in SSM, is rejected
because it adds a credential.

### 3.3 Image retention

GHCR keeps every tag. Keep the last 30 `sha-*` versions and delete older untagged versions with a
scheduled `actions/delete-package-versions` step in `nightly.yml`. **The digests listed in the
host's `releases` file must never be deleted.** Keeping 30 covers that at the expected weekly
deploy cadence. This is optional for v1 and can be added when the package passes about 1 GB.

## 4. Release workflow (`release.yml`)

Unchanged. The app is not part of the CLI tarball matrix (`v*` tags). Its delivery unit is the
per-commit image. If a versioned app release is wanted later, add a `review-app-image` job keyed
on the same tag that retags the commit's existing digest as `vX.Y.Z` without rebuilding.

## 5. `deploy-pds-check.yml` extension (credential-free)

Add `deploy/review-app/**` to the path triggers. New job `review-app-host`:

1. `bash -n` and `shellcheck` on `deploy/review-app/*.sh` and `host/*.sh`.
2. `docker compose -f deploy/review-app/host/compose.yaml config -q`, with a stub external
   network and `REVIEW_APP_IMAGE=example`.
3. **Mount guard** (I-BRA isolation, blocking): with `yq`, the compose volumes must be exactly
   `{/pds/app/data:/data, /pds/app/secrets:/run/secrets:ro}`. Any `/pds` source other than these
   two fails. So do `privileged`, `network_mode: host`, `pid: host`, a `docker.sock` mount, an
   added capability, or a missing `mem_limit`, `memswap_limit` or `read_only: true`.
4. **Image pin guard:** `deploy.sh` must resolve and deploy by digest. A unit test asserts that it
   rejects a tag-only reference.
5. **Caddy:** `caddy validate` (in the `caddy:2.8-alpine` image) on a fixture Caddyfile that
   imports `deploy/review-app/host/app.caddy`, with `PDS_HOSTNAME=openlore.jeffbailey.us`.
6. The existing `contact` job's email grep already covers `deploy/review-app/**`. A new grep adds
   a **secrets guard**: no `ghp_` or `github_pat_` prefix, no `"d":` JWK member and no `BEGIN`
   PEM block under `deploy/`.
7. The existing `tofu fmt/validate` covers `review-app.tf` and `review-app-iam.tf`. The existing
   `check-no-destroy.sh` covers the new workflow jobs.

## 6. `nightly.yml` extension: `live-contract-smoke` (advisory, read-only, no secrets)

This is the "nightly live smoke" the DESIGN handoff asked for (architecture-design §11), limited
to what needs no credential:

| Check | How | KPI / risk |
|---|---|---|
| Production endpoints | `curl` `https://app.openlore.jeffbailey.us/healthz`, `/oauth/client-metadata.json` (validate the required fields; `client_id` equals the URL) and `/oauth/jwks.json` (ES256 only, no `d`) | AC-000, client-metadata drift |
| Self-attested reader path | Run the release CLI's `peer pull` (or the indexer ingest) against the test account `canzantest.bsky.social` / `did:plc:ds4bj4ymxzwpisg4qhlislvc`, which holds the SPIKE-1 self-attested claim; assert `provenance=self-attested`, no reject and a recomputed CID equal to the rkey | **KPI-BRA-6** (production evidence) |
| Authorization-server metadata | GET the test account's PDS (resolved via PLC) `/.well-known/oauth-protected-resource`, then the authorization server's `/.well-known/oauth-authorization-server`; assert `pushed_authorization_request_endpoint` and `dpop_signing_alg_values_supported` contains ES256 | ADR-073 contract drift |
| PLC and handle resolution | Resolve the test handle to the DID document; assert the shape | Contract drift |

Advisory: `continue-on-error: true` at job level, with a step that opens or updates a GitHub
issue labelled `live-smoke` on failure through `gh issue` and `GITHUB_TOKEN`
(`permissions: issues: write`). An end-to-end sign-in smoke needs a browser and a test-account
secret, so it is **not** automated in v1. It is part of the R-DEPLOY manual smoke.

## 7. DORA baseline and targets

| Metric | Today (PDS, CLI) | Target for the app |
|---|---|---|
| Deploy frequency | Ad hoc | On demand, weekly or more often during DELIVER (High) |
| Lead time (commit to production) | n/a | < 1 day. CI about 25 min, then the operator runs `deploy.sh`. |
| Change failure rate | n/a | < 15%, measured from `releases` entries followed by a `rollback` within 24 h |
| Time to restore | n/a | < 15 min: auto-rollback on failed readiness, plus `deploy.sh rollback` |

`deploy.sh status` prints the last 10 `releases` lines, which is enough to compute these by hand.
At < 50 users no DORA dashboard is warranted.
