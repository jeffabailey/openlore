<!-- markdownlint-disable MD024 -->

# User Stories — openlore-foundation (slice-01)

All stories in this file belong to **slice-01-claim-skeleton** (the walking skeleton).
Every story carries a `job_id` traceable to `docs/product/jobs.yaml` per Decision 1.
Stories 001-004 carry mandatory Elevator Pitches; US-005 is `@infrastructure` and
carries an `infrastructure_rationale` instead.

## System Constraints

- **Local-first**: every flow must remain functional with the network offline up to
  the publication boundary. Publishing is the only step that requires network access.
- **Solution-neutral**: stories describe user-observable behavior. The choice between
  DuckDB / Kùzu / SurrealDB is reserved for DESIGN, except where the SSOT explicitly
  fixes it (slice-01 commits to DuckDB to keep the walking skeleton concrete; slice-04
  re-opens the question).
- **Claims-not-truth invariant**: no UI surface in any slice may present a claim
  with language that frames it as a truth assertion. The literal text "not as truth"
  in the compose preview is load-bearing for slice-01.
- **Attribution-preserving**: every claim shown anywhere must show its author DID.
  No "merged consensus" rendering across authors, ever.
- **Retraction without deletion**: claims are retracted by counter-claim per RC-02
  default. Local store may garbage-collect, but the PDS-published record persists.
- **CLI-first**: the CLI is the canonical interface for all slice-01 functionality.
  No UI surface in slice-01.

---

## US-001: Author a single signed claim from the CLI

### Job link

- `job_id`: J-001 (Author a signed philosophical claim)

### Elevator Pitch

- **Before**: I have a structured opinion about a project's philosophy (e.g. "Rust embodies memory-safety as a first-class value") and the only way to publish it is a blog post no one can query.
- **After**: I run `openlore claim add --subject github:rust-lang/rust --predicate embodiesPhilosophy --object org.openlore.philosophy.memory-safety --evidence https://www.rust-lang.org/ --confidence 0.86`, see the composed record with the literal text "not as truth", press Enter, and the CLI prints my claim's CID. Total time: under 30 seconds.
- **Decision enabled**: I can now publish a queryable, attributable, signed philosophical claim without it feeling like I'm asserting truth — which means I'll actually publish opinions I currently self-censor.

### Problem

Jeff (P-001) is a senior engineer building solo who has structured opinions about
which projects embody which philosophies and wants to publish them as queryable,
attributable claims. He finds it costly to publish those opinions today because
blog posts feel like assertions of truth that invite combative responses, and
unstructured prose is unqueryable.

### Who

- Senior Engineer Solo Builder (P-001) on a personal workstation
- Authenticated against their existing ATProto identity
- Comfortable with CLI flags and content-addressed identifiers

### Solution

A `openlore claim add` CLI command that captures (subject, predicate, object, evidence,
confidence) as a structured record, previews it with the literal "not as truth"
framing, and signs on confirmation. The signed record gets a stable CID and is
persisted locally; publication is a separate explicit step (US-003).

### Domain Examples

#### Example 1 (Happy Path)

Jeff Bailey (`did:plc:jeff-test`) wants to publicly claim that Rust embodies
memory-safety. He runs:

```
openlore claim add \
  --subject   github:rust-lang/rust \
  --predicate embodiesPhilosophy \
  --object    org.openlore.philosophy.memory-safety \
  --evidence  https://www.rust-lang.org/ \
  --confidence 0.86
```

The CLI prints the composed record with the literal text "not as truth", waits
on a prompt, and signs on `<Enter>`. Total elapsed time: ~20 seconds.

#### Example 2 (Edge / Confidence Boundary)

Maria Lopez (`did:plc:maria-test`) is less sure: she wants to claim that Mastodon
embodies federation-first as a philosophy, but with confidence 0.55 (only
"weighted", not "well-evidenced"). She runs the same command with
`--confidence 0.55`. The preview displays `0.55 (weighted)`. She signs. The
display bucket is informational only; the signed numeric value is 0.55.

#### Example 3 (Error / Invalid Input)

Aanya Krishnan (`did:plc:aanya-test`) accidentally enters `--confidence 1.4`.
The CLI rejects pre-sign with: `error: --confidence must be in [0.0, 1.0]; got 1.4`.
No file is written, no PDS call is made. She corrects to `0.86` and reruns.

### UAT Scenarios (BDD)

```gherkin
Scenario: Compose preview includes the "not as truth" framing
  Given Jeff has authenticated as did:plc:jeff-test
  When Jeff runs `openlore claim add` with subject github:rust-lang/rust,
       predicate embodiesPhilosophy,
       object org.openlore.philosophy.memory-safety,
       evidence https://www.rust-lang.org/,
       confidence 0.86
  Then the CLI prints a composed record block
  And that block contains the literal text "not as truth"
  And the CLI waits on a confirmation prompt
  And no file has been written under ~/.local/share/openlore/
  And no network call has been made

Scenario: Out-of-range confidence is rejected pre-sign
  Given Jeff has authenticated as did:plc:jeff-test
  When Jeff runs `openlore claim add` with --confidence 1.4 (other flags valid)
  Then the CLI exits with a non-zero status
  And the error message names the flag and the valid range [0.0, 1.0]
  And no file has been written
  And no network call has been made

Scenario: Confidence bucket label is display-only
  Given Jeff composes a claim with --confidence 0.55
  When the CLI prints the preview
  Then the preview shows "0.55 (weighted)"
  And the signed claim contains the numeric value 0.55
  And the signed claim does not contain the bucket label
```

### Acceptance Criteria

- [ ] Compose preview is printed before any signing, persistence, or network I/O.
- [ ] Preview contains the literal text "not as truth".
- [ ] Confidence outside `[0.0, 1.0]` exits non-zero with a useful error and no side effects.
- [ ] Confidence bucket label appears only in display; signed payload stores the numeric value.
- [ ] Author DID in the preview matches the identity resolved at session start.

### Outcome KPIs

See `outcome-kpis.md` KPI-1 and KPI-2 below.

### Technical Notes

- Identity resolution: assumes US-005 (Lexicon + identity bootstrap) is in place.
- Lexicon `org.openlore.claim` field shape is defined in US-005; DESIGN owns canonical schema.
- "Not as truth" text is content-frozen by AC; any rewording requires re-validation against KPI-3.

---

## US-002: Sign and persist a claim locally before any publication

### Job link

- `job_id`: J-001

### Elevator Pitch

- **Before**: When I "submit" an opinion to a tool, it goes straight to the network and I cannot inspect what landed where.
- **After**: I press Enter on the compose preview, see `Signing with did:plc:jeff-test ... ok` and `Computing claim CID ... bafy...`, and find a 412-byte file at `~/.local/share/openlore/claims/<cid>.json` whose signature verifies against my DID — all without a single network call yet.
- **Decision enabled**: I decide whether and when to publish, separately from whether to sign. I always own my signed claim locally first.

### Problem

Jeff wants a hard separation between "this is signed and mine" and "this is on
the network." Tools that conflate these force him across a federated boundary
before he's psychologically committed.

### Who

- Senior Engineer Solo Builder (P-001)
- Just composed a claim via US-001

### Solution

`<Enter>` at the compose prompt signs the canonicalized claim with the author's
key, computes the content-addressed CID, and writes the signed JSON to
`~/.local/share/openlore/claims/<cid>.json`. No network I/O occurs in this step.
A publish prompt follows but is the next story.

### Domain Examples

#### Example 1 (Happy Path)

Jeff confirms. The CLI prints:

```
Signing with did:plc:jeff-test ... ok
Computing claim CID            ... bafyreigh2akiscaildc...n4ka

Written to local store:
  path : ~/.local/share/openlore/claims/bafyreigh2akiscaildc...n4ka.json
  cid  : bafyreigh2akiscaildc...n4ka
  size : 412 bytes
```

He opens the file. The contents match the preview field-for-field plus the signature block.

#### Example 2 (Edge / Re-run determinism)

Aanya re-runs the same `openlore claim add` command with the same flags after
deleting the local file. The resulting CID matches the previous run byte-for-byte.

#### Example 3 (Error / Locked keychain)

Maria's signing key is in a locked keychain. On `<Enter>`, the CLI prompts to
unlock. On cancel, the CLI exits cleanly with no file written and no PDS call.

### UAT Scenarios (BDD)

```gherkin
Scenario: Signing produces a verifiable local file with no network call
  Given Jeff has composed a valid claim and authenticated as did:plc:jeff-test
  When Jeff confirms the sign prompt
  Then a file appears under ~/.local/share/openlore/claims/ named <cid>.json
  And the file's signature verifies against did:plc:jeff-test's public key
  And no outbound HTTP request to a PDS has occurred

Scenario: Re-canonicalization produces identical CIDs
  Given Jeff has just signed a claim with cid bafy...n4ka
  When Jeff deletes the local file and runs the identical `openlore claim add` command
  Then the new run produces a file with cid bafy...n4ka byte-for-byte

Scenario: Locked-keychain cancel leaves no side effects
  Given Maria's signing key requires keychain unlock
  When Maria cancels the unlock prompt
  Then the CLI exits non-zero
  And no file has been written under ~/.local/share/openlore/claims/
  And no network call has been made
```

### Acceptance Criteria

- [ ] Signed claim is written atomically (no half-written file possible).
- [ ] File path is `~/.local/share/openlore/claims/<cid>.json` (XDG-respecting).
- [ ] Signature verifies against the author DID's public key.
- [ ] No network I/O occurs during sign-and-persist.
- [ ] Re-canonicalization produces identical CIDs across runs and machines.

### Outcome KPIs

KPI-4 (round-trip identity) and KPI-5 (local-first invariant).

### Technical Notes

- Atomic write via write-to-tmp + rename.
- DuckDB used for indexed query in US-004; file-based store in this story is the
  canonical artifact. DESIGN may collapse to DuckDB-only if it preserves the
  signature-verifiability guarantee.

---

## US-003: Publish a signed claim to the author's PDS

### Job link

- `job_id`: J-001

### Elevator Pitch

- **Before**: Publishing means committing across a federated boundary with no way back.
- **After**: I press Y at the publish prompt, the CLI prints `Published. at-uri: at://did:plc:jeff-test/org.openlore.claim/<cid>`, and immediately reminds me `Tip: openlore claim retract <cid> to issue a retraction claim`.
- **Decision enabled**: I decide to federate this claim knowing retraction is one command away — which is exactly what makes me willing to publish opinions I currently self-censor.

### Problem

Federation is psychologically expensive when retraction is unclear. Jeff will
not publish if he cannot see, at publish-time, how to take it back.

### Who

- Senior Engineer Solo Builder (P-001)
- Has just signed a claim via US-002
- Has a working ATProto PDS session

### Solution

`Y` at the publish prompt writes the signed claim to the author's PDS as a record
in the `org.openlore.claim` collection, using the claim's CID as the rkey. On
success the CLI prints the `at-uri` AND the retract-command hint. On failure the
local claim remains intact and the publish can be retried idempotently.

### Domain Examples

#### Example 1 (Happy Path)

Jeff hits `Y`. The CLI publishes to `pds.example.com` and prints the at-uri
plus the retract hint.

#### Example 2 (Edge / PDS unreachable)

Maria's PDS is unreachable. The CLI prints `error: PDS write failed; local claim
intact; retry with `openlore claim publish bafy...n4ka``. The local file is unchanged.

#### Example 3 (Error / Already published)

Jeff accidentally reruns `openlore claim publish bafy...n4ka` after a successful
publish. The CLI detects the existing record (same rkey = same CID) and exits
with `claim already published; at-uri: at://...`. No duplicate, no error.

### UAT Scenarios (BDD)

```gherkin
Scenario: Successful publish prints at-uri and the retract hint
  Given Jeff has a signed claim with cid bafy...n4ka
  When Jeff confirms publication
  Then the CLI prints "at-uri: at://did:plc:jeff-test/org.openlore.claim/bafy...n4ka"
  And the CLI prints a tip referencing `openlore claim retract bafy...n4ka`
  And the PDS contains a record at that at-uri

Scenario: PDS unreachable leaves the local claim intact and retry-able
  Given Maria has a signed claim with cid bafy...lzfb
  And Maria's PDS endpoint is unreachable
  When Maria confirms publication
  Then the CLI exits non-zero with an actionable error
  And the local file at ~/.local/share/openlore/claims/bafy...lzfb.json is unchanged
  And the error mentions the retry command `openlore claim publish bafy...lzfb`

Scenario: Republishing a CID is idempotent
  Given Jeff has already published bafy...n4ka
  When Jeff runs `openlore claim publish bafy...n4ka` again
  Then the CLI exits with status 0
  And the CLI message indicates the record was already present
  And no duplicate record is created on the PDS
```

### Acceptance Criteria

- [ ] Successful publish output contains the at-uri AND the retract-command hint.
- [ ] PDS failure leaves the local file unmodified.
- [ ] Republishing the same CID is idempotent (no duplicate, no error).
- [ ] rkey equals claim_cid.

### Outcome KPIs

KPI-3 (claims-not-truth landing) and KPI-6 (publication willingness).

### Technical Notes

- Idempotency relies on the PDS treating rkey collisions as no-ops or "already exists".
- Publish-retry queue is in scope (US-002 wrote the file; this story retries the upload).

---

## US-004: Read back local claims by subject

### Job link

- `job_id`: J-001 (closes the round-trip) + J-002 (first taste of graph query)

### Elevator Pitch

- **Before**: After publishing a claim, I have no fast way to see exactly what I published — I'd have to curl my PDS and hand-parse.
- **After**: I run `openlore graph query --subject github:rust-lang/rust` and see my claim listed with author DID, predicate, object, confidence + bucket, evidence, at-uri, and composed_at — and an explicit footer telling me the result is local-only.
- **Decision enabled**: I trust the round-trip. The thing I see in the query matches the thing I composed and signed, so I'll publish more.

### Problem

Without a fast, faithful read-back, Jeff cannot trust that what he composed is
what landed in the graph. Any silent normalization between compose-time and
read-back time destroys his trust in the system.

### Who

- Senior Engineer Solo Builder (P-001)
- Has at least one local claim, optionally published

### Solution

`openlore graph query --subject <uri>` lists all local claims whose subject
matches the given URI. Output is greppable text by default (`--json` opt-in).
Footer makes it explicit that results are local-only and points at the
`--federated` flag landing in slice-03.

### Domain Examples

#### Example 1 (Happy Path)

Jeff just published a claim about `github:rust-lang/rust`. He runs
`openlore graph query --subject github:rust-lang/rust` and sees the claim
listed — all fields match the compose preview exactly.

#### Example 2 (Edge / Multiple claims same subject)

Aanya has two claims about `github:torvalds/linux` (one for `unix-philosophy`,
one for `pragmatism-over-purity`). Both appear in the output, each fully
attributed with its own confidence and at-uri.

#### Example 3 (Error / No matches)

Maria queries `--subject github:nonexistent/repo`. The CLI exits status 0 with
the message `No local claims about github:nonexistent/repo. Use --federated to
include subscribed authors (subscriptions land in slice-03-federated-read).`

### UAT Scenarios (BDD)

```gherkin
Scenario: Graph query reads back the just-published claim faithfully
  Given Jeff has published a claim about github:rust-lang/rust with cid bafy...n4ka
  When Jeff runs `openlore graph query --subject github:rust-lang/rust`
  Then the output lists exactly that claim
  And the predicate, object, confidence (numeric and bucket), evidence, author DID,
      at-uri, and composed_at all match the values from the compose preview

Scenario: Local-only is the default and is announced in the footer
  Given Jeff has at least one local claim
  When Jeff runs `openlore graph query --subject <any-subject>`
  Then the output footer states the result is local-only
  And the footer mentions the `--federated` flag will arrive in slice-03

Scenario: Empty result is explained, not silent
  Given Maria has no claims about github:nonexistent/repo
  When Maria runs `openlore graph query --subject github:nonexistent/repo`
  Then the CLI exits status 0
  And the output explicitly states no local claims were found for that subject
```

### Acceptance Criteria

- [ ] Output field values match compose-time values exactly (no silent normalization).
- [ ] Default is local-only; footer announces this.
- [ ] Empty result produces a helpful message, not silence or a non-zero exit.
- [ ] `--json` flag produces a structured array (one object per claim).

### Outcome KPIs

KPI-4 (round-trip identity).

### Technical Notes

- DuckDB query backs this. Schema details in DESIGN.
- Output format: text by default, JSON via `--json`.

---

## US-005 (`@infrastructure`): Bootstrap claim Lexicon, identity wiring, and DuckDB schema

### Job link

- `job_id`: `infrastructure-only`
- `infrastructure_rationale`: This story produces no user-observable behavior on its own. It is the prerequisite for US-001 through US-004. It is included in slice-01 because slice-01 already contains 4 user-facing stories (US-001–004), so the slice composition gate ("no slice is 100% @infrastructure") is satisfied.

### Problem

US-001 through US-004 each assume:
(a) the `org.openlore.claim` Lexicon exists and is loadable;
(b) the user's ATProto identity is resolvable at session-start;
(c) the local DuckDB schema is initialized with the right tables.

Without this bootstrap, none of the user-facing stories work.

### Who

- N/A — pure infrastructure. Consumed by US-001–004.

### Solution

- Define `org.openlore.claim` Lexicon (JSON file in repo, registered with the ATProto Lexicon resolver).
- `openlore init` command: resolves the user's ATProto identity, writes `~/.config/openlore/identity.toml`, creates `~/.local/share/openlore/openlore.duckdb`, runs initial schema migrations.
- DuckDB schema for the `claims` table (mirrors the Lexicon fields plus local-only metadata: at-uri, published_at, etc.).

### Domain Examples

#### Example 1

A fresh install runs `openlore init`. The user pastes their ATProto handle and
app-password. The CLI resolves the DID, writes identity.toml, creates the DuckDB,
and exits with `OpenLore initialized for did:plc:jeff-test`.

#### Example 2

Running `openlore init` a second time is idempotent: it detects existing
configuration and exits with `already initialized for did:plc:jeff-test`.

#### Example 3

Running US-001 (`openlore claim add ...`) without first running `openlore init`
exits with `error: not initialized; run \`openlore init\` first.`

### UAT Scenarios (BDD)

```gherkin
Scenario: First-time init creates identity, DuckDB, and is idempotent
  Given a fresh environment with no ~/.config/openlore/ or ~/.local/share/openlore/
  When the user runs `openlore init` with a valid ATProto handle and app-password
  Then ~/.config/openlore/identity.toml exists and names a valid DID
  And ~/.local/share/openlore/openlore.duckdb exists with the claims table present
  And re-running `openlore init` exits status 0 with an "already initialized" message

Scenario: Claim commands gated on init
  Given a fresh environment with no ~/.config/openlore/
  When the user runs `openlore claim add ...`
  Then the CLI exits non-zero
  And the error message names the required `openlore init` command
```

### Acceptance Criteria

- [ ] `org.openlore.claim` Lexicon file exists in repo and is loadable.
- [ ] `openlore init` is idempotent.
- [ ] Claim commands fail loudly if not initialized.
- [ ] DuckDB schema migration is versioned and forward-compatible.

### Technical Notes

- Decisions deferred to DESIGN: exact DuckDB schema (one table vs split), migration tool, identity storage format details.
- This story is the ONLY @infrastructure story in slice-01 and it links 1:1 to all user-facing stories in the slice.
