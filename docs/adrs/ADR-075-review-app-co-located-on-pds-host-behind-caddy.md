# ADR-075: Co-Locate the Review App on the OpenLore PDS Host, Behind the Existing Caddy

- **Status**: Proposed (2026-10-04). The rollout path needs Jeff Bailey's decision (see "Decision needed").
- **Date**: 2026-10-04
- **Deciders**: Jeff Bailey (co-locate, low ops cost), Morgan (nw-solution-architect); DEVOPS owns execution
- **Feature**: bluesky-claim-review-app (DESIGN), resolves the hosting half of OD-BRA-2
- **Builds on**: ADR-066/067/068 (tofu-aws-pds module, one t4g.micro, laptop-applied)

## Context

ATProto OAuth needs a public HTTPS origin (D-2). The OpenLore PDS host already runs:

- **Caddy 2.8**, terminating TLS on 80/443, from a Caddyfile that user-data writes on first boot to
  `/pds/caddy/Caddyfile` on the data volume;
- **the PDS**, as a compose service on a bridge network;
- **a wildcard site** `*.openlore.jeffbailey.us`, with on-demand TLS whose `ask` points at the
  PDS's `/tls-check`, which approves only hosted handles.

DNS is in Cloudflare, where `openlore` and `*.openlore` point at the EIP. User-data runs only on a
new instance. Changing it means replacing the instance, which is cattle: the data volume and EIP
survive.

## Decision

1. **Origin**: `${app_origin}` = `https://app.openlore.jeffbailey.us`.
   - The existing Cloudflare wildcard already resolves it, so **no DNS change** is needed.
   - The label `app` is reserved and must never be created as a PDS handle.
2. **Runtime**:
   - **Packaging.** One container `review-app` (the `openlore-review-app serve` binary). Image:
     `ghcr.io/jeffabailey/openlore-review-app:<git-sha>`, linux/arm64, on the
     `gcr.io/distroless/cc-debian12` base, because DuckDB needs libstdc++.
   - **Compose.** It is defined in its own compose file `/pds/app/compose.yaml` and joined to the
     PDS compose network. It is not added to the module's compose file.
   - **Volumes.** It mounts **only** `/pds/app/data` (the DuckDB file). It must never mount `/pds`,
     so the PLC rotation key in `/pds/secrets.env` stays out of reach.
   - **Secrets.** It reads `/pds/app/secrets.env` (mode 0600), rendered at deploy time from SSM
     SecureStrings under `/openlore/prod/review-app/`:
     - the client ES256 JWK,
     - the token data key,
     - the GitHub token.
   - **Resource limits.** Container `mem_limit: 256m`, DuckDB `memory_limit 64MB`.
3. **Routing**:
   - Caddy gets an **exact-host site block** `app.{$PDS_HOSTNAME} { reverse_proxy review-app:8080 }`
     with an ordinary HTTP-01 certificate.
   - An exact host match takes precedence over the wildcard block, so the on-demand `ask` is never
     consulted for it.
   - The block is delivered through a **generic import hook**, not hard-coded app knowledge.
4. **Generic import hook in `jeffabailey/tofu-aws-pds` (proposed v1.7.0)**:
   - The rendered Caddyfile gains `import /pds/caddy/sites/*.caddy`.
   - The module stays project-agnostic, and TRB is unaffected.
   - Because `/pds` is the data volume, the site file and the app's compose file **survive
     instance replacement**. After the hook exists, app deploys never touch user-data.
5. **App deploys are laptop-initiated** (ADR-068 discipline). An SSM Run Command pulls the image
   tag, re-renders `secrets.env` from SSM, runs `docker compose -f /pds/app/compose.yaml up -d`,
   then checks `https://app.openlore.jeffbailey.us/readyz`. No OIDC/CI deploy roles are added.
   CI builds and pushes the image only.

## Decision needed (Jeff)

Adding the import hook changes user-data, which takes effect only on a **new instance**. Options:

- **(A, recommended)**
  - Release module v1.7.0 with the hook.
  - Take and verify an identity backup.
  - Do one planned instance replacement (`OPENLORE_ALLOW_DELETE=1`). The data volume and EIP
    survive; expect a few minutes of PDS downtime.
  - After that, the app never needs another replacement.
- **(B, bridge)**
  - Hot-patch the live Caddyfile now via SSM (append the import line, reload Caddy), and ship
    v1.7.0 to take over at the next natural replacement.
  - There is no downtime now, but the live Caddyfile drifts from the rendered one until the next
    replacement. If v1.7.0 is not adopted before that replacement, the app silently loses its
    route.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| **A separate host (a second t4g.nano/micro + EIP)** | Rejected. It adds about $7–10/month and a second host to patch, against the low-ops-cost driver. |
| **Serverless (Lambda + function URL, or Cloudflare Workers as in the J-007 `atproto/` path)** | Rejected for v1. The pure cores are Rust with DuckDB, so this would need a new storage choice (DynamoDB/D1), a new deploy toolchain and a cold-start-tolerant scan design. That is more new surface than co-location for under 50 users. |
| **Serve the app on a path of the PDS hostname (`openlore.jeffbailey.us/app`)** | Rejected. It still needs a Caddy change, puts app cookies on the PDS origin (cookie and CSP bleed), and couples app routing to PDS routes. |
| **Run the app as a host systemd unit on 127.0.0.1** | Rejected. Caddy runs in a bridge-network container, so reaching host loopback needs a `host-gateway` mapping in the module's compose. A container on the compose network is simpler and keeps the app isolated from `/pds`. |
| **Hard-code the app site in the module's Caddyfile** | Rejected. It would leak OpenLore-specific knowledge into a shared module that TRB also consumes (ADR-066). |

## Consequences

- **Positive**:
  - About $0/month extra, with no new host, DNS record or certificate authority.
  - TLS and HTTP→HTTPS come from the existing Caddy (AC-000.2).
  - Published claims do not depend on the app's uptime, because they live in users' PDSes
    (NFR-BRA-9).
- **Negative**:
  - **Shared fate.** A PDS host outage or replacement takes the app down, and an app memory spike
    can starve the PDS on 1 GiB. Mitigations:
    - the container memory limit;
    - the DuckDB memory cap;
    - SPIKE-4 measures RSS during a 10-repo scan;
    - fallback: t4g.small (+$6.13/month, a stop/start, not a replacement).
  - **Broader blast radius.** The host now runs a public multi-user app. This is mitigated by
    container isolation from `/pds`, AEAD-encrypted tokens (ADR-074) and a strict CSP.
