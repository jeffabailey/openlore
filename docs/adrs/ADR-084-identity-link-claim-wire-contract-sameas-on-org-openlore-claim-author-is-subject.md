# ADR-084: Identity-Link Claim Wire Contract — `sameAs` on `org.openlore.claim`, Author Is Subject, Numeric GitHub Id in `evidence[]`

- **Status**: Accepted (2026-10-14, user approved as proposed). The Decision text is the
  recommended options of GDL-D1..D5 (`docs/feature/github-did-identity-link/design/wave-decisions.md`).
  Revised by ADR-086/087 (owner-only-signing): a link is also the proof of signing authority (see
  ADR-085 §6).
- **Date**: 2026-10-14
- **Deciders**: Morgan (nw-solution-architect), propose mode; Jeff Bailey (approved)
- **Feature**: github-did-identity-link (DESIGN; no DISCUSS wave — requirements are assumptions A-1..A-9)
- **Realizes**: OD-CPI-1 deferral ("DID linking = a later, opt-in, human-signed `sameAs`-style claim")
- **References**: ADR-005 (Lexicon), ADR-006 (CID), ADR-008 (retraction), ADR-064 (person subject
  `github:<login>`), ADR-070 (basis points), ADR-071 (self-attested provenance), ADR-073 (create-only
  OAuth scopes), ADR-076 (bio-DID ownership proof)

## Context

People in OpenLore are `github:<login>` subjects (ADR-064). A contributor who also has an ATProto
DID has no way to say "this GitHub account is me". The review app already proves, privately, that a
GitHub account belongs to a signed-in DID (the DID as an exact token in the GitHub bio, re-checked
before every scrape, numeric id pinned; ADR-076). Nothing about that proof is public or federated.

Constraints that shape the wire contract:

1. Consent: a link must be opt-in and human-made by the DID owner. Linking someone else's GitHub
   account to your DID, or someone else's DID to a GitHub account, must not produce a link.
2. Both sides must be provable: the DID side by the record's provenance (app signature, or the
   author's own repo per ADR-071); the GitHub side by something only the GitHub account holder can
   write (the bio, ADR-076).
3. GitHub logins are renamed and re-registered; the numeric user id is stable.
4. The review app's OAuth grant is `repo:org.openlore.claim?action=create` (SPIKE-3). A new
   collection needs a new scope and a re-consent from every user.
5. Every shipped reader (peer pull, indexer ingest, viewer, search, retraction rule) already
   handles `org.openlore.claim` with both provenance modes.

## Decision

1. **Record**: an ordinary `org.openlore.claim`. No Lexicon change, no new collection, no CID path
   change.
2. **Shape** (the only shape readers project as an identity link):

   | Field | Value |
   |---|---|
   | `subject` | the author's **bare DID** (`did:plc:…` / `did:web:…`) |
   | `predicate` | `sameAs` (schema.org sense: "a page that unambiguously identifies the same entity"; not OWL transitive closure) |
   | `object` | `github:<login>` — the login as GitHub served it at link time (case preserved; compared case-insensitively, ADR-064) |
   | `evidence` | exactly, in order: `https://github.com/<login>` (where the bio proof lives), `https://api.github.com/user/<numeric-id>` (the stable id, as a URI) |
   | `confidence` | `10000` basis points (an identity assertion, not an opinion) |
   | `author` | app-signed `did#org.openlore.application` **or** self-attested bare DID (ADR-071) |
   | `references` | empty; a re-link after a rename MAY carry `supersedes` → the earlier link CID |

3. **Self-assertion rule** (pure, enforced by readers, not by the Lexicon): a claim is an identity
   link **iff** its ADR-071 provenance verdict passed (app-signed, or self-attested fetched from
   the author's own PDS) ∧ predicate is `sameAs` ∧ `bare_did(author) == subject` ∧ object parses as
   `github:<valid login>` ∧ evidence holds exactly one `https://api.github.com/user/<digits>` ∧
   evidence holds `https://github.com/<login>` whose login equals the object login
   (case-insensitive). Being a link is not being associated: association needs the reader's own passing bio check
   (ADR-085 §5).
   Anything else with predicate `sameAs` is an ordinary claim: displayed as authored, never used to
   associate identities. A third party cannot create a link; there is no "asserted by someone else"
   link state.
4. **The GitHub side is never proven by the record.** The record carries consent (DID side) and the
   pinned id; the GitHub side is proven only by a reader's own live bio check (ADR-085). The
   identity verbs refuse to build a link unless a bio check passes at compose time, and **every
   other OpenLore composer** (`claim add`, `claim counter`, `scrape github --sign`,
   `infer people --sign`, review-app suggestion publish) refuses predicate `sameAs` with a pointer
   to `openlore identity link` — one pure guard in the shared compose path
   (`claim_domain::identity_link::is_reserved_predicate`). A foreign client can still sign a
   well-shaped `sameAs` without a check; readers then show it as an unverified assertion (ADR-085
   §5), never an association.
5. **Retraction** = the shipped ADR-008 `retracts` counter-claim by the same author (CLI
   `claim retract`; the review app's `RetractPlan`). Nothing is ever deleted; nothing is
   auto-retracted when the bio proof disappears.

## Alternatives Considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **New record type `org.openlore.identityLink`** (typed fields `did`, `githubLogin`, `githubId`) | Schema-enforced shape and no free-string convention. But: a new collection needs a new OAuth scope and re-consent for every review-app user (ADR-073); every reader (peer pull, indexer listRecords, viewer, retraction rule, peer_claims schema) needs a second record path; old peers ignore it entirely. | Rejected — cost is a parallel pipeline for a 2-field fact |
| **Reverse direction**: subject `github:<login>`, object DID (ADR-064 subject convention) | `graph query --subject github:<login>` would list the link next to adherence claims. But the self-assertion rule becomes "object == author", which every existing reader reads as a claim *about the GitHub account by someone else*; and scoring would pair it under the person's subject. | Rejected — "the author speaks about itself" is the consent invariant; subject == author states it directly |
| **DID document `alsoKnownAs`** (PLC operation adding `https://github.com/<login>`) | The ATProto-native place for aliases. But it needs a PLC rotation-key operation most bsky.social users cannot perform, it is reserved in practice for the handle, and OpenLore holds no PLC key (ADR-002 derived key only). | Rejected for v1; a reader MAY additionally honor it later |
| **Subject `github-id:<n>`** | Rename-proof but unreadable; diverges from ADR-064. The id is still carried, in `evidence[]`. | Rejected (same as ADR-064) |
| **Predicate `controlsAccount`** | Directional and honest about what is proven. Not the vocabulary OD-CPI-1 committed to; no other reader distinction gained. | Viable alternative — choose if the user prefers it (one constant) |

## Consequences

- **Positive**: zero Lexicon/canonicalization/validator change; the review app needs no new OAuth
  scope; app-signed and self-attested links both work through unchanged verify/ingest paths;
  retraction is the shipped rule; the numeric id is signed, so renames are detectable by any peer.
- **Negative**: the link shape is a convention over free strings (like ADR-064 provenance). A
  malformed `sameAs` claim is silently "not a link"; DISTILL must cover that path. The signed login
  is historical after a rename (the id is authoritative).
- **Scoring impact**: `score()` groups by (subject, object) with no predicate filter today; a link
  claim would become a pairing. ADR-085 §6 makes the scorer drop every `sameAs` claim.
- **Id URI dependency**: if `https://api.github.com/user/<id>` ever proves unsuitable (A-7), the
  fallback `github-id:<n>` is a change to the §3 rule (readers must accept both forms), not a
  free-string substitution.
- **Privacy**: a published link is public and permanent in the ATProto sense (retraction marks it
  withdrawn; copies may persist). The link preview must say so.

## Earned Trust

The link codec is a pure `encode`/`project` pair in `claim-domain::identity_link` with a round-trip
property (encode a proven link → canonicalize → CID → decode → project returns the same DID,
login and id) and a negative property (any mutation of subject, author, predicate or the id URI
makes `project` return "not a link"). A CI fixture publishes a self-attested link to `FakePds`,
re-reads it through `peer pull`, and asserts the CID equals the rkey and the projection is intact.
