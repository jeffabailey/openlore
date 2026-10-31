//! Crafter-owned checks of `deploy/tofu/environments/prod-indexer` and of the image
//! CI builds for it: the settings the DEVOPS tables fix that no scenario pins. Each
//! is a row, read whole, so `1` is not `10`. Nothing here runs tofu or reaches AWS.
//!
//! The reader repeats the small one in `scale_to_zero_platform.rs`, which is a
//! DISTILL suite and has no library to share it through.
#![cfg(unix)]

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const PROD_INDEXER_ROOT: &str = "deploy/tofu/environments/prod-indexer";
const FUNCTIONS: [&str; 3] = ["search", "pass", "probe"];

/// The lowest provider with `invoked_via_function_url` (DEVOPS § Backend).
const LOWEST_AWS_PROVIDER_MINOR: u32 = 67;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = workspace().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{path:?}: {err}"))
}

/// Every `.tf` file of the root, with commented lines gone.
fn tofu() -> String {
    let dir = workspace().join(PROD_INDEXER_ROOT);
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|err| panic!("{dir:?}: {err}"))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "tf"))
        .collect();
    files.sort();
    files
        .iter()
        .map(|path| std::fs::read_to_string(path).expect("a tofu file"))
        .collect::<Vec<_>>()
        .join("\n")
        .lines()
        .filter(|line| !line.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
}

fn lambda_env() -> BTreeMap<String, String> {
    serde_json::from_str(&read(&format!("{PROD_INDEXER_ROOT}/lambda-env.json")))
        .expect("a flat object of strings")
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

/// The body of the block with this exact opening, labels included.
fn block<'a>(text: &'a str, opening: &str) -> &'a str {
    let at = text
        .find(&format!("{opening} {{"))
        .unwrap_or_else(|| panic!("no {opening}"));
    braced(text, at + opening.len() + 1)
}

fn resource<'a>(text: &'a str, kind: &str, name: &str) -> &'a str {
    block(text, &format!(r#"resource "{kind}" "{name}""#))
}

fn policy<'a>(text: &'a str, name: &str) -> &'a str {
    block(text, &format!(r#"data "aws_iam_policy_document" "{name}""#))
}

/// The body of every nested `<name> { … }`.
fn nested<'a>(body: &'a str, name: &str) -> Vec<&'a str> {
    let mut found = Vec::new();
    let mut at = 0;
    for line in body.split_inclusive('\n') {
        let opens = line
            .trim_start()
            .strip_prefix(name)
            .is_some_and(|rest| rest.trim() == "{");
        if opens {
            found.push(braced(body, at + line.find('{').expect("a block")));
        }
        at += line.len();
    }
    found
}

/// The only nested `<name>` block.
fn only<'a>(body: &'a str, name: &str) -> &'a str {
    let found = nested(body, name);
    assert_eq!(found.len(), 1, "one {name}");
    found[0]
}

/// The whole value of `key = value` in this body.
fn setting<'a>(body: &'a str, key: &str) -> Option<&'a str> {
    body.lines().find_map(|line| {
        let value = line.trim_start().strip_prefix(key)?.trim_start();
        value.strip_prefix('=').map(str::trim)
    })
}

/// Every row is the whole value of its key in this body.
fn assert_rows(what: &str, body: &str, rows: &[(&str, &str)]) {
    for (key, value) in rows {
        assert_eq!(setting(body, key), Some(*value), "{what}: {key}");
    }
}

#[test]
fn each_function_is_the_size_and_shape_devops_set() {
    let tofu = tofu();
    let rows = [
        ("search", "512", "10", "3", r#"["search"]"#),
        ("pass", "512", "840", "1", r#"["pass"]"#),
        ("probe", "128", "30", "1", r#"["probe"]"#),
    ];

    for (name, memory, timeout, concurrency, command) in rows {
        let function = resource(&tofu, "aws_lambda_function", name);
        let function_name = format!(r#""openlore-indexer-{name}""#);
        let role = format!("aws_iam_role.{name}.arn");
        assert_rows(
            name,
            function,
            &[
                ("function_name", &function_name),
                ("role", &role),
                ("package_type", r#""Image""#),
                ("architectures", r#"["arm64"]"#),
                ("image_uri", "var.bootstrap_image_uri"),
                ("publish", "false"),
                ("memory_size", memory),
                ("timeout", timeout),
                ("reserved_concurrent_executions", concurrency),
            ],
        );
        assert_rows(
            name,
            only(function, "image_config"),
            &[
                ("entry_point", "local.lambda_entry_point"),
                ("command", command),
                ("working_directory", r#""/""#),
            ],
        );
        assert_rows(
            name,
            only(function, "logging_config"),
            &[
                ("log_format", r#""JSON""#),
                ("log_group", "local.lambda_log_group"),
                ("system_log_level", r#""INFO""#),
            ],
        );
        let ignored = setting(only(function, "lifecycle"), "ignore_changes").expect("a list");
        assert!(ignored.contains("image_uri"), "{name}: deploy.sh owns it");
        assert!(!function.contains("vpc_config"), "{name}: no VPC");
        assert!(!function.contains("tracing_config"), "{name}: X-Ray off");
    }
    for name in ["search", "pass"] {
        let function = resource(&tofu, "aws_lambda_function", name);
        assert_rows(
            name,
            only(function, "ephemeral_storage"),
            &[("size", "512")],
        );
    }
    assert_rows(
        "the entry point",
        &tofu,
        &[(
            "lambda_entry_point",
            r#"["/usr/local/bin/openlore-indexer-lambda"]"#,
        )],
    );
}

/// `OPENLORE_INDEXER_PURGE_UNLISTED=1` is a row: anything else, `10` included,
/// leaves the purge off or refuses the start.
#[test]
fn the_shared_environment_file_holds_each_setting_whole() {
    let expected: BTreeMap<String, String> = [
        ("HOME", "/tmp"),
        ("OPENLORE_INDEXER_DUCKDB_MEMORY_LIMIT_MB", "48"),
        ("OPENLORE_INDEXER_DUCKDB_THREADS", "1"),
        ("OPENLORE_INDEXER_INDEX_PATH", "/tmp/index.duckdb"),
        ("OPENLORE_INDEXER_LAMBDA_FRESHNESS_TTL_SECS", "60"),
        ("OPENLORE_INDEXER_LAMBDA_LAST_PASS_STALE_SECS", "2700"),
        ("OPENLORE_INDEXER_MAX_CONCURRENT_FETCHES", "4"),
        ("OPENLORE_INDEXER_PASS_DEADLINE_SECS", "780"),
        ("OPENLORE_INDEXER_PER_DID_TIMEOUT_SECS", "30"),
        ("OPENLORE_INDEXER_PURGE_UNLISTED", "1"),
        ("OPENLORE_INDEXER_REPO_DIDS_FILE", "/tmp/config/repo-dids"),
    ]
    .into_iter()
    .map(|(name, value)| (name.to_string(), value.to_string()))
    .collect();

    assert_eq!(lambda_env(), expected);
}

#[test]
fn every_function_takes_the_file_and_the_object_store_names() {
    let tofu = tofu();
    let locals = tofu
        .match_indices("locals {")
        .map(|(at, _)| braced(&tofu, at + "locals ".len()))
        .find(|body| body.contains("object_store_env"))
        .expect("the object store names");
    let object_store = braced(
        locals,
        locals.find("object_store_env").expect("the map") + 19,
    );

    assert_rows(
        "the object store names",
        object_store,
        &[
            (
                "OPENLORE_INDEXER_LAMBDA_BUCKET",
                "aws_s3_bucket.index.bucket",
            ),
            ("OPENLORE_INDEXER_LAMBDA_INDEX_KEY", "local.index_key"),
            (
                "OPENLORE_INDEXER_LAMBDA_LAST_PASS_KEY",
                "local.last_pass_key",
            ),
        ],
    );
    assert_rows(
        "the keys",
        &tofu,
        &[
            ("index_key", r#""index/index.duckdb""#),
            ("last_pass_key", r#""index/last-pass.json""#),
            ("write_probe_key", r#""index/.write-probe""#),
        ],
    );
    let environment_of = |name: &str| {
        let function = resource(&tofu, "aws_lambda_function", name);
        only(function, "environment")
    };
    for name in FUNCTIONS {
        assert!(
            environment_of(name).contains("merge(local.lambda_env, local.object_store_env"),
            "{name}"
        );
    }
    assert_rows(
        "the pass",
        environment_of("pass"),
        &[(
            "OPENLORE_INDEXER_LAMBDA_DID_PARAMETER",
            "local.prod_outputs.indexer_repo_dids_parameter",
        )],
    );
}

/// The parser refuses a name in its namespace that it does not read, so a typo
/// here would stop a function at init.
#[test]
fn the_root_sets_only_names_the_parser_reads() {
    let parser = read("crates/openlore-indexer-lambda/src/config.rs");
    let tofu = tofu();
    let mut names: BTreeSet<String> = lambda_env().into_keys().collect();
    names.extend(
        tofu.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .filter(|word| word.starts_with("OPENLORE_INDEXER_LAMBDA_"))
            .map(str::to_string),
    );

    let unread: Vec<&String> = names
        .iter()
        .filter(|name| name.starts_with("OPENLORE_INDEXER_LAMBDA_"))
        .filter(|name| !parser.contains(&format!("\"{name}\"")))
        .collect();

    assert!(names.len() > lambda_env().len(), "the root's own names");
    assert!(unread.is_empty(), "{unread:?}");
}

#[test]
fn the_retry_setting_covers_what_the_schedules_start() {
    let tofu = tofu();
    for (name, event_age) in [("pass", "600"), ("probe", "240")] {
        let config = resource(&tofu, "aws_lambda_function_event_invoke_config", name);
        let alias = format!("aws_lambda_alias.{name}_live.name");
        assert_rows(
            name,
            config,
            &[
                ("qualifier", &alias),
                ("maximum_retry_attempts", "0"),
                ("maximum_event_age_in_seconds", event_age),
            ],
        );
    }
}

#[test]
fn each_schedule_starts_its_function_on_time_with_no_retry() {
    let tofu = tofu();
    let rows = [
        ("pass", r#""cron(0/15 * * * ? *)""#),
        ("probe", r#""cron(2/5 * * * ? *)""#),
    ];

    for (name, expression) in rows {
        let schedule = resource(&tofu, "aws_scheduler_schedule", name);
        assert_rows(
            name,
            schedule,
            &[
                ("schedule_expression", expression),
                ("schedule_expression_timezone", r#""UTC""#),
            ],
        );
        assert_rows(
            name,
            only(schedule, "flexible_time_window"),
            &[("mode", r#""OFF""#)],
        );
        let target = only(schedule, "target");
        let alias = format!("aws_lambda_alias.{name}_live.arn");
        assert_rows(name, target, &[("arn", &alias)]);
        assert_rows(
            name,
            only(target, "retry_policy"),
            &[("maximum_retry_attempts", "0")],
        );
    }
}

#[test]
fn deploy_owns_each_alias_and_search_answers_on_both() {
    let tofu = tofu();
    for (name, label) in [
        ("search_live", "live"),
        ("search_candidate", "candidate"),
        ("pass_live", "live"),
        ("probe_live", "live"),
    ] {
        let alias = resource(&tofu, "aws_lambda_alias", name);
        assert_rows(
            name,
            alias,
            &[
                ("name", &format!("\"{label}\"")),
                ("function_version", r#""$LATEST""#),
            ],
        );
        assert_rows(
            name,
            only(alias, "lifecycle"),
            &[("ignore_changes", "[function_version, description]")],
        );
    }
    for name in ["search_live", "search_candidate"] {
        let qualifier = format!("aws_lambda_alias.{name}.name");
        assert_rows(
            name,
            resource(&tofu, "aws_lambda_function_url", name),
            &[
                ("qualifier", &qualifier),
                ("authorization_type", r#""NONE""#),
                ("invoke_mode", r#""BUFFERED""#),
            ],
        );
        assert_rows(
            name,
            resource(&tofu, "aws_lambda_permission", &format!("{name}_url")),
            &[
                ("action", r#""lambda:InvokeFunctionUrl""#),
                ("qualifier", &qualifier),
                ("principal", r#""*""#),
                ("function_url_auth_type", r#""NONE""#),
            ],
        );
        assert_rows(
            name,
            resource(&tofu, "aws_lambda_permission", &format!("{name}_invoke")),
            &[
                ("action", r#""lambda:InvokeFunction""#),
                ("qualifier", &qualifier),
                ("principal", r#""*""#),
                ("invoked_via_function_url", "true"),
            ],
        );
    }
}

#[test]
fn the_index_bucket_cannot_be_destroyed_or_reached_in_the_clear() {
    let tofu = tofu();
    let bucket = resource(&tofu, "aws_s3_bucket", "index");
    assert_rows(
        "the bucket",
        bucket,
        &[("bucket", r#""openlore-indexer-index-091153021562""#)],
    );
    assert_rows(
        "the bucket",
        only(bucket, "lifecycle"),
        &[("prevent_destroy", "true")],
    );
    assert_rows(
        "versioning",
        only(
            resource(&tofu, "aws_s3_bucket_versioning", "index"),
            "versioning_configuration",
        ),
        &[("status", r#""Enabled""#)],
    );
    assert_rows(
        "the public access block",
        resource(&tofu, "aws_s3_bucket_public_access_block", "index"),
        &[
            ("block_public_acls", "true"),
            ("block_public_policy", "true"),
            ("ignore_public_acls", "true"),
            ("restrict_public_buckets", "true"),
        ],
    );
    assert_rows(
        "ownership",
        only(
            resource(&tofu, "aws_s3_bucket_ownership_controls", "index"),
            "rule",
        ),
        &[("object_ownership", r#""BucketOwnerEnforced""#)],
    );
    assert_rows(
        "encryption",
        only(
            only(
                resource(
                    &tofu,
                    "aws_s3_bucket_server_side_encryption_configuration",
                    "index",
                ),
                "rule",
            ),
            "apply_server_side_encryption_by_default",
        ),
        &[("sse_algorithm", r#""AES256""#)],
    );
    let rule = only(
        resource(&tofu, "aws_s3_bucket_lifecycle_configuration", "index"),
        "rule",
    );
    assert_rows(
        "delete markers",
        only(rule, "expiration"),
        &[("expired_object_delete_marker", "true")],
    );
    assert_rows(
        "uploads",
        only(rule, "abort_incomplete_multipart_upload"),
        &[("days_after_initiation", "1")],
    );
    let denied = only(policy(&tofu, "index_is_tls_only"), "statement");
    assert_rows("plain transport", denied, &[("effect", r#""Deny""#)]);
    assert_rows(
        "plain transport",
        only(denied, "condition"),
        &[
            ("test", r#""Bool""#),
            ("variable", r#""aws:SecureTransport""#),
            ("values", r#"["false"]"#),
        ],
    );
}

#[test]
fn the_registry_is_immutable_and_only_lambda_pulls() {
    let tofu = tofu();
    let repository = resource(&tofu, "aws_ecr_repository", "indexer");
    assert_rows(
        "the registry",
        repository,
        &[
            ("name", r#""openlore-indexer""#),
            ("image_tag_mutability", r#""IMMUTABLE""#),
            ("force_delete", "false"),
        ],
    );
    assert_rows(
        "encryption",
        only(repository, "encryption_configuration"),
        &[("encryption_type", r#""AES256""#)],
    );
    assert_rows(
        "scanning",
        only(repository, "image_scanning_configuration"),
        &[("scan_on_push", "true")],
    );
    let squeezed: String = resource(&tofu, "aws_ecr_lifecycle_policy", "indexer")
        .split_whitespace()
        .collect();
    for needle in [
        r#"tagStatus="untagged"countType="sinceImagePushed"countUnit="days"countNumber=1}"#,
        r#"tagPrefixList=["sha-"]countType="imageCountMoreThan"countNumber=10}"#,
    ] {
        assert!(squeezed.contains(needle), "{needle}");
    }
    let pulling = only(policy(&tofu, "lambda_pulls_images"), "statement");
    assert_rows(
        "the pull",
        pulling,
        &[(
            "actions",
            r#"["ecr:BatchGetImage", "ecr:GetDownloadUrlForLayer"]"#,
        )],
    );
    assert_rows(
        "the pull",
        only(pulling, "principals"),
        &[("identifiers", r#"["lambda.amazonaws.com"]"#)],
    );
    assert_rows(
        "the pull",
        only(pulling, "condition"),
        &[
            ("test", r#""StringLike""#),
            ("variable", r#""aws:SourceArn""#),
            (
                "values",
                r#"["arn:aws:lambda:${local.region}:${local.account_id}:function:openlore-indexer-*"]"#,
            ),
        ],
    );
}

/// A statement of a role's policy, by what it may do, on what, under what.
type Statement<'a> = (&'a str, &'a str, Option<(&'a str, &'a str)>);

/// The policy holds exactly these statements. A row compared to a list of
/// statements, so a grant nobody listed fails too.
fn assert_statements(name: &str, body: &str, expected: &[Statement]) {
    let found: BTreeSet<Statement> = nested(body, "statement")
        .into_iter()
        .map(|statement| {
            let condition = nested(statement, "condition").first().map(|condition| {
                (
                    setting(condition, "variable").expect("a variable"),
                    setting(condition, "values").expect("values"),
                )
            });
            (
                setting(statement, "actions").expect("actions"),
                setting(statement, "resources").expect("resources"),
                condition,
            )
        })
        .collect();
    let expected: BTreeSet<Statement> = expected.iter().copied().collect();
    assert_eq!(found, expected, "{name}");
}

#[test]
fn each_role_can_do_what_its_table_says_and_nothing_else() {
    const GET: &str = r#"["s3:GetObject"]"#;
    const PUT: &str = r#"["s3:PutObject"]"#;
    const INDEX: &str = "[local.index_object_arn]";
    const LAST_PASS: &str = "[local.last_pass_object_arn]";
    const PROBE_MARKER: &str = "[local.write_probe_object_arn]";
    const PRESENT: &str = r#"["false"]"#;
    let tofu = tofu();

    assert_statements(
        "every function",
        policy(&tofu, "every_function"),
        &[
            (
                r#"["s3:ListBucket"]"#,
                "[aws_s3_bucket.index.arn]",
                Some((r#""s3:prefix""#, r#"["index/*"]"#)),
            ),
            (
                r#"["logs:CreateLogStream", "logs:PutLogEvents"]"#,
                "local.lambda_log_arns",
                None,
            ),
        ],
    );
    assert_statements("search", policy(&tofu, "search"), &[(GET, INDEX, None)]);
    assert_statements(
        "pass",
        policy(&tofu, "pass"),
        &[
            (GET, INDEX, None),
            (PUT, INDEX, Some((r#""s3:if-match""#, PRESENT))),
            (PUT, INDEX, Some((r#""s3:if-none-match""#, PRESENT))),
            (PUT, LAST_PASS, None),
            (PUT, PROBE_MARKER, Some((r#""s3:if-none-match""#, PRESENT))),
            (
                r#"["ssm:GetParameter"]"#,
                r#"["arn:aws:ssm:${local.region}:${local.account_id}:parameter${local.prod_outputs.indexer_repo_dids_parameter}"]"#,
                None,
            ),
        ],
    );
    assert_statements("probe", policy(&tofu, "probe"), &[(GET, LAST_PASS, None)]);
    assert_statements(
        "scheduler",
        policy(&tofu, "scheduler"),
        &[(
            r#"["lambda:InvokeFunction"]"#,
            "[aws_lambda_alias.pass_live.arn, aws_lambda_alias.probe_live.arn]",
            None,
        )],
    );
    for name in FUNCTIONS {
        assert_rows(
            name,
            policy(&tofu, name),
            &[(
                "source_policy_documents",
                "[data.aws_iam_policy_document.every_function.json]",
            )],
        );
        let role = resource(&tofu, "aws_iam_role", name);
        assert_rows(
            name,
            role,
            &[
                ("name", &format!(r#""openlore-indexer-{name}""#)),
                (
                    "assume_role_policy",
                    "data.aws_iam_policy_document.lambda_trust.json",
                ),
            ],
        );
    }
    assert!(
        !tofu.contains("aws_iam_role_policy_attachment") && !tofu.contains("managed_policy_arns"),
        "no managed policy: the basic execution role grants CreateLogGroup on *"
    );
    for (name, service) in [
        ("lambda_trust", "lambda.amazonaws.com"),
        ("scheduler_trust", "scheduler.amazonaws.com"),
    ] {
        let statement = only(policy(&tofu, name), "statement");
        assert_rows(
            name,
            only(statement, "principals"),
            &[("identifiers", &format!(r#"["{service}"]"#))],
        );
        assert_rows(
            name,
            only(statement, "condition"),
            &[
                ("variable", r#""aws:SourceAccount""#),
                ("values", "[local.account_id]"),
            ],
        );
    }
}

/// During the trial no function can write into the group A1-A3 read.
#[test]
fn only_the_cut_over_grants_the_prod_log_group() {
    let tofu = tofu();
    let grant = tofu
        .lines()
        .find(|line| line.contains("local.prod_outputs.indexer_log_group}:*"))
        .expect("the grant");

    assert!(grant.contains(r#"var.phase == "cutover" ? ["#), "{grant}");
    assert!(grant.trim_end().ends_with(": [],"), "{grant}");
    assert_rows(
        "the trial group",
        resource(&tofu, "aws_cloudwatch_log_group", "lambda"),
        &[
            ("name", r#""/openlore/prod/indexer-lambda""#),
            ("retention_in_days", "30"),
        ],
    );
}

#[test]
fn the_root_has_its_own_state_and_a_provider_that_knows_its_settings() {
    let tofu = tofu();
    assert_rows(
        "the backend",
        block(&tofu, r#"backend "s3""#),
        &[
            ("bucket", r#""jeffbaileyterraformstate""#),
            ("key", r#""openlore/indexer/prod.tfstate""#),
            ("region", r#""us-west-2""#),
            ("encrypt", "true"),
            ("use_lockfile", "true"),
        ],
    );
    assert!(tofu.contains(r#"key     = "openlore/pds/prod.tfstate""#));
    for reader_contract in ["for_each", "dynamic "] {
        assert!(!tofu.contains(reader_contract), "{reader_contract}");
    }

    let lock = read(&format!("{PROD_INDEXER_ROOT}/.terraform.lock.hcl"));
    let locked = setting(
        block(&lock, r#"provider "registry.opentofu.org/hashicorp/aws""#),
        "version",
    )
    .expect("a locked version");
    let minor: u32 = locked
        .trim_matches('"')
        .strip_prefix("6.")
        .and_then(|rest| rest.split('.').next())
        .and_then(|minor| minor.parse().ok())
        .unwrap_or_else(|| panic!("6.x: {locked}"));
    assert!(minor >= LOWEST_AWS_PROVIDER_MINOR, "{locked}");
}

/// One row of the DEVOPS alarm table.
struct AlarmRow {
    id: &'static str,
    resource: &'static str,
    name: &'static str,
    namespace: &'static str,
    metric: &'static str,
    statistic: &'static str,
    period: &'static str,
    /// Evaluation periods and datapoints to alarm: the table's "n of n".
    of: &'static str,
    operator: &'static str,
    threshold: &'static str,
    missing: &'static str,
    /// The keys and values of `dimensions`, sorted; empty for account level.
    dimensions: &'static [(&'static str, &'static str)],
}

const TRIAL: &str = "local.trial_namespace";
const LAMBDA: &str = r#""AWS/Lambda""#;
const OPENLORE: &str = "local.indexer_namespace";
const AT_LEAST: &str = r#""GreaterThanOrEqualToThreshold""#;
const ABOVE: &str = r#""GreaterThanThreshold""#;
const NOT_BREACHING: &str = r#""notBreaching""#;
const SEARCH_ONLY: &[(&str, &str)] =
    &[("FunctionName", "aws_lambda_function.search.function_name")];

const ALARMS: [AlarmRow; 11] = [
    AlarmRow {
        id: "A1-T",
        resource: "trial_total_outage",
        name: "openlore-indexer-lambda-total-outage",
        namespace: TRIAL,
        metric: "IndexerPassOutage",
        statistic: "Minimum",
        period: "900",
        of: "2",
        operator: AT_LEAST,
        threshold: "1",
        missing: NOT_BREACHING,
        dimensions: &[],
    },
    AlarmRow {
        id: "A2-T",
        resource: "trial_pass_failed",
        name: "openlore-indexer-lambda-pass-failed",
        namespace: TRIAL,
        metric: "IndexerFailure",
        statistic: "Sum",
        period: "300",
        of: "1",
        operator: AT_LEAST,
        threshold: "1",
        missing: NOT_BREACHING,
        dimensions: &[],
    },
    AlarmRow {
        id: "A3-T",
        resource: "trial_not_live",
        name: "openlore-indexer-lambda-not-live",
        namespace: TRIAL,
        metric: "IndexerNotLive",
        statistic: "Maximum",
        period: "300",
        of: "2",
        operator: AT_LEAST,
        threshold: "1",
        missing: r#""breaching""#,
        dimensions: &[],
    },
    AlarmRow {
        id: "A4",
        resource: "search_invocations",
        name: "openlore-indexer-search-invocations",
        namespace: LAMBDA,
        metric: "Invocations",
        statistic: "Sum",
        period: "300",
        of: "1",
        operator: ABOVE,
        threshold: "3000",
        missing: NOT_BREACHING,
        dimensions: SEARCH_ONLY,
    },
    AlarmRow {
        id: "A5",
        resource: "index_size",
        name: "openlore-indexer-index-size",
        namespace: OPENLORE,
        metric: "IndexBytes",
        statistic: "Maximum",
        period: "900",
        of: "1",
        operator: ABOVE,
        threshold: "104857600",
        missing: r#""ignore""#,
        dimensions: &[],
    },
    AlarmRow {
        id: "A6",
        resource: "lambda_errors",
        name: "openlore-indexer-lambda-errors",
        namespace: LAMBDA,
        metric: "Errors",
        statistic: "Sum",
        period: "300",
        of: "1",
        operator: AT_LEAST,
        threshold: "1",
        missing: NOT_BREACHING,
        dimensions: &[],
    },
    AlarmRow {
        id: "A7",
        resource: "lambda_throttles",
        name: "openlore-indexer-lambda-throttles",
        namespace: LAMBDA,
        metric: "Throttles",
        statistic: "Sum",
        period: "300",
        of: "1",
        operator: AT_LEAST,
        threshold: "10",
        missing: NOT_BREACHING,
        dimensions: &[],
    },
    AlarmRow {
        id: "A8",
        resource: "search_hourly",
        name: "openlore-indexer-search-hourly",
        namespace: LAMBDA,
        metric: "Invocations",
        statistic: "Sum",
        period: "3600",
        of: "1",
        operator: ABOVE,
        threshold: "1900",
        missing: NOT_BREACHING,
        dimensions: SEARCH_ONLY,
    },
    AlarmRow {
        id: "A9",
        resource: "edge_requests",
        name: "openlore-indexer-edge-requests",
        namespace: r#""AWS/CloudFront""#,
        metric: "Requests",
        statistic: "Sum",
        period: "3600",
        of: "1",
        operator: ABOVE,
        threshold: "13000",
        missing: NOT_BREACHING,
        dimensions: &[
            ("DistributionId", "aws_cloudfront_distribution.index.id"),
            ("Region", r#""Global""#),
        ],
    },
    AlarmRow {
        id: "A10",
        resource: "freshness_failed",
        name: "openlore-indexer-freshness-failed",
        namespace: OPENLORE,
        metric: "IndexerFreshnessFailed",
        statistic: "Sum",
        period: "900",
        of: "1",
        operator: AT_LEAST,
        threshold: "3",
        missing: NOT_BREACHING,
        dimensions: &[],
    },
    AlarmRow {
        id: "A11",
        resource: "search_5xx",
        name: "openlore-indexer-search-5xx",
        namespace: LAMBDA,
        metric: "Url5xxCount",
        statistic: "Sum",
        period: "300",
        of: "2",
        operator: AT_LEAST,
        threshold: "1",
        missing: NOT_BREACHING,
        dimensions: &[
            ("FunctionName", "aws_lambda_function.search.function_name"),
            (
                "Resource",
                r#""${aws_lambda_function.search.function_name}:${aws_lambda_alias.search_live.name}""#,
            ),
        ],
    },
];

/// How many `resource "<kind>"` the root declares.
fn declared(tofu: &str, kind: &str) -> usize {
    tofu.matches(&format!(r#"resource "{kind}""#)).count()
}

/// The `key = value` rows of the `dimensions` map, sorted; none without the map.
fn dimensions(alarm: &str) -> BTreeMap<&str, &str> {
    let Some(at) = alarm.find("dimensions") else {
        return BTreeMap::new();
    };
    let open = at + alarm[at..].find('{').expect("a map");
    braced(alarm, open)
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(key, value)| (key.trim(), value.trim()))
        .collect()
}

#[test]
fn each_alarm_is_the_row_devops_set() {
    let tofu = tofu();
    assert_eq!(declared(&tofu, "aws_cloudwatch_metric_alarm"), ALARMS.len());

    for row in &ALARMS {
        let alarm = resource(&tofu, "aws_cloudwatch_metric_alarm", row.resource);
        assert_rows(
            row.id,
            alarm,
            &[
                ("alarm_name", &format!("\"{}\"", row.name)),
                ("namespace", row.namespace),
                ("metric_name", &format!("\"{}\"", row.metric)),
                ("statistic", &format!("\"{}\"", row.statistic)),
                ("period", row.period),
                ("evaluation_periods", row.of),
                ("datapoints_to_alarm", row.of),
                ("comparison_operator", row.operator),
                ("threshold", row.threshold),
                ("treat_missing_data", row.missing),
                ("actions_enabled", "var.lambda_alarms_enabled"),
                ("alarm_actions", "local.alarm_topic"),
                ("ok_actions", "local.alarm_topic"),
            ],
        );
        let expected: BTreeMap<&str, &str> = row.dimensions.iter().copied().collect();
        assert_eq!(dimensions(alarm), expected, "{}", row.id);
        let in_the_trial_only = row.id.ends_with("-T");
        assert_eq!(
            setting(alarm, "count"),
            in_the_trial_only.then_some("local.while_trial"),
            "{}",
            row.id
        );
    }
    assert_rows(
        "the shared values",
        &tofu,
        &[
            ("indexer_namespace", r#""OpenLore/Indexer""#),
            ("trial_namespace", r#""OpenLore/IndexerLambdaTrial""#),
            ("alarm_topic", "[local.prod_outputs.backup_alarm_topic_arn]"),
            ("while_trial", r#"var.phase == "trial" ? 1 : 0"#),
        ],
    );
    let switch = block(&tofu, r#"variable "lambda_alarms_enabled""#);
    assert_rows("the switch", switch, &[("default", "false")]);
}

/// Copies, down to the pattern, on the trial group: A2 is not filtered on
/// `no_publish`, because a gate with no usable start means the store really is
/// unusable. The trial's namespace keeps the host's metrics out of Lambda's.
#[test]
fn the_trial_filters_copy_the_prod_filters_onto_their_own_group_and_namespace() {
    let tofu = tofu();
    let prod = read("deploy/tofu/environments/prod/indexer.tf");
    let rows = [
        ("trial_pass_outage", "indexer_pass_outage", "1"),
        ("trial_pass_not_outage", "indexer_pass_not_outage", "0"),
        ("trial_failure", "indexer_failure", "1"),
        ("trial_not_live", "indexer_not_live", "$.not_live"),
    ];

    for (trial, copied, value) in rows {
        let filter = resource(&tofu, "aws_cloudwatch_log_metric_filter", trial);
        let original = resource(&prod, "aws_cloudwatch_log_metric_filter", copied);
        assert_eq!(
            setting(filter, "pattern"),
            setting(original, "pattern"),
            "{trial}"
        );
        assert_rows(
            trial,
            filter,
            &[
                ("count", "local.while_trial"),
                ("log_group_name", "aws_cloudwatch_log_group.lambda.name"),
            ],
        );
        let metric = setting(only(original, "metric_transformation"), "name").expect("a metric");
        assert_rows(
            trial,
            only(filter, "metric_transformation"),
            &[
                ("name", metric),
                ("namespace", "local.trial_namespace"),
                ("value", &format!("\"{value}\"")),
            ],
        );
    }
}

/// The two filters that stay read whichever group the functions write to, and no
/// filter has a default: one would publish for every other line and A1's Minimum
/// could never reach 1.
#[test]
fn the_size_and_freshness_filters_follow_the_log_group() {
    let tofu = tofu();
    let rows = [
        (
            "index_bytes",
            r#""{ $.event = \"indexer.lambda.published\" }""#,
            "IndexBytes",
            "$.index_bytes",
        ),
        (
            "freshness_failed",
            r#""{ $.event = \"indexer.lambda.freshness_check_failed\" || $.event = \"indexer.lambda.served_index_stale\" }""#,
            "IndexerFreshnessFailed",
            "1",
        ),
    ];

    for (name, pattern, metric, value) in rows {
        let filter = resource(&tofu, "aws_cloudwatch_log_metric_filter", name);
        assert_rows(
            name,
            filter,
            &[
                ("log_group_name", "local.lambda_log_group"),
                ("pattern", pattern),
            ],
        );
        assert_rows(
            name,
            only(filter, "metric_transformation"),
            &[
                ("name", &format!("\"{metric}\"")),
                ("namespace", "local.indexer_namespace"),
                ("value", &format!("\"{value}\"")),
            ],
        );
    }
    assert_eq!(declared(&tofu, "aws_cloudwatch_log_metric_filter"), 6);
    assert!(!tofu.contains("default_value"));
}

#[test]
fn the_saved_queries_read_the_log_group_the_functions_write_to() {
    let tofu = tofu();
    let rows = [
        ("cold_starts", "cold-starts", "initDurationMs"),
        ("memory", "memory", "maxMemoryUsedMB"),
        ("publishes", "publishes", "indexer.lambda.published"),
        (
            "freshness_failures",
            "freshness-failures",
            "indexer.lambda.served_index_stale",
        ),
        ("probe", "probe", r#""lambda-probe""#),
    ];

    assert_eq!(
        declared(&tofu, "aws_cloudwatch_query_definition"),
        rows.len()
    );
    for (name, query, needle) in rows {
        let definition = resource(&tofu, "aws_cloudwatch_query_definition", name);
        assert_rows(
            name,
            definition,
            &[
                ("name", &format!(r#""indexer-lambda/{query}""#)),
                ("log_group_names", "[local.lambda_log_group]"),
            ],
        );
        assert!(definition.contains(needle), "{name}: {needle}");
    }
}

#[test]
fn the_edge_is_the_distribution_devops_set() {
    let tofu = tofu();
    let distribution = resource(&tofu, "aws_cloudfront_distribution", "index");
    assert_rows(
        "the distribution",
        distribution,
        &[
            ("aliases", "[local.index_domain]"),
            ("price_class", r#""PriceClass_100""#),
            ("http_version", r#""http2and3""#),
            ("is_ipv6_enabled", "true"),
        ],
    );
    assert!(!distribution.contains("logging_config"), "no client IPs");
    assert!(!distribution.contains("web_acl_id"), "no WAF");
    assert_rows(
        "the certificate",
        only(distribution, "viewer_certificate"),
        &[
            (
                "acm_certificate_arn",
                "aws_acm_certificate_validation.index.certificate_arn",
            ),
            ("ssl_support_method", r#""sni-only""#),
            ("minimum_protocol_version", r#""TLSv1.2_2021""#),
        ],
    );
    assert_rows(
        "the origin",
        only(only(distribution, "origin"), "custom_origin_config"),
        &[
            ("origin_protocol_policy", r#""https-only""#),
            ("origin_ssl_protocols", r#"["TLSv1.2"]"#),
            ("origin_read_timeout", "10"),
        ],
    );

    let specific = nested(distribution, "ordered_cache_behavior");
    assert_eq!(specific.len(), 2, "search and the health check");
    assert_eq!(
        setting(specific[0], "path_pattern"),
        Some(r#""/xrpc/org.openlore.appview.searchClaims""#)
    );
    assert_eq!(setting(specific[1], "path_pattern"), Some(r#""/healthz""#));
    let default = only(distribution, "default_cache_behavior");
    for behavior in specific.into_iter().chain([default]) {
        assert_rows(
            "a behavior",
            behavior,
            &[
                ("viewer_protocol_policy", r#""redirect-to-https""#),
                (
                    "cache_policy_id",
                    "data.aws_cloudfront_cache_policy.caching_disabled.id",
                ),
                (
                    "origin_request_policy_id",
                    "data.aws_cloudfront_origin_request_policy.all_viewer_except_host.id",
                ),
                (
                    "response_headers_policy_id",
                    "aws_cloudfront_response_headers_policy.strip.id",
                ),
            ],
        );
        assert_rows(
            "a behavior",
            only(behavior, "function_association"),
            &[("function_arn", "aws_cloudfront_function.edge.arn")],
        );
    }
    assert_rows(
        "caching",
        block(
            &tofu,
            r#"data "aws_cloudfront_cache_policy" "caching_disabled""#,
        ),
        &[("name", r#""Managed-CachingDisabled""#)],
    );
    assert_rows(
        "the forwarded headers",
        block(
            &tofu,
            r#"data "aws_cloudfront_origin_request_policy" "all_viewer_except_host""#,
        ),
        &[("name", r#""Managed-AllViewerExceptHostHeader""#)],
    );
    assert_rows(
        "the domain",
        &tofu,
        &[("index_domain", r#""index.openlore.jeffbailey.us""#)],
    );
    assert_rows(
        "the certificate",
        resource(&tofu, "aws_acm_certificate", "index"),
        &[
            ("domain_name", "local.index_domain"),
            ("validation_method", r#""DNS""#),
        ],
    );
}

/// The table's method and path pairs, and nothing else: OP-4 runs the function on a few
/// requests, this reads that nothing was added beside them.
#[test]
fn the_edge_function_passes_exactly_three_requests() {
    let tofu = tofu();
    assert_rows(
        "the function",
        resource(&tofu, "aws_cloudfront_function", "edge"),
        &[
            ("name", r#""openlore-index-edge""#),
            ("runtime", r#""cloudfront-js-2.0""#),
            ("publish", "true"),
            ("code", r#"file("${path.module}/edge-function.js")"#),
        ],
    );
    let source = read(&format!("{PROD_INDEXER_ROOT}/edge-function.js"));
    let allowed: BTreeSet<&str> = source
        .lines()
        .filter_map(|line| line.trim().strip_suffix("': true,")?.strip_prefix('\''))
        .collect();

    assert_eq!(
        allowed,
        BTreeSet::from([
            "GET /healthz",
            "HEAD /healthz",
            "POST /xrpc/org.openlore.appview.searchClaims",
        ])
    );
    assert!(source.contains("statusCode: 404"), "not a 403");
    assert!(
        source.contains("=== true ? request : NOT_FOUND"),
        "{source}"
    );
}

#[test]
fn the_edge_strips_every_header_that_names_the_origin() {
    let tofu = tofu();
    let policy = resource(&tofu, "aws_cloudfront_response_headers_policy", "strip");
    assert_rows(
        "the policy",
        policy,
        &[("name", r#""openlore-index-strip""#)],
    );
    let removed: BTreeSet<&str> = nested(only(policy, "remove_headers_config"), "items")
        .into_iter()
        .map(|item| setting(item, "header").expect("a header"))
        .collect();

    assert_eq!(
        removed,
        BTreeSet::from([
            r#""x-amzn-remapped-connection""#,
            r#""x-amzn-remapped-content-length""#,
            r#""x-amzn-remapped-date""#,
            r#""x-amzn-remapped-x-amzn-requestid""#,
            r#""x-amzn-requestid""#,
            r#""x-amzn-trace-id""#,
        ])
    );
}

/// One budget on the tag: an unfiltered one would fire on the PDS host's spend.
/// 20 % of the $10 limit is the $2 target.
#[test]
fn one_budget_on_the_tag_warns_at_two_dollars_and_at_ten() {
    let tofu = tofu();
    assert_eq!(declared(&tofu, "aws_budgets_budget"), 1);
    let budget = resource(&tofu, "aws_budgets_budget", "indexer");
    assert_rows(
        "the budget",
        budget,
        &[
            ("name", r#""openlore-indexer-lambda""#),
            ("budget_type", r#""COST""#),
            ("time_unit", r#""MONTHLY""#),
            ("limit_amount", r#""10""#),
            ("limit_unit", r#""USD""#),
        ],
    );
    assert_rows(
        "the budget",
        only(budget, "cost_filter"),
        &[
            ("name", r#""TagKeyValue""#),
            ("values", r#"["user:Component$openlore-indexer-lambda"]"#),
        ],
    );
    let notifications = nested(budget, "notification");
    assert_eq!(notifications.len(), 2);
    for (notification, percent) in notifications.into_iter().zip(["20", "100"]) {
        assert_rows(
            percent,
            notification,
            &[
                ("notification_type", r#""ACTUAL""#),
                ("comparison_operator", r#""GREATER_THAN""#),
                ("threshold", percent),
                ("threshold_type", r#""PERCENTAGE""#),
                (
                    "subscriber_email_addresses",
                    "[data.aws_ssm_parameter.contact.value]",
                ),
            ],
        );
    }
    assert_rows(
        "the contact",
        block(&tofu, r#"data "aws_ssm_parameter" "contact""#),
        &[("name", r#""/openlore/prod/acme-contact-email""#)],
    );
    assert_rows(
        "the tag",
        resource(&tofu, "aws_ce_cost_allocation_tag", "component"),
        &[
            ("count", "var.cost_allocation_tag_active ? 1 : 0"),
            ("tag_key", r#""Component""#),
            ("status", r#""Active""#),
        ],
    );
    assert_rows(
        "the tag waits for billing data",
        block(&tofu, r#"variable "cost_allocation_tag_active""#),
        &[("default", "false")],
    );
}

/// Both triggers name the root, the validate loop reaches it, and nothing in the
/// workflow can hold a credential or change AWS.
#[test]
fn the_static_checks_cover_the_new_root_without_credentials() {
    let workflow = read(".github/workflows/deploy-pds-check.yml");
    let trigger = format!(r#"      - "{PROD_INDEXER_ROOT}/**""#);
    assert_eq!(
        workflow.lines().filter(|line| *line == trigger).count(),
        2,
        "pull_request and push"
    );

    let validated = workflow
        .lines()
        .find_map(|line| line.trim().strip_prefix("for dir in "))
        .expect("the validate loop");
    assert!(
        validated
            .trim_end_matches("; do")
            .split(' ')
            .any(|dir| dir == PROD_INDEXER_ROOT),
        "{validated}"
    );
    assert!(workflow.contains("tofu fmt -check -recursive deploy/tofu\n"));
    assert!(workflow.contains(r#"tofu -chdir="$dir" init -backend=false"#));
    for refused in [
        "${{ secrets.",
        "configure-aws-credentials",
        "tofu plan",
        "tofu apply",
    ] {
        assert!(!workflow.contains(refused), "{refused}");
    }
}

/// A job of the workflow: its lines up to the next line indented like a job.
fn job<'a>(workflow: &'a str, name: &str) -> &'a str {
    let opening = format!("\n  {name}:\n");
    let start = workflow
        .find(&opening)
        .unwrap_or_else(|| panic!("no job {name}"))
        + opening.len();
    let body = &workflow[start..];
    let end = body
        .match_indices('\n')
        .map(|(at, _)| at + 1)
        .find(|&at| {
            let line = &body[at..];
            line.starts_with("  ") && !line.starts_with("   ")
        })
        .unwrap_or(body.len());
    &body[..end]
}

/// The steps of a job, in order, each with its own lines.
fn steps(job: &str) -> Vec<&str> {
    let starts: Vec<usize> = job
        .match_indices("\n      - ")
        .map(|(at, _)| at + 1)
        .collect();
    let ends = starts.iter().skip(1).copied().chain([job.len()]);
    starts.iter().zip(ends).map(|(&s, e)| &job[s..e]).collect()
}

/// Where the first step holding `needle` stands in its job.
fn step_at(steps: &[&str], needle: &str) -> usize {
    steps
        .iter()
        .position(|step| step.contains(needle))
        .unwrap_or_else(|| panic!("no step holds {needle}"))
}

/// The script of a `run: |` step, unindented.
fn script(step: &str) -> String {
    let (_, block) = step.split_once("run: |\n").expect("a run block");
    block
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The lines of a step's script that run this command, read whole.
fn lines_running<'a>(job: &'a str, command: &str) -> Vec<&'a str> {
    job.lines()
        .map(str::trim)
        .filter(|line| line.contains(command))
        .filter(|line| !line.starts_with("- name:") && !line.starts_with('#'))
        .collect()
}

const ARTIFACT_CHECK: &str = "Artifact check (the self-host binary is AWS-free)";

/// D-STZ-24, R-STZ-9. The first main-push CI run is the real proof; this holds the
/// files to it. `--version` is not the Lambda binary's word, so its smoke asserts the
/// refusal only a binary that loaded and ran can print.
#[test]
fn the_image_carries_both_binaries_and_the_self_host_stays_clean() {
    let dockerfile: BTreeSet<String> = read("crates/openlore-indexer/Dockerfile")
        .lines()
        .map(|line| line.trim().to_string())
        .collect();
    for line in [
        "COPY --chmod=0555 openlore-indexer openlore-indexer-lambda /usr/local/bin/",
        "WORKDIR /",
        r#"org.openlore.indexer.lambda-root="1""#,
        "USER 65532:65532",
        r#"ENTRYPOINT ["/usr/local/bin/openlore-indexer"]"#,
        r#"CMD ["serve"]"#,
    ] {
        assert!(dockerfile.contains(line), "the Dockerfile has `{line}`");
    }

    let ci = read(".github/workflows/ci.yml");
    let build = job(&ci, "indexer-build");
    assert_eq!(
        lines_running(build, "cargo build"),
        [
            "run: cargo build --release -p openlore-indexer",
            "run: cargo build --release -p openlore-indexer-lambda",
        ],
        "two invocations: one would unify the SDK's features into the self-host binary"
    );
    let stage = steps(build);
    let staged = stage[step_at(&stage, "cargo cyclonedx")];
    assert!(staged.contains("for crate in openlore-indexer openlore-indexer-lambda; do"));
    for line in [
        r#"strip "target/release/$crate""#,
        r#"cp "target/release/$crate" out/"#,
        r#"--manifest-path "crates/$crate/Cargo.toml""#,
        r#"cp "crates/$crate/sbom.json" "out/$crate.cdx.json""#,
    ] {
        assert!(staged.contains(line), "{line}");
    }
    let staged_at = step_at(&stage, "cargo cyclonedx");
    let checked_at = step_at(&stage, ARTIFACT_CHECK);
    let uploaded_at = step_at(&stage, "actions/upload-artifact");
    assert!(
        staged_at < checked_at && checked_at < uploaded_at,
        "no artifact leaves unchecked"
    );

    let image = steps(job(&ci, "indexer-image"));
    let built_at = step_at(&image, "docker buildx build");
    let lambda_at = step_at(
        &image,
        "--entrypoint /usr/local/bin/openlore-indexer-lambda",
    );
    let bundle_at = step_at(&image, "/etc/ssl/certs/ca-certificates.crt");
    let serve_at = step_at(&image, "- name: serve-smoke (production posture)");
    let scanned_at = step_at(&image, "aquasecurity/trivy-action");
    let pushed_at = step_at(&image, "docker push");
    assert!(built_at < lambda_at && built_at < bundle_at && built_at < serve_at);
    assert!(lambda_at < scanned_at && bundle_at < scanned_at && serve_at < scanned_at);
    assert!(scanned_at < pushed_at, "nothing is pushed unsmoked");
    assert!(image[step_at(&image, "Smoke (dynamic linking)")].contains(r#""$LOCAL_TAG" --version"#));
    assert!(image[serve_at].contains(r#""$LOCAL_TAG" serve"#));
    let lambda_smoke = image[lambda_at];
    assert!(lambda_smoke.contains(r#""$LOCAL_TAG" --version"#));
    assert!(lambda_smoke.contains(r#"[ "$rc" = 2 ]"#));
    assert!(lambda_smoke.contains("health.startup.refused"));
    let bundle_smoke = image[bundle_at];
    assert!(
        bundle_smoke.contains(r#"docker cp -L "$c:$bundle""#),
        "out of the image itself"
    );
    assert!(
        bundle_smoke.contains("BEGIN CERTIFICATE"),
        "a bundle, not a file"
    );

    let attested = job(&ci, "indexer-image");
    assert_eq!(
        lines_running(attested, "cosign attest"),
        [
            r#"cosign attest --yes --type cyclonedx --predicate "$RUNNER_TEMP/cargo.cdx.json" "$IMAGE@$DIGEST""#,
            r#"cosign attest --yes --type cyclonedx --predicate "$RUNNER_TEMP/cargo-lambda.cdx.json" "$IMAGE@$DIGEST""#,
            r#"cosign attest --yes --type cyclonedx --predicate "$RUNNER_TEMP/image.cdx.json" "$IMAGE@$DIGEST""#,
        ],
        "three SBOMs on the one digest"
    );
    assert_eq!(lines_running(attested, "cosign sign").len(), 1);
    assert!(attested.contains(
        r#"mv "$RUNNER_TEMP/ctx/openlore-indexer-lambda.cdx.json" "$RUNNER_TEMP/cargo-lambda.cdx.json""#
    ));
    assert!(attested.contains("deploy/indexer/deploy.sh lambda deploy ${GITHUB_SHA}"));
}

/// The check itself, run on planted binaries: a string fails the build, an unreadable
/// binary is no pass, and a check that cannot see the strings in the Lambda binary
/// proves nothing about the self-host one.
#[test]
fn a_self_host_binary_holding_an_aws_string_fails_the_build() {
    const LAMBDA: &[u8] = b"\x7fELF\0registry/aws-smithy-runtime-1.16.2/src/lib.rs\0";
    const CLEAN: &[u8] = b"\x7fELF\0registry/rustls-0.23.46/src/lib.rs\0aws\0lambda\0";
    let ci = read(".github/workflows/ci.yml");
    let build = steps(job(&ci, "indexer-build"));
    let check = script(build[step_at(&build, ARTIFACT_CHECK)]);
    let exits_zero = |self_host: Option<&[u8]>, lambda: &[u8]| {
        let dir = tempfile::tempdir().expect("a directory");
        let out = dir.path().join("out");
        std::fs::create_dir(&out).expect("out");
        if let Some(bytes) = self_host {
            std::fs::write(out.join("openlore-indexer"), bytes).expect("the binary");
        }
        std::fs::write(out.join("openlore-indexer-lambda"), lambda).expect("the binary");
        Command::new("bash")
            .args(["-c", &check])
            .current_dir(dir.path())
            .status()
            .expect("bash")
            .success()
    };

    assert!(exits_zero(Some(CLEAN), LAMBDA), "a clean binary passes");
    for planted in [
        "aws-sdk-s3",
        "aws-smithy-types",
        "aws-lc-sys",
        "lambda_runtime",
    ] {
        let dirty = [CLEAN, planted.as_bytes(), b"\0"].concat();
        assert!(!exits_zero(Some(&dirty), LAMBDA), "{planted}");
    }
    assert!(!exits_zero(None, LAMBDA), "no binary is no pass");
    assert!(!exits_zero(Some(CLEAN), CLEAN), "a blind check is no pass");
}

const INHERITS: &str = "rust-version.workspace = true";
const LAMBDA_FLOOR: &str = r#"rust-version = "1.94.1""#;
const WORKSPACE_FLOOR: &str = r#"rust-version = "1.91""#;
const RUST_VERSION_RULE: &str = "rust_version_1_94_only_on_lambda_crates";

/// A workspace on disk, its members named so one sorts first, one in the middle and
/// one last, each on the given `rust-version` line. Rows override the healthy tree.
fn planted_workspace(workspace_line: &str, overrides: &[(&str, &str)]) -> tempfile::TempDir {
    let healthy = [
        ("aaa-first", INHERITS),
        ("adapter-s3-index-object", LAMBDA_FLOOR),
        ("mmm-middle", INHERITS),
        ("openlore-indexer-lambda", LAMBDA_FLOOR),
        ("zzz-last", INHERITS),
    ];
    let dir = tempfile::tempdir().expect("a directory");
    let root = format!(
        "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"2\"\n\n\
         [workspace.package]\n{workspace_line}\n"
    );
    std::fs::write(dir.path().join("Cargo.toml"), root).expect("the root manifest");
    for (name, healthy_line) in healthy {
        let line = overrides
            .iter()
            .find(|(member, _)| *member == name)
            .map_or(healthy_line, |(_, line)| line);
        let source = dir.path().join("crates").join(name).join("src");
        std::fs::create_dir_all(&source).expect("the crate");
        std::fs::write(source.join("lib.rs"), "").expect("the library");
        let manifest = format!(
            "[package]\nname = \"{name}\"\nversion = \"0.0.1\"\nedition = \"2021\"\n{line}\n"
        );
        std::fs::write(
            dir.path().join("crates").join(name).join("Cargo.toml"),
            manifest,
        )
        .expect("the member manifest");
    }
    dir
}

fn rust_version_findings(workspace: &tempfile::TempDir) -> Vec<String> {
    xtask::check_arch::scan_member_rust_versions(workspace.path()).expect("a readable workspace")
}

/// One finding, on the named crate, under the rule.
fn assert_only_finding_names(findings: &[String], member: &str) {
    assert_eq!(findings.len(), 1, "{member}: {findings:?}");
    assert!(findings[0].contains(member), "{findings:?}");
    assert!(findings[0].contains(RUST_VERSION_RULE), "{findings:?}");
}

/// D-STZ-35, DV-STZ-3. Every member's own manifest is read: wherever the third crate
/// sorts, and however its manifest departs from inheriting, it is the one named.
#[test]
fn only_the_two_lambda_crates_set_the_higher_rust_version() {
    assert_eq!(
        xtask::check_arch::scan_member_rust_versions(&workspace()).expect("the real tree"),
        Vec::<String>::new()
    );
    let healthy = planted_workspace(WORKSPACE_FLOOR, &[]);
    assert_eq!(rust_version_findings(&healthy), Vec::<String>::new());

    for third in ["aaa-first", "mmm-middle", "zzz-last"] {
        let planted = planted_workspace(WORKSPACE_FLOOR, &[(third, LAMBDA_FLOOR)]);
        assert_only_finding_names(&rust_version_findings(&planted), third);
    }
    let two = planted_workspace(
        WORKSPACE_FLOOR,
        &[("aaa-first", LAMBDA_FLOOR), ("zzz-last", LAMBDA_FLOOR)],
    );
    assert_eq!(rust_version_findings(&two).len(), 2, "each manifest counts");

    for departure in [
        WORKSPACE_FLOOR,
        r#"rust-version = "1.92""#,
        r#"rust-version="1.94.1" # the SDK"#,
        "",
    ] {
        let planted = planted_workspace(WORKSPACE_FLOOR, &[("mmm-middle", departure)]);
        assert_only_finding_names(&rust_version_findings(&planted), "mmm-middle");
    }
    for lambda_crate in ["adapter-s3-index-object", "openlore-indexer-lambda"] {
        for departure in [INHERITS, r#"rust-version = "1.95""#] {
            let planted = planted_workspace(WORKSPACE_FLOOR, &[(lambda_crate, departure)]);
            assert_only_finding_names(&rust_version_findings(&planted), lambda_crate);
        }
    }

    let inline_table = planted_workspace(
        WORKSPACE_FLOOR,
        &[("mmm-middle", "rust-version = { workspace = true }")],
    );
    assert_eq!(rust_version_findings(&inline_table), Vec::<String>::new());
    let comment_and_other_table = planted_workspace(
        WORKSPACE_FLOOR,
        &[(
            "mmm-middle",
            "# rust-version = \"1.94.1\"\nrust-version.workspace = true\n\n\
             [package.metadata.floor]\nrust-version = \"1.94.1\"",
        )],
    );
    assert_eq!(
        rust_version_findings(&comment_and_other_table),
        Vec::<String>::new(),
        "only the package's own key counts"
    );

    let raised = planted_workspace(LAMBDA_FLOOR, &[]);
    let findings = rust_version_findings(&raised);
    assert_eq!(
        findings.len(),
        1,
        "everyone inheriting 1.94.1: {findings:?}"
    );
    assert!(findings[0].contains("workspace.package"), "{findings:?}");
    assert!(findings[0].contains(RUST_VERSION_RULE), "{findings:?}");
}

/// The rule is on the command CI runs, not only in the library. check-arch needs a
/// whole workspace's sources, so this reads `run` rather than planting a tree.
#[test]
fn check_arch_reports_the_rust_version_rule() {
    let source = read("xtask/src/check_arch.rs");
    let (_, after_opening) = source
        .split_once("pub fn run() -> anyhow::Result<i32> {")
        .expect("the command");
    let (run, _) = after_opening.split_once("\n}\n").expect("its end");
    for line in [
        "let rust_version_findings = scan_member_rust_versions(&workspace_root)?;",
        "rendered.extend(rust_version_findings);",
    ] {
        assert!(run.contains(line), "{line}");
    }
}

/// The names in a job's `needs:` line, read whole.
fn needs(job: &str) -> BTreeSet<&str> {
    job.lines()
        .find_map(|line| line.trim().strip_prefix("needs: ["))
        .and_then(|names| names.strip_suffix(']'))
        .map_or_else(BTreeSet::new, |names| names.split(", ").collect())
}

/// DV-STZ-3. The repo's rust-toolchain.toml says `stable` and beats rustup's default,
/// so a leg that only installs its toolchain would prove nothing about that toolchain.
#[test]
fn the_msrv_job_checks_each_floor_on_its_own_toolchain_before_the_tests_run() {
    let ci = read(".github/workflows/ci.yml");
    let msrv = job(&ci, "msrv");
    let rows: BTreeSet<&str> = msrv.lines().map(str::trim).collect();
    for row in [
        "fail-fast: false",
        "- toolchain: 1.91.0",
        "scope: --workspace --exclude openlore-indexer-lambda --exclude adapter-s3-index-object",
        "- toolchain: 1.94.1",
        "scope: -p openlore-indexer-lambda -p adapter-s3-index-object",
        "CARGO_RESOLVER_INCOMPATIBLE_RUST_VERSIONS: fallback",
        "RUSTUP_TOOLCHAIN: ${{ matrix.toolchain }}",
        "toolchain: ${{ matrix.toolchain }}",
    ] {
        assert!(rows.contains(row), "the msrv job has `{row}`");
    }
    assert_eq!(
        lines_running(msrv, "cargo check"),
        ["cargo check ${{ matrix.scope }} --lib --bins"]
    );
    assert_eq!(
        needs(msrv),
        BTreeSet::new(),
        "commit stage: it needs nothing"
    );
    for gated in ["test", "rio-object-store"] {
        assert!(
            needs(job(&ci, gated)).contains("msrv"),
            "{gated} needs msrv"
        );
    }
}

/// The jobs that build what is signed, and the jobs that hold `id-token: write`.
const SIGNING_PATH_JOBS: [&str; 4] = [
    "review-app-build",
    "review-app-image",
    "indexer-build",
    "indexer-image",
];

/// The `uses:` lines of a job, read whole.
fn action_lines(job: &str) -> Vec<&str> {
    job.lines()
        .map(|line| line.trim().trim_start_matches("- "))
        .filter(|line| line.starts_with("uses:"))
        .collect()
}

/// Pinned as the Trivy step is: the full commit, then the exact version it was at.
fn is_pinned(line: &str) -> bool {
    let Some((action, version)) = line.split_once(" # ") else {
        return false;
    };
    let commit = action
        .rsplit_once('@')
        .map_or("", |(_, reference)| reference);
    let exact = version
        .strip_prefix('v')
        .map(|numbers| numbers.split('.').collect::<Vec<_>>());
    commit.len() == 40
        && commit
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        && exact.is_some_and(|parts| {
            parts.len() == 3 && parts.iter().all(|part| part.parse::<u32>().is_ok())
        })
}

/// A moved tag would run new code with `id-token: write` and `packages: write`, on the path that
/// produces what `deploy.sh` trusts, so every action there is a commit.
#[test]
fn every_action_on_the_signing_path_is_pinned_to_a_commit() {
    let commit = "ed142fd0673e97e23eac54620cfb913e5ce36c25";
    assert!(is_pinned(&format!("uses: a/b@{commit} # v0.36.0")));
    for loose in [
        "uses: a/b@v3".to_string(),
        format!("uses: a/b@{commit}"),
        format!("uses: a/b@{commit} # v3"),
        format!("uses: a/b@{commit} # master"),
        "uses: a/b@ed142fd # v0.36.0".to_string(),
        format!("uses: a/b@{} # v0.36.0", commit.to_uppercase()),
    ] {
        assert!(!is_pinned(&loose), "{loose}");
    }

    let ci = read(".github/workflows/ci.yml");
    for name in SIGNING_PATH_JOBS {
        let actions = action_lines(job(&ci, name));
        assert!(!actions.is_empty(), "{name}: the job was not read");
        let loose: Vec<_> = actions
            .into_iter()
            .filter(|line| !is_pinned(line))
            .collect();
        assert!(loose.is_empty(), "{name}: {loose:?}");
    }
    assert!(
        job(&ci, "msrv").contains("dtolnay/rust-toolchain@"),
        "the msrv job installs its toolchain"
    );
    let on_a_branch = ci.lines().find(|line| line.contains("@master"));
    assert_eq!(on_a_branch, None, "master moves");
}

const LAMBDA_SMOKE: &str = "deploy/indexer/smoke/lambda-smoke.sh";
const RACE_SHIM: &str = "deploy/indexer/smoke/race_shim.py";
const LAMBDA_SMOKE_STEP: &str = "- name: lambda-smoke (the image as Lambda runs it)";
const RIE_STEP: &str = "- name: Fetch the Runtime Interface Emulator";
const SECRET_STEP: &str = "- name: Secret check (no AWS credential in the image)";
const SEED_STEP: &str = "- name: Seed index for lambda-smoke (one claim)";
/// The stubbed world `lambda-smoke.sh` runs in.
const SMOKE_WORLD: &str = include_str!("support/smoke_world.sh");

/// Where `needle` first stands in `text`.
fn position(text: &str, needle: &str) -> usize {
    text.find(needle).unwrap_or_else(|| panic!("no `{needle}`"))
}

/// The event of each `run_once <mode> '<event>'` in the smoke, in order.
fn events_sent(smoke: &str, mode: &str) -> Vec<String> {
    let opening = format!("run_once {mode} '");
    smoke
        .lines()
        .filter_map(|line| line.trim().split_once(&opening))
        .map(|(_, event)| event.strip_suffix('\'').expect("one quoted event"))
        .map(str::to_string)
        .collect()
}

/// The value after `name: ` on a row of `text`, read whole.
fn pinned<'a>(text: &'a str, name: &str) -> &'a str {
    let opening = format!("{name}: ");
    text.lines()
        .find_map(|line| line.trim().strip_prefix(opening.as_str()))
        .unwrap_or_else(|| panic!("no {name}"))
}

/// One run of `lambda-smoke.sh` in `support/smoke_world.sh`, its tools stubbed.
struct SmokeRun {
    status: Option<i32>,
    out: String,
    state: tempfile::TempDir,
}

impl SmokeRun {
    /// Run the smoke in a world where `broken` goes wrong; "" is a good image and store.
    fn in_a_world_where(broken: &str) -> Self {
        let state = tempfile::tempdir().expect("the world");
        let world = state.path().join("world.sh");
        std::fs::write(&world, SMOKE_WORLD).expect("write the world");
        for tool in ["docker", "curl", "python3"] {
            let path = state.path().join(tool);
            let stub = format!(
                "#!/usr/bin/env bash\nSTATE='{}'\nBROKEN='{broken}'\n. '{}'\n{tool}_stub \"$@\"\n",
                state.path().display(),
                world.display(),
            );
            std::fs::write(&path, stub).expect("write a stub");
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .expect("chmod a stub");
        }
        let path = format!(
            "{}:{}",
            state.path().display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let output = Command::new("bash")
            .arg(workspace().join(LAMBDA_SMOKE))
            .env("PATH", path)
            .env("LOCAL_TAG", "the-image")
            .env("RIE_BIN", "/the-rie")
            .env("SEED_INDEX", "/the-seed")
            .env("MOTO_IMAGE", "the-moto")
            .output()
            .expect("run the smoke");
        Self {
            status: output.status.code(),
            out: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            state,
        }
    }

    /// The numbers of the assertions it reached, in the order it reached them.
    fn assertions(&self) -> Vec<u32> {
        self.out
            .lines()
            .filter_map(|line| line.strip_prefix("lambda-smoke: assertion "))
            .filter_map(|line| line.split(':').next()?.parse().ok())
            .collect()
    }

    /// The `docker run` of each function start, in order.
    fn function_starts(&self) -> Vec<String> {
        std::fs::read_to_string(self.state.path().join("calls.log"))
            .expect("the calls")
            .lines()
            .filter(|line| line.starts_with("docker run"))
            .filter(|line| line.contains("--name openlore-lambda-smoke "))
            .map(str::to_owned)
            .collect()
    }

    /// The environment file of the function start at `index`.
    fn environment_of(&self, index: usize) -> String {
        std::fs::read_to_string(self.state.path().join(format!("env-{}", index + 1)))
            .expect("the environment of a start")
    }
}

/// DEVOPS lambda-smoke, run: the script itself, with `docker`, `curl` and `python3` stubbed,
/// so this reads what it does and not what it says. Its one real run is the `indexer-image`
/// job of a main push, so this also holds the job to what that run must be. The assertions
/// come in order, because each one's bucket is the last one's.
#[test]
fn lambda_smoke_runs_every_assertion_as_lambda_runs_the_image() {
    const MODES: [&str; 10] = [
        "search", "search", "pass", "probe", "pass", "search", "pass", "pass", "pass", "pass",
    ];
    const REFUSED_LOOPBACK: [usize; 2] = [4, 5];
    const NO_STORE: usize = 1;
    let smoke = SmokeRun::in_a_world_where("");

    assert_eq!(smoke.status, Some(0), "{}", smoke.out);
    assert_eq!(
        smoke.assertions(),
        (1..=10).collect::<Vec<_>>(),
        "{}",
        smoke.out
    );
    assert!(
        smoke.out.ends_with("lambda-smoke: passed\n"),
        "{}",
        smoke.out
    );

    let starts = smoke.function_starts();
    assert_eq!(starts.len(), MODES.len(), "{starts:?}");
    let lambda_env: Vec<String> = lambda_env()
        .into_iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    for (index, (start, mode)) in starts.iter().zip(MODES).enumerate() {
        for posture in [
            "--user 993:990",
            "--read-only",
            "--tmpfs /tmp:size=512m",
            "--memory 512m",
            "-v /the-rie:/aws-lambda-rie:ro",
            "--entrypoint /aws-lambda-rie",
        ] {
            assert!(
                start.contains(posture),
                "{index} runs with `{posture}`: {start}"
            );
        }
        let image = format!(" the-image /usr/local/bin/openlore-indexer-lambda {mode}");
        assert!(start.ends_with(&image), "{start}");
        let endpoint = if index == NO_STORE {
            "-e AWS_ENDPOINT_URL=http://host.docker.internal:9 "
        } else {
            "-e AWS_ENDPOINT_URL=http://host.docker.internal:5056 "
        };
        assert!(start.contains(endpoint), "{start}");
        assert_eq!(
            start.contains("-e OPENLORE_INDEXER_ALLOW_LOOPBACK_HTTP=1"),
            REFUSED_LOOPBACK.contains(&index),
            "{start}"
        );

        let environment = smoke.environment_of(index);
        let given: BTreeSet<&str> = environment.lines().collect();
        for setting in &lambda_env {
            assert!(given.contains(setting.as_str()), "{setting}: {environment}");
        }
        for name in [
            "AWS_REGION=",
            "OPENLORE_INDEXER_LAMBDA_BUCKET=",
            "OPENLORE_INDEXER_LAMBDA_INDEX_KEY=",
            "OPENLORE_INDEXER_LAMBDA_LAST_PASS_KEY=",
        ] {
            assert!(environment.contains(name), "{name}: {environment}");
        }
        for (name, needed_by) in [
            ("OPENLORE_INDEXER_LAMBDA_DID_PARAMETER=", "pass"),
            ("OPENLORE_INDEXER_LAMBDA_PROBE_BASE_URL=", "probe"),
        ] {
            assert_eq!(
                environment.contains(name),
                mode == needed_by,
                "{environment}"
            );
        }
    }

    let ci = read(".github/workflows/ci.yml");
    let job_body = job(&ci, "indexer-image");
    let image = steps(job_body);
    let serve_at = step_at(&image, "- name: serve-smoke (production posture)");
    let smoke_at = step_at(&image, LAMBDA_SMOKE_STEP);
    let scanned_at = step_at(&image, "aquasecurity/trivy-action");
    assert!(serve_at < smoke_at && smoke_at < scanned_at);
    assert!(
        image[smoke_at].contains(LAMBDA_SMOKE) && image[smoke_at].contains("timeout-minutes:"),
        "the step runs the smoke, on a time limit"
    );
    assert!(
        !job_body.contains("continue-on-error"),
        "the smoke blocks the push"
    );
    assert!(job_body.contains("lambda-smoke: passed"), "job summary");

    let rie = image[step_at(&image, RIE_STEP)];
    assert!(step_at(&image, RIE_STEP) < smoke_at);
    assert!(rie.contains("releases/download/$RIE_VERSION/aws-lambda-rie-arm64"));
    assert!(rie.contains("sha256sum --check --strict"));
    let sha256 = pinned(job_body, "RIE_SHA256");
    assert!(
        sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "{sha256}"
    );
    assert!(pinned(job_body, "RIE_VERSION").starts_with("v1."));
    assert!(
        !read("crates/openlore-indexer/Dockerfile").contains("aws-lambda-rie"),
        "mounted, never in the image"
    );

    assert_eq!(
        pinned(job_body, "MOTO_IMAGE"),
        pinned(job(&ci, "rio-object-store"), "image"),
        "the same moto as the RIO suite"
    );
}

/// Each row is one thing a bad image or a bad store gets wrong: the assertion that must
/// stop the smoke there, and what it says. The good run above is this table's other half.
#[test]
fn lambda_smoke_stops_at_the_assertion_that_catches_a_bad_image() {
    for (broken, assertion, says) in [
        ("working-directory", 1, "WorkingDir is '/home/nonroot'"),
        ("empty-bucket-answers-rows", 2, "the answer is not"),
        (
            "large-body-taken",
            2,
            "the answer is not .statusCode == 413",
        ),
        ("search-starts", 3, "search did not end in an init error"),
        (
            "pass-of-nothing-publishes",
            4,
            "a pass that changed nothing published",
        ),
        ("probe-says-live", 5, "the probe said live"),
        (
            "refused-for-another-reason",
            6,
            "pass refused for another reason",
        ),
        (
            "no-publish-moves-a-version",
            7,
            "a no-publish run moved an object version",
        ),
        (
            "publish-without-if-match",
            8,
            "the publish's If-Match was null",
        ),
        (
            "published-after-the-summary",
            8,
            "indexer.lambda.published does not come before",
        ),
        ("stale-write-lands", 9, "the raced PUT was"),
        (
            "refused-event-ran",
            10,
            "a refused event wrote a pass_summary",
        ),
        ("refused-event-writes", 10, "a refused event wrote:"),
    ] {
        let smoke = SmokeRun::in_a_world_where(broken);

        assert_eq!(smoke.status, Some(1), "{broken}\n{}", smoke.out);
        assert_eq!(
            smoke.assertions().last(),
            Some(&assertion),
            "{broken}\n{}",
            smoke.out
        );
        let error = format!("::error::lambda-smoke: {says}");
        assert!(
            smoke.out.lines().any(|line| line.starts_with(&error)),
            "{broken}\n{}",
            smoke.out
        );
        assert!(!smoke.out.contains("lambda-smoke: passed"), "{broken}");
    }
}

/// D-STZ-44: anything richer than these is refused, so a sloppy event would prove a
/// refusal and no pass.
#[test]
fn each_smoke_invoke_sends_exactly_the_event_its_function_takes() {
    let smoke = read(LAMBDA_SMOKE);
    assert_eq!(
        events_sent(&smoke, "pass"),
        [
            "{}",
            r#"{"mode":"no-publish"}"#,
            "{}",
            "{}",
            r#"{"mdoe":"no-publish"}"#,
        ],
        "assertions 4, 7, 8, 9 and 10"
    );
    assert_eq!(events_sent(&smoke, "probe"), ["{}"], "assertion 5");
}

/// A release image has no test faults, so the stale version comes from the shim:
/// armed before the invoke, and only on the index key, or the write probe trips it.
#[test]
fn the_stale_version_is_armed_before_the_invoke_on_the_index_key_only() {
    let smoke = read(LAMBDA_SMOKE);
    let ninth = &smoke[position(&smoke, r#"assertion "9:"#)..];
    let armed = position(ninth, r#"arm_the_shim "/$BUCKET/$INDEX_KEY""#);
    assert!(armed < position(ninth, "run_once pass '{}'"));
    assert_eq!(smoke.matches("arm_the_shim \"").count(), 1, "armed once");

    let shim = read(RACE_SHIM);
    let imports: BTreeSet<&str> = shim
        .lines()
        .filter(|line| line.starts_with("import ") || line.starts_with("from "))
        .map(|line| line.split_whitespace().nth(1).expect("a module"))
        .collect();
    assert_eq!(
        imports,
        BTreeSet::from([
            "argparse",
            "http.client",
            "http.server",
            "json",
            "threading"
        ]),
        "Python's standard library, and nothing to install"
    );
    assert!(
        shim.contains("self.command == \"PUT\""),
        "a publish, no GET"
    );
    assert!(shim.contains("if_match"), "it logs conditional headers");
    assert!(
        shim.contains("if_none_match"),
        "it logs conditional headers"
    );
}

/// The seed goes round the shim, or the shim's log would hold it. Assertion 8 leaves
/// the purged index current, and a pass from there changes nothing, so assertion 9
/// needs the seed back.
#[test]
fn the_seed_goes_straight_to_moto_and_again_before_the_stale_version() {
    let smoke = read(LAMBDA_SMOKE);
    let seed = &smoke[position(&smoke, "seed_the_bucket() {")..];
    let seed = &seed[..position(seed, "\n}\n")];
    for row in [
        "x-amz-meta-published-at:",
        "x-amz-meta-pass-id:",
        r#"--upload-file "$SEED_INDEX""#,
        r#""$MOTO/$BUCKET/$INDEX_KEY""#,
    ] {
        assert!(seed.contains(row), "the seed has {row}");
    }
    assert!(!seed.contains("$SHIM"), "not through the shim");

    let seeded_at: Vec<usize> = smoke
        .match_indices("\nseed_the_bucket\n")
        .map(|(at, _)| at)
        .collect();
    assert_eq!(seeded_at.len(), 2, "seeded twice");
    let between = |earlier: &str, later: &str, at: usize| {
        position(&smoke, earlier) < at && at < position(&smoke, later)
    };
    assert!(between(
        r#"assertion "6:"#,
        r#"assertion "7:"#,
        seeded_at[0]
    ));
    assert!(between(
        r#"assertion "8:"#,
        r#"assertion "9:"#,
        seeded_at[1]
    ));

    let ci = read(".github/workflows/ci.yml");
    let build = steps(job(&ci, "indexer-build"));
    let seed_step = build[step_at(&build, SEED_STEP)];
    assert!(seed_step.contains("cargo run --release -p adapter-index-store --example seed_index"));
    assert!(step_at(&build, SEED_STEP) < step_at(&build, "actions/upload-artifact"));
    let image = job(&ci, "indexer-image");
    assert!(
        image.contains(
            r#"mv "$RUNNER_TEMP/ctx/seed-index.duckdb" "$RUNNER_TEMP/seed-index.duckdb""#
        ),
        "out of the build context, and so out of the image"
    );
}

/// NFR-IXD-8, on the image. The file that becomes the Lambda environment has its own
/// check in `scale_to_zero_architecture.rs`.
#[test]
fn the_image_shows_no_aws_credential_and_trivy_looks_for_secrets_too() {
    let ci = read(".github/workflows/ci.yml");
    let image = steps(job(&ci, "indexer-image"));
    let secret_at = step_at(&image, SECRET_STEP);
    let scanned_at = step_at(&image, "aquasecurity/trivy-action");
    assert!(secret_at < scanned_at);
    assert!(image[secret_at].contains("{{json .Config.Env}}"));
    assert!(image[secret_at].contains("^AWS_"));
    assert_eq!(pinned(image[scanned_at], "scanners"), "vuln,secret");
}

// ---------------------------------------------------------------------------------------------
// The operator runbook against the scripts it describes
// ---------------------------------------------------------------------------------------------

const RUNBOOK: &str = "deploy/indexer/README.md";
const LAMBDA_SCRIPT: &str = "deploy/indexer/lambda.sh";
const DEPLOY_SCRIPT: &str = "deploy/indexer/deploy.sh";
const EVIDENCE_FILE: &str = "docs/evolution/scale-to-zero-hosting-evidence.md";
const LAMBDA_DISPATCH: &str = "lambda_main() {";
const TARGET_CHECK: &str = "lambda_test_alarm() {";

/// The arms of the first `case` after `opening`, without the `*` that refuses.
fn case_arms(script: &str, opening: &str) -> Vec<String> {
    script[position(script, opening)..]
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("case "))
        .skip(1)
        .take_while(|line| line.trim() != "esac")
        .filter_map(|line| line.trim().split_once(')'))
        .map(|(pattern, _)| pattern)
        .filter(|pattern| *pattern != "*" && !pattern.starts_with('"'))
        .map(str::to_owned)
        .collect()
}

/// What `deploy.sh lambda` dispatches to, read from the script itself.
fn dispatched_verbs(lambda_script: &str) -> BTreeSet<String> {
    case_arms(lambda_script, LAMBDA_DISPATCH)
        .into_iter()
        .collect()
}

/// The verb of each row in the runbook's verb table.
fn verbs_with_a_row(runbook: &str) -> BTreeSet<String> {
    runbook
        .lines()
        .filter_map(|line| line.strip_prefix("| `lambda "))
        .map(verb_at)
        .collect()
}

fn verb_at(text: &str) -> String {
    text.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect()
}

/// Every verb the runbook names anywhere, in a command or in a sentence.
fn verbs_named(runbook: &str) -> BTreeSet<String> {
    ["deploy.sh lambda ", "`lambda "]
        .into_iter()
        .flat_map(|prefix| runbook.split(prefix).skip(1).map(verb_at))
        .filter(|verb| !verb.is_empty())
        .collect()
}

fn verbs_without_a_row(lambda_script: &str, runbook: &str) -> BTreeSet<String> {
    dispatched_verbs(lambda_script)
        .difference(&verbs_with_a_row(runbook))
        .cloned()
        .collect()
}

/// The verb list is read from the real dispatch, so a new verb cannot ship unnamed, and
/// a planted verb and a dropped row show the check can fail.
#[test]
fn the_runbook_names_every_lambda_verb() {
    let (script, runbook) = (read(LAMBDA_SCRIPT), read(RUNBOOK));
    let verbs = dispatched_verbs(&script);
    assert!(verbs.len() >= 20, "the dispatch was not read: {verbs:?}");
    assert!(verbs.contains("pass-off") && verbs.contains("cutover-watch"));

    assert_eq!(verbs_with_a_row(&runbook), verbs, "one row per verb");
    let unknown: Vec<_> = verbs_named(&runbook).difference(&verbs).cloned().collect();
    assert!(unknown.is_empty(), "the script takes no verb {unknown:?}");

    let planted = script.replace(
        "    status) lambda_status",
        "    fly-away) lambda_fly_away \"$@\" ;;\n    status) lambda_status",
    );
    assert_eq!(
        verbs_without_a_row(&planted, &runbook),
        BTreeSet::from(["fly-away".to_owned()]),
        "a verb added without a row fails"
    );
    let dropped: String = runbook
        .lines()
        .filter(|line| !line.starts_with("| `lambda status"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        verbs_without_a_row(&script, &dropped),
        BTreeSet::from(["status".to_owned()]),
        "a row removed from the runbook fails"
    );

    let targets = case_arms(&script, TARGET_CHECK);
    let targets = targets.first().expect("the targets test-alarm accepts");
    let row = runbook
        .lines()
        .find(|line| line.starts_with("| `lambda test-alarm"))
        .expect("the test-alarm row");
    for target in targets.split(" | ") {
        let target = target.trim_matches('"');
        assert!(row.contains(&format!("`{target}`")), "test-alarm {target}");
    }
}

#[test]
fn the_runbook_names_the_two_host_verbs_as_the_host_takes_them() {
    let (script, runbook) = (read(DEPLOY_SCRIPT), read(RUNBOOK));
    let verbs = case_arms(&script, "\ncmd=\"${1:-}\"");
    for verb in ["standby", "resume"] {
        assert!(verbs.iter().any(|arm| arm == verb), "deploy.sh {verb}");
    }
    let flags = case_arms(&script, "refuse_unless_one_standby_flag() {");
    assert_eq!(flags, ["--timers | --container"]);
    for command in ["standby --timers", "standby --container", "resume"] {
        let row = format!("| `deploy.sh {command}` |");
        assert!(runbook.contains(&row), "the runbook lists {command}");
    }
}

/// Each evidence item owed to the operator stages: the name its row starts with, its
/// stage, and the step the row must name.
const EVIDENCE_ITEMS: [(&str, &str, &str); 7] = [
    ("P-5", "Stage 1", "`deploy.sh lambda measure`"),
    ("P-15", "Stage 0", "the simulator below"),
    ("P-17", "Stage 0", "the simulator below"),
    ("P-18", "Stage 1", "20 concurrent raw-URL calls below"),
    (
        "140 MB post-pass steps",
        "Stage 1",
        "Time the local post-pass steps",
    ),
    ("140 MB query", "Stage 1", "Time a search and a reload"),
    ("Metric check", "Stage 0", "`lambda status`"),
];

/// The commands a row calls "below", as the heading of the block that holds them.
const EVIDENCE_BLOCKS: [&str; 2] = [
    "The simulator, for P-15 and P-17:\n\n```sh\n",
    "P-18, 20 concurrent calls:\n\n```sh\n",
];

/// What the runbook leaves out of its evidence rows.
fn missing_evidence(runbook: &str) -> Vec<String> {
    let rows = EVIDENCE_ITEMS.iter().filter_map(|(item, stage, step)| {
        let row = runbook
            .lines()
            .find(|line| line.starts_with(&format!("| {item} |")))?;
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        (cells[2] != *stage || !cells[3].contains(step)).then(|| format!("{item}: {row}"))
    });
    let unlisted = EVIDENCE_ITEMS
        .iter()
        .filter(|(item, ..)| !runbook.contains(&format!("\n| {item} |")))
        .map(|(item, ..)| format!("{item}: no row"));
    let blocks = EVIDENCE_BLOCKS
        .iter()
        .filter(|block| !runbook.contains(**block))
        .map(|block| format!("no block: {block}"));
    rows.chain(unlisted).chain(blocks).collect()
}

/// A planted row and a dropped block show the check can fail.
#[test]
fn each_evidence_item_names_its_stage_and_how_to_record_it() {
    let runbook = read(RUNBOOK);
    assert!(runbook.contains(EVIDENCE_FILE));
    assert_eq!(missing_evidence(&runbook), Vec::<String>::new());

    let vague = runbook.replace("`deploy.sh lambda measure` from your laptop", "Measure it");
    assert_eq!(
        missing_evidence(&vague).len(),
        1,
        "a row that names no step"
    );
    let moved = runbook.replace("| Metric check | Stage 0 |", "| Metric check | Stage 2 |");
    assert_eq!(
        missing_evidence(&moved).len(),
        1,
        "a row in the wrong stage"
    );
    let gone = runbook.replace("P-18, 20 concurrent calls:", "Some calls:");
    assert_eq!(missing_evidence(&gone).len(), 1, "a step that is not below");
}
