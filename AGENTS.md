# AGENTS.md — Working Agreements for AI Agents

Before writing code, tests, or a commit message, read `CODING_STANDARDS.md`.

## Version control: trunk-based development

- **Commit directly to `main`; never branch for, suggest, or open a pull request.**
- Push only when the user explicitly says push, over SSH:
  `git push git@github.com:jeffabailey/openlore.git HEAD:main`.

## Scope of changes

- Tooling fixes that live outside this repo (e.g. the nWave install under `~/.claude/`)
  are applied in place across all active copies; record them in `CONTEXT.md`.

## Slice cadence: try → test → auto-advance

After shipping a slice through the nWave pipeline, do all of the following **without
waiting to be asked** — this is a standing instruction, not a per-slice prompt:

1. **Try it out.** Run a live demo of the shipped behavior against the real binary
   (e.g. spawn the real `ViewerServer` over a production-seeded store and exercise the
   new surface end-to-end).
2. **Make it test-backed.** If a demoed behavior is not yet covered, add the test before
   moving on.
3. **Auto-advance to the next slice.** Immediately pick the strongest grounded next slice
   from the backlog (the open jobs in `docs/product/jobs.yaml`), state which one and why
   in one line, and start its full nWave pipeline. Do **not** stop to ask "ship another
   slice?" — only pause for a genuine fork (e.g. a real scope decision the user must make).
