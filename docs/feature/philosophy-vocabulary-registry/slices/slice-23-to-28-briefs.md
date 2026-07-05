# slice briefs 23–28 · philosophy-vocabulary-registry

Compact briefs for the slices after the slice-22 discovery walking skeleton. Each
is ≤1 day, ships end-to-end, and is dogfoodable alone. Full ACs live in
`feature-delta.md` under the referenced US-PV story. (One brief per slice per the
carpaccio discipline; kept compact here — DESIGN/DELIVER expand the near ones.)

---

## slice-23 · philosophy-show  ·  US-PV-002  ·  `job_id: J-002`
- **Goal:** `openlore philosophy show <name|object>` prints one philosophy's full
  record (name, description, aliases, seeAlso) verbatim from the signed record.
- **IN:** the `show` verb; unknown-name → non-zero + plain guidance (no stack trace).
- **OUT:** editing; viewer; alias resolution behavior (that's slice-26).
- **Learning hypothesis:** confirms the record's description/aliases are the useful
  detail a user needs to *choose* a philosophy; disproves if `list` already suffices
  (then fold show into list).
- **Deps:** slice-22 (records + validator). **Effort:** ≤0.5 day (thin read verb).
- **Reference class:** `claim show`/`graph query --show`. **SPIKE:** none.

## slice-24 · philosophy-mint  ·  US-PV-003  ·  `job_id: J-001`
- **Goal:** `openlore philosophy add --name --description [--alias… --see-also…]`
  composes + SIGNS a new `org.openlore.philosophy` record (federated, open, no
  gatekeeper), mirroring the `claim add` sign flow; prints the new object id.
- **IN:** the `add` verb; sign prompt; local-first persist; seed-name collision
  refusal (AC-003.3); `validate_philosophy_json` reject on invalid (AC-003.4).
- **OUT:** publishing to PDS (deferrable like claims); moderation; viewer authoring.
- **Learning hypothesis:** confirms the "seeded but OPEN" model works — users extend
  the vocabulary and it federates like claims; disproves if minting causes
  fragmentation the aliases (slice-26) can't reconcile (→ reconsider open model).
- **Deps:** slice-22 + the slice-01 signing model. **Effort:** ≤1 day.
- **Reference class:** `claim add` (same compose→sign→persist). **SPIKE:** none.

## slice-25 · claim-compose-suggests-philosophy  ·  US-PV-004  ·  `job_id: J-001`
- **Goal:** `claim add` shows an ADVISORY line for `--object`: resolves a known/alias
  object to its canonical philosophy, or warns "not a known philosophy — will be
  signed as-is" — NEVER blocks (D3).
- **IN:** advisory resolution in the compose preview; the signed payload bytes are
  UNCHANGED by resolution (display-only, AC-004.3).
- **OUT:** rejecting unknown objects; changing the stored object; viewer compose.
- **Learning hypothesis:** confirms an advisory nudge is enough to steer users to
  shared objects without violating "claims not truth"; disproves if users ignore it
  (→ maybe stronger affordance, still non-blocking).
- **Deps:** slice-22 (known set) + slice-26 alias map if resolving aliases (or ship
  known-exact-match first, aliases in 26). **Effort:** ≤1 day.
- **Reference class:** the existing `claim add` compose preview. **SPIKE:** none.

## slice-26 · philosophy-alias-triangulation  ·  US-PV-005  ·  `job_id: J-002 / J-004`
- **Goal:** at read time, claims authored against a philosophy's `aliases` aggregate
  under its canonical object in `graph query`/`score` — near-synonyms connect.
- **IN:** a pure alias→canonical resolution used by the survey/score reads; grouped
  under the canonical philosophy, per-author attribution preserved (anti-merging).
- **OUT:** rewriting stored claim objects (immutable — AC-005.2); a global rename.
- **Learning hypothesis:** THE payoff test — confirms aliases deliver the
  triangulation the product promises (`mem-safety` + `memory-safety` = one view);
  disproves if alias sprawl makes canonicalization ambiguous (→ tighten alias rules).
- **Deps:** slice-22 (records carry aliases). **Effort:** ≤1 day (read-time derivation).
- **Reference class:** slice-20 bare-DID resolution (a read-time canonicalization over
  a stored variant). **SPIKE:** small probe on the resolution site in the survey read.

## slice-27 · viewer-philosophies-surface  ·  US-PV-006  ·  `job_id: J-002`
- **Goal:** read-only `GET /philosophies` viewer surface listing the vocabulary
  (name + description), each linking to the existing `/philosophy?object=` traversal;
  added to `LANDING_HUB_SURFACES` (reachable from the slice-21 persistent nav).
- **IN:** the read-only surface + nav entry; no authoring control (I-VIEW-1/3); offline.
- **OUT:** authoring/minting in the viewer; editing.
- **Learning hypothesis:** confirms discovery-where-you-look adds value over CLmI-only
  list; disproves if the CLI list already covers the need (then defer).
- **Deps:** slice-22 (records) + slice-21 (nav/`LANDING_HUB_SURFACES`). **Effort:** ≤1 day.
- **Reference class:** slice-15 `/peers` read-only list surface. **SPIKE:** none.

## slice-28 · scraper-uses-seeded-philosophies  ·  US-PV-007  ·  `job_id: J-004`
- **Goal:** the scraper's signal→predicate mapping references the SEEDED philosophy
  records (single source) so every proposed object is a known philosophy — kill the
  hardcoded literals + the stray `org.openlore.philosophy.mystery` drift.
- **IN:** re-point the mapping at the seeds; every proposed object `philosophy
  show`-resolves; a signal with no seeded philosophy is explicit (no drift string).
- **OUT:** new signals; auto-signing (human still signs — J-004c).
- **Learning hypothesis:** confirms the seeds are the right single source for
  proposals; disproves if the scraper needs philosophies the seeds lack (→ feeds the
  seed set / mint flow).
- **Deps:** slice-22 (seeds). **Effort:** ≤0.5 day (re-point a mapping). **SPIKE:** none.
- **Reference class:** slice-02 signal_predicate_mapping.
