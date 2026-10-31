# One budget, filtered on the Component tag, because the account already has one and an unfiltered
# $2 would fire on the PDS host's spend. Budgets data lags hours, so this is a backstop.

# Marked sensitive; it lands in the encrypted state, as the PDS root's values do.
data "aws_ssm_parameter" "contact" {
  name = "/openlore/prod/acme-contact-email"
}

resource "aws_budgets_budget" "indexer" {
  name         = "openlore-indexer-lambda"
  budget_type  = "COST"
  time_unit    = "MONTHLY"
  limit_amount = "10"
  limit_unit   = "USD"

  cost_filter {
    name   = "TagKeyValue"
    values = ["user:Component$openlore-indexer-lambda"]
  }

  # 20 % of the $10 limit is the $2 target.
  notification {
    notification_type          = "ACTUAL"
    comparison_operator        = "GREATER_THAN"
    threshold                  = 20
    threshold_type             = "PERCENTAGE"
    subscriber_email_addresses = [data.aws_ssm_parameter.contact.value]
  }

  notification {
    notification_type          = "ACTUAL"
    comparison_operator        = "GREATER_THAN"
    threshold                  = 100
    threshold_type             = "PERCENTAGE"
    subscriber_email_addresses = [data.aws_ssm_parameter.contact.value]
  }
}

# AWS refuses this until the tag has appeared in billing data, up to 24 h after the first tagged
# resource exists, so it waits for the bootstrap's last apply. The budget reads $0 until then.
resource "aws_ce_cost_allocation_tag" "component" {
  count   = var.cost_allocation_tag_active ? 1 : 0
  tag_key = "Component"
  status  = "Active"
}
