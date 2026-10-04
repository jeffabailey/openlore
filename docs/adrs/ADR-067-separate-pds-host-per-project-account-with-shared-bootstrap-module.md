# ADR-067: One PDS Host Per Project, In That Project's Account, Bootstrapped by a Shared Module

- **Status**: Accepted (2026-10-04) — implemented; see `docs/evolution/shared-pds-module-evolution.md` (proposed 2026-10-02)
- **Date**: 2026-10-02
- **Deciders**: Jeff Bailey (wizard lock: separate host per project; OpenLore on profile `jeff`), Apex (nw-platform-architect)
- **Feature**: shared-pds-module (DESIGN)
- **Builds on**: ADR-066

## Context

The user asked whether one host could run several PDS servers. The wizard locked **a separate
host per project**, with the same module applied twice. TRB stays in account 415898136109 (`str`).
OpenLore goes to 091153021562 (`jeff`) at `openlore.jeffbailey.us`, a record in the existing public
zone `Z04289081C40P36K0S8LM`.

Read-only checks on the jeff account (2026-10-02) found:

- **no default VPC** in us-east-1, and the module looks one up;
- no GitHub OIDC provider;
- an existing state bucket `jeffbaileyterraformstate` (us-west-2, versioning not enabled, shared
  with other projects);
- an unrelated `complete-ecs` VPC.

## Decision

1. **Topology.** OpenLore gets its own EC2 host in its own account, created by `modules/pds`. Its
   DNS records go in its own zone: an A record plus a wildcard A for handles.
2. **Bootstrap is shared.** `modules/pds-bootstrap`, generalised from TRB's bootstrap, provides:
   - the account guard and zone checks;
   - the backup bucket;
   - per-environment host roles and instance profiles;
   - optional pieces behind `create_state_bucket`, `create_oidc_provider` and `enable_ci_roles`;
   - `create_default_vpc`, which uses `aws_default_vpc`.

   Environment descriptors are passed in as a map rather than read from a fixed path. Resource
   names keep TRB's `${name_prefix}-…` pattern.
3. **OpenLore bootstrap settings:** `name_prefix = "openlore"`, `expected_account_id = 091153021562`,
   `create_state_bucket = false`, `enable_ci_roles = false`, `create_oidc_provider = false`, and
   `create_default_vpc = true`.
4. **State.** Reuse `jeffbaileyterraformstate` (region us-west-2) with keys
   `openlore/pds/bootstrap.tfstate` and `openlore/pds/prod.tfstate`, and `use_lockfile = true`.
   **Prerequisite:** an operator enables versioning on the bucket once
   (`aws s3api put-bucket-versioning … Status=Enabled`).
5. **TRB bootstrap is untouched in phase 1.** Adopting the shared bootstrap is an optional
   phase 2, done with `moved {}` blocks (see ADR-069).

## Alternatives considered

| Alternative | Verdict |
|---|---|
| Several PDS containers on TRB's host (one per hostname, Caddy multi-site) | Rejected by the user's lock. It would also cross two AWS accounts and two DNS zones, and put two irreplaceable PLC keys on one volume. |
| An OpenLore account on TRB's PDS | Rejected by the user's lock. It couples OpenLore to TRB's invite, rate-limit and namespace policy. |
| Reuse the `complete-ecs` VPC | Rejected. It is another project's dev VPC, and its teardown would strand a `prevent_destroy` volume. |
| Make the module accept `vpc_id`/`subnet_ids` | Deferred. It would change data sources in the live TRB module. A default VPC costs $0. |
| A new dedicated state bucket | Acceptable fallback (OQ-3), about $0. Rejected as the default because an existing bucket comes before a new one. |
| Per-project bootstrap copies | Rejected. The host role's SSM and backup grants are coupled to the PDS module's user_data contract, so they belong with it. |

## Consequences

- **Positive**: identities and blast radius stay isolated per project, with no cross-account DNS.
  OpenLore's bootstrap is about 8 resources, and it creates no OIDC provider or CI roles.
- **Negative**: there are two hosts and two public IPv4 charges, accepted by the lock. The default
  VPC creates subnets in every AZ (free, but more objects). The state bucket lives in another region.
- **Unverified**: that `aws_default_vpc` in provider 6.x creates a missing default VPC. The
  fallback is a one-time `aws ec2 create-default-vpc` (R2).
