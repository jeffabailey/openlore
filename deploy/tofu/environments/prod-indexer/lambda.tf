# Three functions from one image. deploy.sh owns which image and which version each alias points
# at (D-STZ-29), so tofu creates them and then leaves them alone.

locals {
  lambda_entry_point = ["/usr/local/bin/openlore-indexer-lambda"]
  lambda_log_group   = var.phase == "cutover" ? local.prod_outputs.indexer_log_group : aws_cloudwatch_log_group.lambda.name

  # The settings every function reads from a resource here, not from lambda-env.json.
  object_store_env = {
    OPENLORE_INDEXER_LAMBDA_BUCKET        = aws_s3_bucket.index.bucket
    OPENLORE_INDEXER_LAMBDA_INDEX_KEY     = local.index_key
    OPENLORE_INDEXER_LAMBDA_LAST_PASS_KEY = local.last_pass_key
  }
}

resource "aws_cloudwatch_log_group" "lambda" {
  name              = "/openlore/prod/indexer-lambda"
  retention_in_days = 30
}

resource "aws_lambda_function" "search" {
  function_name = "openlore-indexer-search"
  role          = aws_iam_role.search.arn
  package_type  = "Image"
  architectures = ["arm64"]
  image_uri     = var.bootstrap_image_uri
  publish       = false
  memory_size   = 512
  timeout       = 10

  # The kill switch sets this to 0 by CLI, so an apply must not put it back.
  reserved_concurrent_executions = 3

  image_config {
    entry_point       = local.lambda_entry_point
    command           = ["search"]
    working_directory = "/"
  }

  ephemeral_storage {
    size = 512
  }

  logging_config {
    log_format       = "JSON"
    log_group        = local.lambda_log_group
    system_log_level = "INFO"
  }

  environment {
    variables = merge(local.lambda_env, local.object_store_env)
  }

  lifecycle {
    ignore_changes = [image_uri, reserved_concurrent_executions]
  }

  depends_on = [aws_ecr_repository_policy.indexer]
}

resource "aws_lambda_function" "pass" {
  function_name = "openlore-indexer-pass"
  role          = aws_iam_role.pass.arn
  package_type  = "Image"
  architectures = ["arm64"]
  image_uri     = var.bootstrap_image_uri
  publish       = false
  memory_size   = 512
  timeout       = 840

  reserved_concurrent_executions = 1

  image_config {
    entry_point       = local.lambda_entry_point
    command           = ["pass"]
    working_directory = "/"
  }

  ephemeral_storage {
    size = 512
  }

  logging_config {
    log_format       = "JSON"
    log_group        = local.lambda_log_group
    system_log_level = "INFO"
  }

  environment {
    variables = merge(local.lambda_env, local.object_store_env, {
      OPENLORE_INDEXER_LAMBDA_DID_PARAMETER = local.prod_outputs.indexer_repo_dids_parameter
    })
  }

  lifecycle {
    ignore_changes = [image_uri]
  }

  depends_on = [aws_ecr_repository_policy.indexer]
}

resource "aws_lambda_function" "probe" {
  function_name = "openlore-indexer-probe"
  role          = aws_iam_role.probe.arn
  package_type  = "Image"
  architectures = ["arm64"]
  image_uri     = var.bootstrap_image_uri
  publish       = false
  memory_size   = 128
  timeout       = 30

  reserved_concurrent_executions = 1

  image_config {
    entry_point       = local.lambda_entry_point
    command           = ["probe"]
    working_directory = "/"
  }

  logging_config {
    log_format       = "JSON"
    log_group        = local.lambda_log_group
    system_log_level = "INFO"
  }

  # The CloudFront domain in both phases, so the probe never reaches the host during the DNS drain.
  environment {
    variables = merge(local.lambda_env, local.object_store_env, {
      OPENLORE_INDEXER_LAMBDA_PROBE_BASE_URL = "https://${aws_cloudfront_distribution.index.domain_name}"
    })
  }

  lifecycle {
    ignore_changes = [image_uri]
  }

  depends_on = [aws_ecr_repository_policy.indexer]
}

# Scheduler invokes asynchronously, and Lambda's own async retry (default 2) would rerun a crashed
# pass. The setting is per qualifier and the schedules target live, so it goes on live: on the
# bare function it would not apply to what they start.
resource "aws_lambda_function_event_invoke_config" "pass" {
  function_name                = aws_lambda_function.pass.function_name
  qualifier                    = aws_lambda_alias.pass_live.name
  maximum_retry_attempts       = 0
  maximum_event_age_in_seconds = 600
}

resource "aws_lambda_function_event_invoke_config" "probe" {
  function_name                = aws_lambda_function.probe.function_name
  qualifier                    = aws_lambda_alias.probe_live.name
  maximum_retry_attempts       = 0
  maximum_event_age_in_seconds = 240
}

resource "aws_lambda_alias" "search_live" {
  name             = "live"
  function_name    = aws_lambda_function.search.function_name
  function_version = "$LATEST"

  lifecycle {
    ignore_changes = [function_version, description]
  }
}

resource "aws_lambda_alias" "search_candidate" {
  name             = "candidate"
  function_name    = aws_lambda_function.search.function_name
  function_version = "$LATEST"

  lifecycle {
    ignore_changes = [function_version, description]
  }
}

resource "aws_lambda_alias" "pass_live" {
  name             = "live"
  function_name    = aws_lambda_function.pass.function_name
  function_version = "$LATEST"

  lifecycle {
    ignore_changes = [function_version, description]
  }
}

resource "aws_lambda_alias" "probe_live" {
  name             = "live"
  function_name    = aws_lambda_function.probe.function_name
  function_version = "$LATEST"

  lifecycle {
    ignore_changes = [function_version, description]
  }
}

resource "aws_lambda_function_url" "search_live" {
  function_name      = aws_lambda_function.search.function_name
  qualifier          = aws_lambda_alias.search_live.name
  authorization_type = "NONE"
  invoke_mode        = "BUFFERED"
}

resource "aws_lambda_function_url" "search_candidate" {
  function_name      = aws_lambda_function.search.function_name
  qualifier          = aws_lambda_alias.search_candidate.name
  authorization_type = "NONE"
  invoke_mode        = "BUFFERED"
}

# A public Function URL needs both grants (spike finding 9).
resource "aws_lambda_permission" "search_live_url" {
  statement_id           = "public-url"
  action                 = "lambda:InvokeFunctionUrl"
  function_name          = aws_lambda_function.search.function_name
  qualifier              = aws_lambda_alias.search_live.name
  principal              = "*"
  function_url_auth_type = "NONE"
}

resource "aws_lambda_permission" "search_live_invoke" {
  statement_id             = "public-url-invoke"
  action                   = "lambda:InvokeFunction"
  function_name            = aws_lambda_function.search.function_name
  qualifier                = aws_lambda_alias.search_live.name
  principal                = "*"
  invoked_via_function_url = true
}

resource "aws_lambda_permission" "search_candidate_url" {
  statement_id           = "public-url"
  action                 = "lambda:InvokeFunctionUrl"
  function_name          = aws_lambda_function.search.function_name
  qualifier              = aws_lambda_alias.search_candidate.name
  principal              = "*"
  function_url_auth_type = "NONE"
}

resource "aws_lambda_permission" "search_candidate_invoke" {
  statement_id             = "public-url-invoke"
  action                   = "lambda:InvokeFunction"
  function_name            = aws_lambda_function.search.function_name
  qualifier                = aws_lambda_alias.search_candidate.name
  principal                = "*"
  invoked_via_function_url = true
}
