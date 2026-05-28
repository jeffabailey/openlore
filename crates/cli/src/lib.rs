//! `cli` — composition root + verb dispatch.
//!
//! Wires concrete adapters into the pure core. Hosts the two-prompt
//! interactive flow (ADR-003) and the `init | claim add | claim publish
//! <cid> | claim retract <cid> | graph query --subject <uri>` verbs.
//!
//! The `openlore` binary at `src/main.rs` delegates here.
//!
//! Slice-01 status: `init` is implemented (step 05-01). All other verbs
//! still panic in the RED scaffold; subsequent phase-05 steps fill them
//! in one acceptance scenario at a time.

#![allow(dead_code)]
#![forbid(unsafe_code)]

use clap::{Parser, Subcommand};

pub mod errors;
pub mod io;
pub mod orientation;
pub mod paths;
pub mod render;
pub mod verbs;
pub mod wiring;

use paths::OpenLorePaths;
use wiring::Wiring;

/// Top-level CLI surface (ADR-003 locked verb contract).
#[derive(Debug, Parser)]
#[command(
    name = "openlore",
    version,
    about = "OpenLore — sign and publish philosophical claims"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Resolve identity, create DuckDB + identity config (idempotent).
    Init {
        #[arg(long)]
        handle: String,
        #[arg(long = "app-password")]
        app_password: String,
    },
    /// Claim operations (add / publish / retract / counter).
    #[command(subcommand)]
    Claim(ClaimCommand),
    /// Graph operations (query).
    #[command(subcommand)]
    Graph(GraphCommand),
    /// Peer subscription operations (add / pull / remove) — slice-03.
    #[command(subcommand)]
    Peer(PeerCommand),
    /// Scrape a public source for candidate claims — slice-02 (ADR-017).
    #[command(subcommand)]
    Scrape(ScrapeCommand),
}

#[derive(Debug, Subcommand)]
pub enum ClaimCommand {
    /// Compose, preview, sign, optionally publish a new claim.
    Add {
        #[arg(long)]
        subject: String,
        #[arg(long)]
        predicate: String,
        #[arg(long)]
        object: String,
        #[arg(long)]
        evidence: Vec<String>,
        #[arg(long)]
        confidence: f64,
    },
    /// Publish a previously-signed claim by its CID.
    Publish { cid: String },
    /// Retract a published claim by counter-claim referencing original CID.
    Retract { cid: String },
    /// Author a counter-claim against a target CID (slice-03; ADR-015).
    /// `--reason` is REQUIRED at the CLI level (WD-20).
    Counter {
        /// CID of the claim being countered (own or peer's).
        cid: String,
        /// Mandatory free-text explanation for the counter-claim.
        #[arg(long)]
        reason: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum GraphCommand {
    /// Query the local store (and slice-03+ federated peers).
    Query {
        #[arg(long)]
        subject: String,
        /// `--federated` (slice-03): include subscribed peers' claims in
        /// the result. Defaults to local-only (the slice-01 behavior).
        #[arg(long)]
        federated: bool,
    },
}

/// Peer subscription verbs (slice-03; US-FED-001 / US-FED-002 / US-FED-005).
#[derive(Debug, Subcommand)]
pub enum PeerCommand {
    /// Subscribe to a peer's claim stream by DID.
    Add {
        /// The peer DID to subscribe to (e.g. `did:plc:rachel-test`).
        did: String,
    },
    /// Pull + verify + cache claims from every subscribed peer.
    Pull,
    /// Unsubscribe from a peer. `--purge` additionally hard-deletes the
    /// cached peer claims (gated by interactive confirmation).
    Remove {
        /// The peer DID to unsubscribe from.
        did: String,
        /// Hard-delete cached peer claims (WD-21: no `--yes`; the prompt
        /// is interactive). Defaults to soft-remove (cache retained).
        #[arg(long)]
        purge: bool,
        /// Scripting mode — no interactive terminal. WD-36 LOCK: combined
        /// with `--purge` this REFUSES the destructive branch (the `[y/N]`
        /// confirmation cannot be answered without a TTY).
        #[arg(long = "no-tty")]
        no_tty: bool,
    },
}

/// Scrape verbs (slice-02; US-SCR-001..004; ADR-017 / ADR-019).
///
/// `scrape` is a new top-level verb; `github` is its only subcommand in
/// slice-02 (the enum leaves room for future sources). The verb shape is
/// `openlore scrape github <target> [--sign N[,N...]]`.
#[derive(Debug, Subcommand)]
pub enum ScrapeCommand {
    /// Derive candidate claims from a public GitHub target, optionally
    /// signing selected candidates through the slice-01 pipeline.
    Github {
        /// The public GitHub target: `owner/repo` or a bare `user`.
        target: String,
        /// Optional 1-based candidate indices to sign, comma-separated
        /// (`--sign 1` or `--sign 1,3`). Captured here as the RAW string;
        /// the verb-level `SelectionParser` (Phase 03/05) validates the
        /// list (rejecting duplicates / out-of-range) BEFORE any compose
        /// begins. Absent → derive + render only, ZERO writes (the
        /// human-gate, WD-49 / I-SCR-1).
        #[arg(long)]
        sign: Option<String>,
    },
}

/// Dispatch a parsed command. Returns the exit code the caller should
/// hand back to the OS. Slice-01 wires the `init` verb; other arms
/// panic in the RED scaffold per the outside-in plan.
///
/// The wire-probe-use sequence per ADR-009 D-9:
/// 1. Resolve XDG paths.
/// 2. Construct Wiring (instantiates every adapter).
/// 3. Walk the probe gauntlet; refuse with health.startup.refused on any refusal.
/// 4. For non-`init` verbs, check the bootstrap-state arm: identity.toml
///    must exist. Missing identity.toml means the user has not run
///    `openlore init` yet; refuse with a hint pointing at that command.
/// 5. Dispatch the verb.
pub fn dispatch(cli: Cli) -> i32 {
    // Step 1: paths.
    let paths = match OpenLorePaths::from_env() {
        Ok(p) => p,
        Err(err) => {
            eprintln!("openlore: failed to resolve XDG paths: {err:#}");
            return 2;
        }
    };

    // Step 2: wiring.
    let wiring = match Wiring::production(paths) {
        Ok(w) => w,
        Err(err) => {
            eprintln!("openlore: failed to construct adapter wiring: {err:#}");
            return 2;
        }
    };

    // Step 3: probe gauntlet.
    if let Err(refusal) = wiring.probe_gauntlet() {
        emit_health_startup_refused(&refusal);
        return 2;
    }

    // Step 4: bootstrap-state arm. The `init` verb IS the bootstrap;
    // every other verb requires it to have run successfully at least
    // once (identity.toml present at the resolved config path).
    if requires_initialized_state(&cli.command) {
        if let Err(refusal) = wiring.check_initialized_state() {
            emit_health_startup_refused(&refusal);
            return 2;
        }
    }

    // Step 5: dispatch.
    match cli.command {
        Command::Init {
            handle,
            app_password,
        } => match verbs::init::run(
            &wiring,
            &verbs::init::InitArgs {
                handle,
                app_password,
            },
        ) {
            Ok(outcome) => {
                print!("{}", outcome.stdout);
                outcome.exit_code
            }
            Err(err) => {
                eprintln!("openlore init: {err:#}");
                1
            }
        },
        Command::Claim(ClaimCommand::Add {
            subject,
            predicate,
            object,
            evidence,
            confidence,
        }) => match verbs::claim_add::run(
            &wiring,
            &verbs::claim_add::ClaimAddArgs {
                subject,
                predicate,
                object,
                evidence,
                confidence,
            },
        ) {
            Ok(outcome) => {
                print!("{}", outcome.stdout);
                outcome.exit_code
            }
            Err(err) => {
                eprintln!("openlore claim add: {err:#}");
                1
            }
        },
        Command::Claim(ClaimCommand::Publish { cid }) => match verbs::claim_publish::run(
            &wiring,
            &verbs::claim_publish::ClaimPublishArgs { cid },
        ) {
            Ok(outcome) => {
                print!("{}", outcome.stdout);
                outcome.exit_code
            }
            Err(err) => {
                // Typed `PublishError` routes PDS failures through the
                // WS-10 retry-hint renderer; other failure classes fall
                // back to anyhow's chained-cause format. The renderer
                // produces a newline-terminated string so we use
                // `eprint!` (not `eprintln!`) here.
                eprint!("{}", verbs::claim_publish::render_publish_error(&err));
                1
            }
        },
        Command::Claim(ClaimCommand::Retract { cid }) => match verbs::claim_retract::run(
            &wiring,
            &verbs::claim_retract::ClaimRetractArgs { cid },
        ) {
            Ok(outcome) => {
                print!("{}", outcome.stdout);
                outcome.exit_code
            }
            Err(err) => {
                eprintln!("openlore claim retract: {err:#}");
                1
            }
        },
        Command::Graph(GraphCommand::Query { subject, federated }) => {
            match verbs::graph_query::run(
                &wiring,
                &verbs::graph_query::GraphQueryArgs { subject, federated },
            ) {
                Ok(outcome) => {
                    print!("{}", outcome.stdout);
                    outcome.exit_code
                }
                Err(err) => {
                    eprintln!("openlore graph query: {err:#}");
                    1
                }
            }
        }
        Command::Claim(ClaimCommand::Counter { cid, reason }) => {
            match verbs::claim_counter::run(
                &wiring,
                &verbs::claim_counter::ClaimCounterArgs { cid, reason },
            ) {
                Ok(outcome) => {
                    print!("{}", outcome.stdout);
                    outcome.exit_code
                }
                Err(err) => {
                    eprintln!("openlore claim counter: {err:#}");
                    1
                }
            }
        }
        Command::Peer(PeerCommand::Add { did }) => {
            match verbs::peer_add::run(&wiring, &verbs::peer_add::PeerAddArgs { did }) {
                Ok(outcome) => {
                    print!("{}", outcome.stdout);
                    outcome.exit_code
                }
                Err(err) => {
                    eprintln!("openlore peer add: {err:#}");
                    1
                }
            }
        }
        Command::Peer(PeerCommand::Pull) => {
            match verbs::peer_pull::run(&wiring, &verbs::peer_pull::PeerPullArgs::default()) {
                Ok(outcome) => {
                    print!("{}", outcome.stdout);
                    outcome.exit_code
                }
                Err(err) => {
                    eprintln!("openlore peer pull: {err:#}");
                    1
                }
            }
        }
        Command::Peer(PeerCommand::Remove { did, purge, no_tty }) => {
            match verbs::peer_remove::run(
                &wiring,
                &verbs::peer_remove::PeerRemoveArgs { did, purge, no_tty },
            ) {
                Ok(outcome) => {
                    print!("{}", outcome.stdout);
                    outcome.exit_code
                }
                Err(err) => {
                    eprintln!("openlore peer remove: {err:#}");
                    1
                }
            }
        }
        Command::Scrape(ScrapeCommand::Github { target, sign }) => {
            match verbs::scrape_github::run(
                &wiring,
                &verbs::scrape_github::ScrapeGithubArgs { target, sign },
            ) {
                Ok(outcome) => {
                    print!("{}", outcome.stdout);
                    outcome.exit_code
                }
                Err(err) => {
                    eprintln!("openlore scrape github: {err:#}");
                    1
                }
            }
        }
    }
}

/// Predicate: does this verb require `openlore init` to have run first?
///
/// `Init` itself is the bootstrap verb and MUST be permitted on a fresh
/// environment (otherwise the system is unreachable). Every other verb
/// in the ADR-003 contract — `claim add`, `claim publish`, `claim
/// retract`, `graph query`, `peer *`, and the slice-02 `scrape github`
/// — operates on initialized identity + storage state and is therefore
/// gated on the bootstrap-state arm. (`scrape github --sign` reuses the
/// slice-01 sign/publish pipeline, which requires the identity; harvest
/// alone is harmless, but gating uniformly keeps the contract simple.)
fn requires_initialized_state(cmd: &Command) -> bool {
    !matches!(cmd, Command::Init { .. })
}

/// Emit a `health.startup.refused` event to stderr in the structured
/// shape DevOps consumes. The pure data (`reason`, `detail`, `structured`)
/// comes straight from the refusing adapter's `ProbeOutcome::Refused`
/// payload — no enrichment, no rewording.
///
/// For slice-01 the event format is a single JSON-line on stderr plus a
/// human-readable line. The tracing-subscriber integration lands in
/// step 05-17 (observability). The shape of the JSON line is the
/// contract; the human-readable line is convenience.
fn emit_health_startup_refused(refusal: &wiring::ProbeRefusal) {
    let event = serde_json::json!({
        "event": "health.startup.refused",
        "adapter": refusal.adapter,
        "reason": format!("{:?}", refusal.reason),
        "detail": refusal.detail,
        "structured": refusal.structured,
    });
    eprintln!("{event}");
    eprintln!(
        "openlore: refusing to start — {} adapter: {}",
        refusal.adapter, refusal.detail
    );
}

#[cfg(test)]
mod clap_dispatch_tests {
    //! Parse-only unit tests for the slice-03 verb surface (step 01-04).
    //!
    //! These enter through the clap driving port (`Cli::try_parse_from`)
    //! and assert ONLY that the argument vector routes to the correct
    //! `Command` variant with the correct fields. No dispatch / execution
    //! happens — the verb bodies are `todo!()` scaffolds at this step, so
    //! these tests pin the ROUTING contract that Phase 03+ scenarios
    //! depend on, without invoking the (panicking) handlers.
    //!
    //! Port-to-port at the parse scope: the driving port is the clap
    //! parser; the observable outcome is the parsed `Command` ADT. This is
    //! the right RED gate for a dispatch-bootstrap step.

    use super::*;
    use clap::Parser;

    /// Parse helper: build the argv with the leading binary name and parse
    /// through the real `Cli` derive surface. Returns the inner `Command`.
    fn parse(args: &[&str]) -> Command {
        let mut argv = vec!["openlore"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv)
            .unwrap_or_else(|e| panic!("clap must parse {args:?}; got error:\n{e}"))
            .command
    }

    #[test]
    fn peer_add_routes_to_peer_add_with_did() {
        let cmd = parse(&["peer", "add", "did:plc:rachel-test"]);
        match cmd {
            Command::Peer(PeerCommand::Add { did }) => {
                assert_eq!(did, "did:plc:rachel-test");
            }
            other => panic!("expected Peer(Add), got {other:?}"),
        }
    }

    #[test]
    fn peer_pull_routes_to_peer_pull() {
        let cmd = parse(&["peer", "pull"]);
        assert!(
            matches!(cmd, Command::Peer(PeerCommand::Pull)),
            "expected Peer(Pull), got {cmd:?}"
        );
    }

    #[test]
    fn peer_remove_without_purge_defaults_purge_false() {
        let cmd = parse(&["peer", "remove", "did:plc:rachel-test"]);
        match cmd {
            Command::Peer(PeerCommand::Remove { did, purge, no_tty }) => {
                assert_eq!(did, "did:plc:rachel-test");
                assert!(!purge, "--purge must default to false when absent");
                assert!(!no_tty, "--no-tty must default to false when absent");
            }
            other => panic!("expected Peer(Remove), got {other:?}"),
        }
    }

    #[test]
    fn peer_remove_with_purge_flag_sets_purge_true() {
        let cmd = parse(&["peer", "remove", "did:plc:rachel-test", "--purge"]);
        match cmd {
            Command::Peer(PeerCommand::Remove { did, purge, no_tty }) => {
                assert_eq!(did, "did:plc:rachel-test");
                assert!(purge, "--purge flag must set purge=true");
                assert!(!no_tty, "--no-tty must default to false when absent");
            }
            other => panic!("expected Peer(Remove --purge), got {other:?}"),
        }
    }

    #[test]
    fn peer_remove_with_no_tty_flag_sets_no_tty_true() {
        let cmd = parse(&[
            "peer",
            "remove",
            "did:plc:rachel-test",
            "--purge",
            "--no-tty",
        ]);
        match cmd {
            Command::Peer(PeerCommand::Remove { did, purge, no_tty }) => {
                assert_eq!(did, "did:plc:rachel-test");
                assert!(purge, "--purge flag must set purge=true");
                assert!(no_tty, "--no-tty flag must set no_tty=true");
            }
            other => panic!("expected Peer(Remove --purge --no-tty), got {other:?}"),
        }
    }

    #[test]
    fn claim_counter_routes_with_cid_and_reason() {
        let cmd = parse(&[
            "claim",
            "counter",
            "bafytargetcid",
            "--reason",
            "I disagree because X",
        ]);
        match cmd {
            Command::Claim(ClaimCommand::Counter { cid, reason }) => {
                assert_eq!(cid, "bafytargetcid");
                assert_eq!(reason, "I disagree because X");
            }
            other => panic!("expected Claim(Counter), got {other:?}"),
        }
    }

    #[test]
    fn claim_counter_without_reason_is_a_parse_error() {
        // WD-20 / data-models.md §reason: `--reason` is REQUIRED at the
        // CLI verb level. clap must reject the invocation before any
        // dispatch happens.
        let parsed = Cli::try_parse_from(["openlore", "claim", "counter", "bafytargetcid"]);
        assert!(
            parsed.is_err(),
            "claim counter without --reason must be a clap parse error (WD-20)"
        );
    }

    #[test]
    fn graph_query_without_federated_defaults_federated_false() {
        let cmd = parse(&["graph", "query", "--subject", "github:rust-lang/cargo"]);
        match cmd {
            Command::Graph(GraphCommand::Query { subject, federated }) => {
                assert_eq!(subject, "github:rust-lang/cargo");
                assert!(!federated, "--federated must default to false when absent");
            }
            other => panic!("expected Graph(Query), got {other:?}"),
        }
    }

    #[test]
    fn graph_query_with_federated_flag_sets_federated_true() {
        let cmd = parse(&[
            "graph",
            "query",
            "--subject",
            "github:rust-lang/cargo",
            "--federated",
        ]);
        match cmd {
            Command::Graph(GraphCommand::Query { subject, federated }) => {
                assert_eq!(subject, "github:rust-lang/cargo");
                assert!(federated, "--federated flag must set federated=true");
            }
            other => panic!("expected Graph(Query --federated), got {other:?}"),
        }
    }

    // ---- slice-02: `scrape github <target> [--sign N[,N...]]` (ADR-017) ----
    //
    // `scrape` is a NEW top-level verb; `github` its subcommand; `<target>`
    // is `owner/repo` or a bare `user`; `--sign` is an OPTIONAL comma-
    // separated list of 1-based candidate indices. The raw `--sign` string
    // is carried verbatim here — the verb-level `SelectionParser` (Phase 03+,
    // architecture-design §5.1) is what rejects duplicates / out-of-range
    // indices. The clap layer only routes + captures the raw selection.

    #[test]
    fn scrape_github_without_sign_routes_with_target_and_no_selection() {
        let cmd = parse(&["scrape", "github", "rust-lang/cargo"]);
        match cmd {
            Command::Scrape(ScrapeCommand::Github { target, sign }) => {
                assert_eq!(target, "rust-lang/cargo");
                assert!(
                    sign.is_none(),
                    "--sign must default to None (derive+render only, zero writes)"
                );
            }
            other => panic!("expected Scrape(Github), got {other:?}"),
        }
    }

    #[test]
    fn scrape_github_user_with_single_sign_index_routes_with_raw_selection() {
        // US-SCR-003: a bare-user target plus a single `--sign` index.
        let cmd = parse(&["scrape", "github", "torvalds", "--sign", "1"]);
        match cmd {
            Command::Scrape(ScrapeCommand::Github { target, sign }) => {
                assert_eq!(target, "torvalds");
                assert_eq!(
                    sign.as_deref(),
                    Some("1"),
                    "--sign carries the raw index list for the verb's SelectionParser"
                );
            }
            other => panic!("expected Scrape(Github --sign 1), got {other:?}"),
        }
    }

    #[test]
    fn scrape_github_with_comma_separated_sign_list_routes_with_raw_selection() {
        let cmd = parse(&["scrape", "github", "rust-lang/cargo", "--sign", "1,3"]);
        match cmd {
            Command::Scrape(ScrapeCommand::Github { target, sign }) => {
                assert_eq!(target, "rust-lang/cargo");
                assert_eq!(
                    sign.as_deref(),
                    Some("1,3"),
                    "--sign N,N,... is captured verbatim; the verb parses the list"
                );
            }
            other => panic!("expected Scrape(Github --sign 1,3), got {other:?}"),
        }
    }
}
