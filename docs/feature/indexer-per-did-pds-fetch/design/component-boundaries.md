# Component boundaries: indexer-per-did-pds-fetch

The crate count does not change. Every change lands in an existing crate. The "Contract shape"
column follows the effect-isolation mandate:
- **pure** means return-only.
- **bounded-change** means a declared mutation set.
- **plan** means the component returns data and never mutates.

## Change map

| Crate | Layer | Change | Contract shape | Crafter's assertion mechanism |
|---|---|---|---|---|
| `appview-domain` | pure core | **NEW module `ingest_pass`**: `ListingSource`, `ListingPlan`, `ResolutionFailure`, `FetchFailure`, `SkipReason`, `DidFetch`, `PassSummary`. `plan_listing`, `origin_of`, `classify_fetch_failure`, `pds_endpoint_admissible`, `records_of` (moved from `run.rs` `decoded_records_of`, and it now also returns the foreign count), `summarize`, `pass_exit_code`. | pure (total functions over ADTs) | Property tests. `origin_of(Fallback, any)` == `Relay` for all strings. `summarize` gives own+fallback+skipped == configured. `plan_listing` never yields `Fallback` for `Ok(url)`, and a refused `Ok(url)` gives `Skip(PdsAddressRefused)`. `pass_exit_code` == 3 iff configured ≥ 1 and own+fallback == 0. |
| `ports` | ports | NEW pure module `net_policy`: `address_refused(IpAddr)`, `TransportPolicy`. + `IngestError::AddressRefused { host }`. | pure / data | Property test over the full refused-range table, including IPv4-mapped IPv6 and range boundaries (for example 172.15.255.255 is admitted and 172.16.0.0 is refused). |
| `appview-domain` | pure core | `ingest_repo_record`, `provenance_verdict`: **unchanged** | pure | Existing property suites (regression guardrail). |
| `ports` | ports | `IdentityLookupPort::resolve_pds(did) -> Result<String, IdentityLookupError>` **default method** delegating to `resolve_did(..).pds_endpoint`. No existing impl has to change. | read-only port | Default-impl unit test with a fake. |
| `ports` | ports | `ProbeRefusalReason` += `IndexerConfigInvalid`, `IndexerOriginClassificationUnsound` | data | JSON contract test (existing pattern in `probe.rs`). |
| `ports` | ports | `NetworkResultRowRaw` += `provenance: PeerClaimProvenance` | data | Constructor sites are updated, with `AppSigned` as the default in fixtures. |
| `adapter-atproto-did` | effect | `IdentityLookup` overrides `resolve_pds`: it fetches the DID document only (PLC or `did:web`) and extracts `#atproto_pds` through a NEW pure `pds_endpoint_of(did, &document)`, which requires `id == did`. There is **no handle back-check**. NEW `IdentityLookup::guarded(.., TransportPolicy, user_agent)` builds the SSRF-guarded client (guarded `dns_resolver`, no redirects); the review app keeps the unguarded `new`. | read-only effect | Property test on `pds_endpoint_of`. A fake-PLC integration test covers 404, 5xx, id mismatch and a missing service. Guard test: a did:web host resolving to a refused address is never connected to. |
| `adapter-atproto-ingest` | effect | `list_page`: HTTP 5xx and 429 map to `IngestError::Unreachable`. Other non-2xx, including 3xx, stays `BadResponse`. NEW `AtProtoIngestAdapter::guarded(TransportPolicy)`: a `reqwest` client with a custom DNS resolver that drops refused addresses (the connection goes to the checked IP, so rebinding is safe), a pure pre-check of IP-literal hosts and scheme, and redirect policy none. A refusal is `IngestError::AddressRefused`. `probe()` no longer refuses an empty source; it refuses a non-empty source that fails the policy pre-check. | read-only effect | Fake-PDS integration test per status class. A guard test uses a stub resolver returning 10.0.0.1, `::1` (without override) and 169.254.169.254, and asserts that no connection is attempted. |
| `lexicon` | wire DTO | `SearchResultDto.provenance: Option<String>` (`serde(default, skip_serializing_if = "Option::is_none")`) | data | Serde round-trip tests: absent → `None`, and old JSON still deserializes. |
| `adapter-index-query` | effect (CLI side) | decode: `None` / `"app-signed"` → `AppSigned`, `"self-attested"` → `SelfAttested`, any other value → row dropped plus a counted notice | pure decode inside the adapter | Table test over the three classes. |
| `cli` | driver (render) | `render/search.rs`: `[self-attested]` after `[verified]` for self-attested rows. `--show` verification line for self-attested: "self-attested by <repo DID>; read from the author's own PDS" (it never says "signature verified"). | pure render | Golden render tests. The app-signed output is byte-identical (NFR-3). |
| `openlore-indexer` | composition root | NEW `config.rs` with a pure `parse_config(lookup: impl Fn(&str) -> Option<String>, profile: BuildProfile) -> Result<IndexerConfig, ConfigError>`. `BuildProfile::of_this_build()` comes from `cfg!(debug_assertions)`, and the loopback override in `Release` is a `ConfigError` (review-app pattern). It wires the **guarded** adapter constructors with the config's `TransportPolicy`. `run.rs`: `ingest` becomes fetch phase + gate phase. `origin_for` is deleted, replaced by `appview_domain::origin_of`. Upsert failure stays fatal (exit 2). Total outage gives exit 3 via `pass_exit_code`, after `pass_summary`. `serve` projects `provenance`. `IndexerWiring.source_url: String` → `fallback: Option<FallbackUrl>`. | bounded-change (mutation set: `index.duckdb` `indexed_claims` upserts and stdout/stderr only) | Acceptance tests through the binary with fake PLC and fake PDSes. |
| `openlore-indexer` | composition root | `probe_gauntlet`: + `origin_classification_probe()` (pure, in-process, no network) | pure probe | Unit test: a mutated classifier makes it refuse. |
| `xtask` | enforcement | + rule `indexer_origin_only_via_listing_source` (below) | static check | xtask unit test with a fixture source string. |

## Boundary rules (who may do what)

1. **Only `appview_domain::origin_of` turns a fetch into a `RecordOrigin` on the indexer path.**
   The indexer root never calls `RecordOrigin::of` and never names `RecordOrigin::AuthorPds`.
   The review app and `peer pull` keep their direct `RecordOrigin::of` calls, because their
   sources are always the resolved PDS.
2. **The fallback can only be constructed from config.** `FallbackUrl` is built only by
   `parse_config`. `ListingSource::Fallback` is built only by `plan_listing` when
   `ResolutionFailure` is present. A resolved DID has no code path to the fallback (AC-003.2).
3. **The pure core never sees time.** The budget is a `Duration` value in config. The deadline
   is applied in the shell (`tokio::time::timeout_at`). The pure core only receives
   `FetchFailure::TimedOut { stage }`. `appview-domain` stays free of tokio (the existing
   check_arch pure-core arm).
4. **The read/write split is unchanged.** The indexer still wires only read-only ingest and
   identity ports plus the store. Nothing here adds a write method to any driven port the pass
   uses. `IndexStorePort` has no delete, so FR-6's "never removes" holds by construction.
5. **Composition-root invariant: wire, then probe, then use.** Config parse happens before wiring
   (refuse on `ConfigError`). The probe gauntlet, including `origin_classification_probe`, runs
   before any verb.

## check_arch changes (specified, not implemented)

| Rule | Kind | Specification |
|---|---|---|
| `indexer_origin_only_via_listing_source` (NEW) | structural, source scan | Scan `crates/openlore-indexer/src/**/*.rs` (excluding `#[cfg(test)]` modules) for the tokens `RecordOrigin::of` and `RecordOrigin::AuthorPds`. Any hit is a violation: "the indexer derives origin only through appview_domain::origin_of(ListingSource) (ADR-077)". Reuse the existing `syn` visitor and token-scan helpers (`classify_cfg_gated_token`). |
| `indexer_guarded_clients_only` (NEW) | structural, source scan | In `crates/openlore-indexer/src/**/*.rs` (non-test), ban the tokens `AtProtoIngestAdapter::new` and `IdentityLookup::new`. The indexer must wire the `::guarded` constructors (ADR-077 §4 SSRF guard). |
| Loopback override in release | behavioral | No new xtask rule. The proptest `parse_config(Release, ALLOW_LOOPBACK_HTTP=1) == Err` mirrors the review app's `config.rs` test. |
| `indexer_holds_no_signing_or_local_store` | dep graph | **Unchanged.** `futures-util` is not a forbidden class. |
| pure-core arm for `appview-domain` | dep graph | **Unchanged** and must still pass: the new module adds no dependency. |
| Allowed-deps comment in `openlore-indexer/Cargo.toml` | docs | Add `futures-util` (bounded fan-out, ADR-078) to the documented dependency list. |

## Earned Trust: the three layers for this feature

| Layer | Question answered | Mechanism |
|---|---|---|
| Subtype (compile time) | Can the fallback ever be classified as the author's PDS? | `ListingSource::Fallback` carries no resolved endpoint. `origin_of` matches on the arm and has no URL input for it. |
| Structural (static) | Did someone bypass the ADT in the root? | `indexer_origin_only_via_listing_source` token ban |
| Behavioral (runtime/CI) | Does the shipped classifier actually behave? | `origin_classification_probe` at startup. ATs drive fake PLC/PDSes through the lies catalogue (architecture-design.md §9). |
