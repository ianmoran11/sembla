#!/usr/bin/env bash
# Generates the evidence-push deploy key and prints the export the plan needs.
#
# The remote payload pushes its evidence directory to an `evidence/*` branch so
# a long run delivers results without a live SSH session at the end. That needs
# a credential on the VM. This generates the narrowest one available:
#
#   * a *deploy key*, not a personal access token — it authorises exactly one
#     repository and nothing else: no API, no other repo, no account scope;
#   * ED25519, generated fresh per run, never written anywhere but this shell
#     and the VM's user-data;
#   * write access, because it must push. GitHub deploy keys cannot be scoped
#     to a single branch, so branch protection on `main` is what stops a
#     compromised VM from touching the trunk. Set it up — see the RUNBOOK.
#
# Usage:
#   eval "$(bash prepare-deploy-key.sh)"        # exports TF_VAR_evidence_deploy_key
#   bash prepare-deploy-key.sh --show-public    # print the public half only
#
# The public half must be registered once at
#   https://github.com/<owner>/<repo>/settings/keys/new
# with "Allow write access" ticked. Re-running this generates a *new* key, which
# means registering it again; keep the shell alive for the whole session.
set -Eeuo pipefail

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
chmod 0700 "$WORK"

ssh-keygen -t ed25519 -N '' -C "sembla-evidence-push-$(date -u +%Y%m%dT%H%M%SZ)" \
  -f "$WORK/evidence_deploy_key" >/dev/null

PUBLIC="$(cat "$WORK/evidence_deploy_key.pub")"

if [[ "${1:-}" == "--show-public" ]]; then
  printf '%s\n' "$PUBLIC"
  echo
  echo "Register this at https://github.com/<owner>/<repo>/settings/keys/new with 'Allow write access'." >&2
  echo "NOTE: --show-public discards the private half. Run without it to export the usable key." >&2
  exit 0
fi

# stderr carries the human instructions so that `eval "$(...)"` consumes only
# the export on stdout.
{
  echo
  echo "Register this public key at:"
  echo "  https://github.com/<owner>/<repo>/settings/keys/new"
  echo "Tick 'Allow write access'. Title it with today's date so stale keys are obvious."
  echo
  printf '%s\n' "$PUBLIC"
  echo
  echo "Then confirm branch protection on main is enabled (RUNBOOK: 'Evidence push')."
  echo "Delete the deploy key from GitHub once the session's evidence is merged."
  echo
} >&2

printf 'export TF_VAR_evidence_deploy_key=%q\n' "$(cat "$WORK/evidence_deploy_key")"
