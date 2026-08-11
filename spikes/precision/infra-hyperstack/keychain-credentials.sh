#!/usr/bin/env bash
# Persist long-lived provider credentials in macOS Keychain and expose them only
# to an explicitly credentialed child shell/command. Never run with xtrace.
{ set +x; } 2>/dev/null
set -Eeuo pipefail
# Discard any inherited provider key before even resolving local paths. Each
# credentialed command reloads the reviewed Keychain value at the last moment.
unset HYPERSTACK_API_KEY

MODULE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KEYCHAIN_ACCOUNT="${SEMBLA_KEYCHAIN_ACCOUNT:-${USER:-}}"
HYPERSTACK_SERVICE="${SEMBLA_HYPERSTACK_KEYCHAIN_SERVICE:-sembla.hyperstack.api-key}"
TAILSCALE_CLIENT_ID_SERVICE="${SEMBLA_TAILSCALE_CLIENT_ID_KEYCHAIN_SERVICE:-sembla.tailscale.oauth-client-id}"
TAILSCALE_CLIENT_SECRET_SERVICE="${SEMBLA_TAILSCALE_CLIENT_SECRET_KEYCHAIN_SERVICE:-sembla.tailscale.oauth-client-secret}"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:-ianmoran11/sembla}"
TAILSCALE_AUTH_KEY_HELPER="$MODULE_DIR/tailscale-auth-key.py"

SESSION_NAMES=(
  TF_VAR_tailscale_auth_key
  TF_VAR_console_password_hash
  TF_VAR_evidence_deploy_key
  TF_VAR_ssh_host_private_key
  SSH_HOST_KEY_FINGERPRINT
  SEMBLA_HOST_KEY_DIR
  SEMBLA_EVIDENCE_DEPLOY_KEY_ID
  SEMBLA_TAILSCALE_AUTH_KEY_ID
)

usage() {
  cat <<'EOF'
Usage:
  bash keychain-credentials.sh store
  bash keychain-credentials.sh check
  bash keychain-credentials.sh prepare-shell [--repo OWNER/REPO]
  bash keychain-credentials.sh shell
  bash keychain-credentials.sh exec COMMAND [ARG ...]
  bash keychain-credentials.sh cleanup-session [--repo OWNER/REPO]

store
  One-time secure prompts for the long-lived Hyperstack API key and a Tailscale
  OAuth client ID/secret. Existing items are replaced. The OAuth client must
  have only the auth_keys scope for tag:sembla-bench.

prepare-shell
  Asks prepare-paid-session.sh to mint a one-off Tailscale key from Keychain,
  then loads the Hyperstack key, imports the resulting per-session launchctl
  values, and opens a login shell. Preparation helpers never inherit the
  billing-capable Hyperstack key.

shell
  Opens a credentialed login shell for an already prepared session.

exec
  Runs one command with the Hyperstack key and any prepared launchctl session
  values. This works from an already-running Pi process without restarting it.

cleanup-session
  After verified VM/rule destruction, revokes the disposable Tailscale key and
  GitHub deploy key, removes the per-VM host-key directory and stale saved plan,
  and clears launchctl. Long-lived Keychain items are retained.
EOF
}

require_security() {
  [[ -n "$KEYCHAIN_ACCOUNT" ]] \
    || { echo 'could not determine Keychain account; set SEMBLA_KEYCHAIN_ACCOUNT' >&2; exit 1; }
  command -v security >/dev/null 2>&1 \
    || { echo 'macOS security command not found; Keychain workflow requires macOS' >&2; exit 1; }
}

read_item() {
  local service="$1" value=''
  if ! value="$(security find-generic-password \
      -a "$KEYCHAIN_ACCOUNT" -s "$service" -w 2>/dev/null)"; then
    echo "Keychain item unavailable: $service" >&2
    return 1
  fi
  [[ -n "$value" ]] || { echo "Keychain item is empty: $service" >&2; return 1; }
  printf '%s' "$value"
}

validate_items() {
  local hyperstack='' client_id='' client_secret=''
  hyperstack="$(read_item "$HYPERSTACK_SERVICE")"
  client_id="$(read_item "$TAILSCALE_CLIENT_ID_SERVICE")"
  client_secret="$(read_item "$TAILSCALE_CLIENT_SECRET_SERVICE")"
  [[ "$hyperstack" == "${hyperstack//[[:space:]]/}" ]] \
    || { echo 'stored Hyperstack API key contains whitespace' >&2; return 1; }
  [[ "$client_id" == "${client_id//[[:space:]]/}" ]] \
    || { echo 'stored Tailscale OAuth client ID contains whitespace' >&2; return 1; }
  [[ "$client_secret" =~ ^tskey-client-[A-Za-z0-9-]+$ ]] \
    || { echo 'stored Tailscale OAuth client secret has an unexpected format' >&2; return 1; }
  unset hyperstack client_id client_secret
}

store_item() {
  local service="$1" label="$2"
  printf '\nStore %s in macOS Keychain. Input is handled by security and is not echoed.\n' \
    "$label" >&2
  # Keep -w last so /usr/bin/security prompts instead of receiving the secret in
  # argv, shell history, or this process environment.
  security add-generic-password -U -a "$KEYCHAIN_ACCOUNT" \
    -s "$service" -l "$label" -w
}

load_hyperstack() {
  local value=''
  # Do not let an inherited provider key reach the Keychain subprocess.
  unset HYPERSTACK_API_KEY
  value="$(read_item "$HYPERSTACK_SERVICE")"
  [[ "$value" == "${value//[[:space:]]/}" ]] \
    || { echo 'stored Hyperstack API key contains whitespace' >&2; return 1; }
  export HYPERSTACK_API_KEY="$value"
  unset value
}

import_session() {
  local name value
  command -v launchctl >/dev/null 2>&1 \
    || { echo 'launchctl not found; prepared-session import requires macOS' >&2; return 1; }
  for name in "${SESSION_NAMES[@]}"; do
    if ! value="$(launchctl getenv "$name")"; then
      echo "could not inspect prepared session value: $name" >&2
      return 1
    fi
    if [[ -n "$value" ]]; then
      export "$name=$value"
    else
      unset "$name" 2>/dev/null || true
    fi
  done
}

open_shell() {
  local paid_shell="${SHELL:-/bin/bash}"
  [[ -x "$paid_shell" ]] \
    || { echo "configured shell is not executable: $paid_shell" >&2; exit 1; }
  umask 077
  cat >&2 <<'EOF'
Opening a credentialed paid-session shell.
The Hyperstack API key exists only in this shell and its children; it was not
written to launchctl, Terraform variables, or repository files. Exit the shell
to remove that process-environment copy.
EOF
  exec "$paid_shell" -l
}

prepare_shell() {
  local hyperstack_value=''
  # Do not pass an inherited provider key to Keychain/session preparation.
  unset HYPERSTACK_API_KEY
  # Read and validate before minting so a missing/locked provider credential
  # cannot strand a prepared session. Keep it unexported while preparation
  # invokes GitHub, Keychain, and local key helpers.
  hyperstack_value="$(read_item "$HYPERSTACK_SERVICE")"
  [[ "$hyperstack_value" == "${hyperstack_value//[[:space:]]/}" ]] \
    || { echo 'stored Hyperstack API key contains whitespace' >&2; return 1; }
  (
    unset HYPERSTACK_API_KEY
    exec bash "$MODULE_DIR/prepare-paid-session.sh" --tailscale-oauth-keychain "$@"
  )
  import_session
  export HYPERSTACK_API_KEY="$hyperstack_value"
  unset hyperstack_value
  open_shell
}

cleanup_session() {
  local repository="$GITHUB_REPOSITORY" state='' tailscale_id='' deploy_id=''
  # A caller may invoke cleanup from the credentialed shell. Keep the provider
  # key out of every cleanup subprocess except the reconciliation subshell.
  unset HYPERSTACK_API_KEY
  local host_dir='' client_id='' client_secret='' keys='' remaining=''
  while (( $# )); do
    case "$1" in
      --repo)
        [[ $# -ge 2 ]] || { echo '--repo requires OWNER/REPO' >&2; return 2; }
        repository="$2"; shift 2 ;;
      *) echo "unknown cleanup-session argument: $1" >&2; return 2 ;;
    esac
  done
  [[ "$repository" =~ ^[^/]+/[^/]+$ ]] \
    || { echo 'repository must look like OWNER/REPO' >&2; return 2; }
  for command in terraform gh jq python3 launchctl; do
    command -v "$command" >/dev/null 2>&1 \
      || { echo "required command not found: $command" >&2; return 1; }
  done
  [[ -r "$TAILSCALE_AUTH_KEY_HELPER" ]] \
    || { echo "required helper not readable: $TAILSCALE_AUTH_KEY_HELPER" >&2; return 1; }

  if ! state="$(cd "$MODULE_DIR" && terraform state list 2>/dev/null)"; then
    echo 'could not inspect Terraform state; refusing credential cleanup' >&2
    return 1
  fi
  if grep -Eq 'hyperstack_core_virtual_machine\.gpu|hyperstack_core_virtual_machine_sg_rule\.ssh' \
      <<<"$state"; then
    echo 'paid VM/security-rule state still exists; destroy and verify before cleanup' >&2
    return 1
  fi
  if ! (
    load_hyperstack
    cd "$MODULE_DIR"
    exec bash reconcile-orphans.sh
  ); then
    echo 'provider reconciliation is not clean; refusing credential cleanup' >&2
    return 1
  fi

  import_session
  tailscale_id="${SEMBLA_TAILSCALE_AUTH_KEY_ID:-}"
  if [[ -n "$tailscale_id" ]]; then
    client_id="$(read_item "$TAILSCALE_CLIENT_ID_SERVICE")"
    client_secret="$(read_item "$TAILSCALE_CLIENT_SECRET_SERVICE")"
    if ! printf '%s\0%s\0' "$client_id" "$client_secret" \
        | python3 "$TAILSCALE_AUTH_KEY_HELPER" delete --key-id "$tailscale_id"; then
      echo "could not revoke/verify Tailscale auth key $tailscale_id" >&2
      return 1
    fi
    unset client_id client_secret
  fi

  deploy_id="${SEMBLA_EVIDENCE_DEPLOY_KEY_ID:-}"
  if [[ -n "$deploy_id" ]]; then
    [[ "$deploy_id" =~ ^[0-9]+$ ]] \
      || { echo 'prepared GitHub deploy-key ID is invalid' >&2; return 1; }
    keys="$(gh api --paginate --slurp "repos/$repository/keys")" \
      || { echo 'could not list GitHub deploy keys' >&2; return 1; }
    if jq -e --argjson id "$deploy_id" '.[][] | select(.id == $id)' \
        <<<"$keys" >/dev/null; then
      gh api --method DELETE "repos/$repository/keys/$deploy_id" >/dev/null \
        || { echo "could not delete GitHub deploy key $deploy_id" >&2; return 1; }
    fi
    remaining="$(gh api --paginate --slurp "repos/$repository/keys")" \
      || { echo 'could not verify GitHub deploy-key deletion' >&2; return 1; }
    if jq -e --argjson id "$deploy_id" '.[][] | select(.id == $id)' \
        <<<"$remaining" >/dev/null; then
      echo "GitHub deploy key still exists after deletion: $deploy_id" >&2
      return 1
    fi
  fi

  host_dir="${SEMBLA_HOST_KEY_DIR:-}"
  if [[ -n "$host_dir" ]]; then
    case "$host_dir" in
      "$MODULE_DIR"/.host-key-*) rm -rf -- "$host_dir" ;;
      *) echo "refusing unsafe host-key cleanup path: $host_dir" >&2; return 1 ;;
    esac
    [[ ! -e "$host_dir" ]] \
      || { echo "host-key directory remains: $host_dir" >&2; return 1; }
  fi

  for name in "${SESSION_NAMES[@]}"; do
    launchctl unsetenv "$name" \
      || { echo "could not clear launchctl value: $name" >&2; return 1; }
    remaining="$(launchctl getenv "$name")" \
      || { echo "could not verify launchctl cleanup: $name" >&2; return 1; }
    [[ -z "$remaining" ]] \
      || { echo "launchctl value remains after cleanup: $name" >&2; return 1; }
    unset "$name" 2>/dev/null || true
  done
  rm -f -- "$MODULE_DIR/hyperstack-paid.tfplan"
  unset HYPERSTACK_API_KEY
  echo 'Paid-session launchctl/remote credentials cleaned; long-lived Keychain items were retained.'
  echo 'Exit the credentialed parent shell now; a child process cannot erase its parent environment.'
}

[[ $# -ge 1 ]] || { usage >&2; exit 2; }
command_name="$1"
shift
if [[ "$command_name" =~ ^(-h|--help|help)$ ]]; then
  usage
  exit 0
fi
require_security

case "$command_name" in
  store)
    [[ $# == 0 ]] || { echo 'store takes no arguments' >&2; exit 2; }
    store_item "$HYPERSTACK_SERVICE" 'Sembla Hyperstack API key'
    store_item "$TAILSCALE_CLIENT_ID_SERVICE" 'Sembla Tailscale OAuth client ID'
    store_item "$TAILSCALE_CLIENT_SECRET_SERVICE" 'Sembla Tailscale OAuth client secret'
    validate_items
    echo 'Keychain credentials stored and validated; no secret was printed.'
    ;;
  check)
    [[ $# == 0 ]] || { echo 'check takes no arguments' >&2; exit 2; }
    validate_items
    echo 'Keychain credentials are present and structurally valid.'
    ;;
  prepare-shell)
    prepare_shell "$@"
    ;;
  shell)
    [[ $# == 0 ]] || { echo 'shell takes no arguments' >&2; exit 2; }
    unset HYPERSTACK_API_KEY
    import_session
    load_hyperstack
    open_shell
    ;;
  exec)
    [[ $# -ge 1 ]] || { echo 'exec requires a command' >&2; exit 2; }
    unset HYPERSTACK_API_KEY
    import_session
    load_hyperstack
    umask 077
    exec "$@"
    ;;
  cleanup-session)
    cleanup_session "$@"
    ;;
  *)
    echo "unknown command: $command_name" >&2
    usage >&2
    exit 2
    ;;
esac
