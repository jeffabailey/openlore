//! contributor-philosophy-inference — shared acceptance support (DISTILL
//! 2026-09-27, Quinn).
//!
//! Step vocabulary for the five person-inference acceptance files
//! (`contributor_links`, `infer_people`, `infer_people_sign`,
//! `infer_people_evidence_grows`, `scrape_person`). Every WHEN drives the REAL
//! `openlore` binary (the production composition root — Pillar 3); every GIVEN
//! seeds state through the REAL verbs (`scrape github`, `claim add`,
//! `claim retract`, `peer add`/`peer pull`/`peer remove`). The ONLY doubles are
//! the driven-EXTERNAL ones already in the project policy: `FakeGithub`
//! (now serving `/contributors`, the ADR-063 lie catalogue), `PeerPds`,
//! `FakePds`, `FakeIdentity`.
//!
//! Observations are port-exposed (Mandate 8): CLI stdout/stderr/exit, the
//! `FakeGithub` request log, the signed claim artifacts (`claims/<cid>.json`
//! — the SAME bytes `claim publish` federates), the `claims` / `peer_claims`
//! row counts, and the `contribution_links` table whose columns DESIGN pinned
//! (DDD-5) — read exactly like the shipped `assert_no_claim_persisted` reads
//! `claims`.
//!
//! Output-wording pins (Q-CPI-D1, DISTILL-owned) live in the `pin_*` helpers
//! below so a wording change is a one-line edit, not a sweep.

use super::state_delta::{assert_state_delta, set_to, Delta};
use super::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::process::{Command, Stdio};

// =============================================================================
// Vocabulary constants (ADR-064 wire contract)
// =============================================================================

/// Person-level predicate (OD-CPI-2 / ADR-064 §2).
pub const ADHERES: &str = "adheresToPhilosophy";
/// Artifact-level predicate of the repo claims inference reads (D-2).
pub const EMBODIES: &str = "embodiesPhilosophy";
pub const MEMORY_SAFETY: &str = "org.openlore.philosophy.memory-safety";
pub const SEMANTIC_VERSIONING: &str = "org.openlore.philosophy.semantic-versioning";
pub const TEST_DRIVEN: &str = "org.openlore.philosophy.test-driven";
pub const DEPENDENCY_PINNING: &str = "org.openlore.philosophy.dependency-pinning";

// The subscribed peer (`RACHEL_DID` / `RACHEL_SEED` = did:plc:rachel-test,
// seed [7; 32]) is the shared support constant, reused as-is.

/// Pinned clock instants (`OPENLORE_TEST_NOW`) for the accumulation /
/// supersession scenarios.
pub const T1: &str = "2026-09-01T09:00:00Z";
pub const T2: &str = "2026-10-01T09:00:00Z";
pub const T3: &str = "2026-11-01T09:00:00Z";

/// Stable GitHub numeric ids used by the fixtures.
pub const BURNTSUSHI_ID: u64 = 456_674;
pub const DTOLNAY_ID: u64 = 1_940_490;

// =============================================================================
// Fixture contributor lists (FakeGithub `/contributors` bodies)
// =============================================================================

/// `BurntSushi/ripgrep`: BurntSushi #1, then 29 more humans, with TWO bots
/// (`dependabot[bot]`, `github-actions[bot]`) sitting INSIDE the top-30 by
/// commits — so "30 humans" requires looking past them (DDD-2/3). 32 rows.
pub fn ripgrep_contributors() -> Vec<FakeContributor> {
    let mut rows = vec![
        FakeContributor::human("BurntSushi", BURNTSUSHI_ID, 2_000),
        FakeContributor::bot("dependabot[bot]", 49_699_333, 950),
        FakeContributor::bot("github-actions[bot]", 41_898_282, 900),
    ];
    rows.extend(
        (1..=29u64)
            .map(|i| FakeContributor::human(&format!("rg-dev-{i:02}"), 10_000 + i, 800 - i * 10)),
    );
    rows
}

/// `rust-lang/regex`: BurntSushi #1 plus five other humans.
pub fn regex_contributors() -> Vec<FakeContributor> {
    let mut rows = vec![FakeContributor::human("BurntSushi", BURNTSUSHI_ID, 1_500)];
    rows.extend(
        (1..=5u64)
            .map(|i| FakeContributor::human(&format!("rx-dev-{i:02}"), 20_000 + i, 300 - i * 10)),
    );
    rows
}

/// A repo whose only recorded human is BurntSushi (#1) — the inference
/// scenarios' core-maintainer list, so the candidate set stays about him.
pub fn burntsushi_only() -> Vec<FakeContributor> {
    vec![FakeContributor::human("BurntSushi", BURNTSUSHI_ID, 1_000)]
}

/// `BurntSushi/jiff`: BurntSushi #1 plus two humans (third repo for the
/// person view).
pub fn jiff_contributors() -> Vec<FakeContributor> {
    vec![
        FakeContributor::human("BurntSushi", BURNTSUSHI_ID, 900),
        FakeContributor::human("jiff-dev-01", 30_001, 40),
        FakeContributor::human("jiff-dev-02", 30_002, 30),
    ]
}

/// `dtolnay/serde`: dtolnay #1 plus three humans.
pub fn serde_contributors() -> Vec<FakeContributor> {
    vec![
        FakeContributor::human("dtolnay", DTOLNAY_ID, 3_000),
        FakeContributor::human("serde-dev-01", 40_001, 200),
        FakeContributor::human("serde-dev-02", 40_002, 150),
        FakeContributor::human("serde-dev-03", 40_003, 100),
    ]
}

/// `dtolnay/anyhow`: dtolnay #1, a bot at raw position 2, then 7 humans —
/// `--contributors 5` must still yield exactly FIVE humans (the bot does not
/// eat a slot) and name the bot it skipped.
pub fn anyhow_contributors() -> Vec<FakeContributor> {
    let mut rows = vec![
        FakeContributor::human("dtolnay", DTOLNAY_ID, 500),
        FakeContributor::bot("dependabot[bot]", 49_699_333, 120),
    ];
    rows.extend(
        (1..=7u64).map(|i| {
            FakeContributor::human(&format!("anyhow-dev-{i:02}"), 50_000 + i, 100 - i * 5)
        }),
    );
    rows
}

/// A public repo posture serving `rows` on `/contributors` (no repo signals —
/// the candidate list stays empty so the contributors block is the focus).
pub fn repo_with(target: &str, rows: Vec<FakeContributor>) -> FakeGithub {
    FakeGithub::for_public_repo(target).with_contributors(rows)
}

/// A repo posture whose REAL metadata fires all five detectors (5 candidates:
/// [1] memory-safety, [2] dependency-pinning, [3] semantic-versioning,
/// [4] documentation, [5] test-driven) AND serves `rows` on `/contributors`.
pub fn repo_with_all_signals(target: &str, rows: Vec<FakeContributor>) -> FakeGithub {
    FakeGithub::for_public_repo_with_all_signals(target).with_contributors(rows)
}

// =============================================================================
// Driving the production binary
// =============================================================================

/// One subprocess invocation's knobs. `Default` = the standard seams
/// (`OPENLORE_HOME`, DID, key seed, the env's `FakePds`), no GitHub, no stdin,
/// real clock.
#[derive(Default)]
pub struct Invocation<'a> {
    pub github_base: Option<&'a str>,
    pub stdin: &'a str,
    pub now: Option<&'a str>,
    /// Omit `OPENLORE_PDS_ENDPOINT` (the network-disabled composition).
    pub offline: bool,
    pub peers: Vec<PeerSeam<'a>>,
}

/// Run `openlore <args>` through the production composition root with the
/// supplied seams. The single spawn path every person-inference step uses.
pub fn openlore_with(env: &TestEnv, args: &[&str], inv: Invocation<'_>) -> CliOutcome {
    let bin = assert_cmd::cargo::cargo_bin("openlore");
    let mut cmd = Command::new(&bin);
    cmd.args(args)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        .env("PATH", std::env::var("PATH").unwrap_or_default());
    if !inv.offline {
        cmd.env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url());
    }
    if let Some(base) = inv.github_base {
        cmd.env("OPENLORE_GITHUB_API_BASE", base);
    }
    if let Some(now) = inv.now {
        cmd.env("OPENLORE_TEST_NOW", now);
    }
    for peer in &inv.peers {
        cmd.env(peer_resolver_env_var(peer.peer_did), peer.peer_endpoint);
        cmd.env(peer_pubkey_env_var(peer.peer_did), peer.peer_pubkey_hex);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("spawn openlore at {bin:?}: {e}"));
    if !inv.stdin.is_empty() {
        child
            .stdin
            .as_mut()
            .expect("stdin pipe")
            .write_all(inv.stdin.as_bytes())
            .expect("write stdin");
    }
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("wait_with_output");
    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The outcome of one `scrape github` run plus every GitHub path it hit.
pub struct Scrape {
    pub outcome: CliOutcome,
    pub seen_paths: Vec<String>,
}

impl Scrape {
    /// How many `/contributors` reads the run made (KPI-CPI-5).
    pub fn contributor_requests(&self) -> usize {
        self.seen_paths
            .iter()
            .filter(|p| p.ends_with("/contributors"))
            .count()
    }
}

/// WHEN: Maria runs `openlore scrape github <args…>` against `fake`, at an
/// optional pinned instant, feeding `stdin` to any `--sign` prompts.
pub fn scrape_github(
    env: &TestEnv,
    args: &[&str],
    fake: FakeGithub,
    now: Option<&str>,
    stdin: &str,
) -> Scrape {
    let server = GithubServer::start(fake);
    let mut full = vec!["scrape", "github"];
    full.extend_from_slice(args);
    let outcome = openlore_with(
        env,
        &full,
        Invocation {
            github_base: Some(server.base_url()),
            stdin,
            now,
            ..Invocation::default()
        },
    );
    let seen_paths = server.fake().seen_paths();
    Scrape {
        outcome,
        seen_paths,
    }
}

/// GIVEN: `target` has been scraped (at `now`) and its contributors recorded.
/// Asserts the scrape succeeded so a broken precondition fails loudly HERE,
/// not as a confusing downstream assertion.
pub fn given_repo_scraped(
    env: &TestEnv,
    target: &str,
    rows: Vec<FakeContributor>,
    now: Option<&str>,
) -> Scrape {
    let scrape = scrape_github(env, &[target], repo_with(target, rows), now, "");
    assert_eq!(
        scrape.outcome.status, 0,
        "precondition: `scrape github {target}` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        scrape.outcome.stdout, scrape.outcome.stderr
    );
    scrape
}

/// WHEN: `openlore infer people <args…>` (local, real clock, PDS reachable).
pub fn infer_people(env: &TestEnv, args: &[&str]) -> CliOutcome {
    infer_people_with(env, args, "", None)
}

/// WHEN: `openlore infer people <args…>` with stdin for `--sign` and an
/// optional pinned clock.
pub fn infer_people_with(
    env: &TestEnv,
    args: &[&str],
    stdin: &str,
    now: Option<&str>,
) -> CliOutcome {
    let mut full = vec!["infer", "people"];
    full.extend_from_slice(args);
    openlore_with(
        env,
        &full,
        Invocation {
            stdin,
            now,
            ..Invocation::default()
        },
    )
}

/// WHEN: `openlore infer people <args…>` with NO network endpoint wired
/// (local-first guardrail, KPI-5).
pub fn infer_people_offline(env: &TestEnv, args: &[&str]) -> CliOutcome {
    let mut full = vec!["infer", "people"];
    full.extend_from_slice(args);
    openlore_with(
        env,
        &full,
        Invocation {
            offline: true,
            ..Invocation::default()
        },
    )
}

/// stdin for the shared sign batch: accept subject/predicate/object/evidence,
/// set confidence (empty = accept the proposed default), Enter to sign, then
/// answer the SEPARATE publish prompt (`publish` = Y / n). The `supersedes`
/// reference of a STRONGER candidate is shown in the preview, NOT an editable
/// prompt (DISTILL pin, DDD-12).
pub fn sign_stdin(confidence: &str, publish: bool) -> String {
    format!(
        "\n\n\n\n{confidence}\n\n{}\n",
        if publish { "Y" } else { "n" }
    )
}

// =============================================================================
// Seeding signed claims through the real verbs
// =============================================================================

/// The bare DID (fragment stripped) — the form `claim publish` mints into
/// at-uris and ADR-064 evidence (Q-CPI-D3 pin).
pub fn bare(did: &str) -> String {
    did.split('#').next().unwrap_or(did).to_string()
}

/// Maria's bare author DID.
pub fn my_did(env: &TestEnv) -> String {
    bare(env.identity.author_did())
}

/// ADR-064 §3 canonical AT-URI of a supporting claim.
pub fn at_uri(author_did: &str, cid: &str) -> String {
    format!("at://{}/org.openlore.claim/{cid}", bare(author_did))
}

/// ADR-064 §3 person-specific public source for one supporting repo.
pub fn commits_url(repo: &str, login: &str) -> String {
    format!("https://github.com/{repo}/commits?author={login}")
}

/// GIVEN: Maria signed `github:<repo> embodiesPhilosophy <philosophy>` at
/// `confidence` via the real `claim add` verb (signed locally, not published).
/// Returns the claim's CID.
pub fn given_i_signed_repo_claim(
    env: &TestEnv,
    repo: &str,
    philosophy: &str,
    confidence: f64,
    now: Option<&str>,
) -> String {
    given_i_signed(
        env,
        &format!("github:{repo}"),
        EMBODIES,
        philosophy,
        confidence,
        &[format!("https://github.com/{repo}")],
        now,
    )
}

/// GIVEN: Maria signed an arbitrary claim via the real `claim add` verb
/// (Enter to sign; EOF declines publish). Returns its CID.
pub fn given_i_signed(
    env: &TestEnv,
    subject: &str,
    predicate: &str,
    object: &str,
    confidence: f64,
    evidence: &[String],
    now: Option<&str>,
) -> String {
    let confidence = format!("{confidence}");
    let mut args: Vec<&str> = vec![
        "claim",
        "add",
        "--subject",
        subject,
        "--predicate",
        predicate,
        "--object",
        object,
        "--confidence",
        &confidence,
    ];
    for url in evidence {
        args.push("--evidence");
        args.push(url);
    }
    let outcome = openlore_with(
        env,
        &args,
        Invocation {
            stdin: "\n",
            now,
            ..Invocation::default()
        },
    );
    assert_eq!(
        outcome.status, 0,
        "precondition: `claim add` {subject} {predicate} {object} must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );
    signed_cid_from_stdout(&outcome.stdout)
}

/// GIVEN: Maria retracts one of her own claims via the real `claim retract`
/// verb (the self-retraction ADR-060 D-RF-D3 honors). Returns the marker CID.
pub fn given_i_retracted(env: &TestEnv, cid: &str) -> String {
    let outcome = openlore_with(env, &["claim", "retract", cid], Invocation::default());
    assert_eq!(
        outcome.status, 0,
        "precondition: `claim retract {cid}` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );
    signed_cid_from_stdout(&outcome.stdout)
}

/// One peer claim to publish on a `PeerPds` (full control of predicate,
/// evidence and references — the retraction / supersede / counter markers).
#[derive(Debug, Clone)]
pub struct PeerClaimSpec {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub evidence: Vec<String>,
    pub references: Vec<(claim_domain::ReferenceType, String)>,
    pub composed_at: String,
}

impl PeerClaimSpec {
    /// `github:<repo> embodiesPhilosophy <philosophy>` at `confidence`.
    pub fn repo_claim(repo: &str, philosophy: &str, confidence: f64) -> Self {
        Self {
            subject: format!("github:{repo}"),
            predicate: EMBODIES.to_string(),
            object: philosophy.to_string(),
            confidence,
            evidence: vec![format!("https://github.com/{repo}")],
            references: Vec::new(),
            composed_at: "2026-09-02T10:00:00Z".to_string(),
        }
    }

    /// The same assertion carrying one reference (`retracts` / `counters` /
    /// `supersedes`) at a later instant — how a peer marks or replaces it.
    pub fn marking(&self, ref_type: claim_domain::ReferenceType, target_cid: &str) -> Self {
        Self {
            references: vec![(ref_type, target_cid.to_string())],
            composed_at: "2026-09-03T10:00:00Z".to_string(),
            ..self.clone()
        }
    }

    pub fn with_confidence(&self, confidence: f64) -> Self {
        Self {
            confidence,
            ..self.clone()
        }
    }
}

/// Materialize verifiable peer records (REAL Ed25519 + CID) for `specs`;
/// returns `(records, cids-in-order, pubkey_hex)`.
pub fn peer_records(
    peer_did: &str,
    seed: [u8; 32],
    specs: &[PeerClaimSpec],
) -> (Vec<FakePeerRecord>, Vec<String>, String) {
    use claim_domain::{canonicalize, compute_cid, sign, SigningKey, VerifyingKey};
    use ed25519_dalek::SigningKey as DalekSigningKey;

    let dalek_sk = DalekSigningKey::from_bytes(&seed);
    let signing_key = SigningKey(dalek_sk.to_bytes().to_vec());
    let pubkey_hex = hex_lower(&VerifyingKey(dalek_sk.verifying_key().to_bytes().to_vec()).0);
    let author = format!("{peer_did}#org.openlore.application");

    let mut records = Vec::new();
    let mut cids = Vec::new();
    for spec in specs {
        let confidence: claim_domain::Confidence =
            serde_json::from_value(serde_json::json!(spec.confidence)).expect("confidence");
        let unsigned = claim_domain::UnsignedClaim {
            subject: spec.subject.clone(),
            predicate: spec.predicate.clone(),
            object: spec.object.clone(),
            evidence: spec.evidence.clone(),
            confidence,
            author_did: claim_domain::Did(author.clone()),
            composed_at: spec.composed_at.clone(),
            references: spec
                .references
                .iter()
                .map(|(ref_type, cid)| claim_domain::ClaimReference {
                    ref_type: *ref_type,
                    cid: claim_domain::Cid(cid.clone()),
                })
                .collect(),
            reason: None,
        };
        let cid = compute_cid(&canonicalize(&unsigned).expect("canonicalize peer claim"));
        let signature = sign(&cid, &signing_key).expect("sign peer claim");
        let refs: Vec<serde_json::Value> = spec
            .references
            .iter()
            .map(|(ref_type, target)| {
                let name = match ref_type {
                    claim_domain::ReferenceType::Retracts => "retracts",
                    claim_domain::ReferenceType::Corrects => "corrects",
                    claim_domain::ReferenceType::Counters => "counters",
                    claim_domain::ReferenceType::Supersedes => "supersedes",
                };
                serde_json::json!({ "type": name, "cid": target })
            })
            .collect();
        let body = serde_json::json!({
            "subject": spec.subject,
            "predicate": spec.predicate,
            "object": spec.object,
            "evidence": spec.evidence,
            "confidence": spec.confidence,
            "author": author,
            "composedAt": spec.composed_at,
            "references": refs,
            "signature": {
                "kid": author,
                "alg": "EdDSA",
                "sig": base64url_no_pad(&signature.signature_bytes),
            }
        });
        cids.push(cid.0.clone());
        records.push(FakePeerRecord::claim(cid.0, body));
    }
    (records, cids, pubkey_hex)
}

/// A subscribed peer whose published record set can GROW between pulls
/// (retraction / supersede / counter markers arrive later).
pub struct SubscribedPeer {
    pub did: &'static str,
    seed: [u8; 32],
    published: Vec<PeerClaimSpec>,
    /// CIDs of `published`, same order.
    pub cids: Vec<String>,
    _pds: Option<PeerPds>,
}

impl SubscribedPeer {
    /// GIVEN: Maria subscribes to `did` and pulls its `specs` (real `peer add`
    /// + `peer pull`). The returned handle's `cids[i]` is `specs[i]`'s CID.
    pub fn subscribe_and_pull(
        env: &TestEnv,
        did: &'static str,
        seed: [u8; 32],
        specs: Vec<PeerClaimSpec>,
    ) -> Self {
        let mut peer = Self {
            did,
            seed,
            published: Vec::new(),
            cids: Vec::new(),
            _pds: None,
        };
        let (records, _, _) = peer_records(did, seed, &specs);
        let pds = PeerPds::for_peer(did, records);
        let added =
            run_openlore_with_peer_resolver(env, &["peer", "add", did], did, pds.endpoint_url());
        assert_eq!(
            added.status, 0,
            "precondition: `peer add {did}` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            added.stdout, added.stderr
        );
        drop(pds);
        peer.publish_and_pull(env, specs);
        peer
    }

    /// GIVEN: the peer publishes `more` claims (markers or new assertions) and
    /// Maria pulls again. Returns the CIDs of `more`, in order.
    pub fn publish_and_pull(&mut self, env: &TestEnv, more: Vec<PeerClaimSpec>) -> Vec<String> {
        self.published.extend(more.iter().cloned());
        let (records, cids, pubkey) = peer_records(self.did, self.seed, &self.published);
        let pds = PeerPds::for_peer(self.did, records);
        let pulled = run_openlore_pull(
            env,
            &["peer", "pull"],
            self.did,
            pds.endpoint_url(),
            &pubkey,
        );
        assert_eq!(
            pulled.status, 0,
            "precondition: `peer pull` of {} must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.did, pulled.stdout, pulled.stderr
        );
        let new = cids[self.cids.len()..].to_vec();
        self.cids = cids;
        self._pds = Some(pds);
        new
    }

    /// GIVEN: Maria unsubscribes (soft `peer remove`, cache kept).
    pub fn unsubscribe(&self, env: &TestEnv) {
        let removed = run_openlore(env, &["peer", "remove", self.did]);
        assert_eq!(
            removed.status, 0,
            "precondition: `peer remove {}` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.did, removed.stdout, removed.stderr
        );
    }

    /// GIVEN: Maria unsubscribes AND purges the peer's cached claims.
    pub fn unsubscribe_and_purge(&self, env: &TestEnv) {
        let removed = run_openlore_with_stdin(env, &["peer", "remove", self.did, "--purge"], "y\n");
        assert_eq!(
            removed.status, 0,
            "precondition: `peer remove {} --purge` must succeed;\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.did, removed.stdout, removed.stderr
        );
    }
}

/// GIVEN (the canonical US-CPI-002 store): BurntSushi (and only he) is linked
/// to `BurntSushi/ripgrep` and `rust-lang/regex`, #1 in both; Maria signed ripgrep
/// memory-safety at 0.55; subscribed peer Rachel signed regex memory-safety
/// at 0.60. Returns `(maria_ripgrep_cid, rachel)`.
pub fn given_burntsushi_builds_two_signed_memory_safe_repos(
    env: &TestEnv,
) -> (String, SubscribedPeer) {
    // Core-maintainer lists (BurntSushi only) keep the candidate set to the
    // person under discussion; the realistic 30-person lists are exercised by
    // the slice-01 scenarios and the walking skeleton.
    given_repo_scraped(env, "BurntSushi/ripgrep", burntsushi_only(), None);
    given_repo_scraped(env, "rust-lang/regex", burntsushi_only(), None);
    let mine = given_i_signed_repo_claim(env, "BurntSushi/ripgrep", MEMORY_SAFETY, 0.55, None);
    let rachel = SubscribedPeer::subscribe_and_pull(
        env,
        RACHEL_DID,
        RACHEL_SEED,
        vec![PeerClaimSpec::repo_claim(
            "rust-lang/regex",
            MEMORY_SAFETY,
            0.60,
        )],
    );
    (mine, rachel)
}

// =============================================================================
// Observations (port-exposed)
// =============================================================================

/// One recorded contribution link, as DDD-5 pins the `contribution_links`
/// columns. Timestamps normalized to their `YYYY-MM-DD` date.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Link {
    pub repo: String,
    pub person: String,
    pub github_user_id: i64,
    pub rank: i64,
    pub contributions: i64,
    pub first_observed: String,
    pub last_observed: String,
}

/// Every recorded contribution link (empty when the table does not exist yet
/// — "nothing recorded" in its strongest form, mirroring
/// `assert_no_claim_persisted`).
pub fn contribution_links(env: &TestEnv) -> Vec<Link> {
    let path = env.duckdb_path();
    if !path.exists() {
        return Vec::new();
    }
    let conn = duckdb::Connection::open(&path)
        .unwrap_or_else(|e| panic!("open DuckDB for contribution links: {e}"));
    let Ok(mut stmt) = conn.prepare(
        "SELECT repo_subject, person_subject, CAST(github_user_id AS BIGINT), \
         CAST(rank AS BIGINT), CAST(contributions AS BIGINT), \
         CAST(first_observed_at AS VARCHAR), CAST(last_observed_at AS VARCHAR) \
         FROM contribution_links ORDER BY repo_subject, rank, person_subject",
    ) else {
        return Vec::new();
    };
    stmt.query_map([], |row| {
        let first: String = row.get(5)?;
        let last: String = row.get(6)?;
        Ok(Link {
            repo: row.get(0)?,
            person: row.get(1)?,
            github_user_id: row.get(2)?,
            rank: row.get(3)?,
            contributions: row.get(4)?,
            first_observed: first.chars().take(10).collect(),
            last_observed: last.chars().take(10).collect(),
        })
    })
    .unwrap_or_else(|e| panic!("query contribution links: {e}"))
    .map(|r| r.unwrap_or_else(|e| panic!("decode contribution link: {e}")))
    .collect()
}

/// Links recorded for one repo subject (e.g. `github:BurntSushi/ripgrep`).
pub fn links_for(env: &TestEnv, repo_subject: &str) -> Vec<Link> {
    contribution_links(env)
        .into_iter()
        .filter(|l| l.repo.eq_ignore_ascii_case(repo_subject))
        .collect()
}

fn count_rows(env: &TestEnv, table: &str) -> i64 {
    let path = env.duckdb_path();
    if !path.exists() {
        return 0;
    }
    let conn = duckdb::Connection::open(&path).unwrap_or_else(|e| panic!("open DuckDB: {e}"));
    conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap_or(0)
}

/// Every local signed-claim artifact, CID → exact bytes (the byte-compare
/// universe for KPI-CPI-4).
pub fn claim_files(env: &TestEnv) -> BTreeMap<String, Vec<u8>> {
    let dir = env.claims_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return BTreeMap::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let cid = name.strip_suffix(".json")?.to_string();
            Some((cid, std::fs::read(e.path()).ok()?))
        })
        .collect()
}

/// Read back one of Maria's signed claims from its local artifact.
pub fn read_signed(env: &TestEnv, cid: &str) -> claim_domain::SignedClaim {
    let path = env.claims_dir().join(format!("{cid}.json"));
    let bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("read signed claim {}: {e}", path.display()));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("decode signed claim {}: {e}", path.display()))
}

/// The numeric confidence a signed claim records.
pub fn confidence_of(claim: &claim_domain::SignedClaim) -> f64 {
    serde_json::to_value(claim.unsigned.confidence)
        .ok()
        .and_then(|v| v.as_f64())
        .expect("numeric confidence")
}

/// Recompute a signed claim's CID from its unsigned body with the REAL
/// canonicalizer (re-verification, ADR-006).
pub fn recomputed_cid(claim: &claim_domain::SignedClaim) -> String {
    let canonical = claim_domain::canonicalize(&claim.unsigned).expect("canonicalize signed claim");
    claim_domain::compute_cid(&canonical).0
}

/// The CIDs of every local claim Maria authored AFTER `before` (new files).
pub fn new_claim_cids(before: &BTreeMap<String, Vec<u8>>, env: &TestEnv) -> Vec<String> {
    claim_files(env)
        .into_keys()
        .filter(|cid| !before.contains_key(cid))
        .collect()
}

/// The port-exposed store universe every read-only person-inference step must
/// leave untouched.
pub fn store_universe_slots() -> HashSet<String> {
    [
        "local.claims.row_count",
        "local.claim_files",
        "local.peer_claims.row_count",
        "local.contribution_links",
        "pds.records.len",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

/// Capture the store universe (see [`store_universe_slots`]).
pub fn capture_store_universe(env: &TestEnv) -> HashMap<String, String> {
    HashMap::from([
        (
            "local.claims.row_count".to_string(),
            count_rows(env, "claims").to_string(),
        ),
        (
            "local.claim_files".to_string(),
            format!("{:?}", claim_files(env).keys().collect::<Vec<_>>()),
        ),
        (
            "local.peer_claims.row_count".to_string(),
            count_rows(env, "peer_claims").to_string(),
        ),
        (
            "local.contribution_links".to_string(),
            format!("{:?}", contribution_links(env)),
        ),
        (
            "pds.records.len".to_string(),
            env.pds.records().len().to_string(),
        ),
    ])
}

/// THEN: the step wrote NOTHING — every store slot is implicitly unchanged
/// (Mandate 8, fail-closed).
pub fn assert_store_unchanged(before: &HashMap<String, String>, env: &TestEnv) {
    let after = capture_store_universe(env);
    assert_state_delta(before, &after, &store_universe_slots(), &Delta::new());
}

/// THEN: exactly the named slots changed to the given values; everything else
/// in the store universe is unchanged.
pub fn assert_store_delta(
    before: &HashMap<String, String>,
    env: &TestEnv,
    changes: &[(&str, String)],
) {
    let after = capture_store_universe(env);
    let expected = changes.iter().fold(Delta::new(), |d, (slot, value)| {
        d.with_slot(*slot, set_to(value.clone()))
    });
    assert_state_delta(before, &after, &store_universe_slots(), &expected);
}

// =============================================================================
// Output pins (Q-CPI-D1 — DISTILL-owned wording contract)
// =============================================================================

/// The headline line of numbered candidate `[n]` (a line containing `[n] `).
pub fn candidate_line(stdout: &str, n: usize) -> Option<String> {
    let marker = format!("[{n}] ");
    stdout
        .lines()
        .find(|l| l.contains(&marker))
        .map(str::to_string)
}

/// Numbered candidate `[n]`'s block: its headline plus the indented lines
/// beneath it (provenance, arithmetic, labels) up to the next numbered line or
/// the first non-indented line.
pub fn candidate_block(stdout: &str, n: usize) -> Option<String> {
    let marker = format!("[{n}] ");
    let mut lines = stdout.lines().skip_while(|l| !l.contains(&marker));
    let head = lines.next()?;
    let mut block = vec![head.to_string()];
    for line in lines {
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let numbered = (1..=200).any(|k| line.contains(&format!("[{k}] ")));
        if !indented || numbered {
            break;
        }
        block.push(line.to_string());
    }
    Some(block.join("\n"))
}

/// How many numbered candidates `[1]..[k]` the output lists.
pub fn numbered_count(stdout: &str) -> usize {
    (1..)
        .take_while(|n| candidate_line(stdout, *n).is_some())
        .count()
}

/// THEN: candidate `[n]` proposes `person` adheres to `philosophy`.
pub fn assert_candidate(outcome: &CliOutcome, n: usize, person: &str, philosophy: &str) {
    let line = candidate_line(&outcome.stdout, n).unwrap_or_else(|| {
        panic!(
            "expected numbered candidate [{n}];\n--- stdout ---\n{}\n--- stderr ---\n{}",
            outcome.stdout, outcome.stderr
        )
    });
    let short = philosophy.rsplit('.').next().unwrap_or(philosophy);
    assert!(
        line.contains(person) && line.contains(short),
        "candidate [{n}] must propose {person} adheres to {philosophy}; got line: {line:?}\n--- stdout ---\n{}",
        outcome.stdout
    );
}

/// THEN: candidate `[n]`'s provenance has ONE line naming the supporting
/// repo, the person's rank there (`#<rank>`), the supporting claim CID and
/// THAT claim's own author DID (D-7 anti-merging, D-9, KPI-CPI-2).
pub fn assert_provenance(
    outcome: &CliOutcome,
    n: usize,
    repo_subject: &str,
    rank: u32,
    cid: &str,
    author_did: &str,
) {
    let block = candidate_block(&outcome.stdout, n).unwrap_or_else(|| {
        panic!(
            "expected candidate [{n}];\n--- stdout ---\n{}",
            outcome.stdout
        )
    });
    let author = bare(author_did);
    let rank_pin = format!("#{rank}");
    assert!(
        block.lines().any(|l| l.contains(repo_subject)
            && l.contains(&rank_pin)
            && l.contains(cid)
            && l.contains(&author)),
        "candidate [{n}] must show a provenance line with {repo_subject}, {rank_pin}, claim {cid} and its author {author};\n--- block ---\n{block}"
    );
}

/// THEN: candidate `[n]`'s provenance does NOT cite `needle` (a repo or CID).
pub fn assert_provenance_omits(outcome: &CliOutcome, n: usize, needle: &str) {
    let block = candidate_block(&outcome.stdout, n).unwrap_or_else(|| {
        panic!(
            "expected candidate [{n}];\n--- stdout ---\n{}",
            outcome.stdout
        )
    });
    assert!(
        !block.contains(needle),
        "candidate [{n}] must not cite {needle};\n--- block ---\n{block}"
    );
}

/// THEN: candidate `[n]` shows `confidence` (two decimals) in the speculative
/// bucket with its arithmetic (J-002c reproduce-by-hand, DDD-9).
pub fn assert_speculative_confidence(outcome: &CliOutcome, n: usize, confidence: &str) {
    let block = candidate_block(&outcome.stdout, n).unwrap_or_else(|| {
        panic!(
            "expected candidate [{n}];\n--- stdout ---\n{}",
            outcome.stdout
        )
    });
    assert!(
        block.contains(&format!("{confidence} (speculative)")),
        "candidate [{n}] must show confidence '{confidence} (speculative)';\n--- block ---\n{block}"
    );
    assert!(
        block.contains("min("),
        "candidate [{n}] must show the confidence arithmetic (min(…)) so it can be reproduced by hand;\n--- block ---\n{block}"
    );
}

/// THEN: the empty-result pin.
pub fn assert_no_inferred_candidates(outcome: &CliOutcome) {
    assert_eq!(
        outcome.status, 0,
        "no candidates is a normal outcome (exit 0);\n--- stdout ---\n{}\n--- stderr ---\n{}",
        outcome.stdout, outcome.stderr
    );
    assert!(
        outcome.stdout.contains("No inferred candidates"),
        "expected 'No inferred candidates';\n--- stdout ---\n{}",
        outcome.stdout
    );
    assert_eq!(
        numbered_count(&outcome.stdout),
        0,
        "no numbered candidate may be listed"
    );
}

/// THEN: the overlap pin — `<person> → <repo>` (the ASCII `->` of the
/// journey mockup is accepted too).
pub fn shows_overlap(stdout: &str, person: &str, repo: &str) -> bool {
    stdout.contains(&format!("{person} → {repo}"))
        || stdout.contains(&format!("{person} -> {repo}"))
}
