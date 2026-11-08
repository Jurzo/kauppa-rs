# Inputs. None of them is a secret (RR-0014 constraint 1): credentials come
# from environment variables (versions.tf), and the app's secrets are created
# on the droplet. scripts/deploy/check_terraform.sh fails if a variable named
# like a token, secret or key is added without `sensitive = true` or with a
# default; `ssh_public_key` is the one listed exception (a public key).

variable "name" {
  description = "Name prefix for every resource (droplet, volume, firewall, SSH key)."
  type        = string
  default     = "kauppa"

  validation {
    condition     = can(regex("^[a-z][a-z0-9-]{1,30}$", var.name))
    error_message = "name must be lowercase letters, digits and hyphens, 2-31 characters, starting with a letter."
  }
}

variable "region" {
  description = "Droplet, volume and reserved IP region (RR-0014 Q6)."
  type        = string
  default     = "fra1"
}

variable "droplet_size" {
  description = "Droplet size slug. s-1vcpu-2gb is the documented step up (RR-0014 Q6)."
  type        = string
  default     = "s-1vcpu-1gb"
}

variable "droplet_image" {
  description = "Droplet base image slug. cloud-init.yaml.tftpl assumes Ubuntu (apt, docker.io, docker-compose-v2)."
  type        = string
  default     = "ubuntu-24-04-x64"
}

variable "volume_size_gib" {
  description = "Size of the data volume in GiB. It can grow later; it can never shrink."
  type        = number
  default     = 5

  validation {
    condition     = var.volume_size_gib >= 1 && floor(var.volume_size_gib) == var.volume_size_gib
    error_message = "volume_size_gib must be a whole number >= 1."
  }
}

variable "spaces_region" {
  description = "Region of the backup bucket (RR-0014 Q6). Keep it different from the droplet's region."
  type        = string
  default     = "ams3"
}

variable "backup_bucket_name" {
  description = "Name of the backup bucket. Spaces bucket names are unique per region across all customers."
  type        = string

  validation {
    condition     = can(regex("^[a-z0-9][a-z0-9-]{1,61}[a-z0-9]$", var.backup_bucket_name))
    error_message = "backup_bucket_name must be 3-63 lowercase letters, digits and hyphens."
  }
}

variable "ssh_public_key" {
  description = "The operator's SSH public key (e.g. the contents of ~/.ssh/id_ed25519.pub). Public, so not sensitive."
  type        = string

  validation {
    condition     = can(regex("^(ssh-ed25519|ssh-rsa|ecdsa-sha2-nistp(256|384|521)|sk-ssh-ed25519@openssh.com) ", var.ssh_public_key))
    error_message = "ssh_public_key must be an OpenSSH public key line (ssh-ed25519 ..., ssh-rsa ..., ecdsa-sha2-...)."
  }
}

variable "ssh_allowed_cidrs" {
  description = "Source CIDRs allowed to reach SSH (22). Never 0.0.0.0/0 or ::/0 (MB-0022 forbidden)."
  type        = list(string)

  validation {
    condition     = length(var.ssh_allowed_cidrs) > 0
    error_message = "ssh_allowed_cidrs must name at least one operator CIDR."
  }

  validation {
    condition     = alltrue([for c in var.ssh_allowed_cidrs : can(cidrhost(c, 0))])
    error_message = "every ssh_allowed_cidrs entry must be a CIDR, e.g. 203.0.113.7/32."
  }

  validation {
    condition = alltrue([
      for c in var.ssh_allowed_cidrs : can(cidrhost(c, 0)) && tonumber(element(split("/", c), 1)) >= 8
    ])
    error_message = "ssh_allowed_cidrs may not open SSH to the internet: no 0.0.0.0/0, ::/0 or any prefix shorter than /8."
  }
}

variable "enable_ipv6" {
  description = "Give the droplet a public IPv6 address. It changes when the droplet is replaced (the reserved IP is IPv4 only), so an AAAA record must then be updated by hand."
  type        = bool
  default     = false
}
