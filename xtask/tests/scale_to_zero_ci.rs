//! Crafter-owned checks of `deploy/tofu/environments/prod-indexer`: the settings
//! the DEVOPS tables fix that no scenario pins. Each is a row, read whole, so `1`
//! is not `10`. Nothing here runs tofu or reaches AWS.
//!
//! The reader repeats the small one in `scale_to_zero_platform.rs`, which is a
//! DISTILL suite and has no library to share it through.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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
