locals {
  lookup_enabled = !var.offline_plan && (var.enable_discovery || var.create_instance)
  create_cloud   = var.create_instance && var.deployment_kind == "cloud_gpu"
  create_bare    = var.create_instance && var.deployment_kind == "bare_metal"

  ssh_subnet      = split("/", var.ssh_cidr)[0]
  ssh_subnet_size = tonumber(split("/", var.ssh_cidr)[1])

  selected_monthly_cost = var.deployment_kind == "cloud_gpu" ? try(
    data.vultr_plan.selected[0].monthly_cost,
    null,
    ) : try(
    data.vultr_bare_metal_plan.selected[0].monthly_cost,
    null,
  )
  selected_hourly_cost = local.selected_monthly_cost == null ? null : local.selected_monthly_cost / 730
  selected_locations = var.deployment_kind == "cloud_gpu" ? try(
    data.vultr_plan.selected[0].locations,
    [],
    ) : try(
    data.vultr_bare_metal_plan.selected[0].locations,
    [],
  )
  selected_gpu_type = var.deployment_kind == "cloud_gpu" ? try(
    data.vultr_plan.selected[0].gpu_type,
    "unavailable",
  ) : "encoded-in-bare-metal-plan-id"

  bootstrap = templatefile("${path.module}/cloud-init.sh.tftpl", {
    auto_halt_enabled  = tostring(var.auto_halt_enabled)
    auto_halt_hours    = var.auto_halt_hours
    deployment_kind    = var.deployment_kind
    expected_gpu_model = var.expected_gpu_model
    plan_id            = var.plan_id
    region_id          = var.region_id
    repository_ref     = var.repository_ref
    repository_url     = var.repository_url
    run_spike_b64      = base64encode(file("${path.module}/run-spike.sh"))
    ssh_cidr           = var.ssh_cidr
  })
}

data "vultr_plan" "selected" {
  count = local.lookup_enabled && var.deployment_kind == "cloud_gpu" ? 1 : 0

  filter {
    name   = "id"
    values = [var.plan_id]
  }
}

data "vultr_bare_metal_plan" "selected" {
  count = local.lookup_enabled && var.deployment_kind == "bare_metal" ? 1 : 0

  filter {
    name   = "id"
    values = [var.plan_id]
  }
}

check "mode_is_safe" {
  assert {
    condition     = !(var.offline_plan && (var.enable_discovery || var.create_instance))
    error_message = "offline_plan=true cannot discover or create Vultr resources. Export VULTR_API_KEY and set offline_plan=false first."
  }
}

check "resource_family_matches_plan" {
  assert {
    condition = (
      (var.deployment_kind == "cloud_gpu" && startswith(var.plan_id, "vcg-")) ||
      (var.deployment_kind == "bare_metal" && startswith(var.plan_id, "vbm-"))
    )
    error_message = "cloud_gpu requires a vcg-... plan; bare_metal requires a vbm-... plan."
  }
}

check "create_inputs" {
  assert {
    condition     = !var.create_instance || length(var.ssh_key_ids) > 0
    error_message = "create_instance=true requires at least one existing Vultr SSH key UUID."
  }
}

check "plan_is_in_region" {
  assert {
    condition     = !local.lookup_enabled || contains(local.selected_locations, var.region_id)
    error_message = "The selected plan does not advertise the requested region. Choose an ID from live discovery; capacity can still change after plan."
  }
}

check "plan_is_full_rate_candidate" {
  assert {
    condition = !local.lookup_enabled || (
      var.deployment_kind == "cloud_gpu" ?
      can(regex("(?i)(^|[^A-Z0-9])(A100|H100|H200|GH200|B200)($|[^A-Z0-9])", local.selected_gpu_type)) :
      can(regex("(?i)(^|-)(a100|h100|h200|gh200|b200)(-|$)", var.plan_id))
    )
    error_message = "The selected plan is not an A100/H100/H200/GH200/B200-class full-rate candidate. Do not use A16/A40/L40S/T4 for the fp64 decision."
  }
}

check "price_is_within_operator_cap" {
  assert {
    condition = !local.lookup_enabled ? true : (
      local.selected_hourly_cost == null ? false :
      local.selected_hourly_cost <= var.max_hourly_price_usd
    )
    error_message = "Selected plan monthly_cost/730 exceeds max_hourly_price_usd. Review the exact plan and raise the cap explicitly or choose another plan."
  }
}

resource "vultr_firewall_group" "gpu" {
  count = local.create_cloud ? 1 : 0

  description = "${var.name_prefix}: restricted SSH for throwaway precision spike"
}

resource "vultr_firewall_rule" "ssh" {
  count = local.create_cloud ? 1 : 0

  firewall_group_id = vultr_firewall_group.gpu[0].id
  protocol          = "tcp"
  ip_type           = "v4"
  subnet            = local.ssh_subnet
  subnet_size       = local.ssh_subnet_size
  port              = "22"
  notes             = "Restricted operator SSH"
}

resource "vultr_instance" "gpu" {
  count = local.create_cloud ? 1 : 0

  region            = var.region_id
  plan              = var.plan_id
  os_id             = var.os_id
  label             = "${var.name_prefix}-cloud-gpu"
  hostname          = "${var.name_prefix}-gpu"
  firewall_group_id = vultr_firewall_group.gpu[0].id
  ssh_key_ids       = var.ssh_key_ids
  user_data         = local.bootstrap
  enable_ipv6       = false
  activation_email  = false
  tags              = ["sembla", "precision-spike", "throwaway", "full-rate-candidate"]

  depends_on = [vultr_firewall_rule.ssh]

  lifecycle {
    precondition {
      condition     = !var.offline_plan
      error_message = "Paid creation is forbidden while offline_plan=true."
    }
    precondition {
      condition     = length(var.ssh_key_ids) > 0
      error_message = "Paid creation requires at least one existing Vultr SSH key UUID."
    }
    precondition {
      condition     = startswith(var.plan_id, "vcg-")
      error_message = "Cloud GPU creation requires a vcg-... plan ID."
    }
    precondition {
      condition     = contains(local.selected_locations, var.region_id)
      error_message = "The Cloud GPU plan does not advertise the requested region."
    }
    precondition {
      condition     = can(regex("(?i)(^|[^A-Z0-9])(A100|H100|H200|GH200|B200)($|[^A-Z0-9])", local.selected_gpu_type))
      error_message = "The Cloud GPU plan is not a recognized full-rate fp64 candidate."
    }
    precondition {
      condition = local.selected_hourly_cost == null ? false : (
        local.selected_hourly_cost <= var.max_hourly_price_usd
      )
      error_message = "The Cloud GPU plan exceeds max_hourly_price_usd."
    }
  }
}

resource "vultr_bare_metal_server" "gpu" {
  count = local.create_bare ? 1 : 0

  region           = var.region_id
  plan             = var.plan_id
  os_id            = var.os_id
  label            = "${var.name_prefix}-bare-metal-gpu"
  hostname         = "${var.name_prefix}-gpu"
  ssh_key_ids      = var.ssh_key_ids
  user_data        = local.bootstrap
  enable_ipv6      = false
  activation_email = false
  tags             = ["sembla", "precision-spike", "throwaway", "full-rate-candidate"]

  lifecycle {
    precondition {
      condition     = !var.offline_plan
      error_message = "Paid creation is forbidden while offline_plan=true."
    }
    precondition {
      condition     = length(var.ssh_key_ids) > 0
      error_message = "Paid creation requires at least one existing Vultr SSH key UUID."
    }
    precondition {
      condition     = startswith(var.plan_id, "vbm-")
      error_message = "Bare-metal creation requires a vbm-... plan ID."
    }
    precondition {
      condition     = var.accept_bare_metal_bootstrap_exposure
      error_message = "Bare-metal creation requires explicit acceptance of the short SSH-key-only exposure before cloud-init applies the source-CIDR firewall."
    }
    precondition {
      condition     = contains(local.selected_locations, var.region_id)
      error_message = "The bare-metal GPU plan does not advertise the requested region."
    }
    precondition {
      condition     = can(regex("(?i)(^|-)(a100|h100|h200|gh200|b200)(-|$)", var.plan_id))
      error_message = "The bare-metal plan ID is not a recognized full-rate fp64 candidate."
    }
    precondition {
      condition = local.selected_hourly_cost == null ? false : (
        local.selected_hourly_cost <= var.max_hourly_price_usd
      )
      error_message = "The bare-metal GPU plan exceeds max_hourly_price_usd."
    }
  }
}
