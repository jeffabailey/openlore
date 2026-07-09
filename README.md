# OpenLore

Sign and publish **philosophical claims** about software — which projects embody
which engineering philosophies (memory-safety, test-driven, local-first, …), who
contributes them, and how strongly. Claims are *signed opinions, not truth*: every
claim carries its author's signature, anyone can counter-claim, and nothing is ever
hard-deleted.

OpenLore is a **local-first, single-binary Rust CLI** with an optional read-only web
viewer. Your claims live in a local store first; federation over
[ATProto](https://atproto.com) is opt-in.

- **Architecture**: Hexagonal (ports + adapters), modular monolith — one `openlore`
  binary. Functional-leaning Rust (pure core + effect shell).
- **Store**: an embedded DuckDB file + signed `<cid>.json` artifacts under your data home.
- **Identity & signing**: your ATProto DID + a per-app Ed25519 key; claims are
  content-addressed (CIDv1, dag-cbor).
- **Federation**: signed records under the `org.openlore.*` Lexicon namespace.

## Requirements

- **Rust 1.91+** (stable). The toolchain is pinned in `rust-toolchain.toml`.
- macOS or Linux (WSL2 works). The viewer binds `127.0.0.1` only.

## Install

```sh
git clone https://github.com/jeffbailey/openlore
cd openlore
cargo build            # or: cargo build --release
```

Two helper scripts wrap the binary with sensible local-dev defaults:

- `./cli.sh <verb> [args…]` — builds if needed, then runs `openlore <verb> …`
- `./run.sh [--port N] [--seed] [--with-indexer]` — builds + launches the viewer

Examples below use `./cli.sh`. You can equally run `cargo run -p cli -- <verb> …` or
the built `target/debug/openlore` directly.

## Quick start

```sh
# 1. One-time bootstrap: create your identity + local store (idempotent).
./cli.sh init --handle local-dev.openlore --app-password local-dev-password

# 2. Discover the shared philosophy vocabulary and pick an exact object id.
./cli.sh philosophy list
./cli.sh philosophy show memory-safety

# 3. Make your first claim: "rust embodies memory-safety" (confidence 0.85).
#    A preview is shown; press Enter at the prompt to sign locally.
./cli.sh claim add \
    --subject github:rust-lang/rust \
    --predicate embodiesPhilosophy \
    --object org.openlore.philosophy.memory-safety \
    --confidence 0.85

# 4. Query your local graph.
./cli.sh graph query --subject github:rust-lang/rust

# 5. Browse everything in the read-only web viewer.
./run.sh --seed        # open http://127.0.0.1:8788
```

Every verb supports `--help`, e.g. `./cli.sh claim --help`.

## Command reference

### `init` — bootstrap (run once)

```sh
./cli.sh init --handle <handle> --app-password <password>
```

Resolves your identity and creates the DuckDB store + identity config. Idempotent —
safe to re-run. All other verbs require `init` to have run first.

### `philosophy` — the shared vocabulary

Philosophies are the `--object` of a claim. Use the shared vocabulary so your claims
triangulate with everyone else's instead of stranding on a private string.

```sh
./cli.sh philosophy list [--json]                # list the well-known seeds (offline)
./cli.sh philosophy show <name-or-object>         # full record (name/description/aliases/seeAlso)
./cli.sh philosophy add --name <n> --description <d> [--alias <a>…] [--see-also <url>…]
```

`philosophy add` composes, **signs**, and persists a new `org.openlore.philosophy`
record locally (nothing is written before you confirm the sign prompt). Colliding
with a shipped seed name is refused — reuse the existing one or `--alias` onto it.

### `claim` — author and manage claims

```sh
./cli.sh claim add --subject <uri> --predicate <p> --object <uri> \
                   --confidence <0.0–1.0> [--evidence <url>…]
./cli.sh claim publish <cid>          # publish a signed claim to your PDS (opt-in)
./cli.sh claim retract <cid>          # retract via counter-claim (no hard-delete)
./cli.sh claim counter <cid> --reason "<why>"   # counter someone's claim
```

`claim add` is **local-first**: it previews the claim, and only signs + writes after
you press Enter. Subjects are URIs like `github:owner/repo`; objects are philosophy
ids like `org.openlore.philosophy.memory-safety`.

### `graph query` — explore the local (and federated) graph

```sh
./cli.sh graph query --subject <uri>                 # claims about one project
./cli.sh graph query --object <philosophy> --federated   # who embodies a philosophy
./cli.sh graph query --contributor <did>             # one author's whole trail
./cli.sh graph query --object <philosophy> --weighted    # display-only adherence ranking
./cli.sh graph query --object <philosophy> --traverse --depth 3   # walk the edges
./cli.sh graph query --object <philosophy> --explain <subject>    # audit one weight
```

`--object` / `--contributor` / `--traverse` / `--weighted` imply federated scope
(your claims + subscribed peers). A bare `--subject` query stays local-only.

### `peer` — federation

```sh
./cli.sh peer add <did>          # subscribe to a peer's claim stream
./cli.sh peer pull               # pull + verify + cache all subscribed peers
./cli.sh peer remove <did> [--purge]   # unsubscribe (--purge also deletes the cache)
```

### `scrape github` — propose claims from a public source

```sh
./cli.sh scrape github <owner/repo | user>          # derive candidates, write nothing
./cli.sh scrape github <owner/repo> --sign 1,3      # sign selected candidates
```

Without `--sign`, scrape only *proposes* candidates (a human gate) — no writes. `--sign
N[,N…]` signs the chosen 1-based candidates through the normal claim pipeline.

### `search` — query the network index

```sh
./cli.sh search --object <philosophy>       # search the network by philosophy
./cli.sh search --contributor <did>         # by contributor
./cli.sh search --subject <uri>             # by project
./cli.sh search --object <philosophy> --show <cid>   # inspect one result
./cli.sh search --object <philosophy> --share        # emit a shareable query link
```

`search` is the only network verb; `graph query` stays local. An unreachable indexer
degrades gracefully and never blocks the CLI. Run an indexer with `./run.sh --with-indexer`.

### `ui` — the read-only viewer

```sh
./cli.sh ui [--port 8788]      # or, with build + bootstrap handled for you:
./run.sh [--port N] [--seed] [--with-indexer] [--release]
```

A long-running server bound to `127.0.0.1` that renders **your own node's store** as
HTML. It is strictly read-only — no auth, no signing key. All signing stays in the CLI.

## Data & configuration

State lives under your **data home**, set by `OPENLORE_HOME` (the helper scripts
default to `./.openlore-home` in the repo so experiments stay self-contained). Point it
at `$HOME` to use a persistent personal store.

| Variable | Purpose | Dev default |
|---|---|---|
| `OPENLORE_HOME` | Data/config root (DuckDB + signed artifacts) | `./.openlore-home` |
| `OPENLORE_DID` | Signing DID stub for `init` | `did:plc:local-dev` |
| `OPENLORE_KEY_SEED_HEX` | Ed25519 seed (hex) for local signing | 64 zeros (dev key) |
| `PROFILE` | Cargo profile for `cli.sh` (`debug`/`release`) | `debug` |

The dev defaults use a throwaway key — do not publish claims signed with it as if they
were authoritative.

## Development

```sh
cargo test --workspace        # full suite (unit + property + subprocess acceptance)
cargo xtask check-arch        # enforce the pure-core / effect-shell boundary
cargo fmt && cargo clippy     # format + lint
```

- **Paradigm**: functional-leaning Rust — pure claim/vocabulary core, effects at the
  I/O edges. See `docs/adrs/ADR-007-paradigm-functional-rust.md`.
- **Architecture SSOT**: `docs/product/architecture/brief.md`; decisions in `docs/adrs/`;
  shipped-feature history in `docs/evolution/`.

## License

MIT OR Apache-2.0.
