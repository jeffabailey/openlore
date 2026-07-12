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
    /// Search the NETWORK index — slice-05 (ADR-027). A NEW top-level verb
    /// (WD-113): `graph query` stays unambiguously LOCAL; `search` is the only
    /// NETWORK verb. Queries the self-hosted indexer over HTTP/XRPC along one
    /// dimension (`--object`/`--contributor`/`--subject`), inspects one result
    /// (`--show <cid>`), or emits a query-encoding link (`--share`). An
    /// unreachable indexer degrades gracefully — it never blocks the CLI
    /// (WD-116 / KPI-5).
    Search {
        /// Query by OBJECT (philosophy URI) — the headline dimension (US-AV-002).
        #[arg(long)]
        object: Option<String>,
        /// Query by CONTRIBUTOR (DID) — one developer's network trail (US-AV-003).
        #[arg(long)]
        contributor: Option<String>,
        /// Query by SUBJECT (project URI) (US-AV-004).
        #[arg(long)]
        subject: Option<String>,
        /// Inspect one result by CID — full record + the verification line
        /// (US-AV-006). Distinct from an empty dimension search (which exits 0).
        #[arg(long)]
        show: Option<String>,
        /// Emit a stable query-encoding link instead of running the search
        /// (WD-110 / I-AV-8) — encodes the QUERY (dimension + value), never a
        /// result snapshot.
        #[arg(long)]
        share: bool,
        /// HIDE author-self-retracted claims from THIS view only (opt-in,
        /// non-destructive; feature `retraction-aware-search-filter`, US-RF-001).
        /// Absent ⇒ today's default path, byte-identical (I-RF-1). When set, a
        /// claim whose OWN author published a `Retracts` marker for it is removed
        /// from the current results and the surface discloses exactly how many
        /// retraction EVENTS it hid + how to re-run without the flag. A
        /// third-party `Counters`/`Retracts` never hides a row (D-3, no
        /// heckler's veto).
        #[arg(long)]
        hide_retracted: bool,
        /// OPEN a shared `openlore://search?<dim>=<value>` link (the CLI re-run
        /// resolver, Q-DELIVER-AV-3 / US-AV-006 Ex2): RE-RUNS the encoded query
        /// against the CURRENT index (the link encoded the QUERY, never a
        /// snapshot — I-AV-8). A positional argument the verb detects.
        link: Option<String>,
    },
    /// Serve the read-only htmx viewer over localhost HTTP — slice-06 (ADR-028).
    /// A long-running `openlore ui [--port <P>]` server bound to 127.0.0.1 ONLY,
    /// with NO auth and NO signing key, that renders the operator's OWN node
    /// store as server-rendered HTML over a READ-ONLY `StoreReadPort` (I-VIEW-1).
    /// Signing stays EXCLUSIVELY in the CLI verbs (I-VIEW-3 / I-SCR-1).
    Ui {
        /// The localhost port to bind. Defaults to the ADR-028 value (8788). Use
        /// `--port 0` for an OS-assigned ephemeral port (parallel-safe tests; the
        /// bound address is reported as the `viewer.serve.listening` event).
        #[arg(long, default_value_t = 8788)]
        port: u16,
    },
    /// Discover the shared philosophy vocabulary — slice-22 (ADR-059).
    /// `openlore philosophy list` prints the embedded well-known philosophy
    /// seeds (a stable object id + name + one-line description each) so the user
    /// can copy an EXACT shared object into a claim `--object` instead of
    /// inventing a private string (J-002). OFFLINE by construction — reads the
    /// compile-time seed constants; no store, no signer, no network (AC-001.4).
    #[command(subcommand)]
    Philosophy(PhilosophyCommand),
}

/// Philosophy vocabulary verbs (slice-22; US-PV-001; ADR-059).
#[derive(Debug, Subcommand)]
pub enum PhilosophyCommand {
    /// List the embedded well-known philosophy seeds (offline discovery).
    List {
        /// Opt-in machine-readable JSON emission. Text is the DEFAULT view
        /// (AC-001.3 / P-001); `--json` is strictly opt-in.
        #[arg(long)]
        json: bool,
    },
    /// Inspect ONE philosophy in full — slice-23 (US-PV-002; ADR-059 §5).
    /// `openlore philosophy show <name-or-object>` accepts EITHER a bare
    /// name (`memory-safety`) OR the full derived object id
    /// (`org.openlore.philosophy.memory-safety`) and prints the record's
    /// name, full description, aliases, and seeAlso link. OFFLINE by
    /// construction — resolves against the embedded seeds; no store, no
    /// signer, no network (AC-002.1).
    Show {
        /// The philosophy to inspect: a bare name or its full derived object
        /// id (both resolve to the same record).
        #[arg(value_name = "NAME_OR_OBJECT")]
        key: String,
    },
    /// Mint a NEW philosophy — slice-24 (US-PV-003; ADR-059 §4.5).
    /// `openlore philosophy add --name <n> --description <d> [--alias <a>...]
    /// [--see-also <url>...]` composes an `org.openlore.philosophy` record,
    /// signs it locally (reusing the claim signing model — ADR-006), and
    /// persists it as a signed `<cid>.json` artifact + a `philosophies` row.
    /// Local-first: nothing is signed or written before the sign prompt is
    /// confirmed. A name colliding with a shipped seed is refused (AC-003.3);
    /// an empty `--description` is a named-field error (AC-003.4).
    Add {
        /// The philosophy name — its normalized form is the derived object id
        /// segment (`org.openlore.philosophy.<normalize(name)>`).
        #[arg(long)]
        name: String,
        /// A one-line-or-more description of the philosophy.
        #[arg(long)]
        description: String,
        /// Alias strings that triangulate onto this philosophy (repeatable).
        #[arg(long)]
        alias: Vec<String>,
        /// See-also reference links (repeatable).
        #[arg(long = "see-also")]
        see_also: Vec<String>,
    },
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
    ///
    /// Slice-04 (ADR-020) adds the explorer flags on top of the slice-01/03
    /// `--subject`/`--federated` surface. `--subject` stays optional now (the
    /// dimension flags `--object`/`--contributor` are alternative entry points);
    /// the verb body adjudicates which dimension was supplied. The explorer
    /// flags are strictly OPT-IN — a bare `--subject` query is byte-identical to
    /// slice-01/03 (WD-87 / architecture-design §5.2 invariant 2).
    Query {
        /// Slice-01/03 subject dimension. Optional in slice-04 because
        /// `--object`/`--contributor` are alternative dimensions.
        #[arg(long)]
        subject: Option<String>,
        /// `--federated` (slice-03): include subscribed peers' claims in
        /// the result. Defaults to local-only (the slice-01 behavior). The
        /// slice-04 explorer flags IMPLY federated scope (WD-87 / OD-GRAPH-4),
        /// so the verb treats `--object`/`--contributor`/`--traverse`/
        /// `--weighted`/`--explain` as own + peers without an explicit
        /// `--federated`.
        #[arg(long)]
        federated: bool,
        /// Slice-04 (ADR-020): query by OBJECT (philosophy URI) — which
        /// projects embody this philosophy, grouped by subject, every claim
        /// row attributed (US-GRAPH-001). Implies federated scope.
        #[arg(long)]
        object: Option<String>,
        /// Slice-04 (ADR-020): query by CONTRIBUTOR (DID) — one developer's
        /// full reasoning trail across subjects (US-GRAPH-002). Implies
        /// federated scope.
        #[arg(long)]
        contributor: Option<String>,
        /// Slice-04 (ADR-020): traverse contributor↔project↔philosophy edges
        /// from the queried dimension, bounded + cycle-safe (US-GRAPH-004).
        /// OPT-IN. Implies federated scope.
        #[arg(long)]
        traverse: bool,
        /// Slice-04 (ADR-020): traversal depth bound (WD-76). Defaults to 2;
        /// `--depth K` widens the bound for a deeper walk. Only meaningful with
        /// `--traverse`.
        #[arg(long, default_value_t = 2)]
        depth: u8,
        /// Slice-04 (ADR-020): render the transparent, display-only adherence
        /// weight + bucket ranking via the pure `scoring` core (US-GRAPH-003).
        /// OPT-IN. `--score` is an accepted alias. Implies federated scope.
        #[arg(long, visible_alias = "score")]
        weighted: bool,
        /// Slice-04 (ADR-020): audit one subject's weight — render the per-claim
        /// `Contribution` breakdown whose running sum reproduces the displayed
        /// weight by hand (US-GRAPH-005). Takes the subject to explain. A
        /// subject absent from the result set is a usage error (non-zero exit).
        #[arg(long)]
        explain: Option<String>,
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

    // Step 1.5: the `ui` viewer verb is its OWN read-only composition root
    // (component-boundaries.md §"the `ui` verb"; ADR-028/030). It MUST NOT go
    // through the read-write `Wiring::production` below — that would (a) open the
    // store READ-WRITE, conflicting with any process holding the store and
    // surfacing a generic non-viewer error instead of the plain-language
    // store-readable refusal (NFR-VIEW-6 / AC-001.4); and (b) construct the
    // identity / PDS / writable-storage adapters the viewer process is forbidden
    // to hold (I-VIEW-1/3 — no signing key in the web process). So the viewer
    // walks its OWN WIRE→PROBE→USE gauntlet inside `verbs::ui::run` (it opens its
    // own store handle, probes store-readability + loopback, and refuses BEFORE
    // binding a serve loop). Routed here, before the read-write wiring is built.
    if let Command::Ui { port } = cli.command {
        return verbs::ui::run(&paths, &verbs::ui::UiArgs { port });
    }

    // Step 1.6: the `philosophy list` discovery verb is OFFLINE by construction
    // (ADR-059 D3) — it reads the compile-time embedded seed constants, needs NO
    // store handle, NO signer, NO network, and must run even before `init` and
    // with the network disabled (AC-001.4 / I-9). Like `ui` above it is routed
    // here, BEFORE the read-write `Wiring::production` (and its probe gauntlet /
    // bootstrap-state check), so no store is opened and no outbound call can be
    // attempted. The verb returns `(exit_code, stdout)`; the dispatcher prints.
    if let Command::Philosophy(PhilosophyCommand::List { json }) = cli.command {
        let (exit_code, stdout) =
            verbs::philosophy_list::run(&verbs::philosophy_list::PhilosophyListArgs { json });
        print!("{stdout}");
        return exit_code;
    }

    // Step 1.7: the `philosophy show` inspection verb is OFFLINE by construction
    // too (ADR-059 §5 slice-23) — it resolves the name-OR-object key against the
    // compile-time embedded seeds via `lexicon::philosophy::find`, needs NO store
    // handle, NO signer, NO network. Routed here, BEFORE the read-write
    // `Wiring::production`, on the same offline path as `list` (AC-002.1 / I-9).
    if let Command::Philosophy(PhilosophyCommand::Show { key }) = cli.command {
        let (exit_code, stdout) =
            verbs::philosophy_show::run(&verbs::philosophy_show::PhilosophyShowArgs { key });
        print!("{stdout}");
        return exit_code;
    }

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
        Command::Graph(GraphCommand::Query {
            subject,
            federated,
            object,
            contributor,
            traverse,
            depth,
            weighted,
            explain,
        }) => {
            match verbs::graph_query::run(
                &wiring,
                &verbs::graph_query::GraphQueryArgs {
                    subject,
                    federated,
                    object,
                    contributor,
                    traverse,
                    depth,
                    weighted,
                    explain,
                },
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
        Command::Search {
            object,
            contributor,
            subject,
            show,
            share,
            hide_retracted,
            link,
        } => match verbs::search::run(
            &wiring,
            &verbs::search::SearchArgs {
                object,
                contributor,
                subject,
                show,
                share,
                hide_retracted,
                link,
            },
        ) {
            Ok(outcome) => {
                print!("{}", outcome.stdout);
                outcome.exit_code
            }
            Err(err) => {
                eprintln!("openlore search: {err:#}");
                1
            }
        },
        // Slice-06 (ADR-028/030): the read-only `openlore ui` viewer is handled
        // EARLY (Step 1.5 above) as its OWN read-only composition root — it never
        // reaches this read-write-wiring dispatch. This arm is unreachable but
        // kept exhaustive for the match.
        Command::Ui { .. } => unreachable!(
            "the `ui` verb is dispatched as its own read-only composition root \
             before the read-write wiring (see Step 1.5 in `dispatch`)"
        ),
        // Slice-24 (ADR-059 §4.5): the `philosophy add` MINT verb needs BOTH
        // the store and the signer (unlike the offline `list`/`show` reads), so
        // it IS dispatched here, through the read-write wiring, AFTER the probe
        // gauntlet + bootstrap-state check.
        Command::Philosophy(PhilosophyCommand::Add {
            name,
            description,
            alias,
            see_also,
        }) => match verbs::philosophy_add::run(
            &wiring,
            &verbs::philosophy_add::PhilosophyAddArgs {
                name,
                description,
                aliases: alias,
                see_also,
            },
        ) {
            Ok(outcome) => {
                print!("{}", outcome.stdout);
                outcome.exit_code
            }
            Err(err) => {
                eprintln!("openlore philosophy add: {err:#}");
                1
            }
        },
        // Slice-22/23 (ADR-059): the offline `philosophy list`/`show` verbs are
        // handled EARLY (Steps 1.6/1.7 above) as their own store-independent
        // entry points — they never reach this read-write-wiring dispatch. This
        // arm is unreachable but kept exhaustive for the match.
        Command::Philosophy(PhilosophyCommand::List { .. } | PhilosophyCommand::Show { .. }) => {
            unreachable!(
                "the `philosophy list`/`show` verbs are dispatched offline before the \
                 read-write wiring (see Steps 1.6/1.7 in `dispatch`)"
            )
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
            Command::Graph(GraphCommand::Query {
                subject, federated, ..
            }) => {
                assert_eq!(subject.as_deref(), Some("github:rust-lang/cargo"));
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
            Command::Graph(GraphCommand::Query {
                subject, federated, ..
            }) => {
                assert_eq!(subject.as_deref(), Some("github:rust-lang/cargo"));
                assert!(federated, "--federated flag must set federated=true");
            }
            other => panic!("expected Graph(Query --federated), got {other:?}"),
        }
    }

    // ---- slice-04: the six explorer flags (ADR-020 / DD-GRAPH-13) ----
    //
    // RED_UNIT (step 01-04): pin the clap ROUTING contract for the explorer
    // surface. The driving port is the clap parser; the observable outcome is
    // the parsed `Command::Graph(Query { .. })` ADT with the right fields. The
    // verb body is a `todo!()` scaffold at this step — these tests verify only
    // that the six flags parse into the correct command variant, NOT execution.

    #[test]
    fn graph_query_explorer_flags_default_off_and_depth_defaults_to_two() {
        // A bare `--subject` query: every explorer flag is off, depth defaults
        // to the WD-76 value of 2. This is the byte-identical slice-01/03
        // surface (architecture-design §5.2 invariant 2).
        let cmd = parse(&["graph", "query", "--subject", "github:rust-lang/cargo"]);
        match cmd {
            Command::Graph(GraphCommand::Query {
                object,
                contributor,
                traverse,
                depth,
                weighted,
                explain,
                ..
            }) => {
                assert!(object.is_none(), "--object defaults to None");
                assert!(contributor.is_none(), "--contributor defaults to None");
                assert!(!traverse, "--traverse defaults to false");
                assert_eq!(depth, 2, "--depth defaults to the WD-76 value of 2");
                assert!(!weighted, "--weighted defaults to false");
                assert!(explain.is_none(), "--explain defaults to None");
            }
            other => panic!("expected Graph(Query), got {other:?}"),
        }
    }

    #[test]
    fn graph_query_object_dimension_routes_with_philosophy() {
        let cmd = parse(&[
            "graph",
            "query",
            "--object",
            "org.openlore.philosophy.dependency-pinning",
        ]);
        match cmd {
            Command::Graph(GraphCommand::Query { object, .. }) => {
                assert_eq!(
                    object.as_deref(),
                    Some("org.openlore.philosophy.dependency-pinning"),
                    "--object carries the philosophy URI"
                );
            }
            other => panic!("expected Graph(Query --object), got {other:?}"),
        }
    }

    #[test]
    fn graph_query_contributor_dimension_routes_with_did() {
        let cmd = parse(&["graph", "query", "--contributor", "did:plc:rachel-test"]);
        match cmd {
            Command::Graph(GraphCommand::Query { contributor, .. }) => {
                assert_eq!(
                    contributor.as_deref(),
                    Some("did:plc:rachel-test"),
                    "--contributor carries the DID"
                );
            }
            other => panic!("expected Graph(Query --contributor), got {other:?}"),
        }
    }

    #[test]
    fn graph_query_traverse_with_depth_override_routes() {
        let cmd = parse(&[
            "graph",
            "query",
            "--object",
            "org.openlore.philosophy.dependency-pinning",
            "--traverse",
            "--depth",
            "3",
        ]);
        match cmd {
            Command::Graph(GraphCommand::Query {
                traverse, depth, ..
            }) => {
                assert!(traverse, "--traverse flag sets traverse=true");
                assert_eq!(depth, 3, "--depth 3 overrides the default bound");
            }
            other => panic!("expected Graph(Query --traverse --depth 3), got {other:?}"),
        }
    }

    #[test]
    fn graph_query_weighted_flag_and_score_alias_both_route() {
        for flag in ["--weighted", "--score"] {
            let cmd = parse(&[
                "graph",
                "query",
                "--object",
                "org.openlore.philosophy.dependency-pinning",
                flag,
            ]);
            match cmd {
                Command::Graph(GraphCommand::Query { weighted, .. }) => {
                    assert!(weighted, "{flag} sets weighted=true (--score is an alias)");
                }
                other => panic!("expected Graph(Query {flag}), got {other:?}"),
            }
        }
    }

    #[test]
    fn graph_query_explain_routes_with_subject_to_audit() {
        let cmd = parse(&[
            "graph",
            "query",
            "--object",
            "org.openlore.philosophy.dependency-pinning",
            "--weighted",
            "--explain",
            "github:denoland/deno",
        ]);
        match cmd {
            Command::Graph(GraphCommand::Query {
                weighted, explain, ..
            }) => {
                assert!(weighted, "--weighted is set alongside --explain");
                assert_eq!(
                    explain.as_deref(),
                    Some("github:denoland/deno"),
                    "--explain carries the subject to audit"
                );
            }
            other => panic!("expected Graph(Query --explain), got {other:?}"),
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
