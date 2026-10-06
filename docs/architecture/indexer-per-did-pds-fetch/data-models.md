# Data models: indexer-per-did-pds-fetch

> **Status: IMPLEMENTED** (delivered 2026-10-05, steps 01-01..02-04). Copied from
> `docs/feature/indexer-per-did-pds-fetch/design/` at finalize; history in
> `docs/evolution/indexer-per-did-pds-fetch-evolution.md`.

The shapes below are **contracts**: field and arm names plus their meaning. The crafter owns
the internal structure and the derives. The Rust-like notation is illustrative.

## 1. Storage: no schema change (confirmed)

- `indexed_claims.provenance VARCHAR DEFAULT 'app-signed'` already exists
  (`adapter-index-store/src/schema.rs:84-88`, ADR-071 migration). `IndexedClaim.provenance:
  PeerClaimProvenance` is already read and written (`adapter-index-store/src/lib.rs:325-341,
  409`).
- Nothing about passes or skips is persisted. A skipped DID is retried simply because the next
  pass enumerates the config again (AC-002.6). There is no new table, column or migration.

## 2. Pure core: `appview_domain::ingest_pass` (NEW)

```text
TransportPolicy = HttpsPublicOnly                // production (always in release builds)
                | HttpsOrLoopbackHttp            // TEST-ONLY: OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1 in a debug build
FallbackUrl(String)          // validated base URL that passed the policy pre-check; trailing '/' trimmed; built only by parse_config
PdsEndpoint(String)          // a resolved #atproto_pds that passed the policy pre-check (pds_endpoint_admissible)

ResolutionFailure =          // why the DID did not yield a PDS this pass (fallback-eligible)
    NotFound                 // 404 / doc id != DID / no #atproto_pds service
  | Unavailable              // resolver 5xx / transport error / did:web document host address refused
  | TimedOut                 // deadline hit while resolving

ListingSource =
    OwnPds(PdsEndpoint)      // the freshly resolved PDS: the ONLY arm that can yield AuthorPds
  | Fallback(FallbackUrl)    // relay origin, by construction

ListingPlan =
    List(ListingSource)
  | Skip(SkipReason)         // DidUnresolvable (resolution failed, no fallback) or
                             // PdsAddressRefused (resolved endpoint failed the policy pre-check; never falls back)

FetchFailure =               // shell → pure; mapped from IngestError + the deadline
    Unreachable              // transport error, HTTP 5xx, HTTP 429
  | BadResponse              // other non-2xx incl. 3xx (redirects disabled), non-JSON, missing `records`, malformed at:// uri
  | AddressRefused           // guarded DNS resolver left no admissible address (IngestError::AddressRefused)
  | TimedOut

SkipReason = DidUnresolvable | PdsUnreachable | PdsTimeout | ListingFailed | PdsAddressRefused
             // wire tokens: did_unresolvable | pds_unreachable | pds_timeout | listing_failed | pds_address_refused

DidFetch =
    Read    { did, source: ListingSource, listing: RepoListing, resolution_failure: Option<ResolutionFailure> }
  | Skipped { did, reason: SkipReason, fallback_used: bool,
              pds_url: Option<String>,            // own PDS when known, else the fallback URL when used
              fallback_failure: Option<SkipReason> }

PassSummary { configured, own_pds, fallback, skipped }   // invariant: own_pds + fallback + skipped == configured
```

Functions. All are pure and total:

| Function | Contract |
|---|---|
| `pds_endpoint_admissible(&str, TransportPolicy) -> Result<PdsEndpoint, AddressRefused>` | The pre-check. It requires `https`, or `http` only to a loopback host under `HttpsOrLoopbackHttp`. No userinfo. An IP-literal host must not be refused by `ports::net_policy::address_refused`. Trims the trailing `/`. Hostnames are checked after DNS by the adapters' guarded resolver. |
| `plan_listing(Result<String, ResolutionFailure>, TransportPolicy, Option<&FallbackUrl>) -> ListingPlan` | `Ok(url)` that passes the pre-check → `List(OwnPds(e))`. `Ok(url)` that fails it → `Skip(PdsAddressRefused)` (no fallback, AC-003.2). `Err(_)` with a fallback → `List(Fallback(f))`. `Err(_)` without one → `Skip(DidUnresolvable)`. |
| `pass_exit_code(&PassSummary) -> i32` | `3` if `configured ≥ 1 && own_pds + fallback == 0`, else `0`. The shell emits `pass_summary` before exiting. A store failure (exit 2) never reaches this function. |
| `origin_of(&ListingSource, fetched_from: &str) -> RecordOrigin` | `OwnPds(e)` → `RecordOrigin::of(fetched_from, e)`. `Fallback(_)` → `Relay` (the second argument is ignored). |
| `classify_fetch_failure(&ListingSource, FetchFailure) -> SkipReason` plus the `fallback_used` / `fallback_failure` fields | Own PDS: `Unreachable` → `PdsUnreachable`, `TimedOut` → `PdsTimeout`, `BadResponse` → `ListingFailed`, `AddressRefused` → `PdsAddressRefused`. Fallback: `reason = DidUnresolvable`, `fallback_used = true`, `fallback_failure = <the mapped failure>` (AC-003.3, US-IPF-003 ex. 3). |
| `records_of(&Did, &[RepoRecord]) -> (Vec<(rkey, Result<ClaimRecord, String>)>, u64)` | Moved from `run.rs::decoded_records_of`. Returns the repo-bound decoded records plus the count of records whose `at://` repo ≠ the DID (FR-4). |
| `summarize(&[DidFetch]) -> PassSummary` | A fold. `Read` with `OwnPds` → own_pds. `Read` with `Fallback` → fallback. `Skipped` → skipped. |

The deadline rule is implemented in the shell and documented here so ATs can assert it. One
deadline per DID (`per_did_time_budget`) covers resolve, then (if planned) the listing,
including the fallback listing. If it expires while resolving, the result is
`ResolutionFailure::TimedOut`. The fallback is then tried with whatever budget remains, and an
immediate expiry gives `fallback_failure = pds_timeout`. If it expires while listing, the result
is `PdsTimeout`.

## 3. Ports and adapter-visible types

| Type / method | Change |
|---|---|
| `IdentityLookupPort::resolve_pds(&self, did: &str) -> Result<String, IdentityLookupError>` | NEW **default** method: `resolve_did(did).await.map(\|i\| i.pds_endpoint)`. `IdentityLookup` overrides it with a document-only fetch (no handle round-trip). |
| `adapter_atproto_did::pds_endpoint_of(did, &Value) -> Option<String>` | NEW pure fn. Requires `document.id == did` and a service with id `#atproto_pds` or `<did>#atproto_pds` and type `AtprotoPersonalDataServer`. It is shared with `confirmed_identity` (extract, do not duplicate). |
| `IngestError` | + `AddressRefused { host }` (the guarded resolver left no admissible address, or the pre-check refused the host). Mapping change in the adapter: 5xx/429 → `Unreachable`. 3xx is `BadResponse` (redirects disabled). |
| `ports::net_policy` (NEW, pure, `std::net` only) | `address_refused(IpAddr) -> bool` over the refused ranges: 0.0.0.0/8, 127/8, 10/8, 172.16/12, 192.168/16, 169.254/16, `::`, `::1`, fc00::/7, fe80::/10; IPv4-mapped is unwrapped. `TransportPolicy` lives here so both adapters and `appview-domain` share it. Under `HttpsOrLoopbackHttp`, loopback (127/8, `::1`) is admitted and nothing else changes. |
| Adapter constructors | `AtProtoIngestAdapter::guarded(policy)` and `IdentityLookup::guarded(handle_url, plc_url, policy, user_agent)` build a `reqwest` client with the guarded `dns_resolver` and redirect policy none. The existing unguarded constructors stay for the review app. |
| `ProbeRefusalReason` | + `IndexerConfigInvalid`, + `IndexerOriginClassificationUnsound` |
| `NetworkResultRowRaw` | + `provenance: PeerClaimProvenance` |
| `RecordOrigin`, `ClaimRecord`, `RepoListing`, `RepoRecord`, `IndexedClaim` | Unchanged |

## 4. Config (`openlore-indexer::config`, pure `parse_config`)

| Variable | Required | Default | Validation (refuse start on failure) |
|---|---|---|---|
| `OPENLORE_INDEXER_REPO_DIDS` | no | empty (reported) | Split on comma or whitespace. Each entry must match the ATProto DID syntax `did:<method>:<id>` (id chars `[A-Za-z0-9._:%-]`, not ending in `:`, ≤ 2048 chars), and the method must be `plc` or `web`. Duplicates are collapsed (first wins) so the summary arithmetic is exact. |
| `OPENLORE_INDEXER_SOURCE_URL` (name kept, user-confirmed; documented as the **fallback**) | no | absent = no fallback (reported) | When non-empty: an absolute URL that passes the `TransportPolicy` pre-check (https, no userinfo, not a refused IP literal), with no query or fragment. Trailing `/` is trimmed. |
| `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP` | no (TEST-ONLY) | unset → `HttpsPublicOnly` | `1` in a debug build → `HttpsOrLoopbackHttp`. **Set in a release build → refuse start** (`BuildProfile` from `cfg!(debug_assertions)`, the review-app pattern). |
| `OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES` | no | `4` | integer 1..=16 |
| `OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS` | no | `30` | integer 1..=600 |
| `OPENLORE_INDEXER_PLC_ENDPOINT` | no | `https://plc.directory` (also when blank) | Validated at startup under the transport policy, exactly like the fallback (fix-indexer-follow-ups D2): an absolute https URL to a public host, without credentials, query or fragment; plain http to a loopback address only under the test seam. A refusal names the variable and the value. A hostname that resolves only to private addresses is still refused at runtime (IPF-08). |
| `OPENLORE_INDEXER_INDEX_PATH`, `_LISTEN_ADDR` | unchanged | unchanged | unchanged |

`ConfigError { variable: &'static str, value: String, problem: String }`. The rendered message
names the variable and the offending value (the single bad entry, not the whole list).

## 5. Events (stdout, one JSON object per line; WD-105: no claim content)

| Event | When | Fields |
|---|---|---|
| `indexer.config.loaded` | after a successful `parse_config`, before probes | `repo_did_count`, `fallback_configured` (bool), `fallback_url` (when set), `max_concurrent_fetches`, `per_did_time_budget_secs`, `plc_endpoint`, `transport_policy` (`https_public_only` / `https_or_loopback_http`) |
| `indexer.ingest.source_fallback` | a DID was **read** from the fallback | `did`, `reason: "did_unresolvable"`, `fallback_url` |
| `indexer.ingest.source_skipped` | a DID contributed nothing this pass | `did`, `reason`, `fallback_used` (bool), `pds_url` (when known), `fallback_failure` (only when `fallback_used`), `detail` (short transport/status text, ≤ 200 chars, no record bodies) |
| `indexer.ingest.verified` | end of pass (unchanged) | `count` |
| `indexer.ingest.rejected` | end of pass (additive key) | `count`, `by_reason{unsigned, bad_signature, cid_mismatch, schema_unknown, provenance, foreign_repo}`. `count` includes `foreign_repo`. |
| `indexer.ingest.pass_summary` | end of pass, always **before** the process exits (0 or 3) | `configured`, `own_pds`, `fallback`, `skipped`, `duration_ms`, `exit_code` |
| `health.startup.refused` (reused) | config or probe refusal | `binary`, `adapter` (`"config"` / `"origin_classification"` / existing), `reason`, `detail`, `structured{variable, value}` for config |

The existing `eprintln!` refusal line per provenance-refused record is kept (operator readable).

## 6. Wire: search DTO (ADR-079)

```text
SearchResultDto {
  ...unchanged fields...,
  provenance: Option<String>   // "app-signed" | "self-attested"; server always sets it;
                               // absent ⇒ app-signed (old servers); unknown ⇒ CLI drops the row + notice
}
```

There is no `lexicons/` JSON document for `searchClaims`. The Rust DTO in
`crates/lexicon/src/appview_query.rs` is the contract. Its doc comment records the known values.
The change is additive (ADR-005 forward-compat). `SearchResultDto` does not use
`deny_unknown_fields`, so old CLIs ignore the new field.

CLI rendering: an app-signed row is unchanged byte-for-byte. A self-attested row shows
`[verified] [self-attested]`. The result is consistent across the `--object`, `--subject` and
`--contributor` dimensions because all three project through the same `flat_attributed_rows`
(AC-005.3).
