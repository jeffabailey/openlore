# The review app's observability in prod (ADR-075; devops/monitoring-alerting.md,
# devops/observability-design.md). Laptop apply; nothing here touches the host.
#
# What is NOT here, on purpose:
#   - SSM parameters. The operator puts /openlore/prod/review-app/* from the laptop
#     (devops/infrastructure-integration.md §5.1); no secret value ever enters state.
#   - A DNS record. app.openlore.jeffbailey.us is already covered by the wildcard record.
#   - An SNS topic. The alarms reuse the module's backup-alarm topic, already subscribed out of
#     band.
#
# Four alarms only (user decision 2026-10-04): A-1 unreachable, A-3 down or restarted,
# A-7 privacy guardrail breach, A-8 GitHub token expiring. They exist with actions disabled
# until rollout step R-6 sets review_app_alarms_enabled = true.

variable "review_app_alarms_enabled" {
  description = "Whether the review app's alarms notify. False until rollout step R-6 (monitoring-alerting.md)."
  type        = bool
  default     = false
}

locals {
  review_app_fqdn      = "app.${local.descriptor.pds_hostname}"
  review_app_log_group = "/openlore/prod/review-app"
  review_app_namespace = "OpenLore/ReviewApp"
  review_app_topic     = [module.pds.backup_alarm_topic_arn]
}

# App stdout (dockerd awslogs) and the host's host-health stream. 30 days minimizes retention of
# pseudonymous data; longer KPI windows come from the app's aggregate counters.
resource "aws_cloudwatch_log_group" "review_app" {
  name              = local.review_app_log_group
  retention_in_days = 30
}

# -- A-1: external reachability -------------------------------------------------------------

resource "aws_route53_health_check" "review_app" {
  fqdn              = local.review_app_fqdn
  type              = "HTTPS"
  port              = 443
  resource_path     = "/healthz"
  request_interval  = 30
  failure_threshold = 3

  tags = {
    Name = "review-app-healthz"
  }
}

resource "aws_cloudwatch_metric_alarm" "review_app_unreachable" {
  alarm_name          = "openlore-review-app-unreachable"
  alarm_description   = "A-1: https://${local.review_app_fqdn}/healthz failing for 5 min. Check `deploy.sh status`; then monitoring-alerting.md §2.1."
  namespace           = "AWS/Route53"
  metric_name         = "HealthCheckStatus"
  dimensions          = { HealthCheckId = aws_route53_health_check.review_app.id }
  statistic           = "Minimum"
  period              = 60
  evaluation_periods  = 5
  datapoints_to_alarm = 5
  comparison_operator = "LessThanThreshold"
  threshold           = 1
  treat_missing_data  = "breaching"
  actions_enabled     = var.review_app_alarms_enabled
  alarm_actions       = local.review_app_topic
  ok_actions          = local.review_app_topic
}

# -- A-3: down or restarted (from the host's host.health line) ------------------------------

resource "aws_cloudwatch_log_metric_filter" "review_app_down" {
  name           = "review-app-down-or-restarted"
  log_group_name = aws_cloudwatch_log_group.review_app.name
  pattern        = "{ $.event = \"host.health\" && ($.restarts > 0 || $.app_running = 0) }"

  metric_transformation {
    name      = "AppDown"
    namespace = local.review_app_namespace
    value     = "1"
  }
}

resource "aws_cloudwatch_metric_alarm" "review_app_down" {
  alarm_name          = "openlore-review-app-down-or-restarted"
  alarm_description   = "A-3: the app container stopped, restarted or was OOM-killed. Check `deploy.sh status`; then monitoring-alerting.md §2.2."
  namespace           = local.review_app_namespace
  metric_name         = aws_cloudwatch_log_metric_filter.review_app_down.metric_transformation[0].name
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.review_app_alarms_enabled
  alarm_actions       = local.review_app_topic
  ok_actions          = local.review_app_topic
}

# -- A-7: privacy guardrail breach ------------------------------------------------------------

resource "aws_cloudwatch_log_metric_filter" "review_app_guardrail" {
  name           = "review-app-guardrail-breach"
  log_group_name = aws_cloudwatch_log_group.review_app.name
  pattern        = "{ $.event = \"guardrail.breach\" }"

  metric_transformation {
    name      = "GuardrailBreaches"
    namespace = local.review_app_namespace
    value     = "1"
  }
}

resource "aws_cloudwatch_metric_alarm" "review_app_guardrail" {
  alarm_name          = "openlore-review-app-guardrail-breach"
  alarm_description   = "A-7 (PRIVACY): a guardrail.breach event. Read review-app/guardrails in Logs Insights; then monitoring-alerting.md §2.3."
  namespace           = local.review_app_namespace
  metric_name         = aws_cloudwatch_log_metric_filter.review_app_guardrail.metric_transformation[0].name
  statistic           = "Sum"
  period              = 300
  evaluation_periods  = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.review_app_alarms_enabled
  alarm_actions       = local.review_app_topic
  ok_actions          = local.review_app_topic
}

# -- A-8: GitHub token expiring (<= 14 days) --------------------------------------------------

resource "aws_cloudwatch_log_metric_filter" "review_app_token_expiring" {
  name           = "review-app-github-token-expiring"
  log_group_name = aws_cloudwatch_log_group.review_app.name
  pattern        = "{ $.event = \"github.token.expiring\" }"

  metric_transformation {
    name      = "GithubTokenExpiring"
    namespace = local.review_app_namespace
    value     = "1"
  }
}

resource "aws_cloudwatch_metric_alarm" "review_app_token_expiring" {
  alarm_name          = "openlore-review-app-github-token-expiring"
  alarm_description   = "A-8: the GitHub PAT expires within 14 days. Rotate it: infrastructure-integration.md §7.4."
  namespace           = local.review_app_namespace
  metric_name         = aws_cloudwatch_log_metric_filter.review_app_token_expiring.metric_transformation[0].name
  statistic           = "Sum"
  period              = 86400
  evaluation_periods  = 1
  comparison_operator = "GreaterThanOrEqualToThreshold"
  threshold           = 1
  treat_missing_data  = "notBreaching"
  actions_enabled     = var.review_app_alarms_enabled
  alarm_actions       = local.review_app_topic
  ok_actions          = local.review_app_topic
}

output "review_app_health_check_id" { value = aws_route53_health_check.review_app.id }
