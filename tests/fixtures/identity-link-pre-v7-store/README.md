# Frozen pre-v7 store (github-did-identity-link IL-13a / IL-13b)

A store made by the `openlore` binary whose schema head is **v6**, frozen on 2026-10-14
during DISTILL, BEFORE DELIVER adds the v7 `identity_link_checks` migration. It holds:
Jeff's own memory-safety claim on `BurntSushi/ripgrep`, Rachel's pulled memory-safety claim on
`rust-lang/regex`, and the recorded contributors of `BurntSushi/ripgrep`.

| Path | What |
|---|---|
| `db_export/` | DuckDB `EXPORT DATABASE` dump (`schema.sql` incl. `schema_version` rows 1, 3, 4, 5, 6; `load.sql` with relocatable `@FIXTURE_DIR@` paths; one CSV per table). Rebuilt into a real `openlore.duckdb` by `LinkWorld::with_frozen_pre_v7_store`. |
| `home/` | The rest of the store's home: `identity.toml`, the signed claim file, Rachel's peer-claim file. |
| `graph_query_memory_safety.txt` | `openlore graph query --object org.openlore.philosophy.memory-safety --federated` as the v6 binary printed it — the golden read-back. |

`claims.artifact_path` keeps the temporary path of the run that made it; it is a prior row the
upgrade must leave unchanged, not a path the tests open.

**Do not regenerate after v7 lands.** The regenerator
(`OPENLORE_REGEN_PRE_V7=1 cargo test -p cli --test identity_link -- --ignored regenerate`) refuses
unless the store's schema head is v6, and self-checks that the dump rebuilds the same schema
versions, rows and read-back.
