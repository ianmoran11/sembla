#!/usr/bin/env bash
set -Eeuo pipefail

MODULE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SEED_PATH="${RESULTS_SEED_PATH:-$MODULE_DIR/../RESULTS.md}"
SSH_PRIVATE_KEY_PATH="${SSH_PRIVATE_KEY_PATH:-$HOME/.ssh/sembla_hyperstack}"
ARTIFACT_DIR="${ARTIFACT_DIR:-$MODULE_DIR/artifacts-hyperstack-$(date -u +%Y%m%dT%H%M%SZ)}"
KNOWN_HOSTS_FILE="$MODULE_DIR/.hyperstack_known_hosts"

remind_destroy() {
  printf '\n%s\n' "IMPORTANT: inspect the Hyperstack console now. Billing continues in SHUTOFF; delete any VM that Terraform did not destroy." >&2
}
trap remind_destroy EXIT

: "${SSH_HOST_KEY_FINGERPRINT:?set this to the SHA256 host-key fingerprint independently verified in the Hyperstack console/VNC}"
if [[ ! "$SSH_HOST_KEY_FINGERPRINT" =~ ^SHA256:[A-Za-z0-9+/]+={0,2}$ ]]; then
  echo "SSH_HOST_KEY_FINGERPRINT must look like SHA256:..." >&2
  exit 2
fi
if [[ ! -f "$SSH_PRIVATE_KEY_PATH" ]]; then
  echo "SSH private key not found: $SSH_PRIVATE_KEY_PATH" >&2
  exit 2
fi
if [[ ! -s "$SEED_PATH" ]]; then
  echo "Mac-containing seed result not found or empty: $SEED_PATH" >&2
  exit 2
fi

cd "$MODULE_DIR"
PUBLIC_IP="$(terraform output -raw public_ip 2>/dev/null || true)"
if [[ -z "$PUBLIC_IP" || "$PUBLIC_IP" == "null" ]]; then
  PUBLIC_IP="$(
    terraform show -json | python3 -c '
import json
import sys

state = json.load(sys.stdin)
modules = [state.get("values", {}).get("root_module", {})]
while modules:
    module = modules.pop()
    for resource in module.get("resources", []):
        if resource.get("address") == "hyperstack_core_virtual_machine.gpu[0]":
            print(resource.get("values", {}).get("floating_ip") or "")
            raise SystemExit
    modules.extend(module.get("child_modules", []))
'
  )"
fi
SSH_USER="$(terraform output -raw ssh_user)"
if ! python3 - "$PUBLIC_IP" <<'PY'
import ipaddress
import sys

try:
    address = ipaddress.ip_address(sys.argv[1])
except ValueError:
    raise SystemExit(1)
raise SystemExit(0 if address.version == 4 and address.is_global else 1)
PY
then
  echo "Terraform state has no valid global public IPv4 for the paid VM." >&2
  exit 2
fi

mkdir -p "$ARTIFACT_DIR"
terraform output -json selected_profile > "$ARTIFACT_DIR/selected-profile.json"
COLLECTION_ID="$(python3 -c 'import secrets; print(secrets.token_hex(16))')"
printf '%s\n' "$COLLECTION_ID" > "$ARTIFACT_DIR/run-collection-id.txt"

SCANNED_HOST_KEY=""
host_key_deadline=$((SECONDS + 300))
host_key_delay=5
host_key_attempt=0
while (( SECONDS < host_key_deadline )); do
  host_key_attempt=$((host_key_attempt + 1))
  SCANNED_HOST_KEY="$(ssh-keyscan -T 10 -t ed25519 "$PUBLIC_IP" 2>/dev/null || true)"
  [[ -n "$SCANNED_HOST_KEY" ]] && break
  printf 'ED25519 host key is not ready (attempt %s); retrying in %ss.\n' \
    "$host_key_attempt" "$host_key_delay" >&2
  sleep "$host_key_delay"
  if (( host_key_delay < 30 )); then
    host_key_delay=$((host_key_delay + 5))
  fi
done
if [[ -z "$SCANNED_HOST_KEY" ]]; then
  echo "Could not retrieve the VM's ED25519 SSH host key" >&2
  exit 1
fi
ACTUAL_HOST_KEY_FINGERPRINT="$(
  printf '%s\n' "$SCANNED_HOST_KEY" | ssh-keygen -E sha256 -lf - | awk 'NR == 1 { print $2 }'
)"
if [[ "$ACTUAL_HOST_KEY_FINGERPRINT" != "$SSH_HOST_KEY_FINGERPRINT" ]]; then
  echo "SSH host-key fingerprint mismatch; refusing the connection" >&2
  echo "expected: $SSH_HOST_KEY_FINGERPRINT" >&2
  echo "actual:   $ACTUAL_HOST_KEY_FINGERPRINT" >&2
  exit 1
fi
printf '%s\n' "$SSH_HOST_KEY_FINGERPRINT" \
  > "$ARTIFACT_DIR/trusted-ssh-host-fingerprint.txt"
printf '%s\n' "$SCANNED_HOST_KEY" > "$ARTIFACT_DIR/ssh-host-key.pub"
printf '%s\n' "$SCANNED_HOST_KEY" > "$KNOWN_HOSTS_FILE"
chmod 0600 "$KNOWN_HOSTS_FILE"

SSH_OPTIONS=(
  -i "$SSH_PRIVATE_KEY_PATH"
  -o BatchMode=yes
  -o IdentitiesOnly=yes
  -o IPQoS=none
  -o ConnectTimeout=10
  -o ConnectionAttempts=1
  -o ServerAliveInterval=5
  -o ServerAliveCountMax=2
  -o StrictHostKeyChecking=yes
  -o UserKnownHostsFile="$KNOWN_HOSTS_FILE"
)
REMOTE="$SSH_USER@$PUBLIC_IP"

collect_remote_diagnostics() {
  scp "${SSH_OPTIONS[@]}" "$REMOTE:/var/log/sembla-bootstrap.log" \
    "$ARTIFACT_DIR/bootstrap.log" >/dev/null 2>&1 || true
  scp "${SSH_OPTIONS[@]}" "$REMOTE:/var/lib/sembla-bootstrap/diagnostics.log" \
    "$ARTIFACT_DIR/bootstrap-diagnostics.log" >/dev/null 2>&1 || true
  scp "${SSH_OPTIONS[@]}" "$REMOTE:/var/lib/sembla-bootstrap/ssh-self-test.pub" \
    "$ARTIFACT_DIR/ssh-self-test.pub" >/dev/null 2>&1 || true
  ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'nvidia-smi -q' \
    > "$ARTIFACT_DIR/nvidia-smi-q.txt" 2>&1 || true
  ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'cat /var/lib/sembla-bootstrap/repository-commit' \
    > "$ARTIFACT_DIR/repository-commit.txt" 2>&1 || true
  for run in 1 2 3; do
    scp "${SSH_OPTIONS[@]}" "$REMOTE:/home/$SSH_USER/RESULTS.run-$run.md" \
      "$ARTIFACT_DIR/RESULTS.run-$run.md" >/dev/null 2>&1 || true
    scp "${SSH_OPTIONS[@]}" "$REMOTE:/home/$SSH_USER/sembla-run-$run.log" \
      "$ARTIFACT_DIR/run-$run.log" >/dev/null 2>&1 || true
  done
}

echo "Waiting for cloud-init on $REMOTE ..."
BOOTSTRAP_TIMEOUT_SECONDS="${BOOTSTRAP_TIMEOUT_SECONDS:-1200}"
bootstrap_deadline=$((SECONDS + BOOTSTRAP_TIMEOUT_SECONDS))
bootstrap_delay=5
bootstrap_attempt=0
bootstrap_ready=false
while (( SECONDS < bootstrap_deadline )); do
  bootstrap_attempt=$((bootstrap_attempt + 1))
  if bootstrap_status="$(
    ssh "${SSH_OPTIONS[@]}" "$REMOTE" \
      'if test -f /var/lib/sembla-bootstrap/failed; then echo failed; elif test -f /var/lib/sembla-bootstrap/ready; then echo ready; else echo running; fi' \
      2> "$ARTIFACT_DIR/ssh-readiness-last.err"
  )"; then
    case "$bootstrap_status" in
      failed)
        collect_remote_diagnostics
        echo "Cloud-init failed; retrieved complete diagnostics where available" >&2
        exit 1
        ;;
      ready)
        bootstrap_ready=true
        break
        ;;
      running)
        printf 'Bootstrap is still running (attempt %s).\n' "$bootstrap_attempt"
        ;;
      *)
        printf 'Unexpected bootstrap status %q; retrying.\n' "$bootstrap_status" >&2
        ;;
    esac
  else
    printf 'SSH is not ready (attempt %s); retrying in %ss.\n' \
      "$bootstrap_attempt" "$bootstrap_delay" >&2
  fi
  sleep "$bootstrap_delay"
  if (( bootstrap_delay < 30 )); then
    bootstrap_delay=$((bootstrap_delay + 5))
  fi
done
if [[ "$bootstrap_ready" != true ]]; then
  collect_remote_diagnostics
  echo "Timed out waiting for cloud-init after ${BOOTSTRAP_TIMEOUT_SECONDS}s" >&2
  exit 1
fi

SEED_SHA="$(shasum -a 256 "$SEED_PATH" | awk '{print $1}')"
scp "${SSH_OPTIONS[@]}" "$SEED_PATH" "$REMOTE:/home/$SSH_USER/RESULTS.seed.md"
ssh "${SSH_OPTIONS[@]}" "$REMOTE" "
  set -eu
  for i in 1 2 3; do cp /home/$SSH_USER/RESULTS.seed.md /home/$SSH_USER/RESULTS.run-\$i.md; done
  test \"\$(sha256sum /home/$SSH_USER/RESULTS.run-{1,2,3}.md | awk '{print \$1}' | sort -u | wc -l)\" -eq 1
  test \"\$(sha256sum /home/$SSH_USER/RESULTS.seed.md | awk '{print \$1}')\" = '$SEED_SHA'
"

echo "Seeded three byte-identical result files ($SEED_SHA)."
for run in 1 2 3; do
  remote_result="/home/$SSH_USER/RESULTS.run-$run.md"
  remote_log="/home/$SSH_USER/sembla-run-$run.log"
  run_id="$COLLECTION_ID-run-$run"
  echo "Starting NVIDIA decision run $run of 3 ($run_id) ..."
  if ! ssh "${SSH_OPTIONS[@]}" "$REMOTE" \
    "SPIKE_RUN_ID='$run_id' SPIKE_RESULTS_PATH='$remote_result' /home/$SSH_USER/run-spike.sh > '$remote_log' 2>&1"; then
    collect_remote_diagnostics
    echo "Run $run failed; retrieved diagnostics where available" >&2
    exit 1
  fi
  scp "${SSH_OPTIONS[@]}" "$REMOTE:$remote_result" "$ARTIFACT_DIR/RESULTS.run-$run.md"
  scp "${SSH_OPTIONS[@]}" "$REMOTE:$remote_log" "$ARTIFACT_DIR/run-$run.log"
  test -s "$ARTIFACT_DIR/RESULTS.run-$run.md"
  test -s "$ARTIFACT_DIR/run-$run.log"
  if [[ "$(shasum -a 256 "$ARTIFACT_DIR/RESULTS.run-$run.md" | awk '{print $1}')" == "$SEED_SHA" ]]; then
    echo "Run $run returned the unchanged seed instead of NVIDIA evidence" >&2
    exit 1
  fi
done

collect_remote_diagnostics
python3 "$MODULE_DIR/verify-artifacts.py" "$ARTIFACT_DIR"

printf '%s\n' \
  "Collected and verified three independent result files, three external logs, bootstrap log, GPU details, and repository commit." \
  "Artifacts: $ARTIFACT_DIR" \
  "Run terraform destroy now, then confirm deletion in the Hyperstack console."
