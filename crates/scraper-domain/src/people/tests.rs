//! Properties of the pure person-inference area (all `people` submodules,
//! exercised through the `people` surface).

use super::*;
use crate::proptest_strategies::arb_raw_contributors;
use claim_domain::{bare_did, ClaimReference, ReferenceType};
use ports::{AuthorRelationship, ContributionLink, RankedContributor, RawContributor};
use proptest::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

/// Oracle: user ids with NO bot-rule row (the distinct humans).
fn distinct_human_ids(rows: &[RawContributor]) -> BTreeSet<u64> {
    let bot_ids: BTreeSet<u64> = rows
        .iter()
        .filter(|r| is_bot(r))
        .map(|r| r.user_id)
        .collect();
    rows.iter()
        .map(|r| r.user_id)
        .filter(|id| !bot_ids.contains(id))
        .collect()
}

proptest! {
    /// On an `owner/repo` target the count is the override when it fits
    /// one page, a refusal naming the request above it, the default when
    /// absent.
    #[test]
    fn a_repo_target_accepts_counts_up_to_one_page(
        owner in "[a-z]{1,8}",
        repo in "[a-z]{1,8}",
        requested in proptest::option::of(0usize..=250),
    ) {
        let target = format!("{owner}/{repo}");
        let expected = match requested {
            None => Ok(DEFAULT_CONTRIBUTOR_COUNT),
            Some(n) if n <= MAX_CONTRIBUTOR_COUNT => Ok(n),
            Some(n) => Err(ContributorCountError::AboveOnePage { requested: n }),
        };
        prop_assert_eq!(contributor_count_for(&target, requested), expected);
    }

    /// On a person target any override is refused (whatever N), and no
    /// override is fine.
    #[test]
    fn a_person_target_refuses_any_contributor_override(
        user in "[A-Za-z][A-Za-z0-9-]{0,15}",
        requested in proptest::option::of(any::<usize>()),
    ) {
        let outcome = contributor_count_for(&user, requested);
        match requested {
            None => prop_assert_eq!(outcome, Ok(DEFAULT_CONTRIBUTOR_COUNT)),
            Some(_) => prop_assert_eq!(
                outcome,
                Err(ContributorCountError::PersonTarget { target: user.clone() })
            ),
        }
    }
}

proptest! {
    /// Top-N cut + de-dup: exactly min(N, distinct humans) people, each
    /// user id at most once.
    #[test]
    fn selection_records_min_of_n_and_distinct_humans_each_once(
        rows in arb_raw_contributors(),
        top_n in 0usize..=40,
    ) {
        let selection = select_contributors(&rows, top_n);
        let humans = distinct_human_ids(&rows);
        prop_assert_eq!(selection.people.len(), top_n.min(humans.len()));
        let ids: BTreeSet<u64> = selection.people.iter().map(|p| p.github_user_id).collect();
        prop_assert_eq!(ids.len(), selection.people.len());
        prop_assert!(ids.is_subset(&humans));
    }

    /// Bot rule: no recorded person matches it; every named bot does, and
    /// bots are named only while humans are still being collected.
    #[test]
    fn no_bot_is_ever_recorded_and_only_bots_are_named_excluded(
        rows in arb_raw_contributors(),
        top_n in 0usize..=40,
    ) {
        let selection = select_contributors(&rows, top_n);
        let humans = distinct_human_ids(&rows);
        prop_assert!(selection.people.iter().all(|p| !p.login.to_ascii_lowercase().ends_with("[bot]")));
        for bot in &selection.bots_excluded {
            prop_assert!(rows.iter().any(|r| &r.login == bot && !humans.contains(&r.user_id)));
        }
        if top_n == 0 {
            prop_assert!(selection.bots_excluded.is_empty());
        }
    }

    /// Re-rank: ranks are contiguous 1..=k and contributions never rise
    /// as rank falls.
    #[test]
    fn ranks_are_contiguous_and_ordered_by_contributions(
        rows in arb_raw_contributors(),
        top_n in 0usize..=40,
    ) {
        let selection = select_contributors(&rows, top_n);
        let ranks: Vec<u32> = selection.people.iter().map(|p| p.rank).collect();
        let expected: Vec<u32> = (1..=selection.people.len() as u32).collect();
        prop_assert_eq!(ranks, expected);
        prop_assert!(selection.people.windows(2).all(|w| w[0].contributions >= w[1].contributions));
    }

    /// API order is not trusted: any permutation of the rows selects the
    /// identical people and bots.
    #[test]
    fn selection_is_invariant_under_api_row_order(
        (rows, shuffled) in arb_raw_contributors()
            .prop_flat_map(|rows| (Just(rows.clone()), Just(rows).prop_shuffle())),
        top_n in 0usize..=40,
    ) {
        prop_assert_eq!(select_contributors(&rows, top_n), select_contributors(&shuffled, top_n));
    }
}
// ---------------------------------------------------------------------
// Inference properties (DDD-7 / DDD-9 / DDD-10; ADR-064)
// ---------------------------------------------------------------------

use crate::proptest_strategies::{
    arb_hundredths, arb_inference_inputs, arb_person_candidate, arb_supporting_repo,
};

/// DDD-7 oracle, straight from the rule's text over the whole claim set:
/// an `embodiesPhilosophy` claim by me or an ACTIVE peer that is not a
/// retracts/counters marker, and that its OWN author has neither retracted
/// nor superseded.
fn supports(claim: &RepoClaim, all: &[RepoClaim]) -> bool {
    let withdrawn_by_author = |ref_type: ReferenceType| {
        all.iter().any(|other| {
            other.author_did == claim.author_did
                && other
                    .references
                    .iter()
                    .any(|r| r.ref_type == ref_type && r.cid.0 == claim.cid)
        })
    };
    claim.predicate == crate::EMBODIES_PHILOSOPHY
        && matches!(
            claim.relationship,
            AuthorRelationship::You | AuthorRelationship::SubscribedPeer
        )
        && !claim.references.iter().any(|r| {
            matches!(
                r.ref_type,
                ReferenceType::Retracts | ReferenceType::Counters
            )
        })
        && !withdrawn_by_author(ReferenceType::Retracts)
        && !withdrawn_by_author(ReferenceType::Supersedes)
}

proptest! {
    /// DDD-9: confidence = min(29, 15 + 5(k−1), max) — never above the
    /// speculative cap nor the strongest support, monotone in k.
    #[test]
    fn inferred_confidence_is_capped_bounded_by_support_and_monotone_in_breadth(
        k in 1usize..=40,
        more in 0usize..=10,
        max in arb_hundredths(),
    ) {
        let confidence = inferred_confidence(k, max);
        let expected = 29u32.min(15 + 5 * (k as u32 - 1)).min(max.value());
        prop_assert_eq!(confidence.value(), expected);
        prop_assert!(confidence.value() <= 29 && confidence <= max);
        prop_assert!(inferred_confidence(k + more, max) >= confidence);
    }

    /// D-9: a candidate without provenance is unrepresentable — no repos,
    /// or any repo citing no claim, is rejected; otherwise it builds with
    /// the DDD-9 confidence over its supporting repos.
    #[test]
    fn person_candidate_requires_provenance_in_every_supporting_repo(
        repos in proptest::collection::vec(arb_supporting_repo(), 0..4),
        hollow_at in proptest::option::of(0usize..4),
    ) {
        let mut repos = repos;
        let mut seen = BTreeSet::new();
        repos.retain(|r| seen.insert(r.repo_subject.to_ascii_lowercase()));
        if let Some(i) = hollow_at.filter(|i| *i < repos.len()) {
            repos[i].claims.clear();
        }
        let hollow = repos.is_empty() || repos.iter().any(|r| r.claims.is_empty());
        let built = PersonCandidate::new("github:BurntSushi", "org.openlore.philosophy.x", repos.clone());
        if hollow {
            prop_assert_eq!(built, Err(CandidateError::EmptyProvenance));
        } else {
            let candidate = built.expect("provenance present");
            prop_assert_eq!(candidate.support().len(), repos.len());
            prop_assert_eq!(
                candidate.confidence(),
                inferred_confidence(repos.len(), candidate.max_supporting_confidence())
            );
        }
    }

    /// ADR-064 §3/§5 codec: parsing the encoded evidence yields exactly the
    /// cited claims in canonical order (bare authors); each repo closes
    /// with its person-specific commits URL; no entry carries a comma.
    #[test]
    fn provenance_round_trips_through_evidence(candidate in arb_person_candidate()) {
        let evidence = encode_provenance(&candidate);
        let cited: Vec<CitedClaim> = candidate
            .support()
            .iter()
            .flat_map(|repo| repo.claims.iter().map(|c| c.cited.clone()))
            .collect();
        prop_assert_eq!(parse_provenance(&evidence), cited.clone());
        prop_assert_eq!(evidence.len(), cited.len() + candidate.support().len());
        prop_assert!(cited.iter().all(|c| !c.author_did.contains('#')));
        prop_assert!(evidence.iter().all(|e| !e.contains(',')));
        let repo_folded: Vec<String> = candidate.support().iter().map(|r| r.repo_subject.to_ascii_lowercase()).collect();
        prop_assert!(repo_folded.windows(2).all(|w| w[0] <= w[1]));
        let commits: Vec<&String> = evidence.iter().filter(|e| e.starts_with("https://github.com/")).collect();
        prop_assert_eq!(commits.len(), candidate.support().len());
        let suffix = format!("/commits?author={}", candidate.login());
        prop_assert!(commits.iter().all(|url| url.ends_with(&suffix)));
    }

    /// Numbering is deterministic: any permutation of links and claims
    /// yields the identical numbered candidate list.
    #[test]
    fn numbering_is_invariant_under_input_order(
        ((links, claims), (shuffled_links, shuffled_claims)) in arb_inference_inputs()
            .prop_flat_map(|(links, claims)| (
                Just((links.clone(), claims.clone())),
                (Just(links).prop_shuffle(), Just(claims).prop_shuffle()),
            )),
    ) {
        prop_assert_eq!(
            infer_person_candidates(&links, &claims),
            infer_person_candidates(&shuffled_links, &shuffled_claims)
        );
    }

    /// DDD-7 eligibility: every cited claim is an `embodiesPhilosophy`
    /// claim by me or an ACTIVE peer on a repo linked to the person (any
    /// letter case), is no retracts/counters marker, and is neither
    /// retracted nor superseded by its own author; every such claim is
    /// cited by the matching candidate. A third party's counter or
    /// retract never hides it.
    #[test]
    fn candidates_cite_exactly_the_eligible_linked_claims(
        (links, claims) in arb_inference_inputs(),
    ) {
        let candidates = infer_person_candidates(&links, &claims);
        let cited: BTreeSet<(String, String, String)> = candidates
            .iter()
            .flat_map(|c| c.support().iter().flat_map(move |repo| repo.claims.iter().map(move |s| (
                c.person_subject().to_ascii_lowercase(),
                c.philosophy().to_string(),
                s.cited.cid.clone(),
            ))))
            .collect();
        let expected: BTreeSet<(String, String, String)> = links
            .iter()
            .flat_map(|link| claims.iter()
                .filter(|claim| supports(claim, &claims) && claim.repo_subject.eq_ignore_ascii_case(&link.repo_subject))
                .map(move |claim| (
                    link.person_subject.to_ascii_lowercase(),
                    claim.philosophy.clone(),
                    claim.cid.clone(),
                )))
            .collect();
        prop_assert_eq!(cited, expected);
    }
}

// --- the inference report (US-CPI-002 AC1-AC4; D-2 / D-7 / KPI-CPI-2) ---

proptest! {
    /// Unscoped, the report's candidates ARE the inferred candidates; the
    /// footer names exactly the linked repos (case-folded, once each) that
    /// carry zero eligible claims — sorted, never one that supports.
    #[test]
    fn report_footer_names_exactly_the_linked_repos_without_signed_claims(
        (links, claims) in arb_inference_inputs(),
    ) {
        let report = infer_people_report(&links, &claims, &[], &InferenceFilter::default());
        prop_assert_eq!(numbered(&report), infer_person_candidates(&links, &claims));
        prop_assert!(report.candidates.iter().all(|n| n.status == CandidateStatus::New));
        let expected: BTreeSet<String> = links
            .iter()
            .map(|l| subject_key(&l.repo_subject))
            .filter(|repo| !claims.iter().any(|c| supports(c, &claims) && subject_key(&c.repo_subject) == *repo))
            .collect();
        let named: Vec<String> = report.repos_without_signed_claims.iter().map(|r| subject_key(r)).collect();
        prop_assert_eq!(named.iter().cloned().collect::<BTreeSet<_>>(), expected);
        prop_assert!(named.windows(2).all(|w| w[0] < w[1]), "sorted, once each");
    }

    /// Scoping to a person keeps exactly that person's candidates (any
    /// letter case) and names only that person's unsupported repos; every
    /// candidate keeps complete provenance and each cited claim keeps its
    /// OWN author (D-7 — a peer's claim is never re-attributed).
    #[test]
    fn a_person_scoped_report_keeps_only_that_persons_attributed_candidates(
        (links, claims) in arb_inference_inputs(),
        person in proptest::sample::select(&crate::proptest_strategies::PERSON_POOL[..]),
    ) {
        let filter = InferenceFilter {
            person: Some(PersonSubject::parse(&person.to_ascii_uppercase().replacen("GITHUB:", "github:", 1)).expect("pool persons are valid")),
            min_repos: 0,
        };
        let scoped = infer_people_report(&links, &claims, &[], &filter);
        let expected: Vec<PersonCandidate> = infer_person_candidates(&links, &claims)
            .into_iter()
            .filter(|c| subject_key(c.person_subject()) == subject_key(person))
            .collect();
        prop_assert_eq!(numbered(&scoped), expected);
        let persons_repos: BTreeSet<String> = links
            .iter()
            .filter(|l| subject_key(&l.person_subject) == subject_key(person))
            .map(|l| subject_key(&l.repo_subject))
            .collect();
        let unsupported: BTreeSet<String> = persons_repos
            .into_iter()
            .filter(|repo| !claims.iter().any(|c| supports(c, &claims) && subject_key(&c.repo_subject) == *repo))
            .collect();
        let named: BTreeSet<String> = scoped.repos_without_signed_claims.iter().map(|r| subject_key(r)).collect();
        prop_assert_eq!(named, unsupported);
        let author_of: BTreeSet<(String, String)> = claims.iter().map(|c| (c.cid.clone(), bare_did(&c.author_did).to_string())).collect();
        for candidate in numbered(&scoped).iter() {
            prop_assert!(!candidate.support().is_empty());
            for repo in candidate.support() {
                prop_assert!(!repo.claims.is_empty());
                for claim in &repo.claims {
                    prop_assert!(author_of.contains(&(claim.cited.cid.clone(), claim.cited.author_did.clone())));
                }
            }
        }
    }
}

proptest! {
    /// UC-6 read side: the subjects to read are every linked spelling
    /// plus every stored spelling of a linked repo (case-folded join),
    /// nothing else; re-deriving from its own output changes nothing.
    #[test]
    fn repo_subjects_to_read_are_every_spelling_of_a_linked_repo(
        (links, claims) in arb_inference_inputs(),
        extra in proptest::collection::vec("github:[a-zA-Z]{1,3}/[a-zA-Z]{1,3}", 0..4),
    ) {
        let stored: Vec<String> = claims.iter().map(|c| c.repo_subject.clone()).chain(extra).collect();
        let to_read = repo_subjects_to_read(&links, stored.iter().map(String::as_str));
        let linked: BTreeSet<String> = links.iter().map(|l| subject_key(&l.repo_subject)).collect();
        let expected: BTreeSet<String> = links
            .iter()
            .map(|l| l.repo_subject.clone())
            .chain(stored.iter().filter(|s| linked.contains(&subject_key(s))).cloned())
            .collect();
        prop_assert_eq!(&to_read, &expected);
        prop_assert_eq!(repo_subjects_to_read(&links, to_read.iter().map(String::as_str)), to_read);
    }
}

// --- cross-repo overlap (US-CPI-001 AC4) -------------------------------

/// The people a repo's links record, as a selection would rank them.
fn people_of(repo: &str, links: &[ContributionLink]) -> Vec<RankedContributor> {
    links
        .iter()
        .filter(|l| subject_key(&l.repo_subject) == subject_key(repo))
        .map(|l| RankedContributor {
            login: l.person_subject.trim_start_matches("github:").to_string(),
            github_user_id: l.github_user_id,
            rank: l.rank,
            contributions: l.contributions,
        })
        .collect()
}

/// Overlap as case-folded `(person, other repo)` keys.
fn overlap_keys(shared: &[SharedContributor]) -> BTreeSet<(String, String)> {
    shared
        .iter()
        .map(|s| {
            (
                subject_key(&format!("github:{}", s.login)),
                subject_key(&format!("github:{}", s.other_repo)),
            )
        })
        .collect()
}

proptest! {
    /// Every recorded person of the scraped repo who is linked to ANOTHER
    /// repo (subjects compared case-folded) is surfaced with that repo —
    /// once — and the scraped repo itself never appears as "other".
    #[test]
    fn overlap_surfaces_exactly_the_other_repos_each_person_is_linked_to(
        links in proptest::collection::vec(crate::proptest_strategies::arb_contribution_link(), 0..10),
        current in proptest::sample::select(vec![
            "github:BurntSushi/ripgrep", "github:rust-lang/regex", "github:dtolnay/serde",
        ]),
    ) {
        let people = people_of(current, &links);
        let person_keys: BTreeSet<String> = people
            .iter()
            .map(|p| subject_key(&p.person_subject()))
            .collect();
        let expected: BTreeSet<(String, String)> = links
            .iter()
            .filter(|l| subject_key(&l.repo_subject) != subject_key(current))
            .filter(|l| person_keys.contains(&subject_key(&l.person_subject)))
            .map(|l| (subject_key(&l.person_subject), subject_key(&l.repo_subject)))
            .collect();

        let shared = shared_contributors(current, &people, &links);

        prop_assert_eq!(overlap_keys(&shared), expected.clone());
        prop_assert_eq!(shared.len(), expected.len(), "one line per (person, repo)");
    }

    /// Overlap is symmetric: P is shown under A → B exactly when P is
    /// shown under B → A.
    #[test]
    fn overlap_is_symmetric_between_two_scraped_repos(
        links in proptest::collection::vec(crate::proptest_strategies::arb_contribution_link(), 0..10),
    ) {
        let (repo_a, repo_b) = ("github:BurntSushi/ripgrep", "github:rust-lang/regex");
        let persons_towards = |from: &str, to: &str| -> BTreeSet<String> {
            overlap_keys(&shared_contributors(from, &people_of(from, &links), &links))
                .into_iter()
                .filter(|(_, other)| *other == subject_key(to))
                .map(|(person, _)| person)
                .collect()
        };
        prop_assert_eq!(persons_towards(repo_a, repo_b), persons_towards(repo_b, repo_a));
    }

    /// Case never matters: re-casing every recorded subject yields the
    /// same case-folded overlap.
    #[test]
    fn overlap_ignores_subject_case(
        links in proptest::collection::vec(crate::proptest_strategies::arb_contribution_link(), 0..10),
    ) {
        let current = "github:BurntSushi/ripgrep";
        let upper: Vec<ContributionLink> = links
            .iter()
            .map(|l| ContributionLink {
                repo_subject: l.repo_subject.to_ascii_uppercase().replacen("GITHUB:", "github:", 1),
                person_subject: l.person_subject.to_ascii_uppercase().replacen("GITHUB:", "github:", 1),
                ..l.clone()
            })
            .collect();
        let people = people_of(current, &links);
        prop_assert_eq!(
            overlap_keys(&shared_contributors(current, &people, &links)),
            overlap_keys(&shared_contributors(current, &people, &upper))
        );
    }
}

// --- possible renames (Q-CPI-D7 / OD-CPI-1) ---

/// Oracle: the case-folded keys of every OTHER person whose links share a
/// GitHub user id with one of `person`'s links.
fn rename_keys_oracle(person: &str, links: &[ContributionLink]) -> BTreeSet<String> {
    let own_ids: BTreeSet<u64> = links
        .iter()
        .filter(|l| l.person_subject.eq_ignore_ascii_case(person))
        .map(|l| l.github_user_id)
        .collect();
    links
        .iter()
        .filter(|l| own_ids.contains(&l.github_user_id))
        .map(|l| l.person_subject.to_ascii_lowercase())
        .filter(|key| *key != person.to_ascii_lowercase())
        .collect()
}

proptest! {
    /// Exactly the other logins sharing a user id are flagged — once
    /// each, never the person itself in any letter case — and the result
    /// is ordered by key.
    #[test]
    fn possible_renames_flags_exactly_the_logins_sharing_a_user_id(
        links in proptest::collection::vec(crate::proptest_strategies::arb_contribution_link(), 0..10),
        person in proptest::sample::select(&crate::proptest_strategies::PERSON_POOL[..]),
    ) {
        let flagged = possible_renames(person, &links);
        let keys: Vec<String> = flagged.iter().map(|s| s.to_ascii_lowercase()).collect();
        let expected: Vec<String> = rename_keys_oracle(person, &links).into_iter().collect();
        prop_assert_eq!(keys, expected);
        for subject in &flagged {
            prop_assert!(links.iter().any(|l| &l.person_subject == subject), "{} was recorded", subject);
        }
    }

    /// A rename is mutual: if A flags B then B flags A.
    #[test]
    fn possible_renames_are_mutual(
        links in proptest::collection::vec(crate::proptest_strategies::arb_contribution_link(), 0..10),
    ) {
        for a in crate::proptest_strategies::PERSON_POOL {
            for b in possible_renames(a, &links) {
                let back: Vec<String> = possible_renames(&b, &links)
                    .iter()
                    .map(|s| s.to_ascii_lowercase())
                    .collect();
                prop_assert!(back.contains(&a.to_ascii_lowercase()), "{} -> {} not mutual", a, b);
            }
        }
    }
}

// --- filters, already-signed pairs and the person form (US-CPI-002
//     AC5/AC6/AC7; DDD-8 / DDD-13 / D-8 / UC-8) ---

/// One of my own claims about a person, drawn over the inference pools:
/// an adherence (or not), possibly my retraction/supersession of another.
fn arb_own_claims() -> impl Strategy<Value = Vec<OwnClaim>> {
    proptest::collection::vec(
        (
            proptest::sample::select(&crate::proptest_strategies::PERSON_POOL[..]),
            prop_oneof![Just(ADHERES_TO_PHILOSOPHY), Just("usesLanguage")],
            proptest::sample::select(vec![
                "org.openlore.philosophy.memory-safety",
                "org.openlore.philosophy.test-driven",
            ]),
            "bafyown[a-z2-7]{6}",
        ),
        0..5,
    )
    .prop_flat_map(|rows| {
        let n = rows.len();
        (
            Just(rows),
            proptest::collection::vec(
                proptest::option::of((
                    prop_oneof![
                        Just(ReferenceType::Retracts),
                        Just(ReferenceType::Supersedes)
                    ],
                    0..n.max(1),
                )),
                n,
            ),
        )
    })
    .prop_map(|(rows, references)| {
        let cids: Vec<String> = rows.iter().map(|r| r.3.clone()).collect();
        rows.into_iter()
            .zip(references)
            .map(
                |((person, predicate, philosophy, cid), reference)| OwnClaim {
                    subject: person.to_string(),
                    predicate: predicate.to_string(),
                    object: philosophy.to_string(),
                    author_did: "did:plc:me".to_string(),
                    // Hand-authored shape: cites no claim AT-URI (UC-4).
                    evidence: vec!["https://example.org/why".to_string()],
                    composed_at: "2026-09-01T09:00:00Z".to_string(),
                    references: reference
                        .filter(|(_, target)| cids[*target] != cid)
                        .map(|(ref_type, target)| ClaimReference {
                            ref_type,
                            cid: claim_domain::Cid(cids[target].clone()),
                        })
                        .into_iter()
                        .collect(),
                    cid,
                },
            )
            .collect()
    })
}

/// Oracle: my standing adherence pairs — an adherence claim that is no
/// retraction marker and that I have neither retracted nor superseded.
fn standing_pairs(own: &[OwnClaim]) -> BTreeSet<(String, String)> {
    let withdrawn = |claim: &OwnClaim| {
        own.iter().any(|other| {
            other.references.iter().any(|r| {
                r.cid.0 == claim.cid
                    && matches!(
                        r.ref_type,
                        ReferenceType::Retracts | ReferenceType::Supersedes
                    )
            })
        })
    };
    own.iter()
        .filter(|c| c.predicate == ADHERES_TO_PHILOSOPHY)
        .filter(|c| {
            !c.references
                .iter()
                .any(|r| r.ref_type == ReferenceType::Retracts)
        })
        .filter(|c| !withdrawn(c))
        .map(|c| (subject_key(&c.subject), c.object.clone()))
        .collect()
}

/// The numbered candidates of a report, without their status.
fn numbered(report: &InferenceReport) -> Vec<PersonCandidate> {
    report
        .candidates
        .iter()
        .map(|n| n.candidate.clone())
        .collect()
}

fn pair_of(candidate: &PersonCandidate) -> (String, String) {
    (
        subject_key(candidate.person_subject()),
        candidate.philosophy().to_string(),
    )
}

proptest! {
    /// UC-8 / DDD-8 / DDD-13: filters and the already-signed exclusion
    /// apply BEFORE numbering — the numbered list is exactly the
    /// unfiltered inference, in its order, kept when in scope, supported
    /// by ≥ min-repos repos and not already signed by me; the in-filter
    /// signed ones are listed apart, each with a CID of mine for that pair.
    #[test]
    fn filters_and_signed_pairs_apply_before_numbering(
        (links, claims) in arb_inference_inputs(),
        own in arb_own_claims(),
        person in proptest::option::of(proptest::sample::select(&crate::proptest_strategies::PERSON_POOL[..])),
        min_repos in 0usize..4,
    ) {
        let filter = InferenceFilter {
            person: person.map(|p| PersonSubject::parse(p).expect("pool persons are valid")),
            min_repos,
        };
        let report = infer_people_report(&links, &claims, &own, &filter);
        let signed = standing_pairs(&own);
        let in_filter: Vec<PersonCandidate> = infer_person_candidates(&links, &claims)
            .into_iter()
            .filter(|c| person.is_none_or(|p| subject_key(c.person_subject()) == subject_key(p)))
            .filter(|c| c.support().len() >= min_repos)
            .collect();
        let (expected_signed, expected_numbered): (Vec<_>, Vec<_>) = in_filter
            .into_iter()
            .partition(|c| signed.contains(&pair_of(c)));

        prop_assert_eq!(numbered(&report), expected_numbered);
        let shown: Vec<&PersonCandidate> = report.already_signed.iter().map(|a| &a.candidate).collect();
        prop_assert_eq!(shown, expected_signed.iter().collect::<Vec<_>>());
        for already in &report.already_signed {
            prop_assert!(own.iter().any(|c| c.cid == already.cid
                && (subject_key(&c.subject), c.object.clone()) == pair_of(&already.candidate)));
        }
    }

    /// D-8: `github:<login>` is the only accepted person form; anything
    /// else is refused with an error naming that form.
    #[test]
    fn a_person_is_accepted_only_as_github_login(
        login in "[A-Za-z0-9][A-Za-z0-9-]{0,20}",
        other in "[A-Za-z0-9:_./ -]{0,24}",
    ) {
        let named = format!("github:{login}");
        prop_assert_eq!(PersonSubject::parse(&named).map(|p| p.as_str().to_string()), Ok(named));
        let bare = PersonSubject::parse(&login);
        prop_assert!(bare.is_err());
        prop_assert!(bare.unwrap_err().to_string().contains("github:<login>"));
        let is_github_login = other
            .strip_prefix("github:")
            .is_some_and(|l| !l.is_empty() && !l.starts_with('-') && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'));
        prop_assert_eq!(PersonSubject::parse(&other).is_ok(), is_github_login);
    }
}

// --- the scrape's before/after change summary (US-CPI-004; DDD-14) ---

fn numbered_pairs(report: &InferenceReport) -> BTreeSet<(String, String)> {
    report
        .candidates
        .iter()
        .map(|n| pair_of(&n.candidate))
        .collect()
}

proptest! {
    /// A scrape that changed no inference input reports nothing new.
    #[test]
    fn an_unchanged_inference_reports_no_new_candidates(
        (links, claims) in arb_inference_inputs(),
        own in arb_own_claims(),
    ) {
        let report = infer_people_report(&links, &claims, &own, &InferenceFilter::default());
        prop_assert_eq!(new_inferred_candidate_count(&report, &report.clone()), 0);
    }

    /// The hint counts exactly the numbered pairs the run did not propose
    /// before — never one it already proposed, never one it dropped.
    #[test]
    fn only_candidates_absent_before_are_counted_as_new(
        (links_before, claims_before) in arb_inference_inputs(),
        (links_added, claims_added) in arb_inference_inputs(),
        own in arb_own_claims(),
    ) {
        let everything = InferenceFilter::default();
        let before = infer_people_report(&links_before, &claims_before, &own, &everything);
        let links_after: Vec<ContributionLink> = links_before.iter().chain(&links_added).cloned().collect();
        let claims_after: Vec<RepoClaim> = claims_before.iter().chain(&claims_added).cloned().collect();
        let after = infer_people_report(&links_after, &claims_after, &own, &everything);
        let expected = numbered_pairs(&after).difference(&numbered_pairs(&before)).count();
        prop_assert_eq!(new_inferred_candidate_count(&before, &after), expected);
    }
}

// --- STRONGER vs already signed (US-CPI-004; DDD-8 / UC-4 / Q-CPI-D4) ---

/// My adherence claim for `candidate`'s pair, citing the supporting claims
/// of the repos `cited_repos` selects (bit i = repo i; 0 cites no claim —
/// a hand-authored shape), composed on September `day`.
fn my_claim_for(candidate: &PersonCandidate, cited_repos: u8, day: u8, cid: String) -> OwnClaim {
    let evidence = candidate
        .support()
        .iter()
        .enumerate()
        .filter(|(i, _)| cited_repos & (1 << (i % 8)) != 0)
        .flat_map(|(_, repo)| repo.claims.iter().map(|c| c.cited.at_uri()))
        .chain(std::iter::once(format!(
            "https://github.com/o/r/commits?author={}",
            candidate.login()
        )))
        .collect();
    OwnClaim {
        subject: candidate.person_subject().to_string(),
        predicate: ADHERES_TO_PHILOSOPHY.to_string(),
        object: candidate.philosophy().to_string(),
        author_did: "did:plc:me".to_string(),
        cid,
        evidence,
        composed_at: format!("2026-09-{day:02}T09:00:00Z"),
        references: Vec::new(),
    }
}

/// My claims drawn over the inferred candidates: `(which candidate, which
/// repos it cites, which day)`, each with a distinct CID.
fn my_claims(candidates: &[PersonCandidate], picks: &[(usize, u8, u8)]) -> Vec<OwnClaim> {
    if candidates.is_empty() {
        return Vec::new();
    }
    picks
        .iter()
        .enumerate()
        .map(|(k, (which, cited_repos, day))| {
            my_claim_for(
                &candidates[which % candidates.len()],
                *cited_repos,
                *day,
                format!("bafymine{k}"),
            )
        })
        .collect()
}

fn cites_a_claim(claim: &OwnClaim) -> bool {
    claim.evidence.iter().any(|e| e.starts_with("at://"))
}

fn mine_for<'a>(own: &'a [OwnClaim], candidate: &PersonCandidate) -> Vec<&'a OwnClaim> {
    own.iter()
        .filter(|c| (subject_key(&c.subject), c.object.clone()) == pair_of(candidate))
        .collect()
}

fn numbered_status<'a>(
    report: &'a InferenceReport,
    candidate: &PersonCandidate,
) -> Option<&'a CandidateStatus> {
    report
        .candidates
        .iter()
        .find(|n| pair_of(&n.candidate) == pair_of(candidate))
        .map(|n| &n.status)
}

proptest! {
    /// UC-4: a pair I signed by hand (a claim citing no claim AT-URI) is
    /// already signed and never STRONGER, however support grows; and no
    /// STRONGER ever supersedes a hand-authored claim.
    #[test]
    fn a_claim_citing_no_at_uri_is_never_superseded(
        (links, claims) in arb_inference_inputs(),
        picks in proptest::collection::vec((0usize..16, any::<u8>(), 1u8..=28), 0..6),
    ) {
        let inferred = infer_person_candidates(&links, &claims);
        let own = my_claims(&inferred, &picks);
        let report = infer_people_report(&links, &claims, &own, &InferenceFilter::default());
        for candidate in &inferred {
            let mine = mine_for(&own, candidate);
            if mine.iter().any(|c| !cites_a_claim(c)) {
                prop_assert_eq!(numbered_status(&report, candidate), None);
                prop_assert!(report.already_signed.iter().any(|a| pair_of(&a.candidate) == pair_of(candidate)));
            }
        }
        for numbered in &report.candidates {
            if let CandidateStatus::Stronger { supersedes } = &numbered.status {
                let superseded = own.iter().find(|c| &c.cid == supersedes);
                prop_assert!(superseded.is_some_and(cites_a_claim), "supersedes a claim citing support");
            }
        }
    }

    /// DDD-8 / Q-CPI-D4: with only inferred claims for a pair, the pair is
    /// STRONGER exactly when a repo supporting it now is cited by none of
    /// the LATEST (composed_at) claim's AT-URIs; the STRONGER supersedes
    /// that latest claim and every other one stays listed as already
    /// signed. Otherwise the pair is already signed, never numbered.
    #[test]
    fn stronger_supersedes_the_latest_claim_when_support_has_an_uncited_repo(
        (links, claims) in arb_inference_inputs(),
        picks in proptest::collection::vec((0usize..16, any::<u8>(), 1u8..=28), 0..6),
    ) {
        let inferred = infer_person_candidates(&links, &claims);
        // Every claim cites at least its first supporting repo: inferred only.
        let picks: Vec<(usize, u8, u8)> = picks.into_iter().map(|(w, repos, day)| (w, repos | 1, day)).collect();
        let own = my_claims(&inferred, &picks);
        let report = infer_people_report(&links, &claims, &own, &InferenceFilter::default());
        for candidate in &inferred {
            let mine = mine_for(&own, candidate);
            let listed: BTreeSet<&str> = report
                .already_signed
                .iter()
                .filter(|a| pair_of(&a.candidate) == pair_of(candidate))
                .map(|a| a.cid.as_str())
                .collect();
            let Some(latest) = mine.iter().max_by_key(|c| (c.composed_at.clone(), c.cid.clone())) else {
                prop_assert_eq!(numbered_status(&report, candidate), Some(&CandidateStatus::New));
                continue;
            };
            let cited: BTreeSet<&str> = latest.evidence.iter().filter_map(|e| e.rsplit('/').next()).collect();
            let uncited_repo = candidate
                .support()
                .iter()
                .any(|repo| repo.claims.iter().all(|c| !cited.contains(c.cited.cid.as_str())));
            if uncited_repo {
                prop_assert_eq!(
                    numbered_status(&report, candidate),
                    Some(&CandidateStatus::Stronger { supersedes: latest.cid.clone() })
                );
                let others: BTreeSet<&str> = mine.iter().filter(|c| c.cid != latest.cid).map(|c| c.cid.as_str()).collect();
                prop_assert_eq!(listed, others);
            } else {
                prop_assert_eq!(numbered_status(&report, candidate), None);
                prop_assert_eq!(listed.len(), 1);
            }
        }
    }
}

// --- SUPPORT WEAKENED (US-CPI-004 AC3/AC4; DDD-8 / UC-5; D-5) ---

/// UC-5 oracle, straight from the rule's text, for one cited CID over the
/// whole read: absent → missing locally; retracted by its own author →
/// retracted; only cached from a peer I no longer follow → no longer
/// eligible; otherwise it still supports (not weakened).
fn expected_weakening(cid: &str, all: &[RepoClaim]) -> Option<WeakenedSupport> {
    let rows: Vec<&RepoClaim> = all.iter().filter(|c| c.cid == cid).collect();
    let retracted_by_author = |row: &RepoClaim| {
        all.iter().any(|other| {
            other.author_did == row.author_did
                && other
                    .references
                    .iter()
                    .any(|r| r.ref_type == ReferenceType::Retracts && r.cid.0 == row.cid)
        })
    };
    let active = |row: &&RepoClaim| {
        matches!(
            row.relationship,
            AuthorRelationship::You | AuthorRelationship::SubscribedPeer
        )
    };
    if rows.is_empty() {
        Some(WeakenedSupport::MissingLocally)
    } else if rows.iter().any(|row| retracted_by_author(row)) {
        Some(WeakenedSupport::Retracted)
    } else if !rows.iter().any(active) {
        Some(WeakenedSupport::NoLongerEligible)
    } else {
        None
    }
}

/// My inferred claim `bafymine<k>` citing, per pick, either the read's
/// claim at that index or (past the end) a CID no store row has.
fn my_claim_citing(claims: &[RepoClaim], picks: &[usize], k: usize) -> OwnClaim {
    let evidence = picks
        .iter()
        .map(|pick| match claims.get(*pick) {
            Some(claim) => CitedClaim::new(&claim.author_did, &claim.cid).at_uri(),
            None => CitedClaim::new("did:plc:gone", &format!("bafyabsent{pick}")).at_uri(),
        })
        .chain(std::iter::once(
            "https://github.com/o/r/commits?author=someone".to_string(),
        ))
        .collect();
    OwnClaim {
        subject: "github:someone".to_string(),
        predicate: ADHERES_TO_PHILOSOPHY.to_string(),
        object: "org.openlore.philosophy.memory-safety".to_string(),
        author_did: "did:plc:me".to_string(),
        cid: format!("bafymine{k}"),
        evidence,
        composed_at: format!("2026-09-{:02}T09:00:00Z", k + 1),
        references: Vec::new(),
    }
}

proptest! {
    /// DDD-8 / UC-5: each of my standing inferred claims is flagged
    /// exactly when some cited supporting claim is retracted, no longer
    /// eligible, or missing locally — with the distinct cited count and a
    /// correct count per reason; a claim whose every cited claim still
    /// supports it (or that cites none) is never flagged.
    #[test]
    fn weakened_support_counts_each_cited_claim_by_its_reason(
        (links, claims) in arb_inference_inputs(),
        cites in proptest::collection::vec(proptest::collection::vec(0usize..14, 0..6), 0..4),
    ) {
        let own: Vec<OwnClaim> = cites
            .iter()
            .enumerate()
            .map(|(k, picks)| my_claim_citing(&claims, picks, k))
            .collect();
        let report = infer_people_report(&links, &claims, &own, &InferenceFilter::default());
        for mine in &own {
            let cited: BTreeSet<String> =
                parse_provenance(&mine.evidence).into_iter().map(|c| c.cid).collect();
            let expected: BTreeMap<WeakenedSupport, usize> = cited
                .iter()
                .filter_map(|cid| expected_weakening(cid, &claims))
                .fold(BTreeMap::new(), |mut by_reason, reason| {
                    *by_reason.entry(reason).or_insert(0) += 1;
                    by_reason
                });
            let flagged = report.weakened.iter().find(|w| w.cid == mine.cid);
            if expected.is_empty() {
                prop_assert_eq!(flagged, None);
            } else {
                prop_assert!(flagged.is_some(), "weakened claim {} is flagged", mine.cid);
                let flagged = flagged.unwrap();
                prop_assert_eq!(flagged.cited, cited.len());
                prop_assert_eq!(&flagged.weakened, &expected);
                prop_assert_eq!(flagged.person_subject.as_str(), mine.subject.as_str());
            }
        }
    }
}
