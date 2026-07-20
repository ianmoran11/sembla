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

check_failure_exact() {
  local file="$1" expected="$2" output actual
  if output="$(lake env lean "$file" 2>&1)"; then
    echo "expected $file to fail" >&2
    exit 1
  fi
  actual="$(sed -nE '/^[^:]+:[0-9]+:[0-9]+: error: /p' <<<"$output")"
  if [[ "$actual" != "$expected" ]]; then
    echo "unexpected diagnostics for $file:" >&2
    printf 'expected:\n%s\nactual:\n%s\nfull output:\n%s\n' \
      "$expected" "$actual" "$output" >&2
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

check_failure_exact Negative/UnknownBareIdentifier.lean \
  "Negative/UnknownBareIdentifier.lean:10:12: error: unknown state or attribute 'missing'"
check_failure_exact Negative/AmbiguousBareIdentifier.lean \
  "Negative/AmbiguousBareIdentifier.lean:10:12: error: ambiguous identifier 'score': both an attribute and parameter are in scope"
check_failure_exact Negative/DuplicateDerivedParameterName.lean \
  "Negative/DuplicateDerivedParameterName.lean:7:10: error: duplicate parameter runtime name 'beta' for declarations 'β' and 'beta'"
check_failure_exact Negative/DuplicateDerivedTableName.lean \
  "Negative/DuplicateDerivedTableName.lean:9:13: error: duplicate table runtime name 'http_server' for declarations 'HTTPServer' and 'http_server'"
check_failure_exact Negative/DuplicateOverrideTableName.lean \
  "Negative/DuplicateOverrideTableName.lean:9:35: error: duplicate table runtime name 'person' for declarations 'Person' and 'LegacyPerson'"
check_failure_exact Negative/UnsupportedBindingCharacter.lean \
  "Negative/UnsupportedBindingCharacter.lean:5:16: error: identifier 'café' contains unsupported character 'é'"
check_failure_exact Negative/UnsupportedSystemCharacter.lean \
  "Negative/UnsupportedSystemCharacter.lean:7:20: error: identifier 'café' contains unsupported character 'é'"
check_failure_exact Negative/MalformedBindingSeparator.lean \
  "Negative/MalformedBindingSeparator.lean:5:16: error: identifier 'bad__name' has an unsupported separator pattern"
check_failure_exact Negative/PrimeBindingIdentifier.lean \
  "Negative/PrimeBindingIdentifier.lean:5:16: error: identifier 'beta'' contains unsupported character '''"
check_failure_exact Negative/EscapedPunctuationBinding.lean \
  "Negative/EscapedPunctuationBinding.lean:5:16: error: identifier 'bad-name' contains unsupported character '-'"
check_failure_exact Negative/EscapedWhitespaceBinding.lean \
  "Negative/EscapedWhitespaceBinding.lean:5:16: error: identifier 'bad name' contains unsupported character ' '"
check_failure_exact Negative/LeadingBindingSeparator.lean \
  "Negative/LeadingBindingSeparator.lean:5:16: error: identifier '_bad' has an unsupported separator pattern"
check_failure_exact Negative/TrailingBindingSeparator.lean \
  "Negative/TrailingBindingSeparator.lean:5:16: error: identifier 'bad_' has an unsupported separator pattern"
check_failure_exact Negative/LeadingDigitBinding.lean \
  "Negative/LeadingDigitBinding.lean:5:16: error: identifier '2beta' must begin with an ASCII letter or documented Greek letter"
check_failure_exact Negative/TildePriorOutOfRange.lean \
  "Negative/TildePriorOutOfRange.lean:5:41: error: decimal literal is outside the supported finite f64 range"
check_failure_exact Negative/TildePriorNonFinite.lean \
  "Negative/TildePriorNonFinite.lean:5:41: error: real declarations require a decimal or scientific literal"
check_failure_exact Negative/NonBooleanAndAlias.lean \
  "Negative/NonBooleanAndAlias.lean:10:12: error: left operand of ∧ must have type Bool"
check_failure_exact Negative/NonNumericLeAlias.lean \
  "Negative/NonNumericLeAlias.lean:10:21: error: ordered comparison operands must be numeric"
check_failure_exact Negative/IncompatibleMulAlias.lean \
  "Negative/IncompatibleMulAlias.lean:11:13: error: numeric operator requires numeric operands"
check_failure_exact Negative/UnknownEnumNeVariant.lean \
  "Negative/UnknownEnumNeVariant.lean:10:21: error: unknown variant 'X' for attribute 'health'"
check_failure_exact Negative/IncompatibleEnumScalarNe.lean \
  "Negative/IncompatibleEnumScalarNe.lean:10:21: error: comparison operands have incompatible types"

check_failure_exact Negative/UnknownArrowSourceVariant.lean \
  "Negative/UnknownArrowSourceVariant.lean:10:44: error: unknown source variant 'X' for state attribute 'health'"
check_failure_exact Negative/UnknownArrowDestinationVariant.lean \
  "Negative/UnknownArrowDestinationVariant.lean:10:53: error: unknown destination variant 'X' for state attribute 'health'"
check_failure_exact Negative/SplitArrowVariants.lean \
  "Negative/SplitArrowVariants.lean:12:45: error: source variant 'S' occurs in state columns first, but destination variant 'I' occurs in second; reaction endpoints must belong to the same state attribute"
check_failure_exact Negative/UnknownArrowSystem.lean \
  "Negative/UnknownArrowSystem.lean:10:27: error: unknown system 'Missing'"
check_failure_exact Negative/UnknownArrowStateAttribute.lean \
  "Negative/UnknownArrowStateAttribute.lean:10:36: error: unknown state or attribute 'missing'"
check_failure_exact Negative/NonEnumArrowStateAttribute.lean \
  "Negative/NonEnumArrowStateAttribute.lean:12:36: error: reaction state attribute 'score' must have type Enum"
check_failure_exact Negative/NoInferredArrowSystem.lean \
  "Negative/NoInferredArrowSystem.lean:12:17: error: no compatible system for reaction 'infect' among systems: Person, Controller; add 'on System'"
check_failure_exact Negative/MultipleInferredArrowSystems.lean \
  "Negative/MultipleInferredArrowSystems.lean:12:17: error: multiple compatible systems for reaction 'infect': Person, Animal; add 'on System'"
check_failure_exact Negative/MultipleArrowStateAttributes.lean \
  "Negative/MultipleArrowStateAttributes.lean:12:36: error: system 'Person' has multiple enum state attributes: health, mode; add 'attribute:'"
check_failure_exact Negative/NonRealArrowHazard.lean \
  "Negative/NonRealArrowHazard.lean:10:48: error: hazard has type Int; expected Real"
check_failure_exact Negative/ArrowExtraEffects.lean \
  "Negative/ArrowExtraEffects.lean:10:55: error: reaction arrows cannot declare additional guards or effects; use 'transition ... where'"
check_failure_exact Negative/ArrowExtraGuard.lean \
  "Negative/ArrowExtraGuard.lean:10:55: error: reaction arrows cannot declare additional guards or effects; use 'transition ... where'"

lake env lean Positive/ForwardRefPriorless.lean
lake env lean Positive/OutputFieldOrder.lean
lake env lean Positive/ObservationOrder.lean
lake env lean Positive/OptionBBindersNames.lean
lake env lean Sembla/ReactionArrowTests.lean
echo "Lean positioned negative and positive elaboration tests passed"
