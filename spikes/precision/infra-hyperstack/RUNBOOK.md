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
- [ ] **Set `TF_VAR_tailscale_auth_key`** (ephemeral, pre-authorized, tagged)
      *before* the plan. This is the single highest-value item on the list: it
      removes the entire class of failure where your egress IP rotates and locks
      you out of a billing machine, which cannot be repaired by re-applying. The
      collector warns at startup if it is about to use the public path instead.
- [ ] **Confirm `ssh_cidr` matches your current egress IP**
      (`curl -s https://api.ipify.org`). A mismatch produces a TCP timeout, not
      a useful error. Fix it **now**, before the apply — afterwards it is
      `ForceNew` and correcting it destroys the run. Note the value is enforced
      in two places; see "enforced in two places" below.
- [ ] **Delete any leftover `hyperstack-paid.tfplan`.** An already-applied plan
      file is one keystroke from a duplicate VM, and plans are cheap to remake.
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
- [ ] **Decide about the evidence push.** `prepare-deploy-key.sh` makes artifact
      delivery independent of this laptop. Unset, it is silently skipped — that
      is what happened on 2026-07-28. The collector now says which mode it is in
      at startup; read that line rather than assuming.
- [ ] **Run `reconcile-orphans.sh` first, not last.** It is the only check that
      nothing is already billing, and a `terraform destroy` that returned an
      HTTP 500 is not proof it succeeded.

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
  running benchmark. To grant access to a new IP mid-run, see the section below
  — a security-group rule alone is *not* enough.

## `ssh_cidr` is enforced in two places, not one

This cost most of an evening on 2026-07-27 and the fix was not obvious, because
the first thing you try does not work.

`ssh_cidr` is applied **twice**, by two independent enforcement points:

1. the **Hyperstack security group**, created by Terraform; and
2. the **guest's own iptables**, written by cloud-init
   (`cloud-init.sh.tftpl:152-153`):

   ```sh
   iptables -I INPUT 3 -p tcp --dport 22 -s "$SSH_CIDR" -j ACCEPT
   iptables -I INPUT 4 -p tcp --dport 22 -j DROP
   ```

So when your egress IP rotates mid-run, **opening a security-group rule through
the API gets you as far as the guest, which then drops you**, with no packet
back and therefore no error worth reading. The symptom is identical before and
after the fix, which is what makes it expensive: it looks like the API rule did
not take effect, when in fact it worked perfectly and a second wall is behind it.

Both must be opened:

```sh
# 1. Security group, through the Hyperstack API (never by re-applying).
# 2. Guest firewall, from the VNC console:
sudo iptables -I INPUT 3 -p tcp --dport 22 -s <new-ip>/32 -j ACCEPT
```

The rule is inserted at position 3 so it precedes the `DROP` at 4. Appending it
with `-A` puts it after the `DROP` and changes nothing — another way to conclude
wrongly that the network is at fault.

**Prevention is much cheaper than recovery.** Set `TF_VAR_tailscale_auth_key`
before the apply and neither wall is in the path: WireGuard is UDP, needs no
inbound rule, and does not care what your public IP is. The collector now warns
loudly at startup when it is about to use the public path, so this is visible
before hours of compute are committed rather than after.

Note that an *established* session does not survive the rotation either. The
`ESTABLISHED,RELATED` accept at rule 1 keeps existing flows alive, but a changed
source IP is by definition a different flow. The remote job is detached and
keeps running regardless — what you lose is the ability to watch it, collect
from it, and tear it down.
- **The plan file is sensitive.** It contains the console password hash. Keep
  `umask 077`; never commit it (the allowlist `.gitignore` already blocks it).
- **Never delete host key files in cloud-init.** An explicit `HostKey` directive
  already makes sshd serve only the listed key. Deleting the others risks
  sshd failing per-connection after the banner — unrecoverable remotely.
- **`pkill -f <pattern>` over SSH will match its own shell.** The remote shell's
  command line contains the whole script you sent, so `pkill -f sembla_cuda-`
  kills the connection running it and you see the command truncate mid-output.
  Kill by PID or process group (`kill -TERM -"$PID"`, the payload is a `setsid`
  session leader), or exclude yourself with `pgrep -f ... | grep -v $$`. Done on
  2026-07-28; the kill still landed, but the truncated output looked like a
  dropped connection and cost a reconnect to interpret.
- **A second `trap ... EXIT` replaces the first, it does not add to it.** This
  script's `finish` trap prints the "a VM may still be billing" warning. A
  second EXIT trap added for something as small as removing a temp file silently
  disables it, and the only symptom is a warning that no longer appears — on the
  path where you most need it. Add cleanup to `finish`.

## Bounding a hang

`BENCH_CORPUS` runs under `timeout` (`BENCH_CORPUS_TIMEOUT_SECONDS`, default
1800) and exits **7** if it fires. This exists because on 2026-07-28 a GPU-side
deadlock ran for **2h31m** before anyone looked — the ceiling was the
collector's 12-hour poll, so the true exposure was about $24 of billing for a
defect whose reproduction takes 23 seconds. See `DECISIONS.md` §L12.

Two things made it invisible, and both are now fixed. The stage redirected to
its own log instead of tee-ing, so `tail -1 ~/bench.log` — the only progress the
collector shows — froze on the stage header, and a deadlock looked exactly like
a long compile. And there was no timeout.

**The general rule: any stage that can hang needs a bound and a heartbeat.**
A stage that is silent while healthy cannot be distinguished from one that is
silent while stuck, and the difference costs money by the hour.

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

### What now defends against it

**A `create` timeout is not available.** The alpha provider exposes only a
`profile` block on `hyperstack_core_virtual_machine` — there is no `timeouts`
block, so the 5-minute wait cannot be lengthened. Terraform config alone cannot
prevent this failure, which is why the compensating control below is the whole
defence rather than a backstop.

**`reconcile-orphans.sh`** compares the account against Terraform state:

```sh
bash reconcile-orphans.sh            # report only; exits 1 if orphans exist
bash reconcile-orphans.sh --delete   # delete them, with confirmation
```

Only VMs whose name starts with `name_prefix` are ever considered; everything
else in the account is reported as untouched and can never be a deletion
candidate. After deleting it re-queries the account and fails loudly if
anything remains, because a delete response is not proof.

Run it after **any** failed or interrupted apply, and before assuming a session
is finished.

**`destroy-deadline.sh` now covers the orphan case.** Previously it refused to
arm when state held no VM — precisely wrong after an apply timeout, since that
is exactly when state is empty and a machine is billing. It now:

- arms when state has a VM **or** the account has an untracked one;
- at the deadline, destroys what state knows about, then **always** reconciles
  through the API, because state going empty is not proof the account is empty.

Adopting the orphan instead of deleting it is possible when the run is still
wanted:

```sh
terraform import 'hyperstack_core_virtual_machine.gpu[0]' <ID>
```

but note the SSH security-group rule will still be missing, so the VM is
usually unreachable on its public IP even once adopted.

## Failure playbook

| Symptom | Cause | Action |
|---|---|---|
| `Missing API token` | secrets not in this shell | re-export; they do not survive a new shell |
| Plan review: `0o644 exposes sensitive user_data` | forgot `umask 077` | `umask 077`, re-plan |
| `No global public IPv4 ... within the timeout` | IP not attached yet | collector now polls the provider API; or pass `PUBLIC_IP_OVERRIDE=<ip>` |
| SSH: `Permission denied (publickey)` | key not in agent | `ssh-add`; verify `ssh-add -l` |
| SSH: closes after exactly ~30s | path cannot carry a session | change network (hotspot); see above |
| SSH: `Operation timed out` | `ssh_cidr` no longer matches your egress IP | check `curl https://api.ipify.org`; open **both** the security group and the guest iptables rule (see "enforced in two places"); do **not** re-apply |
| Collector prints `SSH to ... failed (n/5)` | the path died mid-run | the message names the likely causes in order; the remote job is unaffected |
| Remote: `could not convert string to float` | GNU-time parser bug | fixed in `f81fef9`; ensure the VM is on that commit or later |

The remote payload is **detached** (`setsid nohup`, status in `~/bench.status`).
A dropped connection loses nothing: re-running the collector rejoins a run in
progress rather than starting a second one. Check state directly with
`cat ~/bench.status` and `tail ~/bench.log` on the VM.

## Known-good sequence

```bash
cd spikes/precision/infra-hyperstack
bash reconcile-orphans.sh                  # nothing should be billing yet
rm -f hyperstack-paid.tfplan               # a consumed plan is a duplicate VM
ssh-add ~/.ssh/sembla_hyperstack           # passphrase prompt
eval "$(bash prepare-host-key.sh)"         # optional pre-seeded host key
export HYPERSTACK_API_KEY=...
export TF_VAR_tailscale_auth_key=...       # ephemeral, pre-authorized, tagged
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

### Verifying a device-observation session

Set both optional stages. They run before the frozen gate and finish in minutes,
so the interesting answer is readable in `~/bench.log` long before the gate ends:

```bash
BENCH_CORPUS=1 BENCH_PROFILE=1 bash run-demographic-benchmark.sh 2>&1 \
  | tee ~/bench-driver.log
```

Order matters and is deliberate. The corpus runs **first** and aborts the run on
CPU/CUDA disagreement, because timing a wrong arm produces numbers that look
like evidence. The profile then measures both the no-grouped configuration —
comparable to every earlier phase table — and the grouped one, which is what the
calibration workflow actually uses and which CUDA rejected outright until
`prds-device-observation/0002`.

Read in this order when it finishes:

| file | question it answers |
|---|---|
| `differential-corpus/exit-code.txt` | does CUDA still agree with the CPU oracle? |
| `profile/grouped-parity.txt` | do the backends agree on grouped views at 5M rows? |
| `profile/timing-grouped-cuda.json` | did `state_transfer` + `state_reconstruct` collapse? |
| `profile/timing-cuda.json` | the same for the comparable no-grouped case |
| `profile/profile-grouped-cuda.stderr` | per-view key-space, occupied and emitted group counts |

If only the no-grouped table improves, `0002` delivered nothing for the driver
model: eligibility is all-or-nothing per run, so a model with any ineligible
view downloads the whole state regardless.

Run the collector under `tmux` if driving from a phone or an unreliable link:
the *remote* job survives disconnection, but the local driver — which performs
collection and teardown — does not.

## Focused CUDA readback/contended-kernel diagnostic

Use this self-contained measurement stage before changing CUDA readback or
kernel scheduling:

```bash
BENCH_CUDA_READBACK_DIAGNOSTIC=1 \
  bash run-demographic-benchmark.sh 2>&1 | tee ~/bench-driver.log
```

The stage fixes the binding case at 10M slots, 24 ticks, seed 9009, independent
noise, and grouped observations. It records:

- one profiler-independent CUDA `run` with per-tick phase timing, including
  `readback_control`;
- equal four-draw Nsight Systems arms at `--draw-workers 1` and `4`, exporting
  CUDA GPU trace, kernel summary, and API summary CSVs;
- D2H call/byte/union/overlap statistics and per-kernel concurrent duration
  penalties in `cuda-readback-diagnostic/analysis.json`; and
- bounded Nsight Compute reports: SOL/launch/occupancy for the three kernels
  with the largest Systems penalty, plus memory/scheduler/warp detail for at
  most two kernels.

Nsight Systems decides real duration and concurrency. Nsight Compute runs only
serial one-worker diagnostic launches because replay serializes kernels and
changes cache/scheduling; its durations must not be used as end-to-end timing.
The CUDA 12.8 image's bundled Nsight Compute 2025.1.1 injection shim lacks a
driver symbol that `cudarc` 0.17.6 resolves at startup. The stage therefore
pins Debian package `nsight-compute-2025.2.1=2025.2.1.3-1`, installs it
from the configured NVIDIA package repository when absent, and records the
exact package, repository policy/source, binary metadata/hash, and tool
version. The stock R570 image restricts GPU counters to administrators, and a
file capability on `ncu` does not propagate to its injected target. Before any
diagnostic CUDA workload, the stage therefore stops persistence, unloads only
loaded NVIDIA modules, and uses NVIDIA's documented temporary regkey method to
reload the driver with `NVreg_RestrictProfilingToAdminUsers=0`. Both `ncu` and
Sembla then run unprivileged, and the collector rejects any residual file
capability on the profiler. After the last profiler launch—or from scoped
`ERR`, `TERM`, `INT`, and `EXIT` failure handlers—the stage reloads the idle
driver with admin-only access. Every privileged service/module operation has a
TERM/KILL bound; the active-mode flag is cleared only after the loaded parameter
is verified as admin-only. Module/driver parameters are recorded before,
during, and after the window, and partial evidence records restoration success
or failure. A transient operator probe motivated this method but is not accepted
evidence; the next retained run must itself prove the active parameter, the
unprivileged metric collection, and admin-only restoration.

Every profiler launch has a TERM deadline and a subsequent KILL deadline;
report imports are bounded separately. Raw `.nsys-rep` and `.ncu-rep` files are
retained alongside their CSV exports. If the stage fails, the payload packages
a checksummed partial diagnostic tree—excluding the large generated state—and
the local driver retrieves and verifies it before returning failure.

The stage fails if `nsys`, passwordless `sudo`, module reload tooling, the exact
profiler package, or temporary counter access is unavailable, if equal-work
kernel counts differ, if four streams are not observed, or if any selected
Compute report is
missing. It writes machine assertions and skips both the frozen §L4 gate and
the full concurrency matrix.

`BENCH_CUDA_READBACK_DIAGNOSTIC=1` is mutually exclusive with every other
`BENCH_*` stage selector and is baked into the detached payload hash. It needs a
paid GPU host. The normal checksum transfer, evidence push, mandatory Terraform
destroy, watchdog, and provider reconciliation rules remain unchanged.

## Supported concurrent CUDA sweep stage

Use the production-interface stage for this PRD's deferred GPU criteria:

```bash
BENCH_CONCURRENCY_SPIKE=1 \
BENCH_CONCURRENCY_SUPPORTED=1 \
BENCH_CONCURRENCY_SPIKE_ONLY=1 \
  bash run-demographic-benchmark.sh 2>&1 | tee ~/bench-driver.log
```

`BENCH_CONCURRENCY_SUPPORTED=1` drives `sweep --draw-workers 1/2/4` directly;
it does not export the hidden worker, free-stream, lockstep, or fused controls.
At both 1M and 10M slots it runs three independent-noise repetitions and one
CRN repetition, compares complete output trees to the sequential arm, exports
pairs and grouped sidecars, and proves the comparator rejects a deliberate
perturbation. The 1M arm forces a completion inversion and records an Nsight
Systems trace for two non-default streams. The 10M arm requests 20 workers to
exceed the conservative device-memory bound and requires the preflight error,
a nonzero exit, and no scientific output directory.

The supported stage is self-contained and cannot be combined with
`BENCH_CONCURRENCY_LOCKSTEP`, `BENCH_CONCURRENCY_FREE_STREAMS`,
`BENCH_CONCURRENCY_FUSED`, or `BENCH_CONCURRENCY_CRN`. It still requires
`BENCH_CONCURRENCY_SPIKE=1` and may be paired with
`BENCH_CONCURRENCY_SPIKE_ONLY=1` to skip the unrelated frozen §L4 gate. Running
it requires the paid GPU host; local CUDA compilation is not GPU evidence.

## Concurrent sweep-draw spike stage

Set both flags below to collect only the direct CUDA concurrency experiment,
without paying to rerun the unrelated frozen §L4 gate:

```bash
BENCH_CONCURRENCY_SPIKE=1 \
BENCH_CONCURRENCY_LOCKSTEP=1 \
BENCH_CONCURRENCY_SPIKE_ONLY=1 \
  bash run-demographic-benchmark.sh 2>&1 | tee ~/bench-driver.log
```

The stage runs worker counts 1, 2, and 4 at 1M and 10M slots, with three
repetitions per count, 20 draws, 24 ticks, independent noise, exported pairs,
and grouped observations. Each repetition compares the complete output tree to
its sequential reference. With `BENCH_CONCURRENCY_LOCKSTEP=1`, lane groups use
explicitly non-blocking streams and wait at every tick boundary. The stage also
runs a deliberately failing comparator control, requires deterministic `k % workers` lane assignment and overlapping
per-round lane timing intervals, records RSS/VRAM/utilization samples, and exports a 1M Nsight Systems
CUDA trace as CSV for actual overlap analysis.

`BENCH_CONCURRENCY_SPIKE_ONLY=1` and `BENCH_CONCURRENCY_LOCKSTEP=1` are each
rejected unless the stage itself is enabled. All three flags are included in the
immutable remote payload hash. The production sweep default remains sequential;
only child benchmark processes receive the hidden experimental environment
variables. Omit `BENCH_CONCURRENCY_LOCKSTEP` only to reproduce the already
recorded independent/default-stream lower-bound arm.

## Retained-backend sweep stage

Set `BENCH_SWEEP=1` only when collecting PRD sweep-throughput evidence. The
stage is additive, runs before the frozen gate, and is expected to be expensive:
it performs baseline and current CPU/CUDA sweeps at both 1M and 10M slots, each
with 20 sequential draws and 24 ticks. Budget the paid session accordingly.

A full baseline commit is mandatory. The collector creates a detached worktree
at that commit and builds its own release CUDA binary; it never switches or
dirties the evidence checkout.

```bash
BENCH_SWEEP=1 \
BENCH_SWEEP_BASELINE_COMMIT=c0acc2c03d0676750178686c029ffc0ecdadc0ea \
  bash run-demographic-benchmark.sh 2>&1 | tee ~/bench-driver.log
```

Artifacts are under `sweep/<scale>/`, with shared state/model hashes, complete
output directories, whole-process `/usr/bin/time` records, externally observed
draw-timing JSON for every arm, and supplementary current `sweep-timing-v1`
JSON. The external format uses the same completed-file boundary for before and
after and contains draw 0, every later draw, and total wall time; the native
current format also records backend setup explicitly. Root-level
baseline/current binary hashes and the resolved baseline commit make the
comparison reproducible.

For each scale, `cpu-cuda-parity.txt` is written only after requiring identical
file sets and comparing every file: draw CSVs, grouped sidecars, summaries,
`summary.csv`, `manifest.csv`, and `run-manifest.json`. The manifest's existing
backend identity is the sole normalized field; all scientifically meaningful
bytes remain exact. The same whole-tree comparison is required for the baseline
CPU/CUDA pair. Before trusting the comparator, the stage makes a byte-identical
copy of the 1M current-CPU tree, perturbs only one grouped sidecar, and requires
comparison with its unmodified source to fail. `negative-control.txt` records
that isolated result. A missing sidecar or a comparator that accepts the
perturbation aborts the stage.

Interpret whole-sweep wall time as the headline. Compare draw 0 with the median
later draw in both arms to verify that setup moves from every baseline draw to
draw zero only. Because the baseline binary predates `--timing-json`, one Python
wrapper observes each completed `draw_N.csv` at 3 ms intervals for **all four
arms** and writes `{baseline,current}-{cpu,cuda}-<scale>.draw-timing.json`. This
keeps before/after timing boundaries identical, does not alter the baseline
binary, and does not infer draw boundaries from an average. Native current
`*-native-timing.json` remains useful for separating setup from draw execution
but is not compared directly with the external baseline timings.

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

