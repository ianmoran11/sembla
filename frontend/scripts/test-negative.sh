#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

check_failure() {
  local file="$1" expected="$2" output
  if output="$(lake env lean "$file" 2>&1)"; then
    echo "expected $file to fail" >&2
    exit 1
  fi
  if ! grep -Fx "$expected" <<<"$output" >/dev/null; then
    echo "unexpected diagnostic for $file:" >&2
    printf '%s\n' "$output" >&2
    exit 1
  fi
}

check_failure Negative/UnknownAttribute.lean \
  "Negative/UnknownAttribute.lean:10:12: error: unknown state or attribute 'workplace'"
check_failure Negative/WrongGuardType.lean \
  "Negative/WrongGuardType.lean:11:12: error: guard has type Real; expected Bool"
check_failure Negative/UnknownRefTarget.lean \
  "Negative/UnknownRefTarget.lean:8:48: error: unknown reference target 'Workplace'"
check_failure Negative/UndeclaredParam.lean \
  "Negative/UndeclaredParam.lean:11:23: error: undeclared parameter 'delta'"
check_failure Negative/UnknownInput.lean \
  "Negative/UnknownInput.lean:11:22: error: unknown input port 'missing'"
check_failure Negative/UnknownEffect.lean \
  "Negative/UnknownEffect.lean:12:11: error: unknown state or attribute 'workplace'"
check_failure Negative/UnknownSystem.lean \
  "Negative/UnknownSystem.lean:9:35: error: unknown system 'Workplace'"
check_failure Negative/UnknownOutputAttribute.lean \
  "Negative/UnknownOutputAttribute.lean:11:36: error: unknown state or attribute 'workplace'"
check_failure Negative/UnknownWirePort.lean \
  "Negative/UnknownWirePort.lean:9:21: error: unknown output port 'source.missing'"
check_failure Negative/IntegerHazard.lean \
  "Negative/IntegerHazard.lean:11:13: error: hazard has type Int; expected Real"
check_failure Negative/RealEffectFromInt.lean \
  "Negative/RealEffectFromInt.lean:12:20: error: effect value has incompatible type"
check_failure Negative/DuplicateEnumVariant.lean \
  "Negative/DuplicateEnumVariant.lean:7:76: error: duplicate enum variant 'S'"
check_failure Negative/EmptyEnum.lean \
  "Negative/EmptyEnum.lean:7:60: error: enum attribute 'health' must declare at least one variant"
check_failure Negative/OrderedEnum.lean \
  "Negative/OrderedEnum.lean:10:21: error: ordered comparison operands must be numeric"
check_failure Negative/RefEffectWithoutClaim.lean \
  "Negative/RefEffectWithoutClaim.lean:14:11: error: writes to Ref attributes require resource claims, which are not supported by this DSL"
check_failure Negative/ZeroStep.lean \
  "Negative/ZeroStep.lean:4:42: error: tick width must be greater than zero"
check_failure Negative/OutOfRangeReal.lean \
  "Negative/OutOfRangeReal.lean:4:44: error: decimal literal is outside the supported finite f64 range"
check_failure Negative/OversizedRows.lean \
  "Negative/OversizedRows.lean:7:44: error: row count exceeds the IR u64 range"
check_failure Negative/UnknownViewTable.lean \
  "Negative/UnknownViewTable.lean:13:33: error: view 'bad_table' refers to unknown table 'Missing'"
check_failure Negative/UnknownViewAttribute.lean \
  "Negative/UnknownViewAttribute.lean:14:50: error: view 'bad_attribute': unknown state or attribute 'missing'"
check_failure Negative/NonBooleanViewFilter.lean \
  "Negative/NonBooleanViewFilter.lean:13:47: error: view 'bad_filter' filter has type Int; expected Bool"
check_failure Negative/CountViewWithValue.lean \
  "Negative/CountViewWithValue.lean:13:18: error: view 'bad_count' with reduce count cannot declare a value expression"
check_failure Negative/UnknownSummaryView.lean \
  "Negative/UnknownSummaryView.lean:15:54: error: summary 'bad_summary' refers to undeclared view 'population.absent'"

lake env lean Positive/ForwardRefPriorless.lean
lake env lean Positive/OutputFieldOrder.lean
lake env lean Positive/ObservationOrder.lean
echo "Lean positioned negative and positive elaboration tests passed"
