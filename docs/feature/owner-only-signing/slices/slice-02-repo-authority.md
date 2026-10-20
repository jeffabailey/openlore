# Slice 02 — Only a repo's owner or listed maintainers sign about it; anyone imports

> Stories: S-OOS-4, S-OOS-5, S-OOS-6, S-OOS-3 (repo checks) · Persona: P-001 (Jeff)
> Jobs: J-011, J-004 (J-004c), J-003 (J-003a), J-001 · Infrastructure-only: **no**
> Depends on: slice 01, GDL slice 1 (owner tier only; the repo-listed tier works without a link)
> Estimate: ~5 days (three stories; split by story if needed)
> Derived from DESIGN slice 2; no DISCUSS interviews.

## User-visible outcome

`openlore scrape github jeffbailey/tidepool --sign 1` signs and publishes, because Jeff's linked
account owns the repo. `openlore scrape github rachelk/kelp --sign 1` lists the candidates, signs
nothing, and prints who can sign and the command they would run. For `acme-tools/anvil`, Jeff signs
only if `.github/openlore-authority` on the default branch lists his DID exactly.
`openlore authority verify` labels pulled repo claims `owner`, `repo-listed` or `third-party` from
public data. When GitHub cannot answer, it records nothing. `openlore claim publish <cid>`
refuses a never-published claim that has no permit.

## IN scope

- `GithubPort::read_repo_authority` (repo read + authority-file read, at most 1 same-host
  redirect).
- v8 `repo_authority_checks` (insert-only) and the new `openlore authority verify [--subject]` verb.
- Gate in `scrape github/person --sign`, `claim add` (repo subjects) and `claim publish`
  (`published_at IS NULL`).
- Offline repo signing from fresh checks. Repo not found.

## OUT of scope

- Reader labels in `graph query`, viewer and search, and `score_attested` (slice 3).

## Acceptance (scenarios)

RA-1..RA-15 (incl. RA-11a/b), CORE-OOS-3.

## Learning hypothesis

KPI-OOS-4: at least one `repo-listed` claim on a real org repo and one `owner` claim, each
confirmed by a second reader running `authority verify`. If no maintainer adds the file, the
repo-listed tier is not a usable stand-in for GitHub's admin role (OOS-D5 option B is the
fallback).
