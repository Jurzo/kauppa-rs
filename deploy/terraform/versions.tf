# Terraform and provider versions, and the state backend (RR-0014 Q2).
#
# State lives in a DigitalOcean Spaces bucket through the S3 backend, locked
# with a lock object (`use_lockfile`, Terraform >= 1.10; no DynamoDB). The
# bucket is created by hand, with versioning on, before `terraform init`
# (deploy/README.md), and its name is passed at init time:
#
#   terraform init -backend-config="bucket=<state-bucket>"
#
# Credentials never go in this block or in -backend-config: the backend reads
# AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY from the operator's shell (a
# limited-access Spaces key scoped to the state bucket).

terraform {
  required_version = ">= 1.10"

  required_providers {
    digitalocean = {
      source  = "digitalocean/digitalocean"
      version = "2.107.0"
    }
  }

  backend "s3" {
    endpoints = {
      s3 = "https://ams3.digitaloceanspaces.com"
    }
    key    = "kauppa-rs/terraform.tfstate"
    region = "us-east-1" # ignored by Spaces; required by the S3 backend

    skip_credentials_validation = true
    skip_requesting_account_id  = true
    skip_metadata_api_check     = true
    skip_region_validation      = true
    skip_s3_checksum            = true

    use_lockfile = true
  }
}

# Credentials come from the operator's shell only (RR-0014 Q3):
#   DIGITALOCEAN_TOKEN                               API token
#   SPACES_ACCESS_KEY_ID / SPACES_SECRET_ACCESS_KEY  Spaces key used to manage
#                                                    the backup bucket
provider "digitalocean" {}
