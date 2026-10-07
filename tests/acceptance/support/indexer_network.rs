//! indexer-per-did-pds-fetch — shared acceptance harness (DISTILL 2026-10-05).
//!
//! Included by the `indexer_per_did_*` / `search_self_attested_label` suites
//! with `#[path = "support/indexer_network.rs"] mod indexer_network;` (after
//! `mod support;`, whose `TestEnv` / binary-resolution helpers it reuses).
//!
//! Architecture of Reference (`docs/architecture/atdd-infrastructure-policy.md`):
//!
//! | Port | Treatment here |
//! |---|---|
//! | `openlore-indexer ingest` / `serve` (driving) | REAL binary, subprocess |
//! | `openlore search` (driving) | REAL binary, subprocess |
//! | `index.duckdb` (driven internal) | REAL file under the scenario's home |
//! | PLC directory, author PDSes, fallback source (driven external) | ONE `FakeAtprotoNetwork`: a directory + one loopback server per PDS host |
//!
//! The fakes listen on `http://127.0.0.1`, so every run sets the TEST-ONLY
//! `OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1` (debug builds only, DD-IPF-5) unless a
//! scenario switches it off on purpose.
//!
//! Vocabulary (Mandate-12 SSOT): typed [`Author`]s, [`Host`]s, [`Claim`]s,
//! [`SkipReason`]s and [`Provenance`]s; one [`IndexerWorld`] whose methods are
//! the Given/When steps; a [`PassReport`] whose readers are the Then
//! observables. Every observable is port-exposed: the indexer's stdout/stderr
//! events and exit code, the `index.duckdb` rows, the `openlore search` output,
//! and the fake network's request log. Nothing reads indexer internals.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use openlore_test_support::{
    BlueskyAccount, FakeAtprotoNetwork, FixtureKeypair, RawRecordSpec, CLAIM_COLLECTION,
};
pub use openlore_test_support::{DidDocPosture, ListingPosture};

use crate::support::{self, CliOutcome, TestEnv};

// =============================================================================
// Domain vocabulary
// =============================================================================

/// The people of the journey (`journey-find-claims-on-every-pds.feature`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Author {
    /// bsky.social author; self-attested claims on her own PDS.
    Priya,
    /// Self-hosted author on `pds.volkov.dev`; app-signed claims.
    Dmitri,
    /// The operator; his own PDS doubles as the fallback source.
    Jeff,
    /// A DID nobody can resolve (no DID document anywhere).
    Ghost,
    /// A DID whose PDS answers with somebody else's records.
    Mallory,
    /// An author whose PDS accepts connections and never answers.
    Sam,
    /// A `did:web` author whose web host serves no DID document right now.
    Wren,
    /// bsky.social author the operator adds to the DID list later
    /// (indexer-deployment US-IXD-003); self-attested claims.
    Tomas,
}

impl Author {
    pub const fn did(self) -> &'static str {
        match self {
            Author::Priya => "did:plc:priyaraman7x2k",
            Author::Dmitri => "did:plc:dvolkov3m9q",
            Author::Jeff => "did:plc:jeffbailey5n2p",
            Author::Ghost => "did:plc:ghost0000",
            Author::Mallory => "did:plc:mallory4k1z",
            Author::Sam => "did:plc:samslowhost6w",
            Author::Wren => "did:web:localhost",
            Author::Tomas => "did:plc:therrera2v6w",
        }
    }

    pub const fn handle(self) -> &'static str {
        match self {
            Author::Priya => "priyaraman.bsky.social",
            Author::Dmitri => "dmitri.volkov.dev",
            Author::Jeff => "jeffbailey.us",
            Author::Ghost => "ghost.invalid",
            Author::Mallory => "mallory.example",
            Author::Sam => "sam.slowhost.example",
            Author::Wren => "localhost",
            Author::Tomas => "tomasherrera.bsky.social",
        }
    }

    /// The host the author's DID document names at the start of a scenario.
    pub const fn home(self) -> Option<Host> {
        match self {
            Author::Priya => Some(Host::MorelBsky),
            Author::Dmitri => Some(Host::VolkovDev),
            Author::Jeff => Some(Host::JeffbaileyUs),
            Author::Ghost => None,
            Author::Mallory => Some(Host::MalloryPds),
            Author::Sam => Some(Host::SlowhostExample),
            Author::Wren => None,
            Author::Tomas => Some(Host::MorelBsky),
        }
    }

    /// The app-signed author identity (`<did>#org.openlore.application`).
    pub fn app_identity(self) -> String {
        format!("{}#org.openlore.application", self.did())
    }
}

/// The PDS hosts of the fake network (labels map to loopback servers).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// `https://morel.us-east.host.bsky.network` — Priya's bsky.social PDS.
    MorelBsky,
    /// `https://pds.volkov.dev`.
    VolkovDev,
    /// `https://pds.jeffbailey.us` — Jeff's PDS, the fallback source.
    JeffbaileyUs,
    /// `https://pds.priyaraman.dev` — where Priya migrates to.
    PriyaramanDev,
    /// Mallory's PDS.
    MalloryPds,
    /// `https://pds.slowhost.example`.
    SlowhostExample,
}

impl Host {
    pub const ALL: [Host; 6] = [
        Host::MorelBsky,
        Host::VolkovDev,
        Host::JeffbaileyUs,
        Host::PriyaramanDev,
        Host::MalloryPds,
        Host::SlowhostExample,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Host::MorelBsky => "morel-us-east-host-bsky-network",
            Host::VolkovDev => "pds-volkov-dev",
            Host::JeffbaileyUs => "pds-jeffbailey-us",
            Host::PriyaramanDev => "pds-priyaraman-dev",
            Host::MalloryPds => "pds-mallory-example",
            Host::SlowhostExample => "pds-slowhost-example",
        }
    }
}

/// Why a DID contributed nothing to a pass (data-models.md §2 wire tokens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    DidUnresolvable,
    PdsUnreachable,
    PdsTimeout,
    ListingFailed,
    PdsAddressRefused,
}

impl SkipReason {
    pub const fn token(self) -> &'static str {
        match self {
            SkipReason::DidUnresolvable => "did_unresolvable",
            SkipReason::PdsUnreachable => "pds_unreachable",
            SkipReason::PdsTimeout => "pds_timeout",
            SkipReason::ListingFailed => "listing_failed",
            SkipReason::PdsAddressRefused => "pds_address_refused",
        }
    }
}

/// How an indexed claim's authorship was established (ADR-071).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    AppSigned,
    SelfAttested,
}

impl Provenance {
    pub const fn token(self) -> &'static str {
        match self {
            Provenance::AppSigned => "app-signed",
            Provenance::SelfAttested => "self-attested",
        }
    }
}

/// Why the indexer refused a record (`indexer.ingest.rejected.by_reason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    CidMismatch,
    Provenance,
    ForeignRepo,
}

impl RefusalReason {
    pub const fn token(self) -> &'static str {
        match self {
            RefusalReason::CidMismatch => "cid_mismatch",
            RefusalReason::Provenance => "provenance",
            RefusalReason::ForeignRepo => "foreign_repo",
        }
    }
}

/// One "project embodies philosophy" claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Claim {
    pub subject: &'static str,
    pub philosophy: &'static str,
    pub basis_points: i64,
}

impl Claim {
    pub fn object(&self) -> String {
        format!("org.openlore.philosophy.{}", self.philosophy)
    }
}

/// The claims of the journey (realistic, illustrative).
pub mod claims {
    use super::Claim;

    pub const CARGO_PIN_REPRODUCIBLE_BUILDS: Claim = Claim {
        subject: "github:priyaraman/cargo-pin",
        philosophy: "reproducible-builds",
        basis_points: 8200,
    };
    pub const CARGO_PIN_DEPENDENCY_PINNING: Claim = Claim {
        subject: "github:priyaraman/cargo-pin",
        philosophy: "dependency-pinning",
        basis_points: 7500,
    };
    pub const TIDEPOOL_MEMORY_SAFETY: Claim = Claim {
        subject: "github:priyaraman/tidepool",
        philosophy: "memory-safety",
        basis_points: 6000,
    };
    pub const FERRITE_REPRODUCIBLE_BUILDS: Claim = Claim {
        subject: "github:dvolkov/ferrite",
        philosophy: "reproducible-builds",
        basis_points: 7000,
    };
    pub const FERRITE_DEPENDENCY_PINNING: Claim = Claim {
        subject: "github:dvolkov/ferrite",
        philosophy: "dependency-pinning",
        basis_points: 6500,
    };
    pub const FERRITE_TEST_DRIVEN: Claim = Claim {
        subject: "github:dvolkov/ferrite",
        philosophy: "test-driven",
        basis_points: 5500,
    };
    pub const OPENLORE_LOCAL_FIRST: Claim = Claim {
        subject: "github:jeffabailey/openlore",
        philosophy: "local-first",
        basis_points: 9000,
    };
    pub const GHOST_APP_SIGNED_ONE: Claim = Claim {
        subject: "github:ghost/lantern",
        philosophy: "reproducible-builds",
        basis_points: 6000,
    };
    pub const GHOST_APP_SIGNED_TWO: Claim = Claim {
        subject: "github:ghost/lantern",
        philosophy: "semantic-versioning",
        basis_points: 6200,
    };
    pub const GHOST_SELF_ATTESTED: Claim = Claim {
        subject: "github:ghost/lantern",
        philosophy: "memory-safety",
        basis_points: 5000,
    };
    pub const WREN_APP_SIGNED: Claim = Claim {
        subject: "github:wren/tidy",
        philosophy: "documentation-first",
        basis_points: 7100,
    };
    pub const SAM_SLOW_CLAIM: Claim = Claim {
        subject: "github:samslow/tortoise",
        philosophy: "documentation-first",
        basis_points: 4000,
    };
}

/// The base URL of a named variable the indexer reads (data-models.md §4).
pub mod var {
    pub const REPO_DIDS: &str = "OPENLORE_INDEXER_REPO_DIDS";
    pub const FALLBACK: &str = "OPENLORE_INDEXER_SOURCE_URL";
    pub const PLC: &str = "OPENLORE_INDEXER_PLC_ENDPOINT";
    pub const LOOPBACK_SEAM: &str = "OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP";
    pub const MAX_CONCURRENT: &str = "OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES";
    pub const PER_DID_TIMEOUT: &str = "OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS";
}

/// An upper bound for any single indexer run in these suites. A run that
/// exceeds it is killed and reported (so a missing deadline fails the scenario
/// instead of hanging the test binary).
const RUN_CEILING: Duration = Duration::from_secs(90);

// =============================================================================
// Records as the author's PDS holds them
// =============================================================================

/// The `org.openlore.claim` record value the review app writes for a
/// self-attested claim (ADR-071: no signature, bare author == repo DID).
pub fn self_attested_value(author: &str, claim: Claim) -> serde_json::Value {
    serde_json::json!({
        "$type": CLAIM_COLLECTION,
        "subject": claim.subject,
        "predicate": "embodiesPhilosophy",
        "object": claim.object(),
        "evidence": [format!("https://{}", claim.subject.replace("github:", "github.com/"))],
        "confidence": claim.basis_points,
        "author": author,
        "composedAt": "2026-10-04T15:02:11Z",
    })
}

/// The CID an honest PDS would key a record under (the ADR-071 `CID == rkey` rule).
pub fn recomputed_cid(value: &serde_json::Value) -> String {
    let text = |k: &str| {
        value
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let claim = claim_domain::UnsignedClaim {
        subject: text("subject"),
        predicate: text("predicate"),
        object: text("object"),
        evidence: value
            .get("evidence")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
        confidence: value
            .get("confidence")
            .and_then(claim_domain::Confidence::from_wire)
            .unwrap_or_else(|| claim_domain::Confidence::from_basis_points(-1)),
        author_did: claim_domain::Did(text("author")),
        composed_at: text("composedAt"),
        references: Vec::new(),
        reason: None,
    };
    let bytes = claim_domain::canonicalize(&claim).expect("canonicalize a claim value");
    claim_domain::compute_cid(&bytes).0
}

/// An app-signed record (real Ed25519 signature by the author's fixture key),
/// as `(record key, value)`.
pub fn app_signed_record(author: &str, claim: Claim) -> (String, serde_json::Value) {
    let raw = RawRecordSpec::valid(
        author,
        claim.subject,
        &claim.object(),
        claim.basis_points as f64 / 10_000.0,
    )
    .into_raw_record();
    let view = support::raw_record_to_list_records_view(&raw);
    (
        view["cid"].as_str().expect("published cid").to_string(),
        view["value"].clone(),
    )
}

/// Maria's initialized OpenLore home (the searcher's CLI identity); the
/// indexer's own `index.duckdb` lives under the same temp home.
pub fn maria_home() -> TestEnv {
    TestEnv::initialized_as(support::FakeIdentity::maria())
}

// =============================================================================
// The world (Given / When steps)
// =============================================================================

/// One hermetic world: the scenario's home (index + CLI identity), the fake
/// ATProto network, and the indexer configuration the operator chose.
pub struct IndexerWorld {
    pub env: TestEnv,
    pub net: FakeAtprotoNetwork,
    configured: Vec<String>,
    fallback: Option<String>,
    plc_endpoint: Option<String>,
    app_signed_authors: Vec<String>,
    extra_env: BTreeMap<String, String>,
    loopback_seam: bool,
}

impl IndexerWorld {
    /// GIVEN the indexer is configured with `authors`' repo DIDs, each author
    /// (except Ghost) hosted on their home PDS. Every [`Host`] is up, so an
    /// author can move and a fallback can be chosen later.
    pub fn configured_with(authors: &[Author]) -> Self {
        let accounts = authors
            .iter()
            .filter_map(|a| {
                a.home()
                    .map(|h| BlueskyAccount::new(a.handle(), a.did(), h.label()))
            })
            .collect();
        let hosts: Vec<&str> = Host::ALL.iter().map(|h| h.label()).collect();
        Self {
            env: maria_home(),
            net: FakeAtprotoNetwork::start_with_extra_hosts(accounts, &hosts),
            configured: authors.iter().map(|a| a.did().to_string()).collect(),
            fallback: None,
            plc_endpoint: None,
            app_signed_authors: Vec::new(),
            extra_env: BTreeMap::new(),
            loopback_seam: true,
        }
    }

    /// GIVEN a crowd of `authors` authors spread round-robin over `hosts` PDS
    /// hosts, each with one self-attested claim on their own PDS. Returns the
    /// world and, per host label, the DIDs it serves.
    pub fn crowd(authors: usize, hosts: usize) -> (Self, BTreeMap<String, Vec<String>>) {
        let host_labels: Vec<String> = (0..hosts).map(|h| format!("crowd-pds-{h:02}")).collect();
        let members: Vec<(String, String)> = (0..authors)
            .map(|i| {
                (
                    format!("did:plc:crowdmember{i:04}"),
                    host_labels[i % hosts].clone(),
                )
            })
            .collect();
        let accounts = members
            .iter()
            .map(|(did, host)| BlueskyAccount::new(&format!("{did}.test"), did, host))
            .collect();
        let world = Self {
            env: maria_home(),
            net: FakeAtprotoNetwork::start(accounts),
            configured: members.iter().map(|(did, _)| did.clone()).collect(),
            fallback: None,
            plc_endpoint: None,
            app_signed_authors: Vec::new(),
            extra_env: BTreeMap::new(),
            loopback_seam: true,
        };
        let mut by_host: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (i, (did, host)) in members.iter().enumerate() {
            let claim = Claim {
                subject: "github:crowd/commons",
                philosophy: "reproducible-builds",
                basis_points: 5000 + i as i64,
            };
            world.publish_self_attested_value(did, self_attested_value(did, claim), None);
            by_host.entry(host.clone()).or_default().push(did.clone());
        }
        (world, by_host)
    }

    /// GIVEN these extra authors (bare DID → home host) besides `authors`; the
    /// operator lists all of them. Used where a scenario needs DIDs the
    /// [`Author`] cast does not have (e.g. two DIDs where one begins the other).
    pub fn configured_with_members(authors: &[Author], members: &[(&str, Host)]) -> Self {
        let mut accounts: Vec<BlueskyAccount> = authors
            .iter()
            .filter_map(|a| {
                a.home()
                    .map(|h| BlueskyAccount::new(a.handle(), a.did(), h.label()))
            })
            .collect();
        accounts.extend(members.iter().map(|(did, host)| {
            BlueskyAccount::new(
                &format!("{}.test", did.replace(':', "-")),
                did,
                host.label(),
            )
        }));
        let hosts: Vec<&str> = Host::ALL.iter().map(|h| h.label()).collect();
        let mut configured: Vec<String> = authors.iter().map(|a| a.did().to_string()).collect();
        configured.extend(members.iter().map(|(did, _)| did.to_string()));
        Self {
            env: maria_home(),
            net: FakeAtprotoNetwork::start_with_extra_hosts(accounts, &hosts),
            configured,
            fallback: None,
            plc_endpoint: None,
            app_signed_authors: Vec::new(),
            extra_env: BTreeMap::new(),
            loopback_seam: true,
        }
    }

    /// GIVEN the DID `did` published these self-attested claims on its PDS.
    pub fn did_publishes_self_attested(&self, did: &str, claims: &[Claim]) -> Vec<String> {
        claims
            .iter()
            .map(|c| self.publish_self_attested_value(did, self_attested_value(did, *c), None))
            .collect()
    }

    /// The repo DIDs the operator configured (in order).
    pub fn configured_dids(&self) -> Vec<String> {
        self.configured.clone()
    }

    // ---------------------------------------------------------------- records

    fn publish_self_attested_value(
        &self,
        repo: &str,
        value: serde_json::Value,
        rkey: Option<&str>,
    ) -> String {
        let key = rkey
            .map(str::to_string)
            .unwrap_or_else(|| recomputed_cid(&value));
        self.net.seed_record(repo, CLAIM_COLLECTION, &key, value);
        key
    }

    /// GIVEN `author` published these self-attested claims on their own PDS.
    /// Returns the record keys (= CIDs).
    pub fn publishes_self_attested(&self, author: Author, claims: &[Claim]) -> Vec<String> {
        claims
            .iter()
            .map(|c| {
                self.publish_self_attested_value(
                    author.did(),
                    self_attested_value(author.did(), *c),
                    None,
                )
            })
            .collect()
    }

    /// GIVEN `author` published these app-signed claims (signed with the
    /// author's application key). Returns the record keys (= CIDs).
    pub fn publishes_app_signed(&mut self, author: Author, claims: &[Claim]) -> Vec<String> {
        if !self.app_signed_authors.iter().any(|d| d == author.did()) {
            self.app_signed_authors.push(author.did().to_string());
        }
        claims
            .iter()
            .map(|c| {
                let (rkey, value) = app_signed_record(author.did(), *c);
                self.net
                    .seed_record(author.did(), CLAIM_COLLECTION, &rkey, value);
                rkey
            })
            .collect()
    }

    /// GIVEN one of `author`'s self-attested records is stored under a key that
    /// is not its content's CID (tampered).
    pub fn publishes_self_attested_under_a_wrong_key(
        &self,
        author: Author,
        claim: Claim,
    ) -> String {
        self.publish_self_attested_value(
            author.did(),
            self_attested_value(author.did(), claim),
            Some("bafyreitamperedkeythatisnotthecontentcid0000000000000"),
        )
    }

    // -------------------------------------------------------- network postures

    /// GIVEN `host` answers listings this way.
    pub fn host_answers(&self, host: Host, posture: ListingPosture) {
        self.net.set_listing_posture(host.label(), posture);
    }

    /// GIVEN `host` drops every connection (or accepts them again).
    pub fn host_is_reachable(&self, host: Host, reachable: bool) {
        self.net.set_host_reachable(host.label(), reachable);
    }

    /// GIVEN the directory answers `author`'s DID-document request this way.
    pub fn did_document_of(&self, author: Author, posture: DidDocPosture) {
        self.net.set_did_doc_posture(author.did(), posture);
    }

    /// GIVEN `author` migrated: their DID document names `host` from now on.
    pub fn author_moves_to(&self, author: Author, host: Host) {
        self.net.move_account(author.did(), host.label());
    }

    /// GIVEN the PDS asked for `requested`'s repo answers with `served`'s records.
    pub fn repo_answers_with_records_of(&self, requested: Author, served: Author) {
        self.net.serve_repo_as(requested.did(), served.did());
    }

    /// The base URL of `host` (what a DID document names).
    pub fn url_of(&self, host: Host) -> String {
        self.net.host_url(host.label())
    }

    // ------------------------------------------------------------ operator config

    /// GIVEN the operator configured `host` as the fallback source.
    pub fn fallback_is(&mut self, host: Host) {
        self.fallback = Some(self.url_of(host));
    }

    /// GIVEN the operator set the fallback source to this exact text.
    pub fn fallback_text_is(&mut self, text: &str) {
        self.fallback = Some(text.to_string());
    }

    /// GIVEN the operator listed exactly this text as the repo DIDs.
    pub fn repo_dids_text_is(&mut self, text: &str) {
        self.configured = vec![text.to_string()];
    }

    /// GIVEN the indexer resolves DIDs through this directory URL.
    pub fn directory_url_is(&mut self, url: &str) {
        self.plc_endpoint = Some(url.to_string());
    }

    /// GIVEN the index cannot store `author`'s claims: a regular file sits where
    /// that author's artifact directory (`indexed_claims/<did fs segment>`, next
    /// to the index DB) belongs, under both the bare DID and the application
    /// identity, so the store write for that author fails.
    pub fn index_cannot_store_claims_of(&self, author: Author) {
        let artifacts_root = support::index_duckdb_path(&self.env)
            .parent()
            .expect("the index DB has a parent directory")
            .join("indexed_claims");
        std::fs::create_dir_all(&artifacts_root).expect("create indexed_claims");
        for identity in [author.did().to_string(), author.app_identity()] {
            std::fs::write(
                artifacts_root.join(identity.replace(':', "_")),
                b"not a dir",
            )
            .expect("place a regular file at the author's artifact directory");
        }
    }

    /// GIVEN the operator set `variable` to `value`.
    pub fn setting(&mut self, variable: &str, value: &str) {
        self.extra_env
            .insert(variable.to_string(), value.to_string());
    }

    /// GIVEN the loopback test seam is NOT set (production transport policy).
    pub fn without_loopback_seam(&mut self) {
        self.loopback_seam = false;
    }

    /// The environment of one indexer run, exactly as the operator configured it
    /// (`pub` since indexer-deployment: the long-running `serve` harness in
    /// `support/indexer_live.rs` starts from it).
    pub fn indexer_env(&self) -> Vec<(String, String)> {
        let mut env = vec![
            (
                "OPENLORE_HOME".to_string(),
                self.env.home.display().to_string(),
            ),
            (
                "OPENLORE_INDEXER_INDEX_PATH".to_string(),
                support::index_duckdb_path(&self.env).display().to_string(),
            ),
            (var::REPO_DIDS.to_string(), self.configured.join(",")),
            (
                var::PLC.to_string(),
                self.plc_endpoint
                    .clone()
                    .unwrap_or_else(|| self.net.directory_url().to_string()),
            ),
            (
                "PATH".to_string(),
                std::env::var("PATH").unwrap_or_default(),
            ),
        ];
        if let Some(fallback) = &self.fallback {
            env.push((var::FALLBACK.to_string(), fallback.clone()));
        }
        if self.loopback_seam {
            env.push((var::LOOPBACK_SEAM.to_string(), "1".to_string()));
        }
        for did in &self.app_signed_authors {
            let key = FixtureKeypair::for_did(did);
            env.push((
                support::peer_pubkey_env_var(did),
                support::hex_lower(&key.verifying_key.0),
            ));
        }
        env.extend(self.extra_env.clone());
        env
    }

    // ------------------------------------------------------------------ When

    /// WHEN one ingest pass runs (`openlore-indexer ingest`, the real binary).
    pub fn one_ingest_pass_runs(&self) -> PassReport {
        self.run_indexer(&["ingest"])
    }

    /// WHEN Jeff starts the indexer (same entry point; a refusal happens before
    /// any pass work).
    pub fn jeff_starts_the_indexer(&self) -> PassReport {
        self.run_indexer(&["ingest"])
    }

    fn run_indexer(&self, args: &[&str]) -> PassReport {
        let bin = support::resolve_workspace_bin("openlore-indexer");
        let mut cmd = Command::new(&bin);
        cmd.args(args).env_clear();
        for (k, v) in self.indexer_env() {
            cmd.env(k, v);
        }
        run_bounded(cmd, RUN_CEILING)
    }

    /// WHEN Maria runs `openlore search <args>` against this world's index
    /// (a REAL `openlore-indexer serve` over the same `index.duckdb`).
    pub fn maria_searches(&self, args: &[&str]) -> CliOutcome {
        let server = self.serve_index();
        search_against(&self.env, args, &server.url)
    }

    /// Start `openlore-indexer serve` over this world's index, with the same
    /// configuration as the passes.
    pub fn serve_index(&self) -> ServingIndexer {
        let bin = support::resolve_workspace_bin("openlore-indexer");
        let mut cmd = Command::new(&bin);
        cmd.arg("serve").env_clear();
        for (k, v) in self.indexer_env() {
            cmd.env(k, v);
        }
        cmd.env("OPENLORE_INDEXER_LISTEN_ADDR", "127.0.0.1:0");
        ServingIndexer::spawn(cmd)
    }

    // ------------------------------------------------------------------ Then

    /// Every row in the indexer's `index.duckdb` (empty when no index exists).
    pub fn indexed_rows(&self) -> Vec<IndexedRow> {
        read_index(&support::index_duckdb_path(&self.env))
    }

    /// The rows attributed to `author` (bare DID for self-attested rows, the
    /// application identity for app-signed rows).
    pub fn rows_of(&self, author: Author) -> Vec<IndexedRow> {
        let app = author.app_identity();
        self.indexed_rows()
            .into_iter()
            .filter(|r| r.author_did == author.did() || r.author_did == app)
            .collect()
    }

    /// Whether the index file exists at all.
    pub fn index_exists(&self) -> bool {
        support::index_duckdb_path(&self.env).exists()
    }

    /// The repos `host` was asked to list.
    pub fn listings_on(&self, host: Host) -> Vec<String> {
        self.net.listings_on(host.label())
    }
}

// =============================================================================
// Observables
// =============================================================================

/// One `indexed_claims` row (port-exposed projection of the index).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedRow {
    pub author_did: String,
    pub cid: String,
    pub subject: String,
    pub object: String,
    pub provenance: String,
}

fn read_index(path: &PathBuf) -> Vec<IndexedRow> {
    if !path.exists() {
        return Vec::new();
    }
    let conn = duckdb::Connection::open(path).expect("open index.duckdb");
    let mut stmt = match conn.prepare(
        "SELECT author_did, cid, subject, object, provenance FROM indexed_claims \
         ORDER BY author_did, cid",
    ) {
        Ok(stmt) => stmt,
        Err(_) => return Vec::new(),
    };
    stmt.query_map([], |r| {
        Ok(IndexedRow {
            author_did: r.get(0)?,
            cid: r.get(1)?,
            subject: r.get(2)?,
            object: r.get(3)?,
            provenance: r.get(4)?,
        })
    })
    .expect("query indexed_claims")
    .filter_map(Result::ok)
    .collect()
}

/// What one `openlore-indexer` run showed the operator.
#[derive(Debug, Clone)]
pub struct PassReport {
    pub status: i32,
    pub stdout: String,
    pub stderr: String,
    pub elapsed: Duration,
}

fn json_lines(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok())
        .filter(|v| v.get("event").is_some())
        .collect()
}

impl PassReport {
    /// Every structured event (stdout and stderr), in order.
    pub fn events(&self) -> Vec<serde_json::Value> {
        let mut all = json_lines(&self.stdout);
        all.extend(json_lines(&self.stderr));
        all
    }

    pub fn events_named(&self, name: &str) -> Vec<serde_json::Value> {
        self.events()
            .into_iter()
            .filter(|e| e["event"] == name)
            .collect()
    }

    /// The `indexer.ingest.source_skipped` events for `did`.
    pub fn skips_of(&self, did: &str) -> Vec<serde_json::Value> {
        self.events_named("indexer.ingest.source_skipped")
            .into_iter()
            .filter(|e| e["did"] == did)
            .collect()
    }

    /// The `indexer.ingest.source_fallback` events for `did`.
    pub fn fallback_reads_of(&self, did: &str) -> Vec<serde_json::Value> {
        self.events_named("indexer.ingest.source_fallback")
            .into_iter()
            .filter(|e| e["did"] == did)
            .collect()
    }

    /// The one `indexer.ingest.pass_summary` event.
    pub fn summary(&self) -> serde_json::Value {
        let all = self.events_named("indexer.ingest.pass_summary");
        assert_eq!(
            all.len(),
            1,
            "MISSING_FUNCTIONALITY: exactly one pass_summary event expected; \
             exit {} stdout:\n{}\nstderr:\n{}",
            self.status,
            self.stdout,
            self.stderr
        );
        all[0].clone()
    }

    /// `indexer.ingest.verified.count`.
    pub fn verified_count(&self) -> u64 {
        self.events_named("indexer.ingest.verified")
            .first()
            .and_then(|e| e["count"].as_u64())
            .unwrap_or_default()
    }

    /// `indexer.ingest.rejected.by_reason.<reason>`.
    pub fn refused_for(&self, reason: RefusalReason) -> u64 {
        self.events_named("indexer.ingest.rejected")
            .first()
            .and_then(|e| e["by_reason"][reason.token()].as_u64())
            .unwrap_or_default()
    }

    /// `indexer.ingest.rejected.count`.
    pub fn refused_total(&self) -> u64 {
        self.events_named("indexer.ingest.rejected")
            .first()
            .and_then(|e| e["count"].as_u64())
            .unwrap_or_default()
    }

    /// The startup refusal (`health.startup.refused`), if the indexer refused.
    pub fn startup_refusal(&self) -> Option<serde_json::Value> {
        self.events_named("health.startup.refused")
            .into_iter()
            .next()
    }

    /// `indexer.config.loaded`, if the configuration was accepted.
    pub fn config_loaded(&self) -> Option<serde_json::Value> {
        self.events_named("indexer.config.loaded")
            .into_iter()
            .next()
    }

    /// Operator-readable text of the run (both streams).
    pub fn text(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }

    /// Diagnostic dump for assertion messages.
    pub fn dump(&self) -> String {
        format!(
            "exit {} after {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.status, self.elapsed, self.stdout, self.stderr
        )
    }

    /// THEN the pass completed with `code` and its summary accounts for every
    /// configured DID: own_pds + fallback + skipped == configured (AC-002.7).
    pub fn assert_pass_completed(&self, code: i32, own_pds: u64, fallback: u64, skipped: u64) {
        let summary = self.summary();
        assert_eq!(self.status, code, "pass exit code\n{}", self.dump());
        assert_eq!(
            summary["own_pds"].as_u64(),
            Some(own_pds),
            "own_pds\n{}",
            self.dump()
        );
        assert_eq!(
            summary["fallback"].as_u64(),
            Some(fallback),
            "fallback\n{}",
            self.dump()
        );
        assert_eq!(
            summary["skipped"].as_u64(),
            Some(skipped),
            "skipped\n{}",
            self.dump()
        );
        assert_eq!(
            summary["configured"].as_u64(),
            Some(own_pds + fallback + skipped),
            "own_pds + fallback + skipped == configured\n{}",
            self.dump()
        );
        assert_eq!(
            summary["exit_code"].as_i64(),
            Some(code as i64),
            "{}",
            self.dump()
        );
    }

    /// THEN `did` was skipped exactly once, for `reason`, without the fallback.
    pub fn assert_skipped(&self, did: &str, reason: SkipReason) -> serde_json::Value {
        let skips = self.skips_of(did);
        assert_eq!(
            skips.len(),
            1,
            "exactly one source_skipped event for {did}\n{}",
            self.dump()
        );
        let skip = skips[0].clone();
        assert_eq!(
            skip["reason"],
            reason.token(),
            "skip reason for {did}\n{}",
            self.dump()
        );
        assert_skip_event_carries_no_claim_content(&skip);
        skip
    }

    /// THEN `did` produced no skip and no fallback read (it was read from its own PDS).
    pub fn assert_read_from_own_pds(&self, did: &str) {
        assert!(
            self.skips_of(did).is_empty(),
            "{did} must not be skipped\n{}",
            self.dump()
        );
        assert!(
            self.fallback_reads_of(did).is_empty(),
            "{did} must not be read through the fallback\n{}",
            self.dump()
        );
    }
}

/// The fields a `source_skipped` event may carry (data-models.md §5, WD-105).
const SKIP_EVENT_FIELDS: [&str; 7] = [
    "event",
    "did",
    "reason",
    "fallback_used",
    "pds_url",
    "fallback_failure",
    "detail",
];

/// AC-002.3: a skip event carries DIDs, URLs, reasons and a short detail — never claim content.
pub fn assert_skip_event_carries_no_claim_content(skip: &serde_json::Value) {
    let object = skip.as_object().expect("skip event is a JSON object");
    for key in object.keys() {
        assert!(
            SKIP_EVENT_FIELDS.contains(&key.as_str()),
            "unexpected field `{key}` in a skip event: {skip}"
        );
    }
    if let Some(detail) = skip["detail"].as_str() {
        assert!(
            detail.chars().count() <= 200,
            "detail is at most 200 chars: {skip}"
        );
        for fragment in ["org.openlore.philosophy", "github:", "\"records\""] {
            assert!(
                !detail.contains(fragment),
                "detail leaks claim content: {skip}"
            );
        }
    }
}

/// Run a command, killing it after `ceiling` (reported as exit -1 with a note).
pub fn run_bounded(mut cmd: Command, ceiling: Duration) -> PassReport {
    let started = Instant::now();
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {cmd:?}: {e}"));
    let mut out = child.stdout.take().expect("stdout pipe");
    let mut err = child.stderr.take().expect("stderr pipe");
    let out_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let err_reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = err.read_to_string(&mut s);
        s
    });
    let mut timed_out = false;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code().unwrap_or(-1),
            Ok(None) if started.elapsed() > ceiling => {
                let _ = child.kill();
                let _ = child.wait();
                timed_out = true;
                break -1;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(e) => panic!("wait for indexer: {e}"),
        }
    };
    let elapsed = started.elapsed();
    let stdout = out_reader.join().unwrap_or_default();
    let mut stderr = err_reader.join().unwrap_or_default();
    if timed_out {
        stderr.push_str(&format!(
            "\n[harness] killed after {ceiling:?}: the run never finished\n"
        ));
    }
    PassReport {
        status,
        stdout,
        stderr,
        elapsed,
    }
}

// =============================================================================
// Serving + searching
// =============================================================================

/// A running `openlore-indexer serve` (killed on drop).
pub struct ServingIndexer {
    pub url: String,
    child: Child,
}

impl ServingIndexer {
    fn spawn(mut cmd: Command) -> Self {
        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap_or_else(|e| panic!("spawn openlore-indexer serve: {e}"));
        let stdout = child.stdout.take().expect("serve stdout");
        let mut reader = BufReader::new(stdout);
        let mut addr = None;
        for _ in 0..50 {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if let Ok(event) = serde_json::from_str::<serde_json::Value>(line.trim()) {
                        if event["event"] == "indexer.serve.listening" {
                            addr = event["addr"].as_str().map(str::to_string);
                            break;
                        }
                    }
                }
            }
        }
        let Some(addr) = addr else {
            let _ = child.kill();
            let mut err = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                let _ = stderr.read_to_string(&mut err);
            }
            panic!(
                "MISSING_FUNCTIONALITY: the indexer did not start serving this configuration \
                 (no indexer.serve.listening); stderr:\n{err}"
            );
        };
        Self {
            url: format!("http://{addr}"),
            child,
        }
    }
}

impl Drop for ServingIndexer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Run `openlore search <args>` as Maria (the real CLI) against `indexer_url`.
pub fn search_against(env: &TestEnv, args: &[&str], indexer_url: &str) -> CliOutcome {
    let bin = support::resolve_workspace_bin("openlore");
    let mut full = vec!["search"];
    full.extend_from_slice(args);
    let output = Command::new(&bin)
        .args(&full)
        .env_clear()
        .env("OPENLORE_HOME", &env.home)
        .env("OPENLORE_DID", env.identity.author_did())
        .env("OPENLORE_KEY_SEED_HEX", &env.identity.seed_hex)
        .env("OPENLORE_PDS_ENDPOINT", env.pds.endpoint_url())
        .env("OPENLORE_INDEXER_URL", indexer_url)
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .stdin(Stdio::null())
        .output()
        .unwrap_or_else(|e| panic!("spawn openlore search at {bin:?}: {e}"));
    CliOutcome {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// One result row as `openlore search` prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchRow {
    pub author_did: String,
    pub subject: String,
    pub object: String,
    pub cid: String,
    /// The marker line (`[verified]`, optionally followed by `[self-attested]`).
    pub markers: String,
    /// Every line of the row block, verbatim (for byte-for-byte comparisons).
    pub block: Vec<String>,
}

/// Parse the attributed rows out of `openlore search` output.
pub fn search_rows(stdout: &str) -> Vec<SearchRow> {
    let mut rows: Vec<SearchRow> = Vec::new();
    for line in stdout.lines() {
        let trimmed = line.trim();
        if let Some(did) = trimmed.strip_prefix("author_did:") {
            rows.push(SearchRow {
                author_did: did.trim().to_string(),
                subject: String::new(),
                object: String::new(),
                cid: String::new(),
                markers: String::new(),
                block: vec![line.to_string()],
            });
            continue;
        }
        let Some(row) = rows.last_mut() else { continue };
        if trimmed.is_empty() || trimmed.starts_with("author:") || !line.starts_with("    ") {
            continue;
        }
        row.block.push(line.to_string());
        if let Some(v) = trimmed.strip_prefix("subject:") {
            row.subject = v.trim().to_string();
        } else if let Some(v) = trimmed.strip_prefix("object:") {
            row.object = v.trim().to_string();
        } else if let Some(v) = trimmed.strip_prefix("cid:") {
            row.cid = v.trim().to_string();
        } else if trimmed.starts_with("[verified]") {
            row.markers = trimmed.to_string();
        }
    }
    rows
}

// =============================================================================
// Tripwire + canned indexer (things that must NOT be contacted / old servers)
// =============================================================================

/// A loopback port that counts every connection it accepts — proof that a
/// refused address was never contacted.
pub struct Tripwire {
    port: u16,
    accepted: Arc<AtomicUsize>,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl Tripwire {
    pub fn arm() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("tripwire bind");
        listener
            .set_nonblocking(true)
            .expect("tripwire nonblocking");
        let port = listener.local_addr().expect("tripwire addr").port();
        let accepted = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let (a, s) = (accepted.clone(), stop.clone());
        let join = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok(_) => {
                        a.fetch_add(1, Ordering::SeqCst);
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self {
            port,
            accepted,
            stop,
            join: Some(join),
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// `https://localhost:<port>` — a hostname that resolves only to loopback.
    pub fn https_localhost_url(&self) -> String {
        format!("https://localhost:{}", self.port)
    }

    pub fn connections(&self) -> usize {
        self.accepted.load(Ordering::SeqCst)
    }
}

impl Drop for Tripwire {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// A stand-in for an indexer that answers every search with a fixed body —
/// an OLD server (no `provenance` field) or one sending an unknown token.
pub struct CannedIndexer {
    pub url: String,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl CannedIndexer {
    pub fn answering(body: serde_json::Value) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("canned indexer bind");
        listener.set_nonblocking(true).expect("nonblocking");
        let url = format!("http://{}", listener.local_addr().expect("addr"));
        let body = body.to_string();
        let stop = Arc::new(AtomicBool::new(false));
        let s = stop.clone();
        let join = std::thread::spawn(move || {
            while !s.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let _ = stream.set_nonblocking(false);
                        drain_http_request(&mut stream);
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                            body.len(),
                            body
                        );
                        let _ = stream.write_all(response.as_bytes());
                        let _ = stream.flush();
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        Self {
            url,
            stop,
            join: Some(join),
        }
    }
}

impl Drop for CannedIndexer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

/// Read one HTTP request (head + `Content-Length` body) so the client never
/// sees a reset.
fn drain_http_request(stream: &mut std::net::TcpStream) {
    let mut acc: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 2048];
    let mut head_end = None;
    for _ in 0..64 {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => acc.extend_from_slice(&chunk[..n]),
        }
        if head_end.is_none() {
            head_end = acc.windows(4).position(|w| w == b"\r\n\r\n").map(|p| p + 4);
        }
        if let Some(end) = head_end {
            let head = String::from_utf8_lossy(&acc[..end]).to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|l| l.strip_prefix("content-length:"))
                .and_then(|v| v.trim().parse::<usize>().ok())
                .unwrap_or(0);
            if acc.len() >= end + length {
                return;
            }
        }
    }
}

/// One search result row on the wire, as an indexer would send it
/// (`provenance` is set only when given — `None` models an old server).
pub fn wire_row(
    author_did: &str,
    claim: Claim,
    cid: &str,
    provenance: Option<&str>,
) -> serde_json::Value {
    let mut row = serde_json::json!({
        "author_did": author_did,
        "cid": cid,
        "subject": claim.subject,
        "predicate": "embodiesPhilosophy",
        "object": claim.object(),
        "confidence": claim.basis_points as f64 / 10_000.0,
        "composed_at": "2026-10-04T15:02:11+00:00",
        "verified_against": author_did,
        "evidence": [],
    });
    if let Some(p) = provenance {
        row["provenance"] = serde_json::json!(p);
    }
    row
}

/// A searchClaims response body carrying `rows`.
pub fn wire_response(rows: Vec<serde_json::Value>) -> serde_json::Value {
    let authors: std::collections::BTreeSet<String> = rows
        .iter()
        .filter_map(|r| r["author_did"].as_str().map(str::to_string))
        .collect();
    serde_json::json!({
        "results": rows.clone(),
        "distinct_author_count": authors.len(),
        "total_claims": rows.len(),
    })
}
