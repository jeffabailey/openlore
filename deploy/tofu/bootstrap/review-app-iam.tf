# The review app's host-role delta (devops/infrastructure-integration.md §3): an inline policy on
# the existing prod host role. It does not modify the module's policy.
#
# Only HOST processes use the role -- dockerd's awslogs driver, render-secrets.sh and the health
# timer, all as root. Containers cannot reach it (module v1.7.0 sets IMDS hop limit 1).
# No SSM write, no CreateLogGroup (the prod root owns the group), no PutMetricData (alarms derive
# from log metric filters). kms:Decrypt for aws/ssm is already granted by the module.

locals {
  review_app_account   = "091153021562"
  review_app_region    = local.environments.prod.aws_region
  review_app_host_role = "openlore-pds-host-prod"
  review_app_ssm_arn   = "arn:aws:ssm:${local.review_app_region}:${local.review_app_account}:parameter/openlore/prod/review-app"
  review_app_logs_arn  = "arn:aws:logs:${local.review_app_region}:${local.review_app_account}:log-group:/openlore/prod/review-app"
}

data "aws_iam_policy_document" "review_app_host" {
  statement {
    sid       = "ReadReviewAppSecrets"
    actions   = ["ssm:GetParameter", "ssm:GetParameters", "ssm:GetParametersByPath"]
    resources = [local.review_app_ssm_arn, "${local.review_app_ssm_arn}/*"]
  }

  statement {
    sid       = "WriteReviewAppLogs"
    actions   = ["logs:CreateLogStream", "logs:PutLogEvents", "logs:DescribeLogStreams"]
    resources = ["${local.review_app_logs_arn}:*"]
  }
}

resource "aws_iam_role_policy" "review_app_host" {
  name   = "openlore-review-app-host"
  role   = local.review_app_host_role
  policy = data.aws_iam_policy_document.review_app_host.json

  # The role is created by the bootstrap module.
  depends_on = [module.pds_bootstrap]
}
