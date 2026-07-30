#!/usr/bin/env bash
# Prepare every per-session secret/identity needed by a paid Hyperstack run.
# Secrets are prompted on /dev/tty, stored in launchctl, and never printed.
# Disable inherited/command-line xtrace before any secret can enter the shell.
{ set +x; } 2>/dev/null
set -Eeuo pipefail

MODULE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GITHUB_REPOSITORY="${GITHUB_REPOSITORY:-ianmoran11/sembla}"
CHECK_ONLY=0
SUCCESS=0
MUTATED=0
REGISTERED_DEPLOY_KEY_ID=''
DEPLOY_PUBLIC=''
HOST_KEY_DIR=''
RETURNED_HOST_KEY_DIR=''
WORK=''

usage() {
  cat <<'EOF'
Usage: bash prepare-paid-session.sh [--check] [--repo OWNER/REPO]

Interactively prepares one fresh paid-run session:
  1. prompts for a Tailscale ephemeral auth key;
  2. prompts twice for the temporary console password;
  3. generates and registers a write-enabled GitHub deploy key;
  4. generates the pre-seeded SSH host identity; and
  5. stores all values in launchctl without printing secrets.

--check only verifies local tools and GitHub branch protection.
EOF
}

while (( $# )); do
  case "$1" in
    --check) CHECK_ONLY=1; shift ;;
    --repo)
      [[ $# -ge 2 ]] || { echo '--repo requires OWNER/REPO' >&2; exit 2; }
      GITHUB_REPOSITORY="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

find_registered_deploy_key_id() {
  [[ -n "$DEPLOY_PUBLIC" ]] || return 1
  local keys
  if ! keys="$(gh api --paginate --slurp \
      "repos/$GITHUB_REPOSITORY/keys")"; then
    return 2
  fi
  jq -r --arg public "$DEPLOY_PUBLIC" '
      .[][]
      | select((.key | split(" ")[0:2] | join(" "))
          == ($public | split(" ")[0:2] | join(" ")))
      | .id
    ' <<<"$keys" | head -1
}

remove_owned_host_dir() {
  local path="$1"
  [[ -n "$path" ]] || return 0
  case "$path" in
    "$MODULE_DIR"/.host-key-*) ;;
    *)
      echo "ROLLBACK WARNING: refusing unsafe host-key cleanup path: $path" >&2
      return 1
      ;;
  esac
  rm -rf -- "$path" || return $?
  [[ ! -e "$path" ]]
}

cleanup() {
  local rc=$?
  local rollback_failed=0
  local rollback_id="$REGISTERED_DEPLOY_KEY_ID"
  local remaining=''
  local lookup_rc=0
  set +e
  unset TAILSCALE_AUTH_KEY TF_VAR_console_password_hash \
    TF_VAR_evidence_deploy_key TF_VAR_ssh_host_private_key
  if [[ "$SUCCESS" != 1 && "$MUTATED" == 1 ]]; then
    # Remove local secret state before attempting any network rollback.
    for name in TF_VAR_tailscale_auth_key TF_VAR_console_password_hash \
      TF_VAR_evidence_deploy_key TF_VAR_ssh_host_private_key \
      SSH_HOST_KEY_FINGERPRINT SEMBLA_HOST_KEY_DIR \
      SEMBLA_EVIDENCE_DEPLOY_KEY_ID; do
      if ! launchctl unsetenv "$name"; then
        echo "ROLLBACK WARNING: launchctl could not clear $name" >&2
        rollback_failed=1
      elif ! remaining="$(launchctl getenv "$name")"; then
        echo "ROLLBACK WARNING: launchctl could not verify removal of $name" >&2
        rollback_failed=1
      elif [[ -n "$remaining" ]]; then
        echo "ROLLBACK WARNING: launchctl value remains: $name" >&2
        rollback_failed=1
      fi
    done
    if ! remove_owned_host_dir "$HOST_KEY_DIR"; then
      echo "ROLLBACK WARNING: remove host-key directory manually: $HOST_KEY_DIR" >&2
      rollback_failed=1
    fi
    if [[ -n "$RETURNED_HOST_KEY_DIR" && "$RETURNED_HOST_KEY_DIR" != "$HOST_KEY_DIR" ]] \
        && ! remove_owned_host_dir "$RETURNED_HOST_KEY_DIR"; then
      echo "ROLLBACK WARNING: inspect unexpected host-key path: $RETURNED_HOST_KEY_DIR" >&2
      rollback_failed=1
    fi
    if [[ -n "$WORK" ]] && ! rm -rf -- "$WORK"; then
      echo "ROLLBACK WARNING: could not remove temporary directory $WORK" >&2
      rollback_failed=1
    fi

    [[ "$rollback_id" =~ ^[0-9]+$ ]] || rollback_id=''
    if [[ -n "$rollback_id" ]] && ! gh api --method DELETE \
        "repos/$GITHUB_REPOSITORY/keys/$rollback_id" >/dev/null; then
      echo "ROLLBACK NOTICE: DELETE for deploy key $rollback_id failed; reconciling by public key." >&2
    fi
    remaining="$(find_registered_deploy_key_id)"
    lookup_rc=$?
    if [[ "$lookup_rc" == 0 && -n "$remaining" ]]; then
      if ! gh api --method DELETE \
          "repos/$GITHUB_REPOSITORY/keys/$remaining" >/dev/null; then
        echo "ROLLBACK NOTICE: DELETE for reconciled deploy key $remaining failed." >&2
      fi
    fi
    remaining="$(find_registered_deploy_key_id)"
    lookup_rc=$?
    if [[ "$lookup_rc" != 0 ]]; then
      echo 'ROLLBACK WARNING: GitHub deploy-key list verification failed.' >&2
      echo "Inspect: https://github.com/$GITHUB_REPOSITORY/settings/keys" >&2
      rollback_failed=1
    elif [[ -n "$remaining" ]]; then
      echo "ROLLBACK WARNING: generated GitHub deploy key still exists: $remaining" >&2
      echo "Delete it: https://github.com/$GITHUB_REPOSITORY/settings/keys" >&2
      rollback_failed=1
    fi

    if [[ "$rollback_failed" == 1 ]]; then
      echo 'Automatic rollback was incomplete; follow the warnings above.' >&2
    else
      echo 'Partial session setup was rolled back and verified.' >&2
    fi
  elif [[ -n "$WORK" ]] && ! rm -rf -- "$WORK"; then
    echo "WARNING: could not remove temporary directory $WORK" >&2
  fi
  return "$rc"
}
trap cleanup EXIT

for command in launchctl gh jq ssh-keygen openssl; do
  command -v "$command" >/dev/null 2>&1 \
    || { echo "required command not found: $command" >&2; exit 1; }
done
for helper in prepare-console-password.sh prepare-deploy-key.sh prepare-host-key.sh; do
  [[ -r "$MODULE_DIR/$helper" ]] \
    || { echo "required helper not readable: $MODULE_DIR/$helper" >&2; exit 1; }
done
printf 'capability-check' | openssl passwd -6 -stdin >/dev/null 2>&1 \
  || { echo 'OpenSSL lacks required SHA-512 crypt support' >&2; exit 1; }
[[ "$GITHUB_REPOSITORY" =~ ^[^/]+/[^/]+$ ]] \
  || { echo 'repository must look like OWNER/REPO' >&2; exit 2; }
gh auth status >/dev/null 2>&1 \
  || { echo 'GitHub CLI is not authenticated; run: gh auth login' >&2; exit 1; }

protection="$(gh api "repos/$GITHUB_REPOSITORY/branches/main/protection")" \
  || { echo 'main branch protection is required before creating a write deploy key' >&2; exit 1; }
jq -e '
  .enforce_admins.enabled == true and
  .required_pull_request_reviews != null and
  .allow_force_pushes.enabled == false and
  .allow_deletions.enabled == false
' <<<"$protection" >/dev/null \
  || { echo 'main protection must require PRs, include admins, and forbid force-push/deletion' >&2; exit 1; }

if [[ "$CHECK_ONLY" == 1 ]]; then
  echo "Ready: tools, GitHub authentication, and $GITHUB_REPOSITORY main protection verified."
  SUCCESS=1
  exit 0
fi

for name in TF_VAR_tailscale_auth_key TF_VAR_console_password_hash \
  TF_VAR_evidence_deploy_key TF_VAR_ssh_host_private_key \
  SSH_HOST_KEY_FINGERPRINT SEMBLA_HOST_KEY_DIR \
  SEMBLA_EVIDENCE_DEPLOY_KEY_ID; do
  if ! existing_value="$(launchctl getenv "$name")"; then
    echo "could not inspect existing launchctl value: $name" >&2
    exit 1
  fi
  if [[ -n "$existing_value" ]]; then
    echo "existing session value detected: $name" >&2
    echo 'Clear/revoke the previous session before preparing a fresh one.' >&2
    exit 1
  fi
done
unset existing_value

[[ -r /dev/tty && -w /dev/tty ]] \
  || { echo 'an interactive terminal (/dev/tty) is required' >&2; exit 1; }

cat >&2 <<'EOF'
Create a fresh ephemeral, pre-authorized Tailscale auth key, then paste it below.
The input is hidden and is never printed.
EOF
printf 'Tailscale ephemeral auth key: ' >&2
IFS= read -r -s TAILSCALE_AUTH_KEY < /dev/tty
printf '\n' >&2
[[ -n "$TAILSCALE_AUTH_KEY" ]] \
  || { echo 'Tailscale auth key was empty' >&2; exit 1; }

WORK="$(mktemp -d)"
chmod 0700 "$WORK"

# Reuse the audited helpers; each emits only shell-quoted exports on stdout.
if ! console_exports="$(bash +x "$MODULE_DIR/prepare-console-password.sh")"; then
  echo 'Console-password preparation failed; nothing was registered or stored.' >&2
  exit 1
fi
eval "$console_exports"
unset console_exports
if ! deploy_exports="$(bash +x "$MODULE_DIR/prepare-deploy-key.sh")"; then
  echo 'Deploy-key preparation failed; nothing was registered or stored.' >&2
  exit 1
fi
eval "$deploy_exports"
unset deploy_exports

printf '%s\n' "$TF_VAR_evidence_deploy_key" > "$WORK/deploy-key"
chmod 0600 "$WORK/deploy-key"
DEPLOY_PUBLIC="$(ssh-keygen -y -f "$WORK/deploy-key")"
DEPLOY_TITLE="sembla-evidence-$(date -u +%Y%m%dT%H%M%SZ)"

printf 'Register a fresh write deploy key on %s? [Y/n] ' "$GITHUB_REPOSITORY" >&2
IFS= read -r answer < /dev/tty
case "${answer:-Y}" in
  Y|y|yes|YES) ;;
  *) echo 'Cancelled before registering or storing credentials.' >&2; exit 1 ;;
esac

MUTATED=1
if ! response="$(gh api --method POST "repos/$GITHUB_REPOSITORY/keys" \
    -f title="$DEPLOY_TITLE" -f key="$DEPLOY_PUBLIC" -F read_only=false)"; then
  echo 'GitHub deploy-key registration failed; attempting reconciliation.' >&2
  exit 1
fi
REGISTERED_DEPLOY_KEY_ID="$(jq -r '.id // empty' <<<"$response" 2>/dev/null || true)"
[[ "$REGISTERED_DEPLOY_KEY_ID" =~ ^[0-9]+$ ]] || REGISTERED_DEPLOY_KEY_ID=''
if [[ -z "$REGISTERED_DEPLOY_KEY_ID" ]]; then
  REGISTERED_DEPLOY_KEY_ID="$(find_registered_deploy_key_id || true)"
fi
[[ "$REGISTERED_DEPLOY_KEY_ID" =~ ^[0-9]+$ ]] \
  || { echo 'GitHub did not return or reconcile a numeric deploy-key ID' >&2; exit 1; }
returned_public="$(jq -r '.key // empty' <<<"$response" 2>/dev/null || true)"
[[ "$(awk '{print $1, $2}' <<<"$returned_public")" \
    == "$(awk '{print $1, $2}' <<<"$DEPLOY_PUBLIC")" ]] \
  || { echo 'GitHub response public key did not match the generated key' >&2; exit 1; }
jq -e '.read_only == false' <<<"$response" >/dev/null \
  || { echo 'GitHub registered the deploy key without write access' >&2; exit 1; }
unset returned_public

# Predetermine the helper path so a helper failure cannot orphan private key bytes.
HOST_KEY_DIR="$MODULE_DIR/.host-key-paid-$(date -u +%Y%m%dT%H%M%SZ)-$$"
if ! host_exports="$(HOST_KEY_DIR="$HOST_KEY_DIR" \
    bash +x "$MODULE_DIR/prepare-host-key.sh")"; then
  echo "Host-key preparation failed; rollback will remove $HOST_KEY_DIR" >&2
  exit 1
fi
eval "$host_exports"
unset host_exports
RETURNED_HOST_KEY_DIR="${SEMBLA_HOST_KEY_DIR:-}"
[[ "$RETURNED_HOST_KEY_DIR" == "$HOST_KEY_DIR" ]] \
  || { echo 'Host-key helper returned an unexpected directory' >&2; exit 1; }

set_launchctl_value() {
  local name="$1" expected="$2" actual=''
  launchctl setenv "$name" "$expected"
  if ! actual="$(launchctl getenv "$name")"; then
    echo "launchctl readback failed: $name" >&2
    return 1
  fi
  [[ "$actual" == "$expected" ]] \
    || { echo "launchctl verification failed: $name" >&2; return 1; }
}
set_launchctl_value TF_VAR_tailscale_auth_key "$TAILSCALE_AUTH_KEY"
set_launchctl_value TF_VAR_console_password_hash "$TF_VAR_console_password_hash"
set_launchctl_value TF_VAR_evidence_deploy_key "$TF_VAR_evidence_deploy_key"
set_launchctl_value TF_VAR_ssh_host_private_key "$TF_VAR_ssh_host_private_key"
set_launchctl_value SSH_HOST_KEY_FINGERPRINT "$SSH_HOST_KEY_FINGERPRINT"
set_launchctl_value SEMBLA_HOST_KEY_DIR "$SEMBLA_HOST_KEY_DIR"
set_launchctl_value SEMBLA_EVIDENCE_DEPLOY_KEY_ID "$REGISTERED_DEPLOY_KEY_ID"

registered_key="$(gh api "repos/$GITHUB_REPOSITORY/keys/$REGISTERED_DEPLOY_KEY_ID")" \
  || { echo 'could not verify the registered GitHub deploy key' >&2; exit 1; }
jq -e --arg public "$DEPLOY_PUBLIC" '
  (.id | type) == "number" and
  .read_only == false and
  ((.key | split(" ")[0:2] | join(" "))
    == ($public | split(" ")[0:2] | join(" ")))
' <<<"$registered_key" >/dev/null \
  || { echo 'registered GitHub deploy key verification failed' >&2; exit 1; }
unset registered_key

SUCCESS=1
cat <<EOF
Paid-session credentials are ready.
  repository:          $GITHUB_REPOSITORY
  deploy key ID:       $REGISTERED_DEPLOY_KEY_ID
  host fingerprint:    $SSH_HOST_KEY_FINGERPRINT
  host-key directory:  $SEMBLA_HOST_KEY_DIR

Secrets were stored in launchctl and were not printed.
After teardown, revoke deploy key $REGISTERED_DEPLOY_KEY_ID, clear the launchctl
session variables, and remove the host-key directory.
EOF
