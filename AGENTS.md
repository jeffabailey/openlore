# AGENTS.md — Working Agreements for AI Agents

- Before writing code, tests, or a commit message, read `CODING_STANDARDS.md`.
- **Trunk-based:** `main` is the only branch; commit there. No pull requests: never
  branch for, draft, or suggest one.
- Commit locally; push when the user says push, over SSH:
  `git push git@github.com:jeffabailey/openlore.git HEAD:main`.
- Tooling fixes outside this repo (e.g. the nWave install under `~/.claude/`): apply in
  place across all active copies, then record them in `CONTEXT.md`.
- After a slice ships through the nWave pipeline, follow `docs/agents/slice-cadence.md`
  without being asked.
