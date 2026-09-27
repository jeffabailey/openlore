# Slice 04 — New repos grow the inference; signed claims never change

> Story: US-CPI-004 (J-004 / J-004d, J-004e) · Persona: P-001 (Maria) · Depends on: slice-03
> Estimate: ~1 day

## Goal

The "as new repositories are scraped" half of the user's intent: after a scrape that changes
inference inputs, print a one-line count of new/stronger inferred candidates; in `infer people` label
candidates `[NEW]` / `[STRONGER]` (signing a STRONGER one creates a NEW claim with a `supersedes`
reference) and flag signed inferred claims whose support was retracted as `[SUPPORT WEAKENED]`. The
system never edits, re-signs, retracts, or deletes a signed claim (D-5).

## Learning hypothesis

- **Disproves if it fails**: that append-only supersession is understandable in the CLI. If Maria
  can't tell which claim is current after superseding, the append-only UX needs a "current view"
  before this ships further.
- **Confirms**: evidence evolution is visible without ever mutating the record (KPI-CPI-4).

## IN scope

- End-of-scrape hint (only when inputs changed; silent otherwise).
- NEW / STRONGER / SUPPORT WEAKENED labels; STRONGER signing adds `supersedes <old-cid>`.
- Byte-identical guarantee for all pre-existing signed claims.

## OUT of scope

- Auto-retract/auto-counter; hiding superseded claims in `graph query` (existing behavior unchanged).

## Acceptance criteria (production data)

- [ ] With Maria's signed `github:BurntSushi → memory-safety` (1 supporting repo), she scrapes
      `rust-lang/regex` and signs a `memory-safety` candidate for it; the scrape prints the hint and
      `infer people` shows BurntSushi memory-safety as STRONGER (2 repos).
- [ ] Signing the STRONGER candidate produces a new CID referencing the old one as superseded; the old
      claim file/row is byte-identical before and after.
- [ ] Signing a different philosophy for regex surfaces `[NEW]` for BurntSushi.
- [ ] Soft-retracting a supporting claim marks the dependent signed inferred claim SUPPORT WEAKENED;
      nothing is retracted automatically.
- [ ] A scrape that changes no inputs prints no hint.

## Dogfood moment

Maria scrapes one more repo from a maintainer she follows and sees the person picture update.

## Dependencies

- slice-03; shipped `ReferenceType::Supersedes` and soft-retraction.
