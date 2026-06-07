# Journey (visual): At-a-glance counter flags on the claims list

> Persona: **P-001 "Maria"** (node operator, counter-claim-scanner hat)
> Job: **J-003b** (counter-claim as first-class disagreement — the AT-A-GLANCE facet)
> Surface: read-only `openlore ui` viewer, `GET /claims` list (slice-06), extended

## Emotional arc

```
[Trigger]        [Scan list]      [Spot flags]     [Drill into thread]   [Goal]
peer pull /      "which of these  "ah — these two  "open the contested   triaged:
authored a       did anyone       got pushback"    claim, read who &     "I know what
counter          push back on?"                    why" (slice-11)        is contested"
  |                |                |                 |                     |
Feels:           Feels:           Feels:            Feels:                Feels:
mild unease →    scanning,        relief +          engaged, in          confident,
"did anyone      slightly         orientation:      control: I chose     un-ambushed:
respond to my    blind today     "no surprise       which thread to      "the list is
claims?"                         ambush"            read                 honest & mine"
Artifacts:       Artifacts:       Artifacts:        Artifacts:            Artifacts:
counter in       ClaimRow page +  COUNTERED_        /claims/{cid}         (no state
claim_refs ∪     counter_presence PRESENCE_FLAG     slice-11 thread       change — read
peer_claim_refs  _for(page CIDs)  per countered row link                  only)
```

Emotional pattern: **Problem Relief** (mild unease about un-noticed disagreement →
orientation while scanning → relief: contested claims are visible without hunting →
confidence: I triage, the system never reorders for me).

## ASCII flow

```
                  GET /claims  (Maria scans her own claims)
                        |
        +---------------v-----------------+
        | list_claims(page)               |  slice-06 — UNCHANGED ordering/paging
        | -> [ClaimRow; <=50]             |
        +---------------+-----------------+
                        |  page CIDs
        +---------------v-----------------+
        | counter_presence_for(&[cid])    |  NEW — ONE aggregate query (no N+1)
        | INDEXED claim_references        |  LOCAL only, no artifact read
        |   UNION ALL peer_claim_refs     |  WHERE referenced_cid IN (...)
        |   ref_type='counters', DISTINCT |  -> {countered CIDs}
        +---------------+-----------------+
                        |  presence set
        +---------------v-----------------+
        | ClaimRowView { ..., is_countered}|  PURE projection: presence.contains(cid)
        | render_claim_row                |  flag ONLY when is_countered
        +---------------+-----------------+
                        |
          +-------------v--------------+
          | row: subj pred obj 0.90 CID|  un-countered -> no marker (slice-06 verbatim)
          | row: subj pred obj 0.90 CID  [Countered]->/claims/{cid}   (countered)
          +----------------------------+
```

## HTML mockup — the `/claims` list with flags (full page, no-JS shape)

```
+-- OpenLore — My Claims ----------------------------------------------------+
| This is a read-only view of the claims you have signed.                    |
| [ My Claims ] [ Peer Claims ]                                              |
| <div id="view-panel"> <div id="claims-table">                             |
|  +-----------------------+-------------------+-----------+------+--------+  |
|  | Subject               | Predicate         | Object    | Conf | CID    |  |
|  +-----------------------+-------------------+-----------+------+--------+  |
|  | github:rust-lang/cargo| embodiesPhilosophy| memory-…  | 0.90 | bafyM… | [Countered] -> /claims/bafyMariaRust
|  | github:…/docs-tool    | embodiesPhilosophy| doc-first | 0.90 | bafyM… |  |   (un-countered: no marker)
|  | github:…/semver-lib   | embodiesPhilosophy| semver    | 0.30 | bafyM… | [Countered] -> /claims/bafyMariaSemver
|  +-----------------------+-------------------+-----------+------+--------+  |
|  1–3 of 3                                                                  |
| </div> </div>                                                             |
| <script src="/static/htmx.min.js"></script>                               |
+----------------------------------------------------------------------------+
```

The `[Countered]` marker is the slice-11 `COUNTERED_PRESENCE_FLAG = "Countered"`
neutral text, rendered as a one-hop `<a href="/claims/{cid}">` link. The order
(`composed_at DESC, cid`), paging, count (`1–3 of 3`), and the `0.30` confidence are
byte-identical to slice-06 — the flag is additive only.

## Error / boundary paths

- **No counters on the page** → presence set empty → no markers → renders exactly as
  slice-06 (no noise).
- **Many counters on one claim** → ONE neutral marker (presence-only), never a count
  or "disputed by N".
- **Peer counter from a since-purged peer** → not in `peer_claim_references` → row not
  flagged (J-003c residue-free, by construction).
- **Network down** → presence read is LOCAL → flags still render (offline).
- **Store read failure on the presence read** → degrade to no-flags (the list still
  renders; never a stack trace), mirroring the slice-06 degrade-to-empty pattern.
