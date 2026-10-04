# ADR-068: OpenLore's PDS Is One t4g.micro Production Host, Applied From a Laptop

- **Status**: Accepted (2026-10-04) — implemented; see `docs/evolution/shared-pds-module-evolution.md` (proposed 2026-10-02)
- **Date**: 2026-10-02
- **Deciders**: Jeff Bailey ("pick the cheapest options"), Apex (nw-platform-architect)
- **Feature**: shared-pds-module (DESIGN)
- **Builds on**: ADR-066, ADR-067

## Context

OpenLore's PDS serves one operator's account with tiny traffic. TRB's sizing, chosen for a
500k-record bulk import (683 MB measured), is t4g.small with a 20 GB data volume. Upstream's PDS
minimum is 1 GB RAM, 1 core and 20 GB SSD. TRB measured about 1,433 bytes per record, so 100k
records come to about 143 MB.

Some resizes are cheap and some are not:

- An EBS gp3 volume grows in place but cannot shrink.
- An instance type change is an in-place stop/start.
- Every public IPv4 address costs $0.005/h, whether it is an EIP or auto-assigned.

OpenLore is a public repo, so GitHub Environments with required reviewers are free. No deploy
frequency or SLO was given. The working assumption is a handful of deploys a year and no
formal SLO.

## Decision

1. **Size.** t4g.micro, 8 GB gp3 root, a **5 GB** gp3 data volume, an EIP, and a 1 GiB swap file.
   Swap arrives in module v1.1.0 as `swap_mb`. It defaults to 0, which renders identical
   user_data, so TRB is unaffected. Cost is about **$10.84/month**.
2. **Environments.** **prod only.** Pre-production testing uses TRB's
   `spike/compose.local.yaml` pattern on a laptop.
3. **Apply model.** The operator applies from a laptop with `AWS_PROFILE=jeff`, always against
   a saved plan:

   ```
   tofu plan -out=tfplan
   deploy/check-plan.sh tfplan   # refuses delete/replace of the volume, EIP, zone or buckets
   tofu apply tfplan
   ```

   CI (`deploy-pds-check.yml`, `paths: deploy/**`) runs credential-free checks only. These are
   fmt, validate `-backend=false`, the no-destroy scan, and the contact-address grep. There
   are no OIDC roles in v1.
4. **Recovery.** EC2 simplified automatic recovery covers host failure. It is on by default and
   free. The identity is recovered from the volume, or from the encrypted S3 archive once the
   operator has placed `backup-pubkey.pem`.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| t4g.small, 20 GB (TRB parity), about $18.17 | Rejected as the starting point. Upgrading later is a stop/start; starting large cannot be undone on the volume. |
| t4g.nano, about $7.78 | Rejected. 0.5 GiB is below upstream's 1 GB minimum, and a PDS that OOMs is worse than none. |
| No public IPv4 (IPv6-only) | Rejected. ghcr.io and GitHub have no IPv6, and NAT64 needs a $32/mo NAT gateway. The default VPC has no IPv6 either. |
| Auto-assigned public IP instead of an EIP | Rejected. Same price, and the address would change on stop/start, forcing DNS churn. |
| prod + test hosts, about $21.67 | Rejected for cost. One operator, rare deploys, and a local compose is enough. |
| CI apply via OIDC (TRB model) | Deferred (OQ-6). It is a toggle (`enable_ci_roles = true`) plus a workflow copy. Today it would add about 6 IAM resources and a VER-7 subject-claim exercise for a few deploys a year. |
| Savings Plan or reservation now | Deferred (OQ-4). Commit only after the PDS has proved useful (feature-delta R1). |

## Consequences

- **Positive**: lowest viable cost, about 40% below TRB parity. Every upward move (more RAM,
  more disk, CI apply, a test environment) is additive and reversible.
- **Negative**: there is no staging host in AWS. Applies depend on one laptop and one human, so
  there is no second reviewer. A micro host has less burst headroom: `standard` credits
  throttle rather than bill.
- **Watch**: memory pressure. Upgrade to t4g.small if swap-in is sustained or the PDS restarts on OOM.
