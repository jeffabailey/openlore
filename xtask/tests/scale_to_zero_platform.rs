//! scale-to-zero-hosting (DISTILL 2026-10-30) — the Operator driving port:
//! `deploy.sh lambda …` run for REAL under `bash` with recording stubs first on
//! `PATH`, the pattern of `indexer_deployment_platform.rs` (DWD-IXD-9), which
//! DEVOPS named for these verbs (§ CI/CD pipeline outline: ref validation, gate
//! order, refusal of a non-Lambda-capable digest, the exit-3 rule, rollback
//! target selection). Nothing reaches AWS, GitHub or a registry. Layer 3,
//! example-only (Mandate 9/11).
//!
//! | ID | Contract | Source |
//! |---|---|---|
//! | OP-1 | `lambda deploy`: the exact gate order; a no-publish pass gate; `live` flips last, each with `prev=`, and the pass with `idx=` | DV-STZ-9, D-STZ-26, D-STZ-44 |
//! | OP-2 | `lambda rollback`: `live` goes back to the recorded `prev=` version | D-STZ-26 |
//! | OP-3 | `lambda stop` / `start`: search concurrency 0 / 3, and the distribution off / on | D-STZ-18d |
//! | OP-4 | Edge: one allowlist function on all three behaviors, every method allowed, `enabled` left to the kill switch | U-STZ-4, D-STZ-18d |
//! | OP-5 | Posture: 7-day index history, 0 async retries, one pass at a time | U-STZ-5, DV-STZ-11 |
//! | OP-6 | Search concurrency is 3 and an apply cannot undo a stop | D-STZ-18a |
//! | OP-7 | `lambda republish`: `$LATEST` set to the live digest first | U-STZ-2 |
//! | OP-8 | `lambda restore-index`: one copy from the chosen version | D-STZ-27 |
//! | OP-9 | `lambda pass-off`/`on`, `probe-off`/`on`: that function only | DEVOPS verbs |
//! | OP-10 | Both schedules send `{}` | U-STZ-7 |
//! | OP-11 | Each verb sends its exact event | D-STZ-44, U-STZ-7 |
//! | OP-12 | No `lambda` verb reaches the host | KPI-IXD-6 attribution |
//! | OP-13 | `standby --timers` and `--container` stop what they name | U-STZ-1 |
//! | OP-14 | Schedules are disabled until the operator enables them | D-STZ-26 |
//! | OP-15 | The registry keeps 10 images | U-STZ-9 |
//! | OP-16 | The probe's base URL does not change with the phase | DEVOPS |
//! | OP-17 | `lambda status` looks for A11's metric on the candidate series | DEVOPS metric check |
//! | OP-E1 | A tag, branch or short sha is refused before any tool runs | DV-STZ-9 step 1 |
//! | OP-E2 | A digest without the Lambda root is refused before ECR | DV-STZ-9 step 5 |
//! | OP-E3 | A pass gate ending in exit 2 stops the deploy; nothing flips | step 12 |
//! | OP-E4 | Exit 3 stops the deploy unless the operator accepts it | step 12 |
//! | OP-E5 | A rollback target whose image left ECR is refused | DEVOPS rollback table |
//! | OP-E6 | `lambda reset-index` deletes nothing without the exact word `reset` | D-STZ-27 |
//! | OP-E7 | A gate reply with no `mode` stops the deploy, whatever else it says | D-STZ-44 |
//! | OP-E8 | Accepting exit 3 changes only exit 3 (C5b) | step 12 |
//! | OP-E9 | `not_live = 1` stops the deploy unless accepted, and that changes only this check | step 13 |
//! | OP-E10 | A rollback with no recorded version is refused (C4b) | D-STZ-26 |
//! | OP-E11 | A republish whose `$LATEST` does not match the live digest is refused | U-STZ-2 |
//! | OP-E12 | `lambda restore-index` is refused in the quarter-hour edge minutes | DEVOPS verbs |
//! | OP-E13 | A gate waits for a running pass or probe, then gives up with nothing flipped | steps 12, 13 |
//! | OP-E14 | A republish whose live digest left ECR is refused | U-STZ-2 |
//! | OP-E15 | A failed edge check flips search back and leaves pass and probe alone | step 14 |
//! | OP-E16 | A failed candidate check stops the deploy, so a deploy cannot undo a stop | step 11, `lambda start` |
//! | OP-E17 | A republish that cannot read the live image is refused, with no `prev=` fallback | U-STZ-2 |
//! | OP-E18 | A copy that would evict a referenced image is refused | U-STZ-9 |
//! | OP-E19 | A failed deploy deletes the copy it made, and only that | U-STZ-9 |
//! | OP-E20 | A stop before the flip moves `candidate` back | steps 12, 13 |
//! | OP-E21 | The errors drill waits out a busy probe | `lambda test-alarm errors` |
//! | OP-E22 | `standby` needs a flag | U-STZ-1 |
//! | OP-E23 | A publish that changed nothing is refused before anything moves | review B1 |
//! | OP-E24 | A failed flip of the pass or the probe undoes what flipped, and says where each is | review H1 |
//! | OP-E25 | Only a missing object reads as an empty bucket; any other failed read stops the verb | review H2 |
//! | OP-18 | `standby --timers` keeps the timers off through a reboot | review H6 |
//! | OP-E26 | A red static-checks run on the commit stops a deploy; a commit with no run does not | review M2 |
//!
//! ## Testability contract for `deploy.sh lambda`
//!
//! * It resolves `aws`, `gh`, `cosign`, `crane`, `curl`, `date` and `sleep` from
//!   `PATH`. `restore-index` reads the minute from `date -u +%M`.
//! * It sends a Lambda payload inline, as `--payload '{"mode":"no-publish"}'`,
//!   and names the output file last on the `lambda invoke` line.
//! * A gate waits with `sleep 30` and counts attempts, so stubbing `sleep` also
//!   stubs the wait.
//! * It writes a distribution config to a file and passes it as `file://…`, or
//!   inline. The stub records either, so a scenario can read `Enabled`.
//! * The tofu root declares each function as its own `aws_lambda_function` with a
//!   literal `function_name`, each cache behavior as its own block, and each
//!   schedule's `input` in its `target` block. No `for_each` or `dynamic` there:
//!   the reader below reads settings, not expressions.
//! * The stubs answer in JSON. If DELIVER reads a field with `--query`, it
//!   adjusts the stub's output; the order and refusal assertions stay.
//!
//! The other verbs (`status`, `kpi`, `measure`, `parity`, `seed`, `cutover-watch`,
//! the other `test-alarm` targets) and the host's `resume` stay with the runbook;
//! see the DISTILL driving-adapter coverage table.
//!
//! These scenarios use operator words (live alias, registry, the verbs): their
//! readers are operators (wave-decisions, the language exemption).
#![cfg(unix)]

use std::collections::BTreeSet;
use std::fmt::Debug;
use std::io::Write as _;
use std::os::unix::fs::PermissionsExt;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const DEPLOY: &str = "deploy/indexer/deploy.sh";
const SHA: &str = "4f2c1ab9e8d7c6b5a4f3e2d1c0b9a8f7e6d5c4b3";
const FUNCTIONS: [&str; 3] = [
    "openlore-indexer-search",
    "openlore-indexer-pass",
    "openlore-indexer-probe",
];
const PROD_INDEXER_ROOT: &str = "deploy/tofu/environments/prod-indexer";
const ALL_METHODS: [&str; 7] = ["DELETE", "GET", "HEAD", "OPTIONS", "PATCH", "POST", "PUT"];
/// What `/healthz` says it serves: the object's `published-at` (D-STZ-12).
const SERVED_AT: &str = "2026-10-27T13:45:01Z";
const PASS_GATE: [&str; 2] = ["lambda invoke", "openlore-indexer-pass"];
const ACCEPT_EXIT_3: (&str, &str) = ("LAMBDA_ACCEPT_EXIT_3", "1");
const ACCEPT_NOT_LIVE: (&str, &str) = ("LAMBDA_ACCEPT_NOT_LIVE", "1");
/// A gate is throttled for ever.
const ALWAYS: u32 = u32::MAX;
/// How a stub logs its call: the pollers run beside the calls they would split.
const RECORD_CALL: &str = include_str!("support/record_call.sh");

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn digest() -> String {
    format!("sha256:{}", "a".repeat(64))
}

/// The hex a Lambda version reports as `CodeSha256` for the image at [`digest`].
fn code_sha() -> String {
    "a".repeat(64)
}

/// Run one outline's rows, and report every row that fails rather than stopping
/// at the first, so one failure does not hide the others.
fn each_row<T: Debug>(rows: impl IntoIterator<Item = T>, row: impl Fn(&T)) {
    let failed: Vec<String> = rows
        .into_iter()
        .filter_map(|case| {
            catch_unwind(AssertUnwindSafe(|| row(&case)))
                .err()
                .map(|cause| {
                    let text = cause
                        .downcast_ref::<String>()
                        .cloned()
                        .or_else(|| cause.downcast_ref::<&str>().map(|s| (*s).to_string()))
                        .unwrap_or_default();
                    format!("{case:?}: {text}")
                })
        })
        .collect();
    assert!(
        failed.is_empty(),
        "{} row(s) failed:\n{}",
        failed.len(),
        failed.join("\n")
    );
}

/// Who still needs an image (U-STZ-9: the guard's reference set).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Holder {
    Nobody,
    Live,
    Latest,
}

/// What the stubbed world looks like.
#[derive(Clone, Copy, Debug)]
struct Posture {
    /// The exit code the new pass version answers the pass gate with.
    pass_gate_exit: i32,
    /// Whether the new pass version says it ran as no-publish (D-STZ-44).
    echoes_mode: bool,
    /// Whether the pass gate ends in a function error.
    pass_gate_crashes: bool,
    /// What the new probe version's health line says.
    not_live: u8,
    /// How many pass and probe gate invokes are throttled before one runs.
    throttled_pass_gates: u32,
    throttled_probe_gates: u32,
    /// Whether the signed image has the Lambda root.
    lambda_capable: bool,
    /// Whether ECR still holds the image (of the deploy, or the rollback target).
    image_in_ecr: bool,
    /// Whether each `live` alias records the version it replaced.
    alias_records_prev: bool,
    /// Whether `$LATEST` reports the digest it was just given.
    code_sha_matches: bool,
    /// What `date -u +%M` says.
    minute: &'static str,
    /// What the candidate URL answers.
    candidate_status: u16,
    /// What the public edge answers once search has flipped.
    edge_after_the_flip: u16,
    /// What the distribution's current config says.
    distribution_enabled: bool,
    /// The index object's `published-at`; the candidate serves [`SERVED_AT`].
    object_published_at: &'static str,
    /// Whether the `live` version's image can be read.
    live_image_readable: bool,
    /// What the oldest `sha-*` image, next to be evicted, is held by.
    oldest_image: Holder,
    /// Whether `$LATEST` already holds the digest being deployed.
    latest_holds_the_build: bool,
    /// Whether `list-metrics` has seen A11's candidate series.
    candidate_5xx_seen: bool,
    /// Whether `update-function-code` fails.
    code_update_fails: bool,
    /// Whether `live`, `candidate` and the `prev=` record hold the digest being
    /// deployed. Otherwise they hold another one, so the deploy's own digest is
    /// referenced only where a scenario says so.
    live_runs_the_build: bool,
    /// What the search function's reserved concurrency reads.
    search_concurrency: u8,
    /// For each function, in [`FUNCTIONS`] order: whether its publish changed nothing, so
    /// Lambda answers with the version `live` already holds.
    publish_answers_live: [bool; 3],
    /// The `live` moves that fail, each as (index into [`FUNCTIONS`], target version).
    live_moves_that_fail: &'static [(usize, u32)],
    /// From which `head-object` call on the index it fails, and with what on stderr.
    index_head_fails: Option<(u32, &'static str)>,
}

impl Default for Posture {
    fn default() -> Self {
        Self {
            pass_gate_exit: 0,
            echoes_mode: true,
            pass_gate_crashes: false,
            not_live: 0,
            throttled_pass_gates: 0,
            throttled_probe_gates: 0,
            lambda_capable: true,
            image_in_ecr: false,
            alias_records_prev: true,
            code_sha_matches: true,
            minute: "07",
            candidate_status: 200,
            edge_after_the_flip: 200,
            distribution_enabled: true,
            object_published_at: SERVED_AT,
            live_image_readable: true,
            oldest_image: Holder::Nobody,
            latest_holds_the_build: false,
            candidate_5xx_seen: true,
            code_update_fails: false,
            live_runs_the_build: true,
            search_concurrency: 3,
            publish_answers_live: [false; 3],
            live_moves_that_fail: &[],
            index_head_fails: None,
        }
    }
}

struct Stubs {
    dir: tempfile::TempDir,
}

impl Stubs {
    fn new(posture: Posture) -> Self {
        let stubs = Self {
            dir: tempfile::tempdir().expect("stub dir"),
        };
        let digest = digest();
        stubs.add("gh", "echo success");
        stubs.add("cosign", "exit 0");
        let labels = if posture.lambda_capable {
            format!(
                r#"{{"org.opencontainers.image.revision":"{SHA}","org.openlore.indexer.lambda-root":"1"}}"#
            )
        } else {
            format!(r#"{{"org.opencontainers.image.revision":"{SHA}"}}"#)
        };
        let working_dir = if posture.lambda_capable {
            "/"
        } else {
            "/home/nonroot"
        };
        stubs.add(
            "crane",
            &format!(
                r#"case "$1" in
  digest) echo {digest} ;;
  config) echo '{{"config":{{"WorkingDir":"{working_dir}","Labels":{labels}}}}}' ;;
  *) exit 0 ;;
esac"#
            ),
        );
        stubs.add(
            "date",
            &format!(
                r#"case "$*" in *%M*) echo {} ;; *) exec /bin/date "$@" ;; esac"#,
                posture.minute
            ),
        );
        // `sleep 30` is a gate's wait: it does not wait, and a loop that never
        // gives up is ended here. Any other sleep is the flip poller's.
        stubs.add(
            "sleep",
            r#"if [ "$1" = 30 ] && [ "$(grep -c '^sleep 30' "$CALLS")" -gt 40 ]; then kill "$PPID"; fi
/bin/sleep 0.01"#,
        );
        stubs.add("curl", &curl(posture));
        stubs.add("systemctl", "exit 0");
        stubs.add("docker", "exit 0");
        stubs.add("aws", &aws(posture, &digest));
        stubs
    }

    fn add(&self, name: &str, body: &str) {
        let path = self.dir.path().join(name);
        let log = self.dir.path().join("calls.log");
        std::fs::write(
            &path,
            format!(
                "#!/usr/bin/env bash\nCALLS='{}'\n{RECORD_CALL}\nrecord_call \"{name} $*\"\n{body}\n",
                log.display()
            ),
        )
        .expect("write stub");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
            .expect("chmod stub");
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("calls.log")).unwrap_or_default()
    }

    /// Run `deploy.sh <args>` with `input` on stdin.
    fn run(&self, args: &[&str], env: &[(&str, &str)], input: &str) -> Run {
        let path = format!(
            "{}:{}",
            self.dir.path().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut child = Command::new("bash")
            .arg(root().join(DEPLOY))
            .args(args)
            .env("PATH", path)
            .env("HOME", self.dir.path())
            // `instance_id` reads this first, so a host verb never reaches the real tofu.
            .env("INDEXER_INSTANCE_ID", "i-test")
            .envs(env.iter().copied())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("run bash");
        // A verb that reads nothing can be gone before this write lands.
        match child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.as_bytes())
        {
            Err(cause) if cause.kind() == std::io::ErrorKind::BrokenPipe => {}
            written => written.expect("write stdin"),
        }
        let output = child.wait_with_output().expect("wait for bash");
        Run {
            status: output.status.code().unwrap_or(-1),
            out: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        }
    }
}

struct Run {
    status: i32,
    out: String,
}

/// The candidate URL and the public edge, as the post-checks see them.
fn curl(posture: Posture) -> String {
    let (candidate, edge) = (posture.candidate_status, posture.edge_after_the_flip);
    format!(
        r#"
args="$*"
url=""
size=0
for a in "$@"; do
  case "$a" in
    http*) url="$a" ;;
    @*) size=$(wc -c <"${{a#@}}") ;;
  esac
done
status=200
case "$url" in
  *_health) ;;
  *createRecord*) status=404 ;;
  */healthz) case "$args" in *"-X POST"*) status=404 ;; esac ;;
  */xrpc/org.openlore.appview.searchClaims) [ "$size" -le 8192 ] || status=413 ;;
  *) status=404 ;;
esac
case "$url" in
  *candidate.lambda-url*) [ {candidate} = 200 ] || status={candidate} ;;
  *)
    if grep -q 'update-alias.*openlore-indexer-search.*live' "$CALLS"; then
      [ {edge} = 200 ] || status={edge}
    fi ;;
esac
case "$args" in
  *http_code*) printf '%s' "$status"; exit 0 ;;
esac
case "$url" in
  */healthz) echo '{{"status":"ok","last_successful_pass_at":"{SERVED_AT}"}}' ;;
  *) echo '{{"results":[]}}' ;;
esac
"#
    )
}

/// The shell test of a throttled gate: `$CALLS` already holds this call.
fn throttled_shell(function: &str, throttled: u32) -> String {
    format!(
        r#"attempt=$(grep -c 'lambda invoke.*{function}' "$CALLS")
    if [ "$attempt" -le {throttled} ]; then
      echo 'An error occurred (TooManyRequestsException)' >&2; exit 254
    fi"#
    )
}

/// AWS, answering what each verb asks.
fn aws(posture: Posture, digest: &str) -> String {
    let in_ecr = if posture.image_in_ecr {
        "true"
    } else {
        "false"
    };
    let pass_exit = posture.pass_gate_exit;
    let mode = if posture.echoes_mode {
        r#","mode":"no-publish","would_publish":false"#
    } else {
        ""
    };
    let not_live = posture.not_live;
    let pass_throttle = throttled_shell(FUNCTIONS[1], posture.throttled_pass_gates);
    let probe_throttle = throttled_shell(FUNCTIONS[2], posture.throttled_probe_gates);
    let pass_answer = if posture.pass_gate_crashes {
        r#"echo '{"errorMessage":"crashed"}' >"$last"
    echo '{"StatusCode":200,"FunctionError":"Unhandled"}'"#
            .to_string()
    } else {
        format!(
            r#"echo '{{"pass_id":"boot-1","exit_code":{pass_exit},"published":false,"index_bytes":0{mode}}}' >"$last"
    echo '{{"StatusCode":200}}'"#
        )
    };
    let description = if posture.alias_records_prev {
        format!("prev=6 {SHA} {digest} 2026-10-27T14:00:00Z")
    } else {
        format!("{SHA} {digest} 2026-10-27T14:00:00Z")
    };
    let live_sha = code_sha();
    let latest_sha = if posture.code_sha_matches {
        code_sha()
    } else {
        "b".repeat(64)
    };
    let repo = "091153021562.dkr.ecr.us-east-1.amazonaws.com/openlore-indexer";
    let drifted = format!("sha256:{}", "c".repeat(64));
    let enabled = posture.distribution_enabled;
    let referenced = if posture.oldest_image == Holder::Live || !posture.live_runs_the_build {
        format!("sha256:{}", "e".repeat(64))
    } else {
        digest.to_string()
    };
    let description = description.replace(digest, &referenced);
    let latest_image = if posture.latest_holds_the_build {
        digest.to_string()
    } else {
        drifted.clone()
    };
    let images = image_list(match posture.oldest_image {
        Holder::Nobody => "sha256:0000",
        Holder::Live => &referenced,
        Holder::Latest => &drifted,
    });
    let candidate_series = if posture.candidate_5xx_seen {
        r#"{"Metrics":[{"MetricName":"Url5xxCount","Dimensions":[{"Name":"Resource","Value":"openlore-indexer-search:candidate"}]}]}"#
    } else {
        r#"{"Metrics":[]}"#
    };
    let live_image = if posture.live_image_readable {
        format!(
            r#"echo '{{"Configuration":{{"Version":"7","Description":"{SHA} {referenced}","CodeSha256":"{live_sha}","RevisionId":"r","State":"Active","LastUpdateStatus":"Successful"}},"Code":{{"ImageUri":"{repo}@{referenced}"}}}}'"#
        )
    } else {
        "echo 'ResourceNotFoundException' >&2; exit 254".to_string()
    };
    let code_update = if posture.code_update_fails {
        "echo 'ServiceException' >&2; exit 254".to_string()
    } else {
        format!(r#"echo '{{"CodeSha256":"{latest_sha}","RevisionId":"r"}}'"#)
    };
    let published_at = posture.object_published_at;
    let search_concurrency = posture.search_concurrency;
    let publishes = FUNCTIONS
        .iter()
        .zip(posture.publish_answers_live)
        .map(|(function, unchanged)| {
            let version = if unchanged { 7 } else { 8 };
            format!(
                r#"*"lambda publish-version"*{function}*) echo '{{"Version":"{version}","State":"Active"}}' ;;"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n  ");
    let failed_moves: String = posture
        .live_moves_that_fail
        .iter()
        .map(|(function, version)| {
            let function = FUNCTIONS[*function];
            format!(
                r#"  *"update-alias"*{function}*"--name live --function-version {version} "*)
    echo 'ServiceException' >&2; exit 254 ;;
"#
            )
        })
        .collect();
    let index_head_fault = posture
        .index_head_fails
        .map_or_else(String::new, |(from, text)| {
            format!(
                r#"if [ "$(grep -c 's3api head-object' "$CALLS")" -ge {from} ]; then
      echo '{text}' >&2; exit 254
    fi"#
            )
        });
    format!(
        r#"
in_ecr={in_ecr}
for last in "$@"; do :; done
for a in "$@"; do
  case "$a" in file://*) record_call "$(tr -d ' \n' <"${{a#file://}}")" ;; esac
done
case "$*" in
{failed_moves}  *"sts get-caller-identity"*) echo '{{"Account":"091153021562"}}' ;;
  *"ecr describe-images"*"--image-ids"*)
    if [ "$in_ecr" = true ]; then
      echo '{{"imageDetails":[{{"imageDigest":"{digest}"}}]}}'
    else
      echo 'ImageNotFoundException' >&2; exit 254
    fi ;;
  *"ecr describe-images"*) echo '{images}' ;;
  *"lambda get-alias"*)
    echo '{{"FunctionVersion":"7","Description":"{description}"}}' ;;
  *"lambda get-function-url-config"*)
    echo '{{"FunctionUrl":"https://candidate.lambda-url.us-east-1.on.aws/"}}' ;;
  *"lambda get-function-configuration"*)
    echo '{{"Version":"7","Description":"{SHA} {digest}","CodeSha256":"{latest_sha}","RevisionId":"r","State":"Active","LastUpdateStatus":"Successful","LoggingConfig":{{"LogGroup":"/openlore/prod/indexer-lambda"}}}}' ;;
  *"lambda get-function-concurrency"*openlore-indexer-search*)
    echo '{{"ReservedConcurrentExecutions":{search_concurrency}}}' ;;
  *"lambda get-function-concurrency"*) echo '{{"ReservedConcurrentExecutions":1}}' ;;
  *"lambda get-function "*"--qualifier"*)
    {live_image} ;;
  *"lambda get-function "*)
    echo '{{"Configuration":{{"Version":"$LATEST","CodeSha256":"{latest_sha}","RevisionId":"r","State":"Active","LastUpdateStatus":"Successful"}},"Code":{{"ImageUri":"{repo}@{latest_image}"}}}}' ;;
  *"lambda update-function-code"*) {code_update} ;;
  {publishes}
  *"lambda invoke"*openlore-indexer-pass*)
    {pass_throttle}
    {pass_answer} ;;
  *"lambda invoke"*openlore-indexer-probe*)
    {probe_throttle}
    case "$*" in
      *"--payload []"*)
        echo '{{"errorMessage":"not an object"}}' >"$last"
        echo '{{"StatusCode":200,"FunctionError":"Unhandled"}}'
        exit 0 ;;
    esac
    echo '{{}}' >"$last"
    log=$(printf '%s\n' '{{"event":"indexer.host.health","not_live":{not_live}}}' | base64 | tr -d '\n')
    echo "{{\"StatusCode\":200,\"LogResult\":\"$log\"}}" ;;
  *"s3api head-object"*)
    {index_head_fault}
    echo '{{"ETag":"\"e\"","Metadata":{{"published-at":"{published_at}"}},"VersionId":"v1","ContentLength":3145728}}' ;;
  *"s3api delete-object"*) echo '{{"VersionId":"v2","DeleteMarker":true}}' ;;
  *"s3api copy-object"*) echo '{{"VersionId":"v3"}}' ;;
  *"cloudwatch describe-alarms"*)
    seen=$(grep -c 'cloudwatch describe-alarms' "$CALLS")
    if [ "$seen" -le 1 ]; then state=ALARM; else state=OK; fi
    case "$*" in
      *--query*) echo "$state" ;;
      *) echo "{{\"MetricAlarms\":[{{\"StateValue\":\"$state\"}}]}}" ;;
    esac ;;
  *"cloudwatch describe-alarm-history"*) echo '{{"AlarmHistoryItems":[]}}' ;;
  *"logs get-query-results"*) echo '{{"status":"Complete","results":[]}}' ;;
  *"cloudwatch list-metrics"*Url5xxCount*candidate*) echo '{candidate_series}' ;;
  *"cloudwatch list-metrics"*) echo '{{"Metrics":[]}}' ;;
  *"ec2 describe-instances"*) echo i-0123456789abcdef0 ;;
  *"ssm send-command"*) echo cmd-1 ;;
  *"ssm get-command-invocation"*) echo Success ;;
  *"cloudfront list-distributions"*)
    echo '{{"DistributionList":{{"Items":[{{"Id":"EDIST","DomainName":"dedge.cloudfront.net","Aliases":{{"Items":["index.openlore.jeffbailey.us"]}}}}]}}}}' ;;
  *"cloudfront get-distribution-config"*)
    echo '{{"ETag":"ECONFIG","DistributionConfig":{{"Enabled":{enabled},"Comment":"index"}}}}' ;;
  *) echo '{{}}' ;;
esac
"#
    )
}

/// Ten `sha-*` images, the registry's limit, oldest first, the oldest being `oldest`.
fn image_list(oldest: &str) -> String {
    let older = format!(
        r#"{{"imageDigest":"{oldest}","imageTags":["sha-old"],"imagePushedAt":1790000000}}"#
    );
    let newer: Vec<String> = (1..10)
        .map(|n| {
            format!(
                r#"{{"imageDigest":"sha256:{}","imageTags":["sha-{n}"],"imagePushedAt":{}}}"#,
                n.to_string().repeat(64),
                1_790_000_000 + n * 1000
            )
        })
        .collect();
    format!(r#"{{"imageDetails":[{older},{}]}}"#, newer.join(","))
}

/// The word after `--payload` on a call line.
fn payload_of(line: &str) -> Option<&str> {
    let mut words = line.split_whitespace();
    words.find(|w| *w == "--payload")?;
    words.next()
}

/// The line of the first call that holds every token.
fn line_of(calls: &str, tokens: &[&str]) -> Option<usize> {
    calls
        .lines()
        .position(|line| tokens.iter().all(|token| line.contains(token)))
}

fn called(calls: &str, tokens: &[&str]) -> bool {
    line_of(calls, tokens).is_some()
}

/// A deploy that stopped: it failed, and no `live` alias moved.
fn assert_stopped_with_nothing_flipped(run: &Run, stubs: &Stubs) {
    assert_ne!(run.status, 0, "{}\n{}", run.out, stubs.calls());
    let calls = stubs.calls();
    assert!(!called(&calls, &["update-alias", "live"]), "{calls}");
}

/// The deploy got as far as this step, so a stop means the step stopped it. With
/// no `lambda` verb, "nothing flipped" alone would hold for the usage message.
fn assert_reached(stubs: &Stubs, step: &[&str]) {
    let calls = stubs.calls();
    assert!(called(&calls, step), "never reached {step:?}\n{calls}");
}

/// The stop came from a check that says so, not from something falling over.
fn assert_refused(run: &Run) {
    assert!(run.out.contains("refusing"), "{}", run.out);
}

/// A deploy that reached the end: every `live` alias moved.
fn assert_flipped(run: &Run, stubs: &Stubs) {
    assert_eq!(run.status, 0, "{}\n{}", run.out, stubs.calls());
    let calls = stubs.calls();
    for function in FUNCTIONS {
        assert!(
            called(&calls, &["update-alias", function, "live"]),
            "{calls}"
        );
    }
}

// -----------------------------------------------------------------------------
// Reading the tofu root
// -----------------------------------------------------------------------------

/// Every `.tf` file of the new root, with commented lines gone.
fn prod_indexer_tofu() -> String {
    uncommented(&root_files(|path| {
        path.extension().is_some_and(|ext| ext == "tf")
    }))
}

/// HCL with its whole-line comments dropped, so an old value left in a comment
/// is not read.
fn uncommented(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.starts_with('#') && !line.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every file of the new root, as written: the edge function may be its own file.
fn prod_indexer_files() -> String {
    root_files(|_| true)
}

fn root_files(keep: impl Fn(&Path) -> bool) -> String {
    let dir = root().join(PROD_INDEXER_ROOT);
    let entries =
        std::fs::read_dir(&dir).unwrap_or_else(|err| panic!("{PROD_INDEXER_ROOT} exists: {err}"));
    let mut files: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && keep(path))
        .collect();
    files.sort();
    files
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The body between the braces that open at `open`, skipping braces in strings.
fn braced(text: &str, open: usize) -> &str {
    let (mut depth, mut in_string, mut escaped) = (0usize, false, false);
    for (at, c) in text[open..].char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return &text[open + 1..open + at];
                }
            }
            _ => {}
        }
    }
    panic!("an unclosed block")
}

/// The body of every `resource "<kind>" "<name>" { … }`.
fn resources<'a>(text: &'a str, kind: &str) -> Vec<&'a str> {
    let opening = format!("resource \"{kind}\"");
    text.match_indices(&opening)
        .map(|(at, _)| braced(text, at + text[at..].find('{').expect("a block")))
        .collect()
}

/// The body of every nested `<name> { … }` or `<name> "<label>" { … }`.
fn blocks<'a>(body: &'a str, name: &str) -> Vec<&'a str> {
    let mut found = Vec::new();
    let mut at = 0;
    for line in body.split_inclusive('\n') {
        let opens = line
            .trim_start()
            .strip_prefix(name)
            .is_some_and(|rest| matches!(rest.chars().next(), Some(' ' | '{')))
            && line.trim_end().ends_with('{');
        if opens {
            found.push(braced(body, at + line.find('{').expect("a block")));
        }
        at += line.len();
    }
    found
}

/// The whole value of `key = value` in this body, so `1` is not `10`. A list may
/// span lines. A trailing comment is dropped.
fn setting(body: &str, key: &str) -> Option<String> {
    let mut lines = body.lines();
    while let Some(line) = lines.next() {
        let Some(rest) = line.trim_start().strip_prefix(key) else {
            continue;
        };
        let Some(value) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        if value.starts_with('=') {
            continue;
        }
        let mut value = value
            .split(" #")
            .next()
            .unwrap_or("")
            .split(" //")
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        while value.starts_with('[') && !value.contains(']') {
            value.push_str(lines.next().expect("the list ends").trim());
        }
        return Some(value);
    }
    None
}

/// The items of a list value, quotes dropped.
fn items(value: &str) -> BTreeSet<String> {
    value
        .trim_matches(['[', ']'])
        .split(',')
        .map(|item| item.trim().trim_matches('"').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

/// The body of the function with this literal name.
fn function<'a>(text: &'a str, name: &str) -> &'a str {
    let literal = format!("\"{name}\"");
    resources(text, "aws_lambda_function")
        .into_iter()
        .find(|body| setting(body, "function_name").as_deref() == Some(literal.as_str()))
        .unwrap_or_else(|| panic!("no aws_lambda_function named {name}"))
}

/// Not a scenario: the positive control of OP-4, OP-5, OP-6 and OP-10.
#[test]
fn the_tofu_reader_reads_whole_values_in_the_right_block() {
    let text = uncommented(
        r#"
# resource "aws_lambda_function" "old" {
#   function_name = "openlore-indexer-pass"
#   reserved_concurrent_executions = 7
# }
resource "aws_lambda_function" "a" {
  function_name = "openlore-indexer-search"
  reserved_concurrent_executions = 10 # not one
  lifecycle {
    ignore_changes = [
      image_uri,
      reserved_concurrent_executions,
    ]
  }
}
resource "aws_lambda_function" "b" {
  function_name                  = "openlore-indexer-pass"
  description                    = "a } brace"
  reserved_concurrent_executions = 1
}
resource "aws_cloudfront_distribution" "d" {
  default_cache_behavior {
    allowed_methods = ["GET", "HEAD"]
    function_association {
      event_type = "viewer-request"
    }
  }
  ordered_cache_behavior {
    allowed_methods = ["POST", "GET"]
  }
}
"#,
    );
    let text = text.as_str();
    let search = function(text, "openlore-indexer-search");
    let pass = function(text, "openlore-indexer-pass");

    assert_eq!(
        setting(search, "reserved_concurrent_executions").as_deref(),
        Some("10"),
        "10 is not 1"
    );
    assert_eq!(
        setting(pass, "reserved_concurrent_executions").as_deref(),
        Some("1"),
        "the block, not the first match in the file"
    );
    let lifecycle = blocks(search, "lifecycle");
    assert_eq!(lifecycle.len(), 1);
    assert!(
        items(&setting(lifecycle[0], "ignore_changes").expect("a list"))
            .contains("reserved_concurrent_executions")
    );
    let distribution = resources(text, "aws_cloudfront_distribution")[0];
    let ordered = blocks(distribution, "ordered_cache_behavior");
    assert_eq!(
        items(&setting(ordered[0], "allowed_methods").expect("a list")),
        items(r#"["GET", "POST"]"#),
        "order does not matter"
    );
    assert_eq!(
        blocks(
            blocks(distribution, "default_cache_behavior")[0],
            "function_association"
        )
        .len(),
        1
    );
    assert_eq!(setting(pass, "reserved"), None, "a prefix is not a key");
}

/// Not a scenario: the stubs run green at hand-off, so a broken stub cannot
/// look like a broken verb when DELIVER un-ignores OP-1..E16.
#[test]
fn the_stubs_answer_what_the_gates_will_ask() {
    let stubs = Stubs::new(Posture {
        pass_gate_exit: 3,
        throttled_pass_gates: 1,
        not_live: 1,
        ..Posture::default()
    });
    let tool = |name: &str, args: &[&str]| {
        let output = Command::new(stubs.dir.path().join(name))
            .args(args)
            .output()
            .expect("run the stub");
        (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).into_owned(),
        )
    };
    let url = "https://x/xrpc/org.openlore.appview.searchClaims";
    let oversized = stubs.dir.path().join("oversized");
    std::fs::write(&oversized, vec![b'a'; 9000]).expect("a large body");
    let body = format!("@{}", oversized.display());
    let payload = stubs.dir.path().join("payload");
    let payload_arg = payload.display().to_string();
    let pass_gate = [
        "lambda",
        "invoke",
        "--function-name",
        FUNCTIONS[1],
        &payload_arg,
    ];
    let probe_gate = [
        "lambda",
        "invoke",
        "--function-name",
        FUNCTIONS[2],
        &payload_arg,
    ];

    assert_eq!(tool("curl", &["-w", "%{http_code}", url]).1, "200");
    assert_eq!(
        tool("curl", &["-w", "%{http_code}", "--data-binary", &body, url]).1,
        "413"
    );
    assert_eq!(
        tool(
            "curl",
            &["-w", "%{http_code}", "-X", "POST", "https://x/healthz"]
        )
        .1,
        "404"
    );
    assert_eq!(tool("curl", &["-w", "%{http_code}", "https://x/"]).1, "404");
    assert_eq!(tool("date", &["-u", "+%M"]).1.trim(), "07");
    assert_eq!(tool("sleep", &["30"]).0, Some(0));
    assert_eq!(
        tool(
            "aws",
            &["ecr", "describe-images", "--image-ids", "imageDigest=x"]
        )
        .0,
        Some(254)
    );
    assert!(tool("crane", &["config", "x"]).1.contains("lambda-root"));
    assert!(tool("aws", &["lambda", "get-alias"]).1.contains("prev=6"));
    let live = [
        "lambda",
        "get-function",
        "--function-name",
        FUNCTIONS[0],
        "--qualifier",
        "live",
    ];
    let latest = ["lambda", "get-function", "--function-name", FUNCTIONS[0]];
    assert!(tool("aws", &live).1.contains(&digest()));
    assert!(!tool("aws", &latest).1.contains(&digest()));
    let images: serde_json::Value =
        serde_json::from_str(&tool("aws", &["ecr", "describe-images", "--repository-name", "x"]).1)
            .expect("JSON");
    assert_eq!(images["imageDetails"].as_array().map(Vec::len), Some(10));
    assert_eq!(payload_of("aws lambda invoke --payload {} out"), Some("{}"));

    assert_eq!(tool("aws", &pass_gate).0, Some(254), "throttled once");
    assert_eq!(tool("aws", &pass_gate).0, Some(0));
    let answer: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&payload).expect("the payload")).expect("JSON");
    assert_eq!(answer["exit_code"], 3);
    assert_eq!(answer["mode"], "no-publish");

    let probed: serde_json::Value =
        serde_json::from_str(&tool("aws", &probe_gate).1).expect("JSON");
    assert!(probed["LogResult"].as_str().is_some_and(|l| !l.is_empty()));

    let config = stubs.dir.path().join("config.json");
    std::fs::write(&config, "{ \"Enabled\": false }").expect("a config");
    let file_arg = format!("file://{}", config.display());
    tool("aws", &["cloudfront", "update-distribution", &file_arg]);
    assert!(stubs.calls().contains(r#"{"Enabled":false}"#));
}

/// OP-1
/// ```gherkin
/// @driving_adapter @real-io @D-STZ-26 @D-STZ-44 @DV-STZ-9 @R-STZ-4 @KPI-IXD-6 @kpi @contract-shape:bounded-change
/// Scenario: A deploy passes every gate in order and flips live last
///   Given main's commit is CI-green and its image is signed and has the Lambda root
///   And the no-publish pass gate and the probe gate of the new version succeed
///   When the operator deploys that commit
///   Then CI, the signature, the Lambda root, the copy to the registry, the new versions, the candidate checks,
///     the no-publish pass gate, the probe gate and the flips happen in exactly that order
///   And search flips first, then the pass, then the probe
///   And each live alias records the version it replaced, and the pass also records the index it started from
///   And the candidate stays on the new version
/// ```
#[test]
fn a_deploy_passes_every_gate_in_order_and_flips_live_last() {
    let stubs = Stubs::new(Posture::default());

    let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

    assert_eq!(run.status, 0, "{}\n{}", run.out, stubs.calls());
    let calls = stubs.calls();
    let order: Vec<(&str, Option<usize>)> = [
        ("ci", vec!["gh", "run"]),
        ("signature", vec!["cosign", "verify"]),
        ("lambda root", vec!["crane", "config"]),
        ("copy", vec!["crane", "copy"]),
        ("code", vec!["update-function-code"]),
        ("version", vec!["publish-version"]),
        (
            "candidate",
            vec!["update-alias", "openlore-indexer-search", "candidate"],
        ),
        (
            "post-checks",
            vec!["curl", "candidate.lambda-url", "/healthz"],
        ),
        (
            "pass gate",
            vec!["lambda invoke", "openlore-indexer-pass", "no-publish"],
        ),
        (
            "probe gate",
            vec!["lambda invoke", "openlore-indexer-probe", "{}"],
        ),
        (
            "flip search",
            vec!["update-alias", "openlore-indexer-search", "live"],
        ),
        (
            "flip pass",
            vec!["update-alias", "openlore-indexer-pass", "live"],
        ),
        (
            "flip probe",
            vec!["update-alias", "openlore-indexer-probe", "live"],
        ),
    ]
    .into_iter()
    .map(|(gate, tokens)| (gate, line_of(&calls, &tokens)))
    .collect();
    assert!(
        order.iter().all(|(_, line)| line.is_some()),
        "{order:?}\n{calls}"
    );
    assert!(
        order.windows(2).all(|w| w[0].1 < w[1].1),
        "{order:?}\n{calls}"
    );
    for tool in ["update-function-code", "publish-version"] {
        assert_eq!(
            calls.matches(tool).count(),
            3,
            "{tool} on all three\n{calls}"
        );
    }
    for function in FUNCTIONS {
        assert!(
            called(
                &calls,
                &[
                    "update-alias",
                    function,
                    "live",
                    "--function-version 8",
                    "prev=7"
                ]
            ),
            "{function} records what it replaced\n{calls}"
        );
    }
    assert!(
        called(&calls, &["update-alias", FUNCTIONS[1], "live", "idx=v1"]),
        "the pass records the index it started from\n{calls}"
    );
    assert!(
        called(&calls, &["s3api head-object"]),
        "the candidate's published time is compared with the object's\n{calls}"
    );
    let moved = |version: &str| {
        let version = format!("--function-version {version}");
        line_of(
            &calls,
            &["update-alias", FUNCTIONS[0], "candidate", &version],
        )
    };
    assert!(
        moved("8").is_some() && moved("7").is_none(),
        "a clean deploy leaves candidate on the new version\n{calls}"
    );
}

/// OP-2
/// ```gherkin
/// @driving_adapter @real-io @D-STZ-26 @KPI-IXD-6 @kpi @contract-shape:bounded-change
/// Scenario: A rollback returns every function to the version it replaced
///   Given each live alias records the version it replaced
///   And that version's image is still in the registry
///   When the operator rolls back
///   Then the live alias of all three functions moves back to that version
/// ```
#[test]
fn a_rollback_returns_every_function_to_the_version_it_replaced() {
    let stubs = Stubs::new(Posture {
        image_in_ecr: true,
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "rollback"], &[], "");

    assert_eq!(run.status, 0, "{}\n{}", run.out, stubs.calls());
    let calls = stubs.calls();
    for function in FUNCTIONS {
        assert!(
            called(
                &calls,
                &["update-alias", function, "live", "--function-version 6"]
            ),
            "{function} goes back to version 6\n{calls}"
        );
    }
}

/// OP-3
/// ```gherkin
/// @driving_adapter @real-io @D-STZ-18d @R-STZ-8 @KPI-STZ-G3 @kpi @contract-shape:bounded-change
/// Scenario: The kill switch stops search and the edge, and leaves the pass and the probe alone
///   When the operator stops search
///   Then search may run 0 instances, the public edge is switched off, and the pass and the probe are untouched
///   When the operator starts search
///   Then search may run 3 instances and the public edge is switched back on
/// ```
#[test]
fn the_kill_switch_stops_search_and_the_edge_and_leaves_the_pass_and_the_probe_alone() {
    let run_verb = |verb: &str, enabled: bool| {
        let stubs = Stubs::new(Posture {
            distribution_enabled: enabled,
            ..Posture::default()
        });
        let run = stubs.run(&["lambda", verb], &[], "");
        assert_eq!(run.status, 0, "{verb}\n{}\n{}", run.out, stubs.calls());
        stubs.calls()
    };

    let stopped = run_verb("stop", true);
    let started = run_verb("start", false);

    let set = "put-function-concurrency";
    let edge = ["update-distribution", "--if-match ECONFIG"];
    let compact = |calls: &str| calls.split_whitespace().collect::<String>();
    let cap_to_zero = [set, FUNCTIONS[0], "--reserved-concurrent-executions 0"];
    assert!(called(&stopped, &cap_to_zero), "{stopped}");
    assert!(called(&stopped, &edge), "{stopped}");
    let stops_search = line_of(&stopped, &cap_to_zero);
    assert!(
        stops_search.is_some() && stops_search < line_of(&stopped, &edge),
        "search stops first, in seconds\n{stopped}"
    );
    assert!(
        compact(&stopped).contains(r#""Enabled":false"#),
        "{stopped}"
    );
    assert!(
        called(
            &started,
            &[set, FUNCTIONS[0], "--reserved-concurrent-executions 3"]
        ),
        "{started}"
    );
    assert!(called(&started, &edge), "{started}");
    assert!(compact(&started).contains(r#""Enabled":true"#), "{started}");
    for calls in [&stopped, &started] {
        assert_eq!(calls.matches(set).count(), 1, "search only\n{calls}");
    }
}

/// OP-4
/// ```gherkin
/// @infrastructure @U-STZ-4 @D-STZ-21 @D-STZ-18d @ADR-083 @contract-shape:pure-function
/// Scenario: The edge is one allowlist, in front of every path
///   Given the new infrastructure root
///   Then its edge function lets through only the search and the health check
///   And every behavior allows every method, so anything else is told "no such place", not "forbidden"
///   And an apply never turns the edge back on after a stop
/// ```
#[test]
fn the_edge_is_one_allowlist_in_front_of_every_path() {
    let (tofu, files) = (prod_indexer_tofu(), prod_indexer_files());

    assert!(
        tofu.contains("resource \"aws_cloudfront_function\""),
        "an edge function"
    );
    assert!(
        files.contains("/xrpc/org.openlore.appview.searchClaims"),
        "the search path"
    );
    assert!(files.contains("/healthz"), "the health path");
    let distributions = resources(&tofu, "aws_cloudfront_distribution");
    assert_eq!(distributions.len(), 1);
    let distribution = distributions[0];
    let behaviors: Vec<&str> = blocks(distribution, "default_cache_behavior")
        .into_iter()
        .chain(blocks(distribution, "ordered_cache_behavior"))
        .collect();
    assert_eq!(behaviors.len(), 3, "three behaviors");
    let every_method: BTreeSet<String> = ALL_METHODS.iter().map(|m| (*m).to_string()).collect();
    for behavior in behaviors {
        let methods = setting(behavior, "allowed_methods").expect("allowed_methods");
        assert_eq!(
            items(&methods),
            every_method,
            "a narrower behavior would answer 403, not 404"
        );
        let associations = blocks(behavior, "function_association");
        assert_eq!(associations.len(), 1, "the allowlist on every behavior");
        assert_eq!(
            setting(associations[0], "event_type").as_deref(),
            Some("\"viewer-request\""),
            "it runs before the origin"
        );
    }
    let lifecycle = blocks(distribution, "lifecycle");
    assert_eq!(lifecycle.len(), 1, "the distribution has a lifecycle block");
    assert!(
        items(&setting(lifecycle[0], "ignore_changes").expect("ignore_changes"))
            .contains("enabled"),
        "lambda stop owns enabled"
    );
    assert!(!tofu.contains("origin_access_control"), "no OAC (D-STZ-20)");
}

/// OP-5
/// ```gherkin
/// @infrastructure @U-STZ-5 @DV-STZ-7 @DV-STZ-11 @D-STZ-6 @contract-shape:pure-function
/// Scenario: A crashed pass is not retried, one pass runs at a time, and the index keeps a week of history
///   Given the new infrastructure root
///   Then the pass and the probe have 0 asynchronous retries
///   And the pass has reserved concurrency 1
///   And noncurrent index versions are kept 7 days, at least the 5 newest
/// ```
#[test]
fn a_crashed_pass_is_not_retried_one_pass_runs_at_a_time_and_the_index_keeps_a_week_of_history() {
    let tofu = prod_indexer_tofu();

    let invoke_configs = resources(&tofu, "aws_lambda_function_event_invoke_config");
    assert_eq!(invoke_configs.len(), 2, "pass and probe");
    for config in invoke_configs {
        assert_eq!(
            setting(config, "maximum_retry_attempts").as_deref(),
            Some("0"),
            "no async retries"
        );
    }
    assert_eq!(
        setting(
            function(&tofu, "openlore-indexer-pass"),
            "reserved_concurrent_executions"
        )
        .as_deref(),
        Some("1"),
        "one pass at a time"
    );
    let rules: Vec<&str> = resources(&tofu, "aws_s3_bucket_lifecycle_configuration")
        .into_iter()
        .filter(|lifecycle| {
            setting(lifecycle, "bucket").is_some_and(|b| b.contains("aws_s3_bucket.index"))
        })
        .flat_map(|lifecycle| blocks(lifecycle, "noncurrent_version_expiration"))
        .collect();
    assert_eq!(rules.len(), 1, "one rule for noncurrent versions");
    assert_eq!(setting(rules[0], "noncurrent_days").as_deref(), Some("7"));
    assert_eq!(
        setting(rules[0], "newer_noncurrent_versions").as_deref(),
        Some("5")
    );
}

/// OP-6 (a fourth concurrent search is throttled by Lambda, which no in-process
/// scenario can reach. This is the honest layer: the setting that does it. The
/// CLI side, a 429 read as busy, is already `a_429_is_busy_with_the_retry_after…`
/// and `a_429_without_a_usable_retry_after_is_still_busy` in `adapter-index-query`.
/// The 429 at cap 3 itself is unverified until the trial (P-18): `lambda
/// test-alarm throttles` sets the concurrency to 0, which is another case.)
/// ```gherkin
/// @infrastructure @D-STZ-18a @R-STZ-8 @KPI-STZ-G3 @kpi @contract-shape:pure-function
/// Scenario: At most three searches run at once, and a stop survives an apply
///   Given the new infrastructure root
///   Then search has reserved concurrency 3, so a fourth concurrent search is turned away
///   And an apply never restores that number after a stop
/// ```
#[test]
fn at_most_three_searches_run_at_once_and_a_stop_survives_an_apply() {
    let tofu = prod_indexer_tofu();
    let search = function(&tofu, "openlore-indexer-search");

    assert_eq!(
        setting(search, "reserved_concurrent_executions").as_deref(),
        Some("3")
    );
    let lifecycle = blocks(search, "lifecycle");
    assert_eq!(lifecycle.len(), 1);
    assert!(
        items(&setting(lifecycle[0], "ignore_changes").expect("ignore_changes"))
            .contains("reserved_concurrent_executions"),
        "lambda stop owns it"
    );
}

/// OP-7
/// ```gherkin
/// @driving_adapter @real-io @U-STZ-2 @D-STZ-29 @contract-shape:bounded-change
/// Scenario: A republish carries the image already live, whatever the next version would have held
///   Given the next version of each function holds a different image
///   And the image already live is still in the registry
///   When the operator republishes
///   Then each function is set to the image already live before a version is published
///   And each new version is published against exactly that image
///   And the new versions pass the candidate and gate checks before live flips
/// ```
#[test]
fn a_republish_carries_the_image_already_live() {
    let stubs = Stubs::new(Posture {
        image_in_ecr: true,
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "republish"], &[], "");

    assert_flipped(&run, &stubs);
    let calls = stubs.calls();
    let image = format!("@{}", digest());
    let code_sha = code_sha();
    for function in FUNCTIONS {
        let set = line_of(&calls, &["update-function-code", function, &image]);
        let published = line_of(&calls, &["publish-version", function, &code_sha]);
        assert!(set.is_some() && set < published, "{function}\n{calls}");
    }
    assert!(!called(&calls, &["crane", "copy"]), "{calls}");
    let gate = line_of(
        &calls,
        &["lambda invoke", "openlore-indexer-pass", "no-publish"],
    );
    let flip = line_of(&calls, &["update-alias", FUNCTIONS[0], "live"]);
    assert!(gate.is_some() && gate < flip, "{calls}");
}

/// OP-8
/// ```gherkin
/// @driving_adapter @real-io @D-STZ-27 @contract-shape:bounded-change
/// Scenario: Restoring the index copies the chosen version back once
///   Given an earlier version of the index is still kept
///   When the operator restores that version
///   Then it becomes the current index, and the version it came from stays
/// ```
#[test]
fn restoring_the_index_copies_the_chosen_version_back_once() {
    let stubs = Stubs::new(Posture::default());

    let run = stubs.run(&["lambda", "restore-index", "v1"], &[], "");

    assert_eq!(run.status, 0, "{}\n{}", run.out, stubs.calls());
    let calls = stubs.calls();
    assert_eq!(calls.matches("copy-object").count(), 1, "{calls}");
    assert!(called(&calls, &["copy-object", "versionId=v1"]), "{calls}");
    assert!(!called(&calls, &["delete-object"]), "{calls}");
}

/// OP-9
/// ```gherkin
/// @driving_adapter @real-io @D-STZ-23 @contract-shape:bounded-change
/// Scenario Outline: Maintenance switches change one function and nothing else
///   When the operator turns <switch>
///   Then <function> may run <instances> instance, and no other function changes
///   Examples:
///     | switch         | function  | instances |
///     | the pass off   | the pass  | 0         |
///     | the pass on    | the pass  | 1         |
///     | the probe off  | the probe | 0         |
///     | the probe on   | the probe | 1         |
/// ```
#[test]
fn maintenance_switches_change_one_function_and_nothing_else() {
    each_row(
        [
            ("pass-off", FUNCTIONS[1], "0"),
            ("pass-on", FUNCTIONS[1], "1"),
            ("probe-off", FUNCTIONS[2], "0"),
            ("probe-on", FUNCTIONS[2], "1"),
        ],
        |(verb, function, cap)| {
            let stubs = Stubs::new(Posture::default());

            let run = stubs.run(&["lambda", verb], &[], "");

            assert_eq!(run.status, 0, "{}\n{}", run.out, stubs.calls());
            let calls = stubs.calls();
            let set = "put-function-concurrency";
            let cap = format!("--reserved-concurrent-executions {cap}");
            assert!(called(&calls, &[set, function, &cap]), "{calls}");
            assert_eq!(calls.matches(set).count(), 1, "{calls}");
        },
    );
}

/// OP-10 (the condition on U-STZ-7: the probe refuses a non-object event, and
/// nothing documents what an event looks like with no `input` set)
/// ```gherkin
/// @infrastructure @U-STZ-7 @D-STZ-23 @contract-shape:pure-function
/// Scenario: Both schedules send an empty event, which the pass and the probe accept
///   Given the new infrastructure root
///   Then the pass schedule and the probe schedule each send an empty event
/// ```
#[test]
fn both_schedules_send_an_empty_event() {
    let tofu = prod_indexer_tofu();

    let schedules = resources(&tofu, "aws_scheduler_schedule");
    assert_eq!(schedules.len(), 2, "pass and probe");
    for schedule in schedules {
        let targets = blocks(schedule, "target");
        assert_eq!(targets.len(), 1);
        assert_eq!(setting(targets[0], "input").as_deref(), Some(r#""{}""#));
    }
}

/// OP-E1
/// ```gherkin
/// @driving_adapter @real-io @error @adversarial @DV-STZ-9 @C6a @contract-shape:unbounded-preservation
/// Scenario Outline: A deploy names one exact build or it is refused before any tool runs
///   When the operator deploys <reference>
///   Then the deploy is refused, and no tool ran
///   Examples:
///     | reference      |
///     | main           |
///     | latest         |
///     | v1.2.3         |
///     | a short sha    |
///     | a short digest |
/// ```
#[test]
fn a_deploy_names_one_exact_build_or_it_is_refused_before_any_tool_runs() {
    each_row(
        ["main", "latest", "v1.2.3", "4f2c1ab", "sha256:abc123"],
        |reference| {
            let stubs = Stubs::new(Posture::default());

            let run = stubs.run(&["lambda", "deploy", reference], &[], "");

            assert_ne!(run.status, 0, "{}", run.out);
            assert!(run.out.contains("refusing"), "{}", run.out);
            assert!(stubs.calls().is_empty(), "{}", stubs.calls());
        },
    );
}

/// OP-E2
/// ```gherkin
/// @driving_adapter @real-io @error @DV-STZ-9 @D-STZ-24 @C6b @contract-shape:unbounded-preservation
/// Scenario: An image that cannot run the pass is refused before the registry sees it
///   Given main's commit is CI-green and its image is signed
///   And the image was built before the Lambda root existed
///   When the operator deploys that commit
///   Then the deploy is refused, and nothing was copied, published or flipped
/// ```
#[test]
fn an_image_that_cannot_run_the_pass_is_refused_before_the_registry_sees_it() {
    let stubs = Stubs::new(Posture {
        lambda_capable: false,
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

    assert_ne!(run.status, 0, "{}", run.out);
    assert!(run.out.contains("refusing"), "{}", run.out);
    let calls = stubs.calls();
    assert!(
        called(&calls, &["cosign", "verify"]),
        "the signature came first\n{calls}"
    );
    for change in [
        "crane copy",
        "update-function-code",
        "publish-version",
        "update-alias",
    ] {
        assert!(!called(&calls, &[change]), "{change}\n{calls}");
    }
}

/// OP-E3
/// ```gherkin
/// @driving_adapter @real-io @error @D-STZ-44 @DV-STZ-9 @C6b @contract-shape:unbounded-preservation
/// Scenario: A pass gate that fails stops the deploy with nothing flipped
///   Given the new pass version answers its gate with exit 2
///   When the operator deploys
///   Then the deploy stops, and no live alias moved
/// ```
#[test]
fn a_pass_gate_that_fails_stops_the_deploy_with_nothing_flipped() {
    let stubs = Stubs::new(Posture {
        pass_gate_exit: 2,
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

    assert_stopped_with_nothing_flipped(&run, &stubs);
    assert_reached(&stubs, &PASS_GATE);
    assert_refused(&run);
}

/// OP-E4
/// ```gherkin
/// @driving_adapter @real-io @error @DV-STZ-9 @C5a @contract-shape:bounded-change
/// Scenario Outline: A pass gate that reached nobody stops the deploy unless the operator accepts it
///   Given the new pass version answers its gate with exit 3
///   And the operator <accepts or not> that exit
///   When the operator deploys
///   Then the deploy <stops or flips live>
///   Examples:
///     | accepts or not  | stops or flips live |
///     | does not accept | stops               |
///     | accepts         | flips live          |
/// ```
#[test]
fn a_pass_gate_that_reached_nobody_stops_the_deploy_unless_the_operator_accepts_it() {
    let exit_3 = Posture {
        pass_gate_exit: 3,
        ..Posture::default()
    };
    let stopped = Stubs::new(exit_3);
    let accepted = Stubs::new(exit_3);

    let refused = stopped.run(&["lambda", "deploy", SHA], &[], "");
    let flipped = accepted.run(&["lambda", "deploy", SHA], &[ACCEPT_EXIT_3], "");

    assert_stopped_with_nothing_flipped(&refused, &stopped);
    assert_reached(&stopped, &PASS_GATE);
    assert_refused(&refused);
    assert_flipped(&flipped, &accepted);
}

/// OP-E5
/// ```gherkin
/// @driving_adapter @real-io @error @D-STZ-25 @C6b @contract-shape:unbounded-preservation
/// Scenario: A rollback to a version whose image was removed is refused
///   Given the version a live alias records has no image left in the registry
///   When the operator rolls back
///   Then the rollback is refused, and no alias moved
/// ```
#[test]
fn a_rollback_to_a_version_whose_image_was_removed_is_refused() {
    let stubs = Stubs::new(Posture::default());

    let run = stubs.run(&["lambda", "rollback"], &[], "");

    assert_ne!(run.status, 0, "{}", run.out);
    assert!(run.out.contains("refusing"), "{}", run.out);
    assert!(
        !called(&stubs.calls(), &["update-alias"]),
        "{}",
        stubs.calls()
    );
}

/// OP-E6
/// ```gherkin
/// @driving_adapter @real-io @error @D-STZ-27 @C4b @C6a @contract-shape:bounded-change
/// Scenario Outline: Resetting the index needs the exact typed word
///   When the operator answers the reset prompt with <answer>
///   Then nothing is deleted
///   When the operator types "reset"
///   Then the index object is deleted once
///   Examples:
///     | answer                     |
///     | yes                        |
///     | the word in capitals       |
///     | nothing                    |
///     | the word and a space       |
/// ```
#[test]
fn resetting_the_index_needs_the_exact_typed_word() {
    each_row(["yes\n", "RESET\n", "\n", "reset \n"], |answer| {
        let declined = Stubs::new(Posture::default());

        let run = declined.run(&["lambda", "reset-index"], &[], answer);

        assert_refused(&run);
        assert!(
            !called(&declined.calls(), &["delete-object"]),
            "{}",
            declined.calls()
        );
    });
    let confirmed = Stubs::new(Posture::default());

    let done = confirmed.run(&["lambda", "reset-index"], &[], "reset\n");

    assert_eq!(done.status, 0, "{}\n{}", done.out, confirmed.calls());
    assert_eq!(
        confirmed.calls().matches("delete-object").count(),
        1,
        "{}",
        confirmed.calls()
    );
}

/// OP-E7 (a reply with no `mode` came from a version that ignored the request, and
/// may have written, D-STZ-44)
/// ```gherkin
/// @driving_adapter @real-io @error @D-STZ-44 @C6a @contract-shape:unbounded-preservation
/// Scenario Outline: A pass gate that does not say it ran as no-publish stops the deploy
///   Given the new pass version answers its gate with exit <exit> and no word of no-publish
///   And the operator <accepts or not> a pass gate that reached nobody
///   When the operator deploys
///   Then the deploy stops, and no live alias moved
///   Examples:
///     | exit | accepts or not  |
///     | 0    | does not accept |
///     | 3    | accepts         |
/// ```
#[test]
fn a_pass_gate_that_does_not_say_it_ran_as_no_publish_stops_the_deploy() {
    each_row(
        [(0, None), (3, Some(ACCEPT_EXIT_3))],
        |(exit, acceptance)| {
            let stubs = Stubs::new(Posture {
                pass_gate_exit: *exit,
                echoes_mode: false,
                ..Posture::default()
            });
            let env: Vec<(&str, &str)> = acceptance.iter().copied().collect();

            let run = stubs.run(&["lambda", "deploy", SHA], &env, "");

            assert_stopped_with_nothing_flipped(&run, &stubs);
            assert_reached(&stubs, &PASS_GATE);
            assert_refused(&run);
        },
    );
}

/// OP-E8 (C5b, re-rated HIGH by DEVOPS: a loose acceptance would let a failed pass
/// reach `live`)
/// ```gherkin
/// @driving_adapter @real-io @error @DV-STZ-9 @C5b @contract-shape:unbounded-preservation
/// Scenario Outline: Accepting a gate that reached nobody accepts nothing else
///   Given the operator accepts a pass gate that reached nobody
///   And the new pass version <fails its gate>
///   When the operator deploys
///   Then the deploy stops, and no live alias moved
///   Examples:
///     | fails its gate      |
///     | answers with exit 2 |
///     | crashes             |
/// ```
#[test]
fn accepting_a_gate_that_reached_nobody_accepts_nothing_else() {
    each_row(
        [
            Posture {
                pass_gate_exit: 2,
                ..Posture::default()
            },
            Posture {
                pass_gate_crashes: true,
                ..Posture::default()
            },
        ],
        |posture| {
            let stubs = Stubs::new(*posture);

            let run = stubs.run(&["lambda", "deploy", SHA], &[ACCEPT_EXIT_3], "");

            assert_stopped_with_nothing_flipped(&run, &stubs);
            assert_reached(&stubs, &PASS_GATE);
            assert_refused(&run);
        },
    );
}

/// OP-E9 (DEVOPS step 13: the first deploy on an empty bucket needs the acceptance)
/// ```gherkin
/// @driving_adapter @real-io @error @DV-STZ-9 @C5a @C5b @contract-shape:bounded-change
/// Scenario Outline: A probe gate that finds the index not live stops the deploy unless accepted
///   Given the new probe version finds the index not live
///   And the new pass version answers its gate with exit <exit>
///   And the operator <accepts or not> an index that is not live
///   When the operator deploys
///   Then the deploy <stops or flips live>
///   Examples:
///     | exit | accepts or not  | stops or flips live |
///     | 0    | does not accept | stops               |
///     | 0    | accepts         | flips live          |
///     | 2    | accepts         | stops               |
/// ```
#[test]
fn a_probe_gate_that_finds_the_index_not_live_stops_the_deploy_unless_accepted() {
    each_row(
        [(0, false, false), (0, true, true), (2, true, false)],
        |(exit, accepts, flips)| {
            let stubs = Stubs::new(Posture {
                not_live: 1,
                pass_gate_exit: *exit,
                ..Posture::default()
            });
            let env: &[(&str, &str)] = if *accepts { &[ACCEPT_NOT_LIVE] } else { &[] };

            let run = stubs.run(&["lambda", "deploy", SHA], env, "");

            if *flips {
                assert_flipped(&run, &stubs);
            } else {
                assert_stopped_with_nothing_flipped(&run, &stubs);
                assert_reached(&stubs, &PASS_GATE);
                assert_refused(&run);
            }
        },
    );
}

/// OP-E10 (C4b: an inverse with nothing to undo)
/// ```gherkin
/// @driving_adapter @real-io @error @D-STZ-26 @C4b @contract-shape:unbounded-preservation
/// Scenario: A rollback with no recorded version is refused
///   Given no live alias records a version it replaced
///   When the operator rolls back
///   Then the rollback is refused, and no alias moved
/// ```
#[test]
fn a_rollback_with_no_recorded_version_is_refused() {
    let stubs = Stubs::new(Posture {
        alias_records_prev: false,
        image_in_ecr: true,
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "rollback"], &[], "");

    assert_ne!(run.status, 0, "{}", run.out);
    assert!(run.out.contains("refusing"), "{}", run.out);
    assert!(
        !called(&stubs.calls(), &["update-alias"]),
        "{}",
        stubs.calls()
    );
}

/// OP-E11
/// ```gherkin
/// @driving_adapter @real-io @error @U-STZ-2 @C6b @contract-shape:unbounded-preservation
/// Scenario: A republish that does not hold the image already live is refused
///   Given a function does not report the image it was just set to
///   When the operator republishes
///   Then the republish is refused, and no version is published and no alias moved
/// ```
#[test]
fn a_republish_that_does_not_hold_the_image_already_live_is_refused() {
    let stubs = Stubs::new(Posture {
        image_in_ecr: true,
        code_sha_matches: false,
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "republish"], &[], "");

    assert_ne!(run.status, 0, "{}", run.out);
    assert!(run.out.contains("refusing"), "{}", run.out);
    let calls = stubs.calls();
    assert!(!called(&calls, &["publish-version"]), "{calls}");
    assert!(!called(&calls, &["update-alias"]), "{calls}");
}

/// OP-E12
/// ```gherkin
/// @driving_adapter @real-io @error @D-STZ-27 @C7c @contract-shape:unbounded-preservation
/// Scenario Outline: Restoring the index is refused when a pass is due or running
///   Given it is minute <minute> of the hour
///   When the operator restores an earlier version of the index
///   Then the restore is refused, and nothing is copied
///   Examples:
///     | minute |
///     | 14     |
///     | 00     |
///     | 31     |
/// ```
#[test]
fn restoring_the_index_is_refused_when_a_pass_is_due_or_running() {
    each_row(["14", "00", "31"], |minute| {
        let stubs = Stubs::new(Posture {
            minute,
            ..Posture::default()
        });

        let run = stubs.run(&["lambda", "restore-index", "v1"], &[], "");

        assert_ne!(run.status, 0, "{}", run.out);
        assert!(run.out.contains("refusing"), "{}", run.out);
        assert!(
            !called(&stubs.calls(), &["copy-object"]),
            "{}",
            stubs.calls()
        );
    });
}

/// OP-E13 (reserved concurrency keeps a gate and a running pass apart, so a gate
/// is throttled and waits: every 30 s for up to 870 s, so at most 30 attempts)
/// ```gherkin
/// @driving_adapter @real-io @error @DV-STZ-9 @C7c @contract-shape:bounded-change
/// Scenario Outline: A gate waits for a running pass or probe, and gives up with nothing flipped
///   Given <function> is busy for <how long>
///   When the operator deploys
///   Then the deploy <waits and flips, or stops>
///   Examples:
///     | function  | how long      | waits and flips, or stops        |
///     | the pass  | a little      | waits, then flips live           |
///     | the probe | a little      | waits, then flips live           |
///     | the pass  | the whole way | gives up, and no live alias moved |
///     | the probe | the whole way | gives up, and no live alias moved |
/// ```
#[test]
fn a_gate_waits_for_a_running_pass_or_probe_and_gives_up_with_nothing_flipped() {
    let busy = |pass, probe| Posture {
        throttled_pass_gates: pass,
        throttled_probe_gates: probe,
        ..Posture::default()
    };
    each_row(
        [
            (FUNCTIONS[1], busy(2, 0), true),
            (FUNCTIONS[2], busy(0, 2), true),
            (FUNCTIONS[1], busy(ALWAYS, 0), false),
            (FUNCTIONS[2], busy(0, ALWAYS), false),
        ],
        |(function, posture, flips)| {
            let stubs = Stubs::new(*posture);

            let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

            let calls = stubs.calls();
            let attempts = calls
                .lines()
                .filter(|line| line.contains("lambda invoke") && line.contains(function))
                .count();
            if *flips {
                assert_flipped(&run, &stubs);
                assert_eq!(attempts, 3, "{calls}");
                assert_eq!(calls.matches("sleep 30").count(), 2, "{calls}");
            } else {
                assert_stopped_with_nothing_flipped(&run, &stubs);
                assert!((2..=30).contains(&attempts), "{attempts} attempts");
            }
        },
    );
}

/// OP-E14
/// ```gherkin
/// @driving_adapter @real-io @error @U-STZ-2 @D-STZ-25 @C6b @contract-shape:unbounded-preservation
/// Scenario: A republish of an image that left the registry is refused
///   Given the image already live is no longer in the registry
///   When the operator republishes
///   Then the republish is refused, and no function changed
/// ```
#[test]
fn a_republish_of_an_image_that_left_the_registry_is_refused() {
    let stubs = Stubs::new(Posture::default());

    let run = stubs.run(&["lambda", "republish"], &[], "");

    assert_ne!(run.status, 0, "{}", run.out);
    assert!(run.out.contains("refusing"), "{}", run.out);
    let calls = stubs.calls();
    for change in ["update-function-code", "publish-version", "update-alias"] {
        assert!(!called(&calls, &[change]), "{change}\n{calls}");
    }
}

/// OP-E15
/// ```gherkin
/// @driving_adapter @real-io @error @D-STZ-26 @DV-STZ-9 @KPI-IXD-6 @kpi @C7b @contract-shape:bounded-change
/// Scenario: A search that fails at the public edge after its flip goes straight back
///   Given every gate of the new version passes
///   And the public edge fails once search has flipped
///   When the operator deploys
///   Then search and its candidate go back to the version they replaced
///   And the deploy fails, and the pass and the probe never flipped
/// ```
#[test]
fn a_search_that_fails_at_the_public_edge_after_its_flip_goes_straight_back() {
    let stubs = Stubs::new(Posture {
        edge_after_the_flip: 500,
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

    assert_ne!(run.status, 0, "{}\n{}", run.out, stubs.calls());
    let calls = stubs.calls();
    let back = "--function-version 7";
    let forward = line_of(
        &calls,
        &["update-alias", FUNCTIONS[0], "live", "--function-version 8"],
    );
    let returned = line_of(&calls, &["update-alias", FUNCTIONS[0], "live", back]);
    assert!(forward.is_some() && forward < returned, "{calls}");
    assert!(
        called(&calls, &["update-alias", FUNCTIONS[0], "candidate", back]),
        "{calls}"
    );
    for function in [FUNCTIONS[1], FUNCTIONS[2]] {
        assert!(
            !called(&calls, &["update-alias", function, "live"]),
            "{calls}"
        );
    }
}

/// OP-E16 (`/healthz` means index published at, D-STZ-12, which is what lets step
/// 11 prove the new code fetched the real index. `lambda start` row: a deploy
/// during a stop fails here, so a deploy cannot undo a stop.)
/// ```gherkin
/// @driving_adapter @real-io @error @DV-STZ-9 @D-STZ-12 @D-STZ-18d @C6b @contract-shape:bounded-change
/// Scenario Outline: A candidate that fails its checks stops the deploy before any gate
///   Given the candidate <fails>
///   When the operator deploys
///   Then the deploy stops before the pass gate, and no live alias moved
///   Examples:
///     | fails                                                     |
///     | answers with a server error                               |
///     | answers that the kill switch stopped it                   |
///     | reports an index published at a time the bucket's does not |
/// ```
#[test]
fn a_candidate_that_fails_its_checks_stops_the_deploy_before_any_gate() {
    each_row(
        [
            Posture {
                candidate_status: 500,
                ..Posture::default()
            },
            Posture {
                candidate_status: 429,
                ..Posture::default()
            },
            Posture {
                object_published_at: "2026-10-27T14:00:02Z",
                ..Posture::default()
            },
        ],
        |posture| {
            let stubs = Stubs::new(*posture);

            let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

            assert_stopped_with_nothing_flipped(&run, &stubs);
            assert_reached(&stubs, &["curl", "candidate.lambda-url"]);
            assert!(
                !called(&stubs.calls(), &["lambda invoke"]),
                "{}",
                stubs.calls()
            );
        },
    );
}

/// OP-11 (D-STZ-44 makes the pass strict, so a wrong event is a refused deploy or
/// a real pass by mistake)
/// ```gherkin
/// @driving_adapter @real-io @D-STZ-44 @U-STZ-7 @contract-shape:bounded-change
/// Scenario Outline: Each operator action sends exactly its own event
///   When the operator <action>
///   Then <function> is invoked with exactly <event>
///   Examples:
///     | action                      | function  | event              |
///     | deploys                     | the pass  | the no-publish one |
///     | deploys                     | the probe | the empty one      |
///     | republishes                 | the pass  | the no-publish one |
///     | republishes                 | the probe | the empty one      |
///     | triggers a pass             | the pass  | the empty one      |
///     | runs the probe              | the probe | the empty one      |
///     | resets and rebuilds         | the pass  | the empty one      |
///     | test-fires the error alarm  | the probe | a list             |
/// ```
#[test]
fn each_operator_action_sends_exactly_its_own_event() {
    const NO_PUBLISH: &str = r#"{"mode":"no-publish"}"#;
    let (pass, probe) = (FUNCTIONS[1], FUNCTIONS[2]);
    let in_ecr = Posture {
        image_in_ecr: true,
        ..Posture::default()
    };
    each_row(
        [
            (vec!["lambda", "deploy", SHA], pass, NO_PUBLISH),
            (vec!["lambda", "deploy", SHA], probe, "{}"),
            (vec!["lambda", "republish"], pass, NO_PUBLISH),
            (vec!["lambda", "republish"], probe, "{}"),
            (vec!["lambda", "trigger"], pass, "{}"),
            (vec!["lambda", "probe"], probe, "{}"),
            (vec!["lambda", "reset-index", "--trigger"], pass, "{}"),
            (vec!["lambda", "test-alarm", "errors"], probe, "[]"),
        ],
        |(args, function, event)| {
            let stubs = Stubs::new(in_ecr);

            stubs.run(args, &[], "reset\n");

            let calls = stubs.calls();
            let payloads: Vec<Option<&str>> = calls
                .lines()
                .filter(|line| line.contains("lambda invoke") && line.contains(function))
                .map(payload_of)
                .collect();
            assert!(!payloads.is_empty(), "never invoked\n{calls}");
            assert!(
                payloads.iter().all(|payload| *payload == Some(*event)),
                "{payloads:?}"
            );
        },
    );
}

/// OP-12 (what makes "no PDS impact" true for these verbs: they share nothing with
/// the host, so there is nothing to measure)
/// ```gherkin
/// @driving_adapter @real-io @KPI-IXD-6 @kpi @contract-shape:unbounded-preservation
/// Scenario Outline: No Lambda operation reaches the shared host
///   When the operator runs <verb>
///   Then a tool ran, and nothing was sent to the host
///   Examples:
///     | verb          |
///     | deploy        |
///     | rollback      |
///     | republish     |
///     | seed          |
///     | stop          |
///     | start         |
///     | status        |
///     | trigger       |
///     | probe         |
///     | pass-off      |
///     | pass-on       |
///     | probe-off     |
///     | probe-on      |
///     | reset-index   |
///     | restore-index |
/// ```
#[test]
fn no_lambda_operation_reaches_the_shared_host() {
    let in_ecr = Posture {
        image_in_ecr: true,
        ..Posture::default()
    };
    each_row(
        [
            vec!["deploy", SHA],
            vec!["rollback"],
            vec!["republish"],
            vec!["seed", SHA],
            vec!["stop"],
            vec!["start"],
            vec!["status"],
            vec!["trigger"],
            vec!["probe"],
            vec!["pass-off"],
            vec!["pass-on"],
            vec!["probe-off"],
            vec!["probe-on"],
            vec!["reset-index"],
            vec!["restore-index", "v1"],
        ],
        |verb| {
            let stubs = Stubs::new(in_ecr);
            let mut args = vec!["lambda"];
            args.extend(verb);

            stubs.run(&args, &[], "reset\n");

            assert_reached(&stubs, &["aws "]);
            let calls = stubs.calls();
            for host_path in [
                "ssm send-command",
                "ssm start-session",
                "ssh ",
                "host standby",
            ] {
                assert!(!called(&calls, &[host_path]), "{host_path}\n{calls}");
            }
        },
    );
}

/// OP-13 (U-STZ-1: the host verb is one verb with explicit flags. The timers go
/// first, so only one writer of the health line exists during the cut-over.)
/// ```gherkin
/// @driving_adapter @real-io @U-STZ-1 @D-STZ-31 @contract-shape:bounded-change
/// Scenario Outline: Standby stops what it names and leaves the rest
///   When the operator puts the host on standby for <what>
///   Then the host is told exactly that
///   And on the host, <stopped> stops, and <left> stays as it was
///   Examples:
///     | what          | stopped                    | left          |
///     | its timers    | the pass and health timers | the container |
///     | its container | the container              | the timers    |
/// ```
#[test]
fn standby_stops_what_it_names_and_leaves_the_rest() {
    each_row(["--timers", "--container"], |flag| {
        let laptop = Stubs::new(Posture::default());
        let run = laptop.run(&["standby", flag], &[], "");
        assert_eq!(run.status, 0, "{}\n{}", run.out, laptop.calls());
        assert_reached(
            &laptop,
            &["ssm send-command", &format!("host standby {flag}")],
        );

        let host = Stubs::new(Posture::default());
        let run = host.run(&["host", "standby", flag], &[], "");
        assert_eq!(run.status, 0, "{}\n{}", run.out, host.calls());
        let calls = host.calls();
        let timers = [
            "openlore-indexer-pass.timer",
            "openlore-indexer-health.timer",
        ];
        let stops_timers = timers
            .iter()
            .all(|timer| called(&calls, &["systemctl", "stop", timer]));
        let stops_container =
            called(&calls, &["docker", "stop"]) || called(&calls, &["docker", "compose", "down"]);
        assert_eq!(stops_timers, *flag == "--timers", "{calls}");
        assert_eq!(stops_container, *flag == "--container", "{calls}");
    });
}

/// OP-14 (until the first flip, `live` is unchecked code, and a schedule could
/// publish from it)
/// ```gherkin
/// @infrastructure @D-STZ-26 @contract-shape:pure-function
/// Scenario: Nothing runs on a schedule until the operator turns it on
///   Given the new infrastructure root
///   Then both schedules follow one setting, and that setting starts off
/// ```
#[test]
fn nothing_runs_on_a_schedule_until_the_operator_turns_it_on() {
    let tofu = prod_indexer_tofu();

    let schedules = resources(&tofu, "aws_scheduler_schedule");
    assert_eq!(schedules.len(), 2, "pass and probe");
    for schedule in schedules {
        assert_eq!(
            setting(schedule, "state").as_deref(),
            Some(r#"var.schedules_enabled ? "ENABLED" : "DISABLED""#)
        );
    }
    let opening = r#"variable "schedules_enabled""#;
    let at = tofu.find(opening).expect("the variable");
    let variable = braced(&tofu, at + tofu[at..].find('{').expect("a block"));
    assert_eq!(setting(variable, "default").as_deref(), Some("false"));
}

/// OP-15
/// ```gherkin
/// @infrastructure @U-STZ-9 @D-STZ-25 @contract-shape:pure-function
/// Scenario: The registry keeps the newest 10 builds
///   Given the new infrastructure root
///   Then its lifecycle rule for built images keeps exactly 10
/// ```
#[test]
fn the_registry_keeps_the_newest_10_builds() {
    let squeezed: String = prod_indexer_tofu().split_whitespace().collect();

    let kept = |number: &str| {
        squeezed.match_indices(number).any(|(at, found)| {
            !squeezed[at + found.len()..].starts_with(|c: char| c.is_ascii_digit())
        })
    };

    assert!(squeezed.contains("aws_ecr_lifecycle_policy"), "the policy");
    assert!(
        kept("countNumber=10") || kept(r#""countNumber":10"#),
        "keep 10"
    );
}

/// OP-16
/// ```gherkin
/// @infrastructure @D-STZ-23 @contract-shape:pure-function
/// Scenario: The probe watches the same address before and after the cut-over
///   Given the new infrastructure root
///   Then the probe's address comes from the distribution
///   And it does not depend on the phase, and it is not the public name
/// ```
#[test]
fn the_probe_watches_the_same_address_before_and_after_the_cut_over() {
    let tofu = prod_indexer_tofu();

    let lines: Vec<&str> = tofu
        .lines()
        .filter(|line| line.contains("PROBE_BASE_URL"))
        .collect();

    assert!(!lines.is_empty(), "the probe has a base URL");
    for line in lines {
        assert!(line.contains("aws_cloudfront_distribution"), "{line}");
        assert!(!line.contains("phase"), "{line}");
        assert!(!line.contains("index.openlore"), "{line}");
    }
}

/// OP-17 (a healthy live search never has a 5xx, so A11's seen-check uses the
/// candidate series, and the alarm's own `:live` dimension is checked for shape)
/// ```gherkin
/// @driving_adapter @real-io @KPI-STZ-G3 @kpi @contract-shape:unbounded-preservation
/// Scenario Outline: The status report looks for the search error metric where a test put it
///   Given the error metric <state> on the candidate
///   When the operator asks for the status
///   Then the report says it is <seen>
///   Examples:
///     | state        | seen     |
///     | has data     | seen     |
///     | has no data  | not seen |
/// ```
#[test]
fn the_status_report_looks_for_the_search_error_metric_where_a_test_put_it() {
    each_row([(true, "seen"), (false, "not seen")], |(has_data, seen)| {
        let stubs = Stubs::new(Posture {
            candidate_5xx_seen: *has_data,
            ..Posture::default()
        });

        let run = stubs.run(&["lambda", "status"], &[], "");

        assert_reached(
            &stubs,
            &[
                "cloudwatch list-metrics",
                "Url5xxCount",
                "openlore-indexer-search:candidate",
            ],
        );
        let report = run
            .out
            .lines()
            .find(|line| line.contains("Url5xxCount"))
            .unwrap_or_else(|| panic!("the report names the metric\n{}", run.out));
        assert!(report.ends_with(seen), "{report}");
    });
}

/// OP-E17 (a `prev=` record names the version before this one, so falling back to
/// it would publish the wrong image)
/// ```gherkin
/// @driving_adapter @real-io @error @U-STZ-2 @D-STZ-29 @C6b @contract-shape:unbounded-preservation
/// Scenario: A republish that cannot read the live image is refused, whatever the record says
///   Given the live version's image cannot be read
///   And the live alias still records the version it replaced
///   When the operator republishes
///   Then the republish is refused, and no function changed
/// ```
#[test]
fn a_republish_that_cannot_read_the_live_image_is_refused() {
    let stubs = Stubs::new(Posture {
        image_in_ecr: true,
        live_image_readable: false,
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "republish"], &[], "");

    assert_ne!(run.status, 0, "{}", run.out);
    assert_refused(&run);
    let calls = stubs.calls();
    for change in ["update-function-code", "publish-version", "update-alias"] {
        assert!(!called(&calls, &[change]), "{change}\n{calls}");
    }
}

/// OP-E18
/// ```gherkin
/// @driving_adapter @real-io @error @U-STZ-9 @D-STZ-25 @C7a @contract-shape:unbounded-preservation
/// Scenario Outline: A copy that would push out an image in use is refused
///   Given the registry is full, and its oldest build is <what it is>
///   When the operator deploys a new build
///   Then the deploy is refused, and nothing was copied or changed
///   Examples:
///     | what it is                              |
///     | the one live runs                       |
///     | the one the next version would be made of |
/// ```
#[test]
fn a_copy_that_would_push_out_an_image_in_use_is_refused() {
    each_row([Holder::Live, Holder::Latest], |holder| {
        let stubs = Stubs::new(Posture {
            oldest_image: *holder,
            ..Posture::default()
        });

        let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

        assert_stopped_with_nothing_flipped(&run, &stubs);
        assert_refused(&run);
        assert_reached(&stubs, &["ecr describe-images"]);
        let calls = stubs.calls();
        for change in ["crane copy", "update-function-code"] {
            assert!(!called(&calls, &[change]), "{change}\n{calls}");
        }
    });
}

/// OP-E19 (a failure after step 8 leaves `$LATEST` on the new digest, so that copy
/// is referenced and stays)
/// ```gherkin
/// @driving_adapter @real-io @error @U-STZ-9 @D-STZ-25 @C7b @contract-shape:bounded-change
/// Scenario Outline: A failed deploy removes the copy it made, and only that
///   Given <where the build stands>
///   And updating the functions fails
///   When the operator deploys
///   Then the deploy stops, and the build <what happens to it>
///   Examples:
///     | where the build stands                  | what happens to it    |
///     | this deploy copied it, and nothing uses it | is deleted again    |
///     | the registry already held it            | is left where it is   |
///     | this deploy copied it, and the next version is made of it | stays |
/// ```
#[test]
fn a_failed_deploy_removes_the_copy_it_made_and_only_that() {
    each_row(
        [
            (false, false, true),
            (true, false, false),
            (false, true, false),
        ],
        |(already_there, held_by_latest, deleted)| {
            let stubs = Stubs::new(Posture {
                image_in_ecr: *already_there,
                latest_holds_the_build: *held_by_latest,
                code_update_fails: true,
                live_runs_the_build: false,
                ..Posture::default()
            });

            let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

            assert_stopped_with_nothing_flipped(&run, &stubs);
            assert_reached(&stubs, &["update-function-code"]);
            assert_eq!(
                called(&stubs.calls(), &["batch-delete-image", &digest()]),
                *deleted,
                "{}",
                stubs.calls()
            );
        },
    );
}

/// OP-E20
/// ```gherkin
/// @driving_adapter @real-io @error @D-STZ-26 @DV-STZ-9 @C7b @contract-shape:bounded-change
/// Scenario Outline: A deploy stopped before the flip puts its candidate back
///   Given <what stops it>
///   When the operator deploys
///   Then the deploy stops, no live alias moved, and the candidate is back on the live version
///   Examples:
///     | what stops it                       |
///     | the pass gate fails                 |
///     | the probe gate finds the index dead |
/// ```
#[test]
fn a_deploy_stopped_before_the_flip_puts_its_candidate_back() {
    each_row(
        [
            Posture {
                pass_gate_exit: 2,
                ..Posture::default()
            },
            Posture {
                not_live: 1,
                ..Posture::default()
            },
        ],
        |posture| {
            let stubs = Stubs::new(*posture);

            let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

            assert_stopped_with_nothing_flipped(&run, &stubs);
            let calls = stubs.calls();
            let moved = |version: &str| {
                let version = format!("--function-version {version}");
                line_of(
                    &calls,
                    &["update-alias", FUNCTIONS[0], "candidate", &version],
                )
            };
            let (forward, back) = (moved("8"), moved("7"));
            assert!(forward.is_some() && forward < back, "{calls}");
        },
    );
}

/// OP-E21 (probe has one slot, so a 429 means a scheduled probe holds it)
/// ```gherkin
/// @driving_adapter @real-io @error @U-STZ-7 @KPI-STZ-G5 @kpi @C7c @contract-shape:unbounded-preservation
/// Scenario: The error alarm drill waits out a busy probe
///   Given a scheduled probe holds the one slot at first
///   When the operator test-fires the error alarm
///   Then the drill waits, says so, and its invoke lands
///   And it sees the alarm fire and clear
/// ```
#[test]
fn the_error_alarm_drill_waits_out_a_busy_probe() {
    let stubs = Stubs::new(Posture {
        throttled_probe_gates: 1,
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "test-alarm", "errors"], &[], "");

    assert_eq!(run.status, 0, "{}\n{}", run.out, stubs.calls());
    let calls = stubs.calls();
    let attempts = calls
        .lines()
        .filter(|line| line.contains("lambda invoke") && line.contains(FUNCTIONS[2]))
        .count();
    assert_eq!(attempts, 2, "{calls}");
    assert!(calls.contains("sleep 30"), "{calls}");
    assert!(run.out.contains("retr"), "it says so\n{}", run.out);
    assert!(
        calls.matches("cloudwatch describe-alarms").count() >= 2,
        "{calls}"
    );
}

/// Everything on the real `PATH` less `absent`, after the stubs (less `absent`).
/// A stub alone cannot hide a tool: the real `PATH` follows the stubs, and on CI
/// `gh` sits in `/usr/bin` beside what the script does need.
fn path_without(stubs: &Stubs, absent: &[&str]) -> String {
    for tool in absent {
        std::fs::remove_file(stubs.dir.path().join(tool)).expect("remove the stub");
    }
    let rest = stubs.dir.path().join("rest");
    std::fs::create_dir(&rest).expect("make the rest of the PATH");
    let real_path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&real_path) {
        for tool in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let name = tool.file_name();
            if !absent.iter().any(|gone| name == *gone) {
                // The first on the `PATH` wins, as it would have.
                let _ = std::os::unix::fs::symlink(tool.path(), rest.join(name));
            }
        }
    }
    format!("{}:{}", stubs.dir.path().display(), rest.display())
}

/// Crafter-owned. The kill switch must work on a bare machine during an incident,
/// so a verb requires only what it runs; the verbs that handle a build still
/// require the tools that verify and copy it.
#[test]
fn the_operator_verbs_need_no_image_tools_and_the_build_verbs_need_them_all() {
    const IMAGE_TOOLS: [&str; 3] = ["gh", "cosign", "crane"];
    let in_ecr = Posture {
        image_in_ecr: true,
        ..Posture::default()
    };
    each_row(
        [
            vec!["stop"],
            vec!["start"],
            vec!["pass-off"],
            vec!["pass-on"],
            vec!["probe-off"],
            vec!["probe-on"],
            vec!["rollback"],
            vec!["reset-index"],
            vec!["restore-index", "v1"],
            vec!["trigger"],
            vec!["probe"],
            vec!["status"],
            vec!["kpi"],
        ],
        |verb| {
            let stubs = Stubs::new(in_ecr);
            let path = path_without(&stubs, &IMAGE_TOOLS);
            let mut args = vec!["lambda"];
            args.extend(verb);

            let run = stubs.run(&args, &[("PATH", &path)], "reset\n");

            assert_eq!(run.status, 0, "{}\n{}", run.out, stubs.calls());
            assert_reached(&stubs, &["aws "]);
        },
    );

    let stubs = Stubs::new(in_ecr);
    let path = path_without(&stubs, &IMAGE_TOOLS);
    stubs.run(&["lambda", "stop"], &[("PATH", &path)], "");
    assert_reached(&stubs, &["put-function-concurrency", FUNCTIONS[0], "0"]);
    assert_reached(&stubs, &["update-distribution"]);

    let verbs = [vec!["deploy", SHA], vec!["republish"], vec!["seed", SHA]];
    let rows = verbs
        .iter()
        .flat_map(|verb| IMAGE_TOOLS.map(|tool| (verb.clone(), tool)));
    each_row(rows, |(verb, tool)| {
        let stubs = Stubs::new(in_ecr);
        let path = path_without(&stubs, &[tool]);
        let mut args = vec!["lambda"];
        args.extend(verb);

        let run = stubs.run(&args, &[("PATH", &path)], "");

        assert_ne!(run.status, 0, "{}\n{}", run.out, stubs.calls());
        assert!(
            run.out.contains(&format!("missing tool: {tool}")),
            "{}",
            run.out
        );
    });
}

/// `curl` as `lambda measure` sees it: every call quick and 200, but for the one
/// `(path, nth call on that path)` that answers `status` in `seconds`. On the raw
/// path 1-5 are the cold calls, 6 warms up, and 7-26 are the warm calls; on the
/// edge 1 warms up, and 2-21 are the warm calls.
fn measured_curl(call: (&str, u32), status: u16, seconds: &str) -> String {
    let (slow_path, slow_call) = call;
    format!(
        r#"
url=""
for a in "$@"; do
  case "$a" in http*) url="$a" ;; esac
done
case "$url" in
  *searchClaims) ;;
  *) printf '404 0.05'; exit 0 ;;
esac
case "$url" in
  *cloudfront*) path=edge; host=cloudfront ;;
  *) path=raw; host=lambda-url ;;
esac
call=$(grep -c "^curl .*$host.*searchClaims" "$CALLS")
if [ "$path" = {slow_path} ] && [ "$call" = {slow_call} ]; then
  printf '%s %s' {status} {seconds}
else
  printf '200 0.05'
fi
"#
    )
}

/// Crafter-owned. `lambda measure` is the gate on R-STZ-1 and R-STZ-2: it passes
/// only when each measured call answers 200 under its limit, and whatever it
/// decides, the candidate and `$LATEST` end up as it found them.
#[test]
fn measure_passes_only_when_every_call_meets_its_limit_on_each_path() {
    let in_ecr = Posture {
        image_in_ecr: true,
        ..Posture::default()
    };
    each_row(
        [
            (("nowhere", 0), 200, "0.05", 0),
            (("raw", 5), 200, "2.999", 0),
            (("raw", 5), 200, "3.000", 1),
            (("raw", 1), 503, "0.05", 1),
            (("raw", 6), 200, "9.000", 0),
            (("raw", 26), 200, "0.199", 0),
            (("raw", 26), 200, "0.200", 1),
            (("raw", 7), 500, "0.05", 1),
            (("edge", 1), 200, "9.000", 0),
            (("edge", 21), 200, "0.199", 0),
            (("edge", 21), 200, "0.200", 1),
            (("edge", 2), 429, "0.05", 1),
        ],
        |(call, status, seconds, exit)| {
            let stubs = Stubs::new(in_ecr);
            stubs.add("curl", &measured_curl(*call, *status, seconds));

            let run = stubs.run(&["lambda", "measure"], &[], "");

            assert_eq!(run.status, *exit, "{}\n{}", run.out, stubs.calls());
            let verdict = if *exit == 0 { "PASS" } else { "FAIL" };
            assert!(
                run.out
                    .contains(&format!("measure: {verdict} {}", digest())),
                "{}",
                run.out
            );
            let calls = stubs.calls();
            let (publishes, deletes) = (
                calls.matches("lambda publish-version").count(),
                calls.matches("lambda delete-function").count(),
            );
            assert_eq!((publishes, deletes), (5, 5), "{calls}");
            let last = |tokens: &[&str]| {
                calls
                    .lines()
                    .rfind(|line| tokens.iter().all(|token| line.contains(token)))
                    .unwrap_or_default()
                    .to_string()
            };
            assert!(
                last(&["update-alias", "candidate"]).contains("--function-version 7"),
                "{calls}"
            );
            assert!(
                last(&["update-function-code"]).contains(&"c".repeat(64)),
                "{calls}"
            );
            assert!(!called(&calls, &["update-alias", "live"]), "{calls}");
        },
    );
}

/// Crafter-owned. The check comes first, so a wrong setting costs no version.
#[test]
fn measure_stops_first_when_search_is_not_at_its_concurrency() {
    let stubs = Stubs::new(Posture {
        image_in_ecr: true,
        search_concurrency: 0,
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "measure"], &[], "");

    assert_eq!(run.status, 1, "{}\n{}", run.out, stubs.calls());
    assert_refused(&run);
    assert!(
        !called(&stubs.calls(), &["publish-version"]),
        "{}",
        stubs.calls()
    );
}

/// OP-E22
/// ```gherkin
/// @driving_adapter @real-io @error @U-STZ-1 @C6a @contract-shape:unbounded-preservation
/// Scenario Outline: Standby needs to be told what to stop
///   When the operator asks for standby with <flags>
///   Then it is refused, and nothing was sent to the host
///   Examples:
///     | flags       |
///     | no flag     |
///     | a wrong one |
/// ```
#[test]
fn standby_needs_to_be_told_what_to_stop() {
    each_row([vec!["standby"], vec!["standby", "--all"]], |args| {
        let stubs = Stubs::new(Posture::default());

        let run = stubs.run(args, &[], "");

        assert_ne!(run.status, 0, "{}", run.out);
        assert_refused(&run);
        assert!(
            !called(&stubs.calls(), &["ssm send-command"]),
            "{}",
            stubs.calls()
        );
    });
}

/// OP-E23, crafter-owned (review B1). Lambda publishes no version for unchanged code and
/// configuration: it answers with the latest one. A flip to that would write a `prev=` naming the
/// version `live` already holds, and the real rollback target would be gone. Some functions
/// unchanged is refused too: from here that is one half-flipped state, whose `prev=` must stay.
#[test]
fn a_publish_that_changed_nothing_is_refused_before_anything_moves() {
    let deploy = vec!["lambda", "deploy", SHA];
    let republish = vec!["lambda", "republish"];
    each_row(
        [
            (deploy.clone(), [true, true, true]),
            (republish.clone(), [true, true, true]),
            (deploy, [true, false, false]),
            (republish.clone(), [false, true, false]),
            (republish, [false, false, true]),
        ],
        |(args, unchanged)| {
            let stubs = Stubs::new(Posture {
                image_in_ecr: true,
                publish_answers_live: *unchanged,
                ..Posture::default()
            });

            let run = stubs.run(args, &[], "");

            assert_ne!(run.status, 0, "{}\n{}", run.out, stubs.calls());
            assert_refused(&run);
            let refusal = run
                .out
                .lines()
                .find(|line| line.contains("nothing changed"))
                .unwrap_or_else(|| panic!("it says nothing changed\n{}", run.out));
            for (function, unchanged) in FUNCTIONS.iter().zip(unchanged) {
                assert_eq!(refusal.contains(function), *unchanged, "{refusal}");
            }
            let calls = stubs.calls();
            assert_eq!(calls.matches("publish-version").count(), 3, "{calls}");
            assert!(!called(&calls, &["update-alias"]), "{calls}");
            assert!(!called(&calls, &["lambda invoke"]), "{calls}");
        },
    );
}

/// OP-E24, crafter-owned (review H1). `lambda rollback` takes every function back one record, so
/// a function that flipped beside one that did not would go a version too far. Step 15 therefore
/// undoes what flipped, each under the record it had, and when an undo fails too it says exactly
/// what is where. Rows: which `live` moves fail; then, for search, the pass and the probe, the
/// version each ends on.
#[test]
fn a_flip_that_stops_at_the_pass_or_the_probe_is_undone_and_names_what_is_where() {
    const SEARCH: usize = 0;
    const PASS: usize = 1;
    const PROBE: usize = 2;
    let moves: [(&'static [(usize, u32)], [u32; 3]); 5] = [
        (&[(PASS, 8)], [7, 7, 7]),
        (&[(PROBE, 8)], [7, 7, 7]),
        (&[(PASS, 8), (SEARCH, 7)], [8, 7, 7]),
        (&[(PROBE, 8), (PASS, 7)], [7, 8, 7]),
        (&[(PROBE, 8), (PASS, 7), (SEARCH, 7)], [8, 8, 7]),
    ];
    each_row(moves, |(fails, ended_on)| {
        let stubs = Stubs::new(Posture {
            live_moves_that_fail: fails,
            ..Posture::default()
        });

        let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

        assert_ne!(run.status, 0, "{}\n{}", run.out, stubs.calls());
        let calls = stubs.calls();
        let record = format!("prev=6 {SHA} {}", digest());
        let back = |function: &str| {
            line_of(
                &calls,
                &[
                    "update-alias",
                    function,
                    "--name live --function-version 7",
                    &record,
                ],
            )
        };
        let forward = line_of(
            &calls,
            &["update-alias", FUNCTIONS[0], "live", "--function-version 8"],
        );
        assert!(forward.is_some() && forward < back(FUNCTIONS[0]), "{calls}");
        let pass_tried_forward = called(
            &calls,
            &["update-alias", FUNCTIONS[1], "live", "--function-version 8"],
        );
        assert!(pass_tried_forward, "{calls}");
        let pass_flipped = !fails.contains(&(PASS, 8));
        assert_eq!(back(FUNCTIONS[1]).is_some(), pass_flipped, "{calls}");
        assert_eq!(
            called(&calls, &["update-alias", FUNCTIONS[2], "live"]),
            pass_flipped,
            "the probe is tried only after the pass\n{calls}"
        );
        let candidate_back = called(
            &calls,
            &[
                "update-alias",
                FUNCTIONS[0],
                "candidate",
                "--function-version 7",
            ],
        );
        assert_eq!(candidate_back, ended_on[SEARCH] == 7, "{calls}");
        for (function, version) in FUNCTIONS.iter().zip(ended_on) {
            let state = format!("{function} on version {version}");
            assert!(run.out.contains(&state), "{state}\n{}", run.out);
        }
        let undo_failures = fails.iter().filter(|(_, version)| *version == 7);
        for (function, _) in undo_failures {
            let said = format!("could not move {} back to version 7", FUNCTIONS[*function]);
            let line = run
                .out
                .lines()
                .find(|line| line.contains(&said))
                .unwrap_or_else(|| panic!("{said}\n{}", run.out));
            assert!(line.contains(&record), "the record to put back: {line}");
        }
        assert!(
            !called(&calls, &["list-versions-by-function"]),
            "a stopped deploy prunes nothing\n{calls}"
        );
    });
}

const NOT_FOUND: &str = "An error occurred (404) when calling the HeadObject operation: Not Found";
const NO_SUCH_KEY: &str = "An error occurred (NoSuchKey) when calling the HeadObject operation";
const FORBIDDEN: &str = "An error occurred (403) when calling the HeadObject operation: Forbidden";
const SLOW_DOWN: &str = "An error occurred (503) when calling the HeadObject operation: SlowDown";
const NO_NETWORK: &str = "Could not connect to the endpoint URL";

/// OP-E25, crafter-owned (review H2). A deploy, a reset and the report each read a failed read as
/// an empty bucket before: `pre-publish index none` in the record, and the id a reset prints gone.
/// A `die` in a command substitution ends only the subshell, so each caller is run to its own
/// stop: step 7 and step 11 of a deploy, then a reset. Each error is one row of step 7.
#[test]
fn a_failed_read_of_the_index_stops_the_verb() {
    each_row([FORBIDDEN, SLOW_DOWN, NO_NETWORK], |error| {
        let stubs = Stubs::new(Posture {
            index_head_fails: Some((1, error)),
            ..Posture::default()
        });

        let run = stubs.run(&["lambda", "deploy", SHA], &[], "");

        assert_stopped_with_nothing_flipped(&run, &stubs);
        assert_refused(&run);
        assert!(run.out.contains(*error), "{}", run.out);
        assert_reached(&stubs, &["s3api head-object"]);
        let calls = stubs.calls();
        assert!(!called(&calls, &["update-function-code"]), "{calls}");
    });

    let at_step_11 = Stubs::new(Posture {
        index_head_fails: Some((2, FORBIDDEN)),
        ..Posture::default()
    });
    let run = at_step_11.run(&["lambda", "deploy", SHA], &[], "");
    assert_stopped_with_nothing_flipped(&run, &at_step_11);
    assert_refused(&run);
    let calls = at_step_11.calls();
    assert!(!called(&calls, &["lambda invoke"]), "{calls}");
    let candidate = |version: &str| {
        let version = format!("--function-version {version}");
        line_of(
            &calls,
            &["update-alias", FUNCTIONS[0], "candidate", &version],
        )
    };
    let forward = candidate("8");
    assert!(forward.is_some() && forward < candidate("7"), "{calls}");

    let reset = Stubs::new(Posture {
        index_head_fails: Some((1, FORBIDDEN)),
        ..Posture::default()
    });
    let run = reset.run(&["lambda", "reset-index", "--trigger"], &[], "reset\n");
    assert_ne!(run.status, 0, "{}\n{}", run.out, reset.calls());
    assert_refused(&run);
    let calls = reset.calls();
    assert!(!called(&calls, &["delete-object"]), "{calls}");
    assert!(!called(&calls, &["lambda invoke"]), "{calls}");
}

/// OP-E25, the other side: a missing object is what an empty bucket really answers.
#[test]
fn a_missing_index_object_is_an_empty_bucket() {
    each_row([NOT_FOUND, NO_SUCH_KEY], |error| {
        let stubs = Stubs::new(Posture {
            index_head_fails: Some((1, error)),
            ..Posture::default()
        });

        let run = stubs.run(&["lambda", "reset-index"], &[], "reset\n");

        assert_eq!(run.status, 0, "{}\n{}", run.out, stubs.calls());
        assert!(run.out.contains("noncurrent: none"), "{}", run.out);
        assert_eq!(
            stubs.calls().matches("delete-object").count(),
            1,
            "{}",
            stubs.calls()
        );
    });
}

/// OP-E25 in the report: the section says it stopped, and the others still print.
#[test]
fn the_status_report_does_not_show_a_failed_read_of_the_index_as_empty() {
    let stubs = Stubs::new(Posture {
        index_head_fails: Some((1, FORBIDDEN)),
        ..Posture::default()
    });

    let run = stubs.run(&["lambda", "status"], &[], "");

    assert!(!run.out.contains("etag none"), "{}", run.out);
    assert!(
        run.out.contains("(this section stopped early"),
        "{}",
        run.out
    );
    assert!(run.out.contains("--- alarms"), "{}", run.out);
}

/// OP-18, crafter-owned (review H6). `stop` lasts until the next boot, and the soak is 7 days. A
/// reboot would bring back a second writer of the health line and a pass timer with a slot to
/// catch up, so the timers are disabled too. `resume` re-enables both, which its own test pins.
#[test]
fn standby_keeps_the_timers_off_through_a_reboot() {
    let stubs = Stubs::new(Posture::default());

    let run = stubs.run(&["host", "standby", "--timers"], &[], "");

    assert_eq!(run.status, 0, "{}\n{}", run.out, stubs.calls());
    let calls = stubs.calls();
    for timer in [
        "openlore-indexer-pass.timer",
        "openlore-indexer-health.timer",
    ] {
        let stopped = line_of(&calls, &["systemctl", "stop", timer]);
        let disabled = line_of(&calls, &["systemctl", "disable", timer]);
        assert!(stopped.is_some() && stopped < disabled, "{timer}\n{calls}");
    }

    let container = Stubs::new(Posture::default());
    let run = container.run(&["host", "standby", "--container"], &[], "");
    assert_eq!(run.status, 0, "{}\n{}", run.out, container.calls());
    assert!(
        !called(&container.calls(), &["systemctl"]),
        "the container's standby leaves the timers\n{}",
        container.calls()
    );
}

/// `gh`, answering `ci.yml` with success and `deploy-pds-check.yml` with `conclusion`.
fn gh_answering(conclusion: &str) -> String {
    format!(
        r#"case "$*" in
  *deploy-pds-check.yml*) echo '{conclusion}' ;;
  *) echo success ;;
esac"#
    )
}

/// OP-E26, crafter-owned (review M2). `deploy-pds-check.yml` is its own workflow, and it only runs
/// when a commit touches `deploy/`. So no run says nothing about the commit, and must not block a
/// deploy, but a run that is there has to have succeeded. An empty conclusion is a run in progress.
/// Rows: the conclusion, then whether the deploy gets through.
#[test]
fn a_red_static_checks_run_stops_a_deploy_and_no_run_does_not() {
    each_row(
        [
            ("success", true),
            ("none", true),
            ("failure", false),
            ("cancelled", false),
            ("timed_out", false),
            ("", false),
        ],
        |(conclusion, passes)| {
            for reference in [SHA.to_string(), digest()] {
                let stubs = Stubs::new(Posture::default());
                stubs.add("gh", &gh_answering(conclusion));

                let run = stubs.run(&["lambda", "deploy", &reference], &[], "");

                let asked = ["gh run list", "deploy-pds-check.yml", "--branch main", SHA];
                assert_reached(&stubs, &asked);
                if *passes {
                    assert_flipped(&run, &stubs);
                } else {
                    assert_stopped_with_nothing_flipped(&run, &stubs);
                    assert_refused(&run);
                    assert!(run.out.contains("deploy-pds-check.yml"), "{}", run.out);
                    let calls = stubs.calls();
                    assert!(!called(&calls, &["crane copy"]), "{calls}");
                }
            }
        },
    );
}

/// Crafter-owned (step 08-09). `resume` has no scenario, since only a real host
/// can show it working, and the runbook covers that. What stubs can pin is the
/// order: isolation before any host file, the container serving before the pass
/// timer, and no pass of its own, which is the runbook's `trigger`.
#[test]
fn resume_brings_the_container_back_before_the_timers_and_runs_no_pass() {
    let laptop = Stubs::new(Posture::default());
    let run = laptop.run(&["resume"], &[], "");
    assert_eq!(run.status, 0, "{}\n{}", run.out, laptop.calls());
    assert_reached(&laptop, &["ssm send-command", "host resume"]);
    assert!(run.out.contains("PDS _health:"), "{}", run.out);

    let base = tempfile::tempdir().expect("base dir");
    let sites = tempfile::tempdir().expect("sites dir");
    let base_path = base.path().display().to_string();
    let sites_path = sites.path().display().to_string();
    let env = [
        ("INDEXER_BASE_DIR", base_path.as_str()),
        ("INDEXER_CADDY_SITES_DIR", sites_path.as_str()),
    ];
    let host = Stubs::new(Posture::default());
    host.add("install", "exit 0");
    host.add(
        "docker",
        &format!(
            r#"case "$*" in
  run*169.254.169.254*) exit 7 ;;
  *' ps -q caddy'*) echo pds-caddy-1 ;;
  inspect*) echo '{sites_path}:/etc/caddy/sites' ;;
  *'cat /etc/caddy/Caddyfile'*) echo 'import /etc/caddy/sites/*.caddy' ;;
  *'caddy adapt'*) echo '"index.openlore.jeffbailey.us"' ;;
esac"#
        ),
    );
    let state = base.path().join("state");
    std::fs::create_dir(&state).expect("state dir");

    let nothing_deployed = host.run(&["host", "resume"], &env, "");
    assert_ne!(nothing_deployed.status, 0, "{}", nothing_deployed.out);
    assert_refused(&nothing_deployed);
    assert!(!called(&host.calls(), &["up -d"]), "{}", host.calls());

    let release = format!("2026-10-01T00:00:00Z {SHA} {} ready_s=4\n", digest());
    std::fs::write(state.join("releases"), release).expect("releases");
    let run = host.run(&["host", "resume"], &env, "");
    assert_eq!(run.status, 0, "{}\n{}", run.out, host.calls());

    let calls = host.calls();
    let order = [
        "169.254.169.254",
        "daemon-reload",
        "enable --now openlore-indexer-health.timer",
        "up -d indexer",
        "--resolve",
        "enable --now openlore-indexer-pass.timer",
    ]
    .map(|step| line_of(&calls, &[step]).unwrap_or_else(|| panic!("no {step}\n{calls}")));
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{calls}");
    assert!(!called(&calls, &["--force-recreate"]), "{calls}");
    assert!(
        !called(&calls, &["openlore-indexer-pass.service"]),
        "{calls}"
    );
}
