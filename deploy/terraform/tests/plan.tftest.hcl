# Plan-only tests with a mocked provider: no credentials, no API calls.
# Run by scripts/deploy/check_terraform.sh (`terraform test`).

mock_provider "digitalocean" {}

variables {
  backup_bucket_name = "kauppa-backups-test"
  ssh_public_key     = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIKauppaTestKeyNotARealKeyAAAAAAAAAAAAAAAAAA test@example"
  ssh_allowed_cidrs  = ["198.51.100.7/32", "2001:db8::/64"]
}

run "firewall_inbound_rules" {
  command = plan

  assert {
    condition     = length(digitalocean_firewall.app.inbound_rule) == 3
    error_message = "the firewall must have exactly three inbound rules (80, 443, 22)"
  }

  assert {
    condition = alltrue([
      for r in digitalocean_firewall.app.inbound_rule : contains(["22", "80", "443"], r.port_range) && r.protocol == "tcp"
    ])
    error_message = "inbound rules may open only TCP 22, 80 and 443"
  }

  assert {
    condition = alltrue([
      for r in digitalocean_firewall.app.inbound_rule :
      toset(r.source_addresses) == toset(["198.51.100.7/32", "2001:db8::/64"]) if r.port_range == "22"
    ])
    error_message = "SSH must be open only to ssh_allowed_cidrs"
  }

  assert {
    condition = alltrue([
      for r in digitalocean_firewall.app.inbound_rule :
      toset(r.source_addresses) == toset(["0.0.0.0/0", "::/0"]) if contains(["80", "443"], r.port_range)
    ])
    error_message = "HTTP and HTTPS must be open from anywhere"
  }
}

run "defaults" {
  command = plan

  assert {
    condition     = digitalocean_droplet.app.region == "fra1" && digitalocean_droplet.app.size == "s-1vcpu-1gb"
    error_message = "default droplet must be s-1vcpu-1gb in fra1 (RR-0014 Q6)"
  }

  assert {
    condition     = digitalocean_volume.data.size == 5 && digitalocean_volume.data.region == "fra1"
    error_message = "default volume must be 5 GiB, in the droplet's region"
  }

  assert {
    condition     = digitalocean_spaces_bucket.backups.region == "ams3" && digitalocean_spaces_bucket.backups.acl == "private"
    error_message = "the backup bucket must be private, in ams3 (RR-0014 Q6)"
  }

  assert {
    condition     = digitalocean_spaces_bucket.backups.force_destroy == false
    error_message = "the backup bucket must not be force-destroyable"
  }

  assert {
    condition     = one(digitalocean_spaces_bucket.backups.versioning).enabled
    error_message = "backup bucket versioning must be on (RR-0014 Q4)"
  }

  assert {
    condition = (
      { for r in digitalocean_spaces_bucket.backups.lifecycle_rule : r.prefix => one(r.expiration).days if r.prefix != "" && r.prefix != null }
      == { "6h/" = 14, "daily/" = 90, "monthly/" = 365, "pre-update/" = 90 }
    )
    error_message = "backup retention must be 6h/ 14 d, daily/ 90 d, monthly/ 365 d, pre-update/ 90 d (RR-0014 Q4)"
  }

  assert {
    condition = anytrue([
      for r in digitalocean_spaces_bucket.backups.lifecycle_rule :
      length(r.noncurrent_version_expiration) == 1 && one(r.noncurrent_version_expiration).days == 30
    ])
    error_message = "noncurrent backup versions must expire after 30 days (RR-0014 Q4)"
  }

  assert {
    condition     = length(digitalocean_droplet.app.volume_ids) == 1
    error_message = "the data volume must be attached to the droplet"
  }
}

run "rejects_ssh_from_anywhere_v4" {
  command = plan

  variables {
    ssh_allowed_cidrs = ["0.0.0.0/0"]
  }

  expect_failures = [var.ssh_allowed_cidrs]
}

run "rejects_ssh_from_anywhere_v6" {
  command = plan

  variables {
    ssh_allowed_cidrs = ["198.51.100.7/32", "::/0"]
  }

  expect_failures = [var.ssh_allowed_cidrs]
}

run "rejects_wide_ssh_prefix" {
  command = plan

  variables {
    ssh_allowed_cidrs = ["128.0.0.0/1"]
  }

  expect_failures = [var.ssh_allowed_cidrs]
}

run "rejects_empty_ssh_cidrs" {
  command = plan

  variables {
    ssh_allowed_cidrs = []
  }

  expect_failures = [var.ssh_allowed_cidrs]
}
