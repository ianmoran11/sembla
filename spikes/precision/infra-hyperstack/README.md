# Prepared Hyperstack GPU infrastructure for the precision spike

> [!CAUTION]
> This module is **non-creating by default**. Hyperstack bills VMs in both
> `ACTIVE` and `SHUTOFF`; guest poweroff is not a billing control. A future paid
> apply requires explicit approval of a saved plan. After artifact retrieval,
> `terraform destroy` is mandatory.

This separate Terraform root leaves the approved AWS module and prepared Vultr
module unchanged. It targets exactly one full-rate NVIDIA GPU and pins the
official alpha provider to `NexGenCloud/hyperstack` `1.50.2-alpha`.

## Security and alpha-provider constraints

- `HYPERSTACK_API_KEY` stays only in the local process environment. It is never
  placed in Terraform variables, state, user-data, outputs, or Git.
- SSH is allowed only on TCP/22 from one canonical public IPv4 `/32`.
- Hyperstack creates VMs with no default ingress. Terraform adds the `/32` rule
  after VM creation; broad IPv4/IPv6 egress is provider-managed.
- Collection requires an ED25519 SSH host-key fingerprint independently read
  from Hyperstack's trusted VNC console; trust-on-first-use is not accepted.
- Port randomization is disabled because automation uses direct port 22.
- VM labels and optional volumes are deliberately omitted due to known alpha
  provider consistency/lifecycle bugs.
- The VM name includes a hash of rendered bootstrap inputs. This forces a
  destroy/recreate when the `/32`, commit, runner, or timer changes because the
  alpha provider does not support in-place VM updates.
- Flavor/image data-source filters are deliberately omitted because provider
  `1.50.2-alpha` mishandles them; nested live region fields are filtered locally.
- A public IP can appear after VM creation. If necessary, run `terraform refresh`
  before `collect-runs.sh`.
- The CUDA image must provide both `nvidia-smi` and `nvcc`. Bootstrap fails rather
  than silently replacing the driver/toolkit used as decision evidence.

## Files

- `.terraform.lock.hcl` — checksums for the exactly pinned alpha provider;
- `discover.sh` — authenticated, read-only region/flavor/stock/image/key/pricebook listing;
- `main.tf` — zero-resource defaults, live selection guards, one VM, and exact `/32` rule;
- `cloud-init.sh.tftpl` — early guest firewall/poweroff timer and CUDA/Rust bootstrap;
- `remote-run-spike.sh` — one CUDA+Vulkan benchmark invocation with Hyperstack provenance;
- `collect-runs.sh` — seeds, executes, and retrieves the required three independent runs;
- `verify-artifacts.py` — rejects incomplete, unbound, wrong-device, or cross-run evidence;
- `example.tfvars` — safe placeholders with paid creation disabled.

## 1. Credential-free validation

No API key is required and no resource is read or created:

```bash
cd spikes/precision/infra-hyperstack
terraform init
terraform fmt -check -recursive
terraform validate
terraform plan -refresh=false -var-file=example.tfvars
bash -n cloud-init.sh.tftpl
bash -n remote-run-spike.sh
bash -n discover.sh
bash -n collect-runs.sh
python3 -m py_compile verify-artifacts.py
```

The offline plan must report **0 to add, 0 to change, 0 to destroy** and a null
`discovery` output.

## 2. Authenticated read-only discovery

The API key exported during account setup is not visible to an already-running
Pi process. Run discovery yourself in the shell where it is exported:

```bash
cd spikes/precision/infra-hyperstack
bash discover.sh | tee hyperstack-discovery.txt
```

If needed, target a listed region explicitly:

```bash
bash discover.sh CANADA-1 | tee hyperstack-discovery.txt
```

The current official catalog suggests `CANADA-1` / `n3-A100x1` (one A100 80 GB
PCIe), but **do not copy those values unless live account discovery confirms
stock, the exact CUDA image, and account-specific price**.

`discover.sh` prints no credential or private key. Record:

1. exact region and existing environment;
2. exact keypair name and environment;
3. exact one-GPU A100/H100/H200/GH200 flavor with live 1x stock;
4. exact region-compatible Ubuntu CUDA image name;
5. account pricebook value plus any public-IP charge.

Copy the safe example locally and fill only those discovered values:

```bash
cp example.tfvars terraform.tfvars
```

Keep these values while performing the first authenticated Terraform plan:

```hcl
offline_plan         = false
enable_discovery     = true
create_instance      = false
accept_paid_creation = false
```

Refresh the operator address immediately before planning:

```bash
printf '%s/32\n' "$(curl -4fsS https://api.ipify.org)"
```

Then run:

```bash
terraform plan -var-file=terraform.tfvars
```

This reads the account but must still report **no resource actions**. Review the
`discovery` and `selected_profile` outputs. Because the provider exposes no
pricebook data source, `expected_hourly_price_usd` is an operator-reviewed live
input and is hard-capped by `max_hourly_price_usd` (default `$5/hour`).

## 3. Future paid plan — explicit approval required

Before spending money:

1. implement and test the portable fired-flag mismatch diagnostic required by
   ADR 0001;
2. push the exact benchmark commit and set its 40-hex SHA as `repository_ref`;
3. confirm live stock, image, `/32`, and complete hourly price again;
4. set `create_instance=true` and `accept_paid_creation=true` locally;
5. create, inspect, and retain a saved plan:

```bash
terraform plan -var-file=terraform.tfvars -out=hyperstack-paid.tfplan
terraform show hyperstack-paid.tfplan
```

The plan must contain exactly:

- one `hyperstack_core_virtual_machine` using one full-rate GPU;
- one `hyperstack_core_virtual_machine_sg_rule` for TCP/22 from the reviewed `/32`;
- no environment, keypair, volume, or unrelated resource creation.

Do not apply until the user explicitly approves that exact saved plan. Apply it
promptly; discard and re-plan if the operator `/32`, stock, image, commit, or
price changes. If plan or apply fails, inspect Hyperstack immediately: if any VM
exists, delete it in the console rather than assuming Terraform rolled it back.
After approval, apply the saved plan rather than recomputing it:

```bash
terraform apply hyperstack-paid.tfplan
```

## 4. Bootstrap and three-run evidence collection

Cloud-init installs an emergency guest poweroff timer first, but billing
continues after poweroff. It then applies the guest `/32` defense, verifies the
selected CUDA image, installs Rust/Vulkan prerequisites, checks out the exact
commit, and compiles the spike. It does **not** start the benchmark automatically.

With the VM active, open its trusted Hyperstack VNC console and run:

```bash
ssh-keygen -E sha256 -lf /etc/ssh/ssh_host_ed25519_key.pub
```

Independently copy only the displayed `SHA256:...` fingerprint, then collect:

```bash
SSH_HOST_KEY_FINGERPRINT='SHA256:replace-from-trusted-console' \
  SSH_PRIVATE_KEY_PATH="$HOME/.ssh/sembla_hyperstack" \
  RESULTS_SEED_PATH="$(cd .. && pwd)/RESULTS.md" \
  bash collect-runs.sh
```

The collector:

- waits for bootstrap completion;
- copies the Mac-containing `RESULTS.md` into three byte-identical remote files;
- performs three same-machine runs with distinct absolute `SPIKE_RESULTS_PATH` values and collector-generated run IDs;
- preserves a separate external log whose start/completion markers bind each run ID to the exact result SHA-256;
- retrieves all results/logs plus the full bootstrap log, `nvidia-smi -q`, and exact commit;
- parses every embedded `machines.nvidia` state and rejects unsupported state
  versions, wrong hardware, software/non-Vulkan adapters, non-full-rate
  classification, wrong workload, missing diagnostics on answered rows,
  strategy-availability drift, or cross-run machine/provenance mismatches;
- requires exact per-strategy guard evidence and three distinct run IDs,
  generation times, result hashes, and matching complete logs.

The verifier intentionally permits a measured candidate to fail qualification
or be unavailable: that outcome is evidence, not a reason to discard the other
candidates. It still requires `fired_mismatch_count` on every answered strategy
and `unexplained_arithmetic_mirror_difference_count` on answered native rows.
A successful artifact verification means the three runs are structurally valid;
it does not claim every candidate qualifies. Local artifact directories are
ignored by Git.

## 5. Destroy immediately

After confirming every artifact is non-empty:

```bash
terraform destroy -var-file=terraform.tfvars
terraform state list
```

The alpha provider waits only 120 seconds for VM deletion. If destroy fails or
times out, inspect the console immediately and delete the VM there. After the
console confirms deletion, rerun `terraform destroy`/`terraform refresh`; remove
stale VM or rule addresses with `terraform state rm` only after confirming the
real resource no longer exists. The final state listing must contain no paid VM
or security-rule resource. **Do not merely stop or power off the VM: `SHUTOFF`
continues billing.** Keep local Terraform state until deletion is confirmed.

No paid Hyperstack resource has been created by this prepared module.
