# ADR-069: the-reality-base Moves Onto the Shared Module With an Address-Stable Source Swap and a No-Op Plan Gate

- **Status**: Accepted (2026-10-04) — implemented; see `docs/evolution/shared-pds-module-evolution.md` (proposed 2026-10-02)
- **Date**: 2026-10-02
- **Deciders**: Jeff Bailey, Apex (nw-platform-architect)
- **Feature**: shared-pds-module (DESIGN)
- **Builds on**: ADR-066. **Constrains**: the-reality-base `deploy/` (executed in that repo, not here)

## Context

TRB's prod PDS is live. Two of its resources must never be replaced:

- `module.pds.aws_ebs_volume.pds_data` holds the PLC rotation key and the `did:plc`.
- `module.pds.aws_eip.pds` is the stable address.

Both carry `prevent_destroy`, and TRB's plan job refuses deletes of `aws_ebs_volume`, `aws_eip`,
`aws_route53_zone` and `aws_s3_bucket`. The instance is cattle.

These facts decide how state behaves:

- State addresses are keyed by the **module call name** (`module "pds"`) and the resource names
  inside the module. The `source` string does not appear in them.
- In AWS provider v6, a `user_data` change is an in-place stop/start
  (`user_data_replace_on_change` defaults to false). cloud-init does not re-run per-instance
  scripts on that stop/start.
- `aws_security_group.name`, `aws_s3_bucket.bucket` and `aws_iam_role.name` are ForceNew.

## Decision

1. **v1.0.0 is a pure extraction.** No resource or data block is renamed. New inputs default or
   are set so that TRB's inputs produce identical names and identical rendered `user_data`.
   A golden `tofu test` pins the pre-extraction sha256. The only permitted diff is in-place tags.
2. **No `moved` blocks for the PDS module.** The call name `module "pds"` is kept, so every
   address is unchanged.
3. **Cut-over sequence:**
   1. Baseline plan = no changes on test and prod. Record the addresses and the user_data hash.
   2. Extract with history, add the golden tests, tag v1.0.0.
   3. Swap `source` and add the 3 inputs on **test**. Plan, then pass the gate, then apply
      (only if non-empty). Postcheck: empty re-plan and `/xrpc/_health` returns 200.
   4. Repeat on **prod**. The volume and EIP must show `no-op`.
   5. Delete the local module and adjust `deploy-pds.yml` check-job paths.
4. **Gate for the cut-over commits.** This is stricter than TRB's standing gate: **no `create`,
   `delete` or `replace` action on any address**. Only `no-op` or `update` is allowed.
5. **Rollback.** Revert the `source` line. Addresses are identical, so the revert plans as a
   no-op. The module-repo tag is never needed for rollback.
6. **Phase 2 (optional, bootstrap).** TRB's bootstrap root calls `modules/pds-bootstrap` with
   `name_prefix = "trb"` and one `moved {}` block per resource, for example
   `aws_iam_role.plan["prod"]` → `module.bootstrap.aws_iam_role.plan["prod"]` and
   `aws_s3_bucket.state` → `module.bootstrap.aws_s3_bucket.state[0]`. The same no-op gate
   applies, and it is human-run.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| `tofu state mv` / `removed` + `import` | Rejected. It is imperative, outside review, and an interrupted run leaves split state. `moved` blocks (phase 2) are declarative and reviewable. |
| Rename the module call (e.g. `module "openlore_compatible_pds"`) with `moved` | Rejected. It adds risk and buys nothing. |
| Fold swap or other v1.1 features into the extraction | Rejected. A refactor and a behaviour change in one plan make the no-op gate meaningless. |
| Accept instance replacement during cut-over | Allowed but not planned. It is safe (the volume and EIP are separate, `stop_instance_before_detaching`), yet it would hide a regression the golden test is meant to catch. |
| Migrate the bootstrap in the same cut-over | Rejected. It doubles the live surface (IAM roles CI depends on) in one change. Phase 2 is independent. |

## Consequences

- **Positive**: the stateful resources are never in a plan's create or delete set. Each step is
  independently revertible, and test proves the swap before prod.
- **Negative**: v1.0.0 must carry TRB-shaped defaults (descriptor shape, `atproto_namespace`
  field) for the 1.x line. Generalising further is a 2.0 with a migration note.
- **Unverified**: that provider v6 still treats `user_data` as in-place. The golden test makes
  this moot for the cut-over, and it should be confirmed against the provider changelog in DELIVER.
