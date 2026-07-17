output "instance_id" {
  description = "Created Hyperstack VM ID, or null during validation/discovery."
  value       = try(hyperstack_core_virtual_machine.gpu[0].id, null)
}

output "public_ip" {
  description = "Created public IPv4 address, or null during validation/discovery."
  value       = try(hyperstack_core_virtual_machine.gpu[0].floating_ip, null)
}

output "ssh_user" {
  description = "Ubuntu SSH account used by collect-runs.sh."
  value       = var.ssh_user
}

output "ssh_command" {
  description = "SSH command skeleton. Add -i ~/.ssh/sembla_hyperstack."
  value = try(
    "ssh ${var.ssh_user}@${hyperstack_core_virtual_machine.gpu[0].floating_ip}",
    null
  )
}

output "discovery" {
  description = "Read-only account-visible selections. Hyperstack flavor data does not include price; verify the pricebook separately."
  value = local.read_api ? {
    regions = [for region in local.live_regions : {
      id          = region.id
      name        = region.name
      description = region.description
    }]
    environments = [for environment in local.live_environments : {
      id     = environment.id
      name   = environment.name
      region = environment.region
    }]
    keypairs = [for keypair in local.live_keypairs : {
      id          = keypair.id
      name        = keypair.name
      environment = keypair.environment.name
      region      = keypair.environment.region
      fingerprint = keypair.fingerprint
    }]
    full_rate_flavors = [for flavor in local.live_flavors : {
      id              = flavor.id
      name            = flavor.name
      display_name    = flavor.display_name
      region_name     = flavor.region_name
      gpu             = flavor.gpu
      gpu_count       = flavor.gpu_count
      cpu             = flavor.cpu
      ram             = flavor.ram
      disk            = flavor.disk
      stock_available = flavor.stock_available
    } if can(regex(local.full_rate_pattern, flavor.gpu))]
    stock = [for stock in local.live_stock_models : {
      region         = stock.region
      stocktype      = stock.stocktype
      model          = stock.model
      available      = stock.available
      configurations = stock.configurations
    } if can(regex(local.full_rate_pattern, stock.model))]
    ubuntu_cuda_images = [for image in local.live_images : {
      id          = image.id
      name        = image.name
      description = image.description
      region_name = image.region_name
      type        = image.type
      version     = image.version
      is_public   = image.is_public
    } if can(regex("(?i)ubuntu", image.name)) && can(regex("(?i)cuda", image.name))]
  } : null
}

output "selected_profile" {
  description = "Selection and reviewed price that a future paid plan will enforce."
  value = {
    provider                  = "hyperstack"
    region                    = var.region_name
    environment               = var.environment_name
    flavor                    = var.flavor_name
    image                     = var.image_name
    expected_gpu              = upper(var.expected_gpu_model)
    expected_gpu_count        = var.expected_gpu_count
    requested_fp64_class      = "full-rate"
    expected_hourly_price_usd = var.expected_hourly_price_usd
    max_hourly_price_usd      = var.max_hourly_price_usd
    ssh_cidr                  = var.ssh_cidr
    repository_ref            = var.repository_ref
  }
}

output "billing_warning" {
  description = "Cost-control invariant for this throwaway module."
  value       = "Hyperstack ACTIVE and SHUTOFF VMs continue billing. Retrieve all artifacts, then terraform destroy immediately. Guest poweroff is not a billing control."
}
