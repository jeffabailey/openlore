# Wizard decisions — bluesky-claim-review-app

- **Idea:** Sign in with Bluesky (ATProto OAuth), infer philosophy claims, approve/disapprove each.
- **Approve** writes a self-attested `org.openlore.claim` into the user's own PDS.
- **Disapprove (resolved 2026-10-03, D-4):** declines stay **private**. They are kept as
  app-side state only, used solely to suppress re-suggestion, and **never** written to the
  user's PDS or anywhere public. There is no public counter-claim or rejection record.
- **Provenance (resolved 2026-10-03, D-5):** approved self-attested claims rely on the **PDS
  repo commit signature**. There is no app-level `#org.openlore.application` signature and no
  PLC doc change for Bluesky users. Consequence for DESIGN: readers, verifiers and the viewer
  (today `crates/claim-domain/src/verify.rs` expects the app-level signature) must accept and
  distinguish a second provenance mode, "self-attested (repo-signed)", and never render these
  claims as unverified or reject them.
- **v1 source:** GitHub only (reuse `scrape person` and signal detection). Bluesky-post
  inference is deferred (needs a text classifier).
- **Ownership proof (resolved 2026-10-03, D-3):** the signed-in user's **DID in their GitHub
  profile bio** (an exact match). Not the handle (handles change) and not a gist in v1. Gist
  and other methods are possible later alternatives. Proof is per signed-in DID. No scrape and
  no suggestions until verified.
- **Ownership re-check (resolved 2026-10-03, D-12):** re-verified **before every scrape**. On
  failure: no scrape, no new suggestions, the link is marked unverified, and an actionable
  message is shown. Approved claims are kept untouched. Pending private suggestions are hidden
  (not deleted) until re-verified.
- **Privacy (reaffirmed, I-BRA-1):** unapproved suggestions stay private until approved. There
  is no PDS write, share post, AppView, search, feed or profile exposure, and no other user can
  see them. Only the authenticated owner sees their queue.
- **Bluesky visibility (resolved 2026-10-03, D-11):** the **share post** (`app.bsky.feed.post`
  linking to the profile page) is **in v1, Release 1**, strictly opt-in. It is previewed and
  explicitly confirmed, declining has no side effects, and it reflects approved claims only.
  The feed generator and labeler come LATER (J-010).
- **Classification:** user-facing, cross-cutting (OAuth, hosting, PDS writes); brownfield.
- **Starting wave:** /nw:discuss (rough idea, explore). DISCUSS artifacts are in `discuss/`.
