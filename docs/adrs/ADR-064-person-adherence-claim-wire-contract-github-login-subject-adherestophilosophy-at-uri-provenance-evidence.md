# ADR-064: Person-Adherence Claim Wire Contract — `github:<login>` Subject, `adheresToPhilosophy` Predicate, AT-URI Provenance in `evidence[]`

- **Status**: Proposed (2026-09-27)
- **Date**: 2026-09-27
- **Deciders**: Morgan (nw-solution-architect), recording the USER-LOCKED OD-CPI-1, OD-CPI-2 and
  OD-CPI-5 (accepted 2026-09-27) for `contributor-philosophy-inference`; DoD item 8.
- **Feature**: contributor-philosophy-inference
- **Companion**: ADR-063 (internal component architecture).
- **References**: ADR-005 (Lexicon), ADR-006 (CID), ADR-008 (reference types), ADR-059
  (philosophy vocabulary), ADR-018 (candidate model; WD-62 display-only `derived-from`).

## Context

Signed inferred adherence claims federate (publish, peer pull, opaque instances). A peer must be
able to audit WHY a person was said to hold a philosophy without the signer's local link table
(D-9), and every supporting claim must stay attributed to ITS author (D-7). The shipped
`org.openlore.claim` Lexicon already has: free-string `subject`/`predicate`/`object`,
`evidence: string[]`, `references[]` with `type ∈ {retracts, corrects, counters, supersedes}`.
No predicate allowlist exists anywhere in the claim pipeline (verified: `lexicon::validate_claim_json`,
`claim-domain`, `appview-domain::ingest`).

## Decision

1. **Subject** = `github:<login>` using the login exactly as GitHub's `/contributors` returns it
   (case-preserved for display; `github:` subjects compare case-insensitively, GitHub logins
   being case-insensitive). The stable numeric GitHub user id is stored LOCALLY on the
   contribution link only (rename detection) — it is NOT part of the signed payload. DID linking
   is deferred (future opt-in, human-signed `sameAs`-style claim).
2. **Predicate** = `adheresToPhilosophy` (person-level; distinct from artifact-level
   `embodiesPhilosophy`). "Inferred" is NOT encoded in the predicate: hand-authored and inferred
   adherence claims are the same predicate so they triangulate. **Object** = a philosophy NSID
   (`org.openlore.philosophy.*`), so the ADR-059 compose advisory applies unchanged.
3. **Provenance = ordinary `evidence[]` strings** (no Lexicon change), deterministic order:
   for each supporting repo in case-folded subject order — first each supporting claim as its
   canonical AT-URI `at://<author-did>/org.openlore.claim/<cid>` (bare DID, fragment stripped —
   the same form `claim publish` mints; encodes BOTH the claim CID (D-9) and its author (D-7)),
   then the person-specific public source `https://github.com/<owner>/<repo>/commits?author=<login>`.
   No entry contains a comma (the compose editor's evidence separator). The human may edit the
   list before signing; whatever is signed is the provenance.
4. **Supersession** = a STRONGER re-sign carries one `references[]` entry
   `{type: "supersedes", cid: <earlier inferred claim CID>}` (ADR-008, shipped). The earlier claim
   is never modified; the system never emits `retracts`/`counters` on the user's behalf.
5. **Reading provenance back** (for STRONGER / SUPPORT WEAKENED): an adherence claim's "cited
   support" = the evidence entries that parse as `at://…/org.openlore.claim/<cid>`. A claim with
   none is treated as hand-authored (never proposed for supersession).
6. The scraper's WD-62 / I-SCR-7 rule (the `derived-from` line is DISPLAY-ONLY) is UNCHANGED for
   repo candidates. For person candidates the preview shows a `derived-from` line too, and the
   provenance ALSO lives in `evidence[]` because D-9 requires it in the signed payload.

## Alternatives Considered

- **New reference type `derivedFrom`** for supporting claims (OD-CPI-5 alt). Semantically crisp,
  but a Lexicon enum change: every shipped validator (`ALLOWED_REFERENCE_TYPES`, the two DuckDB
  `CHECK (ref_type IN …)` constraints, `decode_reference`) REJECTS unknown types — old peers
  would refuse the claim. REJECTED (user-locked to evidence).
- **Bare `cid:<cid>` evidence entries**. Shorter, but loses the author (D-7) and is not a
  resolvable URI. REJECTED.
- **Repo-level source `…/graphs/contributors`**. Proves the repo has contributors, not that THIS
  person does. REJECTED in favor of the person-specific `commits?author=` URL.
- **Predicate `inferredAdherence`**. Splits hand-authored vs inferred into non-triangulating
  predicates. REJECTED (OD-CPI-2).
- **Subject = numeric id (`github-id:<n>`)**. Rename-proof but unreadable and diverges from the
  shipped `github:owner/repo` convention. REJECTED (OD-CPI-1).

## Consequences

- **Positive**: zero Lexicon / canonicalization / validator change; CID deterministic and stable
  across re-verification; a peer can resolve every supporting claim (by at-uri) and re-verify the
  contribution on GitHub; anti-merging holds (each supporting claim names its own author).
- **Negative**: provenance is a CONVENTION over free strings, not schema-enforced — a hand-edited
  or foreign client may sign malformed entries (treated as "cites nothing", i.e. hand-authored).
  A GitHub login rename leaves old signed claims on the old subject (detected and displayed
  locally via the stored user id; never auto-merged).
- **Federation**: consumers that ignore unknown predicates are unaffected; `graph query`,
  scoring and the viewer read these claims as ordinary claims.

## Earned Trust

The provenance codec is a pure encode/parse pair in `scraper-domain` with a round-trip property
test (encode → parse yields exactly the supporting CIDs + authors, for arbitrary DIDs/CIDs/logins),
and a CI fixture signs an inferred claim, re-reads it from disk, recomputes the CID and asserts the
evidence entries are byte-identical (KPI-CPI-2).
