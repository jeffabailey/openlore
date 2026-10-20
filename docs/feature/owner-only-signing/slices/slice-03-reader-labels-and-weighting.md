# Slice 03 — Readers label every GitHub claim and weigh only attested ones

> Story: S-OOS-7 · Persona: P-001 (Jeff) · Job: J-002 (J-002c) · Infrastructure-only: **no**
> Depends on: slice 02 (stored authority checks) · Estimate: ~3 days
> Derived from DESIGN slice 3; no DISCUSS interviews.

## User-visible outcome

`openlore graph query memory-safety` shows every claim about a GitHub account or repo with whose
word it is: `owner`, `repo-listed`, `third-party` or `authority not checked`. Nothing is hidden.
`--weighted --explain` counts only attested claims and prints what it left out, for example
"2 claims not counted: 1 third-party, 1 authority not checked", with the re-check command.
`--include-unattested` adds a per-author section and leaves the weighted rows unchanged. The
viewer shows the same labels and weights as the terminal. `openlore search` labels index rows
"authority not checked here" unless Jeff has his own check.

## IN scope

- Labels in `graph query`, `infer people`, scrape views, viewer `/score` and `/project`, and
  `search`.
- `score_attested` as the only scoring entry. Counters, retractions and links are never counted.
- `--include-unattested`.

## OUT of scope

- Any indexer change (C-OOS-8). Hiding claims (C-OOS-5).

## Acceptance (scenarios)

RD-1..RD-10, CORE-OOS-5.

## Learning hypothesis

KPI-OOS-2: adding a counter, a retraction, a link or an unattested claim never moves a weight. A
second learning: whether weighted views shrink so far before `authority verify` runs that users
stop trusting them. The "not counted" line is meant to point them to the re-check command.
