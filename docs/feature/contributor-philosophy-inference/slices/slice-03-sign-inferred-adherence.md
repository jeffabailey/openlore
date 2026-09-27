# Slice 03 — Sign an inferred adherence (provenance travels with it)

> Story: US-CPI-003 (J-004 / J-004e, J-004c) · Persona: P-001 (Maria) · Depends on: slice-02
> Estimate: ~1 day · Completes the end-to-end person thread (scrape → link → infer → sign)

## Goal

`openlore infer people --sign N[,N…]` pre-fills the existing compose flow with the candidate
(subject `github:<login>`, predicate per OD-CPI-2, philosophy object, provenance, confidence), lets
Maria edit, and signs through the SAME pipeline as `claim add` / scraper `--sign`, with provenance
inside the signed payload (D-9).

## Learning hypothesis

- **Disproves if it fails**: that provenance can live in the signed claim without a Lexicon change
  (OD-CPI-5 evidence-array default) and still be read back by a peer. If the evidence encoding can't
  round-trip (CID stable, parseable after `peer pull`), DESIGN must switch to a reference-type change.
- **Confirms**: the human gate is unchanged — inference only pre-fills; the human signs.

## IN scope

- `--sign` on `infer people`; compose pre-fill; editable confidence in [0.0, 1.0]; out-of-range index
  rejected before compose.
- Provenance encoded in the signed payload (supporting claim refs + GitHub contributors URLs).
- Compose preview shows a `derived-from:` line listing supporting claims.

## OUT of scope

- Supersede/NEW/STRONGER (→ 04); auto-publish (publishing stays the existing separate step).

## Acceptance criteria (production data)

- [ ] On the slice-02 store, `openlore infer people --sign 1` with confidence edited to 0.45 signs
      `github:BurntSushi → memory-safety`; reading it back (`openlore graph query --subject
      github:BurntSushi`, or the stored signed record) shows both supporting claim refs and the two
      GitHub contributors sources for ripgrep and regex.
- [ ] Re-verifying the signed claim recomputes the identical CID.
- [ ] A second machine that `peer pull`s Maria's published claim sees the same provenance.
- [ ] `infer people` without `--sign` writes nothing; `--sign 7` with 3 candidates exits non-zero with
      "candidate 7 does not exist; valid range 1..3".

## Dogfood moment

Maria signs one inference about a maintainer she actually knows and reads it back via
`openlore graph query --subject github:<login>`.

## Dependencies

- slice-02; OD-CPI-2 (predicate) and OD-CPI-5 (provenance encoding) settled in DESIGN.
