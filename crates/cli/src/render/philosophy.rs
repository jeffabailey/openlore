//! `render::philosophy` — pure text renderer for `philosophy list` (slice-22).
//!
//! Turns the embedded `lexicon::philosophy` seed vocabulary into a greppable,
//! human-readable text block (ADR-059; US-PV-001 / AC-001.1). Pure: no I/O, no
//! clock, no store — the verb reads the seeds, this renders them, so it is
//! unit-testable without a subprocess.
//!
//! ## Block format (one per seed)
//!
//! ```text
//! org.openlore.philosophy.memory-safety
//!   memory-safety — Programs cannot corrupt memory: ...
//! ```
//!
//! Blocks are separated by a blank line so downstream `awk`/`grep`/`cut`
//! tooling can split on `\n\n`. The FIRST line of each block is the derived,
//! greppable object id `org.openlore.philosophy.<normalize(name)>` — the EXACT
//! join key a claim's `object` must equal (ADR-059 D1); the object id is
//! DERIVED via `philosophy::object_id`, never stored on the record. The second
//! line carries the human `name` and its one-line `description`.
//!
//! The rendered output is deliberately NOT a JSON array — text is the DEFAULT
//! view (AC-001.3 / P-001); the machine-readable `--json` emission is opt-in and
//! lands in a sibling step.

use lexicon::philosophy::{object_id, Philosophy};

/// Render the philosophy seed vocabulary as the human-readable text view.
///
/// Each seed becomes a two-line greppable block (derived object id, then name +
/// one-line description). Pure + total over the seed slice.
pub fn render_philosophy_list(seeds: &[Philosophy]) -> String {
    seeds
        .iter()
        .map(render_seed_block)
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Render one seed as its greppable block: the derived object id on its own
/// line, then the human name + one-line description indented beneath it.
fn render_seed_block(seed: &Philosophy) -> String {
    let id = object_id(&seed.name);
    format!("{id}\n  {} — {}", seed.name, seed.description)
}

/// Render ONE philosophy record in full for `philosophy show` (slice-23;
/// US-PV-002 / AC-002.1). Pure + total.
///
/// Emits the derived object id, the human name, the FULL description verbatim,
/// then the `aliases:` and `seeAlso:` lines (each value printed plainly so the
/// user can copy the exact alias strings that triangulate onto the record and
/// its see-also links). Format-tolerant: values render verbatim; layout is not
/// a contract.
pub fn render_record(record: &Philosophy) -> String {
    let id = object_id(&record.name);
    let aliases = record.aliases.join(", ");
    let see_also = record.see_also.join(", ");
    format!(
        "{id}\n  {name}\n\n  {description}\n\n  aliases: {aliases}\n  seeAlso: {see_also}",
        name = record.name,
        description = record.description,
    )
}

#[cfg(test)]
mod tests {
    //! Port-to-port unit test at the pure-renderer scope: the driving port is
    //! `render_philosophy_list`'s signature; the observable outcome is the
    //! returned text. Enters through the real embedded `seeds()` so the test
    //! pins the WHOLE vocabulary, not a hand-built fixture.

    use super::*;
    use lexicon::philosophy::{object_id, seeds};

    /// Every seed's derived object id, human name, and description appear in the
    /// rendered text — so no seed is dropped and no block is id-only. This is the
    /// invariant the acceptance suite (PV-1/PV-2/PV-3) rests on, pinned fast at
    /// the pure layer where it costs sub-millisecond to check every seed.
    #[test]
    fn every_seed_renders_its_object_id_name_and_description() {
        let seeds = seeds();
        let rendered = render_philosophy_list(&seeds);

        for seed in &seeds {
            let id = object_id(&seed.name);
            assert!(
                rendered.contains(&id),
                "rendered text must carry the derived object id {id};\n--- rendered ---\n{rendered}"
            );
            assert!(
                rendered.contains(&seed.name),
                "rendered text must carry the seed name {};\n--- rendered ---\n{rendered}",
                seed.name
            );
            assert!(
                rendered.contains(&seed.description),
                "rendered text must carry the seed description for {};\n--- rendered ---\n{rendered}",
                seed.name
            );
        }
    }

    /// The DEFAULT text view is NOT a JSON array (AC-001.3 — JSON is opt-in). A
    /// non-empty vocabulary renders prose that does not parse as a bare array.
    #[test]
    fn rendered_text_is_not_a_json_array() {
        let rendered = render_philosophy_list(&seeds());
        let parsed = serde_json::from_str::<serde_json::Value>(&rendered);
        assert!(
            !matches!(parsed, Ok(serde_json::Value::Array(_))),
            "the text view must not be a JSON array (JSON is strictly opt-in);\n\
             --- rendered ---\n{rendered}"
        );
    }
}
