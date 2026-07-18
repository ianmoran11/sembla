# Prepared Vultr GPU infrastructure for the precision spike

> [!CAUTION]
> This module is intentionally **validation-only by default**. It creates nothing
> unless `offline_plan=false` and `create_instance=true` are both set. A guest
> shutdown does not stop Vultr billing; only deleting the Vultr resource does.

This is a separate alternative to the approved AWS module in `../infra/`. It can
inspect and, after an explicit future approval, provision either a Vultr Cloud
GPU (`vultr_instance`) or GPU bare-metal server
(`vultr_bare_metal_server`). It preserves the PRD-0005 benchmark and result-state
contract without changing the AWS implementation.

No Vultr API key, private key, Terraform state, plan, or operator tfvars belongs
in Git.

## Security action required first

The API key previously pasted into chat must be considered disclosed. Revoke it
in the Vultr console, create a replacement, and do **not** paste the replacement
into chat or a `.tfvars` file. If Vultr API access control is enabled, allow your
current public IP. Export the replacement only in your local shell:

```bash
export VULTR_API_KEY='replacement-key-from-vultr'
```

The Terraform provider and `discover.sh` read that environment variable.

## Current public-catalog finding

A read-only public API check on 2026-07-17 showed no single-A100 Cloud GPU plan
with advertised locations. The public full-rate candidates included:

- one GH200 bare-metal plan in `atl`, approximately $2.75/hour;
- eight-A100 bare metal in `ewr`/`fra`, approximately $20.62/hour.

Marketing availability is not deployment availability, and account-visible
catalogs can differ. Do not hard-code either option. Run account discovery first.
The generic module defaults to a non-existent placeholder plan and refuses paid
creation.

A16, A40, L40S, T4, L4, and gaming/RTX plans are rate-limited for fp64 and do not
settle the full-rate decision. Only an account-visible A100/H100/H200/GH200/B200
candidate may pass this module's full-rate guard, and the benchmark's runtime
CUDA ratio remains authoritative.

## Files

- `discover.sh` — authenticated read-only plan, region, SSH-key, and OS listing;
- `main.tf` — conditional discovery plus Cloud GPU or bare-metal resources;
- `cloud-init.sh.tftpl` — early emergency timer/firewall, CUDA/Rust bootstrap,
  repository checkout, and runner installation;
- `run-spike.sh` — CUDA + Vulkan benchmark with Vultr provenance;
- `example.tfvars` — non-creating placeholders and safety defaults.

## 1. Credential-free validation

```bash
cd spikes/precision/infra-vultr
cp example.tfvars terraform.tfvars
terraform init
terraform fmt -check -recursive
VULTR_API_KEY=000000000000000000000000000000000000 terraform validate
VULTR_API_KEY=000000000000000000000000000000000000 \
  terraform plan -refresh=false -var-file=example.tfvars
```

The offline plan must create **zero** resources and show null discovered cost.
It validates Terraform shape only; it does not claim that a GPU is available.

## 2. Read-only account discovery

After rotating and exporting the API key:

```bash
bash discover.sh
```

Choose only a full-rate candidate shown to your account. Record:

- deployment kind (`cloud_gpu` or `bare_metal`);
- exact plan and region IDs;
- monthly and hourly price;
- exact GPU count/model;
- compatible GPU-enabled Ubuntu image ID and architecture;
- an existing Vultr SSH key UUID.

For GH200, Ubuntu/CUDA must be verified for ARM64 before any apply; the x64
`os_id=1743` example is not acceptable. For Cloud GPU, choose a GPU-enabled image
that supplies the NVIDIA driver. The bootstrap deliberately fails rather than
silently substituting a driver.

Fill `terraform.tfvars`, keeping `create_instance=false`, then run an authenticated
read-only plan:

```bash
terraform plan -var-file=terraform.tfvars \
  -var='offline_plan=false' \
  -var='enable_discovery=true'
```

Review the `discovery` output. Read-only discovery emits check warnings for a
plan outside the selected region, a non-full-rate model, or
`monthly_cost / 730` above `max_hourly_price_usd`; paid resources repeat these as
hard lifecycle preconditions and cannot plan past them. The price guard protects
the plan, not subsequent billing.

## 3. Future paid apply — not authorized or performed

A future apply requires all of the following from the operator:

- a safely exported replacement API key;
- an account-visible full-rate plan with current capacity;
- a compatible GPU-enabled image;
- at least one Vultr SSH key UUID;
- the operator's current public IPv4 `/32`;
- a pushed public repository commit containing the precision spike;
- explicit approval of the exact hourly price and GPU count.

Only after that review would the paid command be:

```bash
terraform plan -var-file=terraform.tfvars \
  -var='offline_plan=false' \
  -var='enable_discovery=true' \
  -var='create_instance=true'

terraform apply -var-file=terraform.tfvars \
  -var='offline_plan=false' \
  -var='enable_discovery=true' \
  -var='create_instance=true'
```

Do not apply a stale saved plan. Cloud GPU receives a Vultr firewall group with
SSH restricted to the operator's exact `/32`. The provider does not expose that
attachment on bare metal. Bare metal therefore has a short SSH-key-only exposure
between first boot and user-data execution; cloud-init installs an early
`iptables` rule and then `ufw`, but it cannot remove that initial interval.
Bare-metal creation is blocked unless
`accept_bare_metal_bootstrap_exposure=true` is set explicitly. SSH-key
authentication is mandatory in either mode.

The emergency guest halt defaults to four hours. It does **not** stop billing and
is not a substitute for destroy.

## 4. Bootstrap and benchmark workflow after a future apply

Use `terraform output -raw ssh_command` and wait for:

```bash
cloud-init status --wait
test -f /var/lib/sembla-bootstrap-complete
```

Bootstrap diagnostics are in `/var/log/sembla-bootstrap.log`. Verify
`nvidia-smi`, `nvcc`, Vulkan, the exact model, and the benchmark's fp64 ratio.
The installed runner is `/usr/local/bin/run-sembla-spike` and the checkout is
`/opt/sembla`.

Seed the Mac-containing `RESULTS.md` before running. For the PRD-0006 decision,
make three byte-identical result copies first and invoke the runner three times
with distinct absolute `SPIKE_RESULTS_PATH` values, preserving an external log
for each run. Reusing one path replaces its previous `nvidia` machine state.

Retrieve all results and logs before teardown. Decision timing comes from each
file's embedded `machines.nvidia.strategies`, not the rendered cross-machine
matrix.

## 5. Destroy immediately

```bash
terraform destroy -var-file=terraform.tfvars \
  -var='offline_plan=false' \
  -var='enable_discovery=true' \
  -var='create_instance=true'
```

Confirm the instance/server no longer appears in the Vultr console. Preserve
local Terraform state until destroy succeeds. A halted or unreachable server
continues billing.
