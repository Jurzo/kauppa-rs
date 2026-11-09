output "reserved_ip" {
  description = "Public IPv4 address. Point the domain's A record here (by hand; RR-0014 Q5)."
  value       = digitalocean_reserved_ip.app.ip_address
}

output "droplet_ipv6" {
  description = "The droplet's IPv6 address when enable_ipv6 is true. It changes when the droplet is replaced."
  value       = var.enable_ipv6 ? digitalocean_droplet.app.ipv6_address : null
}

output "volume_mount" {
  description = "Where cloud-init mounts the data volume on the droplet (KAUPPA_DATA_ROOT in deploy/.env)."
  value       = local.volume_mount
}

output "backup_bucket" {
  description = "Backup bucket name and region (KAUPPA_BACKUP_BUCKET and KAUPPA_BACKUP_ENDPOINT in deploy/.env)."
  value = {
    name     = digitalocean_spaces_bucket.backups.name
    region   = digitalocean_spaces_bucket.backups.region
    endpoint = "${digitalocean_spaces_bucket.backups.region}.digitaloceanspaces.com"
  }
}

output "ssh" {
  description = "SSH command for the operator."
  value       = "ssh root@${digitalocean_reserved_ip.app.ip_address}"
}
