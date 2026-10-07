# The indexer's host-role delta (indexer-deployment DV-IXD-10; devops/infrastructure-integration.md
# §7.1): an inline policy on the existing prod host role, mirroring review-app-iam.tf. It does not
# modify the module's policy.
#
# Only HOST processes use the role -- dockerd's awslogs driver (stream `indexer`), render-dids.sh
# (stream `host-dids`) and the health timer (stream `host-health`), all as root. Containers cannot
# reach it (module v1.7.0 sets IMDS hop limit 1, and deploy.sh install refuses a host without it).
#
# Granted, and nothing more:
#   - read the DID-list parameter (a Standard String, so no KMS grant);
#   - create streams in, write to and describe the indexer's own log group;
#   - FilterLogEvents on that group only (DEVOPS U-1, approved): the health timer reads the age of
#     the last SHIPPED pass_summary, which also proves the awslogs pipeline end to end.
# No SSM write, no CreateLogGroup (the prod root owns the group), no PutMetricData (alarms derive
# from log metric filters), no wildcard resource.

locals {
  indexer_account   = "091153021562"
  indexer_region    = local.environments.prod.aws_region
  indexer_host_role = "openlore-pds-host-prod"
  indexer_dids_arn  = "arn:aws:ssm:${local.indexer_region}:${local.indexer_account}:parameter/openlore/prod/indexer/repo-dids"
  indexer_logs_arn  = "arn:aws:logs:${local.indexer_region}:${local.indexer_account}:log-group:/openlore/prod/indexer"
}

data "aws_iam_policy_document" "indexer_host" {
  statement {
    sid       = "ReadIndexerDidList"
    actions   = ["ssm:GetParameter"]
    resources = [local.indexer_dids_arn]
  }

  # Streams indexer, host-health, host-dids (and test-fire, created by hand for the A1 drill).
  statement {
    sid       = "WriteIndexerLogs"
    actions   = ["logs:CreateLogStream", "logs:PutLogEvents", "logs:DescribeLogStreams"]
    resources = ["${local.indexer_logs_arn}:*"]
  }

  statement {
    sid       = "ReadIndexerHeartbeat"
    actions   = ["logs:FilterLogEvents"]
    resources = ["${local.indexer_logs_arn}:*"]
  }
}

resource "aws_iam_role_policy" "indexer_host" {
  name   = "openlore-indexer-host"
  role   = local.indexer_host_role
  policy = data.aws_iam_policy_document.indexer_host.json

  # The role is created by the bootstrap module.
  depends_on = [module.pds_bootstrap]
}
