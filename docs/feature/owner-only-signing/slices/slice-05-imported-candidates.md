# Slice 05 — Imported candidates are kept locally for offline review

> Story: S-OOS-9 · Persona: P-001 (Jeff) · Job: J-004 (J-004b) · Infrastructure-only: **no**
> Depends on: slice 02 (scrape gating). Independent of slices 03-04. DESIGN ruling U-OOS-6 (IC-4).
> Estimate: ~1.5 days
> Derived from DESIGN slice 5 (holds because A-11 was accepted); no DISCUSS interviews.

## User-visible outcome

After `openlore scrape github acme-tools/anvil`, the candidates are kept on Jeff's machine. With
the network off, `openlore candidates list --subject github:acme-tools/anvil` shows anvil's
dependency-pinning candidate and nothing else. With nothing imported, the command says there are
no imported candidates and succeeds. Importing never signs, publishes or changes a weighted view.

## IN scope

- v8 `imported_candidates` (insert-only, never federated, never fed to inference or scoring).
- The new `openlore candidates list [--subject]` verb, offline.
- Re-importing the same repo lists each candidate once (IC-4, pending U-OOS-6).

## OUT of scope

- Sending candidates to owners (OOS-D12 option C). Deleting imported candidates.

## Acceptance (scenarios)

IC-1..IC-4.

## Learning hypothesis

Whether "import into the database" (the user's words) is met by a local, read-only list. If Jeff
never runs `candidates list` after importing, the printed candidates of OOS-D12 option A were
enough and this table can be dropped.
