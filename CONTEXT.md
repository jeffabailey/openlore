# OpenLore — Resume Context

## Current Task
review-app-repos-and-accounts (choose repos, import any public repo unsigned, up to 3 GitHub accounts) and identity-link viewer slice 4 delivered and archived (`docs/evolution/review-app-repos-and-accounts-evolution.md`, `github-did-identity-link-slice-4-evolution.md`); pushed at 85560a6, CI green, review app live (sha256:af7f7040…, store v2).

## Key Decisions
- Anyone can import a public repo and get private unsigned suggestions; publishing needs a SigningPermit from a same-request owner + bio read (ADR-089). Admin-of-org repos (R4) deferred: needs GitHub OAuth.
- GitHub page reads: 200/h per DID, 1000/h app-wide; scans keep the rest of the 5000/h token.
- Push only when told, over SSH (`git push git@github.com:jeffabailey/openlore.git HEAD:main`); deploy with `deploy/ops.sh review-app deploy <40-hex sha>`.

## Next Steps
- Slice-4 viewer design is still PROPOSED (docs/feature/github-did-identity-link/design); user to confirm its defaults.
- Repos follow-ups (evolution doc "Deferred"): acceptance gaps #214, #424; @security test for the cross-origin guard; drop old `github_links` table.
- Backlog in memory `openlore-follow-up-backlog.md`: `scan.refused`, SPIKE-6, htmx 4, TRUSTED_PROXIES, serde-saphyr; tofu-aws-pds swappiness release.
