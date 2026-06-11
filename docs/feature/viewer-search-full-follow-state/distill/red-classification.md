<!-- markdownlint-disable MD013 -->
# RED Classification — slice-20 (viewer-search-full-follow-state)

> DISTILL Pre-DELIVER fail-for-the-right-reason gate (nw-distill §"Pre-DELIVER
> fail-for-the-right-reason gate"). Every slice-20 acceptance scenario was run once
> against the CURRENT (unimplemented) production code and classified. DELIVER reads
> this file at the RED-phase entry gate (ADR-025 D2) to confirm RED is genuine.
>
> Owner: Quinn (nw-acceptance-designer) · 2026-06-11 · Rust / cucumber-free
> subprocess-HTTP acceptance shape (mirrors slice-16).

## How the run was performed

```
cargo build --bin openlore --bin openlore-indexer        # build-before-run (bins are spawned, not rebuilt by cargo test)
cargo test --test viewer_search_full_follow_state -- --test-threads=1
cargo test --test viewer_search_full_follow_state_invariants -- --test-threads=1
```

Both targets COMPILE green (no `error[...]`, no `ImportError`-equivalent) — they
spawn the real `openlore ui` + `openlore-indexer` bins over HTTP and import only the
existing `support` harness, so there is NO unbuilt Rust dependency. Therefore EVERY
failure is a RUNTIME assertion or a deliberate `todo!()` scaffold — RED, never BROKEN.

## Classification key

- **RED (MISSING_FUNCTIONALITY)** ✅ — the assertion fires because the four-arm
  behavior is unimplemented (the two empty render arms `You | UnsubscribedCache =>
  {}`, the two missing presence reads, the missing precedence). Correct RED.
- **RED (MISSING_FUNCTIONALITY via `todo!()` scaffold)** ✅ — the scenario panics at a
  deliberate Mandate-7 scaffold seam (the OQ-1 per-read cached fault seam) because the
  seam does not exist yet. Correct RED.
- **GREEN-today (guardrail / byte-stable)** ✅ — the scenario PASSES against current
  code because it pins a CARDINAL that must NOT regress (read-only, no-control,
  byte-stable slice-16 rendering, neutral framing when the new arms render nothing, or
  a precedence outcome the slice-16 binary resolution already produces). These are
  intentionally green-from-the-start guardrails; DELIVER must keep them green.
- **BROKEN / SETUP / IMPORT** ❌ — would block handoff. **NONE remain** (two seeding
  bugs found during the gate were fixed; see "Setup bugs found and fixed" below).

## Tally

| File | Scenario | Classification | Reason |
|---|---|---|---|
| `viewer_search_full_follow_state.rs` | FF-1 `all_four_follow_states_render_with_peer_add_only_on_the_genuinely_new_author` (WS) | RED ✅ | own claim + cached peer render `peer add`; self/residue indicators absent |
| | FF-2 `an_own_claim_in_isolation_shows_the_self_indicator_and_no_add_command_anywhere` | RED ✅ | own claim renders `peer add did:plc:test-jeff`; self indicator absent |
| | FF-3 `a_soft_removed_cached_peer_in_isolation_shows_the_residue_indicator_and_no_add_anywhere` | RED ✅ | cached peer renders `peer add did:plc:tobias-test`; residue indicator absent |
| | FF-4 `an_active_and_cached_peer_resolves_to_subscribed_peer_by_precedence` | GREEN-today ✅ | active-and-cached already resolves `SubscribedPeer` via the slice-16 binary (active wins); pins precedence stays green through the four-arm change |
| | FF-5 `an_own_claim_resolves_to_you_even_when_the_active_set_is_populated` | RED ✅ | own row renders `peer add`; `You`-over-`SubscribedPeer` precedence absent |
| | FF-6 `the_slice16_followed_and_unfollowed_states_are_byte_stable_with_no_new_indicators` | GREEN-today ✅ | no-regression guardrail: slice-16 render byte-stable + no new indicator where none applies |
| | FF-7 `a_soft_removed_author_is_matched_despite_the_signing_key_fragment_on_the_result_did` | RED ✅ | fragmented Tobias renders `peer add`; cached fragment-strip + residue indicator absent |
| | FF-8 `neither_new_indicator_is_an_executable_control_and_both_are_neutral` | GREEN-today ✅ | read-only + neutral-framing guardrail: holds today (no control, no pejorative term when arms render nothing) |
| | FF-9 `a_failed_cached_peer_read_degrades_only_that_arm_without_crashing` | RED (todo!) ✅ | panics at `start_viewer_with_failing_cached_peer_read` — the OQ-1 per-read fault seam is MISSING |
| | FF-10 `the_four_follow_states_render_identically_under_htmx_and_no_js` | RED ✅ | self/residue indicators absent in both shapes |
| `viewer_search_full_follow_state_invariants.rs` | FF-INV-NoControl `no_four_arm_follow_state_render_adds_an_executable_control` | GREEN-today ✅ | C-1 CARDINAL guardrail: no executable control today; must stay green |
| | FF-INV-ReadOnly `every_four_arm_follow_state_render_leaves_the_store_read_only` | GREEN-today ✅ | Mandate-8 universe-bound read-only guardrail (`claims`+`peer_claims` counts unchanged); holds today |
| | FF-INV-LocalPerUserNeutral `the_four_arm_resolution_adds_no_network_seam_index_stays_per_user_neutral` | RED ✅ | local-resolution proxy needs the self/residue indicators to flip with the LOCAL sets; absent today |
| | FF-INV-AttributionUnchanged `the_four_arm_resolution_does_not_merge_or_rerank` | RED ✅ | anti-merging proxy needs the self/residue indicators present WITH-state; absent today |
| | FF-INV-NeutralFraming `the_two_new_indicators_are_neutral_never_pejorative` | RED ✅ | non-vacuous: requires the indicators PRESENT (then neutral); absent today |
| | FF-INV-OwnVsCacheDistinct `a_removed_but_cached_author_is_not_shown_as_a_fresh_add_candidate` | RED ✅ | residue indicator absent; the cache author still renders `peer add` (the exact bug the slice fixes) |

### Numeric summary (slice-20 scenarios only; excludes 2 pre-existing `state_delta` framework self-tests)

| Classification | Count |
|---|---|
| RED — MISSING_FUNCTIONALITY (assertion) | 8 |
| RED — MISSING_FUNCTIONALITY (`todo!()` scaffold, OQ-1) | 1 |
| GREEN-today (CARDINAL / byte-stable / precedence guardrail) | 5 |
| **BROKEN / SETUP / IMPORT** | **0** |
| **Total slice-20 scenarios** | **14** |

RED total = **9** (8 assertion + 1 scaffold). GREEN-today guardrails = **5**. Zero BROKEN.

The 5 GREEN-today scenarios are correct by design: they pin cardinals (read-only,
no-control, byte-stability, neutral-when-empty) and a precedence outcome (active-and-
cached → SubscribedPeer) that the slice-16 binary resolution already satisfies — they
exist to FAIL if DELIVER's four-arm change regresses a cardinal, not to drive new
implementation. The 9 RED scenarios drive the new behavior.

## Gate verdict

**PASS.** Every failing scenario fails for the RIGHT reason (MISSING_FUNCTIONALITY —
either a runtime assertion on the unimplemented four-arm render, or the deliberate
OQ-1 `todo!()` per-read fault seam). Zero scenarios are in category 2 (IMPORT_ERROR /
FIXTURE_BROKEN / SETUP_FAILURE) or category 3 (WRONG_ASSERTION / internal-struct
coupling — every assertion scans the OBSERVABLE rendered HTTP body, never a
`viewer-domain` struct field). Handoff to DELIVER is UNBLOCKED.

## Setup bugs found and fixed during the gate (would otherwise have been category-2 BROKEN)

The fail-for-the-right-reason gate caught two seeding-seam bugs in the DISTILL harness
additions; both were fixed so the gate is clean:

1. **Multi-peer `peer pull` seam collision.** `seed_cached_unsubscribed_peer_for`
   internally runs `peer add` + `peer pull`; if an ACTIVE subscription already existed
   (Rachel seeded before Tobias), the pull pulled ALL active peers but only wired
   Tobias's resolver seam → `peer pull must succeed` failed. **Fix:** seed the
   cached-then-soft-removed peer BEFORE the active subscription, so the cached peer's
   pull runs while it is the only active peer, and the active-subscription seed
   (`seed_active_subscription_for`) is add-only (no pull). Applied across FF-1/8/9/10
   and all four-arm invariants.

2. **Re-activating a soft-removed peer.** FF-4 originally seeded Rachel
   cached-then-soft-removed THEN re-`peer add`-ed her, expecting reactivation — but the
   re-add left 0 active rows (`peer add` on a soft-removed peer does not cleanly
   reactivate in this harness). **Fix:** added `seed_active_and_cached_peer_for` (a
   plain `peer add` + `peer pull`, NO remove) — the peer is ACTIVE and CACHED in one
   step, the genuine active-and-cached precedence state. Also reframed the
   structurally-impossible own-AND-self-followed / own-AND-self-cached precedence edge
   (`peer add <own-did>` is rejected by design) to the realizable FF-5
   `You`-beats-a-populated-active-set edge; the impossible edges are pinned at the
   DELIVER unit layer by the total precedence pure fn.

## OQ-1 finding (resolved here, per ADR-057 D-4)

**ESCALATION FIRES — a new per-read fault seam IS needed.** DESIGN's D-4 default bet
was: NO new `#[cfg(debug_assertions)]` fault token; a per-read `Err` is injectable via
a fake `StoreReadPort`. **This bet does not hold for the project's acceptance harness.**
The `ViewerServer` drives the REAL `openlore ui` SUBPROCESS over HTTP against the REAL
DuckDB (Pillar 3 — production composition root); there is NO in-process fake-port
injection point. This is the SAME constraint slice-16 recorded for SF-8 (the viewer
holds ONE long-lived DuckDB connection taken at startup; `make_store_unreadable` would
refuse STARTUP, not a mid-request read). slice-16 resolved it by materializing a
`#[cfg(debug_assertions)]` env seam (`OPENLORE_VIEWER_FAIL_ACTIVE_SET_READ`) +
extending the xtask `VIEWER_FAIL_SEAM_TOKENS` guard.

**DELIVER MUST therefore (the ADR-057 D-4 conditional escalation):**

1. Add a distinct `#[cfg(debug_assertions)]`-gated fault token PER new read at its own
   cfg-gated site (mirroring `active_set_read_with_fault_seam` in
   `crates/adapter-http-viewer/src/lib.rs`):
   - `OPENLORE_VIEWER_FAIL_OWN_DIDS_READ` — induces an `Err` for the
     `distinct_own_author_dids` read (degrades the `You` arm independently).
   - `OPENLORE_VIEWER_FAIL_CACHED_PEER_DIDS_READ` — induces an `Err` for the
     `distinct_cached_peer_author_dids` read (degrades the `UnsubscribedCache` arm
     independently — exercised by FF-9).
2. Extend `VIEWER_FAIL_SEAM_TOKENS` in `xtask/src/check_arch.rs` (currently the 4
   slice-16/17/18 tokens) with the two new tokens, keeping each token a DISTINCT
   literal at its cfg-gated site so `scan_viewer_fail_seam_guard` (sharing
   `classify_cfg_gated_token`) structurally forbids ungated reads.
3. Extend `ViewerServer::start_inner` with two new fault flags (own / cached) and wire
   `start_viewer_with_failing_cached_peer_read` to set
   `OPENLORE_VIEWER_FAIL_CACHED_PEER_DIDS_READ` (replacing the `todo!()` scaffold),
   mirroring the slice-16 `fail_active_set_read` flag.

The release sibling of each seam is the identity function (no env read compiled in),
verified seam-free — exactly the slice-16 / ADR-026 discipline. The OBSERVABLE
degrade-TARGET (a cached-but-removed peer falling through to `NetworkUnfollowed` →
`peer add` when the cached read is absent) is ALSO independently exercisable today (a
soft-removed peer with no cached read IS a `NetworkUnfollowed` row — pinned by FF-3's
success path), so FF-9 pins specifically the read-FAILURE degrade, the one path that
needs the seam.
