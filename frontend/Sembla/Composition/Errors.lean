import Sembla.Composition.Source

namespace Sembla.Composition

/-- Stable linker error codes for `sembla.linker/v1`.  Later composition PRDs
    activate the port- and wire-specific cases already reserved here. -/
inductive LinkErrorCodeV1 where
  | unknownVersion | unsupportedFeature | unsupportedConstruct
  | duplicateStableId | missingDefinition | recursiveDefinition
  | missingPort | inaccessibleDescendantPort | hiddenPortConflict
  | directionMismatch | schemaMismatch
  | multipleDrivers | unboundParameter | ambiguousParameterBinding
  | identityCollision | reservedRuntimeIdentity
  | invalidSummary
deriving Repr, BEq, Ord

structure LinkErrorV1 where
  code : LinkErrorCodeV1
  message : String
  primary : StableId
  related : List StableId
deriving Repr, BEq

/-- Frozen CLI spelling of a stable linker error code. -/
def LinkErrorCodeV1.toString : LinkErrorCodeV1 → String
  | .unknownVersion => "unknownVersion"
  | .unsupportedFeature => "unsupportedFeature"
  | .unsupportedConstruct => "unsupportedConstruct"
  | .duplicateStableId => "duplicateStableId"
  | .missingDefinition => "missingDefinition"
  | .recursiveDefinition => "recursiveDefinition"
  | .missingPort => "missingPort"
  | .inaccessibleDescendantPort => "inaccessibleDescendantPort"
  | .hiddenPortConflict => "hiddenPortConflict"
  | .directionMismatch => "directionMismatch"
  | .schemaMismatch => "schemaMismatch"
  | .multipleDrivers => "multipleDrivers"
  | .unboundParameter => "unboundParameter"
  | .ambiguousParameterBinding => "ambiguousParameterBinding"
  | .identityCollision => "identityCollision"
  | .reservedRuntimeIdentity => "reservedRuntimeIdentity"
  | .invalidSummary => "invalidSummary"

private def errorLess (left right : LinkErrorV1) : Bool :=
  if compare left.code right.code == .lt then true
  else if compare right.code left.code == .lt then false
  else if left.primary.raw < right.primary.raw then true
  else if right.primary.raw < left.primary.raw then false
  else left.message < right.message

/-- Stable API order: `(code, primary, message)`. -/
def sortLinkErrors (errors : List LinkErrorV1) : List LinkErrorV1 :=
  errors.mergeSort errorLess

end Sembla.Composition
