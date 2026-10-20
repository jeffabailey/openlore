# Slice 04 — The review app's share draft quotes only claims whose authority holds

> Story: S-OOS-8 · Persona: P-003 Priya Raman · Job: J-009 (J-009f) · Infrastructure-only: **no**
> Depends on: ADR-089 (shipped), DESIGN ruling U-OOS-4 · Estimate: ~1 day
> Derived from DESIGN slice 4, **reduced to the share-draft rule**; no DISCUSS interviews.

## Why reduced

The rest of DESIGN slice 4 already shipped. The publish-time owner check shipped with
review-app-repos-and-accounts (ADR-088/089: `PublishPlan` needs a permit minted from fresh
in-request reads, tested in `review_app_repos_and_accounts_several.rs` and
`review_app_publish_permit_alone.rs`). The identity-link prompt is GDL slice 3. Only the share
draft is left (UC-21, `distill/wave-decisions.md` U-OOS-1).

## User-visible outcome

Priya published claims about `tidepool` and `kelp`, then transferred kelp to `tidal-labs`. When she
opens the share preview, the draft names only the tidepool claim (test-driven). Kelp's claim is
listed as "not counted: authority not proven". Nothing is posted until she confirms.

## IN scope

- The share preview reads each quoted repo's current owner and bio in the request, as publish
  does, and quotes only claims whose owner tier holds now (DWD-OOS-13).

## OUT of scope

- Publish-time check and link prompt (shipped). Org repos in the review app (CLI-only in v1).

## Acceptance (scenarios)

SA-1.

## Learning hypothesis

Whether up to 2 extra GitHub reads per quoted repo per preview fits the app's request budget. If
it does not, DESIGN must rule on caching under U-OOS-4.
