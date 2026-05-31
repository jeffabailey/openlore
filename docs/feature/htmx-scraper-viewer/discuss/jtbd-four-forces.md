# JTBD Four Forces: htmx-scraper-viewer (slice-06)

The Four Forces shape whether the operator adopts the htmx viewer over the status quo
(raw SQL / CLI batch text). Push + Pull drive demand; Anxiety + Habit resist it. For both
jobs the dominant **anxiety** is "exposing a web surface that could accidentally write or
sign" — resolved by the **read-only + localhost** guardrails, which we surface explicitly.

---

## Job 1: See what is in my store

| Force | Description |
|-------|-------------|
| **Push** (drives away from status quo) | The operator is **blind to their own node's persisted contents without writing SQL.** To answer "what does my node hold?" today they must open a DuckDB shell and hand-write `SELECT` queries against `claims` and `peer_claims`, know the schema, and read raw tabular output. This is high-friction, error-prone, and discourages routine inspection — so the operator rarely checks, and uncertainty about node state accumulates. |
| **Pull** (attracts to new solution) | A browser page that renders the persisted store as a readable list — subject / predicate / object / evidence / confidence / author / CID — with **zero SQL**. Inspection becomes a glance, not a query-writing task. Works **fully offline** against local DuckDB (inherits slice-01 KPI-5 local-first). |
| **Anxiety** (resists new solution) | "Am I exposing a web surface that could be reached by something other than me?" / "Could a browser action accidentally **write to or sign against** my store?" / "Does running a web server put my **signing key** at risk?" → **Resolved by guardrails**: localhost-only personal dashboard, strictly **read-only** (no write/sign code paths), and the **web process never holds the signing key** (I-SCR-1 preserved). |
| **Habit** (inertia of current way) | The operator is comfortable in the terminal and may default to "I'll just SQL it." Mitigation: the viewer must be *faster and lower-effort than writing SQL* for the common "what's in my store" question, and must show the **same domain fields** the operator already reasons about (so no re-learning). |

### Strongest demand-reducing force
The **anxiety about a web surface causing writes/signing or exposing the key** is the
force most likely to block adoption. It is neutralized at the requirement level: read-only
NFR, localhost-only NFR, and the inherited I-SCR-1 / no-key-in-web-process invariant.

---

## Job 2: Browse scrape proposals in the browser

| Force | Description |
|-------|-------------|
| **Push** (drives away from status quo) | **Deciding which candidates matter is awkward in CLI batch text.** `scrape github <target>` emits a wall of proposed candidates as terminal text; scanning, comparing, and weighing them — especially the `derived-from` provenance per candidate — is cramped and easy to lose track of in a scroll-back buffer. |
| **Pull** (attracts to new solution) | Enter a target in the browser, see each `CandidateClaim` as a distinct, scannable HTML row with its display-only `derived-from` badge. Easier visual triage before the operator decides what to sign. Mirrors existing CLI output, so semantics are already familiar. |
| **Anxiety** (resists new solution) | Same core fear: "Could browsing proposals **accidentally sign or persist** a candidate?" / "Is the **key** involved in this web flow?" → **Resolved**: the live-scrape view renders proposals **only**; **signing stays in the CLI**; the web process triggers no sign pipeline and holds no key. Browsing has no persistent side effect (scraper persists NOTHING — candidates are in-memory and die with the request). |
| **Habit** (inertia of current way) | The operator already runs `scrape github <target>` in the terminal and reads candidates there. Mitigation: the browser view must present the **same candidate semantics** (same fields, same `derived-from` framing) so it augments rather than replaces the familiar flow; signing remains exactly where the operator already does it (CLI). |

### Strongest demand-reducing force
Identical to Job 1: anxiety that the web surface could sign/persist or expose the key.
Resolved by the read-only guardrail and the CLI-only signing boundary.

---

## Cross-job force summary

- **Shared push theme**: the operator's view of their node (held + proposable) is trapped
  behind low-legibility surfaces (SQL shell, batch CLI text).
- **Shared pull theme**: a legible, browser-rendered, read-only view of claims.
- **Shared anxiety theme**: web surface → accidental write/sign or key exposure.
  Neutralized identically for both jobs (read-only, localhost, no key in web process).
- **Shared habit theme**: terminal comfort. Neutralized by preserving domain semantics
  and keeping signing in the CLI.
