# ADR-087: Subject Authority Proofs — Linked GitHub Owner (Person, User-Owned Repo) and an In-Repo Authority File (Any Repo), Re-Checkable by Any Reader from Public Data

- **Status**: Proposed (2026-10-14; revised after review iteration 1). **Conditional** on user confirmation of OOS-D4, OOS-D5 and
  OOS-D12/D13 (`docs/feature/owner-only-signing/design/wave-decisions.md`) and of ADR-084/085.
- **Date**: 2026-10-14
- **Deciders**: Morgan (nw-solution-architect), propose mode; Jeff Bailey to confirm
- **Feature**: owner-only-signing (DESIGN)
- **Companion**: ADR-086 (authority model and enforcement)
- **References**: ADR-063 (`GithubPort`), ADR-076 (bio matcher, review-app budget), ADR-084/085
  (identity link, local checks, freshness C-GDL-10), ADR-082 (separate purge port precedent),
  scraper WD-51 (public-data-only), ADR-023 (indexer has no GitHub capability)

## Context

ADR-086 needs a proof that a DID may speak for `github:<login>` or `github:<owner>/<repo>`. The
proof must be:

- re-checkable by **any reader** from public data (federation: a peer must not trust the signer's
  word or the signer's private token);
- obtainable within WD-51 (`adapter-github` calls only public endpoints; the review app uses one
  public-read PAT; the CLI an optional PAT);
- robust to renames, transfers, org ownership and later removal of rights.

GitHub API options evaluated (concretely):

| Option | What it proves | Who can call it | Re-checkable by any reader? |
|---|---|---|---|
| `GET /repos/{o}/{r}` → `owner.id`, `owner.type` (public, anonymous OK; also read inside `harvest_repo`) | The account that owns the repo. This is the only identity the Owner tier recognizes. Whether collaborators can also hold admin on a personal repo is not relied on (A-12): any admin other than the owner account, on any repo, is covered only by a file listing | Anyone | **Yes** |
| `GET /repos/{o}/{r}` with the **signer's own OAuth token** → `permissions.admin` / `maintain` | The token holder's role, incl. org repos | Only the token holder | **No** — signing-time only; needs a GitHub OAuth app, device flow in the CLI and a second OAuth in the review app |
| `GET /repos/{o}/{r}/collaborators/{user}/permission` → `permission`, `role_name` | Any user's role | Authenticated caller; GitHub documents the collaborators endpoints as requiring push access to the repo (assumed to apply here — **unverified**, A-9) | **No** for an arbitrary reader |
| `GET /orgs/{org}/memberships/{user}` → `role: admin` | Org owner | Org members / the user themself | **No** (public membership `GET /orgs/{org}/public_members/{u}` shows no role) |
| Repo topics | — | — | Impossible: topics are `[a-z0-9-]`, a DID contains `:` |
| DID in repo description/homepage (`GET /repos` body) | Whoever may edit "About" (admin; maintain is unverified) | Anyone | Yes, but repurposes a user-visible field and its edit rights are unverified |
| File in the default branch, e.g. `.github/openlore-authority`, read via `GET /repos/{o}/{r}/contents/{path}` | Someone with push access to the default branch put the DID there | Anyone | **Yes** |

## Decision

1. **Person authority** (`github:<login>`): the claim author DID `D` holds an ADR-084 identity link
   to GitHub id `X`, this reader's latest ADR-085 check of `(D, X)` is `verified`, and the subject
   login equals that **latest** check's `login_seen` (case-insensitive). Authority is current: once
   `X` is renamed, `github:<old-login>` may belong to another account, so claims about the old
   login read `Unattested(LoginNotCurrent)` (known gap G-2: after a rename the person re-signs under
   the new login; old claims stay visible as `third-party`). No other DID can hold authority over a
   person. A `Contested` id (the bio names several DIDs) gives each `verified` DID authority.
2. **Repo authority, tier `Owner`** (`github:<owner>/<repo>`): this reader's latest repo
   observation has `owner.type = User` and `owner.id = X`, where `D` holds person authority over `X`
   per rule 1 (minus the login match: the owner is identified by id). Org-owned repos can never pass
   this tier.
3. **Repo authority, tier `RepoListed`** (any repo, incl. org-owned; reader label `repo-listed`,
   deliberately not "admin"): the latest repo observation
   found the file **`.github/openlore-authority`** on the default branch, and the file names `D` as
   an exact DID token (the ADR-076 matcher hoisted to `claim-domain`, ADR-085 §1). File format:
   UTF-8 text, one DID per line, `#` starts a comment, ≤ 4 KiB; any other shape (symlink,
   submodule, directory, oversize, non-UTF-8) is `file_invalid` and grants nothing. No identity link
   is needed for this tier: the repo vouches for the DID, and the claim's ADR-071 provenance proves
   the DID side. **Semantics, stated plainly to the user: this tier means "listed in a file on the
   default branch", not "GitHub admin". Anyone with push access to the default branch can add their
   own DID, with no identity proof (residual risk R-1, part of the A-3 confirmation). CODEOWNERS
   only requests review; it blocks a push only when branch protection requires code-owner review,
   and readers cannot see branch protection (not public). Maintainers who want "admins only" must
   set that protection themselves.**
4. **Observations.** `GithubPort` gains `read_repo_authority(owner, repo) →
   RepoAuthorityObservation { repo_id, full_name_seen, owner_id, owner_type, authority_file:
   Absent | Present(Vec<did>) | Invalid }`: one `GET /repos/{o}/{r}` plus **always** one
   `GET /repos/{o}/{r}/contents/.github/openlore-authority`. The observation is therefore
   **signer-independent**: whoever records it, it holds the same facts, so the latest row never
   hides a listing from another DID (review H-2). 404 on the repo → `repo_not_found`. Rate limit /
   outage on either read → nothing recorded, state unchanged. Both endpoints are public (WD-51
   holds).
   **Redirects**: `build_client()` sets no redirect policy today, so `reqwest` follows redirects to
   any host. This feature adds an explicit policy to `adapter-github`: follow at most one redirect,
   same host only (`api.github.com` or the test base); anything else is `GithubUnavailable`. A
   renamed or transferred repo (301 to `/repositories/{id}`, A-10) then reports its current
   `full_name` and owner from the final body. The same policy serves ADR-085 §4.
5. **Local authority checks (never federated).** Migration **v8** `repo_authority_checks`,
   insert-only, `check_id` sequence PK, keyed for reads by the case-folded subject; columns per
   `architecture-design.md`. Latest = read query. Ports split as in ADR-085 §7:
   `RepoAuthorityCheckReadPort` (latest), `RepoAuthorityCheckWritePort` (insert only); purge is
   folded into ADR-085's `identity forget` precedent only if the user asks for local erasure of repo
   observations (public data; not required for v1). Person authority reuses ADR-085's
   `identity_link_checks` unchanged.
6. **Who refreshes.** `openlore authority verify [--subject <github:…>] [--mine]` (NEW) re-checks the
   authority of every non-exempt `github:` claim in the local store (own and peer), including the
   ADR-085 link checks it needs; per distinct repo subject 2 requests (repo + file) plus 1-2 per
   link check not already done in the same run; per person subject 1-2 (link check). It stops
   cleanly on rate limit, reporting what was checked and the reset time. `scrape github
   <owner/repo>` and `scrape person` record one observation per scraped repo (+2 requests on top
   of the harvest baseline of about 8 GETs per repo: `/repos`, Cargo.lock, `/tags`, CHANGELOG,
   `/readme`, docs, workflows, tests). The harvest's own `/repos` body is not reused: `harvest_repo`
   returns signals only, and widening it would couple the signal SSOT to authority. With `--sign`,
   these verbs also refresh the **signer's own** ADR-085 link check (1-2 requests) before gating,
   so they hold link-check write for the signer's DID only.
   **Budget**: anonymous 60/hour ≈ 6 repos scraped with authority per hour (was ≈ 7 without);
   with `GITHUB_TOKEN` (5000/hour) the budget is not a constraint. When the budget runs out
   mid-scrape, candidates already derived are still listed; repos without a fresh observation
   refuse `--sign` with "rate-limited; retry after <reset>" and nothing is recorded for them. The
   authority reads report through the existing `AuthReport` / `last_rate_budget` channel.
   The review app performs its own reads inside the publish request (C-GDL-10 freshness) and stores
   nothing in the CLI store.
7. **Time semantics.** Authority is current (ADR-086 §6). Rendered labels show the check age; age
   never downgrades a reader label; the **signing gate** requires a check no older than the
   freshness window (default 7 days, A-6).
8. **Indexer.** Unchanged: no GitHub capability; index results render "authority not checked here".

## Alternatives Considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **Signer's GitHub OAuth token, `permissions.admin` (Tier 3)** | Precise for org repos and true "admin". Not re-checkable by readers (a reader would trust the signer's statement), needs a GitHub OAuth app + device flow in the CLI + token storage, and leaves WD-51. | Rejected for v1; revisit if org admins refuse to commit the file |
| **Collaborator-permission endpoint with the reader's PAT** | Exact role for any user, but documented as requiring push access to the repo (A-9, unverified); arbitrary readers cannot call it. | Rejected |
| **Org owner membership** | Not publicly readable with role. | Rejected |
| **Topics / description / README** | Topics cannot hold a DID; description repurposes a visible field with unverified edit rights; README mixes proof with prose. | Rejected |
| **Bio check alone, no identity-link record, for person/owner tiers** | No dependency on GDL, but associates a DID with a GitHub account from a bio scan without the DID owner's published consent (contradicts ADR-084/GDL A-2) and pins no numeric id. | Rejected |
| **Pin `https://api.github.com/repositories/<id>` / user id in the claim's evidence** | Would let readers separate a reused name (repo deleted and recreated, or name freed by a transfer and re-taken) from the original repo. Without it, claims about the old repo and attested claims by the new owner share one subject key in the weight; the old ones read `third-party` (authority is current), so only the new owner's claims count, but `--include-unattested` shows both under one name. Costs: an additive ADR-064 evidence-layout change and an id read at sign time. | Rejected for v1 on cost; known gap G-1 (name reuse). Revisit if name reuse is observed |
| **New crate `authority-domain`** | Clean boundary; a sixth pure-core arm for a module that depends on `identity_link`. | Rejected; module in `claim-domain` beside `identity_link` |
| **Indexer re-verifies authority** | Gives the indexer root a GitHub capability and a shared rate budget (contradicts C-GDL-8). | Rejected for v1 |

## Consequences

- **Positive**: every proof is public and re-checkable by any peer; personal repos need nothing
  beyond the identity link; org repos get a maintainer-controlled delegation path that also lets an
  owner delegate to collaborators; renames and transfers resolve through the redirect and `owner.id`;
  removal of rights shows on the next check.
- **Negative**: "admin" is approximated by "listed in a file by someone with push access" (explicit
  user decision; residual risk R-1: self-listing by a write-role collaborator). One more endpoint
  on the `adapter-github` allowlist (contents of a fixed path), a new redirect policy, one
  migration (v8), two port traits, one verb. Anonymous budget: `authority verify` ≈ 15-30
  subjects per hour, a scrape with authority ≈ 6 repos per hour; a `GITHUB_TOKEN` is recommended.
  Org repos can sign only through the CLI in v1 (the review app scans owned repos only).
- **Privacy**: the authority file and owner ids are public data; observations stay local.

## Earned Trust

| Probe | Runs | Pass criterion | On failure |
|---|---|---|---|
| `repo_authority_checks` adapter `probe()` | CLI startup (wire → probe → use) | In a transaction: insert one sentinel at two `checked_at` values, read the later as latest, roll back | `health.startup.refused` |
| GitHub `/repos` and `/contents` shape | **Required** CI contract test over recorded fixtures `crates/test-support/fixtures/github/repo_owner_user.json`, `repo_owner_org.json`, `repo_renamed_redirect.json`, `authority_file.json`, `authority_file_symlink.json`; optional scheduled live smoke | `owner.id`, `owner.type`, `full_name`, `id` decode; contents `type`, `encoding`, `size`, `content` decode | CI red; at runtime a shape error is `GithubUnavailable`: signing gates fail closed, `authority verify` records nothing |
| Gate totality | CI property | `may_sign` permits iff exempt ∨ Attested within window | CI red |

`FakeGithub` lie catalogue: cross-host redirect (refused); repo renamed (301 to `/repositories/{id}`); transferred (owner changes
between reads); org-owned with no file; file only on a non-default branch (ignored: contents read
uses the default branch); file oversize / symlink / submodule / directory; DID prefix collision and
uppercase DID in the file; repo deleted (404); 403 rate limit; profile ~60 s stale (ADR-076 §6).
