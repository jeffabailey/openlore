# JTBD Four Forces — J-009 (bluesky-claim-review-app)

## Push (frustration with today)

- To appear in OpenLore today, a developer must install the CLI, create a keychain signing
  key, get `#org.openlore.application` published in a PLC document, and log in with an app
  password. Only the maintainer has done this. For an ordinary Bluesky user, the cost is
  effectively infinite.
- Other people can already scrape and sign claims ABOUT a developer (J-004). The developer
  has no lightweight way to say "yes, that's me" or "no, that's not how I build" in their
  own voice.
- Bluesky bios and pinned posts are unstructured. "I care about memory safety" is not
  evidence-backed and not queryable.

## Pull (attraction of the new way)

- Sign in with the handle I already have. In under 5 minutes, see suggestions grounded in my
  own repos, with evidence links.
- The approved claims are mine. They live in my own PDS and travel with my account.
- A profile and a one-tap share post let my followers see how I build.

## Anxiety (what could go wrong)

| # | Anxiety | Mitigation (requirement) |
|---|---------|--------------------------|
| A1 | "Will it publish things about me I didn't agree to?" | I-BRA-1: pending suggestions are visible only to the signed-in owner. No PDS write, post, profile or search exposure until an explicit approve. I-BRA-3: every write traces to a confirm click. |
| A2 | "Will people see what I declined?" | D-4 / I-BRA-2: declines are private app-side state and never written anywhere public. |
| A3 | "Does granting OAuth let this app post or write whatever it wants?" | Least privilege: the consent screen names what it can write. Share posting only happens on explicit confirm (I-BRA-6). OD-BRA-7 covers scope granularity. |
| A4 | "Could someone else claim MY repos, or could I be tricked into claiming theirs?" | D-3 / I-BRA-4: the GitHub bio must contain the signed-in DID exactly. No scan before that. |
| A5 | "Will a machine label me wrongly and make it look like I said it?" | Suggestions start at speculative confidence (0.25). Edit or swap before approving. The evidence is always shown. Retract later (RC-02). |
| A6 | "Will OpenLore treat my claims as second-class or unverified because I have no CLI key?" | D-5 / I-BRA-5: self-attested (repo-signed) provenance is accepted and labelled, never rendered as unverified. |
| A7 | "Can I leave and take my data out of the app?" | US-BRA-012: disconnect and forget. App-side state is purged. PDS records remain mine to retract. |

## Habit (inertia to overcome)

- Bluesky users are used to "Sign in with Bluesky" on third-party apps, such as feed tools
  and starter-pack sites. The flow must look like those apps, not like a developer CLI.
- Developers already curate a GitHub profile README and bio. Pasting a DID into the bio fits
  that habit (Keybase-style proof).
- People triage notification and PR queues with fast approve and dismiss actions. The review
  queue must feel that quick: keyboard-friendly, one decision per card.
