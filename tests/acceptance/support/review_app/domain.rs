//! Domain vocabulary for the review-app acceptance suites (Mandate-12 SSOT).
//!
//! Every persona, philosophy and suggestion named in a scenario is a typed
//! value here; scenario bodies never spell a DID, handle or object id inline.
//! Personas and values are the DISCUSS `requirements.md` personas verbatim.

/// The people in the scenarios (requirements.md § Personas).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Persona {
    /// P-003 primary: Rust developer on bsky.social.
    Priya,
    /// P-003 edge: custom-domain handle, self-hosted PDS.
    Dmitri,
    /// P-003 edge: GitHub account with only forks.
    Aisha,
    /// Adversarial: tries to claim someone else's GitHub account.
    Sam,
}

impl Persona {
    pub const ALL: [Persona; 4] = [
        Persona::Priya,
        Persona::Dmitri,
        Persona::Aisha,
        Persona::Sam,
    ];

    pub fn handle(self) -> &'static str {
        match self {
            Persona::Priya => "priyaraman.bsky.social",
            Persona::Dmitri => "dmitri.volkov.dev",
            Persona::Aisha => "aishabello.bsky.social",
            Persona::Sam => "samortega.bsky.social",
        }
    }

    /// `@handle` as the app displays it.
    pub fn at_handle(self) -> String {
        format!("@{}", self.handle())
    }

    pub fn did(self) -> &'static str {
        match self {
            Persona::Priya => "did:plc:7x3kq2mzv5rj4w6hbn2tqclp",
            Persona::Dmitri => "did:plc:d3mv4lk8x2qp7ty5nw9rbcz6",
            Persona::Aisha => "did:plc:a1shab3ll0x7q2w9e4r6t8yu",
            Persona::Sam => "did:plc:q9rt5wz2b8kd3m1xv7pn4ahe",
        }
    }

    pub fn github_login(self) -> &'static str {
        match self {
            Persona::Priya => "priyaraman",
            Persona::Dmitri => "dvolkov",
            Persona::Aisha => "aishab",
            Persona::Sam => "samortega",
        }
    }

    /// The fake PDS host label serving this persona's repo.
    pub fn pds_host(self) -> &'static str {
        match self {
            Persona::Dmitri => PdsHost::VOLKOV,
            _ => PdsHost::BSKY,
        }
    }
}

/// PDS host labels in the fake ATProto network.
pub struct PdsHost;

impl PdsHost {
    pub const BSKY: &'static str = "bsky-social";
    pub const VOLKOV: &'static str = "volkov-dev";
}

/// Dmitri's OLD DID, still in his GitHub bio (US-BRA-002 Ex 2).
pub const DMITRI_OLD_DID: &str = "did:plc:ab12cd34ef56gh78ij90klmn";

/// A GitHub account nobody in the scenarios owns (Sam tries to claim it).
pub const BURNTSUSHI: &str = "BurntSushi";

/// The handle Priya mistypes (US-BRA-001 Ex 3).
pub const MISTYPED_HANDLE: &str = "priyaramen.bsky.social";

/// The server GitHub token the app is configured with (a fixture value; it
/// must never appear in logs or pages).
pub const SERVER_GITHUB_TOKEN: &str = "github_pat_reviewappfixture_0000000000000000000000";

/// The philosophy vocabulary the shipped signal mapping produces (J-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Philosophy {
    DependencyPinning,
    MemorySafety,
    TestDriven,
    SemanticVersioning,
    DocumentationFirst,
}

impl Philosophy {
    pub const ALL: [Philosophy; 5] = [
        Philosophy::DependencyPinning,
        Philosophy::MemorySafety,
        Philosophy::TestDriven,
        Philosophy::SemanticVersioning,
        Philosophy::DocumentationFirst,
    ];

    pub fn slug(self) -> &'static str {
        match self {
            Philosophy::DependencyPinning => "dependency-pinning",
            Philosophy::MemorySafety => "memory-safety",
            Philosophy::TestDriven => "test-driven",
            Philosophy::SemanticVersioning => "semantic-versioning",
            Philosophy::DocumentationFirst => "documentation-first",
        }
    }

    /// The claim `object` written to the PDS.
    pub fn object(self) -> String {
        format!("org.openlore.philosophy.{}", self.slug())
    }
}

/// The predicate every v1 suggestion carries (D-10).
pub const EMBODIES: &str = "embodiesPhilosophy";

/// A suggestion as the owner sees it: "<login>/<repo> embodies <philosophy>".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Suggestion {
    pub login: &'static str,
    pub repo: &'static str,
    pub philosophy: Philosophy,
}

impl Suggestion {
    pub const fn new(login: &'static str, repo: &'static str, philosophy: Philosophy) -> Self {
        Self {
            login,
            repo,
            philosophy,
        }
    }

    /// `priyaraman/tidepool`
    pub fn repo_path(&self) -> String {
        format!("{}/{}", self.login, self.repo)
    }

    /// The claim subject: `github:priyaraman/tidepool` (D-10).
    pub fn subject(&self) -> String {
        format!("github:{}", self.repo_path())
    }

    /// "priyaraman/tidepool embodies dependency-pinning" — the card phrase.
    pub fn phrase(&self) -> String {
        format!("{} embodies {}", self.repo_path(), self.philosophy.slug())
    }

    /// The fragments that identify this suggestion's card on a page.
    pub fn card_parts(&self) -> [String; 2] {
        [self.repo_path(), self.philosophy.slug().to_string()]
    }
}

/// Priya's canonical suggestions (US-BRA-003 Ex 1: tidepool 4 signals,
/// quill-docs 1).
pub mod priya {
    use super::{Philosophy, Suggestion};

    pub const TIDEPOOL_DEPENDENCY_PINNING: Suggestion =
        Suggestion::new("priyaraman", "tidepool", Philosophy::DependencyPinning);
    pub const TIDEPOOL_MEMORY_SAFETY: Suggestion =
        Suggestion::new("priyaraman", "tidepool", Philosophy::MemorySafety);
    pub const TIDEPOOL_TEST_DRIVEN: Suggestion =
        Suggestion::new("priyaraman", "tidepool", Philosophy::TestDriven);
    pub const TIDEPOOL_SEMANTIC_VERSIONING: Suggestion =
        Suggestion::new("priyaraman", "tidepool", Philosophy::SemanticVersioning);
    pub const QUILL_DOCS_DOCUMENTATION_FIRST: Suggestion =
        Suggestion::new("priyaraman", "quill-docs", Philosophy::DocumentationFirst);
    /// Appears only after she adds a CHANGELOG + semver tags to `estuary` (US-BRA-010).
    pub const ESTUARY_SEMANTIC_VERSIONING: Suggestion =
        Suggestion::new("priyaraman", "estuary", Philosophy::SemanticVersioning);

    /// All five first-scan suggestions.
    pub const FIRST_SCAN: [Suggestion; 5] = [
        TIDEPOOL_DEPENDENCY_PINNING,
        TIDEPOOL_MEMORY_SAFETY,
        TIDEPOOL_TEST_DRIVEN,
        TIDEPOOL_SEMANTIC_VERSIONING,
        QUILL_DOCS_DOCUMENTATION_FIRST,
    ];
}

/// Dmitri's suggestion from his `ferrite` crate (Cargo.lock committed).
pub mod dmitri {
    use super::{Philosophy, Suggestion};

    pub const FERRITE_DEPENDENCY_PINNING: Suggestion =
        Suggestion::new("dvolkov", "ferrite", Philosophy::DependencyPinning);
}

/// Confidence as typed in the edit field and as stored (basis points, BR-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfidenceEntry {
    pub typed: &'static str,
    pub basis_points: i64,
    pub bucket: &'static str,
}

impl ConfidenceEntry {
    pub const fn new(typed: &'static str, basis_points: i64, bucket: &'static str) -> Self {
        Self {
            typed,
            basis_points,
            bucket,
        }
    }
}

/// The default suggestion confidence (D-8, SPIKE finding 5).
pub const DEFAULT_CONFIDENCE: ConfidenceEntry = ConfidenceEntry::new("0.25", 2500, "speculative");
