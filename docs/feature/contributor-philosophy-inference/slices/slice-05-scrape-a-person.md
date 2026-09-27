# Slice 05 — `scrape github <user>` reads a person

> Story: US-CPI-005 (J-004 / J-004a, J-004e) · Persona: P-001 (Maria), P-002 (Priya) · Depends on:
> slices 01-03 · Estimate: ~0.5-1 day

## Goal

Replace today's empty user-target result ("no candidate claims could be derived") with the person's
picture from the local store: linked repos with rank, signed adherence claims, and inferred candidates
— same numbering and `--sign N` as `infer people --person github:<user>`. Still one `/users/{user}`
request; no crawl of the person's repos (D-4).

## Learning hypothesis

- **Disproves if it fails**: that the user-target scrape is the natural entry point for "reading
  people". If Maria still reaches for `infer people --person` instead, the sugar is noise and should be
  dropped rather than maintained.
- **Confirms**: the original "read software AND people" intent is met from one familiar command.

## IN scope

- User-target render: links (repo + rank + last observed), signed adherence claims (CID, confidence),
  inferred candidates; `--sign` delegates to the slice-03 path.
- Unknown person (no links) → guidance to scrape their repos, exit 0.
- Non-existent user → existing not-found error, non-zero exit.

## OUT of scope

- Fetching the person's repo list or scraping their repos automatically; DID linking (OD-CPI-1).

## Acceptance criteria (production data)

- [ ] After scraping `BurntSushi/ripgrep` and `rust-lang/regex`, `openlore scrape github BurntSushi`
      lists both repos with his rank, any signed adherence claims, and numbered candidates.
- [ ] `openlore scrape github BurntSushi --sign 1` signs the same candidate as
      `openlore infer people --person github:BurntSushi --sign 1`.
- [ ] `openlore scrape github octocat` (no links) prints the guidance and exits 0.
- [ ] Exactly one GitHub request is made for a user-target scrape.

## Dogfood moment

Priya-hat: Maria evaluates a maintainer she might invite to collaborate, from one command, in <30 s.

## Dependencies

- slices 01-03; `harvest_user` path in `crates/adapter-github/src/lib.rs` (render change in `cli`).
