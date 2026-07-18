variable "offline_plan" {
  description = "Disable every Vultr API lookup and resource so init/validate/plan can run with an obvious placeholder key. Set false only with VULTR_API_KEY exported."
  type        = bool
  default     = true
}

variable "enable_discovery" {
  description = "Read the selected plan from Vultr without creating resources. Requires offline_plan=false and VULTR_API_KEY."
  type        = bool
  default     = false
}

variable "create_instance" {
  description = "Create the paid GPU resource. Defaults false and must never be enabled without an explicit cost review."
  type        = bool
  default     = false
}

variable "deployment_kind" {
  description = "Vultr resource family: cloud_gpu uses vultr_instance; bare_metal uses vultr_bare_metal_server."
  type        = string
  default     = "cloud_gpu"

  validation {
    condition     = contains(["cloud_gpu", "bare_metal"], var.deployment_kind)
    error_message = "deployment_kind must be cloud_gpu or bare_metal."
  }
}

variable "accept_bare_metal_bootstrap_exposure" {
  description = "Explicitly accept that bare metal has no provider firewall attachment and may expose SSH-key-only port 22 briefly before cloud-init installs the source-CIDR filter."
  type        = bool
  default     = false
}

variable "region_id" {
  description = "Vultr region ID, such as ewr, fra, or atl. It must occur in the selected plan's live locations list."
  type        = string
  default     = "ewr"

  validation {
    condition     = can(regex("^[a-z0-9-]+$", var.region_id))
    error_message = "region_id may contain lowercase letters, digits, and hyphens only."
  }
}

variable "plan_id" {
  description = "Account-visible Vultr GPU plan ID. Discover it live; do not infer a full-rate plan from marketing copy."
  type        = string
  default     = "vcg-replace-with-full-rate-plan"

  validation {
    condition     = can(regex("^(vcg|vbm)-[0-9A-Za-z-]+$", var.plan_id))
    error_message = "plan_id must be a Vultr Cloud GPU (vcg-...) or bare-metal (vbm-...) plan ID."
  }
}

variable "os_id" {
  description = "Vultr OS/application image ID compatible with the selected plan architecture and NVIDIA driver. Ubuntu 22.04 x64 is 1743; GH200/ARM64 requires a separately verified compatible image."
  type        = number
  default     = 1743

  validation {
    condition     = var.os_id > 0 && floor(var.os_id) == var.os_id
    error_message = "os_id must be a positive integer."
  }
}

variable "ssh_key_ids" {
  description = "Existing Vultr SSH key UUIDs applied at provisioning. At least one is required before create_instance=true."
  type        = list(string)
  default     = []

  validation {
    condition     = alltrue([for id in var.ssh_key_ids : length(trimspace(id)) > 0])
    error_message = "ssh_key_ids cannot contain empty values."
  }
}

variable "ssh_cidr" {
  description = "Single trusted public IPv4 CIDR allowed to SSH. Every /0 spelling is rejected."
  type        = string
  default     = "203.0.113.10/32"

  validation {
    condition = (
      can(cidrnetmask(var.ssh_cidr)) &&
      can(regex("^[0-9.]+/[0-9]+$", var.ssh_cidr)) &&
      try(tonumber(split("/", var.ssh_cidr)[1]) == 32, false)
    )
    error_message = "ssh_cidr must be one trusted IPv4 address expressed as /32."
  }
}

variable "expected_gpu_model" {
  description = "Expected full-rate NVIDIA model, verified again by nvidia-smi and the benchmark's runtime fp64 ratio."
  type        = string
  default     = "NVIDIA A100"

  validation {
    condition     = can(regex("(?i)(^|[^A-Z0-9])(A100|H100|H200|GH200|B200)($|[^A-Z0-9])", var.expected_gpu_model))
    error_message = "expected_gpu_model must identify a recognized full-rate NVIDIA A100/H100/H200/GH200/B200-class device."
  }
}

variable "max_hourly_price_usd" {
  description = "Hard Terraform check against selected plan monthly_cost/730. This does not cap billing after creation."
  type        = number
  default     = 5

  validation {
    condition     = var.max_hourly_price_usd > 0
    error_message = "max_hourly_price_usd must be positive."
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
  description = "Pushed branch, tag, or commit fetched and checked out by cloud-init."
  type        = string
  default     = "main"

  validation {
    condition     = can(regex("^[0-9A-Za-z._/-]+$", var.repository_ref))
    error_message = "repository_ref may contain only letters, digits, dot, underscore, slash, and hyphen."
  }
}

variable "name_prefix" {
  description = "Label and hostname prefix for throwaway resources."
  type        = string
  default     = "sembla-precision"

  validation {
    condition     = can(regex("^[a-z0-9-]+$", var.name_prefix))
    error_message = "name_prefix may contain lowercase letters, digits, and hyphens only."
  }
}

variable "auto_halt_enabled" {
  description = "Install an in-guest emergency halt timer. Halting does not stop Vultr billing; terraform destroy is still mandatory."
  type        = bool
  default     = true
}

variable "auto_halt_hours" {
  description = "Hours after boot before the guest emergency halt timer fires."
  type        = number
  default     = 4

  validation {
    condition     = var.auto_halt_hours >= 1 && var.auto_halt_hours <= 24 && floor(var.auto_halt_hours) == var.auto_halt_hours
    error_message = "auto_halt_hours must be a whole number from 1 through 24."
  }
}
