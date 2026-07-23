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

check_failure_exact Negative/UnknownFrequencyKey.lean \
  "Negative/UnknownFrequencyKey.lean:10:71: error: unknown frequency key attribute 'missing' on system 'Person'"
check_failure_exact Negative/RealFrequencyKey.lean \
  "Negative/RealFrequencyKey.lean:10:71: error: frequency key attribute 'risk' on system 'Person' must have type Ref; found Real"
check_failure_exact Negative/IntFrequencyKey.lean \
  "Negative/IntFrequencyKey.lean:10:71: error: frequency key attribute 'visits' on system 'Person' must have type Ref; found Int"
check_failure_exact Negative/EnumFrequencyKey.lean \
  "Negative/EnumFrequencyKey.lean:10:71: error: frequency key attribute 'health' on system 'Person' must have type Ref; found Enum"
check_failure_exact Negative/NonBooleanFrequencyPredicate.lean \
  "Negative/NonBooleanFrequencyPredicate.lean:12:54: error: frequency predicate has type Real; expected Bool"
check_failure_exact Negative/UnknownFrequencyRowAttribute.lean \
  "Negative/UnknownFrequencyRowAttribute.lean:12:54: error: unknown row attribute or model parameter 'missing' in frequency predicate; frequency predicates are row-local; aggregates join on declared Ref keys only"
check_failure_exact Negative/InputAggregateFrequencyPredicate.lean \
  "Negative/InputAggregateFrequencyPredicate.lean:12:54: error: frequency predicates are row-local; aggregates join on declared Ref keys only"
check_failure_exact Negative/RelationalAggregateFrequencyPredicate.lean \
  "Negative/RelationalAggregateFrequencyPredicate.lean:12:54: error: frequency predicates are row-local; aggregates join on declared Ref keys only"
check_failure_exact Negative/SizeAggregateFrequencyPredicate.lean \
  "Negative/SizeAggregateFrequencyPredicate.lean:12:54: error: frequency predicates are row-local; aggregates join on declared Ref keys only"
check_failure_exact Negative/NestedFrequencyPredicate.lean \
  "Negative/NestedFrequencyPredicate.lean:12:54: error: frequency predicates are row-local; aggregates join on declared Ref keys only"
check_failure_exact Negative/FrequencyMissingKey.lean \
  "Negative/FrequencyMissingKey.lean:10:48: error: frequency syntax requires a key: use 'freq (<predicate>) over <ref>'"
check_failure_exact Negative/FrequencyMissingParentheses.lean \
  "Negative/FrequencyMissingParentheses.lean:12:48: error: frequency syntax requires parentheses around the predicate: use 'freq (<predicate>) over <ref>'"

check_failure_exact Negative/CommandArrowAmbiguity.lean \
  "Negative/CommandArrowAmbiguity.lean:9:4: error: multiple compatible systems for reaction 't': A, B; add 'on System'"

check_failure_exact Negative/CommandArrowNoCompatibleSystem.lean \
  "Negative/CommandArrowNoCompatibleSystem.lean:7:4: error: no compatible system for reaction 't' among systems: A; add 'on System'"

check_failure_exact Negative/CommandArrowMultipleStateAttributes.lean \
  "Negative/CommandArrowMultipleStateAttributes.lean:8:13: error: system 'A' has multiple enum state attributes: first, second; add 'attribute:'"

check_failure_exact Negative/CommandCountOutputWithValue.lean \
  "Negative/CommandCountOutputWithValue.lean:8:6: error: count output field 'n' cannot declare a value expression"

check_failure_exact Negative/CommandCountViewWithValue.lean \
  "Negative/CommandCountViewWithValue.lean:7:9: error: view 'v' with reduce count cannot declare a value expression"

check_failure_exact Negative/CommandDuplicateAttribute.lean \
  "Negative/CommandDuplicateAttribute.lean:7:6: error: duplicate attribute 'value'"

check_failure_exact Negative/CommandDuplicateBox.lean \
  "Negative/CommandDuplicateBox.lean:6:6: error: duplicate box 'b'"

check_failure_exact Negative/CommandDuplicateGuard.lean \
  "Negative/CommandDuplicateGuard.lean:9:6: error: general transition 't' has duplicate guard"

check_failure_exact Negative/CommandDuplicateHazard.lean \
  "Negative/CommandDuplicateHazard.lean:10:6: error: general transition 't' has duplicate hazard"

check_failure_exact Negative/CommandDuplicateInputPort.lean \
  "Negative/CommandDuplicateInputPort.lean:8:10: error: duplicate input port 'p'"

check_failure_exact Negative/CommandDuplicateModel.lean \
  "Negative/CommandDuplicateModel.lean:5:0: error: 'Same' has already been declared"

check_failure_exact Negative/CommandDuplicateOutputPort.lean \
  "Negative/CommandDuplicateOutputPort.lean:9:11: error: duplicate output port 'p'"

check_failure_exact Negative/CommandDuplicateParameter.lean \
  "Negative/CommandDuplicateParameter.lean:5:8: error: duplicate parameter 'a'"

check_failure_exact Negative/CommandDuplicateSummary.lean \
  "Negative/CommandDuplicateSummary.lean:8:10: error: duplicate summary 's'"

check_failure_exact Negative/CommandDuplicateSystem.lean \
  "Negative/CommandDuplicateSystem.lean:6:11: error: duplicate system 'A'"

check_failure_exact Negative/CommandDuplicateTable.lean \
  "Negative/CommandDuplicateTable.lean:6:22: error: duplicate table runtime name 'same' for declarations 'A' and 'B'"

check_failure_exact Negative/CommandDuplicateTransition.lean \
  "Negative/CommandDuplicateTransition.lean:8:4: error: duplicate transition 'one'"

check_failure_exact Negative/CommandDuplicateView.lean \
  "Negative/CommandDuplicateView.lean:7:9: error: duplicate view 'v'"

check_failure_exact Negative/CommandDuplicateWireTarget.lean \
  "Negative/CommandDuplicateWireTarget.lean:14:16: error: duplicate wire target 'b.p'"

check_failure_exact Negative/CommandMisplacedSystem.lean \
  "Negative/CommandMisplacedSystem.lean:3:7: error: system declaration must be indented inside a sembla_model box"

check_failure_exact Negative/CommandMissingDt.lean \
  "Negative/CommandMissingDt.lean:3:13: error: sembla_model requires '(dt := <positive decimal>)'"

check_failure_exact Negative/CommandMissingGuard.lean \
  "Negative/CommandMissingGuard.lean:7:15: error: general transition 't' requires exactly one guard"

check_failure_exact Negative/CommandMissingHazard.lean \
  "Negative/CommandMissingHazard.lean:7:15: error: general transition 't' requires exactly one hazard"

check_failure_exact Negative/CommandNoEffects.lean \
  "Negative/CommandNoEffects.lean:7:15: error: general transition 't' requires at least one set effect"

check_failure_exact Negative/CommandNonCountWithoutUsing.lean \
  "Negative/CommandNonCountWithoutUsing.lean:7:9: error: view 'v' with reduce sum must declare a value expression"

check_failure_exact Negative/CommandOutOfRangeDt.lean \
  "Negative/CommandOutOfRangeDt.lean:3:27: error: decimal literal is outside the supported finite f64 range"

check_failure_exact Negative/CommandUnknownOutputField.lean \
  "Negative/CommandUnknownOutputField.lean:8:24: error: unknown state or attribute 'missing'"

check_failure_exact Negative/CommandUnknownOutputSource.lean \
  "Negative/CommandUnknownOutputSource.lean:6:18: error: unknown system 'Missing'"

check_failure_exact Negative/CommandUnknownRef.lean \
  "Negative/CommandUnknownRef.lean:6:15: error: unknown reference target 'Missing'"

check_failure_exact Negative/CommandUnknownSummaryView.lean \
  "Negative/CommandUnknownSummaryView.lean:7:21: error: summary 's' refers to undeclared view 'b.missing'"

check_failure_exact Negative/CommandUnknownTransitionAttribute.lean \
  "Negative/CommandUnknownTransitionAttribute.lean:8:12: error: unknown state or attribute 'missing'"

check_failure_exact Negative/CommandUnknownTransitionInput.lean \
  "Negative/CommandUnknownTransitionInput.lean:9:22: error: unknown input port 'missing'"

check_failure_exact Negative/CommandUnknownTransitionParameter.lean \
  "Negative/CommandUnknownTransitionParameter.lean:9:23: error: undeclared parameter 'missing'"

check_failure_exact Negative/CommandUnknownTransitionSystem.lean \
  "Negative/CommandUnknownTransitionSystem.lean:7:20: error: unknown system 'Missing'"

check_failure_exact Negative/CommandUnknownViewSource.lean \
  "Negative/CommandUnknownViewSource.lean:6:20: error: view 'v' refers to unknown table 'Missing'"

check_failure_exact Negative/CommandUnknownViewValue.lean \
  "Negative/CommandUnknownViewValue.lean:7:26: error: view 'v': unknown state or attribute 'missing'"

check_failure_exact Negative/CommandUnknownWireEndpoint.lean \
  "Negative/CommandUnknownWireEndpoint.lean:10:9: error: unknown output port 'a.missing'"

check_failure_exact Negative/CommandUnsupportedDeclaration.lean \
  "Negative/CommandUnsupportedDeclaration.lean:5:12: error: unsupported Sembla box declaration 'race'"

check_failure_exact Negative/CommandWireSchemaMismatch.lean \
  "Negative/CommandWireSchemaMismatch.lean:13:16: error: wire schema mismatch for 'a.p' -> 'b.p'"

check_failure_exact Negative/CommandWrongEffectType.lean \
  "Negative/CommandWrongEffectType.lean:10:15: error: effect value has incompatible type"

check_failure_exact Negative/CommandWrongGuardType.lean \
  "Negative/CommandWrongGuardType.lean:8:12: error: guard has type Int; expected Bool"

check_failure_exact Negative/CommandWrongHazardType.lean \
  "Negative/CommandWrongHazardType.lean:9:13: error: hazard has type Int; expected Real"

check_failure_exact Negative/CommandWrongOutputFilterType.lean \
  "Negative/CommandWrongOutputFilterType.lean:8:29: error: output filter must have type Bool"

check_failure_exact Negative/CommandWrongOutputType.lean \
  "Negative/CommandWrongOutputType.lean:8:6: error: count output field 'n' must have type Int"

check_failure_exact Negative/CommandWrongViewFilterType.lean \
  "Negative/CommandWrongViewFilterType.lean:7:28: error: view 'v' filter has type Int; expected Bool"

check_failure_exact Negative/CommandWrongViewType.lean \
  "Negative/CommandWrongViewType.lean:7:26: error: view 'v' value has type Enum; expected Real or Int"

check_failure_exact Negative/CommandZeroDt.lean \
  "Negative/CommandZeroDt.lean:3:27: error: tick width must be greater than zero"

check_failure_exact Negative/CommandDuplicateEnumVariant.lean \
  "Negative/CommandDuplicateEnumVariant.lean:6:17: error: duplicate enum variant 'X'"

check_failure_exact Negative/CommandEmptyEnum.lean \
  "Negative/CommandEmptyEnum.lean:6:6: error: enum attribute 'mode' must declare at least one variant"

check_failure_exact Negative/CommandNonNumericInputSum.lean \
  "Negative/CommandNonNumericInputSum.lean:11:30: error: input sum field 'p.mode' must be numeric"

check_failure_exact Negative/CommandOutputSumTypeMismatch.lean \
  "Negative/CommandOutputSumTypeMismatch.lean:8:24: error: output sum value has incompatible type"

check_failure_exact Negative/CommandOversizedRows.lean \
  "Negative/CommandOversizedRows.lean:5:22: error: row count exceeds the IR u64 range"

check_failure_exact Negative/CommandGroupedOversizedRows.lean \
  "Negative/CommandGroupedOversizedRows.lean:5:22: error: row count exceeds the IR u64 range"

check_failure_exact Negative/CommandRefEffect.lean \
  "Negative/CommandRefEffect.lean:12:10: error: writes to Ref attributes require resource claims, which are not supported by this DSL"

check_failure_exact Negative/CommandUnknownSummaryBox.lean \
  "Negative/CommandUnknownSummaryBox.lean:7:19: error: summary 's' refers to unknown box 'missing'"

check_failure_exact Negative/CommandUnknownWireBox.lean \
  "Negative/CommandUnknownWireBox.lean:8:7: error: unknown wire source box 'missing'"

check_failure_exact Negative/CommandUnsupportedModelDeclaration.lean \
  "Negative/CommandUnsupportedModelDeclaration.lean:4:10: error: unsupported Sembla model declaration 'race'"

check_failure_exact Negative/ComponentDuplicateInstance.lean \
  "Negative/ComponentDuplicateInstance.lean:6:11: error: duplicate instance 'population'"

check_failure_exact Negative/ComponentUnknownConstant.lean \
  "Negative/ComponentUnknownConstant.lean:5:20: error: unknown identifier 'MissingComponent'"

check_failure_exact Negative/ComponentUnknownWirePort.lean \
  "Negative/ComponentUnknownWirePort.lean:7:24: error: unknown port 'population.missing'"

check_failure_exact Negative/ComponentUnknownExposePort.lean \
  "Negative/ComponentUnknownExposePort.lean:6:26: error: unknown port 'population.missing'"

check_failure_exact Negative/ComponentUnknownHidePort.lean \
  "Negative/ComponentUnknownHidePort.lean:6:18: error: unknown port 'population.missing'"

check_failure_exact Negative/ComponentUnlabeledWire.lean \
  "Negative/ComponentUnlabeledWire.lean:7:2: error: wire declarations require an explicit label before ':'"

check_failure_exact Negative/ComponentUnsupportedInstanceName.lean \
  "Negative/ComponentUnsupportedInstanceName.lean:5:11: error: identifier 'North'' contains unsupported character '''"

check_failure_exact Negative/ComponentRequiresComposite.lean \
  "Negative/ComponentRequiresComposite.lean:5:2: error: requires declarations are not allowed on composite components"

check_failure_exact Negative/ComponentDt.lean \
  "Negative/ComponentDt.lean:4:30: error: dt is not allowed on a component"

check_failure_exact Negative/CompositionMissingRoot.lean \
  "Negative/CompositionMissingRoot.lean:4:19: error: sembla_composition requires exactly one root"

check_failure_exact Negative/CompositionMissingName.lean \
  "Negative/CompositionMissingName.lean:4:19: error: sembla_composition requires '(name := <exact slug>)'"

check_failure_exact Negative/ComponentDuplicateWireLabel.lean \
  "Negative/ComponentDuplicateWireLabel.lean:8:7: error: duplicate wire label 'duplicate'"

check_failure_exact Negative/ComponentUnknownRequirement.lean \
  "Negative/ComponentUnknownRequirement.lean:5:37: error: component 'def:population' has no requirement 'missing'"

check_failure_exact Negative/CompositionInvalidSummaryPath.lean \
  "Negative/CompositionInvalidSummaryPath.lean:7:21: error: summary source must contain only identifier path segments"

check_failure_exact Negative/CommandEffectIntTypeMismatch.lean \
  "Negative/CommandEffectIntTypeMismatch.lean:12:24: error: effect value has incompatible type"
check_failure_exact Negative/CommandEnumEffectExpression.lean \
  "Negative/CommandEnumEffectExpression.lean:12:18: error: enum effect values must be variant literals"
check_failure_exact Negative/CommandUnknownEffectIdentifier.lean \
  "Negative/CommandUnknownEffectIdentifier.lean:12:21: error: unknown state or attribute 'missing'"
check_failure_exact Negative/CommandAmbiguousEffectIdentifier.lean \
  "Negative/CommandAmbiguousEffectIdentifier.lean:14:20: error: ambiguous identifier 'counter': both an attribute and parameter are in scope"
check_failure_exact Negative/CommandAggregateEffect.lean \
  "Negative/CommandAggregateEffect.lean:14:21: error: aggregates are not supported in effect expressions"
check_failure_exact Negative/CommandIntParamPrior.lean \
  "Negative/CommandIntParamPrior.lean:6:8: error: priors are not supported on Int parameters"
check_failure_exact Negative/CommandIntParamRealLiteral.lean \
  "Negative/CommandIntParamRealLiteral.lean:6:19: error: Int parameter defaults require an integer literal"
check_failure_exact Negative/CommandIntParamCollision.lean \
  "Negative/CommandIntParamCollision.lean:7:8: error: duplicate parameter 'n'"

lake env lean Positive/ForwardRefPriorless.lean
lake env lean Positive/OutputFieldOrder.lean
lake env lean Positive/ObservationOrder.lean
lake env lean Positive/OptionBBindersNames.lean
lake env lean Sembla/ReactionArrowTests.lean
lake env lean Sembla/FrequencyTests.lean
lake env lean Sembla/CommandFrontendTests.lean
echo "Lean positioned negative and positive elaboration tests passed"
