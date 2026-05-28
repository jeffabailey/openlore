//! `claim counter <target_cid> --reason "..."` — author a counter-claim
//! against another claim (slice-03; US-FED-004 / ADR-013 / ADR-015).
//!
//! A counter-claim is the user's OWN signed claim that carries a
//! `references[]` entry of type `counters` pointing at `target_cid`, plus
//! a mandatory free-text `reason`. It NEVER overwrites the target —
//! counter-claims coexist with the claims they counter (the "never
//! overwrite" UX contract; compose preview literal text).
//!
//! ## Pipeline (architecture §5.2)
//!
//! construct (mirror target body + Counters reference + reason)
//!   → `claim_domain::normalize_reason` (NFC; WD-35)
//!   → `claim_domain::validate_counter_claim` (pure core; self-counter +
//!      missing-reason rejection BEFORE the preview)
//!   → render compose preview (BOTH framing literals + reason verbatim +
//!      `counters: <cid> (by <peer>)`)
//!   → two-prompt (Enter to sign; Y to publish — ADR-003)
//!   → canonicalize / compute_cid / sign (SAME pure-core path as
//!      `claim_add`)
//!   → `claim_publish::publish_signed_claim` internals (single publish
//!      code path; NO parallel publish — I-FED-5 / WD-22 / WD-33).
//!
//! ## Single-publish-path (I-FED-5 / WD-22 / WD-33)
//!
//! The publish step funnels through `claim_publish::publish_signed_claim`,
//! the SAME helper `claim_add`'s Y branch and `claim_retract` use. There
//! is no parallel publish code path. The counter-claim is the user's OWN
//! artifact: it is published to the user's OWN PDS and stored in the
//! user's OWN `claims` table (NOT `peer_claims`), with `reason` in the
//! signed payload (canonicalize folds it in — ADR-006 lex order).

use std::io::Write;

use anyhow::{anyhow, Context, Result};
use claim_domain::{
    canonicalize, compute_cid, normalize_reason, validate_counter_claim, Cid, ClaimLookup,
    ClaimReference, Did, ReferenceType, SignedClaim, UnsignedClaim,
};
use ports::{PeerStoragePort, StoragePort};

use crate::io::prompt_line;
use crate::orientation::{self, OrientationMilestone};
use crate::render::{render_counter_compose_preview, ComposedCounterClaim};
use crate::wiring::Wiring;

/// Argument struct for the `claim counter` verb (mirrors the clap
/// subcommand). `reason` is REQUIRED at the CLI level (WD-20); clap
/// rejects the invocation if absent, so this field is non-optional.
#[derive(Debug, Clone)]
pub struct ClaimCounterArgs {
    /// CID of the claim being countered (the user's own OR a peer's).
    pub cid: String,
    /// Mandatory free-text explanation, NFC-normalized at compose time.
    pub reason: String,
}

/// Outcome of one `claim counter` invocation — exit code + stdout chunk,
/// uniform with the other verbs so the dispatcher routes identically.
pub struct ClaimCounterOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// The inclusive upper bound on a counter-claim `--reason`, in Unicode
/// scalar values (WD-20: `1..=1000`; ADR-015 `maxLength` on the Lexicon
/// `reason` field). Length is measured on the NFC-normalized reason so the
/// CLI pre-compose guard agrees with the lexicon layer (LCC-5).
const MAX_REASON_CHARS: usize = 1000;

/// Run the `claim counter` verb.
pub fn run(wiring: &Wiring, args: &ClaimCounterArgs) -> Result<ClaimCounterOutcome> {
    // Step 1: NFC-normalize the reason (WD-35). The normalized bytes are
    // what get displayed AND signed, so a copy-pasted decomposed accent
    // and a precomposed one land on the same CID.
    let reason = normalize_reason(&args.reason);

    // Step 1b: CLI-layer pre-compose guard on `--reason` (WD-20). A missing
    // FLAG is a clap error; an empty or over-length VALUE reaches here and
    // must be rejected BEFORE any target resolution, preview, sign, or
    // publish — the user gets a clear, actionable message instead of a
    // useless compose preview. The lexicon-layer length check (LCC-5) is the
    // defense-in-depth net at sign-time; this is the early, friendly gate.
    check_reason_pre_compose(&reason).map_err(|e| anyhow!(e))?;

    // Step 2: resolve the target across BOTH stores (own `claims` first,
    // then the peer cache). We need the target's author DID for the
    // `counters: <cid> (by <peer>)` preview line AND a `ClaimLookup` for
    // the pure-core self-counter check. Resolving the target also gives us
    // the body we mirror (subject/predicate/object) — a counter-claim is a
    // meta-claim ABOUT the original assertion.
    let target_cid = Cid(args.cid.clone());
    let resolved = resolve_target(wiring, &target_cid)?.ok_or_else(|| {
        anyhow!(
            "no claim with cid {} found in your own store or your peer cache. \
             Pull the peer first (`openlore peer pull`) or check the CID.",
            args.cid
        )
    })?;

    // Step 3: build the unsigned counter-claim. Body mirrors the target
    // (the counter is ABOUT that assertion); confidence = 1.0 (you ARE
    // certain you disagree); evidence = [] (the `reason` + the Counters
    // pointer ARE the body); composed_at from the clock port for a fresh
    // timestamp distinct from the target.
    let confidence: claim_domain::Confidence = serde_json::from_value(serde_json::json!(1.0))
        .map_err(|e| anyhow!("encoding confidence 1.0 for counter-claim: {e}"))?;
    let unsigned = UnsignedClaim {
        subject: resolved.claim.unsigned.subject.clone(),
        predicate: resolved.claim.unsigned.predicate.clone(),
        object: resolved.claim.unsigned.object.clone(),
        evidence: Vec::new(),
        confidence,
        author_did: wiring.identity.author_did().clone(),
        composed_at: wiring.clock.now_utc().to_rfc3339(),
        references: vec![ClaimReference {
            ref_type: ReferenceType::Counters,
            cid: target_cid.clone(),
        }],
        reason: Some(reason.clone()),
    };

    // Step 4: pure-core validation BEFORE the preview (WD-34). Catches the
    // missing-reason + self-counter rejections so the user sees a clear
    // error instead of a useless preview. The lookup spans BOTH stores so
    // countering one's OWN claim (in `claims`) is rejected too.
    let lookup = CombinedClaimLookup {
        storage: wiring.storage.as_ref(),
        peer_storage: wiring.peer_storage.as_ref(),
    };
    let current_user = wiring.identity.author_did().clone();
    validate_counter_claim(&unsigned, &lookup, &current_user).map_err(|e| anyhow!("{e}"))?;

    // Step 4b: first-counter-claim orientation (WD-43 / WD-39). The FIRST
    // EVER successful `claim counter` per install emits a one-time framing
    // block BEFORE the compose preview (gherkin habit scenario 2); it does
    // NOT delay or modify the standard framing — it precedes it. Gated by
    // `[federation] first_counter_claim_completed_at` in identity.toml;
    // once-per-user (NOT first-3-times). A failed orientation write is
    // logged, never fatal (data-models.md §OrientationState).
    let framing_block = maybe_emit_first_counter_claim_orientation(wiring);

    // Step 5: render the compose preview (BOTH framing literals + the
    // `counters: <cid> (by <peer>)` line + the reason verbatim, wrapped at
    // 78 cols). Print to stdout BEFORE the prompts so the user reviews
    // before confirming. The one-time framing block (when present) is
    // emitted first, ahead of the standard preview framing.
    let preview = render_counter_compose_preview(&ComposedCounterClaim {
        target_cid: target_cid.0.clone(),
        target_author_did: crate::verbs::bare_did(&resolved.author_did.0),
        reason,
        author_did: unsigned.author_did.0.clone(),
        composed_at: unsigned.composed_at.clone(),
    });
    {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(framing_block.as_bytes())?;
        stdout.write_all(preview.as_bytes())?;
        stdout.flush()?;
    }

    // Step 6: two-prompt (ADR-003). Enter to sign; EOF before any input is
    // a clean cancel (no side effects, exit 0).
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let sign_prompt = "\nPress Enter to sign this counter-claim locally (or Ctrl-C to cancel): ";
    let confirmation = prompt_line(&mut stdout, &mut stdin, sign_prompt)?;
    if confirmation.is_none() {
        return Ok(ClaimCounterOutcome {
            exit_code: 0,
            stdout: String::new(),
        });
    }

    // Step 7: canonicalize → compute_cid → sign → persist. SAME pure-core
    // path `claim_add` uses; canonicalize folds the `reason` into the
    // bytes so the CID + signature cover it (ADR-006 lex order).
    let canonical_bytes =
        canonicalize(&unsigned).map_err(|e| anyhow!("canonicalizing counter-claim: {e}"))?;
    let unsigned_cid = compute_cid(&canonical_bytes);
    writeln!(stdout, "Computing claim CID {}", unsigned_cid.0)?;
    stdout.flush()?;

    let signature = wiring
        .identity
        .sign(&unsigned_cid)
        .map_err(|e| anyhow!("signing counter-claim: {e}"))?;
    let signed = SignedClaim {
        unsigned,
        signature,
    };

    // The counter-claim is the user's OWN artifact — it lands in the user's
    // OWN `claims` table + `claims/<cid>.json` (NOT peer_claims).
    wiring
        .storage
        .write_signed_claim(&signed)
        .with_context(|| {
            format!(
                "persisting counter-claim {} to local store",
                signed.signature.signed_cid.0
            )
        })?;
    let artifact_path = wiring
        .paths
        .claims_dir()
        .join(format!("{}.json", signed.signature.signed_cid.0));
    writeln!(
        stdout,
        "Written to local store: {}",
        artifact_path.display()
    )?;
    stdout.flush()?;

    // Step 8: SEPARATE publish prompt (ADR-003 two-prompt contract). Y/y
    // publishes; anything else (n/N/Enter/EOF) is a clean decline (local
    // artifact stays put, no PDS call, exit 0).
    let publish_prompt = "\nPublish this counter-claim to your PDS now? (y/N): ";
    let publish_answer = prompt_line(&mut stdout, &mut stdin, publish_prompt)?;
    let confirmed_publish = matches!(
        publish_answer.as_deref().map(str::trim),
        Some("y") | Some("Y") | Some("yes") | Some("YES")
    );
    if confirmed_publish {
        drop(stdout);
        drop(stdin);
        // Single publish code path (I-FED-5 / WD-22 / WD-33): the SAME
        // helper the standalone `claim publish`, `claim add` Y branch, and
        // `claim retract` use. Published to the user's OWN PDS.
        match crate::verbs::claim_publish::publish_signed_claim(wiring, &signed) {
            Ok(publish_outcome) => {
                let rendered =
                    crate::verbs::claim_publish::render_publish_success(&publish_outcome);
                print!("{rendered}");
            }
            Err(err) => {
                // The local artifact is already on disk (local-first
                // invariant — the sign + write path ran BEFORE this
                // publish and is NOT rolled back). Route through the shared
                // renderer so the retry guidance matches the other verbs.
                eprint!(
                    "{}",
                    crate::verbs::claim_publish::render_publish_error(&err)
                );
                return Ok(ClaimCounterOutcome {
                    exit_code: 1,
                    stdout: String::new(),
                });
            }
        }
    }

    Ok(ClaimCounterOutcome {
        exit_code: 0,
        stdout: String::new(),
    })
}

/// The target of a counter, resolved from either store, paired with its
/// author DID. Carrying the author alongside the claim is the anti-merging
/// discipline: a peer claim is never separated from its attribution.
struct ResolvedTarget {
    claim: SignedClaim,
    author_did: Did,
}

/// Resolve `target_cid` across BOTH stores: the user's OWN `claims` table
/// first, then the peer cache. Returns the claim + its author DID, or
/// `None` if neither store knows it.
fn resolve_target(wiring: &Wiring, target_cid: &Cid) -> Result<Option<ResolvedTarget>> {
    // Own store: the author is the local user.
    if let Some(own) = wiring
        .storage
        .read_signed_claim(target_cid)
        .with_context(|| format!("looking up own claim for cid {}", target_cid.0))?
    {
        let author_did = own.unsigned.author_did.clone();
        return Ok(Some(ResolvedTarget {
            claim: own,
            author_did,
        }));
    }
    // Peer cache: `get_peer_claim_by_cid` returns the attribution pair so
    // we never get a claim without its author DID (anti-merging layer-1).
    if let Some((author_did, claim)) = wiring
        .peer_storage
        .get_peer_claim_by_cid(target_cid)
        .map_err(|e| anyhow!("looking up peer claim for cid {}: {e}", target_cid.0))?
    {
        return Ok(Some(ResolvedTarget { claim, author_did }));
    }
    Ok(None)
}

/// Pure CLI-layer pre-compose validation of the (already NFC-normalized)
/// `--reason` (WD-20). Two railway-style failures, in order:
///
/// 1. **Empty** — a reason that is blank (or only whitespace) after
///    normalization fails with the content-frozen requirement literal. A
///    counter MUST explain itself (ADR-015); the message tells the user
///    exactly what to do.
/// 2. **Too long** — more than [`MAX_REASON_CHARS`] Unicode scalar values
///    fails with a message naming the upper bound, so the user knows how
///    far over they are.
///
/// Returns `Ok(())` for a valid reason. Pure — no I/O — so the guard runs
/// before any target resolution or preview rendering (the pre-compose
/// ordering CC-2/CC-3 assert).
fn check_reason_pre_compose(reason: &str) -> Result<(), String> {
    if reason.trim().is_empty() {
        return Err("counter-claims require --reason; explain your disagreement".to_string());
    }
    let chars = reason.chars().count();
    if chars > MAX_REASON_CHARS {
        return Err(format!(
            "--reason is too long: {chars} characters exceeds the {MAX_REASON_CHARS}-character maximum"
        ));
    }
    Ok(())
}

/// Emit the first-counter-claim orientation block exactly once per install
/// (WD-43 / WD-39). Returns the rendered framing text to prepend ahead of
/// the compose preview, or the empty string if it has already fired.
///
/// Mirrors `peer_pull::maybe_emit_first_pull_orientation`: load the
/// `[federation]` snapshot, consult the PURE `should_fire`, record the
/// milestone on first fire, and return the block. A write failure is
/// logged-and-ignored (the orientation may re-fire on the next counter, but
/// the counter itself proceeds) — never fatal (data-models.md
/// §OrientationState).
fn maybe_emit_first_counter_claim_orientation(wiring: &Wiring) -> String {
    let identity_path = wiring.paths.identity_toml();
    let state = orientation::load(&identity_path).unwrap_or_default();
    if !state.should_fire(OrientationMilestone::FirstCounterClaim) {
        return String::new();
    }

    let now = wiring.clock.now_utc().to_rfc3339();
    if let Err(err) =
        orientation::mark_completed(&identity_path, OrientationMilestone::FirstCounterClaim, now)
    {
        // Non-fatal: the orientation may re-fire on the next counter, but the
        // counter itself succeeds. Log to stderr, do not abort.
        eprintln!(
            "openlore claim counter: could not record first-counter-claim orientation: {err:#}"
        );
    }

    first_counter_claim_framing_block()
}

/// PURE render of the one-time first-counter-claim framing block (WD-43;
/// gherkin habit scenario 2, content-frozen). The heading plus four
/// enumerated habit-bridging points, followed by a blank line so the
/// standard compose preview that follows reads as a distinct section.
fn first_counter_claim_framing_block() -> String {
    let mut out = String::new();
    out.push_str("First counter-claim! Some context:\n");
    out.push_str("  - A counter-claim is a SIGNED public artifact attributed to YOU.\n");
    out.push_str("  - It does NOT delete or hide the target claim; both coexist.\n");
    out.push_str("  - You can retract it later via `openlore claim retract <your_cid>`.\n");
    out.push_str("  - The target peer is NOT auto-notified; they will see it next time\n");
    out.push_str("    they pull your claims (if they subscribe to you).\n");
    out.push('\n');
    out
}

/// A `ClaimLookup` spanning BOTH the author store AND the peer cache. The
/// pure-core `validate_counter_claim` self-counter check resolves the
/// target through this so countering one's OWN claim (in `claims`) is
/// rejected even though the target may equally live in `peer_claims`.
struct CombinedClaimLookup<'a> {
    storage: &'a dyn StoragePort,
    peer_storage: &'a dyn PeerStoragePort,
}

impl<'a> ClaimLookup for CombinedClaimLookup<'a> {
    fn signed_by_cid(&self, cid: &Cid) -> Option<SignedClaim> {
        if let Ok(Some(own)) = self.storage.read_signed_claim(cid) {
            return Some(own);
        }
        // The peer-store lookup returns (author_did, claim); the claim
        // already carries `author_did` inside `unsigned`, so the
        // self-counter check (which inspects `resolved.unsigned.author_did`)
        // sees the right attribution.
        self.peer_storage
            .get_peer_claim_by_cid(cid)
            .ok()
            .flatten()
            .map(|(_author, claim)| claim)
    }
}
