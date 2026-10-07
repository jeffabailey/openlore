# Technology Stack: indexer-deployment

> **No new crates** in `Cargo.lock`. At most one added feature flag on an existing dependency (tokio
> `io-util`, if the crafter frames the control channel with async line I/O). Everything else is an
> existing component or a stock platform feature.

| Concern | Choice | License | Status | Alternatives rejected |
|---|---|---|---|---|
| Index store and review store | DuckDB via `duckdb` crate (existing) | MIT | Existing. Adds `memory_limit` (48 MB) and `threads` (1) at open, read back by the probe, in both the indexer and the review app (B9, B11). | SQLite WAL: store rewrite, disproportionate (ADR-080 g) |
| HTTP server | `hyper` 1.x + `hyper-util` + `http-body-util` (existing) | MIT | Existing. Adds a body limit (`http-body-util::Limited`), a header read timeout and a `/healthz` route. | `axum`: banned in `deny.toml` |
| Control channel | Unix domain socket (`tokio::net::UnixListener` or `std::os::unix::net`) | MIT (tokio), std | New use of existing dependencies. Unix only (`cfg(unix)`). | Loopback TCP (mis-bind risk); signals (no result channel); a second container (store lock) |
| Async runtime | tokio current-thread (existing) | MIT | Existing. The pass runs on a dedicated OS thread with its own current-thread runtime (ADR-080 §6). | Multi-thread runtime: not needed |
| Container base | `gcr.io/distroless/cc-debian12`, linux/arm64 (as the review app) | Apache-2.0 | DEVOPS | Alpine/musl: DuckDB needs libstdc++ |
| Image signing | cosign keyless via GitHub OIDC (existing pipeline) | Apache-2.0 | DEVOPS (mirror the review app) | none |
| Scheduler | systemd timer + oneshot service (host) | LGPL-2.1 (OS component) | DEVOPS | cron: no `Persistent=`, no native overlap guard |
| Reverse proxy and TLS | Caddy 2.8 (module-owned), stock directives only | Apache-2.0 | Existing. A new site file through the v1.7.0 import hook. | `caddy-ratelimit` build: changes a shared module (ADR-083) |
| DID list source | AWS SSM Parameter Store, Standard String | proprietary (AWS, already in use for the PDS and review app) | DEVOPS. Free tier. | SecureString: KMS for public data |
| Logs, metrics, alarms | CloudWatch Logs (awslogs driver), metric filters, alarms, existing SNS topic | proprietary (AWS, existing) | DEVOPS. 3 conditions, ≤ $0.30/month. | Custom metrics: cost; Prometheus: a new service |
| Arch enforcement | `cargo xtask check-arch` (syn scans) | project | 3 new rules (architecture-design §11) | — |

Proprietary components are limited to the existing AWS account services that the PDS and the review app
already use (ADR-066..068). No new vendor is added.

## Cost delta (DEVOPS confirms)

| Item | Monthly |
|---|---|
| Container on the existing host | $0 |
| SSM Standard parameter | $0 |
| Log ingestion (about 100 lines per pass × 96 passes, structural only) | < $0.10 |
| 3 alarm conditions (A3 may be one composite alarm) | about $0.10–0.30 |
| **Total** | **≤ $0.40** (NFR-IXD-6: ≤ $2), on t4g.micro (user decision). t4g.small (+$6.13 and an unplanned PDS stop/start) only by operator decision at the gate. |
