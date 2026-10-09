# OpenLore review app: deploy runbook

The hosted review app (`https://app.openlore.jeffbailey.us`) runs as the `review-app` compose
project on the PDS host, behind the PDS's Caddy. The files, the image and the resource settings
are described in `deploy/README.md`, *Review app*; the design is in
`docs/feature/bluesky-claim-review-app/devops/` (infrastructure-integration §7 has the full
R-DEPLOY and rollback detail).

**Going live** (the module release, the one instance replacement, secrets, the first deploy, the
memory gate, enabling and then test-firing the alarms, `REVIEW_APP_LIVE`, the announcement) is
the one ordered [Go-live checklist](../README.md#go-live-checklist) in `deploy/README.md`, shared
with the indexer. This file does not carry its own sequence.

Day to day (laptop, `AWS_PROFILE=jeff`, with `gh`, `cosign`, `jq` and `crane` or `docker buildx`):

```sh
deploy/review-app/deploy.sh deploy <40-hex git sha>   # CI green + cosign verify, then Recreate by digest
deploy/review-app/deploy.sh status                    # container, last logs, releases, host.health
deploy/review-app/deploy.sh rollback [--restore-db]   # previous digest from /pds/app/state/releases
deploy/review-app/deploy.sh redeploy | stop | install
```

- `install` fails closed: it refuses unless containers cannot reach IMDS and the PDS Caddy
  imports `/etc/caddy/sites/*.caddy` from the `/pds/caddy/sites` mount; it never creates that
  directory.
- After an instance replacement, run `deploy/review-app/deploy.sh redeploy` (go-live checklist,
  step 6).
- When an alarm fires: `deploy/review-app/deploy.sh status`, then
  `docs/feature/bluesky-claim-review-app/devops/monitoring-alerting.md` §2.
- KPI sums and forgetting a person go through the loopback admin listener (`GET /admin/kpi`,
  `POST /admin/purge`; kpi-instrumentation §4). There is no `deploy.sh` mode for them.
