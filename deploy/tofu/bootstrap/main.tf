# OpenLore PDS bootstrap: the per-account foundation in the `jeff` account (091153021562).
# Applied once, by a human, from a laptop, BEFORE the prod environment (deploy/README.md).
#
# What it creates (ADR-067): the identity backup bucket, the prod host role + instance profile,
# and the region's default VPC (the account has none; modules/pds looks one up). What it does
# NOT create: a state bucket (jeffbaileyterraformstate is reused, D8), a GitHub OIDC provider or
# CI roles (laptop apply only, D6/ADR-068). Flipping enable_ci_roles later adds CI apply.

terraform {
  required_version = ">= 1.10.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }

  # Existing bucket in us-west-2, shared with other projects; this key is OpenLore's alone.
  # Prerequisite: versioning enabled on the bucket (deploy/README.md, step 0).
  backend "s3" {
    bucket       = "jeffbaileyterraformstate"
    key          = "openlore/pds/bootstrap.tfstate"
    region       = "us-west-2"
    encrypt      = true
    use_lockfile = true
  }
}

locals {
  # The descriptor is the single source of every name. The module reads no files, so the root
  # decodes it and passes the object in.
  environments = {
    prod = jsondecode(file("${path.module}/../../environments/prod.json"))
  }
}

provider "aws" {
  region  = local.environments.prod.aws_region
  profile = var.aws_profile

  default_tags {
    tags = {
      Project   = "openlore"
      ManagedBy = "opentofu"
      Module    = "bootstrap"
    }
  }
}

module "pds_bootstrap" {
  source = "git::https://github.com/jeffabailey/tofu-aws-pds.git//modules/pds-bootstrap?ref=v1.7.0"

  name_prefix         = "openlore"
  project             = "openlore"
  aws_region          = local.environments.prod.aws_region
  expected_account_id = "091153021562"

  hosted_zone_id  = var.hosted_zone_id
  dns_record_name = local.environments.prod.pds_hostname
  environments    = local.environments

  create_state_bucket  = false
  state_bucket_name    = "jeffbaileyterraformstate"
  create_oidc_provider = false
  enable_ci_roles      = false
  create_default_vpc   = true

  # The prod host creates its first account and stores the passwords in SSM (v1.2.0).
  bootstrap_account = true

  # The host reports each successful backup as a PDS/Backup metric for the backup alarm (v1.6.0).
  backup_metrics = true
}
