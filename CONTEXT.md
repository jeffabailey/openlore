# OpenLore — Resume Context

## Current Task
github-did-identity-link slices 1–3 delivered and archived (`docs/evolution/github-did-identity-link-slice-{1,2,3}-evolution.md`); all slice-3 follow-ups (NF-1, F-6, U-20, U-47/49/50, PS-10..12, policy rename) pushed, CI green, review app live at 4c429cf (sha256:85f2d221…). Nothing local or undeployed.

## Key Decisions
- Review app reads every AT Protocol host (owner PDS, handle resolver, PLC, did:web) through the SSRF guard via `atproto_address_policy`; unguarded constructors are deleted, not kept.
- A failed write says "may have been saved" for a 5xx or lost answer (`FailedWrite::MayHaveStored`), "not published" only for a 4xx/unreachable/session error.
- Push only when told, over SSH (`git push git@github.com:jeffabailey/openlore.git HEAD:main`); deploy with `deploy/ops.sh review-app deploy <40-hex sha>`. Classifier blocks stopping apps/editing SSM: hand those to the user.

## Next Steps
- Optional identity-link slice: the viewer's linked-identity line (design/wave-decisions.md l.212); ask the user before starting.
- Still open from before (verify first): release tofu-aws-pds 4451296 (`swappiness`), set `swappiness = 10` in openlore's prod root; go-live step 17 (announce) is the user's.
- Backlog in memory `openlore-follow-up-backlog.md`: `scan.refused` event, SPIKE-6 (bring-your-own PDS), htmx 4 for local viewer, TRUSTED_PROXIES, serde-saphyr.
