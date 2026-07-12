# Upstream Changes — back-propagation from DESIGN to DISCUSS

> Feature: homebrew-binary-distribution · Wave: DESIGN · Author: Morgan (nw-solution-architect)
> Date: 2026-07-12
>
> Per the back-propagation contract, DESIGN records here (not by editing DISCUSS content in place)
> any DISCUSS assumption found false during design. A corresponding `## Wave: DESIGN / [REF] Changed
> Assumptions` section was appended to `feature-delta.md` (CA-1).

## UC-1 (BLOCKING) — DISCUSS assumed a shipped release pipeline that does not exist

### The DISCUSS assumption (quoted verbatim)

From `feature-delta.md` header (lines 8-9):

> "It CONSUMES the already-shipped `release.yml` GitHub-Release tarball + `.sha256` pipeline
> (ADR-011, `openlore-foundation/devops/distribution.md` §1.2); it reinvents no binary production."

From `feature-delta.md` D-7 (line 94):

> "**Consumes the shipped release matrix; reinvents nothing.** The feature depends on `release.yml`'s
> existing 4 tarballs + `.sha256` companions (ADR-011)."

From `feature-delta.md` Pre-requisites (lines 539-543):

> "### Pre-requisites (shipped, inherited)
> - `release.yml` publishing, per `v*` tag, the 4 tarballs … each with a `.sha256` companion
>   (ADR-011, `distribution.md` §1.2).
> - At least one published `v*` GitHub Release for the slice-1 formula to point at."

### What DESIGN found (verified)

The pipeline **does not exist**:

- There is **no** `.github/workflows/release.yml`. The workflows present are only `ci.yml`
  (commit + acceptance on the Rust workspace) and `nightly.yml` (advisory mutation testing).
- `nightly.yml` line 6 states in-code: *"release.yml lands when the first vX.Y.Z tag is cut — out of
  scope for slice-01 step 06-08."*
- There are **no git tags** and **no GitHub Releases** in the repository.
- `docs/product/architecture/brief.md` has **no** release/homebrew/distribution section.

### The corrected assumption

- **ADR-011 nonetheless LOCKS the output contract** — the 4 target triples, the tarball naming
  `openlore-{version}-{triple}.tar.gz`, the `.sha256` companions, plus cosign `.sig` + SBOM
  (ADR-012). This is a stable contract to design against even though the producer does not exist.
- Therefore: **the formula (`Formula/openlore.rb`) and the release-time autobump are DESIGNED NOW
  against ADR-011's locked contract**, but **`release.yml` producing that 4-platform matrix (native
  4-platform builds + cosign + SBOM) is a BLOCKING EXTERNAL PREREQUISITE — a separate future DEVOPS
  feature, NOT part of homebrew-binary-distribution.**
- **Build split (design now, split the build):** the two DISCUSS slices remain valid designs but
  become *executable* only after the prerequisite ships:
  - **slice-01** (dogfood install) cannot run until `release.yml` + **one real tagged `v*` release
    with all 4 tarballs + `.sha256`** exist — there is nothing for the formula's `url`/`sha256` to
    point at until then.
  - **slice-02** (autobump) is designed to **EXTEND the future `release.yml`** (a `bump-formula`
    job + a `formula-smoke` job sequenced after its upload job), so it literally cannot exist until
    `release.yml` does.

### Impact on locked DISCUSS decisions

- **D-7 is re-scoped** (not reversed): the 4-tarball matrix is *contractually locked* (ADR-011) but
  *not yet produced*. The dependency is a **prerequisite to build**, not an **inherited given**.
- **D-1..D-6 are unchanged** — all remain valid; they simply cannot be *executed* until the
  prerequisite lands. The design work (formula shape, tap resolution, autobump mechanism,
  verification model, guardrails) is complete and correct against the locked contract.

### Recommended action for the orchestrator / product owner

1. Raise (or schedule) the **`release.yml` DEVOPS feature** (native 4-platform release builds +
   cosign + SBOM per ADR-011/ADR-012) as an explicit predecessor to homebrew-binary-distribution.
2. Cut at least **one real `v*` tag/release** through it.
3. Only then move homebrew-binary-distribution slice-01 → slice-02 into DELIVER.

Tracked as **OQ-D-1 (BLOCKING)** in the feature-delta DESIGN Open Questions and recorded in
ADR-061 (Consequences → Negative, and Open Questions).
