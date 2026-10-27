# Coding Standards

## Code

- Paradigm: functional, as set in `CLAUDE.md` (Development Paradigm); rationale in
  `docs/adrs/ADR-007-paradigm-functional-rust.md`.

## Tests

- Every demoed behavior is backed by a committed, passing test. A slice's acceptance
  suite, asserting exactly what the demo shows, counts.
- Delete temporary demo scaffolding (e.g. `println!`) before committing.
- Mutation testing: per-feature, with the kill-rate gate in `CLAUDE.md`
  (Mutation Testing Strategy).

## Commits

- Small and green: tests pass before every commit.
- Conventional Commits style (`feat(scope): …`, `fix(scope): …`, `docs(scope): …`, etc.).
- During an nWave DELIVER step, include the `Step-ID: NN-NN` trailer.
