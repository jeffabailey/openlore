# ADR-066: The PDS Deployment Is a Shared OpenTofu Module in Its Own Repo, With a Generic Input Contract

- **Status**: Proposed (2026-10-02)
- **Date**: 2026-10-02
- **Deciders**: Jeff Bailey (user request: "make a module that I can share between the two deployments"), Apex (nw-platform-architect)
- **Feature**: shared-pds-module (DESIGN)
- **References**: the-reality-base ADR-011 (topology), ADR-012 (descriptors and invariants), ADR-013 (bootstrap, OIDC, no-destroy)

## Context

`the-reality-base` (TRB) runs a live PDS from a local module, `deploy/tofu/modules/pds`. OpenLore
needs the same deployment. The module hardcodes TRB facts:

- `name = "trb-pds-${env}"`
- `Project = "the-reality-base"`
- the precondition `reverse(atproto_namespace) == pds_hostname`, which is false for OpenLore
  (`org.openlore` vs `openlore.jeffbailey.us`)

Its comments also cite TRB-specific ADRs. The handle-under-hostname precondition, by contrast, is
a generic ATProto deployment property. The user locked the home of the code: a standalone repo,
`jeffabailey/tofu-aws-pds`, consumed by git ref.

## Decision

1. **Repo:** `jeffabailey/tofu-aws-pds`, **public**. Layout:
   - `modules/pds`
   - `modules/pds-bootstrap`
   - `scripts/`
   - `examples/single-env`
   - `tests` beside each module

   Consumers use
   `source = "git::https://github.com/jeffabailey/tofu-aws-pds.git//modules/pds?ref=vX.Y.Z"`.
   Https with a public repo means TRB's CI, which runs in a different, private org, needs no deploy key.
2. **Generic contract:**
   - New required input `name_prefix`, giving `"${name_prefix}-pds-${env}"`.
   - New required input `project`, which sets the tag.
   - New `require_namespace_matches_hostname` (bool, default `false`). It gates the existing
     namespace precondition inside the **same** `terraform_data.name_invariants` resource, so
     the resource address does not move.
   - The handle-under-hostname precondition stays unconditional.
   - The `descriptor` object keeps its shape, and `atproto_namespace` stays a field.
   - No resource or data block is renamed in 1.x.
3. **Project policy stays in the project.** Descriptor JSON, `check-descriptors.sh` and
   state-key conventions remain in each consumer repo.
4. **Versioning:**
   - SemVer tags on `main`.
   - A repository ruleset blocks updating or deleting `v*` tags.
   - CHANGELOG entries state "addresses changed: none | list".
   - A major version bump means a resource address or a ForceNew attribute changes for
     existing inputs.
   - Consumers bump `?ref=` in a single commit and gate it with plan.
5. **Module CI:** `tofu fmt -check`, `tofu validate`, and `tofu test` with `mock_provider`. The tests
   cover AZ selection (moved from TRB), a golden sha256 of `user_data` for TRB's prod and test
   inputs, the naming contract, and the invariants. A shell parse of the rendered user_data and
   the no-destroy scan also run. CI uses no AWS credentials.

## Alternatives considered

| Alternative | Verdict |
|---|---|
| Module stays in TRB; OpenLore references `git::…/the-reality-base.git//deploy/tofu/modules/pds` | Rejected. TRB is private in another org, and OpenLore releases would couple to TRB's commit cadence. |
| Copy-paste into OpenLore | Rejected. The user explicitly asked for reusable code, and two copies drift. |
| Private module repo over ssh | Rejected. Both consumers' CI would need a deploy key or PAT, and the repo has nothing secret in it. |
| Public Terraform/OpenTofu registry module | Deferred. It needs a `terraform-aws-*` naming convention and registry publication, which is overhead for two consumers. |
| Pin by commit SHA instead of tag | Acceptable alternative. Tags plus a ruleset were chosen for readable bumps. A consumer may pin the SHA with a `# vX.Y.Z` comment. |
| Generic module that drops the namespace precondition entirely | Rejected. TRB relies on it, and removing it would delete a `terraform_data` from TRB state. |

## Consequences

- **Positive**: one implementation and two consumers. TRB's invariants are preserved byte for
  byte, and OpenLore's legitimate namespace/hostname mismatch is allowed. Module tests run in
  milliseconds without credentials.
- **Negative**: a third repo to maintain. Tag discipline is enforced by a ruleset rather than by
  immutability. The comments in the module must be scrubbed of TRB-only references.
- **Revisit**: a third consumer, or a request for a registry listing, means reconsider registry
  publication.
