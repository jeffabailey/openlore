# Slice 02 — Release-time auto-bump: keep the formula in sync so `brew upgrade` works

> Release 2 · Story: US-HB-002 (user-visible) · Job: J-006 · Persona: P-001 (Maria, upgrade path)
> Estimate: ~1 day

## Goal

Make the tap self-maintaining: a `release.yml` step that, on each `v*` tag and **after** the 4
tarballs + `.sha256` are uploaded, regenerates `Formula/openlore.rb`'s `version` + 4 `sha256` from
the published `.sha256` companions and commits it to `main` (trunk-based, per house rule) — plus a
CI smoke test that `brew install`s + runs `openlore --version` on each of the 4 triples so a bad
bump fails the release, not the user (D-6). Result: `brew upgrade openlore` fetches each new,
sha256-verified release with zero manual formula edits.

## Learning hypothesis

If the release pipeline regenerates + commits the formula automatically after artifact upload, and
a per-triple `brew install` smoke test gates it, then the tap will always reflect the latest `v*`
release, `brew upgrade openlore` will work every time, and no broken formula will ever reach a
user — validating **OD-HB-2** (the auto-bump mechanism) as the correct, low-maintenance approach.

## IN scope

- A new post-upload step in `.github/workflows/release.yml` that: reads the 4 published `.sha256`
  values, templates `Formula/openlore.rb` with the new `version` + 4 `url`s + 4 `sha256`s, and
  commits it to `main` (OD-HB-2 default (a); no PR, per trunk-based house rule).
- Strict ordering: the bump step runs only after all 4 tarballs + `.sha256` are uploaded (never
  points the formula at an unpublished tarball).
- A CI smoke test job: `brew install` the formula + run `openlore --version` on each of the 4
  triples; failure blocks the release.
- A freshness assertion: `Formula/openlore.rb` `version` == the `v*` tag (KPI-HB-3).

## OUT of scope

- The formula itself (delivered in slice 01).
- Any change to binary production / the 4-tarball matrix (reused unchanged, D-7).
- Auto-updater / silent upgrade of installed users — bumping the formula only changes what a
  *future* explicit `brew upgrade` fetches (D-5).
- homebrew-core submission, cosign `.sig` in the formula, ghcr bottles (all OUT).

## Acceptance criteria (from US-HB-002 UAT)

- [ ] After a `v*` release, `Formula/openlore.rb` reflects the new `version` + 4 `sha256` (from the
      published `.sha256` companions) with no manual edit (D-6).
- [ ] The bump step runs strictly after the tarballs + `.sha256` are uploaded; it never references
      an unpublished tarball (ordering guard).
- [ ] A CI smoke test `brew install`s + runs `openlore --version` on each of the 4 triples and
      blocks the release on failure.
- [ ] `brew update && brew upgrade openlore` fetches the newly-released, sha256-verified version.
- [ ] Bumping the formula upgrades no already-installed user automatically and triggers no
      phone-home (D-5).
- [ ] A freshness assertion confirms formula `version` == the `v*` tag (KPI-HB-3).

## Dependencies

- US-HB-001 (a correct formula to keep in sync) — slice 01.
- The shipped `release.yml` tarball + `.sha256` matrix (ADR-011) — the bump step consumes its
  outputs.
- **OD-HB-2 (settle first)**: the exact bump mechanism (in-workflow commit vs PR vs dispatch);
  lean default = in-`release.yml` commit-to-main after upload (Risk R-1: ordering/race).

## Estimate

~1 day: the bump step is a small templating + commit; the ordering guard and per-triple smoke test
are the bulk; no application code changes.
