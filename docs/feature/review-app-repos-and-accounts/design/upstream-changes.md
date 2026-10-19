# Upstream Changes: review-app-repos-and-accounts (DESIGN)

DESIGN changes or sharpens these DISCUSS (and earlier DESIGN) assumptions. The source files are
not edited; each change is quoted and stated here.

## UC-RRA-1 — Default ticks and the limit (BR-RRA-3): exact arithmetic

DISCUSS (`user-stories.md`, BR-RRA-3):

> The ticked repos across all accounts (saved or default ticks) plus the imported repos may total
> at most `${scan_repo_cap}`. [...] Default ticks fill only the room left: an account without a
> saved choice gets its best repos ticked (up to 5), in the order accounts were added, until the
> limit is reached

Both sentences hold, but "default ticks count" and "default ticks fill only the room left" can be
read two ways at Save or import time. DESIGN fixes one reading (architecture-design §6.3): a Save
or import counts the **other** accounts' default ticks **as they are shown before the change**.
Consequences: with 10 default ticks shown, an import is refused (US-RRA-005 example 5 holds);
saving 8 on a second account while the first shows 5 default ticks is refused with "You have 13
chosen"; unticking frees room that other accounts' default ticks may then fill. Saved ticks of an
unverified account still count (they return on re-verify).

## UC-RRA-2 — NFR-RRA-4 scan cost includes one listing per account

DISCUSS (`user-stories.md`, NFR-RRA-4):

> A scan costs at most one profile read per linked account plus the per-repo harvest (≈10
> requests per repo).

The scan also needs one owned-repo listing per passing account (it exists today, to rank the
top 5; now also to match saved ticks by id, skip archived or transferred repos and find BR-RRA-5
moves). DESIGN budget: one profile read + one listing per linked account + ≈10 per planned repo.

## UC-RRA-3 — "Unsigned" is decided by a fresh read, not by the stored tag

DISCUSS (`wave-decisions.md`, D-RRA-2):

> Its suggestions are unsigned, private to the owner, and never publishable unless a verified
> account of the owner owns [...] the repo.

DESIGN (ADR-089 §5): publishing is decided only by the permit from the fresh repo-owner and bio
reads. The stored source tag (`imported`) decides the card (Decline only) and visibility. So a
forged request for an imported suggestion whose repo one of her listed, verified accounts truly
owns *now* (before the BR-RRA-5 move ran) would pass the gate. That matches BR-RRA-1/2 as written
(ownership, not the tag). If the user wants the tag to also block, it is a one-line extra check;
listed as an open question.

## UC-RRA-4 — Shared publish gate with owner-only-signing (OQ-6)

owner-only-signing ADR-086 §4:

> The review app's `PublishPlan` execution takes a permit built in the publish request (extends
> `owner_can_publish`).

ADR-087 §1-2 (owner tier) assumes the signer holds a **published, verified** identity link. The
review app's BR-RRA-2 accepts an account on the **private** list with a bio read in the request.
DESIGN delivers the owner tier of `claim_domain::authority` now (ADR-089) with the evidence being
a same-request observation. owner-only-signing then adds local checks (CLI) and the repo-listed
tier to the same module. Reader labels are unchanged: other readers see a review-app claim as
`owner` only once the author publishes the link.

## UC-RRA-5 — `GithubPort::read_repo` is the base of OOS `read_repo_authority`

owner-only-signing brief entry: `GithubPort::read_repo_authority` reads `GET /repos/{o}/{r}` and
always the authority file. DESIGN adds `read_repo` (repo facts only, one request). OOS's method
becomes `read_repo` plus the file read; the redirect policy (≤ 1, same host) is shared.

## UC-RRA-6 — Store and its location

The DESIGN prompt named `crates/adapter-duckdb` for the review-app store. The review app's
private store is `crates/adapter-review-store` over `review-app.duckdb` (ADR-074); `adapter-duckdb`
is the CLI's store and stays forbidden to the review app. All store changes are in
`adapter-review-store`. The bluesky-claim-review-app design now lives in
`docs/architecture/bluesky-claim-review-app/`, not under `docs/feature/.../design/`.

`data-models.md` §3.1 of that design ("`github_links` … `owner_did` PK", visibility
"`state='pending' AND github_links.status='verified'`") is superseded by ADR-088.

## UC-RRA-7 — KPI events refined (OQ-10)

DISCUSS `outcome-kpis.md` lists `suggestion.acted.outside_default` and asks for a restriction to
non-first accounts (US-RRA-004) and to imports (US-RRA-005). DESIGN stores the reason on the
suggestion (`outside_default`: `owned_choice` | `other_account` | `imported`) and adds
`suggestion.acted.outside_default.other_account` and `.imported`. `import.refused.<code>` is
expanded to a closed set. `publish.administered` is not added (R4 deferred).

## UC-RRA-8 — BR-RRA-4 "deleted after the scan" realized as "never inserted"

DISCUSS (BR-RRA-4): "Removing an account or an imported repo during a scan stops reads of it at
once, and its pending suggestions are deleted after the scan (D-RRA-17)." DESIGN: removal deletes
at once, and the scan's insert for that source is refused in the same transaction. The observable
outcome (no pending row for the removed source once the scan ends) is the same.
