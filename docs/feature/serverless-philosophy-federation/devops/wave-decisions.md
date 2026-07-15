# DEVOPS Decisions — serverless-philosophy-federation

> Wave: **DEVOPS** (lean) · Owner: Apex (nw-platform-architect) · Date: 2026-07-15
> Primary artifact: `../feature-delta.md` (DEVOPS `## Wave: DEVOPS / [REF]` sections) ·
> Machine artifact: `../environments.yaml` · Builds on DESIGN (ADR-062, DDD-1..8) + SPIKE-00.
>
> **The defining reality**: self-hosted serverless, NO central instance (D-1/D-3/D-4, ADR-062) →
> **NO central deploy pipeline**. DEVOPS = (1) a CI contract test, (2) a per-user self-deploy model,
> (3) a write-auth secrets frame.

## Key Decisions

- **[DV-1] CI contract test is the headline deliverable** — a NEW `publish-contract.yml` (modeled on
  `formula-smoke.yml`) spins up the `atproto/` Worker via `npx wrangler dev` (local workerd), builds
  `openlore`, and runs the CLI↔Worker round-trip (`publish init`→`push`→`pull`) asserting CID match,
  INCLUDING the `0.0/0.5/1.0` float regression guard. Operationalizes KPI-SF-1 (North Star). (see:
  `../feature-delta.md` DESIGN external-integration annotation; `.github/workflows/formula-smoke.yml`)
- **[DV-2] NO deploy job / NO real deploy in CI** — CI uses local `wrangler dev` ONLY; never a real
  `wrangler deploy`, never `CLOUDFLARE_API_TOKEN`. Matches how SPIKE-00 ran. (see: `../feature-delta.md`
  Pre-requisites; SPIKE-00)
- **[DV-3] Per-user self-deploy model** — each user runs `wrangler deploy` from `atproto/` to their OWN
  Cloudflare account; `openlore publish init <url>` REGISTERS the already-deployed URL (lean D-5
  default; automating `wrangler deploy` is a flagged option DELIVER owns, not decided here). (see:
  D-5 / ADR-027; US-SF-001)
- **[DV-4] Write-auth = per-instance bearer token as a Cloudflare Worker secret** — owner sets it via
  `wrangler secret put`; CLI sends it on writes (`publish push`); reads (records/manifest/card) stay
  public; secret NEVER in the repo/CI. Frames Q-SF-D2 (DISTILL owns the detailed contract). (see:
  Q-SF-D2; D-7)
- **[DV-5] `ci.yml --workspace` already covers the 2 new Rust crates** — `publish-domain` +
  `adapter-publish-http` unit/acceptance run under the existing `cargo nextest run --workspace`;
  `publish-contract.yml` adds ONLY the live-Worker round-trip. No duplication. (see: `.github/workflows/ci.yml`)
- **[DV-6] Per-feature mutation testing UNCHANGED** — the 2 new Rust crates are covered by the existing
  per-feature mutation; no `CLAUDE.md` rewrite. TS Worker mutation is OUT OF SCOPE (dumb Worker; its
  one property is CI-guarded). (see: project `CLAUDE.md` `## Mutation Testing Strategy`)
- **[DV-7] Trunk-based, CI-on-`main` is the authoritative gate** — `publish-contract.yml` triggers on
  `push` to `main` scoped to `atproto/**` + publish crates + `workflow_dispatch`; no PR/branch-protection
  gate (house rule). (see: MEMORY trunk-based-no-prs)
- **[DV-8] No central telemetry (sovereignty)** — runtime observability is owner-only (Cloudflare
  Workers analytics + `wrangler tail`) + greppable CLI output; no aggregation, no phone-home. (see:
  D-1/D-3/D-4)

## Infrastructure Summary

- **Deployment**: Edge / serverless (Cloudflare Workers / workerd); no containers. Per-user atomic
  `wrangler deploy` (recreate); rollback = redeploy prior revision / `wrangler rollback` (DO data
  persists across code swaps).
- **CI/CD**: GitHub Actions; NEW `publish-contract.yml` (CLI↔Worker round-trip on `wrangler dev` +
  `0.0/0.5/1.0` float guard, no deploy) + existing `ci.yml --workspace` for the new crates. Trunk-based.
- **Observability**: Cloudflare built-in (Workers analytics + `wrangler tail`) owner-only + greppable
  CLI output; NO central telemetry.
- **Mutation testing**: per-feature (unchanged; Rust crates covered; TS Worker out of scope).

## Constraints Established

- The CI contract test MUST use local `wrangler dev` (workerd) only — never a real deploy, never
  `CLOUDFLARE_API_TOKEN`.
- The Worker computes no CID (opaque byte store); the sole canonicalizer is Rust `claim-domain`.
- `PUT /records/:cid` is idempotent per CID (re-PUT = no-op).
- Write path is owner-authed (bearer token as Worker secret); reads are public; secret never in repo/CI.
- `release.yml` does NOT build/deploy the Worker — the Worker is user-deployed, not a release artifact.
- New workflow triggers are disjoint from `ci.yml`/`release.yml`/`nightly.yml`/`formula-smoke.yml`
  (additive, path-scoped).

## Upstream Changes

- None. DEVOPS operationalizes ADR-062 as-is; no DESIGN assumption changed. No `upstream-changes.md`
  warranted.
