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
PUBLIC_IP="$(terraform output -raw public_ip)"
SSH_USER="$(terraform output -raw ssh_user)"
if [[ -z "$PUBLIC_IP" || "$PUBLIC_IP" == "null" ]]; then
  echo "Terraform has no public IP yet. Run terraform refresh once if apply just completed." >&2
  exit 2
fi

mkdir -p "$ARTIFACT_DIR"
terraform output -json selected_profile > "$ARTIFACT_DIR/selected-profile.json"
COLLECTION_ID="$(python3 -c 'import secrets; print(secrets.token_hex(16))')"
printf '%s\n' "$COLLECTION_ID" > "$ARTIFACT_DIR/run-collection-id.txt"

SCANNED_HOST_KEY=""
for _ in $(seq 1 60); do
  SCANNED_HOST_KEY="$(ssh-keyscan -T 10 -t ed25519 "$PUBLIC_IP" 2>/dev/null || true)"
  [[ -n "$SCANNED_HOST_KEY" ]] && break
  sleep 5
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
printf '%s\n' "$SCANNED_HOST_KEY" > "$KNOWN_HOSTS_FILE"
chmod 0600 "$KNOWN_HOSTS_FILE"

SSH_OPTIONS=(
  -i "$SSH_PRIVATE_KEY_PATH"
  -o BatchMode=yes
  -o ConnectTimeout=10
  -o StrictHostKeyChecking=yes
  -o UserKnownHostsFile="$KNOWN_HOSTS_FILE"
)
REMOTE="$SSH_USER@$PUBLIC_IP"

collect_remote_diagnostics() {
  scp "${SSH_OPTIONS[@]}" "$REMOTE:/var/log/sembla-bootstrap.log" \
    "$ARTIFACT_DIR/bootstrap.log" >/dev/null 2>&1 || true
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
for _ in $(seq 1 90); do
  if ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'test -f /var/lib/sembla-bootstrap/failed'; then
    collect_remote_diagnostics
    echo "Cloud-init failed; retrieved complete diagnostics where available" >&2
    exit 1
  fi
  if ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'test -f /var/lib/sembla-bootstrap/ready'; then
    break
  fi
  sleep 10
done
if ! ssh "${SSH_OPTIONS[@]}" "$REMOTE" 'test -f /var/lib/sembla-bootstrap/ready'; then
  collect_remote_diagnostics
  echo "Timed out waiting for cloud-init" >&2
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
