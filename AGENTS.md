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

## Slice cadence: try → test → auto-advance

After shipping a slice through the nWave pipeline, do all of the following **without
waiting to be asked** — this is a standing instruction, not a per-slice prompt:

1. **Try it out.** Run a live demo of the shipped behavior against the real binary
   (e.g. spawn the real `ViewerServer` over a production-seeded store and exercise the
   new surface end-to-end).
2. **Make it test-backed.** The demo must be backed by a committed, passing test — never
   an ad-hoc one-off. The slice's acceptance suite (which asserts exactly what the demo
   shows) satisfies this; if a behavior is demoed but not covered, add the test before
   moving on. Do not leave temporary demo scaffolding (e.g. `println!`) in the tree.
3. **Auto-advance to the next slice.** Immediately pick the strongest grounded next slice
   from the backlog (the open jobs in `docs/product/jobs.yaml`), state which one and why
   in one line, and start its full nWave pipeline. Do **not** stop to ask "ship another
   slice?" — only pause for a genuine fork (e.g. a real scope decision the user must make).

Keep committing directly to `main` throughout (see trunk-based rules above).
