#!/usr/bin/env bash
# Focused, idempotent zero-resource teardown used by the H100 A/B/C collector.
# Source this file and call:
#   cuda_final_state_teardown ARTIFACT_DIR MODULE_DIR TFVARS_FILE

cuda_final_state_teardown() {
  local artifact_dir="$1"
  local module_dir="$2"
  local tfvars_file="$3"
  local marker="$artifact_dir/.cuda-final-state-teardown-started"
  local completed_marker="$artifact_dir/.cuda-final-state-teardown-complete"
  local attempts_file="$artifact_dir/teardown-attempt-count.txt"
  local status_file="$artifact_dir/teardown-status.txt"
  local terraform_bin="${FOCUSED_TERRAFORM_BIN:-terraform}"
  local reconcile_bin="${FOCUSED_RECONCILE_BIN:-$module_dir/reconcile-orphans.sh}"
  local timeout_bin="${FOCUSED_TIMEOUT_BIN:-timeout}"
  local timeout_seconds="${FOCUSED_TEARDOWN_TIMEOUT_SECONDS:-900}"
  local teardown_status=0
  local destroy_status=0
  local state_status=0
  local delete_status=0
  local report_status=0

  mkdir -p "$artifact_dir"
  if [[ -e "$completed_marker" && -s "$status_file" ]] \
      && [[ "$(cat "$status_file")" == "0" ]]; then
    return 0
  fi
  local attempts=0
  [[ -s "$attempts_file" ]] && attempts="$(cat "$attempts_file")"
  [[ "$attempts" =~ ^[0-9]+$ ]] || attempts=0
  attempts=$((attempts + 1))
  printf '%s\n' "$attempts" > "$attempts_file"
  printf 'attempt %s started %s\n' "$attempts" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    > "$marker"
  rm -f "$completed_marker"

  # Nsight Systems does not require changing RmProfilingAdminOnly.  The remote
  # focused driver verifies it is 1 before and after profiling, so local
  # teardown records that no counter relaxation needs restoration.
  printf '%s\n' 'not-modified; remote evidence must show RmProfilingAdminOnly: 1 before/after' \
    > "$artifact_dir/counter-restoration-status.txt"

  set +e
  (
    cd "$module_dir" || exit 1
    "$timeout_bin" --signal=TERM --kill-after=30s "${timeout_seconds}s" \
      "$terraform_bin" destroy -var-file="$tfvars_file" \
        -var=create_instance=true -var=accept_paid_creation=true -auto-approve
  ) > "$artifact_dir/terraform-destroy.log" 2>&1
  destroy_status=$?
  printf '%s\n' "$destroy_status" > "$artifact_dir/terraform-destroy-status.txt"
  (( destroy_status == 0 )) || teardown_status=1

  (
    cd "$module_dir" || exit 1
    "$terraform_bin" state list
  ) > "$artifact_dir/terraform-state-after-destroy.txt" 2> "$artifact_dir/terraform-state-after-destroy.stderr"
  state_status=$?
  (( state_status == 0 )) || teardown_status=1

  if (( destroy_status != 0 || state_status != 0 )) \
      || grep -qE 'hyperstack_core_virtual_machine|hyperstack_core_security_rule|security_rule' \
        "$artifact_dir/terraform-state-after-destroy.txt"; then
    (
      cd "$module_dir" || exit 1
      TFVARS_FILE="$tfvars_file" \
        "$timeout_bin" --signal=TERM --kill-after=30s "${timeout_seconds}s" \
        "$reconcile_bin" --delete --yes
    ) > "$artifact_dir/reconcile-delete.log" 2>&1
    delete_status=$?
    printf '%s\n' "$delete_status" > "$artifact_dir/reconcile-delete-status.txt"
    (( delete_status == 0 )) || teardown_status=1
  else
    printf '%s\n' 'not-required' > "$artifact_dir/reconcile-delete.log"
    printf '%s\n' '0' > "$artifact_dir/reconcile-delete-status.txt"
  fi

  # Report-only reconciliation is mandatory even when destroy/delete appeared
  # successful.  It is the provider-account proof used with Terraform state.
  (
    cd "$module_dir" || exit 1
    TFVARS_FILE="$tfvars_file" \
      "$timeout_bin" --signal=TERM --kill-after=30s "${timeout_seconds}s" \
      "$reconcile_bin"
  ) > "$artifact_dir/reconcile-final.log" 2>&1
  report_status=$?
  printf '%s\n' "$report_status" > "$artifact_dir/reconcile-final-status.txt"
  (( report_status == 0 )) || teardown_status=1

  (
    cd "$module_dir" || exit 1
    "$terraform_bin" state list
  ) > "$artifact_dir/terraform-state-final.txt" 2> "$artifact_dir/terraform-state-final.stderr"
  state_status=$?
  (( state_status == 0 )) || teardown_status=1
  if grep -qE 'hyperstack_core_virtual_machine|hyperstack_core_security_rule|security_rule' \
      "$artifact_dir/terraform-state-final.txt"; then
    teardown_status=1
  fi
  grep -Fq 'tracked in Terraform state: 0' "$artifact_dir/reconcile-final.log" \
    || teardown_status=1
  grep -Eq 'No orphans|Verified: no .* VMs remain' "$artifact_dir/reconcile-final.log" \
    || teardown_status=1

  printf '%s\n' "$teardown_status" > "$status_file"
  if (( teardown_status == 0 )); then
    printf '%s\n' 'zero paid VM/security-rule resources verified' \
      > "$artifact_dir/zero-resource-result.txt"
    printf 'attempt %s completed %s\n' "$attempts" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
      > "$completed_marker"
  else
    printf '%s\n' 'zero-resource verification FAILED' \
      > "$artifact_dir/zero-resource-result.txt"
  fi
  set -e
  return "$teardown_status"
}
