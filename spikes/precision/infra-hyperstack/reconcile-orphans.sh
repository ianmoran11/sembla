#!/usr/bin/env bash
# Finds VMs this module created that Terraform state does not know about.
#
# Why this exists: `terraform apply` waits a provider-hardcoded 5 minutes for
# the VM to stabilise. Spot provisioning routinely exceeds it, and Terraform
# then errors *after* creating the VM. The result is a machine that is ACTIVE
# and billing but absent from state, where `terraform destroy` cannot see it
# and `destroy-deadline.sh` will not guard it. The provider exposes no
# `timeouts` block — only `profile` — so the wait cannot be lengthened. This
# script is the compensating control.
#
#   bash reconcile-orphans.sh              # report only (default, safe)
#   bash reconcile-orphans.sh --delete     # delete orphans, with confirmation
#   bash reconcile-orphans.sh --delete --yes   # non-interactive
#
# Only VMs whose name starts with `name_prefix` from the tfvars are ever
# considered. Anything else in the account is listed as untouched and is never
# a deletion candidate, so this cannot reach unrelated infrastructure.
set -Eeuo pipefail

MODULE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TFVARS_FILE="${TFVARS_FILE:-terraform.tfvars}"
cd "$MODULE_DIR"

DELETE=false
ASSUME_YES=false
for arg in "$@"; do
  case "$arg" in
    --delete) DELETE=true ;;
    --yes) ASSUME_YES=true ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

: "${HYPERSTACK_API_KEY:?export HYPERSTACK_API_KEY; the API is the only place an orphan is visible}"

NAME_PREFIX="$(python3 - "$TFVARS_FILE" <<'PY'
import re, sys, pathlib
text = pathlib.Path(sys.argv[1]).read_text()
match = re.search(r'^\s*name_prefix\s*=\s*"([^"]+)"', text, re.M)
print(match.group(1) if match else "sembla-precision")
PY
)"

# Terraform state is the authority on what is tracked. Read the file directly
# rather than shelling out: `terraform state list` can trigger a provider
# refresh, which is slow and can fail for exactly the resources in question.
STATE_IDS="$(python3 - <<'PY'
import json, pathlib
path = pathlib.Path("terraform.tfstate")
if not path.exists():
    raise SystemExit
try:
    state = json.loads(path.read_text())
except Exception:
    raise SystemExit
for resource in state.get("resources", []):
    if resource.get("type") != "hyperstack_core_virtual_machine":
        continue
    for instance in resource.get("instances", []):
        vm_id = instance.get("attributes", {}).get("id")
        if vm_id is not None:
            print(vm_id)
PY
)"

API_JSON="$(curl -sS --max-time 30 \
  https://infrahub-api.nexgencloud.com/v1/core/virtual-machines \
  -H "api_key: $HYPERSTACK_API_KEY" -H 'Accept: application/json')"

REPORT="$(printf '%s' "$API_JSON" | python3 - "$NAME_PREFIX" "$STATE_IDS" <<'PY'
import json, sys
prefix = sys.argv[1]
tracked = {line.strip() for line in sys.argv[2].splitlines() if line.strip()}
try:
    instances = json.loads(sys.stdin.read()).get("instances", [])
except Exception:
    raise SystemExit("could not parse the Hyperstack response; check the API key")

orphans, tracked_rows, untouched = [], [], []
for vm in instances:
    vm_id, name = str(vm.get("id")), vm.get("name") or ""
    row = f"{vm_id}\t{name}\t{vm.get('status')}\t{vm.get('floating_ip')}"
    if not name.startswith(prefix):
        untouched.append(row)
    elif vm_id in tracked:
        tracked_rows.append(row)
    else:
        orphans.append(row)

print("ORPHANS")
print("\n".join(orphans))
print("TRACKED")
print("\n".join(tracked_rows))
print("UNTOUCHED")
print("\n".join(untouched))
PY
)"

section() { printf '%s' "$REPORT" | awk -v s="$1" '$0==s{f=1;next} /^(ORPHANS|TRACKED|UNTOUCHED)$/{f=0} f && NF'; }

ORPHANS="$(section ORPHANS)"
TRACKED="$(section TRACKED)"
UNTOUCHED="$(section UNTOUCHED)"

echo "name_prefix: $NAME_PREFIX"
echo
printf 'tracked in Terraform state: %s\n' "$([[ -n "$TRACKED" ]] && echo "$(wc -l <<< "$TRACKED" | tr -d ' ')" || echo 0)"
[[ -n "$TRACKED" ]] && printf '  %s\n' "$TRACKED"
printf 'other VMs in the account (never touched): %s\n' \
  "$([[ -n "$UNTOUCHED" ]] && echo "$(wc -l <<< "$UNTOUCHED" | tr -d ' ')" || echo 0)"

if [[ -z "$ORPHANS" ]]; then
  echo
  echo "No orphans. Every ${NAME_PREFIX}* VM in the account is tracked in state."
  exit 0
fi

echo
echo "ORPHANED — billing, and invisible to terraform destroy and destroy-deadline.sh:"
printf '  %s\n' "$ORPHANS"

if [[ "$DELETE" != true ]]; then
  echo
  echo "Report only. Re-run with --delete to remove them, or adopt one with:"
  echo "  terraform import 'hyperstack_core_virtual_machine.gpu[0]' <ID>"
  echo "Adoption is the right choice only if you still want the run; otherwise delete."
  exit 1
fi

if [[ "$ASSUME_YES" != true ]]; then
  echo
  read -r -p "Delete the VMs listed above? [yes/N] " reply
  [[ "$reply" == "yes" ]] || { echo "aborted"; exit 1; }
fi

while IFS=$'\t' read -r vm_id name _status _ip; do
  [[ -z "$vm_id" ]] && continue
  echo "deleting $vm_id ($name)"
  curl -sS --max-time 30 -X DELETE \
    "https://infrahub-api.nexgencloud.com/v1/core/virtual-machines/$vm_id" \
    -H "api_key: $HYPERSTACK_API_KEY" -H 'Accept: application/json' >/dev/null
done <<< "$ORPHANS"

# Never trust the delete response; the account listing is the only proof.
sleep 5
REMAINING="$(curl -sS --max-time 30 \
  https://infrahub-api.nexgencloud.com/v1/core/virtual-machines \
  -H "api_key: $HYPERSTACK_API_KEY" -H 'Accept: application/json' \
  | python3 - "$NAME_PREFIX" <<'PY'
import json, sys
prefix = sys.argv[1]
instances = json.loads(sys.stdin.read()).get("instances", [])
names = [v.get("name") or "" for v in instances]
print(sum(1 for n in names if n.startswith(prefix)))
PY
)"

if [[ "$REMAINING" != "0" ]]; then
  echo "STILL PRESENT: $REMAINING ${NAME_PREFIX}* VM(s) remain. Delete them in the Hyperstack console NOW." >&2
  exit 1
fi
echo "Verified: no ${NAME_PREFIX}* VMs remain in the account."
