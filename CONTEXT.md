# OpenLore — Resume Context

## Current Task
Go-live checklist (`deploy/README.md`) done through step 16 (all pushed, origin/main 204da73); review app live at 61b237a (htmx 4 in-place scan status, batch + in-card approve). Step 17 (announce) is the user's; A-8 token-expiring OK email still due (1-day period).

## Key Decisions
- `vm.swappiness` 10 (live host by hand); memory gate passed with scans (indexer 63 MB/100, review app 27 MB/192), so no t4g.small.
- Pre-announce, AWS/infra updates need no per-step OK (user, 2026-10-13); ask again after step 17. Push only when told, over SSH (`git push git@github.com:jeffabailey/openlore.git HEAD:main`).
- Auto-mode classifier blocks the agent stopping apps or editing SSM; hand those commands to the user.

## Next Steps
- tofu-aws-pds 4451296 (`swappiness` input) is pushed but unreleased: release it, then set `swappiness = 10` in openlore's prod root.
- Live-check htmx scan polling and Approve selected as canzantest (user drives sign-in/publish).
- Backlog in memory `openlore-follow-up-backlog.md`: `scan.refused` event (unanswered), SPIKE-6, htmx 4 for local viewer, TRUSTED_PROXIES, serde-saphyr.
