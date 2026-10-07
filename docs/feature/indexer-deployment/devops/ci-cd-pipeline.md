# CI/CD Pipeline: indexer-deployment (DEVOPS)

> Extends the existing workflows; replaces none. CI is the authoritative gate (the operator's host
> is slow). CI builds, scans, signs and publishes the image; it never deploys. Deploys are
> laptop-initiated (ADR-068/075).

## 1. Pipeline overview

```text
push to main
  ci.yml
   ├─ commit stage (parallel): fmt │ clippy │ cargo-deny │ check-arch (+xtask tests) │ check-probes
   ├─ test (acceptance stage, nextest --workspace)
   ├─ release-guard (indexer)       needs: [test]   -- unchanged
   ├─ review-app-build → review-app-image            -- unchanged
   └─ indexer-build   (needs: [test]; main push only; ubuntu-24.04-arm; container rust:1-bookworm)
      └─ indexer-image (needs: [indexer-build]; ubuntu-24.04-arm)
           docker build (COPY binary into distroless) → smoke → trivy → push GHCR
           → cosign sign (keyless) → SBOM attestations → SLSA provenance
  deploy-pds-check.yml (paths: deploy/**): tofu fmt/validate │ check-plan │ review-app host checks │ + indexer-host
nightly.yml: mutants (advisory) │ live-contract-smoke (advisory) │ + index-smoke (advisory, KPI-IXD-1/3)
laptop: deploy/indexer/deploy.sh deploy <sha>   (CI green + cosign verify, then SSM Run Command)
```

Wall clock: the indexer image path runs in parallel with the review-app path, about 8 min warm,
about 20 min cold (DuckDB's bundled C++). No change to the commit or acceptance stage budgets.

## 2. Commit and acceptance stages (DELIVER-owned additions)

| Gate | Job | Enforces |
|---|---|---|
| `index_store_delete_only_in_purge`, `index_purge_only_in_pass_runner`, `indexer_search_handler_read_only` | `check-arch` | architecture-design §11 |
| `check-probes` sees a `probe()` on the `IndexPurgePort` implementation | `check-probes` | Earned Trust |
| Acceptance tests for B1-B15 (ADR-080..083 scenarios, AC-002.2 timing over the checkpoint) | `test` | DISTILL |
| `release-guard` (IPF-30) | `release-guard` | **Unchanged.** It stays and keeps proving the release binary refuses the loopback seam; the image smoke repeats the same check on the shipped artifact (§3.2). |

Per-feature mutation testing (project CLAUDE.md): during DELIVER, `cargo mutants` scoped to the
modified files (`appview-domain` `plan_purge`/`pass_exit_code`, `adapter-index-store` `purge.rs`,
the indexer config parser), kill rate ≥ 80%.

## 3. Image jobs (new in `ci.yml`, mirroring the review-app jobs)

Trigger: `if: github.event_name == 'push' && github.ref == 'refs/heads/main'`. Not
path-filtered, so every green main sha is deployable. `ci.yml`'s `cancel-in-progress` may leave a
superseded sha without an image; `deploy.sh` refuses it, so deploy the newer sha.

### 3.1 `indexer-build`

- `needs: [test]`, `runs-on: ubuntu-24.04-arm`, `container: rust:1-bookworm@sha256:<same pin as review-app-build>`,
  `permissions: contents: read`.
- Steps: checkout; `Swatinem/rust-cache@v2` (key `indexer-arm64`);
  `cargo build --release -p openlore-indexer` (**no `--locked`**: `Cargo.lock` is gitignored,
  same as the review-app job and `release.yml`); `strip`; `cargo cyclonedx` for
  `crates/openlore-indexer` → `openlore-indexer.cdx.json`; upload artifact
  `openlore-indexer-aarch64` (1-day retention).

### 3.2 `indexer-image`

- `needs: [indexer-build]`, `runs-on: ubuntu-24.04-arm`, job-scoped
  `permissions: contents: read, packages: write, id-token: write, attestations: write`.
- `IMAGE=ghcr.io/jeffabailey/openlore-indexer`.
- `crates/openlore-indexer/Dockerfile` (runtime-only):

  ```dockerfile
  FROM gcr.io/distroless/cc-debian12:nonroot@sha256:<same pin as the review app>
  LABEL org.opencontainers.image.source="https://github.com/jeffabailey/openlore" \
        org.opencontainers.image.title="openlore-indexer" \
        org.opencontainers.image.licenses="MIT OR Apache-2.0"
  COPY --chmod=0555 openlore-indexer /usr/local/bin/openlore-indexer
  USER 65532:65532
  EXPOSE 8080
  ENTRYPOINT ["/usr/local/bin/openlore-indexer"]
  CMD ["serve"]
  ```

- Steps:
  1. Build `--platform linux/arm64 --load` with label `org.opencontainers.image.revision=$GITHUB_SHA`.
  2. **Smoke (blocking):**
     - `docker run --rm img --version` (dynamic linking on distroless);
     - **serve smoke** with the production posture: `--read-only --tmpfs /tmp --cap-drop ALL
       --security-opt no-new-privileges:true --memory 128m --memory-swap 128m --init`,
       a temp `/data` bind mount, a `/config` directory holding `repo-dids` = `did:web:smoke.invalid`,
       the production env from `infrastructure-integration.md` §2, `-p 127.0.0.1:18080:8080`. Then:
       `GET /healthz` 200 within 20 s; `POST` search → 200 empty; `GET /xrpc/x` → 404;
       an 8 KB body → not 413 (200 or 400 by content), a 9 KB body → 413; `docker exec <c> openlore-indexer trigger` exits **3** (the only DID
       cannot resolve) and the container log contains exactly one `indexer.ingest.pass_summary`
       with `exit_code: 3` and a `pass_id`; a second concurrent `trigger` either coalesces (0) or
       runs after; `docker stop` completes in < 3 s (init forwards SIGTERM).
     - **loopback-seam refusal** on the shipped binary: starting with
       `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1` exits non-zero with `health.startup.refused`.
     The serve smoke depends on B1-B5, B8, B9 and B12; it lands in the same commit as the
     first image job (DELIVER). Until then the job runs `--version` only and is not deployable
     (`deploy.sh` refuses a digest whose job summary lacks `serve-smoke: passed`).
  3. **Scan (blocking):** `aquasecurity/trivy-action` pinned by sha (same pin as review-app),
     `severity: CRITICAL,HIGH`, `ignore-unfixed: true`, `exit-code: 1`.
  4. **Push** `sha-$GITHUB_SHA` and `main`; record the pushed digest (same extraction as review-app).
  5. **Sign:** `cosign sign --yes $IMAGE@$DIGEST` (keyless, GitHub OIDC).
  6. **SBOM:** syft image SBOM (`anchore/sbom-action`), then `cosign attest --type cyclonedx` for
     both the cargo and image SBOMs.
  7. **Provenance:** `actions/attest-build-provenance@v2`, `push-to-registry: true`.
  8. Job summary: digest, tags, `serve-smoke: passed`, and the exact
     `deploy/indexer/deploy.sh deploy $GITHUB_SHA` command.

GHCR: the first push creates a **private** package; the operator makes `openlore-indexer` public
once (I-1). `deploy.sh install` checks anonymous pull and prints this step if it fails.

Retention: as for the review app (optional nightly cleanup keeping the last 30 `sha-*`
versions). Digests listed in `/pds/indexer/state/releases` must never be deleted.

## 4. `release.yml`

Unchanged. The indexer CLI binary stays in the tarball matrix as today; the container image is
per-commit.

## 5. `deploy-pds-check.yml`: new job `indexer-host` (credential-free)

Add `deploy/indexer/**` to the path triggers.

1. `bash -n` and `shellcheck` on `deploy/indexer/deploy.sh` and `host/*.sh`.
2. `bats deploy/indexer/tests/render-dids.bats` (the H1 inode regression, `infrastructure-integration.md` §4).
3. `docker compose -f deploy/indexer/host/compose.yaml config -q` with a stub external network and
   `INDEXER_DIGEST=sha256:` + 64 zeros.
4. **Mount and posture guard** (blocking: any violation fails the job and leaves main red, so
   `deploy.sh` refuses the sha; `yq`; each compose file is checked on its own, the indexer at 128m
   here and the review app at its own value, 192m after B11): volumes exactly
   `{/pds/indexer/data:/data, /pds/indexer/config:/config:ro}`; `init: true`;
   `read_only: true`; `mem_limit == memswap_limit == 128m`; `oom_score_adj >= 900`;
   `container_name: openlore-indexer`; no `privileged`, `network_mode: host`, `pid: host`,
   `cap_add`, `docker.sock`, published `ports`; env has no `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP`
   or `OPENLORE_INDEXER_REPO_DIDS`. The review-app guard additionally asserts `init: true` and
   the B11 DuckDB cap variables once B11 lands.
5. `systemd-analyze verify` on the four unit files, plus a grep that `TimeoutStartSec` of the pass
   service is ≥ 26 min and the timer has `Persistent=true`.
6. `caddy validate` (caddy:2.8-alpine) on a fixture Caddyfile that imports both `app.caddy` and
   `index.caddy`, with `PDS_HOSTNAME=openlore.jeffbailey.us`; a grep that `index.caddy` has
   `max_size 8KB` and exactly two proxied matchers.
7. Digest guard: a unit test that `deploy.sh deploy main` and a short sha are refused.
8. Existing `tofu fmt/validate` covers `indexer.tf` and `indexer-iam.tf`; a module-ref lockstep
   check is already in place for the two roots.

## 6. `nightly.yml`: `index-smoke` (advisory, no secrets)

| Check | How | KPI |
|---|---|---|
| Public health | `GET https://index.openlore.jeffbailey.us/healthz` 200; `last_successful_pass_at` ≤ 30 min old | KPI-IXD-3 (spot) |
| Network results | `POST` a fixed broad search; resolve each distinct author DID through `plc.directory` (or `did:web`) to its PDS service endpoint; assert ≥ 2 distinct hosts and every row has an author | KPI-IXD-1 |
| Surface | `GET /xrpc/com.atproto.repo.createRecord` → 404 | AC-001.3 |
| TLS | curl validates the certificate | AC-001.2 |

`continue-on-error: true`; on failure it opens or updates a GitHub issue labelled `index-smoke`
(same mechanism as `live-contract-smoke`, `permissions: issues: write`). The run prints the host
count, which `kpi-instrumentation.md` reads for the KPI-IXD-1 baseline.

## 7. DORA baseline and targets

| Metric | Today | Target |
|---|---|---|
| Deploy frequency | Never deployed | On demand, weekly or more often during DELIVER |
| Lead time | n/a | < 1 day (CI about 25 min, then `deploy.sh`) |
| Change failure rate | n/a | < 15%: `releases` entries followed by a `rollback` within 24 h |
| Time to restore | n/a | < 15 min: automatic rollback on not-ready, `deploy.sh rollback` about 1 min |

`deploy.sh host-status` prints the last 10 `releases` lines; enough to compute these by hand.
