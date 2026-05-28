<!-- markdownlint-disable MD024 -->

# User Stories — openlore-github-scraper (slice-02)

All stories in this file belong to **slice-02-github-scraper** (the third
sibling feature shipped after openlore-foundation and openlore-federated-read,
per the WD-13 sequence federation->scrapers). Every story carries a `job_id`
traceable to `docs/product/jobs.yaml` per Decision 1. Stories US-SCR-001..005
carry mandatory Elevator Pitches; US-SCR-006 is `@infrastructure` and carries
an `infrastructure_rationale` instead.

## System Constraints

These are cross-cutting constraints that apply to every story in this feature.
The first six are **inherited from the slice-01 / slice-03 lineage** and are
repeated here for the reviewer's convenience. They are NOT relitigated.

- **Human-gate (load-bearing)**: the scraper PROPOSES candidate claims; the
  human SIGNS. No candidate is ever signed, persisted as a claim, or published
  except by passing through the slice-01 compose-sign-publish pipeline with the
  human's explicit signing gesture. The scraper has no signing key and no
  publish code path. This preserves the slice-01 "claims are signed human
  assertions" invariant.
- **Public-data-only (no surveillance)**: the scraper reads ONLY public GitHub
  data. The target is the SUBJECT of a possible claim, never a controller of it.
  Private/non-existent/inaccessible targets are refused. Counter-claim and
  retraction remain first-class (J-004 anxiety mitigation, inherited from
  slice-01/03).
- **Claims-not-truth invariant**: the literal text "not as truth" still appears
  in any compose preview reached from a scraper candidate (it is the SAME
  slice-01 preview). No UI surface frames any claim — or any candidate — as a
  truth assertion.
- **Confidence numeric-only (WD-10)**: candidate and signed confidence is numeric
  `[0.0, 1.0]`; display buckets (speculative `<0.3`, weighted `0.3-0.7`,
  well-evidenced `0.7-0.9`, triangulated `>0.9`) are display-only and MUST NEVER
  be persisted. Scraper candidates default to **0.25 (speculative)**.
- **Single publish path (ADR-003)**: a signed-from-scraper claim publishes via
  the SAME `VerbClaimPublish` internals as a hand-authored claim. No parallel
  publish path.
- **CLI-first**: the CLI remains the canonical interface; no daemon, no
  scheduled scraping, no web UI in slice-02. The scraper is invoked on-demand.

Constraints introduced new by this slice:

- **Verb shape**: `openlore scrape github <target> [--sign N[,N...]]` is a sugar
  verb (symmetric with the slice-03 `peer pull` / `claim counter` additions). The
  `--sign` continuation invokes the slice-01 compose-sign-publish pipeline. The
  ADR-003/ADR-013 verb-contract amendment that adds `scrape github` is a DESIGN
  deliverable.
- **Small auditable signal->predicate mapping**: the default mapping lives in
  `docs/product/jobs.yaml :: J-004.signal_predicate_mapping` (the SSOT). It is
  small by design (5 entries in slice-02) and every candidate names the exact
  signal that produced it. The human edits any candidate before signing.
- **Optional PAT auth**: the scraper works unauthenticated for small targets
  (subject to GitHub's anonymous rate limit) and uses an optional Personal Access
  Token via `GITHUB_TOKEN` env var (or config) for higher rate limits and larger
  / contributor targets.
- **Provenance line**: a signed-from-scraper claim records an informational
  `derived-from: openlore-github-scraper (signal: ...)` line. Whether this lives
  in the signed payload (as an OPTIONAL, CID-stable-when-absent field per ADR-005,
  mirroring the slice-03 `reason` treatment) or stays display-only is a DESIGN
  call; the product contract is "the reader can see the claim originated from a
  scraper run; the provenance NEVER changes the confidence or federation
  behavior."
- **Pure / effect split (ADR-007)**: `scraper-domain` is PURE (derives candidate
  claims from already-fetched GitHub data; no I/O). `adapter-github` is the EFFECT
  shell (GitHub REST/GraphQL over HTTPS behind a new `GithubPort`); it ships a
  `probe()` per ADR-009 I-4.

### Glossary (terms introduced by this slice)

- **Target**: a public GitHub repository (`owner/repo`) or user/contributor
  (`user`) the user wants to evaluate through a philosophy lens.
- **Signal**: a public GitHub artifact or measurable property harvested by
  `adapter-github` (e.g. "Cargo.lock committed", "test/source ratio 0.61",
  "docs/ directory present + README 412 lines").
- **Candidate claim**: a PROPOSAL derived (purely) by `scraper-domain` from one
  or more signals via the signal->predicate mapping. It carries a suggested
  predicate, object, evidence URL, and a conservative default confidence (0.25).
  It is NOT a claim until the human signs it.
- **Sign-from-scraper**: carrying a candidate through the slice-01
  compose-sign-publish pipeline via `scrape github <target> --sign N`, where the
  candidate pre-fills the editable compose fields.
- **derived-from provenance**: the informational record that a signed claim
  originated from a scraper run, naming the source signal.

---

## US-SCR-001: Harvest a public GitHub target's signals

### Job link

- `job_id`: J-004 (sub-job J-004a harvest public GitHub signals)

### Elevator Pitch

- **Before**: When I want to evaluate rust-lang/cargo's maintainers through a
  philosophy lens, I open twenty browser tabs — README, Cargo.lock, the docs
  tree, the test directory, the CHANGELOG — and form an impression by hand,
  which takes 20 minutes and leaves no artifact.
- **After**: I run `openlore scrape github rust-lang/cargo`, see a banner that
  only public data is read and nothing is published, then "Harvested 5 public
  signals in 2.1s" — the tool gathered the artifacts for me.
- **Decision enabled**: I can decide, in seconds rather than minutes, whether a
  target has any philosophy signals worth claiming about at all — which means
  I'll actually run the lens on targets I'm only mildly curious about.

### Problem

Maria Lopez (P-002) is a tech lead evaluating whether to adopt `rust-lang/cargo`
patterns for her team's build tooling. To form a defensible view of the
project's engineering philosophy, she currently opens the README, the
`Cargo.lock`, the `docs/` tree, the test directory, and the CHANGELOG by hand —
roughly twenty minutes of tab-juggling that produces only a mental impression,
no queryable artifact. There is no tool that gathers a target's PUBLIC signals
for her, and she is wary of anything that smells like a surveillance/scraping
tool that touches private data.

### Who

- Researcher / Tech Lead (P-002) wearing the contributor-evaluator hat
- Already authenticated against their own ATProto identity (per slice-01 init)
- Comfortable with `owner/repo` and `user` GitHub identifiers and CLI flags
- Online (harvest requires network)
- Anxious that the tool might read private data or feel like surveillance

### Solution

A `openlore scrape github <target>` CLI command that resolves a PUBLIC GitHub
repo or user, harvests a small set of public signals (README/docs presence and
size, dependency-manifest pinning, test/source ratio, semver/CHANGELOG
presence, primary language), and reports how many signals it found. Harvest
reads ONLY public endpoints. It does NOT derive or sign anything on its own —
the candidate derivation (US-SCR-002) is the next step in the same invocation.

### Domain Examples

#### Example 1 (Happy Path)

Maria Lopez (`did:plc:maria-test`) runs `openlore scrape github rust-lang/cargo`.
The CLI prints the public-data-only banner, resolves the repo (default branch
`master`), harvests 5 signals (Cargo.lock committed, docs/ present, test ratio
0.61, CHANGELOG+semver, Rust+no-unsafe), and reports `Harvested 5 signals in
2.1s.` No claim is signed.

#### Example 2 (Edge / Contributor (user) target)

Tobias Weber runs `openlore scrape github torvalds`. The CLI resolves the
GitHub USER `torvalds` (not a repo), harvests cross-repo aggregate signals from
the user's public repositories, and reports the signal count. (Deep
cross-repo triangulation is deferred to slice-04; slice-02 harvests a bounded
aggregate.)

#### Example 3 (Error / Target does not exist)

Aanya Krishnan runs `openlore scrape github ghost-org/ghost-repo`. The GitHub
API returns 404. The CLI prints `error: github target ghost-org/ghost-repo not
found (HTTP 404)`. No candidates are produced. Exit code is non-zero.

#### Example 4 (Error / Target is private)

Maria runs `openlore scrape github acme-corp/secret-repo`, a private repo she
can see in her browser because she is a member. The scraper, reading only public
endpoints, gets a 404/403. The CLI prints `error: target is not public; the
OpenLore scraper only reads public data`. No candidates. Exit non-zero.

### UAT Scenarios (BDD)

```gherkin
Scenario: Harvest public signals from a public GitHub repo
  Given the GitHub repo rust-lang/cargo is public
  When Maria runs `openlore scrape github rust-lang/cargo`
  Then the CLI prints a banner stating only public data is read and nothing is published
  And the CLI reports the count of public signals harvested
  And no claim is signed and no PDS write occurs

Scenario: Scraping a non-existent target produces no candidates
  Given the target ghost-org/ghost-repo does not exist on GitHub
  When Aanya runs `openlore scrape github ghost-org/ghost-repo`
  Then the CLI exits with a non-zero status
  And the error names the target and the not-found cause
  And zero candidate claims are produced

Scenario: A private target is refused because the scraper reads only public data
  Given acme-corp/secret-repo is a private repository
  When Maria runs `openlore scrape github acme-corp/secret-repo`
  Then the CLI exits with a non-zero status
  And the error message states the scraper only reads public data
  And no private GitHub endpoint is called

Scenario: Harvest requires network and fails gracefully offline
  Given the machine has no network connectivity
  When Tobias runs `openlore scrape github rust-lang/cargo`
  Then the CLI exits with a non-zero status
  And the error message states that `scrape` requires network access
  And no partial candidate list is rendered
```

### Acceptance Criteria

- [ ] `openlore scrape github <target>` resolves a PUBLIC GitHub repo or user before harvesting.
- [ ] A public-data-only banner is printed BEFORE any harvest begins.
- [ ] Harvest reads ONLY public GitHub endpoints; no private/authenticated-private endpoint is ever called.
- [ ] The CLI reports the count of public signals harvested.
- [ ] A non-existent target exits non-zero, names the target, and produces zero candidates.
- [ ] A private/inaccessible target exits non-zero with a "scraper only reads public data" message.
- [ ] Harvest is the only network step; offline invocation exits non-zero with a clear "requires network" message and renders no partial list.

### Outcome KPIs

See `outcome-kpis.md` KPI-SCR-1 (cost-to-first-claim, harvest is the entry
step) and KPI-SCR-4 (public-data-only guardrail).

### Technical Notes

- Depends on US-SCR-006 (`GithubPort` + `adapter-github` in place).
- New port surface: `GithubPort` with (e.g.) `resolve_target(target) -> TargetKind`, `harvest_repo(repo) -> Vec<Signal>`, `harvest_user(user) -> Vec<Signal>`. Behind the port lives the EFFECT shell `adapter-github`. MUST include `probe()` per ADR-009 I-4 (probe asserts the public API is reachable with a 250ms budget per I-5).
- `adapter-github` uses GitHub REST/GraphQL over HTTPS. Unauthenticated by default; optional PAT (US-SCR-004).
- The list of harvested signal TYPES is bounded by the signal->predicate mapping in `jobs.yaml`; adapter need not harvest signals the mapping cannot use.

---

## US-SCR-002: Derive auditable candidate claims from harvested signals

### Job link

- `job_id`: J-004 (sub-job J-004b derive editable candidate claims)

### Elevator Pitch

- **Before**: Even after gathering a target's signals, I face a blank
  `openlore claim add` prompt and have to recall the exact philosophy predicate
  vocabulary and craft each claim from scratch — the recall cost is the real
  friction.
- **After**: Right after harvest, I see "Candidate claims for subject
  github:rust-lang/cargo (5 derived — NOTHING is signed)" with each candidate
  naming the predicate, the source signal, an evidence URL, and a conservative
  0.25 (speculative) confidence.
- **Decision enabled**: I can decide which proposed claims are worth pursuing by
  scanning a traceable list instead of recalling vocabulary from memory — which
  means I'll produce claims I would otherwise never have bothered to author.

### Problem

After harvesting a target's signals, Maria still faces the friction the slice-01
J-001 success signal calls out: a claim is fast to write "once the user knows
the predicate vocabulary." The vocabulary recall is precisely the cost the
scraper should remove. Maria needs the tool to PROPOSE candidate claims — each
mapping a concrete public signal to a philosophy predicate — so she has a strong,
auditable starting point rather than a blank prompt. Critically, she must be
able to trust the proposals: each candidate must name the exact signal that
produced it, and none may be signed or published automatically.

### Who

- Researcher / Tech Lead (P-002), has just harvested a target via US-SCR-001
- Author-engineers (P-001) evaluating a dependency's maintainers
- Online or offline (derivation is PURE; it runs on already-harvested signals)
- Wants a traceable starting point, not a machine asserting on their behalf

### Solution

After harvest, the CLI runs the PURE `scraper-domain` derivation: it maps each
harvested signal to a candidate claim via the signal->predicate mapping in
`jobs.yaml`, assigns the conservative default confidence (0.25, speculative),
and renders a numbered candidate list. Every candidate names the exact signal
that produced it. NO candidate is written to `author_claims`, signed, or
published. A target with no matching signals proposes nothing (exit 0; not an
error).

### Domain Examples

#### Example 1 (Happy Path)

Maria's harvest of `rust-lang/cargo` produced 5 signals. The CLI renders 5
candidates: `[1] dependency-pinning` (from "Cargo.lock committed"),
`[2] documentation-first` (from "docs/ + README 412 lines"),
`[3] test-driven` (from "test ratio 0.61"), `[4] semantic-versioning` (from
"CHANGELOG + semver tags"), `[5] memory-safety` (from "Rust + no unsafe").
Each shows confidence 0.25 (speculative). Footer: "These are PROPOSALS ...
none is a claim until YOU sign it."

#### Example 2 (Edge / Target with no matching signals)

Tobias scrapes `some-user/empty-experiment`, a bare repo with an empty README,
no manifest, no tests, no tags. Harvest finds zero signals the mapping can use.
The CLI prints `No candidate claims could be derived from public signals for
github:some-user/empty-experiment.` Exit code is 0 (nothing to propose is not
an error). Zero rows are written to `author_claims`.

#### Example 3 (Edge / One signal maps but the user disagrees with the mapping)

Aanya scrapes a repo where `Cargo.lock` is committed, so candidate `[1]
dependency-pinning` is proposed. Aanya knows this particular project commits
`Cargo.lock` only because it is a binary crate (where it is conventional), not
as a philosophical stance. She simply does NOT select candidate 1. Because the
candidate named its source signal ("Cargo.lock committed"), she could audit and
reject the derivation. Nothing is signed.

#### Example 4 (Edge / Multiple signals map to the same predicate)

Maria scrapes a repo with both `docs/` AND an unusually long README AND high
doc-comment density. Rather than proposing three near-duplicate
`documentation-first` candidates, `scraper-domain` collapses them into ONE
`documentation-first` candidate whose source-signal line lists all three
contributing signals.

### UAT Scenarios (BDD)

```gherkin
Scenario: Derived candidates are proposals with traceable signals
  Given Maria has harvested 5 matching public signals from rust-lang/cargo
  When the CLI renders the candidate list
  Then exactly 5 candidate claims are shown numbered 1..5
  And each candidate names the exact public signal that produced it
  And each candidate's confidence is 0.25 displayed as "speculative"
  And the footer states that nothing is a claim until the user signs it
  And no candidate has been signed, persisted as a claim, or published

Scenario: A target with no matching signals proposes nothing without erroring
  Given the target some-user/empty-experiment has no signals matching the mapping
  When the CLI finishes harvesting
  Then the CLI prints "No candidate claims could be derived"
  And exit code is 0
  And zero rows are written to author_claims

Scenario: Multiple signals for one predicate collapse into a single candidate
  Given a target has a docs/ directory AND a 400-line README AND high doc-comment density
  When the CLI derives candidates
  Then exactly one documentation-first candidate is shown
  And that candidate's source-signal line lists all three contributing signals

Scenario: Candidate confidence never exceeds the conservative default at proposal time
  Given any target has been harvested
  When the CLI renders candidates
  Then every candidate's proposed confidence is 0.25 (speculative)
  And no candidate is proposed with a confidence above 0.3
```

### Acceptance Criteria

- [ ] After harvest, the CLI renders a numbered candidate list derived purely from the harvested signals via the `jobs.yaml` signal->predicate mapping.
- [ ] Every candidate names the exact public signal(s) that produced it.
- [ ] Every candidate's proposed confidence is the conservative default 0.25 (speculative bucket per WD-10); no candidate is proposed above 0.3.
- [ ] Multiple signals mapping to the same predicate collapse into ONE candidate listing all contributing signals.
- [ ] No candidate is written to `author_claims`, signed, or published at derivation time.
- [ ] A target with zero matching signals prints a clear "no candidates derived" message and exits 0 (not an error).
- [ ] The candidate-list footer states that nothing is a claim until the user signs it.

### Outcome KPIs

See `outcome-kpis.md` KPI-SCR-3 (auditability — this story is the load-bearing
surface) and KPI-SCR-1 (cost-to-first-claim — the candidate list removes the
vocabulary-recall cost).

### Technical Notes

- Depends on US-SCR-001 (signals harvested) and US-SCR-006 (`scraper-domain` crate).
- `scraper-domain` is PURE (ADR-007): it takes `Vec<Signal>` + the mapping and returns `Vec<CandidateClaim>`. No I/O. This makes the derivation trivially unit-testable and mutation-testable.
- The signal->predicate mapping is the `jobs.yaml :: J-004.signal_predicate_mapping` SSOT; `scraper-domain` must consume it, not hardcode a divergent copy. DESIGN owns the serde shape for loading/embedding it.
- A `CandidateClaim` carries: subject (= `github_target`), predicate, object, evidence URL, default confidence 0.25, and the list of source signals. It is an in-memory ADT, never persisted as-is.

---

## US-SCR-003: Review, edit, and sign a candidate via the slice-01 pipeline

### Job link

- `job_id`: J-004 (sub-job J-004c human-always-signs) + J-001 (the underlying compose-sign-publish flow)

### Elevator Pitch

- **Before**: I have a list of proposed candidates, but if the tool signed them
  for me it would be putting words in my mouth — turning my reasoning into a
  machine's assertion, which I will not accept.
- **After**: I run `openlore scrape github rust-lang/cargo --sign 1`, the
  candidate pre-fills the SAME compose preview I know from slice-01 (with "not
  as truth"), I raise confidence from 0.25 to 0.55 and add a note, press Enter
  to sign, then Y to publish — and the success message reminds me how to
  retract.
- **Decision enabled**: I can turn a scraper proposal into MY signed,
  evidence-backed claim — editing whatever I disagree with first — which means I
  trust the tool to lower my cost without ever speaking for me.

### Problem

Maria has reviewed the candidate list and wants to sign candidate 1
(`dependency-pinning`). But the whole psychological contract of the feature is
that the tool must NOT sign on her behalf. She needs to carry the candidate into
the slice-01 compose-sign-publish pipeline she already trusts — edit any field
(she wants to raise the speculative 0.25 to 0.55 because she has personal
knowledge the project's `Cargo.lock` is deliberate policy, and add a clarifying
note) — and sign it herself. The signed claim must be byte-shape-identical to a
hand-authored one, plus an informational provenance line.

### Who

- Researcher / Tech Lead (P-002), has a candidate list from US-SCR-002
- Author-engineers (P-001) in the same flow
- Online to publish; sign succeeds offline (slice-01 local-first, inherited)
- Insistent that the human, not the tool, is the signer

### Solution

A `--sign <N>` continuation of `scrape github <target>` that takes candidate N,
pre-fills the slice-01 compose editor with the candidate's fields (subject,
predicate, object, evidence, confidence 0.25), lets the user accept or edit each
field (confidence editing enforces `[0.0,1.0]`), then runs the SAME
compose-sign-publish pipeline as `claim add` — including the literal "not as
truth" preview and the human signing gesture. The signed claim records an
informational `derived-from` provenance line. An out-of-range `--sign` index is
rejected before any compose begins.

### Domain Examples

#### Example 1 (Happy Path)

Maria runs `openlore scrape github rust-lang/cargo --sign 1`. The compose
editor pre-fills subject `github:rust-lang/cargo`, predicate
`embodiesPhilosophy`, object `...dependency-pinning`, evidence the Cargo.lock
URL, confidence `0.25`. She presses Enter through subject/predicate/object/
evidence, types `0.55` for confidence, and adds a note. The compose preview
shows "not as truth", confidence "0.55 (weighted)", and `derived-from:
openlore-github-scraper (signal: Cargo.lock committed)`. She presses Enter
(sign), CID computed, then `y` (publish). Success message includes the retract
hint.

#### Example 2 (Edge / User accepts all defaults unchanged)

Tobias runs `--sign 3` and presses Enter through every field including the 0.25
confidence. The candidate signs with confidence 0.25 (speculative) exactly as
proposed. The signed claim's fields equal the candidate's proposed fields
byte-for-byte (no auto-inflation). KPI-SCR-5 records this as a zero-edit sign.

#### Example 3 (Error / `--sign` index out of range)

Aanya's candidate list has 5 entries. She runs `--sign 9`. The CLI rejects
before composing: `error: candidate 9 does not exist; valid range 1..5`. No
claim is composed, signed, or published.

#### Example 4 (Error / edited confidence out of range)

Maria runs `--sign 1` and types `1.5` for confidence. The CLI re-prompts:
`confidence must be between 0.0 and 1.0`. She types `0.55`. Compose proceeds.
No claim is written until a valid confidence is entered.

#### Example 5 (Edge / publish declined, sign retained)

Tobias runs `--sign 2`, signs locally (Enter), then answers `N` to "Publish to
your PDS now?". The claim is persisted locally (slice-01 behavior) and NOT
published. He can publish later with `openlore claim publish <cid>`.

### UAT Scenarios (BDD)

```gherkin
Scenario: A reviewed candidate signs and publishes via the slice-01 pipeline
  Given Maria has a candidate list for github:rust-lang/cargo
  When Maria runs `openlore scrape github rust-lang/cargo --sign 1`
  And Maria raises the confidence from 0.25 to 0.55 and accepts the other fields
  And Maria presses Enter to sign and confirms the publish prompt
  Then the compose preview contained the literal text "not as truth"
  And the claim is signed with Maria's DID via the slice-01 VerbClaimAdd path
  And the claim is published via the SAME VerbClaimPublish path as a hand-authored claim
  And the signed claim records a confidence of 0.55 (numeric only, per WD-10)
  And the signed claim records a derived-from provenance naming the source signal
  And the publish success message mentions the retract command

Scenario: Accepting all candidate defaults signs exactly what was proposed
  Given Tobias has a candidate list and selects candidate 3
  When Tobias presses Enter through every field including the 0.25 confidence
  Then the signed claim's subject, predicate, object, and evidence equal candidate 3's proposed values
  And the signed claim's confidence is 0.25 (no auto-inflation)

Scenario: An out-of-range selection is rejected before composing
  Given the candidate list shows 5 candidates
  When Aanya runs `openlore scrape github rust-lang/cargo --sign 9`
  Then the CLI exits non-zero with "candidate 9 does not exist; valid range 1..5"
  And no claim is composed, signed, or published

Scenario: Editing confidence out of range re-prompts without writing a claim
  Given Maria runs `openlore scrape github rust-lang/cargo --sign 1`
  When Maria enters 1.5 for the confidence field
  Then the CLI re-prompts that confidence must be between 0.0 and 1.0
  And no claim is written until a valid confidence is entered

Scenario: Declining publish retains the locally signed claim
  Given Tobias runs `--sign 2` and signs locally
  When Tobias answers "N" to the publish prompt
  Then the claim is persisted locally and NOT published
  And the CLI hints that it can be published later with `openlore claim publish <cid>`
```

### Acceptance Criteria

- [ ] `openlore scrape github <target> --sign <N>` pre-fills the slice-01 compose editor with candidate N's fields.
- [ ] The user can accept or edit each field; confidence editing enforces `[0.0,1.0]` and re-prompts on invalid input without writing a claim.
- [ ] The compose preview contains the literal text "not as truth" (inherited I-7).
- [ ] Sign and publish reuse the slice-01 `VerbClaimAdd` / `VerbClaimPublish` internals (no parallel publish path).
- [ ] If the user edits no fields, the signed claim's fields equal the candidate's proposed values byte-for-byte (no auto-inflation of confidence).
- [ ] The signed claim records an informational `derived-from` provenance naming the source signal (provenance never alters confidence or federation behavior).
- [ ] An out-of-range `--sign` index is rejected before any compose begins (exits non-zero; nothing written).
- [ ] Declining the publish prompt retains the locally signed claim and hints at `openlore claim publish <cid>`.
- [ ] The publish success message mentions the retract command (inherited I-8).

### Outcome KPIs

See `outcome-kpis.md` KPI-SCR-1 (cost-to-first-claim — north star; this story is
the value-capture surface), KPI-SCR-2 (human-gate guardrail), KPI-SCR-5 (edit
rate).

### Technical Notes

- Depends on US-SCR-002 (candidate list exists) and US-SCR-006 (provenance field if stored in the signed payload).
- Reuses slice-01 `VerbClaimAdd`, `VerbClaimPublish`, `claim-domain::compute_cid`, `claim-domain::sign` UNCHANGED. The only new code is the candidate->compose pre-fill mapping and the optional provenance line.
- The `derived-from` provenance: DESIGN decides whether it lives in the signed payload (as an OPTIONAL, CID-stable-when-absent field per ADR-005, mirroring the slice-03 `reason` field WD-32/ADR-015) or stays display-only. Product contract: never changes confidence or federation.
- Confidence pre-fill is the speculative default 0.25; only the human may raise it. The "no auto-inflation" property is enforced by acceptance test `candidate_confidence_no_autoinflate`.

---

## US-SCR-004: Use an optional Personal Access Token for higher rate limits

### Job link

- `job_id`: J-004 (sub-job J-004a harvest — enabling real-target reach)

### Elevator Pitch

- **Before**: When I scrape a busy contributor like `torvalds` or a large
  monorepo, the unauthenticated GitHub rate budget runs out mid-harvest and I
  get a partial, useless result with no idea why.
- **After**: I set `GITHUB_TOKEN` once; `openlore scrape github torvalds`
  reports "auth: authenticated (4982/5000 rate budget)" and the harvest
  completes; without the token, the same command degrades gracefully with a
  clear "set GITHUB_TOKEN for higher limits" message instead of a cryptic
  failure.
- **Decision enabled**: I can run the lens on the REAL targets I care about —
  active maintainers and large projects — instead of only toy repos, which is
  the difference between the scraper being a demo and being useful.

### Problem

The walking skeleton (US-SCR-001..003) works unauthenticated on small repos, but
GitHub's anonymous rate limit (60 requests/hour) is exhausted almost immediately
on a real evaluation target — a contributor with many public repos, or a large
monorepo with a deep file tree. Maria gets a partial harvest and a confusing
`HTTP 403 rate limit exceeded` with no remediation. She needs the scraper to use
her optional Personal Access Token for the higher authenticated rate budget
(5000 requests/hour), and to degrade gracefully (with a clear remediation hint)
when no token is present.

### Who

- Researcher / Tech Lead (P-002) evaluating active maintainers / large projects
- Author-engineers (P-001) evaluating a dependency's full maintainer set
- Has a GitHub PAT they are willing to expose via `GITHUB_TOKEN` env var or config
- Online (harvest requires network)

### Solution

`adapter-github` reads an optional Personal Access Token from the `GITHUB_TOKEN`
environment variable (or a config entry). When present, harvest uses the
authenticated rate budget and reports the remaining budget. When absent, harvest
runs unauthenticated and, if it hits the anonymous rate limit, exits non-zero
with a clear `set GITHUB_TOKEN for higher rate limits` remediation. The token is
NEVER logged, NEVER written to any claim, and NEVER published — it is an effect-
shell credential only.

### Domain Examples

#### Example 1 (Happy Path / Authenticated)

Maria exports `GITHUB_TOKEN=ghp_...` and runs `openlore scrape github torvalds`.
The CLI reports `auth: authenticated (4982/5000 rate budget)`, harvests the
contributor's cross-repo signals, and renders candidates. The token never
appears in any output, claim, or log line.

#### Example 2 (Edge / Unauthenticated small target succeeds)

Tobias has no `GITHUB_TOKEN` set and runs `openlore scrape github
small-org/tiny-lib` (a 3-file repo). The harvest stays within the anonymous
budget, completes, and reports `auth: unauthenticated (anonymous rate limit)`.
Candidates render normally.

#### Example 3 (Error / Unauthenticated large target hits rate limit)

Aanya has no token and runs `openlore scrape github torvalds`. The harvest
exhausts the anonymous budget mid-way. The CLI exits non-zero with `error:
GitHub anonymous rate limit exhausted; set GITHUB_TOKEN for higher limits
(5000/hour)`. No partial candidate list is rendered (avoids a misleadingly
incomplete proposal set).

#### Example 4 (Edge / Invalid token)

Maria sets a stale `GITHUB_TOKEN`. GitHub returns 401. The CLI exits non-zero
with `error: GITHUB_TOKEN was rejected by GitHub (HTTP 401); unset it to scrape
anonymously, or provide a valid token`. The token value is NOT echoed.

### UAT Scenarios (BDD)

```gherkin
Scenario: Authenticated harvest uses the higher rate budget
  Given the GITHUB_TOKEN environment variable holds a valid PAT
  When Maria runs `openlore scrape github torvalds`
  Then the CLI reports it is authenticated and shows the remaining rate budget
  And the harvest completes for a target that would exhaust the anonymous budget
  And the token value never appears in any output line, claim, or log

Scenario: Unauthenticated harvest of a small target succeeds
  Given no GITHUB_TOKEN is set
  When Tobias runs `openlore scrape github small-org/tiny-lib`
  Then the CLI reports it is unauthenticated
  And the harvest completes within the anonymous rate budget
  And candidates are rendered normally

Scenario: Unauthenticated harvest hitting the rate limit fails with remediation
  Given no GITHUB_TOKEN is set
  And the target torvalds requires more requests than the anonymous budget allows
  When Aanya runs `openlore scrape github torvalds`
  Then the CLI exits non-zero
  And the error message suggests setting GITHUB_TOKEN for higher limits
  And no partial candidate list is rendered

Scenario: An invalid token is reported without echoing its value
  Given GITHUB_TOKEN holds a stale or invalid PAT
  When Maria runs `openlore scrape github rust-lang/cargo`
  Then the CLI exits non-zero with an HTTP 401 explanation
  And the error suggests unsetting the token or providing a valid one
  And the token value is not echoed anywhere
```

### Acceptance Criteria

- [ ] `adapter-github` reads an optional PAT from `GITHUB_TOKEN` (env) or config; absence is valid (unauthenticated mode).
- [ ] When authenticated, the CLI reports authenticated status and the remaining rate budget.
- [ ] When unauthenticated, harvest works for small targets within the anonymous budget.
- [ ] Hitting the anonymous rate limit exits non-zero with a `set GITHUB_TOKEN` remediation and renders NO partial candidate list.
- [ ] An invalid/rejected token exits non-zero with an HTTP-401 explanation and a remediation hint.
- [ ] The token value is NEVER logged, echoed, written to a claim, or published.

### Outcome KPIs

See `outcome-kpis.md` KPI-SCR-1 (cost-to-first-claim must hold for REAL targets,
not just toy repos).

### Technical Notes

- Depends on US-SCR-001 (harvest path exists) and US-SCR-006 (`adapter-github`).
- The token is an effect-shell credential held only in `adapter-github`; `scraper-domain` (pure) never sees it.
- Reuse the slice-01 supply-chain posture: no new secret-handling crate beyond what `adapter-atproto-*` already uses for credentials; the token is read from env/config and passed as an `Authorization` header only.
- DESIGN owns whether config-file token support ships in slice-02 or only env-var; env-var is the minimum.

---

## US-SCR-005: Select and sign several candidates in one pass

### Job link

- `job_id`: J-004 (sub-job J-004c human-always-signs — batch efficiency on the human-gated flow)

### Elevator Pitch

- **Before**: When I agree with three of the five proposed candidates, I have to
  re-run `--sign 1`, then `--sign 3`, then `--sign 4` separately — three full
  invocations for one evaluation session.
- **After**: I run `openlore scrape github rust-lang/cargo --sign 1,3,4` and the
  CLI walks me through three compose previews in sequence — I sign each
  individually, with a "(2 of 3 signed)" progress line — never losing the
  per-claim human signing gesture.
- **Decision enabled**: I can capture a whole evaluation session's worth of
  claims in one pass without the tool ever batch-signing on my behalf — every
  claim is still individually reviewed and signed by me.

### Problem

When Maria's evaluation of a target yields several candidates she agrees with,
signing them one invocation at a time (`--sign 1`, then re-run, `--sign 3`,
re-run, `--sign 4`) is tedious and re-harvests each time. She needs to select
several candidates in one pass — but WITHOUT the tool batch-signing on her
behalf. Each selected candidate must still flow through the slice-01 compose
preview and require her individual signing gesture; batch selection is a
convenience over the human-gate, never a bypass of it.

### Who

- Researcher / Tech Lead (P-002), agrees with several candidates in one session
- Author-engineers (P-001) capturing a full evaluation of a dependency's maintainers
- Online to publish; sign succeeds offline (inherited)
- Will NOT accept a "sign all" that skips per-claim review

### Solution

`--sign <N,N,...>` accepts a comma-separated list of candidate indices. The CLI
walks them in order, presenting each candidate's slice-01 compose preview
individually (editable, with "not as truth"), requiring the human's signing
gesture for each, and showing running progress ("(2 of 3 signed)"). There is NO
"sign all without review" affordance. Any single sign can be skipped (Ctrl-C on
that candidate) without aborting the rest; the progress summary reports how many
were signed vs skipped.

### Domain Examples

#### Example 1 (Happy Path)

Maria runs `openlore scrape github rust-lang/cargo --sign 1,3,4`. The CLI shows
candidate 1's compose preview; she edits confidence and signs. Progress: "(1 of
3 signed)". Candidate 3's preview; she signs as-is. "(2 of 3 signed)". Candidate
4's preview; she signs. "(3 of 3 signed). Published 3 claims." Each was
individually previewed and signed.

#### Example 2 (Edge / Skip one mid-batch)

Tobias runs `--sign 1,2,5`. He signs 1, then on candidate 2's preview decides he
disagrees and presses Ctrl-C for that candidate only. The CLI prints `skipped
candidate 2` and proceeds to candidate 5, which he signs. Summary: `2 signed, 1
skipped`.

#### Example 3 (Error / Duplicate or out-of-range index in the list)

Aanya runs `--sign 1,1,9` on a 5-candidate list. The CLI rejects before
composing: `error: invalid selection '1,1,9' — duplicate index 1; index 9 out
of range (valid 1..5)`. No claim is composed.

#### Example 4 (Edge / Single index still works)

Maria runs `--sign 2` (single index, no comma). Behaves exactly like US-SCR-003
single-candidate sign. Batch is a superset, not a replacement.

### UAT Scenarios (BDD)

```gherkin
Scenario: Signing several candidates walks each through an individual compose preview
  Given Maria has a 5-candidate list for github:rust-lang/cargo
  When Maria runs `openlore scrape github rust-lang/cargo --sign 1,3,4`
  Then the CLI presents candidate 1's compose preview and requires a signing gesture
  And after signing, the CLI shows "(1 of 3 signed)" and presents candidate 3
  And after signing 3, the CLI shows "(2 of 3 signed)" and presents candidate 4
  And each candidate is signed individually via the slice-01 pipeline
  And there is no "sign all without review" affordance

Scenario: A candidate can be skipped mid-batch without aborting the rest
  Given Tobias runs `--sign 1,2,5` on a valid candidate list
  When Tobias signs candidate 1, then cancels candidate 2's compose
  Then the CLI prints "skipped candidate 2"
  And the CLI proceeds to candidate 5
  And the final summary reports "2 signed, 1 skipped"

Scenario: An invalid selection list is rejected before composing
  Given the candidate list shows 5 candidates
  When Aanya runs `openlore scrape github rust-lang/cargo --sign 1,1,9`
  Then the CLI exits non-zero naming the duplicate index and the out-of-range index
  And no claim is composed, signed, or published

Scenario: A single-index selection behaves identically to single-candidate sign
  Given Maria has a candidate list
  When Maria runs `openlore scrape github rust-lang/cargo --sign 2`
  Then the flow is identical to selecting one candidate in US-SCR-003
```

### Acceptance Criteria

- [ ] `--sign <N,N,...>` accepts a comma-separated list of candidate indices.
- [ ] Each selected candidate is presented in its OWN slice-01 compose preview and requires the human's individual signing gesture.
- [ ] A running progress indicator shows "(k of M signed)".
- [ ] There is NO "sign all without review" affordance; batch is a convenience over the human-gate, never a bypass.
- [ ] A candidate can be skipped mid-batch (cancel its compose) without aborting the remaining selections; the summary reports signed vs skipped counts.
- [ ] An invalid selection list (duplicate index, out-of-range index) is rejected before any compose begins, naming the offending indices.
- [ ] A single index behaves identically to US-SCR-003.

### Outcome KPIs

See `outcome-kpis.md` KPI-SCR-1 (amortized cost-to-claim across several claims in
one session) and KPI-SCR-2 (human-gate preserved — each claim individually
signed).

### Technical Notes

- Depends on US-SCR-003 (single-candidate sign must work first).
- Implemented as an iteration over US-SCR-003's single-sign flow; no new publish path. The progress indicator and skip handling are CLI-driver concerns.
- DESIGN owns the exact skip gesture (Ctrl-C-per-candidate vs an explicit "skip" input); the product contract is "skip one without aborting the rest."

---

## US-SCR-006 `@infrastructure`: Bootstrap GithubPort, adapter-github, and scraper-domain

### `infrastructure_rationale`

This story exists to introduce the two new crates the brief calls for
(`adapter-github` + `scraper-domain`) and the new `GithubPort` trait that wires
them into the hexagonal architecture. It is an `@infrastructure` story because it
has no end-user-observable behavior on its own — every user-visible behavior is
in US-SCR-001..005. Without this story, those five stories cannot ship. It is
grouped with them in Release 1 (the walking skeleton release) because
US-SCR-001..003 (the slice-02 walking-skeleton trio) all depend on it.

The slice satisfies the BLOCKING slice-level Elevator Pitch check (per
`nw-po-review-dimensions` Dimension 0 §5): five user-visible stories
(US-SCR-001..005) accompany this one infrastructure story. The slice is NOT 100%
`@infrastructure`.

### Job link

- `job_id`: `infrastructure-only`

### Problem (infra perspective)

Slices 01 and 03 shipped 8 production crates with no GitHub surface. Slice-02
introduces TWO new crates per the brief: `adapter-github` (EFFECT shell over the
GitHub REST/GraphQL API) and `scraper-domain` (PURE derivation of candidate
claims from harvested signals). A new `GithubPort` trait in the `ports` crate
defines the boundary. None of these are user-visible on their own, but every
US-SCR-001..005 story depends on them. The hexagonal invariants (I-1..I-5) and
the functional pure/effect split (ADR-007) MUST hold for the new crates exactly
as they do for the existing ones.

### Solution (infra)

- Add `GithubPort` to the `ports` crate (PURE traits): `resolve_target(target)
  -> TargetKind`, `harvest_repo(repo) -> Vec<Signal>`, `harvest_user(user) ->
  Vec<Signal>`, plus a `ProbeOutcome`-returning `probe()` per ADR-009 I-4. The
  `Signal` and `CandidateClaim` ADTs live in pure code.
- Add `scraper-domain` crate (PURE core): `derive_candidates(signals,
  mapping) -> Vec<CandidateClaim>` and the signal->predicate mapping loader.
  No dependency on `tokio`/`reqwest`/`duckdb` (I-2). Depends only on
  `claim-domain` (for the claim shape) and `lexicon`.
- Add `adapter-github` crate (EFFECT shell): implements `GithubPort` over the
  GitHub API using the workspace HTTP client; reads the optional `GITHUB_TOKEN`;
  ships `probe()` with the 250ms budget (I-5). Only the `cli` crate wires it (I-3).
- Extend `cli`: add the `scrape github <target> [--sign ...]` verb; wire
  `adapter-github` -> `GithubPort` -> `scraper-domain` -> slice-01
  `VerbClaimAdd`/`VerbClaimPublish`.
- Optional Lexicon extension: IF DESIGN chooses to store the `derived-from`
  provenance in the signed payload, add it as an OPTIONAL field on
  `org.openlore.claim` (CID-stable when absent per ADR-005, mirroring the
  slice-03 `reason` field). Otherwise provenance stays display-only.
- Extend `xtask check-arch` to cover the two new crates: `scraper-domain` is
  pure (no I/O deps); `adapter-github` ships a `probe()`.

### Acceptance Criteria

- [ ] `GithubPort` trait exists in the `ports` crate with `probe()` per ADR-009 I-4.
- [ ] `scraper-domain` crate compiles as PURE: no dependency on `tokio`, `reqwest`, `duckdb`, `keyring`, or any I/O crate (I-2); `cargo xtask check-arch` passes.
- [ ] `adapter-github` crate implements `GithubPort`, reads the optional `GITHUB_TOKEN`, and ships a `probe()` that runs within the 250ms budget (I-5).
- [ ] Only the `cli` crate wires `adapter-github` into `GithubPort` (I-3).
- [ ] IF the `derived-from` provenance is stored in the signed payload, it is an OPTIONAL field that is CID-stable when absent (lexicon conformance test asserts this); otherwise provenance is display-only and no Lexicon change ships.
- [ ] `cargo xtask check-arch` and `cargo xtask check-probes` pass with the two new crates; `cargo deny check` passes for any new dependency (I-11).

### UAT Scenarios (BDD — infrastructure surface)

```gherkin
Scenario: The scraper crates honor the hexagonal pure/effect split
  Given the workspace includes scraper-domain and adapter-github
  When `cargo xtask check-arch` runs
  Then scraper-domain has no dependency on any I/O crate
  And adapter-github is only wired into GithubPort by the cli crate
  And the check passes

Scenario: The GitHub adapter ships a probe within the budget
  Given adapter-github implements GithubPort
  When `cargo xtask check-probes` runs
  Then adapter-github exposes a probe() returning a ProbeOutcome
  And the probe runs within the 250ms timeout budget
  And the check passes
```

### Outcome KPIs

n/a — supports KPI-SCR-1..4 indirectly (provides the harvest + derivation
surface every user-visible story builds on).

### Technical Notes

- Depends on slice-01 `claim-domain`, `lexicon`, `ports`, `cli` being present (this story extends, does not replace).
- New production crate count: +2 (`adapter-github`, `scraper-domain`). This is the FIRST slice to add crates since slice-01 (slice-03 added zero per WD-26). The brief's Component Inventory must gain two rows at finalize.
- Any new HTTP-client dependency for `adapter-github` MUST pass `cargo deny check` (I-11). Prefer reusing the workspace HTTP client already pulled in by `adapter-atproto-pds` to avoid a new dependency.
- Coordinates with US-SCR-003 on whether the `derived-from` provenance is a signed-payload field or display-only.

---

## Summary table

| Story | Title | Job link | Right-sized? | DoR status |
|---|---|---|---|---|
| US-SCR-001 | Harvest a public GitHub target's signals | J-004 | YES (1.5 days, 4 scenarios) | PASS (see DoR section in feature-delta.md) |
| US-SCR-002 | Derive auditable candidate claims from signals | J-004 | YES (2 days, 4 scenarios) | PASS |
| US-SCR-003 | Review, edit, and sign a candidate via slice-01 | J-004 + J-001 | YES (2 days, 5 scenarios) | PASS |
| US-SCR-004 | Use an optional PAT for higher rate limits | J-004 | YES (1.5 days, 4 scenarios) | PASS |
| US-SCR-005 | Select and sign several candidates in one pass | J-004 | YES (1 day, 4 scenarios) | PASS |
| US-SCR-006 | Bootstrap GithubPort + adapter-github + scraper-domain (`@infrastructure`) | `infrastructure-only` | YES (2 days, 2 scenarios) | PASS (with infra rationale) |

Total estimated effort: ~10 days at moderate confidence. Slice composition gate:
PASS — 5 user-visible stories + 1 infrastructure story; slice is NOT 100%
`@infrastructure` (per `nw-po-review-dimensions` Dimension 0 §5).
