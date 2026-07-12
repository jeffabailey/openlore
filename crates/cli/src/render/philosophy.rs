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
/// line, then the human name + one-line description, then — when the seed carries
/// any — an `aliases:` line listing the shorthand strings `philosophy show`
/// resolves (slice-30), and a `seeAlso:` line listing its reference links
/// (slice-33). Each optional line renders only when its field is non-empty (no
/// empty label). Both render as bare, comma-separated strings (mirroring
/// `render_record`), never NSID-prefixed object ids, so the greppable object-id
/// surface is unchanged.
fn render_seed_block(seed: &Philosophy) -> String {
    let id = object_id(&seed.name);
    let mut block = format!("{id}\n  {} — {}", seed.name, seed.description);
    if !seed.aliases.is_empty() {
        block.push_str(&format!("\n  aliases: {}", seed.aliases.join(", ")));
    }
    if !seed.see_also.is_empty() {
        block.push_str(&format!("\n  seeAlso: {}", seed.see_also.join(", ")));
    }
    block
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

/// Render the compose preview for `philosophy add` (slice-24; US-PV-003 /
/// AC-003.1/.2). Pure + total.
///
/// Names WHAT would be minted — the derived object id, the human name, the
/// full description, aliases, seeAlso, and the minting author + compose
/// timestamp — so the local-first user reviews the record BEFORE the sign
/// prompt (nothing is signed or written until they confirm). The framing line
/// mirrors `claim add`'s "asserted by you, not as truth" posture: a minted
/// philosophy is proposed by the author, not decreed as canon. Format-tolerant:
/// values render verbatim; layout is not a contract.
pub fn render_compose_preview(record: &Philosophy, author_did: &str, composed_at: &str) -> String {
    let id = object_id(&record.name);
    let aliases = if record.aliases.is_empty() {
        "(none)".to_string()
    } else {
        record.aliases.join(", ")
    };
    let see_also = if record.see_also.is_empty() {
        "(none)".to_string()
    } else {
        record.see_also.join(", ")
    };
    format!(
        "Compose preview (philosophy is minted by you, not decreed as canon)\n\
         \x20 object:      {id}\n\
         \x20 name:        {name}\n\
         \x20 description: {description}\n\
         \x20 aliases:     {aliases}\n\
         \x20 seeAlso:     {see_also}\n\
         \x20 author:      {author_did}\n\
         \x20 composedAt:  {composed_at}\n",
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

    /// slice-31 (alias discoverability): each seed's list block surfaces the alias
    /// strings that `philosophy show` now resolves (slice-30), under an `aliases:`
    /// label — so a user browsing the vocabulary sees which shorthand strings map
    /// onto each philosophy. Pinned over the WHOLE embedded set: every seed's
    /// exact `aliases: <joined>` line renders (a seed with no aliases renders no
    /// such line — no empty label).
    #[test]
    fn every_seed_renders_its_aliases_in_the_list() {
        let seeds = seeds();
        let rendered = render_philosophy_list(&seeds);

        assert!(
            rendered.contains("aliases:"),
            "the list must label the alias strings it surfaces (alias discoverability);\n\
             --- rendered ---\n{rendered}"
        );
        for seed in &seeds {
            if seed.aliases.is_empty() {
                continue;
            }
            let line = format!("aliases: {}", seed.aliases.join(", "));
            assert!(
                rendered.contains(&line),
                "list must surface the aliases for {} as {line:?} (the shorthand \
                 `philosophy show` resolves);\n--- rendered ---\n{rendered}",
                seed.name
            );
        }
    }

    /// slice-33 (reference discoverability): each seed's list block surfaces its
    /// `seeAlso` reference links — until now only `philosophy show` (render_record)
    /// showed them — under a `seeAlso:` label, so a user browsing the vocabulary
    /// sees where to read more without opening each record. Pinned over the WHOLE
    /// embedded set: every seed's exact `seeAlso: <joined>` line renders.
    #[test]
    fn every_seed_renders_its_see_also_in_the_list() {
        let seeds = seeds();
        let rendered = render_philosophy_list(&seeds);

        assert!(
            rendered.contains("seeAlso:"),
            "the list must label the seeAlso references it surfaces (reference discoverability);\n\
             --- rendered ---\n{rendered}"
        );
        for seed in &seeds {
            if seed.see_also.is_empty() {
                continue;
            }
            let line = format!("seeAlso: {}", seed.see_also.join(", "));
            assert!(
                rendered.contains(&line),
                "list must surface the seeAlso links for {} as {line:?};\n--- rendered ---\n{rendered}",
                seed.name
            );
        }
    }

    /// The no-seeAlso branch pinned against a CONSTRUCTED record with no seeAlso
    /// (all embedded seeds currently carry one, so this guards the empty-label
    /// guarantee directly): a seed with no seeAlso renders its object id + name +
    /// description but NO `seeAlso:` label.
    #[test]
    fn a_seed_with_no_see_also_renders_no_see_also_label() {
        let bare = Philosophy {
            name: "no-seealso-example".to_string(),
            description: "A philosophy that carries no seeAlso links.".to_string(),
            aliases: Vec::new(),
            see_also: Vec::new(),
        };
        let block = render_seed_block(&bare);
        assert!(
            block.contains("no-seealso-example") && !block.contains("seeAlso:"),
            "a seed with no seeAlso must render its name but NO empty `seeAlso:` label;\n\
             --- block ---\n{block}"
        );
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

    /// The compose preview (slice-24) names the record being minted — its
    /// derived object id, human name, full description, every alias, the author
    /// DID, and the compose timestamp — so PA-2's "preview shown before the
    /// sign prompt" beat holds. Pinned over a novel (non-seed) record so the
    /// preview is exercised on genuinely new mint content.
    #[test]
    fn compose_preview_names_the_record_being_minted() {
        let record = Philosophy {
            name: "capability-security".to_string(),
            description: "Grant each component only the minimum authority it needs.".to_string(),
            aliases: vec!["ocap".to_string(), "cap-sec".to_string()],
            see_also: vec!["https://en.wikipedia.org/wiki/Capability-based_security".to_string()],
        };
        let preview =
            render_compose_preview(&record, "did:plc:test-jeff", "2026-07-08T12:00:00+00:00");

        assert!(
            preview.contains(&object_id(&record.name)),
            "must name the object id"
        );
        assert!(
            preview.contains("capability-security"),
            "must name the philosophy"
        );
        assert!(
            preview.contains(&record.description),
            "must show the full description verbatim"
        );
        for alias in &record.aliases {
            assert!(preview.contains(alias.as_str()), "must show alias {alias}");
        }
        assert!(
            preview.contains("did:plc:test-jeff"),
            "must name the author DID"
        );
        assert!(
            preview.contains("2026-07-08T12:00:00+00:00"),
            "must show composedAt"
        );
    }
}
