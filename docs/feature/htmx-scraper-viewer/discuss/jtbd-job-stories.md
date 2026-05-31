# JTBD Job Stories: htmx-scraper-viewer (slice-06)

> Feature framing: **DELTA on the existing 19-crate functional-Rust workspace.** Prior
> slices (01 foundation, 02 github-scraper, 03 federated-read, 04 scoring-graph,
> 05 appview-search) are all SHIPPED. This slice adds a **read-only htmx UI** for the
> **node operator** on **localhost**. Signing stays in the CLI; no signing key in the
> web process (inherited invariant I-SCR-1).

The node operator runs an OpenLore node on their own machine. They have two distinct,
recurring jobs that this feature serves. Job 1 (store inspection) is the **north star**;
Job 2 (live-scrape browsing) is secondary.

---

## Job 1 (PRIMARY / north star): See what is in my store

### Job Story

> **When** I have been running my OpenLore node for a while — signing my own claims and
> pulling federated claims from peers into my local DuckDB store —
> **I want to** browse the actual persisted contents of my node (my signed `claims` and
> the `peer_claims` I have federated) in my browser, without writing raw SQL,
> **so I can** confirm what my node actually holds, spot-check a claim's subject /
> predicate / object / evidence / confidence, and trust that my node's state matches my
> mental model.

### Dimensions

- **Functional**: Render the persisted rows of the local DuckDB store (`claims` +
  `peer_claims`) as readable HTML — subject, predicate, object, evidence, confidence,
  author DID, composed-at, CID — so the operator can answer "what is in my node?" by
  looking, not by querying.
- **Emotional**: Move from *blind / uneasy* ("I do not actually know what my node holds")
  to *grounded / in control* ("I can see it; my node's state is legible to me"). Relief
  that inspection requires zero SQL and zero risk of mutating anything.
- **Social**: As a node operator in a federated network, being able to *show* what one's
  node holds — and to vouch credibly for it — depends on the operator themselves having a
  clear, trustworthy view first. Legibility precedes accountability.

### Concrete grounding (real data)

- Operator **Maria Santos** (`did:plc:maria...`) has signed 312 claims and federated
  1,840 `peer_claims` from 4 peers. She wants to confirm her claim
  `("rust-lang/rust", "is-maintained-by", "The Rust Project")` with confidence `0.90`
  is actually persisted and carries the evidence URL she attached.

---

## Job 2 (SECONDARY): Browse scrape proposals in the browser

### Job Story

> **When** I am considering harvesting claims from a GitHub target and I want to weigh
> which candidate claims are worth signing,
> **I want to** enter a scrape target in my browser and see the proposed `CandidateClaim`
> values rendered as readable HTML (the same proposals the CLI `scrape github <target>`
> would show without `--sign`),
> **so I can** evaluate the candidates visually — rather than scrolling CLI batch text —
> before deciding (back in the CLI) which, if any, to sign.

### Dimensions

- **Functional**: Accept a scrape target, run the live harvest + candidate-derivation
  (the propose step of slice-02, no persistence), and render the resulting in-memory
  `CandidateClaim` ADTs as HTML. Read-only: no sign action, no key, no DB write.
- **Emotional**: Move from *squinting at batch text* (awkward to scan a wall of CLI
  output) to *scanning a legible list* (each candidate distinct, its `derived-from`
  provenance shown for display only). Calm certainty that browsing cannot accidentally
  sign or persist anything.
- **Social**: Reviewing candidates carefully before signing is an act of stewardship —
  the operator is the human gate (I-SCR-1) protecting the network from low-quality or
  spurious claims. A legible review surface reinforces that responsibility.

### Concrete grounding (real data)

- Maria enters target `tokio-rs/tokio`. The live scrape proposes 7 `CandidateClaim`
  values (e.g. `("tokio-rs/tokio", "has-license", "MIT")`, each with a `derived-from`
  badge such as `LICENSE file @ HEAD`). She scans them in the browser; nothing is
  persisted; she later signs two of them from the CLI.

---

## Job relationship

Job 1 answers *"what do I already hold?"* (database-backed, offline-capable).
Job 2 answers *"what could I add?"* (network-backed, live, no persistence).
They share the **node operator** persona, the **localhost** setting, the **read-only**
guardrail, and the **claim** as the central domain object — but they draw from different
sources (persisted DuckDB vs live harvest) and have different availability requirements
(offline vs network). Job 1 is the heart of the feature; Job 2 mirrors an existing CLI
capability in a more legible surface.
