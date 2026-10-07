# Acceptance review: indexer-deployment (DISTILL)

- **Reviewer**: nw-acceptance-designer-reviewer (thorough), 2026-10-06. One iteration.
- **Verdict**: APPROVED. 0 blockers, 0 high, 2 medium, 2 low. Composite 9/10.
- **Mandates**: CM-A, CM-B, CM-C, CM-D, CM-E, CM-F, CM-H pass. CM-G (Tier B) is not applicable;
  see DWD-IXD-12.

## Findings and dispositions

| # | Sev | Finding | Disposition |
|---|---|---|---|
| 1 | medium | The 1 s search budget (PS-10, PS-11, PS-12) could be flaky on slow hardware | Kept: it is the NFR-IXD-3 contract. DELIVER watches these three on CI. If they flake, the fix is to measure server-side time per request (HTTP only) instead of relaxing the budget. |
| 2 | medium | Some Then steps use pass vocabulary ("reports exactly one summary") | Accepted: `pass_summary` is the operator's domain term (alarms and freshness key on it, US-IXD-004/005). |
| 3 | low | Unix socket path length | Already handled: the socket lives in a short `/tmp/olx*` directory (`short_socket_dir`). |
| 4 | low | Outline sizes (AS-50 9 rows, RAC-1 8 rows) | No change needed. |

## Self-review items carried to DELIVER

These are listed in `wave-decisions.md` under "Upstream findings": render-dids `chown` as non-root,
bats vs the Rust-driven script test, control-socket refusal rule, the CLI's message on a 500,
and the B9 read-back adapter test.

## Definition of Done (DISTILL → DELIVER)

| # | Item | Status |
|---|---|---|
| 1 | Scenarios written, compiling, scaffolded RED | done (74, all `#[ignore]`) |
| 2 | Test pyramid: layer-2 PBT, layer-3/4 ATs, structural xtask checks | done |
| 3 | Peer review approved | done |
| 4 | Runs in CI | `test` job (nextest workspace) and `check-arch` (`cargo test -p xtask`), unchanged workflows |
| 5 | Demonstrable story | WS-1 |
| 6-8 | Policy, language, state-delta port | inherited; policy rows appended |
| 9 | Reconciliation | passed, 0 contradictions |
| 10-13 | Mandates 8, 9, 10, 11 | met (Tier B not applicable) |
| 14-16 | Pillars 1-3 | met |
| 17-20 | Mandate-12 | typed vocabulary in `indexer_live.rs` / `indexer_network.rs`; steps delegate; ratio about 7.5× (informational) |
| 21 | Completeness audit | 14/15, COMPLETE (C5b gap, LOW) |
| 22 | PBT / table density | 10 generators + pinned tables; closed sets as tables |
