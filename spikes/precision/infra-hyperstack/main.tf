locals {
  read_api        = !var.offline_plan && (var.enable_discovery || var.create_instance)
  create_instance = !var.offline_plan && var.create_instance

  full_rate_pattern    = "(?i)(^|[^A-Z0-9])(A100|H100|H200|GH200)([^A-Z0-9]|$)"
  expected_gpu_pattern = "(?i)(^|[^A-Z0-9])${upper(var.expected_gpu_model)}([^A-Z0-9]|$)"
  ssh_address          = split("/", var.ssh_cidr)[0]
  non_public_ipv4_pattern = join("", [
    "^(0\\.|10\\.|",
    "100\\.(6[4-9]|[7-9][0-9]|1[01][0-9]|12[0-7])\\.|",
    "127\\.|169\\.254\\.|172\\.(1[6-9]|2[0-9]|3[01])\\.|",
    "192\\.(0\\.(0|2)\\.|168\\.)|",
    "198\\.(1[89]\\.|51\\.100\\.)|203\\.0\\.113\\.|",
    "(22[4-9]|23[0-9]|24[0-9]|25[0-5])\\.)",
  ])
  ssh_is_non_public = can(regex(local.non_public_ipv4_pattern, local.ssh_address))

  live_regions = local.read_api ? tolist(data.hyperstack_core_regions.available[0].core_regions) : []
  live_environments = local.read_api ? tolist(
    data.hyperstack_core_environments.available[0].core_environments
  ) : []
  live_keypairs = local.read_api ? tolist(
    data.hyperstack_core_keypairs.available[0].core_keypairs
  ) : []
  live_flavors = local.read_api ? flatten([
    for group in data.hyperstack_core_flavors.available[0].core_flavors : group.flavors
  ]) : []
  live_images = local.read_api ? flatten([
    for group in data.hyperstack_core_images.available[0].core_images : group.images
  ]) : []
  live_stock_models = local.read_api ? flatten([
    for stock in data.hyperstack_core_stocks.available[0].stocks : [
      for model in stock.models : {
        region         = stock.region
        stocktype      = stock.stocktype
        model          = model.model
        available      = model.available
        configurations = model.configurations
      }
    ]
  ]) : []

  matching_regions = [
    for region in local.live_regions : region
    if region.name == var.region_name
  ]
  matching_environments = [
    for environment in local.live_environments : environment
    if environment.name == var.environment_name && environment.region == var.region_name
  ]
  matching_keypairs = [
    for keypair in local.live_keypairs : keypair
    if keypair.name == var.key_name && keypair.environment.name == var.environment_name
  ]
  matching_flavors = [
    for flavor in local.live_flavors : flavor
    if flavor.name == var.flavor_name && flavor.region_name == var.region_name
  ]
  matching_images = [
    for image in local.live_images : image
    if image.name == var.image_name && image.region_name == var.region_name
  ]
  matching_stock_models = [
    for stock in local.live_stock_models : stock
    if stock.region == var.region_name &&
    can(regex(local.expected_gpu_pattern, stock.model))
  ]

  selected_flavor = length(local.matching_flavors) == 1 ? local.matching_flavors[0] : null
  selected_image  = length(local.matching_images) == 1 ? local.matching_images[0] : null

  bootstrap = templatefile("${path.module}/cloud-init.sh.tftpl", {
    emergency_poweroff_hours = var.emergency_poweroff_hours
    environment_name_b64     = base64encode(var.environment_name)
    expected_gpu_count       = var.expected_gpu_count
    expected_gpu_model_b64   = base64encode(upper(var.expected_gpu_model))
    flavor_name_b64          = base64encode(var.flavor_name)
    image_name_b64           = base64encode(var.image_name)
    region_name_b64          = base64encode(var.region_name)
    remote_run_spike_b64     = base64encode(file("${path.module}/remote-run-spike.sh"))
    repository_ref           = var.repository_ref
    repository_url_b64       = base64encode(var.repository_url)
    ssh_cidr_b64             = base64encode(var.ssh_cidr)
    ssh_user                 = var.ssh_user
  })
  # VM Update is unsupported in provider 1.50.2-alpha and user_data is not
  # marked ForceNew. Include every rendered bootstrap input in a ForceNew name
  # so CIDR/ref/script/timer changes destroy and recreate instead of failing an
  # in-place update or leaving the guest firewall stale.
  bootstrap_fingerprint = substr(sha256(local.bootstrap), 0, 12)
}

check "offline_mode_is_non_creating" {
  assert {
    condition     = !var.offline_plan || (!var.enable_discovery && !var.create_instance)
    error_message = "offline_plan=true disables discovery and creation; leave both enable_discovery and create_instance false."
  }
}

check "paid_mode_is_explicit" {
  assert {
    condition     = !var.create_instance || (!var.offline_plan && var.enable_discovery && var.accept_paid_creation)
    error_message = "Paid creation requires offline_plan=false, enable_discovery=true, and accept_paid_creation=true."
  }
}

data "hyperstack_core_regions" "available" {
  count = local.read_api ? 1 : 0
}

data "hyperstack_core_environments" "available" {
  count = local.read_api ? 1 : 0
}

data "hyperstack_core_keypairs" "available" {
  count = local.read_api ? 1 : 0
}

# v1.50.2-alpha mishandles the optional flavor/image filters and returned state.
# Query the complete account-visible sets and filter their nested region fields.
data "hyperstack_core_flavors" "available" {
  count = local.read_api ? 1 : 0
}

data "hyperstack_core_images" "available" {
  count = local.read_api ? 1 : 0
}

data "hyperstack_core_stocks" "available" {
  count = local.read_api ? 1 : 0
}

resource "hyperstack_core_virtual_machine" "gpu" {
  count = local.create_instance ? 1 : 0

  name                      = "${substr(var.name_prefix, 0, 32)}-${local.bootstrap_fingerprint}"
  environment_name          = var.environment_name
  flavor_name               = var.flavor_name
  image_name                = var.image_name
  key_name                  = var.key_name
  assign_floating_ip        = true
  create_bootable_volume    = false
  enable_port_randomization = false
  user_data                 = local.bootstrap

  lifecycle {
    precondition {
      condition     = var.enable_discovery
      error_message = "Paid creation requires enable_discovery=true so account-visible selections are checked in the same plan."
    }
    precondition {
      condition     = var.accept_paid_creation
      error_message = "Paid creation requires accept_paid_creation=true after explicit plan and price review."
    }
    precondition {
      condition     = !local.ssh_is_non_public
      error_message = "Paid creation requires a real public operator IPv4 /32; private, reserved, multicast, and TEST-NET addresses are rejected."
    }
    precondition {
      condition     = length(local.matching_regions) == 1
      error_message = "region_name must match exactly one account-visible Hyperstack region."
    }
    precondition {
      condition     = length(local.matching_environments) == 1
      error_message = "environment_name must match exactly one live environment in region_name."
    }
    precondition {
      condition     = length(local.matching_keypairs) == 1
      error_message = "key_name must match exactly one live keypair in environment_name."
    }
    precondition {
      condition     = length(local.matching_flavors) == 1
      error_message = "flavor_name must match exactly one live flavor in region_name."
    }
    precondition {
      condition     = length(local.matching_flavors) == 1 ? local.selected_flavor.stock_available : false
      error_message = "The selected flavor is not currently reported in stock."
    }
    precondition {
      condition     = length(local.matching_flavors) == 1 ? local.selected_flavor.gpu_count == var.expected_gpu_count : false
      error_message = "The selected flavor must expose exactly one GPU."
    }
    precondition {
      condition = length(local.matching_flavors) == 1 ? (
        can(regex(local.full_rate_pattern, local.selected_flavor.gpu)) &&
        can(regex(local.expected_gpu_pattern, local.selected_flavor.gpu))
      ) : false
      error_message = "The selected flavor GPU must contain an exact A100/H100/H200/GH200 token and match expected_gpu_model."
    }
    precondition {
      condition = length(local.matching_stock_models) > 0 ? anytrue([
        for stock in local.matching_stock_models : stock.configurations.n1x > 0
      ]) : false
      error_message = "Live Hyperstack stock must report at least one 1x configuration for expected_gpu_model in region_name."
    }
    precondition {
      condition     = length(local.matching_images) == 1
      error_message = "image_name must match exactly one live image in region_name."
    }
    precondition {
      condition = length(local.matching_images) == 1 ? (
        can(regex("(?i)ubuntu", local.selected_image.name)) &&
        can(regex("(?i)cuda", local.selected_image.name))
      ) : false
      error_message = "The selected image must explicitly identify both Ubuntu and CUDA; bootstrap refuses to substitute a driver/toolkit image."
    }
    precondition {
      condition = (
        var.expected_hourly_price_usd > 0 &&
        var.expected_hourly_price_usd <= var.max_hourly_price_usd
      )
      error_message = "The reviewed expected_hourly_price_usd must be positive and at or below max_hourly_price_usd."
    }
    precondition {
      condition     = var.repository_ref != "0000000000000000000000000000000000000000"
      error_message = "Replace the all-zero repository_ref with the exact pushed benchmark commit before paid creation."
    }
  }
}

resource "hyperstack_core_virtual_machine_sg_rule" "ssh" {
  count = local.create_instance ? 1 : 0

  virtual_machine_id = hyperstack_core_virtual_machine.gpu[0].id
  direction          = "ingress"
  ethertype          = "IPv4"
  protocol           = "tcp"
  port_range_min     = 22
  port_range_max     = 22
  remote_ip_prefix   = var.ssh_cidr

  lifecycle {
    precondition {
      condition     = endswith(var.ssh_cidr, "/32") && !local.ssh_is_non_public
      error_message = "The SSH rule must remain one real public IPv4 /32."
    }
  }
}
