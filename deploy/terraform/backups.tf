# The backup bucket (RR-0014 Q4). Terraform owns it and its retention; the
# access key the droplet uploads with is created by hand in the DigitalOcean
# console, scoped to this bucket, and never passes through Terraform
# (RR-0014 Q3, constraint 1).
#
# Prefixes written by deploy/backup/kauppa-backup:
#   6h/          every 6 hours                      kept 14 days
#   daily/       the 03:00 UTC run                  kept 90 days
#   monthly/     the first daily run of each month  kept 12 months
#   pre-update/  on demand, before every update     kept 90 days
#
# Versioning is on: an overwritten or deleted object stays as a noncurrent
# version for 30 days, so a key that can delete objects can't destroy
# history at once.

locals {
  backup_retention_days = {
    "6h"         = 14
    "daily"      = 90
    "monthly"    = 365
    "pre-update" = 90
  }
}

resource "digitalocean_spaces_bucket" "backups" {
  name          = var.backup_bucket_name
  region        = var.spaces_region
  acl           = "private"
  force_destroy = false

  versioning {
    enabled = true
  }

  dynamic "lifecycle_rule" {
    for_each = local.backup_retention_days
    content {
      id      = "expire-${lifecycle_rule.key}"
      prefix  = "${lifecycle_rule.key}/"
      enabled = true

      expiration {
        days = lifecycle_rule.value
      }
    }
  }

  lifecycle_rule {
    id                                     = "noncurrent-versions"
    enabled                                = true
    abort_incomplete_multipart_upload_days = 1

    noncurrent_version_expiration {
      days = 30
    }
  }

  # RR-0014 constraint 8: a plan that destroys or replaces the bucket fails.
  lifecycle {
    prevent_destroy = true
  }
}
