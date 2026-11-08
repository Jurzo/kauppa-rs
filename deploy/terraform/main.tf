# The host: one droplet, its data volume, a reserved IP and a cloud firewall
# (MB-0022 P4). DNS is not managed here (RR-0014 Q5): the owner adds the A
# record by hand, after MB-0023, MB-0025 and MB-0026 have merged.

locals {
  anywhere = ["0.0.0.0/0", "::/0"]

  # Must match cloud-init.yaml.tftpl and deploy/README.md: the volume is
  # mounted at /mnt/<volume name>.
  volume_name  = "${var.name}-data"
  volume_mount = "/mnt/${local.volume_name}"
}

resource "digitalocean_ssh_key" "operator" {
  name       = "${var.name}-operator"
  public_key = var.ssh_public_key
}

# The database, the app's secrets and nothing else live here. It is its own
# resource, so replacing the droplet keeps it, and prevent_destroy makes any
# plan that would destroy or replace it fail (RR-0014 constraint 8).
resource "digitalocean_volume" "data" {
  name                    = local.volume_name
  region                  = var.region
  size                    = var.volume_size_gib
  initial_filesystem_type = "ext4"
  description             = "kauppa-rs data: SQLite database and secrets/ (MB-0022)"

  lifecycle {
    prevent_destroy = true
  }
}

resource "digitalocean_droplet" "app" {
  name       = "${var.name}-app"
  region     = var.region
  size       = var.droplet_size
  image      = var.droplet_image
  ipv6       = var.enable_ipv6
  monitoring = true
  ssh_keys   = [digitalocean_ssh_key.operator.fingerprint]

  # Attached at creation, so the device exists when cloud-init runs. When the
  # droplet is replaced, the volume is detached and re-attached to the new one.
  volume_ids = [digitalocean_volume.data.id]

  # No secret in user_data (RR-0014 constraint 1): it is readable through the
  # droplet metadata service and kept in plan files.
  user_data = templatefile("${path.module}/cloud-init.yaml.tftpl", {
    volume_name  = local.volume_name
    volume_mount = local.volume_mount
    app_uid      = 10001
  })

  graceful_shutdown = true
}

resource "digitalocean_reserved_ip" "app" {
  region = var.region
}

resource "digitalocean_reserved_ip_assignment" "app" {
  ip_address = digitalocean_reserved_ip.app.ip_address
  droplet_id = digitalocean_droplet.app.id
}

# Cloud firewalls filter before traffic reaches the droplet; Docker's
# published ports bypass a host firewall such as ufw, so this is the one that
# counts. Inbound: HTTP and HTTPS from anywhere, SSH only from the operator's
# CIDRs. scripts/deploy/check_terraform.sh asserts this shape.
resource "digitalocean_firewall" "app" {
  name        = "${var.name}-app"
  droplet_ids = [digitalocean_droplet.app.id]

  inbound_rule {
    protocol         = "tcp"
    port_range       = "80"
    source_addresses = local.anywhere
  }

  inbound_rule {
    protocol         = "tcp"
    port_range       = "443"
    source_addresses = local.anywhere
  }

  inbound_rule {
    protocol         = "tcp"
    port_range       = "22"
    source_addresses = var.ssh_allowed_cidrs
  }

  # Outbound: apt, image pulls from GHCR and Docker Hub, ACME (Let's Encrypt)
  # and the backup uploads to Spaces.
  outbound_rule {
    protocol              = "tcp"
    port_range            = "1-65535"
    destination_addresses = local.anywhere
  }

  outbound_rule {
    protocol              = "udp"
    port_range            = "1-65535"
    destination_addresses = local.anywhere
  }

  outbound_rule {
    protocol              = "icmp"
    destination_addresses = local.anywhere
  }
}
