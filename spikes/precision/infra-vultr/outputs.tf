locals {
  created_id = var.deployment_kind == "cloud_gpu" ? try(
    vultr_instance.gpu[0].id,
    null,
    ) : try(
    vultr_bare_metal_server.gpu[0].id,
    null,
  )
  created_ip = var.deployment_kind == "cloud_gpu" ? try(
    vultr_instance.gpu[0].main_ip,
    null,
    ) : try(
    vultr_bare_metal_server.gpu[0].main_ip,
    null,
  )
}

output "discovery" {
  description = "Read-only selected-plan evidence. Null cost means offline lookup was intentionally disabled."
  value = {
    deployment_kind = var.deployment_kind
    plan_id         = var.plan_id
    region_id       = var.region_id
    gpu_type        = local.selected_gpu_type
    locations       = local.selected_locations
    monthly_cost    = local.selected_monthly_cost
    hourly_cost     = local.selected_hourly_cost
    price_cap       = var.max_hourly_price_usd
  }
}

output "instance_id" {
  description = "Created Vultr instance/server ID, or null during validation/discovery."
  value       = local.created_id
}

output "public_ip" {
  description = "Created public IPv4 address, or null during validation/discovery."
  value       = local.created_ip
}

output "ssh_command" {
  description = "SSH command after creation. Vultr Ubuntu images normally use root; verify the selected image."
  value       = local.created_ip == null ? null : "ssh root@${local.created_ip}"
}

output "fetch_results_command" {
  description = "Result retrieval command after creation."
  value       = local.created_ip == null ? null : "scp root@${local.created_ip}:/opt/sembla/spikes/precision/RESULTS.md ./RESULTS-vultr.md"
}

output "billing_warning" {
  description = "Cost-control invariant for this throwaway module."
  value       = "Guest halt does not stop Vultr billing. Retrieve all artifacts and run terraform destroy immediately."
}
