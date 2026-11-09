## Development Paradigm

This project follows the **functional programming** paradigm. Use @nw-functional-software-crafter for implementation.

See `docs/adrs/ADR-007-paradigm-functional-rust.md` for the decision rationale (pure claim-domain core, effect shell at I/O edges, ADTs over inheritance, signing/CID as pure transformations).

## Mutation Testing Strategy

This project uses **per-feature** mutation testing. Runs after refactoring during each delivery, scoped to modified files. Kill rate gate: >= 80%.
