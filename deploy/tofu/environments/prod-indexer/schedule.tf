# Both start DISABLED: until the first flip live is unchecked $LATEST code and a pass could publish
# from it (D-STZ-26). Each sends exactly {}, because a richer event is refused (D-STZ-44).

resource "aws_scheduler_schedule_group" "indexer" {
  name = "openlore-indexer"
}

resource "aws_scheduler_schedule" "pass" {
  name                         = "openlore-indexer-pass"
  group_name                   = aws_scheduler_schedule_group.indexer.name
  state                        = var.schedules_enabled ? "ENABLED" : "DISABLED"
  schedule_expression          = "cron(0/15 * * * ? *)"
  schedule_expression_timezone = "UTC"

  flexible_time_window {
    mode = "OFF"
  }

  target {
    arn      = aws_lambda_alias.pass_live.arn
    role_arn = aws_iam_role.scheduler.arn
    input    = "{}"

    retry_policy {
      maximum_retry_attempts = 0
    }
  }
}

# Minute 2 of each 5-minute block, so each of A3's 300 s periods gets exactly one line; a rate
# schedule would drift against the period boundaries.
resource "aws_scheduler_schedule" "probe" {
  name                         = "openlore-indexer-probe"
  group_name                   = aws_scheduler_schedule_group.indexer.name
  state                        = var.schedules_enabled ? "ENABLED" : "DISABLED"
  schedule_expression          = "cron(2/5 * * * ? *)"
  schedule_expression_timezone = "UTC"

  flexible_time_window {
    mode = "OFF"
  }

  target {
    arn      = aws_lambda_alias.probe_live.arn
    role_arn = aws_iam_role.scheduler.arn
    input    = "{}"

    retry_policy {
      maximum_retry_attempts = 0
    }
  }
}
