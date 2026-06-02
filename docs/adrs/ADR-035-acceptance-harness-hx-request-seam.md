# ADR-035: Acceptance-Harness Seam for Driving Both Shapes — `get_htmx` / `post_form_htmx` Set `HX-Request`

- **Status**: Accepted / shipped (slice-07 viewer-htmx-swaps, DELIVER 2026-06-02). Seam materialized in DISTILL/DELIVER (the `get_htmx` / `post_form_htmx` + `is_fragment` / `is_full_page` / `references_external_cdn` harness seams).
- **Date**: 2026-06-02
- **Deciders**: Morgan (nw-solution-architect), per OD-HX-6 for viewer-htmx-swaps (slice-07).
- **Feature**: viewer-htmx-swaps (slice-07)
- **Extends**: the slice-06 `ViewerServer` HTTP acceptance harness (`tests/acceptance/support/mod.rs`).
- **Resolves**: OD-HX-6 (how the harness sends/withholds `HX-Request` to drive both shapes against the real `openlore ui`).

## Context

slice-07's acceptance contract is: drive each route WITH and WITHOUT the `HX-Request`
header against the REAL `openlore ui` and assert the fragment shape (with) vs the complete
full page (without). The slice-06 harness `ViewerServer` already spawns the real binary and
exposes `get(path) -> ViewerResponse` and `post_form(path, fields) -> ViewerResponse`, each
building a fresh `reqwest::blocking::Client` request and setting NO `HX-Request` header — so
the existing methods already model the NO-header (full-page) path. What is missing is a
header-SETTING variant for the htmx (fragment) path.

This is a TEST convention only — zero production impact (the production handler reads the
header per ADR-033; the harness just chooses to send it or not).

## Decision

**Add two methods to `ViewerServer`, mirroring the existing `get` / `post_form` but adding
the `HX-Request` header, so a scenario can drive either shape against the same running
viewer:**

```text
/// GET <base><path> WITH `HX-Request: true` — drives the htmx FRAGMENT shape
/// (ADR-033). Mirror of `get`; the only delta is the header.
pub fn get_htmx(&self, path: &str) -> ViewerResponse {
    reqwest::blocking::Client::new()
        .get(format!("{}{}", self.base_url, path))
        .header("HX-Request", "true")
        .timeout(Duration::from_secs(10))
        .send()...
}

/// POST <base><path> form WITH `HX-Request: true` — drives the htmx scrape-results
/// FRAGMENT shape (US-HX-003). Mirror of `post_form`; the only delta is the header.
pub fn post_form_htmx(&self, path: &str, fields: &[(&str,&str)]) -> ViewerResponse { ... }
```

- The existing `get` / `post_form` (no header) REMAIN the full-page/no-JS-path drivers —
  they already model "HX-Request absent" exactly (curl/bookmark/no-JS). No change to them,
  so the slice-06 26-scenario suite that uses them stays byte-identical (I-HX-4).
- The header VALUE is `"true"` (what htmx sends); ADR-033 keys on PRESENCE, so the exact
  value is not load-bearing, but `"true"` matches real htmx for fidelity.
- For the offline/no-CDN property (US-HX-005), the harness asserts on the rendered HTML of
  full-page responses that no page references an off-host htmx URL, and (DELIVER) exercises
  the swap path with the network down. The asset is fetched at `GET /static/htmx.min.js` —
  the harness can `get("/static/htmx.min.js")` and assert `200` + a JS content-type to pin
  the asset route.
- The harness sends raw HTTP and does NOT run a JS engine — so it tests the SERVER contract
  (header → shape, ids present, parity), which is exactly the observable surface I-HX-1/5
  live on. The browser-side swap behavior (no flash/scroll) is a UX property verified
  manually / out of scope for the HTTP harness (the BDD scenarios assert the server-side
  shape that ENABLES it).

## Alternatives Considered

| Option | Evaluation | Rejected because |
|--------|-----------|------------------|
| **A `headers: &[(&str,&str)]` parameter on the existing `get`/`post_form`** | One method, fully general. | **Rejected (call-site clarity + slice-06 churn).** Changing the existing signatures would touch every slice-06 call site (no-regression churn) and make the common no-header path verbose. Two named methods (`get`/`get_htmx`) read as "the two shapes" at the call site — the slice-07 ubiquitous language. |
| **A boolean `get(path, htmx: bool)`** | One method. | **Rejected.** Boolean-parameter blindness at call sites (`get(p, true)` is opaque); `get_htmx(p)` names the intent. |
| **Drive a headless browser (real htmx execution)** | Tests the actual swap + history. | **Rejected for the AT harness (scope/cost).** The server contract (header→shape, id agreement, parity, no-CDN) is the testable invariant surface; a headless browser adds heavy infra to test client-side htmx (a vendored, mature library) rather than OUR code. The no-flash/no-scroll UX is verified manually. |

## Consequences

### Positive
- Scenarios drive BOTH shapes against the real binary with one extra method pair; the
  with/without-header contract (I-HX-1) is directly exercisable.
- Slice-06 methods unchanged → the slice-06 suite stays green by construction (I-HX-4); the
  no-header methods ARE the no-JS-path drivers.
- Reads as the slice-07 ubiquitous language: `get` = full page, `get_htmx` = fragment.

### Negative
- The HTTP harness cannot assert the browser-side in-place feel (no flash/scroll reset) —
  that is a UX property outside raw HTTP. Accepted: the harness pins the server contract
  that enables it; the in-place feel is a manual/visual check.

## Revisit Trigger
- A scenario needs to send additional htmx request headers (`HX-Target`, `HX-Current-URL`)
  → generalize into one `get_with_headers` helper that `get_htmx` delegates to.
- The team adopts a headless-browser acceptance lane → add it alongside (not replacing) the
  HTTP harness for the UX-feel assertions.
