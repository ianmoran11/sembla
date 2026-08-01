#!/usr/bin/env bash
set -euo pipefail

frontend_root="$(cd "$(dirname "$0")/.." && pwd)"
temporary_root="$(mktemp -d "${TMPDIR:-/tmp}/sembla-proof-audit.XXXXXX")"
escape_source=""
escape_module_leaf=""
audit_lock="$frontend_root/.lake/sembla-proof-audit.lock"
audit_lock_owned=""

# The namespace-escape self-test briefly adds a synthetic module beneath
# `Sembla/`. Concurrent audits would discover or remove each other's module, so
# reject overlapping runs before either can report a misleading proof failure.
mkdir -p "$frontend_root/.lake"
if ! mkdir "$audit_lock" 2>/dev/null; then
  echo "another Lean proof audit is already using this worktree: $audit_lock" >&2
  echo "if no audit is running, remove the stale lock directory and retry" >&2
  rm -rf "$temporary_root"
  exit 2
fi
audit_lock_owned=1
printf '%s\n' "$$" > "$audit_lock/pid"

cleanup_escape_artifacts() {
  if [[ -n "$escape_source" ]]; then
    rm -f "$escape_source"
  fi
  if [[ -n "$escape_module_leaf" && -d "$frontend_root/.lake/build" ]]; then
    find "$frontend_root/.lake/build" -type f \
      -name "${escape_module_leaf}.*" -delete
  fi
}

cleanup() {
  cleanup_escape_artifacts
  rm -rf "$temporary_root"
  if [[ -n "$audit_lock_owned" ]]; then
    rm -rf "$audit_lock"
  fi
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

existing_proof_sources=(
  "$frontend_root"/Sembla/Lumping*.lean
  "$frontend_root"/Sembla/Composition/Spec*.lean
)

covered_sources=(
  "$frontend_root/Sembla/Semantics.lean"
  "$frontend_root/Sembla/Frontend/Builders.lean"
)
while IFS= read -r source; do
  covered_sources+=("$source")
done < <(
  for directory in \
    "$frontend_root/Sembla/Semantics" \
    "$frontend_root/Sembla/Frontend/Builders"; do
    if [[ -d "$directory" ]]; then
      find "$directory" -type f -name '*.lean' -print
    fi
  done | LC_ALL=C sort
)

scan_forbidden_source() {
  python3 -B - "$@" <<'PY'
import re
import sys
from pathlib import Path


def lean_code_only(text: str) -> str:
    """Remove nested comments, line comments, and strings while preserving lines."""
    output = []
    index = 0
    block_depth = 0
    in_string = False
    while index < len(text):
        pair = text[index:index + 2]
        char = text[index]
        if block_depth:
            if pair == "/-":
                block_depth += 1
                output.extend("  ")
                index += 2
            elif pair == "-/":
                block_depth -= 1
                output.extend("  ")
                index += 2
            else:
                output.append("\n" if char == "\n" else " ")
                index += 1
        elif in_string:
            if char == "\\" and index + 1 < len(text):
                output.extend("  ")
                index += 2
            elif char == '"':
                in_string = False
                output.append(" ")
                index += 1
            else:
                output.append("\n" if char == "\n" else " ")
                index += 1
        elif pair == "--":
            while index < len(text) and text[index] != "\n":
                output.append(" ")
                index += 1
        elif pair == "/-":
            block_depth = 1
            output.extend("  ")
            index += 2
        elif char == '"':
            in_string = True
            output.append(" ")
            index += 1
        else:
            output.append(char)
            index += 1
    return "".join(output)

pattern = re.compile(
    r"\b(?:sorry|admit|axiom|native_decide|unsafe|implemented_by)\b"
)
failed = False
for argument in sys.argv[1:]:
    path = Path(argument)
    code = lean_code_only(path.read_text())
    for match in pattern.finditer(code):
        line = code.count("\n", 0, match.start()) + 1
        print(f"{path}:{line}: forbidden Lean proof construct: {match.group(0)}", file=sys.stderr)
        failed = True
sys.exit(1 if failed else 0)
PY
}

scan_forbidden_source "${existing_proof_sources[@]}" "${covered_sources[@]}"

required_theorems=(
  lookupTotal_bump
  lookupTotal_groupTotals
  groupedCount_eq_naiveCount
  plan_rewrite_congr
)
for theorem in "${required_theorems[@]}"; do
  if ! grep -Eq "^[[:space:]]*theorem[[:space:]]+${theorem}([[:space:](]|$)" \
      "${existing_proof_sources[@]}"; then
    echo "required Lean theorem is missing or renamed: $theorem" >&2
    exit 1
  fi
done

required_spec_declarations=(
  linkV1_produces_valid_plan
  preservationStatement
  staticPreservationStatement
  denoteSourceStatic
  denotePlanStatic
)
for declaration in "${required_spec_declarations[@]}"; do
  if ! grep -Eq "^[[:space:]]*(def|theorem)[[:space:]]+${declaration}([[:space:](]|$)" \
      "${existing_proof_sources[@]}"; then
    echo "required Lean specification declaration is missing or renamed: $declaration" >&2
    exit 1
  fi
done

discover_audit_modules() {
  {
    printf '%s\n' "$frontend_root/Sembla.lean"
    find "$frontend_root/Sembla" -type f -name '*.lean' -print
  } | LC_ALL=C sort -u | while IFS= read -r source; do
    relative="${source#"$frontend_root"/}"
    module="${relative%.lean}"
    printf '%s\n' "${module//\//.}"
  done
}

run_namespace_audit() {
  local audit_root="$1"
  local audit_modules=()
  while IFS= read -r module; do
    audit_modules+=("$module")
  done < <(discover_audit_modules)

  {
    printf 'import Sembla.Semantics.ProofAudit\n'
    for module in "${audit_modules[@]}"; do
      if [[ "$module" != "Sembla.Semantics.ProofAudit" ]]; then
        printf 'import %s\n' "$module"
      fi
    done
    printf '\n#audit_proofs Sembla.Semantics Sembla.Frontend.Builders\n'
  } > "$audit_root"

  (
    cd "$frontend_root"
    lake build "${audit_modules[@]}"
    lake env lean "$audit_root"
  )
}

audit_root="$temporary_root/AuditRoot.lean"
run_namespace_audit "$audit_root"

forbidden_fixture="$temporary_root/ForbiddenDeclarations.lean"
cat > "$forbidden_fixture" <<'LEAN'
theorem temporarySorry : True := by sorry
theorem temporaryAdmit : True := by admit
axiom temporaryAxiom : True
theorem temporaryNative : True := by native_decide
unsafe def temporaryUnsafe : Nat := 0
@[implemented_by temporaryReplacement]
def temporaryImplementation : Nat := 0
def temporaryReplacement : Nat := 0
LEAN
forbidden_log="$temporary_root/forbidden.log"
if scan_forbidden_source "$forbidden_fixture" >"$forbidden_log" 2>&1; then
  echo "proof-audit negative self-test unexpectedly accepted forbidden source forms" >&2
  exit 1
fi
for token in sorry admit axiom native_decide unsafe implemented_by; do
  if ! grep -F "forbidden Lean proof construct: $token" "$forbidden_log" >/dev/null; then
    echo "proof-audit negative self-test did not reject '$token'" >&2
    exit 1
  fi
done
echo "proof-audit negative self-test passed: forbidden source forms rejected"

axiom_fixture="$temporary_root/UnapprovedAxiom.lean"
cat > "$axiom_fixture" <<'LEAN'
import Sembla.Semantics.ProofAudit

namespace Sembla.ProofAuditNegative

axiom unapproved : True
theorem dependsOnUnapproved : True := unapproved
opaque hiddenProposition : True := True.intro

end Sembla.ProofAuditNegative

#audit_proofs Sembla.ProofAuditNegative
LEAN
axiom_log="$temporary_root/unapproved-axiom.log"
if (
  cd "$frontend_root"
  lake env lean "$axiom_fixture"
) >"$axiom_log" 2>&1; then
  echo "proof-audit negative self-test unexpectedly accepted an unapproved axiom" >&2
  exit 1
fi
if ! grep -F \
    "theorem Sembla.ProofAuditNegative.dependsOnUnapproved depends on unapproved axioms: Sembla.ProofAuditNegative.unapproved" \
    "$axiom_log" >/dev/null; then
  echo "proof-audit negative self-test did not report the transitive unapproved axiom" >&2
  exit 1
fi
if ! grep -F \
    "opaque proposition in covered module: Sembla.ProofAuditNegative.hiddenProposition" \
    "$axiom_log" >/dev/null; then
  echo "proof-audit negative self-test did not reject the opaque proposition" >&2
  exit 1
fi
echo "proof-audit negative self-test passed: unapproved axiom and opaque proposition rejected"

escape_module_leaf="ProofAuditEscapeNegative$$"
escape_axiom_name="proofAuditEscapeUnapproved$$"
escape_theorem_name="proofAuditEscapeTheorem$$"
escape_source="$frontend_root/Sembla/${escape_module_leaf}.lean"
cat > "$escape_source" <<LEAN
namespace Sembla.Semantics

axiom ${escape_axiom_name} : True
theorem ${escape_theorem_name} : True := ${escape_axiom_name}

end Sembla.Semantics
LEAN
location_audit_root="$temporary_root/LocationAuditRoot.lean"
location_log="$temporary_root/location-bypass.log"
if run_namespace_audit "$location_audit_root" >"$location_log" 2>&1; then
  echo "proof-audit negative self-test unexpectedly omitted a covered-namespace module" >&2
  exit 1
fi
if ! grep -F \
    "theorem Sembla.Semantics.${escape_theorem_name} depends on unapproved axioms: Sembla.Semantics.${escape_axiom_name}" \
    "$location_log" >/dev/null; then
  echo "proof-audit negative self-test did not report the escaped covered-namespace theorem" >&2
  exit 1
fi
rm -f "$escape_source"
cleanup_escape_artifacts
if [[ -e "$escape_source" ]]; then
  echo "proof-audit negative self-test did not clean up its escaped source module" >&2
  exit 1
fi
echo "proof-audit negative self-test passed: covered namespace cannot escape by module location"

echo "Lean proof hygiene and deterministic axiom inventory checks passed"
