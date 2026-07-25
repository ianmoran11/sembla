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
      `TF_VAR_ssh_host_private_key`. All three must be in the *same* shell as
      the plan, the apply, and the collector.
- [ ] **`umask 077` before `terraform plan`.** The plan file embeds the console
      password hash inside user-data; `review-paid-plan.py` refuses a `0644`
      plan, and it is right to.
- [ ] **`emergency_poweroff_hours` exceeds the expected run time.** It defaults
      low. A guest poweroff mid-run loses the work *and* keeps billing.
- [ ] **Arm the billing watchdog** (`destroy-deadline.sh arm <hours>`) before
      starting anything long.

## The network requirement

**A path that passes `ssh-keyscan` can still be unable to carry an SSH session.**

On 2026-07-25 three separate VMs were unreachable from a home ADSL/NBN
connection: `ssh-keyscan` succeeded every time while authenticated SSH stalled
for exactly 30 seconds and then died. That 30 seconds is `LoginGraceTime`
expiring server-side. `sshd` logged connections closing at `[preauth]` while the
client logged the server closing — each end blaming the other, which is the
signature of a middlebox silently dropping the flow. A traceroute showed a
private hop (`10.20.22.53`) inside the ISP, i.e. CGNAT or a tunnel.

The same code, same region, same image, over a phone hotspot on cellular:
connected in **2 seconds**.

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
defence in depth; it did **not** fix the ISP path.

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

## Timing, cost, and capacity

Measured on `n3-H100x1` (H100 PCIe 80GB, 28 vCPU, 177 GiB RAM), CANADA-1:

| Phase | Duration |
|---|---|
| `terraform apply` (VM ACTIVE) | ~2m45s |
| Floating IP attach after ACTIVE | seconds to minutes — **poll, don't assume** |
| cloud-init bootstrap to `ready` | ~2 minutes |
| `cargo build --release --features cuda` | ~13s warm, few minutes cold |
| Full benchmark, both backends, 10M + 50M | ~4–5 hours |

At $2.50672/hr that is roughly **$12** for a complete run. Discovery, planning,
and `terraform destroy` are free.

Capacity notes: the 50M CPU row needs ~20 GiB by the collector's budget
(400 B/slot + 2 GiB); the H100 flavor's 177 GiB is ample. A state artifact is
48 bytes/slot exactly — 2.24 GiB at 50M.

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
```

Run the collector under `tmux` if driving from a phone or an unreliable link:
the *remote* job survives disconnection, but the local driver — which performs
collection and teardown — does not.

## Planned improvement: collect through GitHub, not SSH

The current design needs a live SSH session at the *end* of a multi-hour run to
retrieve artifacts. That couples a 5-hour job to 5 hours of stable connectivity,
which is the wrong dependency — and on 2026-07-25 it was the binding constraint,
not compute.

The better design: the remote payload commits its evidence directory to the
repository and pushes, so collection is `git pull` from anywhere and the local
driver becomes optional. Sketch:

- Provision a **fine-grained, contents-write, single-repo** deploy token or
  deploy key, passed through `TF_VAR_*` into user-data the way the host key is;
  never into tfvars or state beyond what user-data already carries.
- The payload commits to a dedicated branch (`evidence/hyperstack-<UTC>`), never
  to `main`, so a failed or partial run cannot disturb the trunk.
- Push after `SHA256SUMS` is written, so the pushed tree is self-verifying.
- Keep the local driver for teardown, but let it exit cleanly once the push is
  confirmed; the watchdog remains the billing backstop either way.
- The VM must still be destroyed — a push does not stop billing.

This also removes the mobile-data cost of pulling artifacts over a hotspot, and
makes an unattended overnight run genuinely unattended.
