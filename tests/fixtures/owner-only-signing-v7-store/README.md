# Frozen v7 store (owner-only-signing RA-12b)

A store made by the `openlore` binary whose schema head is **v7** (identity links, ADR-085),
frozen on 2026-10-20 during DISTILL (review iteration 1, ruling R5), BEFORE DELIVER adds the v8
`repo_authority_checks` migration. It holds Jeff's link `did:plc:test-jeff` → GitHub
`jeffbailey` (id 1234567), with two link checks:

| check_id | checked_at | verdict |
|---|---|---|
| 5 | 2026-09-20 09:00 (older than 7 days at every RA-12b instant) | verified |
| 8 | 2026-10-01 09:00 (the latest; decides the gate) | verified |

| Path | What |
|---|---|
| `db_export/` | DuckDB `EXPORT DATABASE` dump (`schema.sql` incl. `schema_version` rows 1, 3-7 and the `identity_link_check_ids` sequence at its current value; `load.sql` with relocatable `@FIXTURE_DIR@` paths; one CSV per table). Rebuilt into a real `openlore.duckdb` by `LinkWorld::with_frozen_store_from`. |
| `home/` | The rest of the store's home: `identity.toml` and the signed link claim file. |
| `identity_list.txt` | `openlore identity list` as the v7 binary printed it at 2026-10-03T09:00Z — the golden read-back RA-12b compares against. |

No `.duckdb` file is committed (same dump approach as `identity-link-pre-v7-store`), so the
`*.duckdb` ignore rule needs no exception.

**Do not regenerate after v8 lands.** The regenerator
(`OPENLORE_REGEN_V7=1 cargo test -p cli --test owner_only_signing_repos -- --ignored regenerate`)
refuses unless the store's schema head is v7, and self-checks that the dump rebuilds the same
schema versions, link checks and `identity list` output, and that a new check after the rebuild
does not collide with the frozen ones.
