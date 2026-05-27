# Expanded Gherkin Scenarios — openlore-federated-read (slice-03)

- **Wave**: DISCUSS, ask-intelligent expansion (fired trigger: AC ambiguity around peer-trust + brigading)
- **Date**: 2026-05-27
- **Owner**: Luna (nw-product-owner)
- **Purpose**: complement the happy/edge/error scenarios embedded in
  `user-stories.md` and the journey YAMLs with anxiety-path and habit-path
  scenarios derived from the J-003 four-forces analysis.

J-003 has THREE distinct demand-reducing anxieties (per the updated `jobs.yaml`):

1. **Bad-actor absorption**: "What if I accidentally absorb a bad-actor's claims into my reasoning?" — mitigated by anti-merging + per-claim attribution.
2. **Subscription regret**: "What if I subscribe to a DID I later regret subscribing to — is subscription a one-way commitment?" — mitigated by US-FED-005 (`peer remove --purge`).
3. **Brigade reprisal**: "What if I publish a counter-claim and the peer's followers brigade me?" — same as J-001 anxiety, mitigated by claim-not-truth framing and soft-retract semantics.

The user-stories already cover happy / edge / error scenarios. This file adds:

- **4 anxiety-path scenarios** that validate the system's safety nets for the three J-003 anxieties (anxiety #1 gets two scenarios because it has two distinct attack surfaces).
- **2 habit-path scenarios** that validate the system actually shifts the user's default behavior.

Each scenario references its originating user story and the `job_id`. Where a
CLI verb or behavior is implied but not yet locked in `user-stories.md`, a
`# DISTILL: confirm` comment marks the scenario so the acceptance designer
can resolve it against DESIGN's final shape.

---

## Anxiety-path scenarios

### Anxiety scenario 1: bad-actor publishes a record with someone else's DID in the author field

`job_id`: J-003 (sub-job J-003a anti-merging)
Originating story: US-FED-002 (peer pull)
Anxiety addressed: "What if I subscribe to a peer who turns out to be an
adversary, and they publish a record claiming to be from a different DID I
trust, so the bad claim shows up attributed to the trusted DID in my view?"

```gherkin
Feature: Adversarial peer cannot inject claims attributed to other DIDs

  Scenario: A peer publishes a record whose signature does not verify against the peer's DID
    Given Maria has subscribed to did:plc:rachel-test
    And Rachel's PDS contains a record whose author field claims did:plc:trusted-test
        but whose signature does NOT verify against either did:plc:rachel-test or did:plc:trusted-test
    When Maria runs `openlore peer pull`
    Then the record is rejected with reason "signature invalid"
    And the record is NOT stored in peer_claims under ANY author DID
    And the pull summary explicitly flags the rejection
    And exit code is non-zero

  Scenario: A peer publishes a record correctly signed against an unexpected DID
    Given Maria has subscribed to did:plc:rachel-test
    And Rachel's PDS contains a record whose author field is did:plc:trusted-test
        AND whose signature DOES verify against did:plc:trusted-test
        (this is a legitimate cross-published record, NOT an attack)
    When Maria runs `openlore peer pull`
    Then the record is stored in peer_claims attributed to did:plc:trusted-test (NOT did:plc:rachel-test)
        # DISTILL: confirm — DESIGN must decide whether cross-attributed records
        #         are stored at all, or rejected with "peer published a record
        #         claiming to be from a different DID; rejected as out-of-scope
        #         for slice-03 subscription model." Recommended: REJECT for
        #         slice-03 (simpler trust model); revisit in slice-04 scoring.
    And Maria's subsequent federated query shows the record correctly attributed to its actual author DID
    And no claim is ever stored attributed to a DID whose signature it does not verify against
```

### Anxiety scenario 2: cached peer claims become stale or are tampered after-the-fact

`job_id`: J-003 (sub-job J-003a)
Originating story: US-FED-003 (federated query)
Anxiety addressed: "Once a claim is in my peer_claims store, what if the
local file gets tampered (disk corruption, malware, my own mistake) and the
query shows me bad data attributed to the peer?"

```gherkin
Feature: Cached peer claims are re-verifiable at query time

  Scenario: A cached peer claim with a corrupted signature is flagged at query time
    Given Maria has pulled Rachel's claim bafy...n4ka with valid signature
    And someone has tampered with the local file at ~/.local/share/openlore/claims/bafy...n4ka.json
        such that the signature no longer verifies
    When Maria runs `openlore graph query --subject <subject> --federated`
        # DISTILL: confirm — DESIGN must decide whether to re-verify peer claim
        #         signatures at every query (cost: re-verify per row) OR
        #         lazily when the user runs `openlore peer verify --all`
        #         (cost: explicit user step). Recommended: query-time re-verify
        #         IS the safety net; performance optimization deferred to
        #         slice-04 if needed.
    Then the row for bafy...n4ka displays the annotation "[signature invalid — re-pull required]"
    And the row's contents are still shown but visually distinguished
    And the footer warns about the count of unverified rows
    And exit code is non-zero to flag the integrity violation
```

### Anxiety scenario 3: subscription regret — unsubscribe must be visibly complete

`job_id`: J-003 (sub-job J-003c revocability)
Originating story: US-FED-005 (peer remove)
Anxiety addressed: "If I `peer remove --purge` a peer, can I verify that
their data is truly gone — not just that the CLI told me it was?"

```gherkin
Feature: Hard purge produces an auditable empty-state for the removed peer

  Scenario: Auditing after hard purge confirms zero residue
    Given Maria has subscribed to did:plc:rachel-test
    And Maria has pulled 12 of Rachel's claims
    And Maria has authored 2 counter-claims against 2 of Rachel's claims
    When Maria runs `openlore peer remove did:plc:rachel-test --purge` and confirms
    And Maria then runs `openlore peer audit did:plc:rachel-test`
        # DISTILL: confirm — `peer audit` is not in US-FED-005 AC. It is the
        #         audit verb implied by the revocability promise. DESIGN may
        #         surface this as `peer status did:plc:rachel-test`, or
        #         `peer info did:plc:rachel-test`, or fold it into
        #         `peer list --include-purged`. Confirm the verb shape.
    Then the audit reports "did:plc:rachel-test: not subscribed; 0 cached peer claims; 2 of your counter-claims still reference this peer's CIDs (preserved as your own published artifacts)"
    And the audit does NOT show any cached peer_claims rows attributed to Rachel
    And the audit DOES list Maria's 2 counter-claims with a "counters bafy...n4ka (peer not subscribed)" annotation
    And the audit exits 0
```

### Anxiety scenario 4: brigade reprisal — counter-claim flow does not over-expose the author

`job_id`: J-003 (sub-job J-003b) + J-001 brigading anxiety
Originating story: US-FED-004 (counter-claim authoring)
Anxiety addressed: "If I publish a counter-claim, am I starting a brigade
war? Can I retract gracefully if it spirals?"

```gherkin
Feature: Counter-claims are retractable via the same soft-retract semantics as any claim

  Scenario: Counter-claim author retracts their own counter via the slice-01 retract path
    Given Maria has published counter-claim bafy...new countering Rachel's bafy...n4ka
    And the response community has reacted strongly to bafy...new
    When Maria runs `openlore claim retract bafy...new`
    Then a retraction claim is signed and published referencing bafy...new
        (per WD-11: retraction is itself a counter-claim referencing the original CID)
    And bafy...new STILL resolves on Maria's PDS at its original at-uri
        (soft-retract; hard-delete forbidden per WD-11)
    And subsequent federated queries show bafy...new annotated "retracted by author"
        AND Rachel's bafy...n4ka annotated "countered-by bafy...new (retracted by author)"
    And NO hard-delete option is offered even with --force
    And the retraction publication does NOT trigger a notification to Rachel
        # DISTILL: confirm — notifications are out of slice-03 scope per the
        #         deferred-features list in story-map.md. This scenario verifies
        #         that the system does NOT auto-notify peers of retractions.

  Scenario: Peer of a countered claim observes the retraction in their own federated query
    Given Rachel is subscribed to Maria (did:plc:maria-test)
    And Maria has retracted her counter-claim bafy...new
    When Rachel runs `openlore peer pull` and then `openlore graph query --subject <subject> --federated`
    Then Rachel sees bafy...new in her peer_claims attributed to Maria
        AND annotated "retracted by author"
    And Rachel's own bafy...n4ka still appears with "countered-by bafy...new (retracted by author)"
    And no claim is hidden; retraction is a public structured annotation, not a delete
```

---

## Habit-path scenarios

These scenarios test that the system actively reshapes the user's existing
default behavior.

### Habit scenario 1: bridging from passive HN-skim to active subscription

`job_id`: J-003 (the federated-read habit force)
Originating story: US-FED-001 (peer add)
Habit addressed: the user currently reads aggregator sites (HN, Reddit,
awesome-lists) and never structurally "follows" anyone — following is
something they associate with social media, not with technical knowledge.

```gherkin
Feature: A first-time peer-add session normalizes subscription as a technical primitive

  Scenario: Maria adds her first peer and immediately runs a federated query
    Given Maria has authenticated as did:plc:maria-test
    And Maria has zero peer subscriptions
    When Maria runs `openlore peer add did:plc:rachel-test`
        AND immediately follows with `openlore peer pull`
        AND then `openlore graph query --subject github:rust-lang/cargo --federated`
    Then the peer add output explicitly suggests the next two commands
        (`openlore peer pull` and `openlore graph query --federated`)
        with copy-pasteable lines
    And the peer pull output, on first ever pull, prints a one-line orientation
        (`First federated pull complete. From now on, run 'openlore peer pull'
         on demand — claims do not auto-refresh.`)
        # DISTILL: confirm — the first-pull orientation message is not in US-FED-002
        #         AC. It is a habit-bridging affordance. DESIGN may choose to
        #         show it once-per-user (state in identity.toml) or every pull
        #         that adds new peers. Confirm the trigger condition.
    And the federated query output, on the first ever invocation with --federated,
        prints a one-line orientation
        (`First federated query complete. Peer claims appear under their author DIDs.
         No claims are merged. Use `openlore peer add <did>` to follow more peers.`)
    And total elapsed time from `peer add` to first federated query result is under 30 seconds
```

### Habit scenario 2: bridging from "disagree privately" to counter-claim authoring

`job_id`: J-003 (sub-job J-003b) + J-001 habit force
Originating story: US-FED-004 (counter-claim authoring)
Habit addressed: when a user disagrees with a peer's claim, their default
today is to (a) mutter "well, that's wrong" privately, (b) post a reply on
social media, or (c) write a blog post. The counter-claim verb must feel as
light as those existing defaults — otherwise the verb exists but is unused.

```gherkin
Feature: The federated query output makes counter-claim authoring the path of least resistance

  Scenario: User reads a peer claim they disagree with; counter-claim is one copy-paste away
    Given Maria has pulled Rachel's claim bafy...n4ka about github:rust-lang/cargo
    And Maria disagrees with the claim
    When Maria runs `openlore graph query --subject github:rust-lang/cargo --federated`
    Then the output for Rachel's claim row includes a copy-pasteable counter template:
        `openlore claim counter bafy...n4ka --reason "..." --subject github:rust-lang/cargo --predicate embodiesPhilosophy --object org.openlore.philosophy.dependency-pinning --evidence ... --confidence ...`
        # DISTILL: confirm — the inline counter template is more aggressive than
        #         the current US-FED-003 AC, which only requires a tip line.
        #         DESIGN may choose to show the template only via `--verbose`
        #         or `--counter-prompt`. Confirm.
    And the template pre-fills subject, predicate, and object from the target claim
    And the template requires the user to provide --reason, --evidence, --confidence
    And the user can copy, paste, edit, and run in under 60 seconds end-to-end

  Scenario: First-ever counter-claim authoring includes a one-time framing message
    Given Maria has never authored a counter-claim before
    When Maria runs `openlore claim counter bafy...n4ka --reason "..." [other flags]`
    Then the compose preview, on this first invocation, includes a one-time framing block:
        ```
        First counter-claim! Some context:
          - A counter-claim is a SIGNED public artifact attributed to YOU.
          - It does NOT delete or hide the target claim; both coexist.
          - You can retract it later via `openlore claim retract <your_cid>`.
          - The target peer is NOT auto-notified; they will see it next time
            they pull your claims (if they subscribe to you).
        ```
        # DISTILL: confirm — this one-time framing is a habit-bridging affordance.
        #         DESIGN may choose to show it the first 3 times, or only the
        #         first time. State lives in identity.toml. Confirm the trigger.
    And on subsequent counter-claim invocations the framing block is omitted
    And the framing block does NOT delay or modify the standard "not as truth" + "counter-claims coexist" framing
```

---

## Coverage summary

| Force | Scenario count | Stories touched | Sub-jobs |
|---|---|---|---|
| Anxiety (bad-actor absorption) | 2 scenarios across 1 feature | US-FED-002, US-FED-003 | J-003a |
| Anxiety (subscription regret) | 1 scenario | US-FED-005 | J-003c |
| Anxiety (brigade reprisal) | 2 scenarios | US-FED-004, slice-01 retract | J-003b + J-001 |
| Habit (HN-skim -> subscription) | 1 scenario | US-FED-001, US-FED-002, US-FED-003 | J-003 (general) |
| Habit (disagree privately -> counter-claim) | 2 scenarios | US-FED-003, US-FED-004 | J-003b |
| Push + Pull (already covered) | n/a | US-FED-001..006 | — |

This file meets the ask-intelligent expansion target (3+ anxiety + 2+ habit;
delivered 4 anxiety + 2 habit). DISTILL should treat the `# DISTILL: confirm`
comments as resolution checkpoints — each one indicates a behavior implied by
the requirement but not yet locked in `user-stories.md`. None of these
scenarios introduce a new *behavior contract* DESIGN has not seen; they
stretch existing contracts across additional force-driven situations.

---

## Changelog

- 2026-05-27 — Luna — initial write under ask-intelligent expansion (AC ambiguity trigger: peer-trust + brigading + revocability semantics).
