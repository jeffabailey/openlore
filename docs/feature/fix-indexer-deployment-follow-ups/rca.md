# RCA — indexer-deployment follow-ups

Source: nw-troubleshooter investigation, 2026-10-08 (read-only). User decisions recorded at the end.

## D1 — PURGE_UNLISTED=0 refused: NOT a defect
ADR-082 (lines 36-37) and data-models §14 say "`1` or unset; any other value is refused". `purge_switch` (crates/openlore-indexer/src/config.rs:354-365), AS-55 and CORE-8 all match, and compose sets "1". The evolution note at docs/evolution/indexer-deployment-evolution.md:177 misstated the ADR.
**Fix:** correct that line (doc only).

## D2 — search failures never page (real gap)
- The not_live formula (deploy/indexer/host/health-timer.sh:218-221) uses only running / healthz / summary_45m / dids_age. The search probe (:96-109, :212) already runs every 2 min, but its result goes only into the log line.
- /healthz checks only `unusable_reason()`, which reports only a poisoned mutex (adapter-index-store/src/lib.rs:167-170), and a poisoned store makes serve exit within 50 ms (run.rs:365-370). Every other store error becomes a 500 with no log event (search_handler.rs:107, `map_err(|_store_error| IndexUnavailable)`), so A2 can't see it either.
- ADR-080 §7 (line 81) promised more than the code does. No test fails the search: the XP-13 stub curl always succeeds (xtask/tests/indexer_deployment_platform.rs:574-577).
**Fix:** search_probe captures the HTTP status, and not_live adds search_status == 500. 503, 429 and 408 must NOT count (503 is the busy reply during a pass or purge). A3's 2-of-2 rule absorbs a single blip. Also log a structured search store-error event from the binary (no query text) for diagnosis; it does not page. Update the formula text in observability-design.md, monitoring-alerting.md (A3) and the script header.
**Regression:** xtask `a_search_500_makes_the_host_not_live` (stub: search 500, healthz 200 → not_live:1) plus a 503 case staying not_live:0. Unit/acceptance test for the new log event.

## D3 — render-secrets swaps the directory (hardening; no current user impact)
compose bind-mounts /pds/app/secrets:/run/secrets:ro. render-secrets.sh:55-58 renames the directory, then deletes the old one (:22). The app reads secrets only at startup (wiring.rs:384, :388), and both callers force-recreate the container right after rendering (deploy.sh:206, :234, :291). The one gap: if `docker pull` (:235) fails after rendering, the old container keeps running against an empty /run/secrets.
**Fix:** per-file write-then-rename in the same directory (as render-dids.sh does), removing stale files and never swapping the directory. Rotation stays restart-based.
**Regression:** xtask `two_renders_replace_secret_files_and_never_the_directory`, modelled on the inode test (:449). It fails today.

## D4 — tautological test oracles
1. The TEST_FAULT property (config.rs:747-766): the generator and the oracle both come from the code under test. Replace it with a literal token table, or delete it (a concrete test already exists in config_contracts.rs).
2. CORE-4 `list_oracle` (tests/acceptance/indexer_deployment_core.rs:251-265) mirrors entries_of/with_new, and `sut_read_did_list` (:125-130) drops `problem`. Keep `problem` and use a literal example table per deployment_contracts.rs:70-128.
3. CORE-5 `sut_pass_exit` (:132-152): the harness chooses which failure wins (find_map order), so the property only checks cause.is_some() == (code == 2). Pass all failures to the code under test and assert the expected cause token from a literal table.
4. The control round-trips (control.rs:362-395) only check that encode and decode agree. Add a literal wire assertion (`{"request":"run_pass"}` etc.).
Why missed: the mutants were killed by adding concrete tests, but the self-referential properties stayed, and no review checklist item asks "does the oracle call or mirror the code under test?"

## User decisions (2026-10-08)
- D1: no behaviour change; correct the evolution note.
- D2: the host health check counts search 500 (not 503/429/408) toward A3 not_live, plus a structured search store-error log event from the binary. No new alarm.
- D3: include the per-file render-secrets hardening in this delivery.
- D4: fix all four oracles.

## Known gaps
- No test pins pass-failure precedence (the first failure ends a pass, ingest_pass.rs): `with_fault` takes one fault; PS-17/PS-20 cover single failures.
