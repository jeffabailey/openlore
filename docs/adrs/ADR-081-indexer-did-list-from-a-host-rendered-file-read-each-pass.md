# ADR-081: The DID List Comes From a Host-Rendered File That Each Pass Reads, With the Last Good Copy Kept by the Host

- **Status**: Proposed (2026-10-06)
- **Date**: 2026-10-06
- **Deciders**: Morgan (nw-solution-architect); DEVOPS owns the host script
- **Feature**: indexer-deployment (DESIGN). Resolves OQ-IXD-3.
- **Builds on**: ADR-078 §6 (config refusal names the variable and the bad entry), ADR-075 (secrets
  rendered on the host, never AWS credentials in a container), ADR-080 (passes run inside `serve`)

## Context

The DID list is a static SSM parameter that the operator edits (WD-IXD-4). An edit must take effect on
the next pass with no redeploy and no restart (FR-IXD-6). Today the list is read once, at process start,
from `OPENLORE_INDEXER_REPO_DIDS`. Under ADR-080 the process is a long-running `serve`, so a startup-only
read would need a restart for every edit. The container must not hold AWS credentials
(NFR-IXD-8, IMDS hop limit 1). A read failure must reuse the last good list and never run with an empty
one (FR-IXD-7). A malformed list must refuse the pass with exit 2, naming the bad entry, while search
keeps serving (FR-IXD-8).

## Decision

1. **Parameter.** `/openlore/prod/indexer/repo-dids` is a **Standard `String`** parameter. It is not a
   `SecureString`: DIDs are public, so `kms:Decrypt` is not needed and the host role needs only
   `ssm:GetParameter` on `/openlore/prod/indexer/*`. The value uses the existing format (DIDs separated by
   commas or whitespace).
2. **Host renders, and keeps the last good copy (availability).** Before every pass, the timer's
   service runs a host script (DEVOPS: `render-dids.sh`, as `ExecStartPre`). It reads the parameter with
   the instance role, then:
   1. writes a **staging file** in the **same directory** (`/pds/indexer/config/.repo-dids.new`);
   2. sets it to `chmod 0444`;
   3. `rename(2)`s the **file** over `/pds/indexer/config/repo-dids`.

   It must **never swap the directory**. Unlike `render-secrets.sh` (lines 21 and 56-58), which moves
   the whole secrets directory aside and puts a new one in place, it must not move the directory:
   the container's bind mount resolves the directory once, at container start, and keeps the old
   directory inode. A directory swap would leave the running `serve` reading a moved or deleted
   directory until its next restart. A file rename inside the mounted directory is visible to the
   container immediately.
   - **On any read failure** (SSM unreachable, throttled, access denied) the script leaves the existing
     file untouched and **exits 0**, so the pass still runs on the last good list.
   - **Staleness is visible (M5).** On a successful render the script touches a stamp file,
     `/pds/indexer/config/.rendered-at`. On a failure it ships `indexer.dids.render_failed {cause}`
     to the indexer log group (same mechanism as `health-timer.sh` `emit`).
   - The binary adds `repo_dids_age_secs` (the list file's mtime age) to each pass's
     `indexer.config.loaded`.
   - The host health timer reports `dids_stale=1` once the stamp is older than **2 h** (8 failed
     renders). That is one of the inputs to the existing **liveness alarm A3**, so it adds no alarm.
   - The script does **not** validate DIDs. Validity is the binary's job (point 4), so there is one
     source of truth for DID syntax.
3. **Directory mount, not file mount.** The container mounts `/pds/indexer/config` **read-only, as a
   directory**, at `/config`. A single-file bind mount pins the original inode, so an atomic rename on
   the host would never be seen. This is a probed environment lie (see Earned Trust).
4. **The binary reads the file at the start of every pass (validity).** New variable
   `OPENLORE_INDEXER_REPO_DIDS_FILE` (a path). Each pass:
   1. reads the file and parses it with the **existing pure `parse_repo_dids`** (the same syntax, order and
      de-duplication rules as the env variable);
   2. emits `indexer.config.loaded`, carrying `pass_id` and `repo_dids_source: "file"`, so the story's
      `repo_did_count: 13` is visible per pass.

   Outcomes:

   | File state | Pass outcome |
   |---|---|
   | Valid, non-empty | Normal pass |
   | Valid but empty (whitespace only) | Today's "no DIDs configured" pass (exit 0). Purge is suppressed (ADR-082). |
   | Malformed entry | The pass is **refused**: `indexer.ingest.pass_refused {variable: "OPENLORE_INDEXER_REPO_DIDS_FILE", value: <bad entry>}`, then `pass_summary {exit_code: 2}`. Nothing is fetched or purged, and the index keeps serving. |
   | Missing or unreadable | The pass is refused the same way (exit 2, `cause: "repo_dids_unreadable"`). The host never deletes the file, so this means a deployment fault. |
5. **`serve` startup does not refuse on the list.** A bad or missing file at startup refuses only the
   passes, never search. A deploy during a malformed-list window must not take search down (FR-IXD-8).
   The one-shot `ingest` verb keeps the ADR-078 §6 behavior (`health.startup.refused`, exit 2).
6. **Mutual exclusion.** If both `OPENLORE_INDEXER_REPO_DIDS` and `OPENLORE_INDEXER_REPO_DIDS_FILE` are
   set, the configuration is refused at startup (exit 2, naming both). The env variable stays for local
   and test use.

## Alternatives considered

| Alternative | Evaluation | Verdict |
|---|---|---|
| **Restart `serve` after each SSM render (env variable unchanged)** | No binary change for the list. But every pass would restart search, which breaks the deploy-only downtime budget and AC-003.1 ("no restart"). It would also abandon any pass in progress. | Rejected |
| **The container reads SSM itself** | It needs AWS credentials in a public-facing container, which NFR-IXD-8 and ADR-075 forbid. | Rejected |
| **SecureString parameter** | It adds `kms:Decrypt` to the host role for data that is public. | Rejected |
| **The host validates DIDs and refuses to render a malformed list** | It duplicates DID syntax in bash (two sources of truth), and a malformed edit would be silently ignored with no exit 2 and no email, which violates FR-IXD-8 and AC-004.2. | Rejected |
| **The binary keeps the last good list (in memory or on disk)** | It mixes availability into the binary, and an in-memory copy is lost on restart. The host already owns the SSM read, so keeping the old file is free. | Rejected |
| **Watch the file (inotify) instead of reading it per pass** | The pass is the only consumer, so reading per pass is simpler and portable, and needs no watcher thread. | Rejected |

## Review-app `render-secrets.sh` assessment (same pinned-inode question)

`render-secrets.sh` swaps the secrets **directory**. The running review-app container keeps the old,
now moved and then deleted, directory until its next start. This is **not broken today**:

- the review app reads its secrets **only at startup** (`wiring.rs` `read_secrets`, including
  `data-key-previous` for rotation);
- `deploy.sh` always restarts or recreates the container after rendering;
- docker re-resolves a bind-mount source path on every container start.

The constraint, which DEVOPS must document in the review-app runbook, is that **rotating a secret
requires a container restart. Rendering alone does nothing.** Between the render and the restart, the
container's `/run/secrets` points at a deleted directory, so any future lazy secret read would fail.

Recommended fix (low cost, DEVOPS): render per file with a rename, as above. Keep the directory and
rename each `<name>.new` over `<name>`, so the mount always shows current content. Separately, add a
check-arch or AT guard that secrets are read only in review-app wiring, which makes "restart to
rotate" an enforced property rather than an accident.

## Consequences

- **Positive**:
  - An edit takes effect on the next pass, with no restart and no credentials in the container.
  - A read failure reuses the last good list by construction (the host never truncates the file).
  - One DID parser.
- **Negative**:
  - It adds one config variable and one per-pass file read.
  - If the host has never rendered the file (a first deploy with SSM unreachable), every pass is
    refused with exit 2 and an email. That is the correct alarm.
- **Purge interaction**: a malformed, missing or unreadable list never reaches the purge step (ADR-082).

## Earned Trust (required fault-injection scenarios)

| Lie or fault | Required behavior |
|---|---|
| SSM unreachable at render | The old file is unchanged and the pass runs on the last good list (AC-003.4). |
| **Two consecutive edits** rendered by file rename while one `serve` runs (no restart) | Pass N sees edit 1, and pass N+1 sees edit 2 (`repo_did_count` changes both times). This proves a directory mount with a file rename, and fails if either a file mount or a directory swap is used. |
| SSM failing for more than 2 h | `render_failed` lines in CloudWatch, a growing `repo_dids_age_secs`, `dids_stale=1`, and A3 fires |
| A file with a malformed entry (`tomas`) | `pass_refused` names the variable and `tomas`, then `exit_code: 2`. Searches still answer from the prior index (AC-003.3). |
| The file is missing or `chmod 000` | The pass is refused with exit 2. Search is unaffected. |
| A file with a UTF-8 BOM or CRLF | CRLF splits as whitespace. A BOM makes the first entry malformed, so exit 2 names it (it fails loud, never silently drops a DID). |
