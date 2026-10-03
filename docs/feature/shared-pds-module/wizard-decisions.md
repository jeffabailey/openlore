# shared-pds-module — /nw:new decisions (2026-10-02)

Request: add a PDS deployment for OpenLore like `the-reality-base/deploy`, with the PDS
infrastructure extracted into a module both deployments share. Cheapest options.

| Decision | Answer |
|---|---|
| Topology | Separate host per project, same shared module applied twice |
| OpenLore AWS account | profile `jeff` (separate account) |
| the-reality-base AWS account | profile `str` (415898136109), unchanged |
| Module home | new standalone repo, consumed by git tag (`git::ssh://…//modules/pds?ref=vX`) |
| Classification | Infrastructure, brownfield (reality-base module is live in prod) |
| Starting wave | DESIGN |

Open for DESIGN: OpenLore's PDS domain/hosted zone; what stays project-specific
(name invariants, tags, naming prefix, bootstrap/IAM); migrating reality-base's live
state onto the extracted module with zero replacement of the volume and EIP.
