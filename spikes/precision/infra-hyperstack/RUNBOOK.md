# Hyperstack demographic-benchmark runbook

Operating notes for `run-demographic-benchmark.sh`, written after the first
end-to-end attempt on 2026-07-25. Everything here is a fact that cost time or
money to learn. Read the preflight before spending anything.

The module README remains authoritative for provisioning, security posture, and
teardown; this file covers *operating* the benchmark collection specifically.

## Preflight checklist

Each item below has failed at least once in practice.

- [ ] **`ssh-add` the key.** `~/.ssh/sembla_hyperstack` is passphrase-protected.
      The collector uses `BatchMode=yes` and therefore can never prompt: without
      an agent-loaded key every connection ends in `Permission denied
      (publickey)`, no matter how healthy the VM is. Run `ssh-add
      ~/.ssh/sembla_hyperstack`, confirm with `ssh-add -l`, and make sure the
      shell running the collector inherits `SSH_AUTH_SOCK`.
- [ ] **Check the network path can carry an SSH session.** See "The network
      requirement" below. This is the single most expensive failure mode.
- [ ] **Confirm `ssh_cidr` matches your current egress IP**
      (`curl -s https://api.ipify.org`). A mismatch produces a TCP timeout, not
      a useful error.
- [ ] **Secrets available**: `HYPERSTACK_API_KEY`,
      `TF_VAR_console_password_hash`, and — if using the pre-seeded host key —
      `TF_VAR_ssh_host_private_key`. If pushing evidence to GitHub, also
      `TF_VAR_evidence_deploy_key` (see "Evidence push"). All must be in the
      *same* shell as the plan, the apply, and the collector.
- [ ] **`umask 077` before `terraform plan`.** The plan file embeds the console
      password hash — and the evidence deploy key, if set — inside user-data;
      `review-paid-plan.py` refuses a `0644` plan, and it is right to.
- [ ] **`emergency_poweroff_hours` exceeds the expected run time.** It defaults
      low. A guest poweroff mid-run loses the work *and* keeps billing.
- [ ] **Arm the billing watchdog** (`destroy-deadline.sh arm <hours>`) before
      starting anything long.

## The network requirement

**A path that passes `ssh-keyscan` can still be unable to carry an SSH session.**

On 2026-07-25 three separate VMs were unreachable from one workstation's home
network: `ssh-keyscan` succeeded every time while authenticated SSH stalled for
exactly 30 seconds and then died. That 30 seconds is `LoginGraceTime` expiring
server-side. `sshd` logged connections closing at `[preauth]` while the client
logged the server closing — each end blaming the other.

The same code, same region, same image, over a phone hotspot on cellular:
connected in **2 seconds**.

**The cause is not confirmed, and it is not provider-specific.** Over the same
home network, `ssh -T git@github.com` fails identically on port 22 *and* 443,
while raw TCP to those hosts carries the banner and the server's KEXINIT intact.
So connections establish and only the key exchange dies — which points at local
packet filtering (a VPN client's kill switch, for instance) at least as strongly
as at the ISP. Rule out local software before blaming the network: quit any VPN
client and retry a plain `ssh -T git@github.com`.

Diagnosis order, cheapest first:

1. `ssh-keyscan -T 8 -t ed25519 <ip>` — if this works but SSH does not, suspect
   the path, not the host.
2. Time the failure. **Exactly ~30s means `LoginGraceTime`**, i.e. the handshake
   stalled rather than being refused. Instant failure means something else.
3. `ssh -vvv` and find the last packet type sent. Stalling after
   `SSH2_MSG_KEX_ECDH_INIT` implicates large packets.
4. Read `/var/log/auth.log` on the VNC console. This is the only source of the
   server's own account and it settles the question in one line.

The collector forces `KexAlgorithms=curve25519-sha256` because OpenSSH 9.x
defaults to the `sntrup761` hybrid, whose client public key is ~1200 bytes — a
single packet large enough to vanish on a path with an MTU problem. This is
defence in depth; it did **not** fix the failing home network.

## Reaching the VM over Tailscale (recommended)

Set `TF_VAR_tailscale_auth_key` and the guest joins your tailnet as
`sembla-bench` during bootstrap. The collector then finds it automatically via
`tailscale status` and connects over WireGuard instead of the public IP.

Why this is the better path:

- **It does not need your ISP to carry an SSH session.** WireGuard is UDP, so a
  network that mangles TCP SSH is irrelevant.
- **No `/32` rule to maintain**, and nothing breaks when your public IP rotates
  mid-run — the failure that stranded a run on 2026-07-25.
- **Nothing new is trusted.** `tailscale up --ssh=false` deliberately does *not*
  enable Tailscale SSH: authentication stays with the pinned host key and your
  keypair. The tailnet is transport only.

Use an **ephemeral, pre-authorized, tagged** auth key so the node removes itself
from your tailnet when the VM dies. The guest also opens tcp/22 on `tailscale0`
— without that the tailnet path is established but unusable, because the guest
firewall drops port 22 from anything but the operator `/32`.

Override the node name with `TAILSCALE_NODE=<name>`; force the public path with
`PUBLIC_IP_OVERRIDE=<ip>`.

### Retrofitting a VM that is already running

A VM created without the auth key can still be pulled onto the tailnet from the
VNC console, which is useful for rescuing a run when SSH is unavailable:

```bash
curl -fsSL https://tailscale.com/install.sh | sudo sh
sudo tailscale up --hostname=sembla-bench --ssh=false
sudo iptables -I INPUT 3 -i tailscale0 -p tcp --dport 22 -j ACCEPT
```

`tailscale up` without `--authkey` prints a login URL — open it on a phone and
approve. That avoids typing a long key into a VNC console.

## Things that must not be changed casually

- **`ssh_cidr` is part of the VM's identity.** It feeds the rendered bootstrap,
  whose hash forms the VM name, which is `ForceNew`. Changing `ssh_cidr` in
  `terraform.tfvars` and applying will **destroy and recreate the VM**, losing a
  running benchmark. To grant access to a new IP mid-run, add a security-group
  rule through the Hyperstack API and reconcile Terraform state at teardown.
- **The plan file is sensitive.** It contains the console password hash. Keep
  `umask 077`; never commit it (the allowlist `.gitignore` already blocks it).
- **Never delete host key files in cloud-init.** An explicit `HostKey` directive
  already makes sshd serve only the listed key. Deleting the others risks
  sshd failing per-connection after the banner — unrecoverable remotely.

## PRD 0005 frozen protocol

`run-demographic-benchmark.sh` now implements only the frozen §L4 collection:
10,000,000 slots, 24 ticks, seed 9009, three replicates per backend, and the
no-grouped model. It refuses the old `BENCH_SCALES_*`, `BENCH_TICKS`, and
`BENCH_SEED` overrides rather than allowing an accidental non-gate run. There is
no 50M row in this collection.

The remote payload builds and hashes one release binary, synthesizes and hashes
one state artifact, and interleaves three CUDA and three CPU no-grouped runs
against those exact paths. It aborts if the repository commit, binary, or state
hash changes, or if any gate output differs across backend or replicate. Three
paired CPU full/no-ageing runs report the ageing-share median and spread without
making the §K2 decision.

The retrieved directory contains raw timing/output files, `bench-results.json`,
`bench-results.md`, a verdict `README.md`, separate GPU/CPU/RAM provenance,
explicit assertion results, and `SHA256SUMS`. The collector requires a new local
evidence directory, preserves the remote manifest as `SHA256SUMS.remote`, and
verifies it before destroying the VM; a final manifest then covers the remote
evidence plus local collection and teardown logs. Resumption uses an immutable,
content-hashed remote payload and refuses to overwrite a different live or
completed payload. Do not edit the generated verdict to make a failed gate pass;
a ratio below 3× is a complete result.

## Timing, cost, and capacity

Measured provisioning timings on `n3-H100x1` (H100 PCIe 80GB, 28 vCPU, 177 GiB
RAM), CANADA-1:

| Phase | Duration |
|---|---|
| `terraform apply` (VM ACTIVE) | ~2m45s |
| Floating IP attach after ACTIVE | seconds to minutes — **poll, don't assume** |
| cloud-init bootstrap to `ready` | ~2 minutes |
| `cargo build --release --features cuda` | ~13s warm, few minutes cold |
| Frozen §L4 benchmark | budget roughly 1–2 hours; the collector timeout is 12h |

At $2.50672/hr, budget roughly **$3–$5** plus any public-IP charge for the
expected run. Discovery, planning, and `terraform destroy` are free. The 10M
state is 48 bytes/slot (about 458 MiB), and the H100 flavor's 177 GiB host RAM is
ample. Keep the eight-hour emergency timer and the independent destroy watchdog;
neither a guest poweroff nor a benchmark failure stops billing.

## The apply-timeout orphan (2026-07-26)

**The most expensive failure this module has.** It happened, and the recovery
is worth knowing before it happens again.

`terraform apply` waits 5 minutes for the VM to reach a stable state. A slow
spot provision exceeds that, so Terraform errors *after* the VM was created:

```text
Error: Waiting for state change error
Timeout 5m0s reached waiting for resource state change
```

The VM is then **ACTIVE and billing but absent from Terraform state**, which
means:

- `terraform destroy` finds nothing to destroy;
- `destroy-deadline.sh` refuses to arm — it checks `terraform state list`, so
  the billing watchdog does **not** cover this VM;
- the SSH security-group rule was never created either, so the machine is
  usually unreachable on its public IP even though it is running and charging.

**Never re-run `terraform apply` here.** It creates a *second* VM rather than
adopting the first.

### Recovery

```sh
# 1. Find it. The name matches name_prefix + the bootstrap fingerprint.
curl -sS https://infrahub-api.nexgencloud.com/v1/core/virtual-machines \
  -H "api_key: $HYPERSTACK_API_KEY" -H 'Accept: application/json' \
| python3 -c "
import json,sys
for v in json.load(sys.stdin).get('instances',[]):
    print(v.get('id'), v.get('name'), v.get('status'), v.get('floating_ip'))
"

# 2. Delete by id.
curl -sS -X DELETE https://infrahub-api.nexgencloud.com/v1/core/virtual-machines/<ID> \
  -H "api_key: $HYPERSTACK_API_KEY" -H 'Accept: application/json'

# 3. Verify. Do not trust the delete response.
#    Re-run step 1 and require an empty instance list.
```

Then delete the consumed `hyperstack-paid.tfplan`: a stale plan file that has
already been applied is one keystroke away from a duplicate VM.

### Not yet fixed

Two changes would turn this from an orphan into a delay, and neither is done:

1. A `timeouts { create = "15m" }` block on
   `hyperstack_core_virtual_machine.gpu` so a slow spot provision does not
   error at all.
2. A post-apply reconciliation step: if state holds no VM, query the API for
   one matching `name_prefix` and either import it or delete it. Right now the
   operator is the only thing standing between a timeout and an untracked
   billing machine.

## Failure playbook

| Symptom | Cause | Action |
|---|---|---|
| `Missing API token` | secrets not in this shell | re-export; they do not survive a new shell |
| Plan review: `0o644 exposes sensitive user_data` | forgot `umask 077` | `umask 077`, re-plan |
| `No global public IPv4 ... within the timeout` | IP not attached yet | collector now polls the provider API; or pass `PUBLIC_IP_OVERRIDE=<ip>` |
| SSH: `Permission denied (publickey)` | key not in agent | `ssh-add`; verify `ssh-add -l` |
| SSH: closes after exactly ~30s | path cannot carry a session | change network (hotspot); see above |
| SSH: `Operation timed out` | `ssh_cidr` no longer matches your egress IP | check `curl https://api.ipify.org`; add an API-side rule, do **not** re-apply |
| Remote: `could not convert string to float` | GNU-time parser bug | fixed in `f81fef9`; ensure the VM is on that commit or later |

The remote payload is **detached** (`setsid nohup`, status in `~/bench.status`).
A dropped connection loses nothing: re-running the collector rejoins a run in
progress rather than starting a second one. Check state directly with
`cat ~/bench.status` and `tail ~/bench.log` on the VM.

## Known-good sequence

```bash
cd spikes/precision/infra-hyperstack
ssh-add ~/.ssh/sembla_hyperstack           # passphrase prompt
eval "$(bash prepare-host-key.sh)"         # optional pre-seeded host key
export HYPERSTACK_API_KEY=...
eval "$(bash prepare-console-password.sh)"
umask 077
# confirm ssh_cidr matches: curl -s https://api.ipify.org
terraform plan -var-file=terraform.tfvars \
  -var=create_instance=true -var=accept_paid_creation=true \
  -out=hyperstack-paid.tfplan
python3 review-paid-plan.py hyperstack-paid.tfplan     # human approval gate
terraform apply hyperstack-paid.tfplan
bash destroy-deadline.sh arm 7
bash run-demographic-benchmark.sh 2>&1 | tee ~/bench-driver.log
# On success, verify the new hyperstack-l4-<UTC>/SHA256SUMS once more before
# using bench-results.json to update the verdict documents.
```

Run the collector under `tmux` if driving from a phone or an unreliable link:
the *remote* job survives disconnection, but the local driver — which performs
collection and teardown — does not.

## Evidence push: collect through GitHub, not SSH

**Implemented 2026-07-26.** Previously the design needed a live SSH session at
the *end* of a multi-hour run to retrieve artifacts, coupling a 5-hour job to
5 hours of stable connectivity. On 2026-07-25 that, not compute, was the
binding constraint.

The remote payload now commits its evidence directory to an `evidence/<UTC>`
branch and pushes, so collection is a `git fetch` from anywhere and the local
driver becomes insurance rather than a dependency.

### One-time setup

1. `eval "$(bash prepare-deploy-key.sh)"` — generates a fresh ED25519 deploy
   key, prints the public half, and exports `TF_VAR_evidence_deploy_key`.
2. Register the printed public key at
   `https://github.com/<owner>/<repo>/settings/keys/new` with **Allow write
   access** ticked.
3. **Enable branch protection on `main`.** GitHub deploy keys cannot be scoped
   to a branch, so protection on the trunk is the only thing preventing a
   compromised VM from touching it. The payload refuses to push to `main` or
   `master` and only ever pushes `evidence/*`, but that is the payload policing
   itself; branch protection is the control that does not depend on the VM.
4. Delete the deploy key from GitHub once the session's evidence is merged.
   Keys are per-session by design — re-running the script makes a new one.

Leave `TF_VAR_evidence_deploy_key` unset to disable the push entirely; the SSH
collection path is unchanged and remains the primary route.

### Properties worth knowing

- The push happens **after** `SHA256SUMS` is written and verified, so the
  pushed tree is self-verifying.
- The branch is an **orphan**: evidence is an artifact, not a change to the
  source tree, and a detached history cannot conflict with trunk.
- The payload **scans for credential-shaped strings** before pushing and
  refuses if it finds any. A push is irreversible.
- A failed push does **not** fail the run. The tarball stays on disk and SSH
  collection still works; the failure is reported loudly instead.
- GitHub's SSH host keys are **pinned** in cloud-init. Trust-on-first-use here
  would undo the reason the VM's own host key is pre-seeded.
- **A push does not stop billing.** The VM must still be destroyed.

### What this does and does not make safe

It makes *artifact delivery* independent of your laptop. It does not make
teardown independent of your laptop — that is still `terraform destroy`, run
either by the driver or by `destroy-deadline.sh`.

The watchdog was also hardened on 2026-07-26: it now polls an absolute
deadline instead of sleeping for a duration, and holds a `caffeinate -s`
assertion while armed. Before that change, a watchdog armed at bedtime would
not fire, because macOS suspends a sleeping process when the machine sleeps —
an overnight run would have billed until the machine next woke.

Even so, prefer to run when you are awake. Every session so far has produced
at least one surprise needing a decision.

