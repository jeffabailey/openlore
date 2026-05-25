# Expanded Gherkin Scenarios — openlore-foundation (slice-01)

> Wave: DISCUSS, ask-intelligent expansion (fired trigger: AC ambiguity around
> trust/confidence semantics)
> Date: 2026-05-25
> Owner: Luna (nw-product-owner)
> Purpose: complement the happy/edge/error scenarios already embedded in
> `journeys/author-and-publish-claim.yaml` and `discuss/user-stories.md` with
> anxiety-path and habit-path scenarios derived from the J-001 four-forces analysis.

J-001's load-bearing demand-reducing force is **anxiety** ("what if I publish and
someone brigades me / what if I'm wrong / what if someone with more authority
disagrees"). The journey YAML and US-001..US-004 currently cover only happy, edge,
and error paths. This file adds:

- **3 anxiety-path scenarios** that validate the system's safety nets.
- **2 habit-path scenarios** that validate the system actually changes behavior the
  user defaults to today.

Each scenario references its originating user story and the `job_id`. Where a CLI
verb does not yet appear verbatim in `user-stories.md` or the journey YAML, a
`# DISTILL: confirm command name` comment marks the scenario so the acceptance
designer can resolve the verb against DESIGN's final CLI structure.

---

## Anxiety-path scenarios

### Anxiety scenario 1: brigading / public disagreement after publishing

`job_id`: J-001
Originating stories: US-003 (publish), US-001 (compose framing)
Anxiety addressed: "What if I publish and someone with more authority disagrees
and brigades my claim?"

```gherkin
Feature: A published claim remains visible and counter-claimable, never deletable

  Scenario: A peer publishes a counter-claim with damning evidence
    Given Jeff has published claim bafy...n4ka asserting that github:rust-lang/rust
          embodies org.openlore.philosophy.memory-safety with confidence 0.86
    And Maria (did:plc:maria-test) has subscribed to Jeff's claim stream
    When Maria publishes a counter-claim
        # DISTILL: confirm command name — likely `openlore claim add` with an
        #         additional `--counters bafy...n4ka` flag, or
        #         `openlore claim counter bafy...n4ka` as a sugar verb.
          that references bafy...n4ka via the counter-claim CID-reference field
          asserting the same subject with confidence 0.92 and contradicting evidence
    Then Jeff's local store shows BOTH claims when queried by subject github:rust-lang/rust
    And each claim displays its own author DID and confidence
    And no claim is rendered as "winning" or "merged"
    And neither claim is hidden or down-weighted automatically

  Scenario: User is notified of an inbound counter-claim against one of their claims
    Given Jeff has published claim bafy...n4ka
    And Maria has published a counter-claim referencing bafy...n4ka
    When Jeff runs `openlore claim status bafy...n4ka`
        # DISTILL: confirm command name — `claim status` is the closest existing
        #         verb shape; could also be `claim inspect` or surface this in
        #         `graph query --subject ... --include-counters`.
    Then the output lists the counter-claim with Maria's DID and confidence
    And the output reminds Jeff that retraction is `openlore claim retract bafy...n4ka`
    And the output explicitly states that NO hard-delete option exists
    And the output explains that retraction is itself a new claim and is also public

  Scenario: Soft-retract via counter-claim preserves public history
    Given Jeff has published claim bafy...n4ka
    And Maria has published a damning counter-claim against it
    When Jeff issues `openlore claim retract bafy...n4ka`
    Then a new retraction claim is signed and published referencing bafy...n4ka
    And bafy...n4ka still resolves on Jeff's PDS at its original at-uri
    And bafy...n4ka still appears in `openlore graph query --subject github:rust-lang/rust`
    And the query output annotates bafy...n4ka as "retracted by author"
    And no hard-delete command is offered, even with --force
```

### Anxiety scenario 2: "I made a mistake — the evidence URL was wrong"

`job_id`: J-001
Originating stories: US-001 (compose), US-003 (publish), retract (per WD-11 / OD-3)
Anxiety addressed: "What if I publish something with a typo or wrong evidence?
Can I take it back without it being deleted out from under me?"

```gherkin
Feature: Mistakes are corrected by issuing new claims, never by deleting old ones

  Scenario: Author corrects a typo'd evidence URL by publishing a corrective claim
    Given Jeff has published claim bafy...n4ka with evidence
          https://www.rustt-lang.org/    # note the typo
    And Jeff notices the typo after publishing
    When Jeff runs `openlore claim add` for the same subject/predicate/object
          with corrected evidence https://www.rust-lang.org/
          and a corrective-reference field pointing to bafy...n4ka
        # DISTILL: confirm command name — the corrective-reference field name is
        #         not yet specified in the Lexicon; DESIGN owns the field; the
        #         CLI flag could be `--corrects bafy...n4ka` or `--supersedes`.
    Then the new claim is signed with a new CID bafy...m9pq
    And the new claim is published to Jeff's PDS
    And the new claim's body contains a reference back to bafy...n4ka
    And bafy...n4ka REMAINS visible on the PDS and in graph queries
    And `openlore graph query --subject <subject>` lists BOTH claims in
        chronological order, each with its own author DID and CID
    And the listing annotates bafy...m9pq as "corrects bafy...n4ka"
```

### Anxiety scenario 3: calibration anxiety — "what if my confidence is wrong?"

`job_id`: J-001
Originating stories: US-001 (compose preview), bound by WD-10 (OD-2)
Anxiety addressed: "I'm about to commit to confidence 0.9 (well-evidenced) but I'm
not actually sure. Can I downgrade before this becomes public?"

```gherkin
Feature: Confidence bucket label is visible in the preview AND is downgradeable

  Scenario: User reconsiders confidence after seeing the bucket label in the preview
    Given Jeff has authenticated as did:plc:jeff-test
    When Jeff runs `openlore claim add` with --confidence 0.9 and otherwise valid flags
    Then the compose preview displays "0.90 (well-evidenced)"
    And the preview contains the literal text "not as truth"
    And the preview offers an --edit option to re-open the flags before signing

  Scenario: User downgrades from well-evidenced to weighted before signing
    Given Jeff is at the compose preview with confidence 0.9 displayed as "well-evidenced"
    When Jeff cancels the prompt and re-runs `openlore claim add` with --confidence 0.55
        # DISTILL: if `--edit` opens a flag-rewriting prompt in DESIGN, this
        #         scenario can collapse to "Jeff selects --edit, lowers confidence
        #         to 0.55, and the preview re-renders" — confirm command flow.
    Then the new preview displays "0.55 (weighted)"
    And no claim has been signed during this exchange
    And no file has been written under ~/.local/share/openlore/
    And no network call has been made

  Scenario: Signed payload never contains the bucket label
    Given Jeff has composed a claim with --confidence 0.55
    When Jeff confirms the sign step
    Then the signed claim file at ~/.local/share/openlore/claims/<cid>.json
          contains the numeric value 0.55
    And the signed claim file does NOT contain the bucket label "weighted"
    And the signed claim file does NOT contain ANY bucket label
```

---

## Habit-path scenarios

These scenarios test that the system actively reshapes the user's existing default
behavior. The J-001 habit force is "engineers already write blog posts and README
opinions; the new behavior must feel at most as heavy as `git commit -m`."

### Habit scenario 1: nudge from blog-post default toward claim-authoring

`job_id`: J-001
Originating story: US-001 (compose entry point)
Habit addressed: the user reflexively opens a blog editor when they have a
philosophical opinion.

```gherkin
Feature: A `--from-url` hint nudges the user from blog-post drafting toward claim-authoring

  Scenario: User has a URL they were about to blog about; claim-authoring is one verb away
    Given Jeff has a browser tab open at https://www.rust-lang.org/
    And Jeff was about to draft a blog post asserting that Rust embodies memory-safety
    When Jeff instead runs
          `openlore claim add --from-url https://www.rust-lang.org/`
        # DISTILL: confirm command name — `--from-url` is not yet in US-001 as a
        #         flag; it is proposed here as a habit-bridging affordance. DESIGN
        #         may choose `--evidence https://...` only and treat URL pre-fill
        #         differently. Confirm the affordance.
    Then the CLI parses the URL and pre-populates the --evidence flag
    And the CLI prompts for the still-required subject/predicate/object/confidence
    And on completion the compose preview displays the URL as evidence
    And the preview contains the literal text "not as truth"
    And the total elapsed time from URL to signed claim is under 60 seconds
```

### Habit scenario 2: nudge from passive reading toward first authoring

`job_id`: J-001 (the authoring nudge) + J-003 (the federated read context)
Originating story: US-004 (read back), and the future federated-read flow in slice-03
Habit addressed: the user reads other people's claims but never publishes their
own — the classic lurker pattern that produces a sparse graph.

```gherkin
Feature: A contribution view nudges habitual readers to publish their first claim

  Scenario: Reader-only user is shown a contribution gap and a smallest-first-claim suggestion
    Given Maria has authenticated as did:plc:maria-test
    And Maria has used `openlore graph query --subject <various>` 50 times in the last 30 days
    And Maria has never run `openlore claim add` successfully
    When Maria runs `openlore graph contrib --me`
        # DISTILL: confirm command name — `graph contrib` does not yet appear in
        #         user-stories.md; the closest existing verb is `graph query`.
        #         DESIGN may surface this as `claim stats --me`, `me`, or
        #         `whoami --contribs`. Confirm.
    Then the output reports the read count (50 claims read across N subjects)
    And the output reports the publish count (0 claims published)
    And the output proposes the smallest possible first claim, naming a real
        recently-queried subject and suggesting a `claim add` command line
    And the proposed claim is pre-filled with subject and a sensible default
        predicate; the user provides object, evidence, and confidence
    And the message reminds Maria that publication is opt-in (per US-003)
    And the message includes the literal text "not as truth" framing

  Scenario: Contribution view stays silent for users who already publish regularly
    Given Jeff has authenticated as did:plc:jeff-test
    And Jeff has published 12 claims in the last 30 days
    When Jeff runs `openlore graph contrib --me`
        # DISTILL: confirm command name — same flag as scenario above.
    Then the output reports the read and publish counts
    And the output does NOT nudge Jeff to publish a first claim
    And the output thanks Jeff briefly for contributing
```

---

## Coverage summary

| Force | Scenario count | Stories touched |
|---|---|---|
| Anxiety | 3 (with 5 sub-scenarios) | US-001, US-003, retract (per WD-11) |
| Habit | 2 (with 3 sub-scenarios) | US-001, US-004, slice-03 horizon |
| Push (already covered) | — | US-001, US-002 |
| Pull (already covered) | — | US-001, US-003, US-004 |

This file meets the ask-intelligent expansion target (3 anxiety + 2 habit). DISTILL
should treat the `# DISTILL: confirm command name` comments as resolution
checkpoints — each one indicates a CLI verb that is implied by the requirement but
not yet locked in `user-stories.md`. None of these scenarios introduce a new
*behavior contract* DESIGN has not seen; they only stretch existing contracts
across additional force-driven situations.

---

## Changelog

- 2026-05-25 — Luna — initial write under ask-intelligent expansion (AC ambiguity trigger).
