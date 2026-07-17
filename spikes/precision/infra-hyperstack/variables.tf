variable "offline_plan" {
  description = "Disable every Hyperstack API lookup and paid resource. Keep true for credential-free validation."
  type        = bool
  default     = true
}

variable "enable_discovery" {
  description = "Read account-visible regions, environments, keypairs, flavors, and images without creating resources. Requires offline_plan=false."
  type        = bool
  default     = false
}

variable "create_instance" {
  description = "Create the paid GPU VM. Defaults false and must not be enabled without explicit approval of a saved plan."
  type        = bool
  default     = false
}

variable "accept_paid_creation" {
  description = "Explicit acknowledgement that the selected Hyperstack VM and attached public IP incur charges until destroyed."
  type        = bool
  default     = false
}

variable "region_name" {
  description = "Exact account-visible Hyperstack region name selected by authenticated discovery."
  type        = string
  default     = "replace-after-discovery"

  validation {
    condition     = can(regex("^[A-Za-z0-9][A-Za-z0-9._-]{1,63}$", var.region_name))
    error_message = "region_name must be a non-empty Hyperstack region identifier containing only letters, numbers, dot, underscore, or hyphen."
  }
}

variable "environment_name" {
  description = "Existing Hyperstack environment in region_name."
  type        = string
  default     = "replace-after-discovery"

  validation {
    condition     = length(trimspace(var.environment_name)) >= 2 && length(var.environment_name) <= 50
    error_message = "environment_name must contain 2 to 50 characters."
  }
}

variable "flavor_name" {
  description = "Exact one-GPU full-rate flavor name selected from live discovery."
  type        = string
  default     = "replace-after-discovery"

  validation {
    condition     = length(trimspace(var.flavor_name)) >= 2 && length(var.flavor_name) <= 128
    error_message = "flavor_name must contain 2 to 128 characters."
  }
}

variable "image_name" {
  description = "Exact region-compatible Ubuntu CUDA image name selected from live discovery."
  type        = string
  default     = "replace-after-discovery"

  validation {
    condition     = length(trimspace(var.image_name)) >= 2 && length(var.image_name) <= 256
    error_message = "image_name must contain 2 to 256 characters."
  }
}

variable "key_name" {
  description = "Existing Hyperstack SSH keypair name in environment_name."
  type        = string
  default     = "sembla-hyperstack"

  validation {
    condition     = length(trimspace(var.key_name)) >= 2 && length(var.key_name) <= 50
    error_message = "key_name must contain 2 to 50 characters."
  }
}

variable "ssh_cidr" {
  description = "Operator public IPv4 as one canonical /32. Broad CIDRs are rejected. Refresh immediately before a paid plan."
  type        = string
  default     = "198.51.100.10/32"

  validation {
    condition = can(cidrhost(var.ssh_cidr, 0)) && can(regex("^([0-9]{1,3}\\.){3}[0-9]{1,3}/32$", var.ssh_cidr)) ? (
      split("/", var.ssh_cidr)[1] == "32" &&
      split("/", var.ssh_cidr)[0] == cidrhost(var.ssh_cidr, 0)
    ) : false
    error_message = "ssh_cidr must be one canonical IPv4 host route such as 198.51.100.10/32."
  }
}

variable "ssh_user" {
  description = "SSH user supplied by the selected Ubuntu image."
  type        = string
  default     = "ubuntu"

  validation {
    condition     = can(regex("^[a-z_][a-z0-9_-]{0,31}$", var.ssh_user))
    error_message = "ssh_user must be a valid Linux account name."
  }
}

variable "expected_gpu_model" {
  description = "GPU model substring expected from both the live flavor and nvidia-smi, for example A100."
  type        = string
  default     = "A100"

  validation {
    condition     = contains(["A100", "H100", "H200", "GH200"], upper(var.expected_gpu_model))
    error_message = "expected_gpu_model must be A100, H100, H200, or GH200. B200 is blocked until the runtime classifier supports it."
  }
}

variable "expected_gpu_count" {
  description = "Exact GPU count required by the decision run. This module intentionally permits only one GPU."
  type        = number
  default     = 1

  validation {
    condition     = var.expected_gpu_count == 1
    error_message = "expected_gpu_count must remain 1; multi-GPU hosts are intentionally rejected."
  }
}

variable "expected_hourly_price_usd" {
  description = "Reviewed all-in hourly VM estimate from live Hyperstack discovery/pricebook. Terraform cannot derive this from the flavor data source."
  type        = number
  default     = 0

  validation {
    condition     = var.expected_hourly_price_usd >= 0
    error_message = "expected_hourly_price_usd cannot be negative."
  }
}

variable "max_hourly_price_usd" {
  description = "Hard paid-plan cap for the reviewed hourly estimate."
  type        = number
  default     = 5

  validation {
    condition     = var.max_hourly_price_usd > 0 && var.max_hourly_price_usd <= 10
    error_message = "max_hourly_price_usd must be greater than 0 and no more than 10."
  }
}

variable "repository_url" {
  description = "Public Git repository cloned by cloud-init."
  type        = string
  default     = "https://github.com/ianmoran11/sembla.git"

  validation {
    condition     = can(regex("^https://github\\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+\\.git$", var.repository_url))
    error_message = "repository_url must be an HTTPS GitHub clone URL ending in .git."
  }
}

variable "repository_ref" {
  description = "Exact 40-hex commit checked out for all decision runs. Replace the all-zero offline placeholder before paid creation."
  type        = string
  default     = "0000000000000000000000000000000000000000"

  validation {
    condition     = can(regex("^[0-9a-f]{40}$", var.repository_ref))
    error_message = "repository_ref must be an exact lowercase 40-hex Git commit."
  }
}

variable "emergency_poweroff_hours" {
  description = "Guest poweroff timer installed before network bootstrap. Hyperstack SHUTOFF still bills; only terraform destroy stops all VM charges."
  type        = number
  default     = 4

  validation {
    condition     = var.emergency_poweroff_hours >= 1 && var.emergency_poweroff_hours <= 8 && floor(var.emergency_poweroff_hours) == var.emergency_poweroff_hours
    error_message = "emergency_poweroff_hours must be a whole number from 1 through 8."
  }
}

variable "name_prefix" {
  description = "Prefix for the throwaway VM name and labels."
  type        = string
  default     = "sembla-precision"

  validation {
    condition     = can(regex("^[a-z][a-z0-9-]{2,39}$", var.name_prefix))
    error_message = "name_prefix must be 3 to 40 lowercase letters, numbers, or hyphens and start with a letter."
  }
}
