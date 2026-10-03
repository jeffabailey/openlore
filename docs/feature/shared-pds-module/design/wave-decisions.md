# DESIGN Wave Decisions — shared-pds-module

> Wave: DESIGN (platform / delivery-infrastructure scope) · Mode: PROPOSE · Architect: Apex (nw-platform-architect)
> Date: 2026-10-02 · ADRs: ADR-066..069 · Feature-delta: `docs/feature/shared-pds-module/feature-delta.md`

## Locked before DESIGN (wizard)

- There is a separate host per project, built from the same module.
- OpenLore deploys to `jeff` (091153021562) at `openlore.jeffbailey.us`.
- TRB deploys to `str` (415898136109), unchanged.
- The module lives in a standalone repo `jeffabailey/tofu-aws-pds` and is consumed by git tag.
- Choose the cheapest option wherever there is a choice.

## Key Decisions (PROPOSE-mode options + trade-offs)

The user may override any RECOMMENDED option.

### OD-1: Module input contract (HIGH)

| Option | Pros | Cons |
|---|---|---|
| **(a) RECOMMENDED: `name_prefix` + `project` + opt-in `require_namespace_matches_hostname` (default false) inside the existing `terraform_data`** | TRB renders identically. OpenLore's mismatch is legal. No address moves | v1.x carries TRB's descriptor shape |
| (b) Remove the namespace precondition. Each caller adds its own | Smallest module | Deletes a `terraform_data` from TRB state, and TRB must re-implement the check |
| (c) Fully generic `name` + `tags` map inputs | Maximum flexibility | Easy to pass a name that ForceNew-replaces TRB's security group |

**Verdict: (a).** See ADR-066.

### OD-2: Bootstrap placement (MED)

| Option | Pros | Cons |
|---|---|---|
| **(a) RECOMMENDED: shared `modules/pds-bootstrap` with toggles. OpenLore uses it now, and TRB optionally moves in phase 2 with `moved` blocks** | One copy of the host-role and backup contract. OpenLore needs no OIDC/CI roles | Toggle matrix to test |
| (b) Per-project bootstrap copies | Zero coupling | Drift. The host-role grants must match the module's user_data |
| (c) Migrate TRB's bootstrap in the same cut-over | One migration | Puts live CI IAM roles in the same change as the PDS swap |

**Verdict: (a).** See ADR-067.

### OD-3: OpenLore sizing and environments (MED)

| Option | $/mo | Verdict |
|---|---|---|
| **(a) RECOMMENDED: t4g.micro + swap, 5 GB data, EIP, prod only** | ≈10.84 | Meets the 1 GB minimum. Upsizing is reversible |
| (b) t4g.small, 20 GB (TRB parity) | ≈18.17 | Over-provisioned. The volume cannot shrink later |
| (c) micro, prod + test | ≈21.67 | Doubles cost for rare deploys |
| (d) t4g.nano | ≈7.78 | Below the PDS minimum. Rejected |

**Verdict: (a).** See ADR-068.

### OD-4: OpenLore apply model (MED)

| Option | Pros | Cons |
|---|---|---|
| **(a) RECOMMENDED: laptop apply of a saved plan + local delete gate. CI runs static checks only** | No OIDC or IAM roles. Matches a few deploys a year | One human, so no second reviewer |
| (b) TRB-style plan-on-push + dispatch apply via OIDC + an Environment reviewer (free on a public repo) | Auditable and repeatable | About 6 IAM resources and a VER-7 subject-claim exercise |
| (c) CI plan-only (drift detection) | Catches drift | Still needs a plan role and OIDC provider |

**Verdict: (a).** (b) stays a one-toggle upgrade. See ADR-068.

### OD-5: Network and state for the jeff account (MED)

| Option | Verdict |
|---|---|
| **Network (a) RECOMMENDED: `aws_default_vpc` in OpenLore's bootstrap.** The module is unchanged | Free, no TRB impact |
| Network (b): module `vpc_id`/`subnet_ids` inputs | Deferred. It touches live TRB data sources |
| Network (c): reuse `complete-ecs` VPC | Rejected. It belongs to another project |
| **State (a) RECOMMENDED: reuse `jeffbaileyterraformstate` (us-west-2) after versioning is enabled** | The existing bucket comes first |
| State (b): new `openlore-tofu-state-…` bucket | About $0. Acceptable fallback (OQ-3) |

See ADR-067.

### OD-6: TRB migration (HIGH)

| Option | Verdict |
|---|---|
| **(a) RECOMMENDED: address-stable source swap. Golden render test, no-op plan gate, test then prod** | No `moved` blocks needed. The volume and EIP never appear in create/delete |
| (b) `tofu state mv` / import | Rejected. Imperative and unreviewed |
| (c) Extraction plus new features in one release | Rejected. It defeats the no-op gate |

See ADR-069.

### OD-7: Module distribution (LOW)

| Option | Verdict |
|---|---|
| **(a) RECOMMENDED: public repo, https git source, semver tags + a tag-protection ruleset** | No CI credentials. Readable bumps |
| (b) SHA pins | Allowed per consumer |
| (c) Private repo over ssh | Rejected. It needs deploy keys in two orgs |
| (d) Registry | Deferred |

See ADR-066.

## Architecture Summary

- **Style**: unchanged for Rust (ADR-009). This feature adds HCL and shell only, and no OpenLore
  crate changes.
- **Driving ports**: operator `tofu plan/apply` (laptop), TRB GitHub Actions (OIDC, unchanged),
  static-check workflows, and module tag releases.
- **Driven adapters**: EC2 t4g, EBS gp3, EIP, Route 53, SSM SecureString, S3 (state and
  backup), Let's Encrypt, plc.directory, and ghcr.io.
- **C4**: L1 and L2 in the feature-delta. L2 is also in the brief §Platform Architecture.
- **DORA**: low-frequency, human-gated deploys are intentional. Lead time is minutes, recovery is
  instance replacement with volume reattach, and change-failure exposure is limited by the no-op
  and delete gates.

## Risks to carry into DISTILL / DELIVER

1. **R1 (HIGH): OpenLore claims are not writable to a stock PDS as implemented.** The causes are
   the float `confidence` (absent from the ATProto data model), the CID divergence from
   ADR-062 / SPIKE-00, and `createRecord` being sent without auth. This is application work and
   out of this feature's scope. Run VER-OL-1 on local compose first.
2. R2: `aws_default_vpc` creation behaviour in provider 6.x. The fallback is
   `aws ec2 create-default-vpc`.
3. R3: memory on t4g.micro. Swap mitigates it, and the fallback is an in-place resize.
4. R6: `backup-pubkey.pem` must be placed by hand before identity backups work.

## Handoff

- **DISTILL**: acceptance checks are `tofu test` goldens (render identity, naming, AZ selection,
  invariants), the no-op plan gate, `/xrpc/_health`, and a TLS handshake on the hostname.
- **DELIVER** order:
  1. Module repo plus v1.0.0.
  2. TRB cut-over: test, then prod.
  3. v1.1.0 (swap).
  4. OpenLore `deploy/` plus bootstrap.
  5. OpenLore prod apply.

  Each step happens only after the user approves it. Nothing is applied in this wave.
- **DELIVER gates before the OpenLore bootstrap apply** (peer review, 2026-10-02):
  1. **State versioning (OQ-3).** Run
     `aws s3api put-bucket-versioning --bucket jeffbaileyterraformstate --versioning-configuration Status=Enabled --region us-west-2 --profile jeff`.
     Verify with `get-bucket-versioning`, which must return `Enabled`. The alternative is a
     dedicated bucket via `create_state_bucket = true`.
  2. **Default VPC (R2).** Plan the bootstrap first. If `aws_default_vpc` does not create the
     VPC, run `aws ec2 create-default-vpc --region us-east-1 --profile jeff` and re-plan. This
     goes into the `deploy/README.md` Contingencies section.

## Peer review

`nw-platform-architect-reviewer`, iteration 1: **approved**. Critical: 0, High: 0, Medium: 2.
Both mediums (the state-versioning gate and the default-VPC contingency) are addressed by the
DELIVER gates above.
