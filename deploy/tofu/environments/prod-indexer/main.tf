# The scale-to-zero indexer's prod resources (scale-to-zero-hosting, feature-delta.md § Infrastructure).
# Applied from a laptop against a saved plan that deploy/check-plan.sh has passed. Nothing here
# touches the host or the PDS module.

locals {
  descriptor = jsondecode(file("${path.module}/../../../environments/prod.json"))
  region     = local.descriptor.aws_region
  account_id = "091153021562"

  # The settings no resource decides, shared with the CI lambda-smoke so it runs the real posture.
  lambda_env = jsondecode(file("${path.module}/lambda-env.json"))

  index_key       = "index/index.duckdb"
  last_pass_key   = "index/last-pass.json"
  write_probe_key = "index/.write-probe"

  prod_outputs = data.terraform_remote_state.prod.outputs
}

provider "aws" {
  region  = local.region
  profile = var.aws_profile

  default_tags {
    tags = {
      Project     = "openlore"
      Environment = local.descriptor.environment
      ManagedBy   = "opentofu"
      Component   = "openlore-indexer-lambda"
    }
  }
}

# The prod root owns these values and they will move at decommission, so remote state is the
# honest reference until then.
data "terraform_remote_state" "prod" {
  backend = "s3"

  config = {
    bucket  = "jeffbaileyterraformstate"
    key     = "openlore/pds/prod.tfstate"
    region  = "us-west-2"
    profile = var.aws_profile
  }
}
