# ADR-076: GitHub Ownership Proof Matching and a Proportionate, In-Process Rate Budget

- **Status**: Accepted (2026-10-04) — implemented; see `docs/evolution/bluesky-claim-review-app-evolution.md` (proposed 2026-10-04)
- **Date**: 2026-10-04
- **Deciders**: Jeff Bailey (D-3, D-12; "simple, proportionate, no queues"), Morgan (nw-solution-architect)
- **Feature**: bluesky-claim-review-app (DESIGN), resolves OD-BRA-3, OD-BRA-10 and OD-BRA-12

## Context

The app scans GitHub server-side for several users. Unauthenticated GitHub REST allows 60
requests per hour per IP, and a token allows 5,000 per hour. One scan of 10 owned repos costs
2 requests (profile and repo list) plus about 8 per repo for `harvest_repo` (around 80 in total;
DELIVER confirms the exact count from `adapter-github`). D-3 and D-12 require the signed-in DID to
appear in the GitHub bio as an exact token, checked immediately before every scrape. The scale is
under 50 users on a single process.

## Decision

1. **One server-side GitHub token.**
   - A fine-grained PAT with **no repository permissions** (public read only), stored as an SSM
     SecureString.
   - It is passed to the existing `adapter-github` through its existing optional-token path
     (WD-63), so there is no adapter change.
   - Its rate-limit and no-token-leak probe arms are reused.
   - Per-user GitHub OAuth is not used in v1.
2. **The budget lives in the process, with pure arithmetic** (`review-domain`, the clock injected):
   - **Global**:
     - Before each repo, the scan reads the last observed `X-RateLimit-Remaining`. Below a floor of
       300 it pauses, keeping its partial results, and shows "GitHub is busy, resuming at HH:MM"
       (AC-003.7).
     - At most **2 concurrent scans**.
   - **Per DID**:
     - 1 concurrent scan;
     - 6 scans per day;
     - 20 verify attempts per hour;
     - 100 publishes per day;
     - 5 share posts per day (OD-BRA-12).
     - The numbers are configuration; DEVOPS tunes them.
   - **Per IP**: 20 sign-in starts per 10 minutes.
   - The state is in-memory and resets on restart. That is acceptable, because the limits are
     abuse controls, not billing.
3. **Scans run as in-process background tasks.** There is no queue or worker infrastructure.
   - Each repo's suggestions are persisted as soon as that repo completes, so progress is resumable.
   - The `scan_runs` row records progress for htmx polling.
   - On startup, a run left `running` becomes `interrupted` and is offered for resume.
4. **Ownership verdict (pure, OD-BRA-10)**:
   - **Tokenizing.** The bio is scanned for DID-shaped tokens: the maximal runs matching
     `did:[a-z]+:[A-Za-z0-9._:%-]+`, with trailing sentence punctuation (`.,;:!?)]}>`) stripped.
   - **Matching.** A token matches when it is **byte-equal** to the signed-in DID. There is no case
     folding, because `did:plc` is lowercase by spec, and no substring match. For example,
     `did:plc:abc` does not match inside `did:plc:abcd`.
   - **Several DIDs** in a bio are allowed; only the signed-in one counts.
   - **Verdicts.** `Verified`, `DidMissing`, `DifferentDid(first_found)`, `NoBio`,
     `AccountNotFound`, `RateLimited(retry_after)`, and `IdentityChanged`. `IdentityChanged`
     means the stored numeric GitHub id no longer matches the login, for example after a rename.
     The user re-verifies.
   - The stored link carries the **numeric GitHub user id** as well as the login.
5. **Re-verification gate (I-BRA-4).** The scan task is a pipeline whose first stage is
   `read_person` followed by the ownership verdict.
   - **Only `Verified` yields the capability value the next stage needs**, a `VerifiedOwnership`
     token. Without it, repo listing cannot be expressed.
   - **On any other verdict:**
     - the link becomes `unverified`;
     - pending suggestions become hidden by derivation (ADR-074);
     - nothing is deleted.
6. **Freshness caveat.** GitHub may serve a profile up to about 60 seconds stale. A `DidMissing`
   verdict within 2 minutes of the user opening the instructions adds "If you just edited your
   bio, wait a minute and retry."

## Alternatives considered

| Alternative | Verdict |
|---|---|
| **Per-user GitHub OAuth** | Rejected for v1. It adds a second OAuth integration and a stored GitHub credential per user for public data. It would prove ownership more strongly, so it remains a later alternative (D-3). |
| **A job queue (Redis/SQS) + workers** | Rejected. It is disproportionate for under 50 users and adds operations cost. In-process tasks plus persisted progress cover restarts. |
| **Unauthenticated access** | Rejected. 60 requests per hour per IP allows less than one full scan per hour for the whole service. |
| **A GitHub App installation token** | Rejected. Its higher limits are unnecessary, and it needs app registration and key management. |
| **A substring match for the DID** | Rejected. It is the prefix-collision class (`did:plc:abc` matching inside `did:plc:abcd`) that OD-BRA-10 warns about. |

## Consequences

- **Positive**:
  - About 50 full scans per hour at most, against an expected handful per day.
  - No new infrastructure.
  - The I-BRA-4 gate is a type-level capability, not a convention.
- **Negative**:
  - All users share one token, so a burst from one user can delay others. The per-DID limits
    bound this.
  - In-memory limits reset when the app restarts.
- **Revisit triggers**: more than 50 scans per hour sustained, or a need for private-repo signals.
  Either would mean per-user GitHub OAuth.
