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
- SSH is allowed only on TCP/22 from one canonical public IPv4 `/32` and remains key-only. A temporary password enables trusted VNC-console recovery but is never accepted by `sshd`. Its plaintext is never passed to Terraform; the sensitive hash remains only in ignored local plan/state and rendered guest user-data until deletion.
- Hyperstack creates VMs with no default ingress. Terraform adds the `/32` rule
  after VM creation; broad IPv4/IPv6 egress is provider-managed.
- Collection requires an ED25519 SSH host-key fingerprint that is known
  independently of the connection being secured; trust-on-first-use is never
  accepted. **Amended 2026-07-25 — two sound paths, pick one:**
  - *Pre-seeded (preferred, and the only unattended one).* `prepare-host-key.sh`
    generates an ephemeral ED25519 host keypair locally; `TF_VAR_ssh_host_private_key`
    carries it into rendered user-data, and cloud-init installs it as the VM's
    only host key before sshd is restarted, failing the bootstrap if sshd does
    not then serve it. The fingerprint is therefore known *before the VM exists*,
    which removes the trust-on-first-use window entirely rather than closing it
    by hand. The collector waits for a key matching the pinned fingerprint and
    never accepts another, so the brief pre-cloud-init window in which the image's
    own key is served is a retry, not an acceptance.
    **Residual exposure, stated plainly:** the host private key is in rendered
    user-data and in local Terraform state (0600, ignored by this module's
    allowlist `.gitignore`), and cloud-init unlinks the on-VM user-data cache
    after installing it. The provider-side copy remains until the VM is
    destroyed. The key is single-use, per-VM, and worthless after destroy — but
    it is a key in user-data, and that is the trade being made.
  - *Console-read (original).* Leave `TF_VAR_ssh_host_private_key` unset and read
    the fingerprint from Hyperstack's trusted VNC console as in §4. Unchanged,
    still supported, still requires a human at a browser.
- Port randomization is disabled because automation uses direct port 22.
- VM labels and optional volumes are deliberately omitted due to known alpha
  provider consistency/lifecycle bugs.
- The VM name includes a hash of rendered bootstrap inputs. This forces a
  destroy/recreate when the `/32`, commit, runner, or timer changes because the
  alpha provider does not support in-place VM updates.
- Flavor/image data-source filters are deliberately omitted because provider
  `1.50.2-alpha` mishandles them; nested live region fields are filtered locally.
- A public IP can appear after VM creation and a refresh-only operation with non-creating variables can omit its conditional output. `collect-runs.sh` therefore falls back to the paid VM's `floating_ip` in Terraform state; do not run an unreviewed normal apply merely to restore an output.
- The CUDA image must provide both `nvidia-smi` and `nvcc`. Bootstrap fails rather
  than silently replacing the driver/toolkit used as decision evidence.

## Files

- `.terraform.lock.hcl` — checksums for the exactly pinned alpha provider;
- `discover.sh` — authenticated, read-only region/flavor/stock/image/key/pricebook listing;
- `main.tf` — zero-resource defaults, live selection guards, one VM, and exact `/32` rule;
- `cloud-init.sh.tftpl` — early guest firewall/poweroff timer and CUDA/Rust bootstrap;
- `prepare-console-password.sh` — Bash/OpenSSL 3 helper that reads the one-time VNC password without echo and emits only a hash export;
- `prepare-host-key.sh` — generates the ephemeral ED25519 host keypair and emits only the two exports, so the fingerprint is known before first boot;
- `run-demographic-benchmark.sh` — unattended demographic-benchmark collection and mandatory destroy (§4b);
- `remote-run-spike.sh` — one CUDA+Vulkan benchmark invocation with Hyperstack provenance;
- `collect-runs.sh` — resolves the state IP, performs bounded/backed-off SSH readiness checks, then seeds, executes, and retrieves the required three independent runs;
- `verify-artifacts.py` — rejects incomplete, unbound, wrong-device, host-key, or cross-run evidence;
- `review-paid-plan.py` — emits a credential-free allowlisted summary and hash of an exact saved plan;
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
bash -n prepare-console-password.sh
bash -n remote-run-spike.sh
bash -n discover.sh
bash -n collect-runs.sh
python3 -m py_compile verify-artifacts.py review-paid-plan.py
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

1. push the exact benchmark/infrastructure commit and set its 40-hex SHA as `repository_ref`;
2. confirm live stock, image, `/32`, and complete hourly price again;
3. choose a strong one-time VNC-console password and export only its SHA-512 crypt hash to Terraform. The helper must be launched with Bash and requires OpenSSL 3 with `passwd -6` support; it prints installation guidance rather than falling back to incompatible stock LibreSSL:

```bash
unset TF_VAR_console_password_hash
eval "$(bash ./prepare-console-password.sh)"
test -n "${TF_VAR_console_password_hash:-}"
```

Keep the plaintext only in a secure password manager until teardown. It is for the trusted VNC console account `ubuntu`; SSH password and keyboard-interactive authentication remain disabled. Keep the hash environment variable in the same authenticated shell through destroy.

4. protect local state and the saved plan. Terraform's `sensitive` marker redacts display but does **not** encrypt plan/state storage; both contain the console password hash inside sensitive user-data:

```bash
umask 077
chmod 600 terraform.tfstate terraform.tfstate.backup 2>/dev/null || true
```

5. create, inspect, and retain a saved plan without changing the non-creating values in `terraform.tfvars`:

```bash
terraform plan -var-file=terraform.tfvars \
  -var=create_instance=true \
  -var=accept_paid_creation=true \
  -out=hyperstack-paid.tfplan
python3 review-paid-plan.py hyperstack-paid.tfplan
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

Bootstrap writes start, local SSH self-test, ready, and failure diagnostics directly to the trusted Hyperstack VNC console. If interactive recovery is needed, log in there as `ubuntu` with the one-time console password; do not enable SSH passwords. Obtain the ED25519 fingerprint from the first-boot `SSH HOST KEY FINGERPRINTS` output or, after console login, run:

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

- reads the public IP from outputs or Terraform state, verifies the VNC-trusted host key, and waits for bootstrap with one bounded SSH probe and increasing backoff;
- copies the Mac-containing `RESULTS.md` into three byte-identical remote files;
- performs three same-machine runs with distinct absolute `SPIKE_RESULTS_PATH` values and collector-generated run IDs;
- preserves a separate external log whose start/completion markers bind each run ID to the exact result SHA-256;
- retrieves all results/logs plus bootstrap/SSH diagnostics, local self-test key, `nvidia-smi -q`, and exact commit;
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

## 4b. Demographic benchmark collection (unattended)

`run-demographic-benchmark.sh` reuses everything above — same VM, same host-key
trust model, same `/32` rule — with a different remote payload: it builds
`sembla-cli --features cuda` and runs `scripts/bench-demographic.sh` for both
backends, filling all four pending hardware rows of
[`docs/demographic-benchmark.md`](../../../docs/demographic-benchmark.md) from
one session. A GPU host carries both the CUDA device and the ≥32 GiB the 50M CPU
row needs, so CPU and CUDA scales come from the same machine and the same commit.

It adds **no Terraform resources.** Run it after §3's reviewed paid apply, in
place of §4's collector. With the pre-seeded host key there is no interactive
step at all — generate the key, apply, collect:

```bash
eval "$(bash prepare-host-key.sh)"       # exports the key and its fingerprint
# ... §3's reviewed paid apply, with TF_VAR_ssh_host_private_key now in scope ...
bash run-demographic-benchmark.sh        # unattended through to destroy
rm -rf "$SEMBLA_HOST_KEY_DIR"            # the key dies with the VM
```

`prepare-host-key.sh` prints the fingerprint to stderr and exports it as
`SSH_HOST_KEY_FINGERPRINT`, so the collector pins a value that existed before the
VM did. On the console-read path, export that variable by hand instead and leave
`TF_VAR_ssh_host_private_key` unset; everything downstream is identical.

Everything after that point is unattended: it waits for bootstrap, builds,
benchmarks CUDA then CPU, retrieves an evidence directory with `SHA256SUMS` and
GPU/RAM/commit provenance into `docs/evidence/demographic-bench/hyperstack-<UTC>/`,
verifies the checksums locally, then runs §5's destroy itself and **fails loudly
if any paid resource survives in state**.

The remote run is **detached** (`setsid nohup`, status in `~/bench.status`), so a
sleeping laptop, a closed lid, or a dropped network connection cannot kill hours
of GPU time. Re-running the script rejoins a run already in progress instead of
starting a second one; it only starts a new run when no benchmark PID is alive.
Before a 50M collection, check that `emergency_poweroff_hours` exceeds the
expected run time — that timer powers the guest off mid-run otherwise, and
billing continues through a `SHUTOFF` anyway. `KEEP_VM=1` opts out of the destroy;
nothing else does. Scales, ticks, and seed are overridable
(`BENCH_SCALES_CUDA`, `BENCH_SCALES_CPU`, `BENCH_TICKS`, `BENCH_SEED`); the
defaults match the committed local evidence so the rows are comparable.

The remote payload refuses a CPU scale the host cannot hold in RAM rather than
producing a measurement of the pager. The CUDA arm benchmarks the no-grouped
model only, because grouped observations are CPU-only under DECISIONS §K6 — that
restriction is the subject of a scheduled decision, not an oversight.

> [!NOTE]
> The paid apply itself still requires reviewing and approving a saved plan
> (§3). That gate is deliberate — it is the billing control, not a trust
> control — and this script does not bypass it. Everything after the apply is
> unattended, including the destroy.

## 5. Destroy immediately

After confirming every artifact is non-empty:

```bash
terraform destroy -var-file=terraform.tfvars \
  -var=create_instance=true \
  -var=accept_paid_creation=true
terraform state list
rm -f hyperstack-paid.tfplan
chmod 600 terraform.tfstate terraform.tfstate.backup 2>/dev/null || true
unset TF_VAR_console_password_hash
```

The alpha provider waits only 120 seconds for VM deletion. If destroy fails or
times out, inspect the console immediately and delete the VM there. After the
console confirms deletion, rerun `terraform destroy`/`terraform refresh`; remove
stale VM or rule addresses with `terraform state rm` only after confirming the
real resource no longer exists. The final state listing must contain no paid VM
or security-rule resource. **Do not merely stop or power off the VM: `SHUTOFF`
continues billing.** Keep local Terraform state until deletion is confirmed.

No paid Hyperstack resource currently exists. The first A100 attempt was destroyed after OpenSSH stalled before server key exchange; the replacement bootstrap and collector retain bounded recovery and console diagnostics for that failure mode.
