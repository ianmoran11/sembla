#!/usr/bin/env bash
# Generate the ephemeral ED25519 SSH *host* key that cloud-init pre-seeds, so the
# host-key fingerprint is known before the VM boots and trust-on-first-use never
# happens. This replaces the manual VNC-console fingerprint read (README §4).
#
#   eval "$(bash prepare-host-key.sh)"
#
# emits the two exports the apply and the collector need, and nothing else on
# stdout. The key is single-use: generate a new one per VM, and let it die with
# the VM. Key material is written under the module directory, which is ignored by
# the module's allowlist .gitignore.
set -Eeuo pipefail

MODULE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KEY_DIR="${HOST_KEY_DIR:-$MODULE_DIR/.host-key-$(date -u +%Y%m%dT%H%M%SZ)}"
KEY_PATH="$KEY_DIR/ssh_host_ed25519_key"

if [[ -e "$KEY_PATH" ]]; then
  echo "refusing to overwrite an existing host key at $KEY_PATH" >&2
  exit 2
fi

mkdir -p "$KEY_DIR"
chmod 0700 "$KEY_DIR"
ssh-keygen -t ed25519 -N '' -C "sembla-ephemeral-host-key" -f "$KEY_PATH" >/dev/null
chmod 0600 "$KEY_PATH"
chmod 0644 "$KEY_PATH.pub"

FINGERPRINT="$(ssh-keygen -E sha256 -lf "$KEY_PATH.pub" | awk '{print $2}')"

# TF_VAR_ssh_host_private_key carries the key into the rendered user-data only;
# like the console password hash it must never be placed in a tfvars file.
printf 'export TF_VAR_ssh_host_private_key=%q\n' "$(cat "$KEY_PATH")"
printf 'export SSH_HOST_KEY_FINGERPRINT=%q\n' "$FINGERPRINT"
printf 'export SEMBLA_HOST_KEY_DIR=%q\n' "$KEY_DIR"

{
  echo "Generated an ephemeral ED25519 host key."
  echo "  directory:   $KEY_DIR"
  echo "  fingerprint: $FINGERPRINT"
  echo
  echo "The fingerprint above is now known before the VM exists, so no console"
  echo "read is required. Destroy the directory after the VM is destroyed:"
  echo "  rm -rf $KEY_DIR"
} >&2
