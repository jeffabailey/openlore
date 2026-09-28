//! Person-inference render (contributor-philosophy-inference US-CPI-002/003;
//! Q-CPI-D1 load-bearing substrings pinned by DISTILL). Pure: values in,
//! strings out.

use scraper_domain::{confidence_arithmetic, AlreadySigned, InferenceReport, PersonCandidate};

/// The whole `infer people` output: the candidate list (or the empty-result
/// line), the pairs I already signed (unnumbered, with my claim's CID —
/// DDD-8), followed, unindented so it never joins a candidate's block, by the
/// linked repos that fed nothing because no signed philosophy claim is about
/// them (D-2 / KPI-CPI-3 — unsigned scraper candidates are never support).
pub fn render_inference_report(report: &InferenceReport) -> String {
    let mut out = if report.candidates.is_empty() && !report.already_signed.is_empty() {
        "No new inferred candidates (every inference here is already signed).\n".to_string()
    } else {
        render_person_candidates(&report.candidates)
    };
    out.extend(report.already_signed.iter().map(render_already_signed));
    if !report.repos_without_signed_claims.is_empty() {
        out.push_str(&format!(
            "Not used (no signed philosophy claims): {}\n",
            report.repos_without_signed_claims.join(", ")
        ));
    }
    out
}

/// The numbered inferred-candidate list, or the empty-result line. Each
/// candidate's block: a `[n] NEW <person> adheres to <philosophy>` headline
/// (NEW: no standing adherence of mine for the pair — DDD-8), one
/// provenance line per supporting claim (repo, rank, claim CID, that claim's
/// OWN author — D-7), and the speculative confidence with its arithmetic
/// (J-002c).
pub fn render_person_candidates(candidates: &[PersonCandidate]) -> String {
    if candidates.is_empty() {
        return "No inferred candidates (no signed repo claims about recorded contributors).\n"
            .to_string();
    }
    let mut out = String::from(
        "Inferred person candidates (speculative — derived from signed repo claims, not as truth):\n",
    );
    for (index, candidate) in candidates.iter().enumerate() {
        out.push_str(&render_person_candidate(index + 1, candidate));
    }
    out
}

fn render_person_candidate(number: usize, candidate: &PersonCandidate) -> String {
    let mut block = format!(
        "  [{number}] NEW {} adheres to {}\n",
        candidate.person_subject(),
        philosophy_short_name(candidate.philosophy())
    );
    for repo in candidate.support() {
        for claim in &repo.claims {
            block.push_str(&format!(
                "        {} (#{}) — claim {} by {} at {}\n",
                repo.repo_subject,
                repo.rank,
                claim.cited.cid,
                claim.cited.author_did,
                claim.confidence
            ));
        }
    }
    block.push_str(&format!(
        "        confidence {} (speculative) = {}\n",
        candidate.confidence(),
        confidence_arithmetic(
            candidate.support().len(),
            candidate.max_supporting_confidence()
        )
    ));
    block
}

/// An inferred pair I already signed: never numbered, cited by my claim CID.
fn render_already_signed(already: &AlreadySigned) -> String {
    format!(
        "  - {} adheres to {} — already signed (claim {})\n",
        already.candidate.person_subject(),
        philosophy_short_name(already.candidate.philosophy()),
        already.cid
    )
}

/// The DISPLAY-ONLY `derived-from` summary for a person candidate's compose
/// preview (ADR-064 §6; UC-7): the count of supporting claims, then each one
/// by its AT-URI under its OWN author (D-7). The provenance itself travels in
/// `evidence[]`.
pub fn render_person_derived_from(candidate: &PersonCandidate) -> String {
    let cited: Vec<String> = candidate
        .support()
        .iter()
        .flat_map(|repo| repo.claims.iter().map(|claim| claim.cited.at_uri()))
        .collect();
    let repos = candidate.support().len();
    let summary = format!(
        "  derived-from: {} signed repo claim{} across {repos} repo{}\n",
        cited.len(),
        if cited.len() == 1 { "" } else { "s" },
        if repos == 1 { "" } else { "s" }
    );
    cited
        .iter()
        .fold(summary, |block, uri| block + "    " + uri + "\n")
}

/// `org.openlore.philosophy.memory-safety` → `memory-safety`.
fn philosophy_short_name(philosophy: &str) -> &str {
    philosophy.rsplit('.').next().unwrap_or(philosophy)
}
