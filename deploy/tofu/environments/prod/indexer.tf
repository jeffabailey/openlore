# The network indexer's prod resources (indexer-deployment DV-IXD-8; devops/monitoring-alerting.md,
# devops/observability-design.md §5, devops/infrastructure-integration.md §7.2). Laptop apply;
# nothing here touches the host. Creates only: 1 parameter, 1 log group, 4 metric filters,
# 3 alarms, 4 saved queries (13 to add).
#
# What is NOT here, on purpose:
#   - An SNS topic or subscription. The three alarms reuse the module's backup-alarm topic,
#     already subscribed out of band, and notify recoveries too (ok_actions).
#   - A DNS record. index.openlore.jeffbailey.us is covered by the wildcard record.
#
# Exactly three alarms (user decision 2026-10-06): A1 total outage, A2 pass failed / refused /
# store unusable, A3 not live. They exist with actions disabled until rollout step I-6 sets
# indexer_alarms_enabled = true (variables.tf).
#
# The metric filters carry NO default_value: a default publishes 0 for every non-matching batch,
# so the Minimum of a period could never reach 1 and A1 would never fire (wave-decisions.md,
# review item 1). A period without a pass_summary has no data point; A1 treats it as notBreaching
# and gaps in passes are A3's job.

locals {
  indexer_log_group = "/openlore/prod/indexer"
  indexer_namespace = "OpenLore/Indexer"
  indexer_topic     = [module.pds.backup_alarm_topic_arn]
}

# The DID list the pass renders into /pds/indexer/config/repo-dids (render-dids.sh). A Standard
# String: public repository DIDs, not a secret. Seeded with the operator's own OpenLore DID
# (DEVOPS U-2); the operator edits the list with the AWS CLI (deploy/indexer/README.md), so the
# value is never drift.
resource "aws_ssm_parameter" "indexer_repo_dids" {
  name        = "/openlore/prod/indexer/repo-dids"
  description = "OpenLore indexer: comma- or newline-separated repository DIDs to index."
  type        = "String"
  tier        = "Standard"
  value       = "did:plc:pnyxfnpkcldxtitsw64ycahw"

  lifecycle {
    ignore_changes = [value]
  }
}

# serve stdout/stderr (dockerd awslogs, stream `indexer`) and the host's `host-health` and
# `host-dids` streams. Structural events only; 30 days, as the review app.
resource "aws_cloudwatch_log_group" "indexer" {
  name              = local.indexer_log_group
  retention_in_days = 30
}

# -- Metric filters (4 filters, 3 metrics; monitoring-alerting.md §1) -------------------------

# A pass that skipped every DID (exit 3) publishes 1 ...
resource "aws_cloudwatch_log_metric_filter" "indexer_pass_outage" {
  name           = "indexer-pass-outage"
  log_group_name = aws_cloudwatch_log_group.indexer.name
  pattern        = "{ $.event = \"indexer.ingest.pass_summary\" && $.exit_code = 3 }"

  metric_transformation {
    name      = "IndexerPassOutage"
    namespace = local.indexer_namespace
    value     = "1"
  }
}

# ... and any other pass publishes 0 to the same metric, so a period's Minimum is 1 only when
# every pass in it was a total outage.
resource "aws_cloudwatch_log_metric_filter" "indexer_pass_not_outage" {
  name           = "indexer-pass-not-outage"
  log_group_name = aws_cloudwatch_log_group.indexer.name
  pattern        = "{ $.event = \"indexer.ingest.pass_summary\" && $.exit_code != 3 }"

  metric_transformation {
    name      = "IndexerPassOutage"
    namespace = local.indexer_namespace
    value     = "0"
  }
}

resource "aws_cloudwatch_log_metric_filter" "indexer_failure" {
  name           = "indexer-failure"
  log_group_name = aws_cloudwatch_log_group.indexer.name
  pattern        = "{ ($.event = \"indexer.ingest.pass_summary\" && $.exit_code = 2) || $.event = \"health.startup.refused\" || $.event = \"indexer.store.unusable\" }"

  metric_transformation {
    name      = "IndexerFailure"
    namespace = local.indexer_namespace
    value     = "1"
  }
}

# The host health line computes not_live from every liveness cause (container down, /healthz
# failing, no shipped pass_summary in 45 min, DID list older than 2 h); render failures surface
# through that staleness rather than through a filter of their own.
resource "aws_cloudwatch_log_metric_filter" "indexer_not_live" {
  name           = "indexer-not-live"
  log_group_name = aws_cloudwatch_log_group.indexer.name
  pattern        = "{ $.event = \"indexer.host.health\" }"

  metric_transformation {
    name      = "IndexerNotLive"
    namespace = local.indexer_namespace
    value     = "$.not_live"
  }
}

# -- Alarms (exactly 3, on the existing topic; monitoring-alerting.md §1) ----------------------

# A1: two consecutive exit-3 passes. Passes start on the quarter hour and 900 s periods align to
# the same boundaries, so each period holds one pass_summary.
resource "aws_cloudwatch_metric_alarm" "indexer_total_outage" {
  alarm_name          = "openlore-indexer-total-outage"
  alarm_description   = "A1: two consecutive passes skipped every DID (exit 3). Check `deploy.sh status` skip reasons; then monitoring-alerting.md §2.1."
  namespace           = local.indexer_namespace
  metric_name         = "IndexerPassOutage"
  statistic           = "Minimum"
  period              = 900
  evaluation_periods  = 2
  datapoints_to_alarm = 2
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.indexer_alarms_enabled
  alarm_actions       = local.indexer_topic
  ok_actions          = local.indexer_topic

  depends_on = [
    aws_cloudwatch_log_metric_filter.indexer_pass_outage,
    aws_cloudwatch_log_metric_filter.indexer_pass_not_outage,
  ]
}

resource "aws_cloudwatch_metric_alarm" "indexer_pass_failed" {
  alarm_name          = "openlore-indexer-pass-failed"
  alarm_description   = "A2: a pass exited 2, serve refused to start, or the store is unusable. Read the cause of the last pass_summary or refusal; then monitoring-alerting.md §2.2."
  namespace           = local.indexer_namespace
  metric_name         = "IndexerFailure"
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  datapoints_to_alarm = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.indexer_alarms_enabled
  alarm_actions       = local.indexer_topic
  ok_actions          = local.indexer_topic

  depends_on = [aws_cloudwatch_log_metric_filter.indexer_failure]
}

# Missing data is breaching: a dead host, a dead health timer or broken host log shipping pages.
resource "aws_cloudwatch_metric_alarm" "indexer_not_live" {
  alarm_name          = "openlore-indexer-not-live"
  alarm_description   = "A3: the index is not live (container down, /healthz failing, no pass in 45 min, stale DID list, or no health lines). Check `deploy.sh status` and the not_live fields; then monitoring-alerting.md §2.3."
  namespace           = local.indexer_namespace
  metric_name         = "IndexerNotLive"
  statistic           = "Maximum"
  period              = 300
  evaluation_periods  = 2
  datapoints_to_alarm = 2
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "breaching"
  actions_enabled     = var.indexer_alarms_enabled
  alarm_actions       = local.indexer_topic
  ok_actions          = local.indexer_topic

  depends_on = [aws_cloudwatch_log_metric_filter.indexer_not_live]
}

# -- Saved Logs Insights queries (observability-design.md §5; free) ---------------------------
# Structural fields only; TEST* pass ids are the A1 drill's injected lines.

resource "aws_cloudwatch_query_definition" "indexer_freshness" {
  name            = "indexer/freshness"
  log_group_names = [aws_cloudwatch_log_group.indexer.name]
  query_string    = <<-EOT
    filter event = "indexer.ingest.pass_summary" and pass_id not like /^TEST/
    | fields @timestamp, exit_code, configured, own_pds, fallback, skipped, purged_authors, duration_ms, pass_id
    | sort @timestamp desc
    | limit 8
  EOT
}

# KPI-IXD-3 over the full 30 days: every exit-0 pass time (at most 2,880 rows).
resource "aws_cloudwatch_query_definition" "indexer_kpi_freshness" {
  name            = "indexer/kpi-freshness"
  log_group_names = [aws_cloudwatch_log_group.indexer.name]
  query_string    = <<-EOT
    filter event = "indexer.ingest.pass_summary" and exit_code = 0 and pass_id not like /^TEST/
    | fields @timestamp
    | sort @timestamp asc
    | limit 10000
  EOT
}

resource "aws_cloudwatch_query_definition" "indexer_exit_codes" {
  name            = "indexer/exit-codes"
  log_group_names = [aws_cloudwatch_log_group.indexer.name]
  query_string    = <<-EOT
    filter event = "indexer.ingest.pass_summary" and pass_id not like /^TEST/
    | stats count() by exit_code, bin(1d)
  EOT
}

# Skips, purges and refusals of a pass; narrow with `and pass_id = "<id>"` when running it.
resource "aws_cloudwatch_query_definition" "indexer_skips" {
  name            = "indexer/skips"
  log_group_names = [aws_cloudwatch_log_group.indexer.name]
  query_string    = <<-EOT
    filter event in ["indexer.ingest.source_skipped", "indexer.ingest.author_purged", "indexer.ingest.pass_refused"]
    | fields @timestamp, pass_id, event, did, reason, cause, claims_removed
    | sort @timestamp desc
    | limit 200
  EOT
}
