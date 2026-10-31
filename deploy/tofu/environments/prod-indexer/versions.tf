terraform {
  required_version = ">= 1.10.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 6.0"
    }
  }

  # Same bucket as the prod root, under the indexer's own key: an apply here can never plan
  # against the PDS module, which can replace the instance (D-STZ-28).
  backend "s3" {
    bucket       = "jeffbaileyterraformstate"
    key          = "openlore/indexer/prod.tfstate"
    region       = "us-west-2"
    encrypt      = true
    use_lockfile = true
  }
}
