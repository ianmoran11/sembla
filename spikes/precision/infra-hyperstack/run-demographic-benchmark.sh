#!/usr/bin/env bash
# Unattended demographic-benchmark collection on the provisioned Hyperstack GPU VM.
#
# This script assumes the VM already exists via this module's reviewed paid-apply
# path. It adds no Terraform resources and changes no provisioning decision: the
# host-key trust model, the /32 SSH rule, and the mandatory destroy are inherited
# from `collect-runs.sh` and the module README.
#
# It fills the four pending rows of docs/demographic-benchmark.md from one VM:
# a GPU host has both the CUDA device and the >=32 GiB of RAM the 50M CPU row
# requires, so CPU and CUDA scales are collected in a single session.
#
# Required environment:
#   HYPERSTACK_API_KEY          - needed only for the terraform destroy at the end
#   SSH_HOST_KEY_FINGERPRINT    - SHA256:... read from the Hyperstack VNC console
# Optional environment:
#   SSH_PRIVATE_KEY_PATH        - default ~/.ssh/sembla_hyperstack
#   BENCH_SCALES_CUDA           - default 10000000,50000000
#   BENCH_SCALES_CPU            - default 10000000,50000000
#   BENCH_TICKS / BENCH_SEED    - default 24 / 9009 (match the local evidence)
#   ARTIFACT_DIR                - default docs/evidence/demographic-bench/hyperstack-<UTC>
#   TFVARS_FILE                 - default terraform.tfvars
#   KEEP_VM=1                   - skip the automatic destroy (billing continues)
#   BOOTSTRAP_TIMEOUT_SECONDS   - default 1800
#   BENCH_TIMEOUT_SECONDS       - default 43200 (12h) for the whole remote run
#
# The remote run is detached, so a dropped connection loses nothing: re-running
# this script rejoins the run in progress rather than starting a second one.
# Check that the VM's emergency poweroff timer (emergency_poweroff_hours)
# exceeds the expected run time before starting a 50M collection.
set -Eeuo pipefail

MODULE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$MODULE_DIR/../../.." && pwd)"
SSH_PRIVATE_KEY_PATH="${SSH_PRIVATE_KEY_PATH:-$HOME/.ssh/sembla_hyperstack}"
KNOWN_HOSTS_FILE="$MODULE_DIR/.hyperstack_known_hosts"
TFVARS_FILE="${TFVARS_FILE:-terraform.tfvars}"
# Note the ":-" is deliberately absent: an EMPTY value must mean "skip this
# backend", not "use the default". With ":-" it silently ran the full list.
BENCH_SCALES_CUDA="${BENCH_SCALES_CUDA-10000000,50000000}"
BENCH_SCALES_CPU="${BENCH_SCALES_CPU-10000000,50000000}"
BENCH_TICKS="${BENCH_TICKS:-24}"
BENCH_SEED="${BENCH_SEED:-9009}"
BOOTSTRAP_TIMEOUT_SECONDS="${BOOTSTRAP_TIMEOUT_SECONDS:-1800}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
ARTIFACT_DIR="${ARTIFACT_DIR:-$REPO_ROOT/docs/evidence/demographic-bench/hyperstack-$STAMP}"

DESTROYED=false
REMOTE_SCRIPT=""
finish() {
  local status=$?
  [[ -n "$REMOTE_SCRIPT" ]] && rm -f "$REMOTE_SCRIPT"
  if [[ "$DESTROYED" != true ]]; then
    printf '\n%s\n' "IMPORTANT: a Hyperstack VM may still exist. Billing continues in SHUTOFF as well as ACTIVE." >&2
    printf '%s\n' "Destroy it now:  cd $MODULE_DIR && terraform destroy -var-file=$TFVARS_FILE" >&2
    printf '%s\n' "Then confirm in the Hyperstack console that no VM remains." >&2
  fi
  exit "$status"
}
trap finish EXIT

: "${SSH_HOST_KEY_FINGERPRINT:?set this to the SHA256 host-key fingerprint independently verified in the Hyperstack console/VNC}"
if [[ ! "$SSH_HOST_KEY_FINGERPRINT" =~ ^SHA256:[A-Za-z0-9+/]+={0,2}$ ]]; then
  echo "SSH_HOST_KEY_FINGERPRINT must look like SHA256:..." >&2
  exit 2
fi
if [[ ! -f "$SSH_PRIVATE_KEY_PATH" ]]; then
  echo "SSH private key not found: $SSH_PRIVATE_KEY_PATH" >&2
  exit 2
fi
for scales in "$BENCH_SCALES_CUDA" "$BENCH_SCALES_CPU"; do
  if [[ -n "$scales" && ! "$scales" =~ ^[1-9][0-9]*(,[1-9][0-9]*)*$ ]]; then
    echo "invalid scale list: $scales" >&2
    exit 2
  fi
done
if [[ -z "$BENCH_SCALES_CUDA" && -z "$BENCH_SCALES_CPU" ]]; then
  echo "nothing to do: both scale lists are empty" >&2
  exit 2
fi

cd "$MODULE_DIR"

# Hyperstack assigns the public IP a moment AFTER the VM is created, and a
# refresh-only operation with non-creating variables can omit the conditional
# output (module README, alpha-provider constraints). Resolve with retries --
# output, then floating_ip in state, then one refresh -- because failing here
# strands a VM that is already billing.
resolve_public_ip() {
  local ip
  ip="$(terraform output -raw public_ip 2>/dev/null || true)"
  if [[ -z "$ip" || "$ip" == "null" ]]; then
    ip="$(terraform show -json 2>/dev/null | python3 -c 'import json,sys
try:
    state = json.load(sys.stdin)
except Exception:
    raise SystemExit
modules = [state.get("values", {}).get("root_module", {})]
while modules:
    module = modules.pop()
    for resource in module.get("resources", []):
        if resource.get("address") == "hyperstack_core_virtual_machine.gpu[0]":
            print(resource.get("values", {}).get("floating_ip") or "")
            raise SystemExit
    modules.extend(module.get("child_modules", []))' || true)"
  fi
  printf '%s' "$ip"
}

is_tailnet_ipv4() {
  python3 -c 'import ipaddress,sys
try:
    a = ipaddress.ip_address(sys.argv[1])
except ValueError:
    raise SystemExit(1)
raise SystemExit(0 if a.version == 4 and a in ipaddress.ip_network("100.64.0.0/10") else 1)' "$1" 2>/dev/null
}

is_usable_ssh_target() {
  is_global_ipv4 "$1" || is_tailnet_ipv4 "$1"
}

is_global_ipv4() {
  python3 -c 'import ipaddress,sys
try:
    a = ipaddress.ip_address(sys.argv[1])
except ValueError:
    raise SystemExit(1)
raise SystemExit(0 if a.version == 4 and a.is_global else 1)' "$1" 2>/dev/null
}

# Prefer the tailnet when the guest joined one: WireGuard does not depend on the
# operator's ISP carrying an SSH session, needs no inbound rule, and survives the
# operator's public IP changing mid-run. Falls back to the public IP silently.
resolve_tailscale_ip() {
  command -v tailscale >/dev/null || return 0
  tailscale status --json 2>/dev/null | python3 -c '
import json, sys
try:
    data = json.load(sys.stdin)
except Exception:
    raise SystemExit
want = sys.argv[1] if len(sys.argv) > 1 else "sembla-bench"
for peer in (data.get("Peer") or {}).values():
    name = (peer.get("HostName") or "").lower()
    if name == want or name.startswith(want + "-"):
        if peer.get("Online"):
            for ip in peer.get("TailscaleIPs") or []:
                if ":" not in ip:
                    print(ip)
                    raise SystemExit
' "${TAILSCALE_NODE:-sembla-bench}" 2>/dev/null || true
}

PUBLIC_IP="${PUBLIC_IP_OVERRIDE:-}"
if [[ -z "$PUBLIC_IP" ]]; then
  TS_IP="$(resolve_tailscale_ip)"
  if [[ -n "$TS_IP" ]]; then
    echo "Found tailnet node ${TAILSCALE_NODE:-sembla-bench} at $TS_IP; using it instead of the public IP."
    PUBLIC_IP="$TS_IP"
  fi
fi
ip_deadline=$((SECONDS + ${IP_TIMEOUT_SECONDS:-600}))
ip_refreshed=false
while [[ -z "$PUBLIC_IP" ]] && (( SECONDS < ip_deadline )); do
  candidate="$(resolve_public_ip)"
  if is_global_ipv4 "$candidate"; then
    PUBLIC_IP="$candidate"
    break
  fi
  if [[ -n "${HYPERSTACK_API_KEY:-}" ]]; then
    # Ask the provider API directly: it is authoritative and immediate, whereas
    # Terraform state only learns the floating IP on a refresh. Refreshing ONCE
    # is not enough -- the IP attaches seconds to minutes after creation, so a
    # single early refresh sees nothing and the loop then spins uselessly.
    candidate="$(
      python3 - <<'PY' 2>/dev/null || true
import json, os, urllib.request
key = os.environ.get("HYPERSTACK_API_KEY", "")
req = urllib.request.Request(
    "https://infrahub-api.nexgencloud.com/v1/core/virtual-machines",
    headers={"api_key": key, "Accept": "application/json",
             # Hyperstack's edge rejects urllib's default user agent.
             "User-Agent": "sembla-precision-discovery/1.0"}, method="GET")
try:
    data = json.load(urllib.request.urlopen(req, timeout=30))
except Exception:
    raise SystemExit
for vm in (data.get("instances") or data.get("virtual_machines") or []):
    ip = vm.get("floating_ip")
    if ip:
        print(ip)
        break
PY
    )"
    if is_global_ipv4 "$candidate"; then
      PUBLIC_IP="$candidate"
      echo "Resolved public IP from the provider API."
      # Bring state into line so later terraform operations agree.
      terraform refresh -var-file="$TFVARS_FILE" \
        -var=create_instance=true \
        -var=accept_paid_creation=true >/dev/null 2>&1 || true
      break
    fi
  fi
  echo "Waiting for the public IP to be assigned."
  sleep 15
done
if [[ -z "$PUBLIC_IP" ]] || ! is_usable_ssh_target "$PUBLIC_IP"; then
  echo "No global public IPv4 for the paid VM within the timeout." >&2
  echo "The VM exists and is billing. Read its IP from the Hyperstack console" >&2
  echo "and re-run with PUBLIC_IP_OVERRIDE=<ip>, or destroy it now." >&2
  exit 2
fi
echo "Public IP: $PUBLIC_IP"
SSH_USER="$(terraform output -raw ssh_user)"

mkdir -p "$ARTIFACT_DIR"

# --- host-key verification: pinned fingerprint only, never trust-on-first-use ---
# The pinned key is either pre-seeded (prepare-host-key.sh, known before the VM
# existed) or read from the trusted VNC console. Either way the loop waits for a
# key that MATCHES; a non-matching key is never accepted, only retried. The retry
# matters for the pre-seeded path: sshd serves the image's own key for the few
# seconds before cloud-init installs the seeded one, and failing on that first
# scan would be a race, not a security event.
SCANNED_HOST_KEY=""
ACTUAL_HOST_KEY_FINGERPRINT=""
LAST_SEEN_FINGERPRINT="(none)"
host_key_deadline=$((SECONDS + 600))
host_key_delay=5
while (( SECONDS < host_key_deadline )); do
  SCANNED_HOST_KEY="$(ssh-keyscan -T 10 -t ed25519 "$PUBLIC_IP" 2>/dev/null || true)"
  if [[ -n "$SCANNED_HOST_KEY" ]]; then
    ACTUAL_HOST_KEY_FINGERPRINT="$(
      printf '%s\n' "$SCANNED_HOST_KEY" | ssh-keygen -E sha256 -lf - | awk 'NR == 1 { print $2 }'
    )"
    [[ "$ACTUAL_HOST_KEY_FINGERPRINT" == "$SSH_HOST_KEY_FINGERPRINT" ]] && break
    LAST_SEEN_FINGERPRINT="$ACTUAL_HOST_KEY_FINGERPRINT"
    ACTUAL_HOST_KEY_FINGERPRINT=""
    printf 'Host key does not match the pinned fingerprint yet; waiting.\n'
  fi
  sleep "$host_key_delay"
  (( host_key_delay < 30 )) && host_key_delay=$((host_key_delay + 5))
done
if [[ "$ACTUAL_HOST_KEY_FINGERPRINT" != "$SSH_HOST_KEY_FINGERPRINT" ]]; then
  echo "Never observed the pinned SSH host key; refusing the connection" >&2
  echo "expected:  $SSH_HOST_KEY_FINGERPRINT" >&2
  echo "last seen: $LAST_SEEN_FINGERPRINT" >&2
  exit 1
fi
printf '%s\n' "$SSH_HOST_KEY_FINGERPRINT" > "$ARTIFACT_DIR/trusted-ssh-host-fingerprint.txt"
printf '%s\n' "$SCANNED_HOST_KEY" > "$KNOWN_HOSTS_FILE"
chmod 0600 "$KNOWN_HOSTS_FILE"

SSH_OPTIONS=(
  -i "$SSH_PRIVATE_KEY_PATH"
  -o BatchMode=yes
  -o IdentitiesOnly=yes
  -o IPQoS=none
  # OpenSSH 9.x defaults to the sntrup761 hybrid KEX, whose client public key is
  # ~1200 bytes. On a path that silently drops packets near the MTU, that single
  # packet vanishes, the server never completes KEX, and the connection dies at
  # LoginGraceTime 30 -- indistinguishable from a hung host. curve25519 sends 32
  # bytes instead. The server offers it first, so this costs nothing.
  -o KexAlgorithms=curve25519-sha256,curve25519-sha256@libssh.org
  -o ConnectTimeout=10
  -o ConnectionAttempts=1
  -o ServerAliveInterval=15
  -o ServerAliveCountMax=40
  -o StrictHostKeyChecking=yes
  -o UserKnownHostsFile="$KNOWN_HOSTS_FILE"
)
REMOTE="$SSH_USER@$PUBLIC_IP"

# --- wait for cloud-init to finish provisioning CUDA, Rust, and the checkout ---
bootstrap_deadline=$((SECONDS + BOOTSTRAP_TIMEOUT_SECONDS))
bootstrap_delay=5
bootstrap_ready=false
while (( SECONDS < bootstrap_deadline )); do
  if status="$(
    ssh "${SSH_OPTIONS[@]}" "$REMOTE" \
      'if test -f /var/lib/sembla-bootstrap/failed; then echo failed; elif test -f /var/lib/sembla-bootstrap/ready; then echo ready; else echo running; fi' \
      2>/dev/null
  )"; then
    case "$status" in
      ready) bootstrap_ready=true; break ;;
      failed)
        scp "${SSH_OPTIONS[@]}" "$REMOTE:/var/log/sembla-bootstrap.log" \
          "$ARTIFACT_DIR/bootstrap.log" >/dev/null 2>&1 || true
        echo "Bootstrap reported failure; see $ARTIFACT_DIR/bootstrap.log" >&2
        exit 1
        ;;
      *) printf 'Bootstrap still running.\n' ;;
    esac
  fi
  sleep "$bootstrap_delay"
  (( bootstrap_delay < 30 )) && bootstrap_delay=$((bootstrap_delay + 5))
done
if [[ "$bootstrap_ready" != true ]]; then
  echo "Timed out waiting for cloud-init after ${BOOTSTRAP_TIMEOUT_SECONDS}s" >&2
  exit 1
fi

scp "${SSH_OPTIONS[@]}" "$REMOTE:/var/log/sembla-bootstrap.log" \
  "$ARTIFACT_DIR/bootstrap.log" >/dev/null 2>&1 || true

# --- remote payload -----------------------------------------------------------
# Written to a file and piped over stdin so quoting stays local and auditable.
REMOTE_SCRIPT="$(mktemp)"
cat > "$REMOTE_SCRIPT" <<'REMOTE_EOF'
set -Eeuo pipefail
# shellcheck disable=SC1091
source /etc/sembla-spike.env
: "${SPIKE_DIR:?cloud-init did not set SPIKE_DIR}"
: "${CUDA_HOME:?cloud-init did not set CUDA_HOME}"
export PATH="$HOME/.cargo/bin:$CUDA_HOME/bin:$PATH"
export LD_LIBRARY_PATH="$CUDA_HOME/lib64:${LD_LIBRARY_PATH:-}"

SCALES_CUDA="$1"; SCALES_CPU="$2"; TICKS="$3"; SEED="$4"
OUT_ROOT="$HOME/demographic-bench"
rm -rf "$OUT_ROOT"; mkdir -p "$OUT_ROOT"

command -v nvidia-smi >/dev/null
nvidia-smi --query-gpu=name,driver_version,memory.total,pci.bus_id \
  --format=csv,noheader > "$OUT_ROOT/nvidia-smi.txt"
GPU_NAME="$(awk -F', *' 'NR==1 {print $1}' "$OUT_ROOT/nvidia-smi.txt")"
RAM_GIB="$(awk '/MemTotal:/ {printf "%.0f", $2/1048576}' /proc/meminfo)"
git -C "$SPIKE_DIR" rev-parse HEAD > "$OUT_ROOT/repository-commit.txt"
{
  echo "gpu=$GPU_NAME"
  echo "host_ram_gib=$RAM_GIB"
  echo "cuda_home=$CUDA_HOME"
  nvcc --version | tail -2
} > "$OUT_ROOT/provenance.txt"

cd "$SPIKE_DIR"
echo "=== building sembla-cli with CUDA ==="
cargo build --locked --release -p sembla-cli --features cuda

run_bench() {
  local backend="$1" scales="$2" out="$3"
  [[ -z "$scales" ]] && return 0
  echo "=== bench backend=$backend scales=$scales ==="
  MACHINE_CLASS="Hyperstack $GPU_NAME, ${RAM_GIB} GiB host RAM, backend $backend"
  scripts/bench-demographic.sh \
    --scales "$scales" --seed "$SEED" --ticks "$TICKS" \
    --backend "$backend" --out "$out" \
    --machine-class "$MACHINE_CLASS" \
    --sembla "$SPIKE_DIR/target/release/sembla"
}

# Refuse a CPU scale this host cannot hold: a swapping run measures the pager,
# not the model. The budget is 400 B/slot plus 2 GiB, roughly twice the measured
# local peak RSS (1.90 GiB at 10M, docs/evidence/demographic-bench/local-2026-07-25),
# because that measurement is sublinear and taken on a machine under memory
# pressure, so it is an optimistic basis for a guard.
if [[ -n "$SCALES_CPU" ]]; then
  LARGEST_CPU="$(printf '%s\n' "${SCALES_CPU//,/$'\n'}" | sort -n | tail -1)"
  NEEDED_GIB=$(( 2 + (LARGEST_CPU * 400) / (1024 * 1024 * 1024) ))
  if (( RAM_GIB < NEEDED_GIB )); then
    echo "refusing CPU scale $LARGEST_CPU: host has ${RAM_GIB} GiB, budget is ${NEEDED_GIB} GiB" >&2
    echo "a swapping run measures paging, not the model" >&2
    echo "set BENCH_SCALES_CPU to a smaller list if this host is intentional" >&2
    exit 3
  fi
fi

run_bench cuda "$SCALES_CUDA" "$OUT_ROOT/cuda"
run_bench cpu "$SCALES_CPU" "$OUT_ROOT/cpu"

cd "$OUT_ROOT"
find . -type f \( -name '*.json' -o -name '*.md' -o -name '*.txt' \) -print0 \
  | sort -z | xargs -0 sha256sum > SHA256SUMS
tar -czf "$HOME/demographic-bench.tar.gz" -C "$HOME" demographic-bench
echo "SEMBLA_BENCH_COMPLETE"
REMOTE_EOF

# The remote run is hours long at 50M. Detach it so a laptop sleeping, a network
# drop, or a closed lid cannot kill the work — and so this driver can be re-run
# to rejoin a run already in progress instead of starting a second one.
scp "${SSH_OPTIONS[@]}" "$REMOTE_SCRIPT" "$REMOTE:bench-payload.sh" >/dev/null
REMOTE_SCRIPT_DONE=1

REMOTE_ARGS="$(printf '%q %q %q %q' "$BENCH_SCALES_CUDA" "$BENCH_SCALES_CPU" "$BENCH_TICKS" "$BENCH_SEED")"
ssh "${SSH_OPTIONS[@]}" "$REMOTE" "bash -s" <<REMOTE_LAUNCH
set -Eeuo pipefail
if [[ -f ~/bench.pid ]] && kill -0 "\$(cat ~/bench.pid)" 2>/dev/null; then
  echo "rejoining the benchmark already running as PID \$(cat ~/bench.pid)"
  exit 0
fi
rm -f ~/bench.log ~/bench.status
setsid nohup bash -c '
  if bash ~/bench-payload.sh $REMOTE_ARGS; then
    echo SEMBLA_BENCH_COMPLETE > ~/bench.status
  else
    echo "SEMBLA_BENCH_FAILED rc=\$?" > ~/bench.status
  fi
' > ~/bench.log 2>&1 < /dev/null &
echo \$! > ~/bench.pid
echo "started benchmark as PID \$(cat ~/bench.pid)"
REMOTE_LAUNCH

echo "Benchmark running detached on $REMOTE. Polling until it finishes."
echo "A dropped connection is harmless: re-run this script to rejoin."
bench_status=""
poll_deadline=$((SECONDS + ${BENCH_TIMEOUT_SECONDS:-43200}))
while (( SECONDS < poll_deadline )); do
  bench_status="$(ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'cat ~/bench.status 2>/dev/null || true' 2>/dev/null || true)"
  [[ -n "$bench_status" ]] && break
  # Surface the current phase so a watcher can see progress without attaching.
  ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'tail -1 ~/bench.log 2>/dev/null || true' 2>/dev/null || true
  sleep 120
done
ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'cat ~/bench.log' > "$ARTIFACT_DIR/remote-run.log" 2>/dev/null || true
if [[ "$bench_status" != "SEMBLA_BENCH_COMPLETE" ]]; then
  echo "Remote benchmark did not complete: ${bench_status:-still running at timeout}" >&2
  echo "Log: $ARTIFACT_DIR/remote-run.log" >&2
  echo "The VM is still up. Re-run this script to rejoin, or destroy it now." >&2
  exit 1
fi

scp "${SSH_OPTIONS[@]}" "$REMOTE:demographic-bench.tar.gz" "$ARTIFACT_DIR/" >/dev/null
tar -xzf "$ARTIFACT_DIR/demographic-bench.tar.gz" -C "$ARTIFACT_DIR" --strip-components=1
rm -f "$ARTIFACT_DIR/demographic-bench.tar.gz"

if command -v sha256sum >/dev/null; then
  (cd "$ARTIFACT_DIR" && sha256sum -c SHA256SUMS >/dev/null) \
    && echo "Artifact checksums verified."
elif command -v shasum >/dev/null; then
  (cd "$ARTIFACT_DIR" && shasum -a 256 -c SHA256SUMS >/dev/null) \
    && echo "Artifact checksums verified."
fi

# --- mandatory teardown -------------------------------------------------------
if [[ "${KEEP_VM:-0}" == "1" ]]; then
  echo "KEEP_VM=1: leaving the VM running. Billing continues; destroy it yourself." >&2
else
  : "${HYPERSTACK_API_KEY:?export HYPERSTACK_API_KEY so the VM can be destroyed}"
  echo "Destroying the VM."
  # Same invocation as README §5: the paid variables must be set for destroy to
  # target the VM that a paid apply created.
  terraform destroy -var-file="$TFVARS_FILE" \
    -var=create_instance=true \
    -var=accept_paid_creation=true \
    -auto-approve
  terraform state list > "$ARTIFACT_DIR/terraform-state-after-destroy.txt"
  if grep -qE 'hyperstack_core_virtual_machine|security_rule' "$ARTIFACT_DIR/terraform-state-after-destroy.txt"; then
    echo "DESTROY INCOMPLETE: paid resources remain in state. Open the Hyperstack console and delete the VM now." >&2
    exit 1
  fi
  DESTROYED=true
  rm -f hyperstack-paid.tfplan
  echo "Terraform destroy completed and state is clean. Confirm in the Hyperstack console that no VM remains."
fi

echo
echo "Evidence written to: $ARTIFACT_DIR"
for f in "$ARTIFACT_DIR"/cuda/bench-results.md "$ARTIFACT_DIR"/cpu/bench-results.md; do
  [[ -f "$f" ]] && { echo; echo "--- $f ---"; cat "$f"; }
done
