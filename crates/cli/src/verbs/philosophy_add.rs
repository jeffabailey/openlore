//! `philosophy add` — compose → SIGN → persist a NEW `org.openlore.philosophy`
//! record (slice-24; US-PV-003 / AC-003.1..4; ADR-059 §4.5).
//!
//! Mirrors `claim add`'s two-prompt, local-first flow (ADR-003):
//!
//! 1. Compose an `org.openlore.philosophy` record from the flags.
//! 2. Validate it via the PURE `lexicon::validate_philosophy_json` BEFORE any
//!    prompt or side effect — a missing/blank required field (e.g. an empty
//!    `--description`) is a named-field hard error (AC-003.4 / PA-4), NEVER a
//!    panic.
//! 3. (03-02 inserts the seed-collision pre-check HERE, before the prompt.)
//! 4. Render the compose preview + print the sign prompt; block on stdin.
//!    - Empty stdin / EOF = clean cancel: exit 0, preview shown, NO write
//!      (PA-2 — the local-first invariant, mirroring `claim add` / WS-3).
//!    - `<Enter>` confirms the sign.
//! 5. On confirm: derive the canonical bytes, `compute_cid`, sign via
//!    `IdentityPort` (reusing `claim_domain`'s signing model verbatim — ADR-006,
//!    NO new primitive), and persist via `StoragePort::write_signed_philosophy`
//!    (atomic `<cid>.json` artifact embedding the author DID + the DB row).
//!    Then print the derived object id + the written path.
//!
//! LOCAL-FIRST INVARIANT (KPI-5 / AC-003.2): NO storage write happens before
//! the user confirms the sign prompt. The author DID is recorded in the signed
//! artifact (PA-5) — it is embedded on `SignedPhilosophy`, self-describing off
//! the DB.

use std::io::Write;

use anyhow::{anyhow, Context, Result};
use claim_domain::{compute_cid, Did};
use lexicon::Philosophy;
use ports::SignedPhilosophy;
use serde::Serialize;

use crate::io::prompt_line;
use crate::render::render_compose_preview;
use crate::wiring::Wiring;

/// Argument struct for the `philosophy add` verb (mirrors the clap subcommand).
#[derive(Debug, Clone)]
pub struct PhilosophyAddArgs {
    pub name: String,
    pub description: String,
    pub aliases: Vec<String>,
    pub see_also: Vec<String>,
}

/// Outcome of one `philosophy add` invocation: the exit code + a stdout chunk
/// the dispatcher prints.
pub struct PhilosophyAddOutcome {
    pub exit_code: i32,
    pub stdout: String,
}

/// The canonical-bytes payload the philosophy CID is computed over. Reuses
/// `claim_domain::compute_cid` + `IdentityPort::sign` verbatim (ADR-006); the
/// deterministic serde serialization (fixed field order) is the canonical byte
/// source for the LOCAL content address (slice-24 persists locally only — no
/// federation of philosophy records yet, so no CBOR wire contract is owed). The
/// signed bytes cover the record content, its derived object id, the author,
/// and the compose timestamp.
#[derive(Serialize)]
struct UnsignedPhilosophy<'a> {
    philosophy: &'a Philosophy,
    object_id: &'a str,
    author: &'a str,
    composed_at: &'a str,
}

/// Run the `philosophy add` verb. Composes, validates, previews, blocks on the
/// sign prompt, and on confirmation signs + persists the record locally.
///
/// Needs BOTH the store and the signer (unlike `philosophy list`/`show`, which
/// are offline seed reads), so it is dispatched AFTER `Wiring::production`.
pub fn run(wiring: &Wiring, args: &PhilosophyAddArgs) -> Result<PhilosophyAddOutcome> {
    // Step 1: compose the record from the flags.
    let record = Philosophy {
        name: args.name.clone(),
        description: args.description.clone(),
        aliases: args.aliases.clone(),
        see_also: args.see_also.clone(),
    };

    // Step 2: pre-sign validation (AC-003.4 / PA-4). A missing/blank required
    // field is a named-field hard error BEFORE any preview, prompt, sign, or
    // write — the pure validator names the offending field (e.g. `description`)
    // and never panics.
    let record_json =
        serde_json::to_value(&record).map_err(|e| anyhow!("encoding philosophy record: {e}"))?;
    lexicon::validate_philosophy_json(&record_json)
        .map_err(|e| anyhow!("invalid philosophy record: {e}"))?;

    let object_id = lexicon::object_id(&args.name);
    let author_did = wiring.identity.author_did().0.clone();
    let composed_at = wiring.clock.now_utc().to_rfc3339();

    // Step 3 (03-02): the seed-collision pre-check lands here, before the
    // preview + sign prompt, so a colliding name is refused with NO write.

    // Step 4: render + print the preview, then block on the sign prompt. The
    // preview is written directly to stdout (not buffered into the outcome) so
    // the user sees it BEFORE the prompt is consumed — in both interactive and
    // piped-stdin modes.
    let preview = render_compose_preview(&record, &author_did, &composed_at);
    {
        let mut stdout = std::io::stdout().lock();
        stdout.write_all(preview.as_bytes())?;
        stdout.flush()?;
    }

    let sign_prompt = "\nPress Enter to sign locally (or Ctrl-C to cancel): ";
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let confirmation = prompt_line(&mut stdout, &mut stdin, sign_prompt)?;

    if confirmation.is_none() {
        // EOF before confirming — clean cancel. NO side effect has happened
        // (KPI-5 / PA-2 local-first): no sign, no write, no PDS call.
        return Ok(PhilosophyAddOutcome {
            exit_code: 0,
            stdout: String::new(),
        });
    }
    drop(stdout);
    drop(stdin);

    // Step 5: sign — canonical bytes → CID → IdentityPort::sign (ADR-006 reused
    // verbatim, no new signing model).
    let unsigned = UnsignedPhilosophy {
        philosophy: &record,
        object_id: &object_id,
        author: &author_did,
        composed_at: &composed_at,
    };
    let canonical_bytes =
        serde_json::to_vec(&unsigned).map_err(|e| anyhow!("canonicalizing philosophy: {e}"))?;
    let cid = compute_cid(&canonical_bytes);

    let signature = wiring
        .identity
        .sign(&cid)
        .map_err(|e| anyhow!("signing philosophy: {e}"))?;

    let signed = SignedPhilosophy {
        philosophy: record,
        object_id: object_id.clone(),
        // The author DID is embedded on the signed record so the artifact is
        // self-describing off the DB (PA-5 / AC-003.2).
        author_did: Did(author_did),
        composed_at,
        signature,
    };

    // Persist: atomic signed `<cid>.json` artifact + the philosophies row.
    wiring
        .storage
        .write_signed_philosophy(&signed)
        .with_context(|| format!("persisting signed philosophy {object_id} to local store"))?;

    let artifact_path = wiring
        .paths
        .philosophies_dir()
        .join(format!("{}.json", signed.signature.signed_cid.0));

    let mut out = String::new();
    out.push_str(&format!("Minted philosophy: {object_id}\n"));
    out.push_str(&format!("Written to local store: {}\n", artifact_path.display()));

    Ok(PhilosophyAddOutcome {
        exit_code: 0,
        stdout: out,
    })
}
