# OpenLore — Resume Context

## Current Task
Nothing in flight: all 30 features in `docs/feature/` are finalized (main `a63235d`, CI green). Last shipped: `contributor-philosophy-inference` (scrape records contributors; `infer people` proposes signed person→philosophy claims) and `fix-viewer-double-users-fetch`.

## Key Decisions
- Inferred person claims are signed only by a human (`--sign`), from signed, standing repo claims; links are append-only; STRONGER supersedes, never edits.
- Homebrew smoke test is macOS-only and post-release (ADR-061 D-6 amendment).
- ADR-063/064 left Proposed by choice.

## Next Steps (full backlog: "Issues and follow-ups" in the two evolution docs below)
- GitHub request budget: `/repos/{o}/{r}` fetched twice per repo scrape; add a cross-driving-port request-count check (`docs/evolution/fix-viewer-double-users-fetch-evolution.md`).
- Housekeeping: KPI-CPI-1..5 + KPI-HB-1..3 into `kpi-contracts.yaml`; `Confidence::try_new` panic scaffold; release notes dead rc6 link; pin cargo-cyclonedx (`docs/evolution/contributor-philosophy-inference-evolution.md`, `docs/evolution/github-release-binaries-evolution.md`, `docs/evolution/homebrew-binary-distribution-evolution.md`).
- Tooling: cargo-mutants `-p cli` baseline (21 appview_search failures in copied tree); verify bump-formula → formula-smoke dispatch on the next GA tag.
