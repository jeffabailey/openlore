# JTBD Job Stories — bluesky-claim-review-app

> Registry: `docs/product/jobs.yaml`. This feature ADDS **J-009** (primary for this feature)
> with sub-jobs J-009a..f, and ADDS **J-010** (a later job, mapped here but out of v1 scope).
> It REUSES J-001, J-004 (J-004b/c/e) and J-007 as related jobs.

## Why a new job (not a sub-job of J-004 or J-001)

J-004 is about evaluating **someone else's** body of work. The operator scrapes a target and
signs claims **as the author, about the target**. This feature turns that around: the
**subject is the signed-in person themselves**, the author is **the same person**, and the
person **does not run the CLI, hold a keychain key, or manage an app password**. The
motivation is different (self-presentation with consent, not evaluation), and so is the
anxiety ("what will it say about ME, and who sees it before I agree?"). That warrants a
first-class job. J-001 (author a signed claim) is the mechanism underneath. J-009 is the
reason a non-CLI Bluesky user would ever reach it.

## J-009 — Curate a consented, self-attested philosophy profile from my Bluesky identity

**Job story**

> When I'm a developer on Bluesky whose public GitHub work says something about how I build,
> I want OpenLore to suggest the philosophies my work embodies and let me approve, edit, or
> privately decline each one, using only my Bluesky sign-in,
> so I can publish an accurate, self-attested picture of how I build into my own PDS,
> where nothing is said about me without my consent.

- **Functional:** sign in with my Bluesky handle. Prove my GitHub account is mine. See
  suggestions inferred from my public GitHub work. Approve, edit or decline each one. Write
  the approved ones as `org.openlore.claim` records into **my own** PDS.
- **Emotional:** in control ("nothing goes out until I say so"); recognised ("that IS how I
  build"); proud to share.
- **Social:** a credible, self-attested signal of my engineering values that is visible to
  peers on the network I already use. It is not a profile someone else wrote about me.

### Sub-jobs

| ID | Name | Job story (abridged) | Load-bearing |
|----|------|----------------------|--------------|
| J-009a | Use my existing Bluesky identity — no keys, no CLI | When I want to take part, I want to sign in with the Bluesky handle I already have, so I don't install a CLI, manage a keychain key or paste an app password. | yes |
| J-009b | Prove the GitHub account is mine before anything is inferred | When the app reads a GitHub account, I want it to require proof that I own it (my DID in my GitHub bio), so nobody, me included, can claim someone else's repos. | yes |
| J-009c | Review machine suggestions privately, one card at a time | When suggestions appear, I want to see each one's philosophy, confidence and evidence in a queue only I can see, so I judge them without anyone watching or any of it leaking. | yes |
| J-009d | Approve (optionally edited) into my own PDS, self-attested | When I agree with a suggestion, I want to adjust confidence or swap the philosophy, preview the exact record, and write it to my own PDS, so the published claim is my reasoning in my repo, and OpenLore accepts it as self-attested. | yes |
| J-009e | Decline privately and never be nagged again | When a suggestion is wrong, I want to decline it without publishing anything, so it never comes back and nobody learns I declined it. | yes |
| J-009f | Share my approved profile on Bluesky, on my terms | When I'm proud of my approved claims, I want to preview and explicitly post a link to my OpenLore profile, so my followers see it. Nothing is posted unless I confirm. | no |

## J-010 — Find developers on Bluesky who share my philosophies (LATER — not v1)

> When I've published how I build, I want to find people on Bluesky who build the same way,
> through a feed or a badge inside the Bluesky app I already use,
> so I can follow and collaborate with them without leaving my social habitat.

It is realized later by the feed generator and labeler slices on the story map. It is
related to J-005 (discover across the network) but **inside Bluesky's own surfaces** rather
than OpenLore's search.

## Related existing jobs

- **J-001** (author a signed claim). J-009d is a new authoring channel. RC-02 (soft-retract,
  never hard-delete) and WD-10 (display-only buckets) carry over.
- **J-004b/c** (candidates; the human always signs). J-009c/d reuse the candidate
  derivation, and the "never auto-publish" rule carries over unchanged.
- **J-004e** (person inference). This becomes a LATER source of person-level self-claims.
- **J-007** (shareable philosophy card linked from Bluesky). J-009f targets the same social
  surface, but for a user who self-hosts nothing.
