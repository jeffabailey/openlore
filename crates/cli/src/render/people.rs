//! Person-inference render (contributor-philosophy-inference US-CPI-002/003;
//! Q-CPI-D1 load-bearing substrings pinned by DISTILL). Pure: values in,
//! strings out.

use ports::ContributionLink;
use scraper_domain::{
    confidence_arithmetic, AlreadySigned, CandidateStatus, InferenceReport, NumberedCandidate,
    PersonCandidate, WeakenedClaim, WeakenedSupport,
};

/// The whole `infer people` output: the candidate list (or the empty-result
/// line), the pairs I already signed (unnumbered, with my claim's CID —
/// DDD-8), followed, unindented so it never joins a candidate's block, by the
/// linked repos that fed nothing because no signed philosophy claim is about
/// them (D-2 / KPI-CPI-3 — unsigned scraper candidates are never support).
/// A signed inferred claim whose support weakened carries its SUPPORT
/// WEAKENED line right under the line that shows it (DDD-8 / UC-5); one no
/// line shows is listed on its own. Nothing is ever acted on (D-5).
pub fn render_inference_report(report: &InferenceReport) -> String {
    let mut out = if report.candidates.is_empty() && !report.already_signed.is_empty() {
        "No new inferred candidates (every inference here is already signed).\n".to_string()
    } else {
        render_person_candidates(&report.candidates, &report.weakened)
    };
    for already in &report.already_signed {
        out.push_str(&render_already_signed(already));
        out.push_str(&weakened_line_for(&already.cid, &report.weakened));
    }
    for weakened in report
        .weakened
        .iter()
        .filter(|weakened| !is_shown_elsewhere(&weakened.cid, report))
    {
        out.push_str(&format!(
            "  - {} adheres to {} — signed (claim {})\n",
            weakened.person_subject,
            philosophy_short_name(&weakened.philosophy),
            weakened.cid
        ));
        out.push_str(&render_support_weakened(weakened));
    }
    if !report.repos_without_signed_claims.is_empty() {
        out.push_str(&format!(
            "Not used (no signed philosophy claims): {}\n",
            report.repos_without_signed_claims.join(", ")
        ));
    }
    out
}

/// The numbered inferred-candidate list, or the empty-result line. Each
/// candidate's block: a `[n] NEW|STRONGER <person> adheres to <philosophy>`
/// headline (DDD-8), for a STRONGER one the earlier claim it supersedes and
/// the reassurance that claim stays unchanged (D-5), one provenance line per
/// supporting claim (repo, rank, claim CID, that claim's OWN author — D-7),
/// and the speculative confidence with its arithmetic (J-002c).
/// A STRONGER block flags the superseded claim's weakened support right
/// under the line naming it (UC-5).
pub fn render_person_candidates(
    candidates: &[NumberedCandidate],
    weakened: &[WeakenedClaim],
) -> String {
    if candidates.is_empty() {
        return "No inferred candidates (no signed repo claims about recorded contributors).\n"
            .to_string();
    }
    let mut out = String::from(
        "Inferred person candidates (speculative — derived from signed repo claims, not as truth):\n",
    );
    for (index, candidate) in candidates.iter().enumerate() {
        out.push_str(&render_person_candidate(index + 1, candidate, weakened));
    }
    out
}

fn render_person_candidate(
    number: usize,
    numbered: &NumberedCandidate,
    weakened: &[WeakenedClaim],
) -> String {
    let candidate = &numbered.candidate;
    let mut block = format!(
        "  [{number}] {} {} adheres to {}\n",
        status_label(&numbered.status),
        candidate.person_subject(),
        philosophy_short_name(candidate.philosophy())
    );
    if let CandidateStatus::Stronger { supersedes } = &numbered.status {
        let repos = candidate.support().len();
        block.push_str(&format!(
            "        you signed claim {supersedes}; now {repos} repo{} support it — \
             signing SUPERSEDES it with a new claim (your existing claim is unchanged)\n",
            if repos == 1 { "" } else { "s" }
        ));
        block.push_str(&weakened_line_for(supersedes, weakened));
    }
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

fn status_label(status: &CandidateStatus) -> &'static str {
    match status {
        CandidateStatus::New => "NEW",
        CandidateStatus::Stronger { .. } => "STRONGER",
    }
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

/// Is my weakened claim `cid` already shown — as an already-signed line or as
/// the claim a STRONGER candidate supersedes?
fn is_shown_elsewhere(cid: &str, report: &InferenceReport) -> bool {
    report.already_signed.iter().any(|already| already.cid == cid)
        || report.candidates.iter().any(|numbered| {
            matches!(&numbered.status, CandidateStatus::Stronger { supersedes } if supersedes == cid)
        })
}

/// The SUPPORT WEAKENED line for my claim `cid`, or nothing when its support
/// holds.
fn weakened_line_for(cid: &str, weakened: &[WeakenedClaim]) -> String {
    weakened
        .iter()
        .find(|claim| claim.cid == cid)
        .map(render_support_weakened)
        .unwrap_or_default()
}

/// `SUPPORT WEAKENED: <n> of <total> supporting claims <reason>[; …]` — the
/// claim is unchanged; retracting or countering it stays the user's choice.
fn render_support_weakened(claim: &WeakenedClaim) -> String {
    let noun = if claim.cited == 1 { "claim" } else { "claims" };
    let reasons: Vec<String> = claim
        .weakened
        .iter()
        .map(|(reason, count)| {
            format!(
                "{count} of {} supporting {noun} {}",
                claim.cited,
                reason_text(*reason)
            )
        })
        .collect();
    format!(
        "        SUPPORT WEAKENED: {} — your claim is unchanged \
         (`claim retract` / `claim counter` are your choice)\n",
        reasons.join("; ")
    )
}

fn reason_text(reason: WeakenedSupport) -> &'static str {
    match reason {
        WeakenedSupport::Retracted => "retracted",
        WeakenedSupport::NoLongerEligible => "from a peer no longer subscribed",
        WeakenedSupport::MissingLocally => "not in local store",
    }
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

/// The `scrape github <user>` PERSON view (US-CPI-005 / D-4): the person's
/// recorded links (repo + rank), then the person-scoped inference report —
/// my signed adherence with its CID, and the numbered candidates exactly as
/// `infer people --person` numbers them (DDD-13). An unlinked person gets
/// guidance, never an error. Other logins recorded with the same GitHub user
/// id are flagged `possible rename` under the links — flagged only, never
/// merged (Q-CPI-D7 / OD-CPI-1). Pure: values in, string out.
pub fn render_person_view(
    person_subject: &str,
    links: &[ContributionLink],
    possible_renames: &[String],
    report: &InferenceReport,
) -> String {
    if links.is_empty() {
        return render_unlinked_person(person_subject);
    }
    let mut out = render_person_links(person_subject, links);
    out.push_str(&render_possible_renames(possible_renames));
    out.push_str(&render_inference_report(report));
    if has_no_inference(report) {
        out.push_str(HOW_TO_ENABLE_INFERENCE);
    }
    out
}

/// `github:<login> is not linked to any repo you've scraped` + how to link.
fn render_unlinked_person(person_subject: &str) -> String {
    format!(
        "{person_subject} is not linked to any repo you've scraped.\n\
         Scrape a repo they contribute to first: `openlore scrape github <owner>/<repo>` \
         records its top contributors, then read {person_subject} again.\n"
    )
}

/// The person's links, one repo per line with their rank there, by repo.
fn render_person_links(person_subject: &str, links: &[ContributionLink]) -> String {
    let mut ordered: Vec<&ContributionLink> = links.iter().collect();
    ordered.sort_by(|a, b| a.repo_subject.cmp(&b.repo_subject));
    let header = format!(
        "{person_subject} is linked to {} scraped repo{}:\n",
        ordered.len(),
        if ordered.len() == 1 { "" } else { "s" }
    );
    ordered.iter().fold(header, |out, link| {
        out + &format!("  {} (#{})\n", link.repo_subject, link.rank)
    })
}

/// One `possible rename` line per other login sharing the person's GitHub
/// user id; empty when there is none.
fn render_possible_renames(possible_renames: &[String]) -> String {
    possible_renames
        .iter()
        .map(|other| {
            format!(
                "  possible rename: {other} has the same GitHub user id \
                 (kept separate, never merged)\n"
            )
        })
        .collect()
}

/// Nothing inferred, signed or flagged for this person.
fn has_no_inference(report: &InferenceReport) -> bool {
    report.candidates.is_empty() && report.already_signed.is_empty() && report.weakened.is_empty()
}

/// How to get inferred candidates for a linked person (US-CPI-005 SP-7).
const HOW_TO_ENABLE_INFERENCE: &str = "To enable inference: sign philosophy claims about \
these repos (`openlore scrape github <owner>/<repo> --sign N`); signed repo claims are \
what person candidates are inferred from.\n";

#[cfg(test)]
mod person_view_tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use proptest::prelude::*;

    fn empty_report() -> InferenceReport {
        InferenceReport {
            candidates: Vec::new(),
            already_signed: Vec::new(),
            repos_without_signed_claims: Vec::new(),
            weakened: Vec::new(),
        }
    }

    fn link(person: &str, repo: &str, rank: u32) -> ContributionLink {
        let observed = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        ContributionLink {
            repo_subject: format!("github:{repo}"),
            person_subject: person.to_string(),
            github_user_id: 1,
            rank,
            contributions: 10,
            first_observed_at: observed,
            last_observed_at: observed,
        }
    }

    proptest! {
        /// Every recorded link shows on its own line with the person's rank,
        /// under a header counting the linked repos; no line is numbered like
        /// a candidate (so `--sign` numbering stays the report's).
        #[test]
        fn a_linked_person_view_lists_every_repo_with_its_rank(
            ranked in prop::collection::btree_map("[a-z]{1,8}/[a-z]{1,8}", 1u32..=30, 1..6)
        ) {
            let person = "github:someone";
            let links: Vec<ContributionLink> =
                ranked.iter().map(|(repo, rank)| link(person, repo, *rank)).collect();
            let view = render_person_view(person, &links, &[], &empty_report());
            let plural = if links.len() == 1 { "" } else { "s" };
            let header = format!("{person} is linked to {} scraped repo{plural}", links.len());
            prop_assert!(view.contains(&header), "{view}");
            for (repo, rank) in &ranked {
                let shown = view
                    .lines()
                    .any(|line| line.contains(&format!("github:{repo} (#{rank})")));
                prop_assert!(shown, "{repo} #{rank}\n{view}");
            }
            prop_assert!(!view.contains("[1]"), "{view}");
            prop_assert!(!view.contains("not linked to any repo"), "{view}");
        }

        /// A person with no recorded link gets guidance naming them and how
        /// to link them — never a candidate list.
        #[test]
        fn an_unlinked_person_view_is_guidance(login in "[a-zA-Z][a-zA-Z0-9-]{0,20}") {
            let person = format!("github:{login}");
            let view = render_person_view(&person, &[], &[], &empty_report());
            let guidance = format!("{person} is not linked to any repo you've scraped");
            prop_assert!(view.contains(&guidance), "{view}");
            prop_assert!(view.contains("openlore scrape github <owner>/<repo>"), "{view}");
            prop_assert!(!view.contains("[1]"), "{view}");
        }
    }
}
