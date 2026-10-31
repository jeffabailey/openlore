# One role per function, exact object ARNs, no managed policy: AWSLambdaBasicExecutionRole grants
# logs:CreateLogGroup on *.

locals {
  index_object_arn       = "${aws_s3_bucket.index.arn}/${local.index_key}"
  last_pass_object_arn   = "${aws_s3_bucket.index.arn}/${local.last_pass_key}"
  write_probe_object_arn = "${aws_s3_bucket.index.arn}/${local.write_probe_key}"

  # The prod group is granted only at cut-over, so during the trial no function can write into
  # the group A1-A3 read.
  lambda_log_arns = concat(
    ["arn:aws:logs:${local.region}:${local.account_id}:log-group:${aws_cloudwatch_log_group.lambda.name}:*"],
    var.phase == "cutover" ? ["arn:aws:logs:${local.region}:${local.account_id}:log-group:${local.prod_outputs.indexer_log_group}:*"] : [],
  )
}

data "aws_iam_policy_document" "lambda_trust" {
  statement {
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["lambda.amazonaws.com"]
    }

    condition {
      test     = "StringEquals"
      variable = "aws:SourceAccount"
      values   = [local.account_id]
    }
  }
}

data "aws_iam_policy_document" "scheduler_trust" {
  statement {
    actions = ["sts:AssumeRole"]

    principals {
      type        = "Service"
      identifiers = ["scheduler.amazonaws.com"]
    }

    condition {
      test     = "StringEquals"
      variable = "aws:SourceAccount"
      values   = [local.account_id]
    }
  }
}

# What every function needs. Without ListBucket a missing object is 403, not 404, and D-STZ-13's
# empty start would refuse instead.
data "aws_iam_policy_document" "every_function" {
  statement {
    sid       = "TellMissingFromForbidden"
    actions   = ["s3:ListBucket"]
    resources = [aws_s3_bucket.index.arn]

    condition {
      test     = "StringLike"
      variable = "s3:prefix"
      values   = ["index/*"]
    }
  }

  statement {
    sid       = "WriteLogs"
    actions   = ["logs:CreateLogStream", "logs:PutLogEvents"]
    resources = local.lambda_log_arns
  }
}

data "aws_iam_policy_document" "search" {
  source_policy_documents = [data.aws_iam_policy_document.every_function.json]

  # HEAD needs s3:GetObject; there is no HeadObject action.
  statement {
    sid       = "ReadIndex"
    actions   = ["s3:GetObject"]
    resources = [local.index_object_arn]
  }
}

data "aws_iam_policy_document" "pass" {
  source_policy_documents = [data.aws_iam_policy_document.every_function.json]

  statement {
    sid       = "ReadIndex"
    actions   = ["s3:GetObject"]
    resources = [local.index_object_arn]
  }

  # The two publish statements: an unconditional overwrite of the index is denied by IAM too.
  statement {
    sid       = "PublishOverTheVersionRead"
    actions   = ["s3:PutObject"]
    resources = [local.index_object_arn]

    condition {
      test     = "Null"
      variable = "s3:if-match"
      values   = ["false"]
    }
  }

  statement {
    sid       = "PublishTheFirstIndex"
    actions   = ["s3:PutObject"]
    resources = [local.index_object_arn]

    condition {
      test     = "Null"
      variable = "s3:if-none-match"
      values   = ["false"]
    }
  }

  statement {
    sid       = "WritePassRecord"
    actions   = ["s3:PutObject"]
    resources = [local.last_pass_object_arn]
  }

  statement {
    sid       = "CheckWritesAreConditional"
    actions   = ["s3:PutObject"]
    resources = [local.write_probe_object_arn]

    condition {
      test     = "Null"
      variable = "s3:if-none-match"
      values   = ["false"]
    }
  }

  statement {
    sid       = "ReadDidList"
    actions   = ["ssm:GetParameter"]
    resources = ["arn:aws:ssm:${local.region}:${local.account_id}:parameter${local.prod_outputs.indexer_repo_dids_parameter}"]
  }
}

data "aws_iam_policy_document" "probe" {
  source_policy_documents = [data.aws_iam_policy_document.every_function.json]

  statement {
    sid       = "ReadPassRecord"
    actions   = ["s3:GetObject"]
    resources = [local.last_pass_object_arn]
  }
}

data "aws_iam_policy_document" "scheduler" {
  statement {
    sid       = "StartLiveAliases"
    actions   = ["lambda:InvokeFunction"]
    resources = [aws_lambda_alias.pass_live.arn, aws_lambda_alias.probe_live.arn]
  }
}

resource "aws_iam_role" "search" {
  name               = "openlore-indexer-search"
  assume_role_policy = data.aws_iam_policy_document.lambda_trust.json
}

resource "aws_iam_role_policy" "search" {
  name   = "openlore-indexer-search"
  role   = aws_iam_role.search.id
  policy = data.aws_iam_policy_document.search.json
}

resource "aws_iam_role" "pass" {
  name               = "openlore-indexer-pass"
  assume_role_policy = data.aws_iam_policy_document.lambda_trust.json
}

resource "aws_iam_role_policy" "pass" {
  name   = "openlore-indexer-pass"
  role   = aws_iam_role.pass.id
  policy = data.aws_iam_policy_document.pass.json
}

resource "aws_iam_role" "probe" {
  name               = "openlore-indexer-probe"
  assume_role_policy = data.aws_iam_policy_document.lambda_trust.json
}

resource "aws_iam_role_policy" "probe" {
  name   = "openlore-indexer-probe"
  role   = aws_iam_role.probe.id
  policy = data.aws_iam_policy_document.probe.json
}

resource "aws_iam_role" "scheduler" {
  name               = "openlore-indexer-scheduler"
  assume_role_policy = data.aws_iam_policy_document.scheduler_trust.json
}

resource "aws_iam_role_policy" "scheduler" {
  name   = "openlore-indexer-scheduler"
  role   = aws_iam_role.scheduler.id
  policy = data.aws_iam_policy_document.scheduler.json
}
