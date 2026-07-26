#!/usr/bin/env bash
# Detached billing watchdog: destroys the paid VM at a deadline regardless of
# what happens to the collector, the shell, the tmux session, or the network.
#
# Hyperstack bills ACTIVE and SHUTOFF alike, and the guest poweroff timer is not
# a billing control, so the only real stop is `terraform destroy`. If the driver
# dies — dropped phone connection, closed laptop, timeout, crash — nothing else
# runs it. This does.
#
#   bash destroy-deadline.sh arm [hours]     # default 6
#   bash destroy-deadline.sh status
#   bash destroy-deadline.sh disarm
#
# The API key is inherited through the environment and never written to disk.
# `at` is deliberately not used: macOS ships atrun disabled in launchd, so an
# `at` job would appear to be scheduled and then silently never fire.
set -Eeuo pipefail

MODULE_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TFVARS_FILE="${TFVARS_FILE:-terraform.tfvars}"
STATE_DIR="$MODULE_DIR/.destroy-deadline"
PID_FILE="$STATE_DIR/pid"
DEADLINE_FILE="$STATE_DIR/deadline"
LOG_FILE="$STATE_DIR/watchdog.log"

vm_in_state() {
  cd "$MODULE_DIR"
  terraform state list 2>/dev/null | grep -q 'hyperstack_core_virtual_machine'
}

watchdog_alive() {
  [[ -f "$PID_FILE" ]] && kill -0 "$(cat "$PID_FILE")" 2>/dev/null
}

case "${1:-}" in
  arm)
    hours="${2:-6}"
    if [[ ! "$hours" =~ ^[0-9]+([.][0-9]+)?$ ]]; then
      echo "hours must be numeric" >&2
      exit 2
    fi
    : "${HYPERSTACK_API_KEY:?export HYPERSTACK_API_KEY; the watchdog inherits it and cannot destroy without it}"
    if watchdog_alive; then
      echo "already armed as PID $(cat "$PID_FILE"), deadline $(cat "$DEADLINE_FILE")" >&2
      echo "disarm first if you want to change the deadline" >&2
      exit 2
    fi
    if ! vm_in_state; then
      echo "no VM in Terraform state; nothing to guard" >&2
      exit 2
    fi
    mkdir -p "$STATE_DIR"
    chmod 0700 "$STATE_DIR"
    seconds="$(python3 -c "print(int(float('$hours') * 3600))")"
    deadline="$(python3 -c "
import datetime
print((datetime.datetime.now() + datetime.timedelta(seconds=$seconds)).strftime('%Y-%m-%d %H:%M:%S'))")"

    deadline_epoch="$(python3 -c "import time; print(int(time.time()) + $seconds)")"

    if ! command -v caffeinate >/dev/null; then
      echo "warning: caffeinate not found; system sleep will delay this watchdog" >&2
    fi

    # nohup + & detaches from the terminal; on macOS there is no setsid. The
    # child is reparented when this shell exits and keeps running.
    nohup bash -c '
      # Hold a system-sleep assertion for as long as this watchdog lives.
      # Without it macOS suspends us when the machine sleeps, so a watchdog
      # armed at bedtime fires hours late and bills the whole night. `-w $$`
      # ties the assertion to this process, so `disarm` releases it too.
      if command -v caffeinate >/dev/null; then
        caffeinate -s -w $$ &
      fi
      # Poll an absolute deadline rather than sleeping for a duration: a relative
      # sleep does not advance while the host is suspended, so it would fire late
      # by however long the machine slept. This fires within the poll interval of
      # the wall-clock deadline even if the host suspends and resumes.
      while (( $(date +%s) < $1 )); do sleep 30; done
      cd "$2"
      {
        printf "[%s] deadline reached\n" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        if ! terraform state list 2>/dev/null | grep -q hyperstack_core_virtual_machine; then
          printf "[%s] no VM in state; the collector already destroyed it. Nothing to do.\n" \
            "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
          exit 0
        fi
        printf "[%s] VM still present -- destroying to stop billing\n" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        terraform destroy -var-file="$3" \
          -var=create_instance=true \
          -var=accept_paid_creation=true \
          -auto-approve
        if terraform state list 2>/dev/null | grep -q -E "hyperstack_core_virtual_machine|security_rule"; then
          printf "[%s] DESTROY INCOMPLETE -- delete the VM in the Hyperstack console NOW\n" \
            "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
          exit 1
        fi
        printf "[%s] destroy complete; state is clean\n" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
      } 2>&1
    ' _ "$deadline_epoch" "$MODULE_DIR" "$TFVARS_FILE" >> "$LOG_FILE" 2>&1 &

    echo $! > "$PID_FILE"
    printf '%s\n' "$deadline" > "$DEADLINE_FILE"
    chmod 0600 "$PID_FILE" "$DEADLINE_FILE"
    echo "Armed: PID $(cat "$PID_FILE") will destroy the VM at $deadline (local time)."
    echo "Log:   $LOG_FILE"
    echo "This survives the collector, the shell, and tmux. It does not survive a reboot."
    if command -v caffeinate >/dev/null; then
      echo "System sleep is held off while this watchdog is armed (caffeinate -s)."
    fi
    ;;

  status)
    if watchdog_alive; then
      echo "armed: PID $(cat "$PID_FILE"), fires at $(cat "$DEADLINE_FILE")"
      vm_in_state && echo "VM currently in state: yes" || echo "VM currently in state: no (watchdog will no-op)"
    else
      echo "not armed"
      [[ -f "$LOG_FILE" ]] && { echo "--- last log ---"; tail -5 "$LOG_FILE"; }
    fi
    ;;

  disarm)
    if watchdog_alive; then
      kill "$(cat "$PID_FILE")" 2>/dev/null || true
      echo "disarmed PID $(cat "$PID_FILE")"
    else
      echo "was not armed"
    fi
    rm -f "$PID_FILE" "$DEADLINE_FILE"
    ;;

  *)
    echo "usage: bash destroy-deadline.sh arm [hours] | status | disarm" >&2
    exit 2
    ;;
esac
