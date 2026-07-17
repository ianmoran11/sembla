# Safe credential-free defaults. Copy to terraform.tfvars; that file is ignored.
offline_plan         = true
enable_discovery     = false
create_instance      = false
accept_paid_creation = false

# Replace every discovery placeholder with exact account-visible values.
region_name      = "replace-after-discovery"
environment_name = "replace-after-discovery"
flavor_name      = "replace-after-discovery"
image_name       = "replace-after-discovery"
key_name         = "sembla-hyperstack"

# Replace TEST-NET with: $(curl -4fsS https://api.ipify.org)/32
ssh_cidr = "198.51.100.10/32"

expected_gpu_model = "A100"
expected_gpu_count = 1

# Fill from the live Hyperstack pricebook after selecting the exact flavor.
expected_hourly_price_usd = 0
max_hourly_price_usd      = 5

# Replace with the exact pushed commit that includes the fired-flag diagnostic.
repository_ref = "0000000000000000000000000000000000000000"

emergency_poweroff_hours = 4
name_prefix              = "sembla-precision"
