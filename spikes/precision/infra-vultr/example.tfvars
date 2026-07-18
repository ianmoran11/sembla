# No credentials or paid resources are used with these safety defaults.
offline_plan     = true
enable_discovery = false
create_instance  = false

# Replace only after `bash discover.sh` confirms an account-visible full-rate
# plan, compatible region, exact hourly price, and OS architecture.
deployment_kind                      = "cloud_gpu"
accept_bare_metal_bootstrap_exposure = false
region_id                            = "ewr"
plan_id                              = "vcg-replace-with-full-rate-plan"
os_id                                = 1743 # Ubuntu 22.04 x64; not valid for an ARM64 GH200 plan.

# Existing Vultr SSH key UUIDs, never private key material.
ssh_key_ids = []
ssh_cidr    = "203.0.113.10/32"

expected_gpu_model   = "NVIDIA A100"
max_hourly_price_usd = 5
auto_halt_enabled    = true
auto_halt_hours      = 4
repository_url       = "https://github.com/ianmoran11/sembla.git"
repository_ref       = "main" # Replace with a pushed commit containing the spike.
name_prefix          = "sembla-precision"
