#!/usr/bin/env bash
# Unattended demographic-benchmark collection on the provisioned Hyperstack GPU VM.
#
# This script assumes the VM already exists via this module's reviewed paid-apply
# path. It adds no Terraform resources and changes no provisioning decision: the
# host-key trust model, the /32 SSH rule, and the mandatory destroy are inherited
# from `collect-runs.sh` and the module README.
#
# It executes DECISIONS.md §L's frozen gate on one VM: one 10M state artifact,
# one release binary, three no-grouped replicates per backend, and three paired
# full/no-ageing CPU replicates for the separately reported §K2 trigger input.
#
# Required environment:
#   HYPERSTACK_API_KEY          - needed only for the terraform destroy at the end
#   SSH_HOST_KEY_FINGERPRINT    - SHA256:... read from the Hyperstack VNC console
# Optional environment:
#   SSH_PRIVATE_KEY_PATH        - default ~/.ssh/sembla_hyperstack
#   ARTIFACT_DIR                - default docs/evidence/demographic-bench/hyperstack-l4-<UTC>
#   TFVARS_FILE                 - default terraform.tfvars
#   KEEP_VM=1                   - skip the automatic destroy (billing continues)
#   BOOTSTRAP_TIMEOUT_SECONDS   - default 1800
#   BENCH_TIMEOUT_SECONDS       - default 43200 (12h) for the whole remote run
#   BENCH_PROFILE=1             - additionally collect the phase-attribution
#                                 profile (5M rows, 2 ticks, both backends, both
#                                 the no-grouped and grouped configurations,
#                                 --timing-json plus nsys). Additive: the frozen
#                                 §L4 protocol is unchanged either way.
#   BENCH_CORPUS=1              - additionally run the CPU/CUDA differential
#                                 corpus, including the grouped demographic
#                                 configuration. Runs BEFORE the frozen gate and
#                                 aborts it on disagreement: timing an arm that
#                                 disagrees with the CPU oracle is worse than
#                                 collecting no timing at all.
#   SSH_FAILURE_LIMIT           - default 5 consecutive poll failures before the
#                                 collector gives up and says why
#
# Both BENCH_PROFILE and BENCH_CORPUS are baked into the content-addressed
# payload hash, so a run with them set will not silently rejoin a plain gate run
# already in progress -- it is correctly treated as a different payload.
#
# The remote run is detached, so a dropped connection loses nothing: re-running
# this script rejoins the run in progress rather than starting a second one.
# Check that the VM's emergency poweroff timer (emergency_poweroff_hours)
# exceeds the expected run time before starting the frozen collection.
set -Eeuo pipefail

MODULE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$MODULE_DIR/../../.." && pwd)"
SSH_PRIVATE_KEY_PATH="${SSH_PRIVATE_KEY_PATH:-$HOME/.ssh/sembla_hyperstack}"
KNOWN_HOSTS_FILE="$MODULE_DIR/.hyperstack_known_hosts"
TFVARS_FILE="${TFVARS_FILE:-terraform.tfvars}"
BOOTSTRAP_TIMEOUT_SECONDS="${BOOTSTRAP_TIMEOUT_SECONDS:-1800}"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
ARTIFACT_DIR="${ARTIFACT_DIR:-$REPO_ROOT/docs/evidence/demographic-bench/hyperstack-l4-$STAMP}"

DESTROYED=false
REMOTE_SCRIPT=""
POLL_ERR=""
finish() {
  local status=$?
  [[ -n "$REMOTE_SCRIPT" ]] && rm -f "$REMOTE_SCRIPT"
  [[ -n "$POLL_ERR" ]] && rm -f "$POLL_ERR"
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
for legacy_override in BENCH_SCALES_CUDA BENCH_SCALES_CPU BENCH_TICKS BENCH_SEED; do
  if [[ -n "${!legacy_override:-}" ]]; then
    echo "$legacy_override is not configurable for the frozen §L4 protocol" >&2
    exit 2
  fi
done
if [[ -e "$ARTIFACT_DIR" ]]; then
  echo "evidence directory already exists; choose a new ARTIFACT_DIR: $ARTIFACT_DIR" >&2
  exit 2
fi
mkdir -p "$ARTIFACT_DIR"

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
  # WAIT for the node rather than checking once. The single check this replaced
  # could not succeed on a fresh VM: the collector reaches this line within
  # seconds of the apply, while the guest only joins the tailnet part-way
  # through cloud-init, a minute or two later. So the tailnet branch was never
  # taken on a new machine and every fresh session silently fell back to the
  # public IP -- the path whose /32 pinning stranded a run on 2026-07-27. The
  # feature was working; nothing ever gave it the chance to be used.
  #
  # Only wait when there is a reason to expect a node. With no auth key the
  # guest was never going to join, and waiting would add minutes to every run
  # that deliberately uses the public path.
  if [[ -n "${TF_VAR_tailscale_auth_key:-}" ]] && command -v tailscale >/dev/null; then
    ts_deadline=$((SECONDS + ${TAILSCALE_TIMEOUT_SECONDS:-300}))
    ts_announced=false
    while (( SECONDS < ts_deadline )); do
      TS_IP="$(resolve_tailscale_ip)"
      [[ -n "$TS_IP" ]] && break
      if [[ "$ts_announced" != true ]]; then
        echo "Waiting up to $(( (ts_deadline - SECONDS) / 60 )) minutes for tailnet node ${TAILSCALE_NODE:-sembla-bench} to appear."
        echo "It joins part-way through cloud-init, so it is not there yet on a fresh VM."
        ts_announced=true
      fi
      sleep 10
    done
  else
    TS_IP="$(resolve_tailscale_ip)"
  fi
  if [[ -n "${TS_IP:-}" ]]; then
    echo "Found tailnet node ${TAILSCALE_NODE:-sembla-bench} at $TS_IP; using it instead of the public IP."
    PUBLIC_IP="$TS_IP"
  elif [[ -n "${TF_VAR_tailscale_auth_key:-}" ]]; then
    echo "Tailnet node did not appear within the timeout; falling back to the public IP." >&2
    echo "Check the guest's bootstrap log for the 'tailscale up' step." >&2
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
if is_tailnet_ipv4 "$PUBLIC_IP"; then
  echo "Transport: tailnet ($PUBLIC_IP). Survives an egress-IP change."
else
  # Say this before hours of compute are committed to a path that is known to
  # break. On 2026-07-27 the operator's egress IP rotated mid-run and locked a
  # healthy VM out for the rest of the evening.
  echo "Transport: public IP ($PUBLIC_IP)." >&2
  echo "WARNING: this path is pinned to one /32 in TWO places -- the Hyperstack" >&2
  echo "security group AND the guest's own iptables rule. If your egress IP" >&2
  echo "rotates mid-run you lose access to a healthy, billing machine, and you" >&2
  echo "cannot fix it by re-applying, because ssh_cidr is ForceNew." >&2
  if [[ -z "${TF_VAR_tailscale_auth_key:-}" ]]; then
    echo "TF_VAR_tailscale_auth_key was not set, so the guest never joined a" >&2
    echo "tailnet. Setting it (ephemeral, pre-authorized, tagged) before the" >&2
    echo "apply is the fix; see RUNBOOK.md 'Reaching the VM over Tailscale'." >&2
  else
    echo "TF_VAR_tailscale_auth_key IS set, so the guest was meant to join the" >&2
    echo "tailnet and has not appeared. Check 'tailscale status' and the guest's" >&2
    echo "bootstrap log before committing hours of compute to this path." >&2
  fi
  echo "Current egress IP: $(curl -s --max-time 10 https://api.ipify.org || echo unknown)" >&2
  grep -n 'ssh_cidr' "$TFVARS_FILE" >&2 || true
fi

# The evidence push is the only artifact path that does not depend on this
# laptop surviving the run. On 2026-07-28 it was silently skipped because the
# variable was unset, and nobody noticed until the run was over -- an unset
# optional feature and a working one look identical from here.
if [[ -z "${TF_VAR_evidence_deploy_key:-}" ]]; then
  echo "Evidence push: DISABLED (TF_VAR_evidence_deploy_key unset)." >&2
  echo "Artifacts will arrive only over SSH, so collection depends on this" >&2
  echo "session surviving to the end. Run prepare-deploy-key.sh before the" >&2
  echo "apply to make delivery independent of it." >&2
else
  echo "Evidence push: enabled; artifacts will also land on an evidence/<UTC> branch."
fi
SSH_USER="$(terraform output -raw ssh_user)"

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
# Baked into the payload rather than passed at launch, so the content-addressed
# payload hash reflects it: a profile run and a plain gate run are then correctly
# treated as different payloads instead of one rejoining the other.
printf 'export BENCH_PROFILE=%q\n' "${BENCH_PROFILE:-0}" > "$REMOTE_SCRIPT"
printf 'export BENCH_CORPUS=%q\n' "${BENCH_CORPUS:-0}" >> "$REMOTE_SCRIPT"
cat >> "$REMOTE_SCRIPT" <<'REMOTE_EOF'
set -Eeuo pipefail
# shellcheck disable=SC1091
source /etc/sembla-spike.env
: "${SPIKE_DIR:?cloud-init did not set SPIKE_DIR}"
: "${CUDA_HOME:?cloud-init did not set CUDA_HOME}"
export PATH="$HOME/.cargo/bin:$CUDA_HOME/bin:$PATH"
export LD_LIBRARY_PATH="$CUDA_HOME/lib64:${LD_LIBRARY_PATH:-}"

# The frozen protocol is deliberately not configurable here. Changing any of
# these values would create a benchmark, but not §L4 gate evidence.
SCALE=10000000
TICKS=24
SEED=9009
AREAS=4
PRESENT_FRACTION=0.8
STREAMS='birth:600,overseas:250,internal:150'
REPLICATES=3
OUT_ROOT="$HOME/demographic-bench"
WORK="$OUT_ROOT/work"
RUNS="$OUT_ROOT/runs"
rm -rf "$OUT_ROOT"
mkdir -p "$WORK" "$RUNS"
export LC_ALL=C

command -v nvidia-smi >/dev/null
command -v sha256sum >/dev/null
command -v /usr/bin/time >/dev/null
nvidia-smi --query-gpu=name,driver_version,memory.total,pci.bus_id,uuid \
  --format=csv,noheader > "$OUT_ROOT/gpu-provenance.txt"
nvidia-smi -q >> "$OUT_ROOT/gpu-provenance.txt"
lscpu > "$OUT_ROOT/cpu-provenance.txt"
cat /proc/meminfo > "$OUT_ROOT/ram-provenance.txt"
GPU_NAME="$(awk -F', *' 'NR==1 {print $1}' "$OUT_ROOT/gpu-provenance.txt")"
RAM_GIB="$(awk '/MemTotal:/ {printf "%.0f", $2/1048576}' /proc/meminfo)"
SESSION_ID="$(cat /proc/sys/kernel/random/uuid)"
printf '%s\n' "$SESSION_ID" > "$OUT_ROOT/session-id.txt"
HOST_ID="$({ cat /etc/machine-id; head -1 "$OUT_ROOT/gpu-provenance.txt"; } | sha256sum | awk '{print $1}')"
printf '%s\n' "$HOST_ID" > "$OUT_ROOT/host-identity.sha256"

cd "$SPIKE_DIR"
if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo 'remote repository has tracked changes; refusing gate evidence' >&2
  exit 4
fi
COMMIT_BEFORE="$(git rev-parse HEAD)"
printf '%s\n' "$COMMIT_BEFORE" > "$OUT_ROOT/repository-commit.txt"
STARTED_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

echo '=== building one sembla-cli binary with CUDA ==='
cargo build --locked --release -p sembla-cli --features cuda
BIN="$SPIKE_DIR/target/release/sembla"
BIN_SHA="$(sha256sum "$BIN" | awk '{print $1}')"
printf '%s  target/release/sembla\n' "$BIN_SHA" > "$OUT_ROOT/binary.sha256"

# --- optional phase-attribution profile ---------------------------------------
# Runs BEFORE the frozen gate, deliberately: it finishes in minutes where the
# gate takes hours, so its result is readable in ~bench.log early and the
# operator can let the gate continue or tear down without waiting.
#
# Additive only: the frozen §L4 protocol below is untouched, which is why the
# scale/tick overrides remain refused. This stage answers a different question —
# where the CUDA path's wall time actually goes — using the case from
# docs/evidence/cuda-l4-20260726 so the numbers are directly comparable to the
# ~10,200 ms that run recorded as "unaccounted (host CPU)".
if [[ "${BENCH_PROFILE:-0}" == "1" ]]; then
  echo '=== phase-attribution profile (5M rows, 2 ticks) ==='
  PROFILE_DIR="$OUT_ROOT/profile"
  mkdir -p "$PROFILE_DIR"
  PROFILE_SCALE=5000000
  PROFILE_TICKS=2
  PROFILE_STATE="$WORK/profile-5m.state"

  "$BIN" synth-state \
    --model fixtures/demographic/benchmark/demographic_slots.full.json \
    --slots "$PROFILE_SCALE" --areas "$AREAS" --present-fraction "$PRESENT_FRACTION" \
    --streams "$STREAMS" --seed "$SEED" --out "$PROFILE_STATE" \
    > "$PROFILE_DIR/synth-state.stdout" 2> "$PROFILE_DIR/synth-state.stderr"
  sha256sum "$PROFILE_STATE" | awk '{print $1"  profile-5m.state"}' \
    > "$PROFILE_DIR/profile-state.sha256"

  python3 - "$PROFILE_SCALE" "$PROFILE_STATE.model.json" "$WORK" <<'PY'
import json, pathlib, sys
scale = int(sys.argv[1])
model = json.loads(pathlib.Path(sys.argv[2]).read_text())
for box in model["boxes"]:
    for table in box["tables"]:
        if table["name"] in {"person_slot", "slot_resource"}:
            table["size_hint"] = scale
work = pathlib.Path(sys.argv[3])
for src_name, out_name in (
    ("demographic_slots.no-grouped.json", "profile-no-grouped.json"),
    # The grouped variant is the configuration the real calibration workflow
    # uses, and until prds-device-observation/0002 the CUDA backend rejected it
    # outright, so it has never been profiled on a GPU.
    ("demographic_slots.full.json", "profile-grouped.json"),
):
    src = pathlib.Path.cwd() / "fixtures/demographic/benchmark" / src_name
    tmpl = json.loads(src.read_text())
    for box in tmpl["boxes"]:
        for table in box["tables"]:
            if table["name"] in {"person_slot", "slot_resource"}:
                table["size_hint"] = scale
    (work / out_name).write_text(json.dumps(tmpl, separators=(",", ":")) + "\n")
PY

  # Both backends, so the shared host phases can be read side by side.
  #
  # Two configurations, because they answer different questions. The no-grouped
  # case is the one the earlier profiles measured, so it is what makes the
  # before/after phase table comparable. The grouped case is what the driver
  # model actually runs, and 0001's eligibility is all-or-nothing per run: a
  # model with grouped views downloaded the whole state regardless of how many
  # ungrouped views qualified. If 0002 worked, `state_transfer` and
  # `state_reconstruct` collapse in the grouped table too; if only the
  # no-grouped table improves, 0002 delivered nothing for the real workflow.
  #
  # The CUDA grouped runs also emit one `cuda_device_grouped_observation` line
  # per view per tick on stderr, carrying key_space_size, occupied_groups and
  # emitted_groups. Those stderr files are the §J14.2 evidence for 0002's
  # key-space criterion, so they are collected, not discarded.
  for backend in cuda cpu; do
    "$BIN" run "$WORK/profile-no-grouped.json" \
      --seed "$SEED" --population "$PROFILE_STATE" --backend "$backend" \
      --ticks "$PROFILE_TICKS" \
      --timing-json "$PROFILE_DIR/timing-$backend.json" \
      --out "$PROFILE_DIR/profile-$backend.csv" \
      > "$PROFILE_DIR/profile-$backend.stdout" 2> "$PROFILE_DIR/profile-$backend.stderr"

    # A grouped CUDA failure must not lose the no-grouped tables already
    # written: those satisfy 0001's criteria on their own, and an abort here
    # would throw away a result that cost the same GPU hour to produce.
    if "$BIN" run "$WORK/profile-grouped.json" \
      --seed "$SEED" --population "$PROFILE_STATE" --backend "$backend" \
      --ticks "$PROFILE_TICKS" --enable grouped-observations \
      --timing-json "$PROFILE_DIR/timing-grouped-$backend.json" \
      --out "$PROFILE_DIR/profile-grouped-$backend.csv" \
      > "$PROFILE_DIR/profile-grouped-$backend.stdout" \
      2> "$PROFILE_DIR/profile-grouped-$backend.stderr"; then
      echo "grouped profile ($backend) complete"
    else
      echo "GROUPED PROFILE FAILED ($backend); no-grouped results are unaffected" >&2
      tail -20 "$PROFILE_DIR/profile-grouped-$backend.stderr" >&2 || true
      touch "$PROFILE_DIR/profile-grouped-$backend.FAILED"
    fi
  done

  # The two backends must agree on the grouped observations, and this is the
  # cheapest place to notice they do not -- the differential corpus runs at 1000
  # rows, so this is the only grouped CPU/CUDA comparison at realistic scale.
  #
  # The grouped values are NOT in the main results CSV. Each view is written to
  # its own `<stem>.grouped.<view>.csv` sidecar (main.rs:2202), and the scalar
  # summaries to `<stem>.summaries.csv`. Comparing only the main CSV would
  # compare two files that contain no grouped data at all and always pass --
  # exactly the kind of check that looks like evidence and is not. So compare
  # every artifact the run produced, and require the file SETS to match too.
  if [[ ! -f "$PROFILE_DIR/profile-grouped-cuda.FAILED" \
        && ! -f "$PROFILE_DIR/profile-grouped-cpu.FAILED" ]]; then
    (
      cd "$PROFILE_DIR"
      cuda_files="$(ls profile-grouped-cuda.csv profile-grouped-cuda.*.csv 2>/dev/null \
        | sed 's/^profile-grouped-cuda//' | sort)"
      cpu_files="$(ls profile-grouped-cpu.csv profile-grouped-cpu.*.csv 2>/dev/null \
        | sed 's/^profile-grouped-cpu//' | sort)"
      if [[ -z "$cuda_files" ]]; then
        echo 'GROUPED PARITY INCONCLUSIVE: no CUDA grouped output files found'
        exit 0
      fi
      if [[ "$cuda_files" != "$cpu_files" ]]; then
        echo 'GROUPED PARITY FAILED: backends produced different output file sets'
        echo "cuda: $cuda_files"
        echo "cpu:  $cpu_files"
        exit 0
      fi
      mismatch=0
      compared=0
      while IFS= read -r suffix; do
        [[ -z "$suffix" ]] && continue
        compared=$((compared + 1))
        if ! cmp -s "profile-grouped-cuda$suffix" "profile-grouped-cpu$suffix"; then
          echo "GROUPED PARITY FAILED: profile-grouped-{cuda,cpu}$suffix differ"
          mismatch=1
        fi
      done <<<"$cuda_files"
      if (( mismatch == 0 )); then
        echo "grouped CPU/CUDA outputs identical at 5M rows across $compared file(s):"
        printf '  profile-grouped-*%s\n' $cuda_files
      fi
    ) | tee "$PROFILE_DIR/grouped-parity.txt"
    # `|| true` matters: the payload runs under `set -e`, so a bare
    # `grep -q ... && echo ...` would abort the whole benchmark on the HAPPY
    # path, when grep finds no failure and returns 1.
    if grep -q 'GROUPED PARITY FAILED' "$PROFILE_DIR/grouped-parity.txt"; then
      echo 'grouped parity check FAILED; see profile/grouped-parity.txt' >&2
    fi
  fi

  # nsys still gives per-kernel detail the timers cannot. Failure here must not
  # lose the timing JSON, which is the primary artifact.
  if command -v nsys >/dev/null; then
    (
      cd "$PROFILE_DIR"
      nsys profile --trace=cuda --force-overwrite=true -o profile-cuda \
        "$BIN" run "$WORK/profile-no-grouped.json" \
        --seed "$SEED" --population "$PROFILE_STATE" --backend cuda \
        --ticks "$PROFILE_TICKS" --out nsys-run.csv \
        > nsys-run.stdout 2> nsys-run.stderr
      nsys stats --report cuda_gpu_kern_sum profile-cuda.nsys-rep > nsys-kern-sum.txt 2>&1
      nsys stats --report cuda_api_sum      profile-cuda.nsys-rep > nsys-api-sum.txt 2>&1
      rm -f profile-cuda.nsys-rep nsys-run.csv
    ) || echo 'nsys profiling failed; timing JSON is unaffected' >&2
  else
    echo 'nsys not found; skipping kernel-level detail' > "$PROFILE_DIR/nsys-missing.txt"
  fi

  rm -f "$PROFILE_STATE" "$PROFILE_STATE.model.json"
  echo '=== phase-attribution profile complete ==='
fi

# --- optional CPU/CUDA differential corpus -------------------------------------
# Nothing ran this before 2026-07-28. `crates/sembla-cuda/scripts/run-differential-corpus.sh`
# has existed for some time and prds-device-observation/0002 added the grouped
# demographic configuration to it, but no collector ever invoked it, so the
# corpus was only ever run by hand -- which is why several PRDs still carry
# "CPU/CUDA differential equality" as hardware-pending with no automation behind
# it. It costs minutes against the gate's hours, so it runs before the gate.
#
# This is the correctness precondition for everything else in the session: the
# gate below times the CUDA arm, and a fast wrong answer is worth nothing.
# SEMBLA_REQUIRE_CUDA=1 turns the script's "no GPU, skip cleanly" behaviour into
# a hard failure -- on a GPU host a skip means something is broken, not absent.
if [[ "${BENCH_CORPUS:-0}" == "1" ]]; then
  echo '=== CPU/CUDA differential corpus ==='
  CORPUS_DIR="$OUT_ROOT/differential-corpus"
  mkdir -p "$CORPUS_DIR"
  # `timeout` and `tee`, both learned on 2026-07-28 (DECISIONS.md §L12).
  #
  # The corpus deadlocked on a GPU kernel and ran for 2h31m before anyone
  # noticed, because of two independent mistakes in the first version of this
  # stage. It redirected to run.log instead of tee-ing, so `tail -1 ~/bench.log`
  # -- the only progress the collector shows -- froze on the stage header and a
  # hang looked exactly like a long compile. And it had no timeout, so the
  # ceiling was the collector's 12-hour poll. A deadlock that costs 23 seconds
  # to reproduce cost 2.5 hours of GPU time.
  #
  # The corpus took 23s on 2026-07-19, so the default below is roughly 75x the
  # known-good duration. It bounds a hang; it does not constrain a healthy run.
  set +e
  (
    cd "$SPIKE_DIR"
    # The script refuses to run against a dirty tree, deliberately: differential
    # evidence is only meaningful for an exact commit. Record what it saw.
    git rev-parse HEAD > "$CORPUS_DIR/commit.txt"
    git status --porcelain > "$CORPUS_DIR/worktree-status.txt"
    SEMBLA_REQUIRE_CUDA=1 SEMBLA_CUDA_EVIDENCE_DIR="$CORPUS_DIR" \
      timeout --kill-after=60 "${BENCH_CORPUS_TIMEOUT_SECONDS:-1800}" \
      bash crates/sembla-cuda/scripts/run-differential-corpus.sh
  ) 2>&1 | tee "$CORPUS_DIR/run.log"
  corpus_rc=${PIPESTATUS[0]}
  set -e
  printf '%s\n' "$corpus_rc" > "$CORPUS_DIR/exit-code.txt"
  if (( corpus_rc == 124 || corpus_rc == 137 )); then
    echo "=== DIFFERENTIAL CORPUS TIMED OUT after ${BENCH_CORPUS_TIMEOUT_SECONDS:-1800}s ===" >&2
    echo 'This is a hang, not slowness: the corpus completed in 23s on 2026-07-19.' >&2
    echo 'A GPU-side deadlock is the likeliest cause; see DECISIONS.md §L12 for the' >&2
    echo 'one already found, which passes at launch geometry 1x1 and hangs at 1x32.' >&2
    tail -40 "$CORPUS_DIR/run.log" >&2 || true
    echo "Full log: $CORPUS_DIR/run.log" >&2
    exit 7
  fi
  if (( corpus_rc == 0 )); then
    echo '=== differential corpus PASSED ==='
  else
    # Loud, and it stops the session here. Timing an arm that disagrees with the
    # CPU oracle would produce numbers that look like evidence and are not; that
    # is a worse outcome than no numbers, because it is harder to notice later.
    echo "=== DIFFERENTIAL CORPUS FAILED (rc=$corpus_rc) ===" >&2
    tail -40 "$CORPUS_DIR/run.log" >&2 || true
    echo 'Refusing to run the frozen gate: CUDA disagrees with the CPU oracle.' >&2
    echo "Full log: $CORPUS_DIR/run.log" >&2
    exit 6
  fi
fi

STATE="$WORK/initial.state"
echo '=== synthesizing one shared 10M state artifact ==='
"$BIN" synth-state \
  --model fixtures/demographic/benchmark/demographic_slots.full.json \
  --slots "$SCALE" --areas "$AREAS" --present-fraction "$PRESENT_FRACTION" \
  --streams "$STREAMS" --seed "$SEED" --out "$STATE" \
  > "$OUT_ROOT/synth-state.stdout" 2> "$OUT_ROOT/synth-state.stderr"
STATE_SHA="$(sha256sum "$STATE" | awk '{print $1}')"
printf '%s  initial.state\n' "$STATE_SHA" > "$OUT_ROOT/initial-state.sha256"

python3 - "$SCALE" "$STATE.model.json" "$WORK" <<'PY'
import json, pathlib, sys
scale = int(sys.argv[1])
full_companion = pathlib.Path(sys.argv[2])
out = pathlib.Path(sys.argv[3])
root = pathlib.Path.cwd()
templates = {
    "full": full_companion,
    "no-ageing": root / "fixtures/demographic/benchmark/demographic_slots.no-ageing.json",
    "no-grouped": root / "fixtures/demographic/benchmark/demographic_slots.no-grouped.json",
}
for name, path in templates.items():
    model = json.loads(path.read_text())
    for box in model["boxes"]:
        for table in box["tables"]:
            if table["name"] in {"person_slot", "slot_resource"}:
                table["size_hint"] = scale
    (out / f"{name}.json").write_text(
        json.dumps(model, separators=(",", ":")) + "\n"
    )
PY
(
  cd "$WORK"
  sha256sum full.json no-ageing.json no-grouped.json > "$OUT_ROOT/models.sha256"
)

measure() {
  local label="$1"; shift
  echo "=== measurement $label ==="
  /usr/bin/time -f '{"wall_seconds":%e,"peak_rss_kib":%M}' \
    -o "$RUNS/$label.time.json" \
    "$@" > "$RUNS/$label.stdout" 2> "$RUNS/$label.stderr"
  python3 - "$RUNS/$label.time.json" <<'PY'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
row = json.loads(p.read_text())
if row["wall_seconds"] <= 0 or row["peak_rss_kib"] <= 0:
    raise SystemExit(f"invalid timing record: {row}")
PY
}

# Interleave the gate arms. Every command below names the same state path and
# binary path; their hashes are asserted again after the final replicate.
for replicate in 1 2 3; do
  measure "cuda-$replicate" "$BIN" run "$WORK/no-grouped.json" \
    --seed "$SEED" --population "$STATE" --backend cuda --ticks "$TICKS" \
    --out "$RUNS/cuda-$replicate.csv"
  measure "cpu-$replicate" "$BIN" run "$WORK/no-grouped.json" \
    --seed "$SEED" --population "$STATE" --backend cpu --ticks "$TICKS" \
    --out "$RUNS/cpu-$replicate.csv"
  measure "ageing-full-$replicate" "$BIN" run "$WORK/full.json" \
    --seed "$SEED" --population "$STATE" --backend cpu --ticks "$TICKS" \
    --enable grouped-observations --out "$RUNS/ageing-full-$replicate.csv"
  measure "ageing-no-ageing-$replicate" "$BIN" run "$WORK/no-ageing.json" \
    --seed "$SEED" --population "$STATE" --backend cpu --ticks "$TICKS" \
    --enable grouped-observations --out "$RUNS/ageing-no-ageing-$replicate.csv"
done

# Differential corpus coverage is separate, but gate evidence must not time an
# incorrect arm. Require exact equality of results, summaries, and the printed
# results/final-state/observation hash tuple across all six gate replicates.
GATE_RESULTS_SHA="$(sha256sum "$RUNS/cuda-1.csv" | awk '{print $1}')"
GATE_SUMMARIES_SHA="$(sha256sum "$RUNS/cuda-1.csv.summaries.csv" | awk '{print $1}')"
GATE_HASHES_SHA="$(sha256sum "$RUNS/cuda-1.stdout" | awk '{print $1}')"
for arm in cuda-{1,2,3} cpu-{1,2,3}; do
  if [[ "$(sha256sum "$RUNS/$arm.csv" | awk '{print $1}')" != "$GATE_RESULTS_SHA" ]]; then
    echo "gate results differ across backend/replicate: $arm" >&2
    exit 5
  fi
  if [[ "$(sha256sum "$RUNS/$arm.csv.summaries.csv" | awk '{print $1}')" != "$GATE_SUMMARIES_SHA" ]]; then
    echo "gate summaries differ across backend/replicate: $arm" >&2
    exit 5
  fi
  if [[ "$(sha256sum "$RUNS/$arm.stdout" | awk '{print $1}')" != "$GATE_HASHES_SHA" ]]; then
    echo "gate result/final-state/observation hashes differ across backend/replicate: $arm" >&2
    exit 5
  fi
done
{
  printf '%s  results.csv (all CPU/CUDA replicates identical)\n' "$GATE_RESULTS_SHA"
  printf '%s  summaries.csv (all CPU/CUDA replicates identical)\n' "$GATE_SUMMARIES_SHA"
  printf '%s  printed execution hashes (all CPU/CUDA replicates identical)\n' "$GATE_HASHES_SHA"
} > "$OUT_ROOT/gate-outputs.sha256"

COMMIT_AFTER="$(git rev-parse HEAD)"
BIN_SHA_AFTER="$(sha256sum "$BIN" | awk '{print $1}')"
STATE_SHA_AFTER="$(sha256sum "$STATE" | awk '{print $1}')"
if [[ "$COMMIT_AFTER" != "$COMMIT_BEFORE" ]]; then
  echo 'repository commit changed during benchmark' >&2
  exit 5
fi
if [[ "$BIN_SHA_AFTER" != "$BIN_SHA" ]]; then
  echo 'benchmark binary changed between arms' >&2
  exit 5
fi
if [[ "$STATE_SHA_AFTER" != "$STATE_SHA" ]]; then
  echo 'shared state artifact changed between arms' >&2
  exit 5
fi
if [[ -n "$(git status --porcelain --untracked-files=no)" ]]; then
  echo 'remote repository gained tracked changes during benchmark' >&2
  exit 5
fi
FINISHED_UTC="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

python3 - "$OUT_ROOT" "$COMMIT_BEFORE" "$BIN_SHA" "$STATE_SHA" \
  "$GATE_RESULTS_SHA" "$GATE_SUMMARIES_SHA" "$GATE_HASHES_SHA" \
  "$HOST_ID" "$SESSION_ID" "$GPU_NAME" "$RAM_GIB" "$STARTED_UTC" "$FINISHED_UTC" <<'PY'
import json, pathlib, statistics, sys
(
    root_s, commit, binary_sha, state_sha,
    gate_results_sha, gate_summaries_sha, gate_hashes_sha,
    host_id, session_id, gpu_name, ram_gib, started, finished,
) = sys.argv[1:]
root = pathlib.Path(root_s)
runs = root / "runs"

def timing(label):
    return json.loads((runs / f"{label}.time.json").read_text())

def series(prefix):
    return [timing(f"{prefix}-{i}")["wall_seconds"] for i in range(1, 4)]

def summary(values):
    return {
        "replicates": values,
        "median_seconds": statistics.median(values),
        "min_seconds": min(values),
        "max_seconds": max(values),
        "spread_seconds": max(values) - min(values),
    }

cuda = series("cuda")
cpu = series("cpu")
full = series("ageing-full")
no_ageing = series("ageing-no-ageing")
ageing = [(f - n) / f for f, n in zip(full, no_ageing)]
speedup = statistics.median(cpu) / statistics.median(cuda)
ageing_median = statistics.median(ageing)
doc = {
    "schema_version": "sembla.cuda-validation-l4-benchmark/v1",
    "protocol": {
        "model": "fixtures/demographic/benchmark/demographic_slots.no-grouped.json",
        "slots": 10_000_000,
        "ticks": 24,
        "seed": 9009,
        "areas": 4,
        "present_fraction": 0.8,
        "streams": "birth:600,overseas:250,internal:150",
        "replicates_per_backend": 3,
        "reported_statistic": "median",
        "gate_cuda_speedup_at_least": 3.0,
    },
    "provenance": {
        "repository_commit": commit,
        "binary_sha256": binary_sha,
        "initial_state_sha256": state_sha,
        "gate_results_sha256": gate_results_sha,
        "gate_summaries_sha256": gate_summaries_sha,
        "gate_execution_hashes_sha256": gate_hashes_sha,
        "host_identity_sha256": host_id,
        "session_id": session_id,
        "gpu_name": gpu_name,
        "host_ram_gib": int(ram_gib),
        "started_utc": started,
        "finished_utc": finished,
    },
    "assertions": {
        "one_host": True,
        "one_session": True,
        "repository_commit_identical_for_all_arms": True,
        "binary_identical_for_all_arms": True,
        "initial_state_identical_for_all_arms": True,
        "gate_results_summaries_and_state_hashes_identical_for_all_arms": True,
        "three_replicates_per_backend": len(cuda) == len(cpu) == 3,
    },
    "measurements": {
        "cuda_no_grouped": summary(cuda),
        "cpu_no_grouped": summary(cpu),
        "cpu_full": summary(full),
        "cpu_no_ageing": summary(no_ageing),
        "ageing_cost_share": {
            "replicates": ageing,
            "median": ageing_median,
            "min": min(ageing),
            "max": max(ageing),
            "spread": max(ageing) - min(ageing),
        },
    },
    "verdict": {
        "cpu_median_over_cuda_median": speedup,
        "l4_gate_met": speedup >= 3.0,
        "ageing_k2_threshold": 0.10,
        "ageing_evidence_direction": "strengthens" if ageing_median >= 0.10 else "weakens",
        "k2_decided_here": False,
    },
}
(root / "bench-results.json").write_text(json.dumps(doc, indent=2, sort_keys=True) + "\n")

c = doc["measurements"]["cuda_no_grouped"]
p = doc["measurements"]["cpu_no_grouped"]
a = doc["measurements"]["ageing_cost_share"]
v = doc["verdict"]
verdict = "MET" if v["l4_gate_met"] else "NOT MET"
ratio = v["cpu_median_over_cuda_median"]
fmt = lambda xs: ", ".join(f"{x:.3f}" for x in xs)
readme = f"""# CUDA validation §L4 frozen benchmark evidence

This directory records one frozen-case session on one host at repository commit
`{commit}`. The collector asserted that every arm used binary SHA-256
`{binary_sha}` and initial-state SHA-256 `{state_sha}`; it aborted if the commit,
binary, or state changed.

## Gate result

- CUDA no-grouped replicates: {fmt(c['replicates'])} s; median **{c['median_seconds']:.3f} s**; spread {c['min_seconds']:.3f}–{c['max_seconds']:.3f} s.
- CPU no-grouped replicates: {fmt(p['replicates'])} s; median **{p['median_seconds']:.3f} s**; spread {p['min_seconds']:.3f}–{p['max_seconds']:.3f} s.
- Same-host CPU-median / CUDA-median ratio: **{ratio:.3f}×**.
- §L4 verdict: **{verdict}** (required: CUDA at least 3× faster).

## Ageing share

Paired full/no-ageing CPU replicates produce ageing shares
{', '.join(f'{x:.2%}' for x in a['replicates'])}; median **{a['median']:.2%}**;
spread {a['min']:.2%}–{a['max']:.2%}. This **{v['ageing_evidence_direction']}**
the existing evidence for the §K2 10% trigger. It does **not** decide §K2.

`bench-results.json` is the machine-readable record. `bench-results.md` lists all
raw replicate timings. GPU, CPU, and RAM provenance are in the three named
`*-provenance.txt` files. Verify the directory with `sha256sum -c SHA256SUMS`
(or `shasum -a 256 -c SHA256SUMS` on macOS).
"""
(root / "README.md").write_text(readme)

lines = [
    "# Frozen demographic benchmark results", "",
    f"Repository commit: `{commit}`; session: `{session_id}`; host identity: `{host_id}`.", "",
    "| Measurement | Replicate 1 s | Replicate 2 s | Replicate 3 s | Median s | Min–max s |",
    "|---|---:|---:|---:|---:|---:|",
]
for name, data in [
    ("CUDA no-grouped", c), ("CPU no-grouped", p),
    ("CPU full", doc["measurements"]["cpu_full"]),
    ("CPU no-ageing", doc["measurements"]["cpu_no_ageing"]),
]:
    r = data["replicates"]
    lines.append(
        f"| {name} | {r[0]:.3f} | {r[1]:.3f} | {r[2]:.3f} | "
        f"{data['median_seconds']:.3f} | {data['min_seconds']:.3f}–{data['max_seconds']:.3f} |"
    )
lines += [
    "", f"§L4 ratio: **{ratio:.3f}×**; verdict: **{verdict}**.", "",
    f"Ageing-share replicates: {', '.join(f'{x:.6f}' for x in a['replicates'])}; "
    f"median **{a['median']:.6f}**; range {a['min']:.6f}–{a['max']:.6f}. "
    f"This {v['ageing_evidence_direction']} the §K2 trigger evidence; §K2 is not decided here.", "",
]
(root / "bench-results.md").write_text("\n".join(lines))

assert all(doc["assertions"].values())
(root / "assertions.txt").write_text(
    "PASS one host and one session\n"
    "PASS repository commit identical for both backends and every replicate\n"
    "PASS binary SHA-256 identical for both backends and every replicate\n"
    "PASS initial-state SHA-256 identical for both backends and every replicate\n"
    "PASS gate results, summaries, and execution hashes identical for both backends and every replicate\n"
    "PASS exactly three no-grouped replicates per backend\n"
)
PY

rm -rf "$WORK"
cd "$OUT_ROOT"
find . -type f ! -name SHA256SUMS -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum -c SHA256SUMS >/dev/null
tar -czf "$HOME/demographic-bench.tar.gz" -C "$HOME" demographic-bench

# --- evidence push (optional) -------------------------------------------------
# Pushing happens *after* SHA256SUMS is written and verified, so the pushed tree
# is self-verifying: whoever pulls it can re-check every file against a manifest
# that was produced before the push existed.
#
# This is what makes a long run genuinely unattended. Without it, artifacts only
# reach the workstation if an SSH session is alive at the end of a multi-hour
# job, which on 2026-07-25 was the binding constraint rather than compute.
# It does NOT stop billing: the VM must still be destroyed.
# shellcheck disable=SC1091
[[ -f /etc/sembla-evidence-push.env ]] && . /etc/sembla-evidence-push.env
if [[ "${EVIDENCE_PUSH_ENABLED:-0}" == "1" ]]; then
  echo '=== pushing evidence to GitHub ==='
  PUSH_BRANCH="evidence/hyperstack-$(date -u +%Y%m%dT%H%M%SZ)"
  case "$PUSH_BRANCH" in
    evidence/*) ;;
    *) echo "refusing to push to a non-evidence branch: $PUSH_BRANCH" >&2; exit 6 ;;
  esac

  # A rented VM must not be able to disturb the trunk even by accident.
  if [[ "$PUSH_BRANCH" == "main" || "$PUSH_BRANCH" == "master" ]]; then
    echo 'refusing to push to a trunk branch' >&2
    exit 6
  fi

  # Refuse to publish anything that looks like a credential. The evidence tree
  # is timings, hashes, and provenance text; a key or token in it means
  # something upstream is wrong, and pushing is irreversible.
  if grep -rIl -E 'BEGIN [A-Z ]*PRIVATE KEY|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|api[_-]?key' \
      "$OUT_ROOT" >/dev/null 2>&1; then
    echo 'refusing to push: evidence tree contains something that looks like a credential' >&2
    grep -rIl -E 'BEGIN [A-Z ]*PRIVATE KEY|ghp_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|api[_-]?key' "$OUT_ROOT" >&2 || true
    exit 6
  fi

  PUSH_TREE="$HOME/evidence-push"
  rm -rf "$PUSH_TREE"
  git clone --quiet --no-checkout "$SPIKE_DIR" "$PUSH_TREE"
  cd "$PUSH_TREE"
  git config user.email 'evidence@sembla.invalid'
  git config user.name 'Sembla evidence collector'
  git remote remove origin
  git remote add origin "$EVIDENCE_PUSH_REMOTE"
  # An orphan branch: evidence is an artifact, not a change to the source tree,
  # and a detached history cannot conflict with or accidentally revert trunk.
  git checkout --quiet --orphan "$PUSH_BRANCH"
  git rm -rq --cached . 2>/dev/null || true
  rm -rf ./*
  mkdir -p "docs/evidence/demographic-bench/$(basename "$OUT_ROOT")"
  cp -R "$OUT_ROOT/." "docs/evidence/demographic-bench/$(basename "$OUT_ROOT")/"
  git add -A
  git commit --quiet -m "Evidence: $(basename "$OUT_ROOT") from $(cat "$OUT_ROOT/host-identity.sha256")"
  if git push --quiet origin "HEAD:refs/heads/$PUSH_BRANCH"; then
    echo "SEMBLA_EVIDENCE_PUSHED $PUSH_BRANCH"
    printf '%s\n' "$PUSH_BRANCH" > "$HOME/evidence-branch"
  else
    # A failed push must not fail the run: the tarball is still on disk and the
    # SSH path still works. Say so loudly rather than exiting non-zero.
    echo 'SEMBLA_EVIDENCE_PUSH_FAILED (tarball intact; collect over SSH)' >&2
  fi
  cd "$OUT_ROOT"
fi

echo SEMBLA_BENCH_COMPLETE
REMOTE_EOF

# The frozen run is long enough that a workstation interruption is plausible.
# Detach it so a laptop sleeping, a network drop, or a closed lid cannot kill
# the work. A retry must never overwrite a payload that a detached shell may
# still be reading: identify the immutable payload by its content hash and
# upload only when no matching run is active or complete.
PAYLOAD_SHA="$(python3 - "$REMOTE_SCRIPT" <<'PY'
import hashlib, pathlib, sys
print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())
PY
)"
REMOTE_STATE="$(ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'bash -s' <<'REMOTE_STATE_EOF'
set -Eeuo pipefail
payload_sha="$(cat ~/bench.payload.sha256 2>/dev/null || true)"
if [[ -f ~/bench.pid ]] && kill -0 "$(cat ~/bench.pid)" 2>/dev/null; then
  printf 'running %s\n' "$payload_sha"
elif [[ "$(cat ~/bench.status 2>/dev/null || true)" == SEMBLA_BENCH_COMPLETE ]]; then
  printf 'complete %s\n' "$payload_sha"
else
  printf 'idle\n'
fi
REMOTE_STATE_EOF
)"
case "$REMOTE_STATE" in
  "running $PAYLOAD_SHA"|"complete $PAYLOAD_SHA")
    echo "Rejoining benchmark state: $REMOTE_STATE"
    ;;
  running\ *|complete\ *)
    echo "remote benchmark uses a different payload; refusing to overwrite it: $REMOTE_STATE" >&2
    exit 1
    ;;
  idle)
    REMOTE_PAYLOAD="bench-payload-$PAYLOAD_SHA.sh"
    scp "${SSH_OPTIONS[@]}" "$REMOTE_SCRIPT" "$REMOTE:$REMOTE_PAYLOAD" >/dev/null
    ssh "${SSH_OPTIONS[@]}" "$REMOTE" "bash -s" <<REMOTE_LAUNCH
set -Eeuo pipefail
printf '%s\n' '$PAYLOAD_SHA' > ~/bench.payload.sha256
rm -f ~/bench.log ~/bench.status
setsid nohup bash -c '
  if bash ~/$REMOTE_PAYLOAD; then
    echo SEMBLA_BENCH_COMPLETE > ~/bench.status
  else
    echo "SEMBLA_BENCH_FAILED rc=\$?" > ~/bench.status
  fi
' > ~/bench.log 2>&1 < /dev/null &
echo \$! > ~/bench.pid
echo "started benchmark as PID \$(cat ~/bench.pid)"
REMOTE_LAUNCH
    ;;
  *)
    echo "unexpected remote benchmark state: $REMOTE_STATE" >&2
    exit 1
    ;;
esac

echo "Benchmark running detached on $REMOTE. Polling until it finishes."
echo "A dropped connection is harmless: re-run this script to rejoin."
bench_status=""
poll_deadline=$((SECONDS + ${BENCH_TIMEOUT_SECONDS:-43200}))

# SSH failures during the poll are REPORTED, never swallowed. Until 2026-07-27
# both the status read and the progress read ended in `2>/dev/null || true`, so
# an unreachable VM was indistinguishable from a healthy one mid-phase: the loop
# printed nothing and spun for its full 12 hours while the machine billed. That
# is precisely what happened when the operator's egress IP rotated mid-run --
# both firewalls pin the /32, so every connection began timing out silently.
#
# A single failure is still not fatal. Transient refusals are normal while the
# guest reboots or the tailnet re-handshakes, so the loop tolerates a run of
# them and aborts only once the path looks genuinely gone.
ssh_failures=0
SSH_FAILURE_LIMIT="${SSH_FAILURE_LIMIT:-5}"
# Cleaned up by `finish`, not a second EXIT trap: a second `trap ... EXIT` would
# REPLACE the first and silently disable the "a VM may still be billing" warning.
POLL_ERR="$(mktemp)"
poll_err="$POLL_ERR"

while (( SECONDS < poll_deadline )); do
  if bench_status="$(ssh "${SSH_OPTIONS[@]}" "$REMOTE" \
      'cat ~/bench.status 2>/dev/null || true' 2>"$poll_err")"; then
    if (( ssh_failures > 0 )); then
      echo "SSH to $REMOTE recovered after $ssh_failures consecutive failure(s)." >&2
    fi
    ssh_failures=0
    [[ -n "$bench_status" ]] && break
    # Surface the current phase so a watcher can see progress without attaching.
    ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'tail -1 ~/bench.log 2>/dev/null || true' \
      2>>"$poll_err" || true
  else
    ssh_failures=$((ssh_failures + 1))
    echo "SSH to $REMOTE failed ($ssh_failures/$SSH_FAILURE_LIMIT): $(tr '\n' ' ' <"$poll_err")" >&2
    if (( ssh_failures >= SSH_FAILURE_LIMIT )); then
      echo >&2
      echo "Giving up on the SSH path after $ssh_failures consecutive failures." >&2
      echo "THE VM IS STILL RUNNING AND STILL BILLING. The remote job is detached" >&2
      echo "and unaffected, so nothing is lost -- but the path to it is gone." >&2
      echo >&2
      echo "Most likely cause, in order:" >&2
      echo "  1. Your egress IP changed. Both the Hyperstack security group and" >&2
      echo "     the guest iptables rule pin a /32, so a rotation locks you out" >&2
      echo "     of a healthy machine. Compare:" >&2
      echo "       curl -s https://api.ipify.org" >&2
      echo "       grep ssh_cidr $TFVARS_FILE" >&2
      echo "     Do NOT re-apply to fix it: ssh_cidr is ForceNew and re-applying" >&2
      echo "     destroys the running benchmark. Add an API-side rule instead," >&2
      echo "     and open the guest rule from the VNC console." >&2
      echo "  2. The tailnet node dropped. Check 'tailscale status'." >&2
      echo "  3. The guest hit emergency_poweroff_hours." >&2
      echo >&2
      echo "Evidence may still arrive without SSH: the payload pushes to an" >&2
      echo "evidence/<UTC> branch, retrievable with 'git fetch' from anywhere." >&2
      echo "Confirm billing is stopped either way: bash reconcile-orphans.sh" >&2
      exit 1
    fi
  fi
  sleep 120
done

if ! ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'cat ~/bench.log' \
    > "$ARTIFACT_DIR/remote-run.log" 2>"$poll_err"; then
  echo "warning: could not retrieve ~/bench.log: $(tr '\n' ' ' <"$poll_err")" >&2
fi
EVIDENCE_BRANCH="$(ssh "${SSH_OPTIONS[@]}" "$REMOTE" \
  'cat ~/evidence-branch 2>/dev/null || true' 2>"$poll_err" || true)"
if [[ -z "$EVIDENCE_BRANCH" && -s "$poll_err" ]]; then
  echo "warning: could not read the evidence branch name: $(tr '\n' ' ' <"$poll_err")" >&2
fi
if [[ -n "$EVIDENCE_BRANCH" ]]; then
  echo "Evidence was pushed to branch: $EVIDENCE_BRANCH"
  echo "It is retrievable with 'git fetch origin $EVIDENCE_BRANCH' from anywhere,"
  echo "independently of this session surviving."
  printf '%s\n' "$EVIDENCE_BRANCH" > "$ARTIFACT_DIR/evidence-branch.txt"
fi

if [[ "$bench_status" != "SEMBLA_BENCH_COMPLETE" ]]; then
  echo "Remote benchmark did not complete: ${bench_status:-still running at timeout}" >&2
  echo "Log: $ARTIFACT_DIR/remote-run.log" >&2
  echo "The VM is still up. Re-run this script to rejoin, or destroy it now." >&2
  if [[ -n "$EVIDENCE_BRANCH" ]]; then
    echo "Note: evidence for a completed earlier phase is already on $EVIDENCE_BRANCH." >&2
  fi
  exit 1
fi

scp "${SSH_OPTIONS[@]}" "$REMOTE:demographic-bench.tar.gz" "$ARTIFACT_DIR/" >/dev/null
tar -xzf "$ARTIFACT_DIR/demographic-bench.tar.gz" -C "$ARTIFACT_DIR" --strip-components=1
rm -f "$ARTIFACT_DIR/demographic-bench.tar.gz"
mv "$ARTIFACT_DIR/SHA256SUMS" "$ARTIFACT_DIR/SHA256SUMS.remote"

if command -v sha256sum >/dev/null; then
  if ! (cd "$ARTIFACT_DIR" && sha256sum -c SHA256SUMS.remote >/dev/null); then
    echo "artifact checksum verification failed; refusing teardown" >&2
    exit 1
  fi
elif command -v shasum >/dev/null; then
  if ! (cd "$ARTIFACT_DIR" && shasum -a 256 -c SHA256SUMS.remote >/dev/null); then
    echo "artifact checksum verification failed; refusing teardown" >&2
    exit 1
  fi
else
  echo "no SHA-256 verification utility; refusing teardown" >&2
  exit 1
fi
echo "Transferred artifact checksums verified."

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

# The remote checksum set proves transfer integrity. Regenerate it after local
# collection and teardown so it also covers bootstrap/driver logs and the final
# empty Terraform-state assertion, then verify the exact committed directory.
python3 - "$ARTIFACT_DIR" <<'PY'
import hashlib, pathlib, sys
root = pathlib.Path(sys.argv[1])
lines = []
for path in sorted(p for p in root.rglob("*") if p.is_file() and p.name != "SHA256SUMS"):
    digest = hashlib.sha256(path.read_bytes()).hexdigest()
    lines.append(f"{digest}  {path.relative_to(root).as_posix()}")
(root / "SHA256SUMS").write_text("\n".join(lines) + "\n")
PY
if command -v sha256sum >/dev/null; then
  (cd "$ARTIFACT_DIR" && sha256sum -c SHA256SUMS >/dev/null)
elif command -v shasum >/dev/null; then
  (cd "$ARTIFACT_DIR" && shasum -a 256 -c SHA256SUMS >/dev/null)
else
  echo "no SHA-256 verification utility for final evidence" >&2
  exit 1
fi
echo "Final evidence checksums verified."

echo
echo "Evidence written to: $ARTIFACT_DIR"
for f in "$ARTIFACT_DIR/README.md" "$ARTIFACT_DIR/bench-results.md"; do
  [[ -f "$f" ]] && { echo; echo "--- $f ---"; cat "$f"; }
done
