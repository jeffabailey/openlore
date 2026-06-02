# AGENTS.md — Working Agreements for AI Agents

Conventions any agent (Claude Code, etc.) must follow in this repository.

## Version control: trunk-based development

- **Commit directly to `main`.** This project practices trunk-based development.
- **Do NOT open pull requests.** No PRs, ever — not for features, fixes, refactors,
  or tooling changes. If a change is ready, commit it to `main`.
- **Do NOT suggest, prepare, or branch for PRs.** Don't propose "upstreaming via a PR,"
  don't create feature branches expecting a PR, don't draft PR descriptions.
- Keep commits small, conventional, and green (tests pass before commit).
- There is intentionally **no git remote** configured — nothing is pushed. The local
  `main` history is the record of truth.

## Commit messages

- Conventional Commits style (`feat(scope): …`, `fix(scope): …`, `docs(scope): …`, etc.).
- During an nWave DELIVER step, include the `Step-ID: NN-NN` trailer.
- Co-author trailer for AI-assisted commits is fine.

## Scope of changes

- Tooling fixes that live outside this repo (e.g. the nWave install under `~/.claude/`)
  are applied in place across all active copies; record them in `CONTEXT.md`. Still no PRs.
