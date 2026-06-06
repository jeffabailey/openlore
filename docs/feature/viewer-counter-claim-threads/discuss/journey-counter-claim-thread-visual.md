# Journey (visual): Reading the disagreement around a claim — slice-11

> Persona: **P-001 "Maria"** (node operator, counter-claim-reader hat)
> Goal: drill into a claim on the local `openlore ui` viewer → SEE the disagreement
> (who countered it, with what CID, and why) → understand BOTH sides — without the
> disagreement ever changing the claim.
> Job: **J-003b** (VIEW half). Authoring stays the slice-03 CLI.

## Emotional arc

Pattern: **Uncertainty → Informed → Sovereign-confident** (a Discovery/Confidence
blend).

```
Maria opens a claim        Sees it is countered,        Reads both sides; the claim
in the browser             reads the verbatim reasons   is untouched; she decides
   |                          |                            |
 Uncertain               -> Informed                  -> Sovereign-confident
 "Is anyone disputing       "Maria/Tobias countered      "I can see the disagreement
  this? I can't tell."       this, here's exactly why."   in full; the system didn't
                                                          pick a winner. My call."
```

No jarring transition: the flag (disputed) arrives WITH the thread that explains it,
so "this is disputed" is never a dead-end anxiety — it immediately resolves into
"...and here is who/why".

## ASCII flow

```
[Trigger: Maria opens /claims/{cid}]
        |
        v
  +-----------------------------+        un-countered
  | GET /claims/{cid}           |---------------------> [claim alone, as slice-06/07]
  | (existing route, extended)  |                        Feels: clean, no noise
  +-----------------------------+
        |  countered (query_counter_claims non-empty)
        v
  +-------------------------------------------------------+
  | Claim rendered VERBATIM (confidence 0.91 UNCHANGED)   |
  | + "Countered" flag (neutral presence marker)          |
  | + Counter-claims thread:                              |
  |     - did:plc:maria-test (you)  cid bafy...new        |
  |         reason: "Cargo's dependency pinning is..."     |
  |     - did:plc:tobias-test       cid bafy...t0bi       |
  |         reason: "Pinning here is CI policy, not..."    |
  +-------------------------------------------------------+
        |
        v
  [Maria reads both sides] -> decides: trust / cite / counter (via CLI)
   Feels: Sovereign-confident
```

## TUI/HTML mockup — countered claim (full page, no-JS shape)

```
+-- GET /claims/bafy...n4ka -------------------------------------------------+
| OpenLore — Claim Detail                                                    |
| Read-only viewer. Authoring happens in the CLI.                            |
|                                                                            |
|  [ Countered ]   <-- neutral presence flag; NOT a score/verdict           |
|                                                                            |
|  Subject     github:rust-lang/cargo                                        |
|  Predicate   embodiesPhilosophy                                            |
|  Object      org.openlore.philosophy.dependency-pinning                    |
|  Confidence  0.91            <-- VERBATIM, UNCHANGED by the counters       |
|  Author      did:plc:rachel-test                                          |
|  Composed at 2026-05-22T09:18:44Z                                          |
|  CID         bafy...n4ka                                                   |
|                                                                            |
|  Evidence                                                                  |
|   - https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html |
|                                                                            |
|  Counter-claims                                                            |
|   - Author  did:plc:maria-test (you)                                       |
|     CID     bafy...new        <-- links to /claims/bafy...new             |
|     Reason  Cargo's dependency pinning is opt-in, not philosophical;       |
|             pinning is a tool, not a value.                                |
|                                                                            |
|   - Author  did:plc:tobias-test                                           |
|     CID     bafy...t0bi       <-- links to /claims/bafy...t0bi            |
|     Reason  Pinning here is CI policy, not a stated value of the project.  |
|                                                                            |
|  Back to My Claims                                                         |
+----------------------------------------------------------------------------+
```

Notes on the mockup:
- The original claim block is BYTE-FOR-BYTE the slice-06/07 render. The counters are
  appended BELOW; nothing above changes (shown-never-applied, I-CT-2).
- Two counters = two items, each under its own author DID + CID (anti-merging, I-CT-3).
  There is NO "disputed by 2" or "net confidence" line anywhere.
- Confidence `0.91` is verbatim (I-CT-4). No counter has re-weighted it.

## TUI/HTML mockup — un-countered claim (no noise)

```
+-- GET /claims/bafy...solo -------------------------------------------------+
| OpenLore — Claim Detail                                                    |
| Read-only viewer. Authoring happens in the CLI.                            |
|                                                                            |
|  Subject     github:openlore/openlore                                      |
|  Predicate   embodiesPhilosophy                                            |
|  Object      org.openlore.philosophy.memory-safety                         |
|  Confidence  0.75                                                          |
|  Author      did:plc:maria-test                                          |
|  Composed at 2026-05-26T12:00:00Z                                          |
|  CID         bafy...solo                                                   |
|                                                                            |
|  Evidence                                                                  |
|   - https://example.org/evidence/1                                         |
|                                                                            |
|  Back to My Claims                                                         |
+----------------------------------------------------------------------------+
       ^ NO "Counter-claims" section. NO "0 counters". NO empty noise.
```

## TUI/HTML mockup — counter with no reason (boundary)

```
|  Counter-claims                                                            |
|   - Author  did:plc:someone-else                                          |
|     CID     bafy...blank                                                   |
|     Reason  (no reason provided)   <-- explicit state, never a blank line  |
```

A peer record may satisfy the Lexicon yet carry an empty `reason` (ADR-015: optional
at wire, required only at the OpenLore `claim counter` verb). The viewer renders an
explicit "no reason provided" state — never a crash, never a blank line that reads as
a render bug.

## Failure / edge modes (feed DISTILL error-scenario generation)

- **Un-countered claim** → render the claim alone, no counter section (US-CT-003).
- **Counter with empty reason** (non-OpenLore client) → explicit "no reason provided".
- **Counter from a purged peer** → it lived in `peer_claims`; after `peer remove
  --purge` it is simply absent from the thread (no dangling/ghost item) — consistent
  with J-003c, no special-case code needed.
- **Unknown CID** → the existing guided 404 (slice-07) is unchanged; no thread/flag.
- **Network down** → the thread still renders (local read); only the vendored htmx
  asset is referenced (I-CT-5).
- **Store unreadable** → the existing guided error (NFR-VIEW-6) carries through; no
  raw stack trace.
