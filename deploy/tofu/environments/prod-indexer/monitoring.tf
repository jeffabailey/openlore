# Filters, alarms and saved queries (feature-delta.md § Monitoring contracts). All alarms use the
# prod root's topic: no new topic or subscription, and recoveries notify too. Their actions stay
# off until the bootstrap gate.
#
# No filter has a default_value: a default publishes for every non-matching batch, so the Minimum
# of a period could never reach 1 and A1-T would never fire.

locals {
  indexer_namespace = "OpenLore/Indexer"
  trial_namespace   = "OpenLore/IndexerLambdaTrial"
  alarm_topic       = [local.prod_outputs.backup_alarm_topic_arn]

  # The prod root's A1-A3 watch the host until cut-over, then the same filters read Lambda's lines
  # in the prod log group, so the trial's copies go away with the trial.
  while_trial = var.phase == "trial" ? 1 : 0
}

# -- Trial copies of the prod filters, on a namespace of their own --------------------------------

resource "aws_cloudwatch_log_metric_filter" "trial_pass_outage" {
  count          = local.while_trial
  name           = "indexer-lambda-pass-outage"
  log_group_name = aws_cloudwatch_log_group.lambda.name
  pattern        = "{ $.event = \"indexer.ingest.pass_summary\" && $.exit_code = 3 }"

  metric_transformation {
    name      = "IndexerPassOutage"
    namespace = local.trial_namespace
    value     = "1"
  }
}

resource "aws_cloudwatch_log_metric_filter" "trial_pass_not_outage" {
  count          = local.while_trial
  name           = "indexer-lambda-pass-not-outage"
  log_group_name = aws_cloudwatch_log_group.lambda.name
  pattern        = "{ $.event = \"indexer.ingest.pass_summary\" && $.exit_code != 3 }"

  metric_transformation {
    name      = "IndexerPassOutage"
    namespace = local.trial_namespace
    value     = "0"
  }
}

# Unfiltered on no_publish: a gate with no usable start means the store really is unusable.
resource "aws_cloudwatch_log_metric_filter" "trial_failure" {
  count          = local.while_trial
  name           = "indexer-lambda-failure"
  log_group_name = aws_cloudwatch_log_group.lambda.name
  pattern        = "{ ($.event = \"indexer.ingest.pass_summary\" && $.exit_code = 2) || $.event = \"health.startup.refused\" || $.event = \"indexer.store.unusable\" }"

  metric_transformation {
    name      = "IndexerFailure"
    namespace = local.trial_namespace
    value     = "1"
  }
}

resource "aws_cloudwatch_log_metric_filter" "trial_not_live" {
  count          = local.while_trial
  name           = "indexer-lambda-not-live"
  log_group_name = aws_cloudwatch_log_group.lambda.name
  pattern        = "{ $.event = \"indexer.host.health\" }"

  metric_transformation {
    name      = "IndexerNotLive"
    namespace = local.trial_namespace
    value     = "$.not_live"
  }
}

# -- Filters that stay, on whichever group the functions write to -----------------------------------

resource "aws_cloudwatch_log_metric_filter" "index_bytes" {
  name           = "indexer-lambda-index-bytes"
  log_group_name = local.lambda_log_group
  pattern        = "{ $.event = \"indexer.lambda.published\" }"

  metric_transformation {
    name      = "IndexBytes"
    namespace = local.indexer_namespace
    value     = "$.index_bytes"
  }
}

resource "aws_cloudwatch_log_metric_filter" "freshness_failed" {
  name           = "indexer-lambda-freshness-failed"
  log_group_name = local.lambda_log_group
  pattern        = "{ $.event = \"indexer.lambda.freshness_check_failed\" || $.event = \"indexer.lambda.served_index_stale\" }"

  metric_transformation {
    name      = "IndexerFreshnessFailed"
    namespace = local.indexer_namespace
    value     = "1"
  }
}

# -- Trial alarms, A1-A3 on the trial metrics ---------------------------------------------------

resource "aws_cloudwatch_metric_alarm" "trial_total_outage" {
  count               = local.while_trial
  alarm_name          = "openlore-indexer-lambda-total-outage"
  alarm_description   = "A1-T: two consecutive Lambda passes skipped every DID (exit 3). Read the skip reasons in /openlore/prod/indexer-lambda."
  namespace           = local.trial_namespace
  metric_name         = "IndexerPassOutage"
  statistic           = "Minimum"
  period              = 900
  evaluation_periods  = 2
  datapoints_to_alarm = 2
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic

  depends_on = [
    aws_cloudwatch_log_metric_filter.trial_pass_outage,
    aws_cloudwatch_log_metric_filter.trial_pass_not_outage,
  ]
}

resource "aws_cloudwatch_metric_alarm" "trial_pass_failed" {
  count               = local.while_trial
  alarm_name          = "openlore-indexer-lambda-pass-failed"
  alarm_description   = "A2-T: a Lambda pass exited 2 (a 412 included), including a pre-flip gate, or the store is unusable. Read the cause of the last pass_summary."
  namespace           = local.trial_namespace
  metric_name         = "IndexerFailure"
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  datapoints_to_alarm = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic

  depends_on = [aws_cloudwatch_log_metric_filter.trial_failure]
}

# Missing data is breaching: a probe that stops running pages.
resource "aws_cloudwatch_metric_alarm" "trial_not_live" {
  count               = local.while_trial
  alarm_name          = "openlore-indexer-lambda-not-live"
  alarm_description   = "A3-T: the Lambda index is not live, or the probe stopped writing lines. Check `deploy.sh lambda status` and the not_live fields of the probe line."
  namespace           = local.trial_namespace
  metric_name         = "IndexerNotLive"
  statistic           = "Maximum"
  period              = 300
  evaluation_periods  = 2
  datapoints_to_alarm = 2
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "breaching"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic

  depends_on = [aws_cloudwatch_log_metric_filter.trial_not_live]
}

# -- Permanent alarms, A4-A11 --------------------------------------------------------------------

resource "aws_cloudwatch_metric_alarm" "search_invocations" {
  alarm_name          = "openlore-indexer-search-invocations"
  alarm_description   = "A4: search passed 3,000 calls in 5 min. If this is abuse, `deploy.sh lambda stop`; nothing stops by itself."
  namespace           = "AWS/Lambda"
  metric_name         = "Invocations"
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  datapoints_to_alarm = 1
  comparison_operator = "GreaterThanThreshold"
  threshold           = 3000
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic

  dimensions = {
    FunctionName = aws_lambda_function.search.function_name
  }
}

# Maximum, not Sum: the size of the largest publish in the period. Publishes are rare, so the alarm
# keeps its state until a smaller one.
resource "aws_cloudwatch_metric_alarm" "index_size" {
  alarm_name          = "openlore-indexer-index-size"
  alarm_description   = "A5: a published index passed 100 MB; the cold start fails near 140 MB (D-STZ-33)."
  namespace           = local.indexer_namespace
  metric_name         = "IndexBytes"
  statistic           = "Maximum"
  period              = 900
  evaluation_periods  = 1
  datapoints_to_alarm = 1
  comparison_operator = "GreaterThanThreshold"
  threshold           = 104857600
  treat_missing_data  = "ignore"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic

  depends_on = [aws_cloudwatch_log_metric_filter.index_bytes]
}

# Account level: valid while these are the only functions in us-east-1.
resource "aws_cloudwatch_metric_alarm" "lambda_errors" {
  alarm_name          = "openlore-indexer-lambda-errors"
  alarm_description   = "A6: a function crashed, timed out or refused an event, a refused pass event included (D-STZ-44)."
  namespace           = "AWS/Lambda"
  metric_name         = "Errors"
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  datapoints_to_alarm = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic
}

resource "aws_cloudwatch_metric_alarm" "lambda_throttles" {
  alarm_name          = "openlore-indexer-lambda-throttles"
  alarm_description   = "A7: functions are being throttled. Search's cap of 3 is doing its job, or a stop is in force."
  namespace           = "AWS/Lambda"
  metric_name         = "Throttles"
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  datapoints_to_alarm = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 10
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic
}

resource "aws_cloudwatch_metric_alarm" "search_hourly" {
  alarm_name          = "openlore-indexer-search-hourly"
  alarm_description   = "A8: search passed 1,900 calls in an hour, the rate that spends the month's $2 (D-STZ-18e)."
  namespace           = "AWS/Lambda"
  metric_name         = "Invocations"
  statistic           = "Sum"
  period              = 3600
  evaluation_periods  = 1
  datapoints_to_alarm = 1
  comparison_operator = "GreaterThanThreshold"
  threshold           = 1900
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic

  dimensions = {
    FunctionName = aws_lambda_function.search.function_name
  }
}

resource "aws_cloudwatch_metric_alarm" "edge_requests" {
  alarm_name          = "openlore-indexer-edge-requests"
  alarm_description   = "A9: the edge passed 13,000 requests in an hour, past the free tier's rate (D-STZ-18f). 404s and 429s are billed too."
  namespace           = "AWS/CloudFront"
  metric_name         = "Requests"
  statistic           = "Sum"
  period              = 3600
  evaluation_periods  = 1
  datapoints_to_alarm = 1
  comparison_operator = "GreaterThanThreshold"
  threshold           = 13000
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic

  dimensions = {
    DistributionId = aws_cloudfront_distribution.index.id
    Region         = "Global"
  }
}

# At idle only the probe drives checks, 3 per 900 s, so 3 means every one in a quarter failed or
# read stale. Evidence, not proof: the stale reading is best effort (D-STZ-48).
resource "aws_cloudwatch_metric_alarm" "freshness_failed" {
  alarm_name          = "openlore-indexer-freshness-failed"
  alarm_description   = "A10: search's freshness checks failed or the probe read a stale index 3 times in 15 min. Run the indexer-lambda/freshness-failures query."
  namespace           = local.indexer_namespace
  metric_name         = "IndexerFreshnessFailed"
  statistic           = "Sum"
  period              = 900
  evaluation_periods  = 1
  datapoints_to_alarm = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 3
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic

  depends_on = [aws_cloudwatch_log_metric_filter.freshness_failed]
}

# At idle the probe makes 2 calls per 5 min, so a threshold above 1 could not fire when it matters;
# 2 of 2 absorbs a single blip.
resource "aws_cloudwatch_metric_alarm" "search_5xx" {
  alarm_name          = "openlore-indexer-search-5xx"
  alarm_description   = "A11: search on live answered 5xx in two consecutive 5 min periods, a failed init included."
  namespace           = "AWS/Lambda"
  metric_name         = "Url5xxCount"
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 2
  datapoints_to_alarm = 2
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.lambda_alarms_enabled
  alarm_actions       = local.alarm_topic
  ok_actions          = local.alarm_topic

  dimensions = {
    FunctionName = aws_lambda_function.search.function_name
    Resource     = "${aws_lambda_function.search.function_name}:${aws_lambda_alias.search_live.name}"
  }
}

# -- Saved Logs Insights queries (free) ----------------------------------------------------------
# The three functions share a log group, and a platform.report line does not name its function;
# the stream does.

resource "aws_cloudwatch_query_definition" "cold_starts" {
  name            = "indexer-lambda/cold-starts"
  log_group_names = [local.lambda_log_group]
  query_string    = <<-EOT
    filter type = "platform.report" and ispresent(record.metrics.initDurationMs)
    | parse @logStream /\/(?<function_name>openlore-indexer-[a-z]+)\[/
    | stats count() as cold_starts, pct(record.metrics.initDurationMs, 95) as init_p95_ms, max(record.metrics.initDurationMs) as init_max_ms by function_name, bin(1d)
  EOT
}

resource "aws_cloudwatch_query_definition" "memory" {
  name            = "indexer-lambda/memory"
  log_group_names = [local.lambda_log_group]
  query_string    = <<-EOT
    filter type = "platform.report"
    | parse @logStream /\/(?<function_name>openlore-indexer-[a-z]+)\[/
    | stats max(record.metrics.maxMemoryUsedMB) as max_memory_mb by function_name, bin(1d)
  EOT
}

resource "aws_cloudwatch_query_definition" "publishes" {
  name            = "indexer-lambda/publishes"
  log_group_names = [local.lambda_log_group]
  query_string    = <<-EOT
    filter event = "indexer.lambda.published"
    | fields @timestamp, pass_id, index_bytes
    | sort @timestamp desc
    | limit 50
  EOT
}

resource "aws_cloudwatch_query_definition" "freshness_failures" {
  name            = "indexer-lambda/freshness-failures"
  log_group_names = [local.lambda_log_group]
  query_string    = <<-EOT
    filter event in ["indexer.lambda.freshness_check_failed", "indexer.lambda.served_index_stale"]
    | fields @timestamp, event, served, recorded, lag_s
    | sort @timestamp desc
    | limit 100
  EOT
}

# source narrows the prod group to the probe once the host's health lines share it.
resource "aws_cloudwatch_query_definition" "probe" {
  name            = "indexer-lambda/probe"
  log_group_names = [local.lambda_log_group]
  query_string    = <<-EOT
    filter event = "indexer.host.health" and `source` = "lambda-probe"
    | fields @timestamp, not_live, healthz_ok, search_ok, search_status, search_ms, last_pass_age_s, last_pass_exit_code
    | sort @timestamp desc
    | limit 96
  EOT
}
