# Walking skeleton: indexer-deployment (DISTILL)

> **Status: IMPLEMENTED (not yet deployed)**. Walking skeleton GREEN from step 01-03 (`dd79705`). Copied from `docs/feature/indexer-deployment/distill/` at finalize; history in `docs/evolution/indexer-deployment-evolution.md`.

## The user-visible slice

Maria sets `OPENLORE_INDEXER_URL` and runs `openlore search --object
org.openlore.philosophy.reproducible-builds`. She sees Priya's self-attested `cargo-pin` claim
(bsky.social) and Dmitri's app-signed `ferrite` claim (pds.volkov.dev), each attributed to its own
author. They got there because the host timer fired `openlore-indexer trigger`, which ran one pass
**inside** the long-running `serve` that owns the index (ADR-080).

Litmus: a stakeholder reads WS-1 and says "yes, that is the live network index Maria needs".

## Scenarios (`tests/acceptance/indexer_deployment_walking_skeleton.rs`)

| ID | Scenario | Proves |
|---|---|---|
| WS-1 | Maria finds claims from authors on different PDSes after the scheduled pass | `serve` + DID list file + control socket + `trigger` + in-`serve` pass + one `pass_summary` + `openlore search` over the same store, ≥ 2 hosts, attribution |
| WS-0 | A fresh deployment answers before its first pass | empty index answers (no 5xx); `/healthz` 200 with `last_successful_pass_at: null` |
| WS-2 | A claim Priya approves after a pass becomes searchable on the next pass | the next triggered pass picks up new records with no restart (AC-002.1 mechanism) |

## Wiring (Architecture of Reference, project policy)

| Port | Class | Treatment |
|---|---|---|
| `openlore-indexer serve` (production posture) | driving | REAL binary, subprocess (`support/indexer_live.rs::LiveIndex`) |
| `openlore-indexer trigger` (the timer's `docker exec`) | driving | REAL binary, subprocess, serve's environment |
| `openlore search` | driving | REAL binary, subprocess |
| `/healthz`, `searchClaims` | driving | REAL listener, in-test HTTP |
| `index.duckdb` | driven internal | REAL file |
| DID list file | driven internal | REAL file, replaced by rename in a config directory |
| PLC + PDSes | driven external | `FakeAtprotoNetwork` |

Tags: `@walking_skeleton @wiring_e2e @driving_port @real-io`. The image, compose, timer and Caddy are
proven by the CI posture tests (`xtask/tests/indexer_deployment_platform.rs`) and by the DEVOPS image
serve-smoke (AS-56 is its in-repo twin). The live end-to-end over `https://index.openlore.jeffbailey.us`
is the nightly `index-smoke` (DV-IXD-15), not an acceptance test.

## Status at hand-off

RED for the right reason (no `trigger` verb, no `/healthz`; see `red-classification.md`) and
`#[ignore = "DELIVER 01-0x: …"]`. DELIVER enables WS-1 first.
