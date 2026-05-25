# Visual Journey: Author and Publish a Claim

Persona: **P-001 Senior Engineer Solo Builder (Jeff-class)**
Job: **J-001 Author a signed philosophical claim**
Slice: **slice-01-claim-skeleton** (the walking skeleton)

## Flow

```
[Trigger]                [Step 1]            [Step 2]              [Step 3]            [Step 4]
Read a blog/README   ->  Compose claim   ->  Sign & persist    ->  Publish to PDS  ->  Read back via graph
that resonates           openlore claim       <Enter>               <Y>                 openlore graph query
                         add ...              (local file)          (federated)         (local-only by default)

Feels:                   Feels:               Feels:                Feels:              Feels:
Skeptical-curious   ->   Focused            ->Reassured          -> Quietly-confident-> Validated
"will this read as       "I see what          "It's mine,           "Federated, but     "That's MY reasoning,
 truth-assertion?"        will be signed"      locally, first"       retraction is        attributed, queryable"
                                                                     one command away"

Shared artifacts flowing across steps:
  author_did  -----------> step 1 -> 2 -> 3 -> 4   (source: ~/.config/openlore/identity.toml)
  claim_cid                       -> 2 -> 3 -> 4   (source: content-addressed hash)
  at_uri                                -> 3 -> 4  (source: derived from did + cid)
  composed_at -----------> step 1 -------> 4       (source: system clock UTC)
```

## Emotional arc

This is the **confidence-building-with-explicit-trust-buffer** pattern. Standard
confidence-build (anxious -> focused -> confident) is augmented with an explicit
trust buffer at step 1: the CLI prints the literal text "not as truth" in the
compose preview. This is the load-bearing UX moment that addresses the J-001
anxiety force ("am I being asked to assert this is true?").

The arc must not skip step 2 (local persist before publish). Going straight from
compose -> publish would force the user across a federated boundary without an
intermediate "it's mine, locally" beat.

```
confidence ^
           |                                       . end (validated)
           |                            . step 4
           |                  . step 3
           |          . step 2 (local-first beat)
           |  . step 1
           | . start (skeptical-curious)
           +--------------------------------------------> time
```

## Step 1: Compose claim intent

```
$ openlore claim add \
    --subject   github:rust-lang/rust \
    --predicate embodiesPhilosophy \
    --object    org.openlore.philosophy.memory-safety \
    --evidence  https://www.rust-lang.org/ \
    --confidence 0.86

Composing claim (not yet signed, not yet published)
----------------------------------------------------
  subject     : github:rust-lang/rust
  predicate   : embodiesPhilosophy
  object      : org.openlore.philosophy.memory-safety
  evidence    : https://www.rust-lang.org/
  confidence  : 0.86  (well-evidenced)
  author      : did:plc:jeff-test
  timestamp   : 2026-05-25T12:00:00Z

This is YOUR reasoning. It will be signed and published as a claim,
not as truth. Others can counter-claim or weight it independently.

Press Enter to sign, Ctrl-C to cancel, or rerun with --edit for a diff editor.
```

## Step 2: Sign and persist locally

```
Signing with did:plc:jeff-test ... ok
Computing claim CID            ... bafyreigh2akiscaildc...n4ka

Written to local store:
  path : ~/.local/share/openlore/claims/bafyreigh2akiscaildc...n4ka.json
  cid  : bafyreigh2akiscaildc...n4ka
  size : 412 bytes

Publish to your PDS now? [Y/n]
```

## Step 3: Publish to ATProto PDS

```
Publishing to https://pds.example.com ...
  record collection : org.openlore.claim
  record rkey       : bafyreigh2akiscaildc...n4ka
  ... ok (HTTP 200, 124ms)

Published.
  at-uri : at://did:plc:jeff-test/org.openlore.claim/bafyreigh2akiscaildc...n4ka
  local  : ~/.local/share/openlore/claims/bafyreigh2akiscaildc...n4ka.json

Tip: `openlore claim retract bafyreigh2akiscaildc...n4ka` to issue a retraction claim.
```

## Step 4: Read it back through the graph

```
$ openlore graph query --subject github:rust-lang/rust

Claims about github:rust-lang/rust (1 found, local store)
---------------------------------------------------------
  did:plc:jeff-test
    embodiesPhilosophy  org.openlore.philosophy.memory-safety
    confidence          0.86  (well-evidenced)
    evidence            https://www.rust-lang.org/
    at-uri              at://did:plc:jeff-test/org.openlore.claim/bafyreigh2akiscaildc...n4ka
    composed_at         2026-05-25T12:00:00Z

Showing local claims only. Use `--federated` to include subscribed authors
(subscriptions land in slice-03-federated-read).
```

## Trust-model UX choices that are load-bearing

1. **Compose-preview literal text "not as truth"** — Step 1 must include this exact
   reframing. If lost, the J-001 anxiety force is not addressed.
2. **Local-persist step distinct from publish** — Step 2 must be its own beat. Combining
   into "sign + publish in one command" forces a federated boundary the user has not yet
   accepted.
3. **`--federated` is opt-in for query** — Step 4 must default to local-only. Defaulting
   to federated would silently merge other authors' claims into what looks like the user's view.
4. **Retract hint at publish-time** — Step 3 must show the retract command. Telling the
   user "you can take this back" at the moment of publication is what makes publication
   feel safe.

## Shared artifact registry (compact)

| Artifact | Source of truth | Consumers | Integration risk |
|---|---|---|---|
| `author_did` | `~/.config/openlore/identity.toml` (resolved from ATProto session) | steps 1-4, signed payload, PDS, graph | HIGH — drift breaks attribution and signature verification |
| `claim_cid` | content-addressed hash of canonical signed claim | steps 2-4, local store filename, PDS rkey, graph node id, retract reference | HIGH — non-determinism breaks round-trip identity |
| `at_uri` | derived: `at://{author_did}/org.openlore.claim/{claim_cid}` | steps 3-4 | MEDIUM — derived value, mismatch implies upstream drift |
| `composed_at` | system clock (UTC, RFC3339) | steps 1, 4 | LOW |
| `local_claim_store` | `~/.local/share/openlore/claims/` (XDG) | steps 2-4 | MEDIUM — path drift breaks read-back |
