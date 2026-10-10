# OpenLore's prod PDS (openlore.jeffbailey.us), a thin root over the shared module (ADR-066).
# One environment only (ADR-068): t4g.micro, 5 GB data volume, 1 GiB swap, EIP. Applied from a
# laptop against a saved plan that deploy/check-plan.sh has passed.

terraform {
  required_version = ">= 1.10.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }

  # Same bucket as the bootstrap; the key matches the descriptor's tofu_state_key.
  backend "s3" {
    bucket       = "jeffbaileyterraformstate"
    key          = "openlore/pds/prod.tfstate"
    region       = "us-west-2"
    encrypt      = true
    use_lockfile = true
  }
}

locals {
  descriptor = jsondecode(file("${path.module}/../../../environments/prod.json"))
}

provider "aws" {
  region  = local.descriptor.aws_region
  profile = var.aws_profile

  default_tags {
    tags = {
      Project     = "openlore"
      Environment = local.descriptor.environment
      ManagedBy   = "opentofu"
    }
  }
}

module "pds" {
  source = "git::https://github.com/jeffabailey/tofu-aws-pds.git//modules/pds?ref=v1.7.0"

  name_prefix = "openlore"
  project     = "openlore"
  # org.openlore is a product namespace, not the reverse of openlore.jeffbailey.us, so the
  # namespace check stays off (the default). The handle-under-hostname check is always on.

  descriptor            = local.descriptor
  hosted_zone_id        = var.hosted_zone_id
  instance_profile_name = var.instance_profile_name
  backup_bucket         = var.backup_bucket
  ami_id                = var.ami_id
  ssh_key_name          = var.ssh_key_name
  ssh_ingress_cidr      = var.ssh_ingress_cidr

  swap_mb = 1024

  # First boot creates jeff.openlore.jeffbailey.us and a CLI app password; the passwords land
  # in SSM /openlore/prod/{account-password,cli-app-password} (see outputs).
  bootstrap_account = true

  # The account's claim-signing key, from `openlore key` (OPENLORE_DID=did:plc:pnyx...). Published
  # into the DID document so peers can verify claims this identity signs. The private key stays
  # in the operator's OS keychain; only this public did:key is here.
  verification_methods = {
    "org.openlore.application" = "did:key:z6MkpwHtDxopasFQ89TVijaSDqyTUvp4auQARnJgQj5LbQgR"
  }

  # Encrypted identity backup to the backup bucket every day (v1.5.0; key: deploy/README.md §5).
  backup_on_calendar = "daily"

  # Alarm (SNS, subscribed out of band) after two UTC days with no successful backup (v1.6.0).
  backup_alarm = true
}
