variable "aws_region" {
  description = "AWS region in which to create the throwaway GPU VM."
  type        = string
  default     = "us-east-1"

  validation {
    condition     = can(regex("^[a-z]{2}(-[a-z]+)+-[0-9]+$", var.aws_region))
    error_message = "aws_region must look like an AWS region, for example us-east-1."
  }
}

variable "availability_zone" {
  description = "Optional availability zone. Leave null to let AWS choose; set a GPU-capable zone if capacity is constrained."
  type        = string
  default     = null
  nullable    = true

  validation {
    condition     = var.availability_zone == null || can(regex("^[a-z]{2}(-[a-z]+)+-[0-9]+[a-z]$", var.availability_zone))
    error_message = "availability_zone must be null or look like us-east-1a."
  }
}

variable "gpu_class" {
  description = "GPU throughput class: commodity selects a T4/rate-limited instance; full_rate selects an A100/full-rate-f64 instance."
  type        = string
  default     = "commodity"

  validation {
    condition     = contains(["commodity", "full_rate"], var.gpu_class)
    error_message = "gpu_class must be either commodity or full_rate."
  }
}

variable "ami_id" {
  description = "Immutable regional AMI ID. The default pins Amazon's x86_64 Ubuntu 22.04 Base OSS NVIDIA Driver DLAMI 20260714 in us-east-1; override it when changing regions."
  type        = string
  default     = "ami-072e487908654a0d2"

  validation {
    condition     = can(regex("^ami-[0-9a-f]{17}$", var.ami_id))
    error_message = "ami_id must be an immutable 17-hex-digit AWS AMI ID; mutable resolve:ssm selectors are not accepted."
  }
}

variable "key_name" {
  description = "Name of an existing EC2 key pair used for SSH."
  type        = string
  nullable    = false

  validation {
    condition     = length(trimspace(var.key_name)) > 0
    error_message = "key_name must name an existing EC2 key pair."
  }
}

variable "ssh_cidr" {
  description = "Single trusted IPv4 CIDR allowed to SSH. Public-anywhere ingress is forbidden."
  type        = string
  nullable    = false

  validation {
    condition = (
      can(cidrnetmask(var.ssh_cidr)) &&
      can(regex("^[0-9.]+/[0-9]+$", var.ssh_cidr)) &&
      try(tonumber(split("/", var.ssh_cidr)[1]) > 0, false)
    )
    error_message = "ssh_cidr must be a valid restricted IPv4 CIDR; every spelling of a /0 network is forbidden."
  }
}

variable "offline_plan" {
  description = "Use mock provider credentials and suppress AWS API checks so plan works without credentials. Set false for apply and destroy."
  type        = bool
  default     = true
}

variable "auto_stop_enabled" {
  description = "Install an in-guest systemd timer that powers off (stops) the instance after auto_stop_hours."
  type        = bool
  default     = true
}

variable "auto_stop_hours" {
  description = "Hours after each boot before the safety timer stops the instance."
  type        = number
  default     = 4

  validation {
    condition     = var.auto_stop_hours >= 1 && var.auto_stop_hours <= 24 && floor(var.auto_stop_hours) == var.auto_stop_hours
    error_message = "auto_stop_hours must be a whole number from 1 through 24."
  }
}

variable "use_spot" {
  description = "Request Spot capacity instead of On-Demand. Spot can be interrupted and RESULTS.md can be lost before retrieval."
  type        = bool
  default     = false
}

variable "spot_max_price" {
  description = "Optional maximum hourly Spot price as a string; null uses the current Spot price."
  type        = string
  default     = null
  nullable    = true

  validation {
    condition     = var.spot_max_price == null || can(regex("^[0-9]+(\\.[0-9]+)?$", var.spot_max_price))
    error_message = "spot_max_price must be null or a non-negative decimal string."
  }
}

variable "repository_url" {
  description = "Public HTTPS Git repository cloned by cloud-init."
  type        = string
  default     = "https://github.com/ianmoran11/sembla.git"

  validation {
    condition     = can(regex("^https://[^[:space:]']+$", var.repository_url))
    error_message = "repository_url must be a public HTTPS URL without whitespace or single quotes."
  }
}

variable "repository_ref" {
  description = "Branch, tag, or commit fetched and checked out by cloud-init."
  type        = string
  default     = "main"

  validation {
    condition     = can(regex("^[0-9A-Za-z._/-]+$", var.repository_ref))
    error_message = "repository_ref may contain only letters, digits, dot, underscore, slash, and hyphen."
  }
}

variable "root_volume_gb" {
  description = "Encrypted gp3 root volume size."
  type        = number
  default     = 100

  validation {
    condition     = var.root_volume_gb >= 50 && var.root_volume_gb <= 500 && floor(var.root_volume_gb) == var.root_volume_gb
    error_message = "root_volume_gb must be a whole number from 50 through 500."
  }
}

variable "name_prefix" {
  description = "Prefix used in AWS resource names and tags."
  type        = string
  default     = "sembla-precision"

  validation {
    condition     = can(regex("^[0-9A-Za-z-]{1,40}$", var.name_prefix))
    error_message = "name_prefix must contain 1-40 letters, digits, or hyphens."
  }
}
