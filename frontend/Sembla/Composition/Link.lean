import Sembla.Composition.Errors
import Sembla.Composition.Json
import Sembla.Hash
import Sembla.PlanExport
import Sembla.PlanJson

namespace Sembla.Composition

open Sembla

structure LinkStatisticsV1 where
  leaves : Nat
  transitions : Nat
  mailboxes : Nat
deriving Repr, BEq

structure LinkReportV1 where
  warnings : List String
  statistics : LinkStatisticsV1
deriving Repr, BEq

structure LinkResultV1 where
  plan : Plan.ExecutablePlanV1
  report : LinkReportV1
deriving Repr, BEq

private def sid (raw : String) : StableId := ⟨raw⟩

private def mkError
    (code : LinkErrorCodeV1) (primary : StableId) (message : String)
    (related : List StableId := []) : LinkErrorV1 :=
  { code, message, primary, related }

private def fail (errors : List LinkErrorV1) : Except (List LinkErrorV1) α :=
  .error (sortLinkErrors errors)

private def sortBy (items : List α) (key : α → String) : List α :=
  items.mergeSort fun left right => key left < key right

private def idSlug (kindPrefix : String) (id : StableId) : String :=
  id.raw.drop kindPrefix.length

private def definition? (definitions : List ComponentDefinitionV1) (id : StableId) :
    Option ComponentDefinitionV1 :=
  definitions.find? fun definition => definition.id == id

private def envelopeErrors (source : CompositionSourceV1) : List LinkErrorV1 :=
  let versionErrors :=
    if source.schemaVersion == Plan.compositionSourceSchema then []
    else [mkError .unknownVersion source.modelId
      s!"unknown schema version '{source.schemaVersion}'; supported: {Plan.compositionSourceSchema}"]
  let featureErrors := source.requiredFeatures.map fun feature =>
    mkError .unsupportedFeature source.modelId s!"unsupported required feature '{feature}'"
  versionErrors ++ featureErrors

private def constructErrors (source : CompositionSourceV1) : List LinkErrorV1 :=
  let laterConstructs := (source.definitions.map fun definition =>
    match definition.body with
    | .primitive _ => []
    | .composite body =>
        let exposureErrors := body.exposures.map fun exposure =>
          mkError .unsupportedConstruct exposure.id
            s!"exposure '{exposure.id.raw}' is unsupported by wire linking; PRD 0009 adds exposures"
        let hiddenErrors := body.hiddenPorts.map fun hidden =>
          mkError .unsupportedConstruct definition.id
            s!"hidden port on '{definition.id.raw}' is unsupported by wire linking; PRD 0009 adds hiding"
            [hidden.instance_, hidden.port]
        exposureErrors ++ hiddenErrors).join
  let rootPrimitive := match definition? source.definitions source.rootDefinition with
    | some { body := .primitive _, .. } =>
        [mkError .unsupportedConstruct source.rootDefinition
          "primitive root definitions are unsupported in V1 because executable leaf paths must be nonempty"]
    | _ => []
  laterConstructs ++ rootPrimitive

private def duplicateDefinitionErrors (definitions : List ComponentDefinitionV1) : List LinkErrorV1 :=
  let rec loop (seen : List StableId) : List ComponentDefinitionV1 → List LinkErrorV1
    | [] => []
    | definition :: rest =>
        let errors := if seen.contains definition.id then
          [mkError .duplicateStableId definition.id
            s!"duplicate definition id '{definition.id.raw}'" [definition.id]]
        else []
        errors ++ loop (definition.id :: seen) rest
  loop [] definitions

private def missingDefinitionErrors (source : CompositionSourceV1) : List LinkErrorV1 :=
  let rootErrors := if definition? source.definitions source.rootDefinition |>.isSome then [] else
    [mkError .missingDefinition source.rootDefinition
      s!"root definition '{source.rootDefinition.raw}' does not exist"]
  let instanceErrors := (source.definitions.map fun owner =>
    match owner.body with
    | .primitive _ => []
    | .composite body => body.instances.filterMap fun item =>
        if definition? source.definitions item.definition |>.isSome then none
        else some (mkError .missingDefinition item.id
          s!"instance '{item.id.raw}' references missing definition '{item.definition.raw}'"
          [item.definition, owner.id])).join
  rootErrors ++ instanceErrors

private def suffixFrom (id : StableId) : List StableId → List StableId
  | [] => []
  | head :: rest => if head == id then head :: rest else suffixFrom id rest

private partial def collectCycles
    (definitions : List ComponentDefinitionV1) (current : StableId)
    (stack : List StableId) : List (List StableId) :=
  if stack.contains current then
    [suffixFrom current stack ++ [current]]
  else
    match definition? definitions current with
    | none => []
    | some definition =>
        match definition.body with
        | .primitive _ => []
        | .composite body => (body.instances.map fun item =>
            collectCycles definitions item.definition (stack ++ [current])).join

private def rotateTo (first : StableId) (items : List StableId) : List StableId :=
  let rec loop (before : List StableId) : List StableId → List StableId
    | [] => items
    | head :: rest =>
        if head == first then head :: rest ++ before.reverse
        else loop (head :: before) rest
  loop [] items

private def normalizeCycle (cycle : List StableId) : List StableId :=
  let core := cycle.take (cycle.length - 1)
  match sortBy core (·.raw) with
  | [] => []
  | first :: _ =>
      let rotated := rotateTo first core
      rotated ++ [first]

private def cycleErrors (source : CompositionSourceV1) : List LinkErrorV1 :=
  let candidates := (collectCycles source.definitions source.rootDefinition []).map fun cycle =>
    let normalized := normalizeCycle cycle
    match normalized with
    | [] => mkError .recursiveDefinition source.rootDefinition "recursive definition cycle"
    | primary :: _ => mkError .recursiveDefinition primary
        ("recursive definition cycle: " ++ String.intercalate " -> " (normalized.map (·.raw)))
        normalized
  candidates.foldl (fun unique candidate =>
    if unique.any fun error =>
        error.primary == candidate.primary && error.message == candidate.message then unique
    else candidate :: unique) []

private def resolveParameter (environment : List ParameterBinding) (name : String) : String :=
  match environment.find? fun binding => binding.requirement == name with
  | some binding => binding.parameter
  | none => name

private def resolvedBindings
    (environment : List ParameterBinding) (item : InstanceDeclV1)
    (target : ComponentDefinitionV1) : List ParameterBinding :=
  target.parameterRequirements.filterMap fun requirement =>
    match item.parameterBindings.find? fun binding => binding.requirement == requirement with
    | none => none
    | some binding => some {
        requirement
        parameter := resolveParameter environment binding.parameter }

private def bindingErrorsFor
    (source : CompositionSourceV1) (environment : List ParameterBinding)
    (item : InstanceDeclV1) (target : ComponentDefinitionV1) : List LinkErrorV1 :=
  let unknownRequirements := item.parameterBindings.filterMap fun binding =>
    if target.parameterRequirements.contains binding.requirement then none
    else some (mkError .ambiguousParameterBinding item.id
      s!"binding requirement '{binding.requirement}' is not declared by '{target.id.raw}'"
      [target.id])
  let requirementErrors := (target.parameterRequirements.map fun requirement =>
    let candidates := item.parameterBindings.filter fun binding => binding.requirement == requirement
    match candidates with
    | [] => [mkError .unboundParameter item.id
        s!"requirement '{requirement}' of '{target.id.raw}' is unbound" [target.id]]
    | [binding] =>
        let parameterName := resolveParameter environment binding.parameter
        if source.parameters.any fun parameter => parameter.name == parameterName then []
        else [mkError .unboundParameter item.id
          s!"requirement '{requirement}' names missing model parameter '{parameterName}'"
          [target.id]]
    | _ => [mkError .ambiguousParameterBinding item.id
        s!"requirement '{requirement}' of '{target.id.raw}' has multiple bindings" [target.id]]).join
  unknownRequirements ++ requirementErrors

private partial def reachableBindingErrors
    (source : CompositionSourceV1) (definition : ComponentDefinitionV1)
    (environment : List ParameterBinding) : List LinkErrorV1 :=
  match definition.body with
  | .primitive _ => []
  | .composite body => (body.instances.map fun item =>
      match definition? source.definitions item.definition with
      | none => []
      | some target =>
          bindingErrorsFor source environment item target ++
          reachableBindingErrors source target (resolvedBindings environment item target)).join

mutual
  private partial def rewriteExpr (bindings : List ParameterBinding) : IR.Expr → IR.Expr
    | .real value => .real value
    | .int value => .int value
    | .bool value => .bool value
    | .enum variant => .enum variant
    | .param name =>
        match bindings.find? fun binding => binding.requirement == name with
        | some binding => .param binding.parameter
        | none => .param name
    | .selfAttr name => .selfAttr name
    | .add lhs rhs => .add (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .sub lhs rhs => .sub (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .mul lhs rhs => .mul (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .div lhs rhs => .div (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .eq lhs rhs => .eq (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .ne lhs rhs => .ne (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .lt lhs rhs => .lt (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .le lhs rhs => .le (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .gt lhs rhs => .gt (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .ge lhs rhs => .ge (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .and lhs rhs => .and (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .or lhs rhs => .or (rewriteExpr bindings lhs) (rewriteExpr bindings rhs)
    | .not expression => .not (rewriteExpr bindings expression)
    | .enumIs attr variant => .enumIs attr variant
    | .input port aggregate => .input port (rewriteAggregate bindings aggregate)
    | .agg operation table fkAttr selfFkAttr filter =>
        .agg (rewriteAggOp bindings operation) table fkAttr selfFkAttr (rewriteExpr bindings filter)

  private partial def rewriteAggOp (bindings : List ParameterBinding) : IR.AggOp → IR.AggOp
    | .count => .count
    | .sum value => .sum (rewriteExpr bindings value)

  private partial def rewriteAggregate
      (bindings : List ParameterBinding) : IR.Aggregate → IR.Aggregate
    | .mk operation filter =>
        .mk (rewriteAggOp bindings operation) (filter.map (rewriteExpr bindings))
end

private def rewriteTransition
    (bindings : List ParameterBinding) (transition : IR.Transition) : IR.Transition :=
  { transition with
    guard := rewriteExpr bindings transition.guard
    hazard := rewriteExpr bindings transition.hazard
    effects := transition.effects.map fun effect =>
      match effect with
      | .setAttr attr value => .setAttr attr (rewriteExpr bindings value)
    contests := transition.contests.map fun claim =>
      { resource := rewriteExpr bindings claim.resource
        ordering := match claim.ordering with
          | .raceTime => .raceTime
          | .key expression => .key (rewriteExpr bindings expression) } }

private def rewriteOutput
    (bindings : List ParameterBinding) (output : IR.OutputDecl) : IR.OutputDecl :=
  { output with builder := match output.builder with
    | .perTable table fields => .perTable table (fields.map fun field =>
        { field with
          op := rewriteAggOp bindings field.op
          filter := field.filter.map (rewriteExpr bindings) }) }

private def rewriteView
    (bindings : List ParameterBinding) (view : IR.ViewDecl) : IR.ViewDecl :=
  { view with
    filter := view.filter.map (rewriteExpr bindings)
    value := view.value.map (rewriteExpr bindings) }

private def canonicalBox (modelBox : IR.Box) : IR.Box :=
  { modelBox with
    tables := sortBy modelBox.tables (·.name)
    transitions := sortBy modelBox.transitions (·.name)
    inputs := sortBy modelBox.inputs (·.name)
    outputs := sortBy modelBox.outputs (·.name)
    views := sortBy modelBox.views (·.name) }

private structure ExpandedLeaf where
  modelBox : IR.Box
  occurrence : String
  definition : StableId
  ownerInstance : StableId
  instancePath : List StableId
  bindings : List ParameterBinding
deriving Repr, BEq

private partial def expandDefinition
    (definitions : List ComponentDefinitionV1) (definition : ComponentDefinitionV1)
    (chain : List String) (instancePath : List StableId)
    (bindings : List ParameterBinding) : List ExpandedLeaf :=
  match definition.body with
  | .primitive body =>
      let boxName := if chain.isEmpty then idSlug "def:" definition.id
        else String.intercalate "/" chain
      [{ modelBox := canonicalBox {
           name := boxName
           tables := body.tables
           «transitions» := body.transitions.map (rewriteTransition bindings)
           «inputs» := body.inputs
           «outputs» := body.outputs.map (rewriteOutput bindings)
           «views» := body.views.map (rewriteView bindings) }
         occurrence := "occ:" ++ boxName
         definition := definition.id
         ownerInstance := match instancePath.reverse with
           | owner :: _ => owner
           | [] => definition.id
         instancePath
         bindings }]
  | .composite body => (body.instances.map fun item =>
      match definition? definitions item.definition with
      | none => []
      | some target => expandDefinition definitions target
          (chain ++ [idSlug "inst:" item.id]) (instancePath ++ [item.id])
          (resolvedBindings bindings item target)).join

private structure ResolvedWireEndpoint where
  instance_ : InstanceDeclV1
  definition : ComponentDefinitionV1
  port : PortDeclV1
  boxName : String
  occurrence : String

private structure ExpandedWire where
  modelWire : IR.Wire
  declaration : StableId
  wireOccurrence : String
  sourceOccurrence : String
  targetOccurrence : String
  sourcePort : StableId
  targetPort : StableId

deriving Repr, BEq

private structure WireResolution where
  errors : List LinkErrorV1
  wire : Option ExpandedWire

private def resolveWireEndpoint
    (definitions : List ComponentDefinitionV1) (owner : CompositeBodyV1)
    (ownerChain : List String) (wire : WireDeclV1) (childId portId : StableId) :
    Except LinkErrorV1 ResolvedWireEndpoint := do
  let child ← match owner.instances.find? fun item => item.id == childId with
    | some child => pure child
    | none => throw (mkError .missingPort wire.id
        s!"wire '{wire.id.raw}' references missing direct child '{childId.raw}'" [childId])
  let childDefinition ← match definition? definitions child.definition with
    | some childDefinition => pure childDefinition
    | none => throw (mkError .missingPort wire.id
        s!"wire '{wire.id.raw}' cannot resolve child definition '{child.definition.raw}'"
        [child.id, child.definition])
  match childDefinition.body with
  | .composite _ =>
      throw (mkError .missingPort wire.id
        s!"wire '{wire.id.raw}' references composite child '{child.id.raw}'; boundary wiring requires exposures (PRD 0009)"
        [child.id, childDefinition.id, portId])
  | .primitive _ =>
      let port ← match childDefinition.ports.find? fun candidate => candidate.id == portId with
        | some port => pure port
        | none => throw (mkError .missingPort wire.id
            s!"wire '{wire.id.raw}' references missing port '{portId.raw}' on '{child.id.raw}'"
            [child.id, childDefinition.id, portId])
      let chain := ownerChain ++ [idSlug "inst:" child.id]
      let boxName := String.intercalate "/" chain
      pure {
        instance_ := child
        definition := childDefinition
        port
        boxName
        occurrence := "occ:" ++ boxName }

private def resolveWire
    (definitions : List ComponentDefinitionV1) (owner : CompositeBodyV1)
    (ownerChain : List String) (wire : WireDeclV1) : WireResolution :=
  let source := resolveWireEndpoint definitions owner ownerChain wire
    wire.sourceInstance wire.sourcePort
  let target := resolveWireEndpoint definitions owner ownerChain wire
    wire.targetInstance wire.targetPort
  match source, target with
  | .error sourceError, .error targetError => { errors := [sourceError, targetError], wire := none }
  | .error sourceError, .ok _ => { errors := [sourceError], wire := none }
  | .ok _, .error targetError => { errors := [targetError], wire := none }
  | .ok source, .ok target =>
      let directionErrors :=
        (if source.port.direction == .output then [] else
          [mkError .directionMismatch wire.id
            s!"wire '{wire.id.raw}' source port '{wire.sourcePort.raw}' is not an output"
            [source.instance_.id, source.definition.id, source.port.id]]) ++
        (if target.port.direction == .input then [] else
          [mkError .directionMismatch wire.id
            s!"wire '{wire.id.raw}' target port '{wire.targetPort.raw}' is not an input"
            [target.instance_.id, target.definition.id, target.port.id]])
      let schemaErrors := if source.port.schema == target.port.schema then [] else
        [mkError .schemaMismatch wire.id
          s!"wire '{wire.id.raw}' schema mismatch: source {reprStr source.port.schema}; target {reprStr target.port.schema}"
          [source.port.id, target.port.id]]
      let errors := directionErrors ++ schemaErrors
      if !errors.isEmpty then { errors, wire := none }
      else
        let ownerOccurrence := "occ:" ++ String.intercalate "/" ownerChain
        let wireOccurrence := ownerOccurrence ++ "#wire:" ++ idSlug "wire:" wire.id
        { errors := []
          wire := some {
            modelWire := {
              source := { box := source.boxName, port := idSlug "port:" source.port.id }
              target := { box := target.boxName, port := idSlug "port:" target.port.id } }
            declaration := wire.id
            wireOccurrence
            sourceOccurrence := source.occurrence
            targetOccurrence := target.occurrence
            sourcePort := source.port.id
            targetPort := target.port.id } }

private partial def expandWires
    (definitions : List ComponentDefinitionV1) (definition : ComponentDefinitionV1)
    (chain : List String) : List LinkErrorV1 × List ExpandedWire :=
  match definition.body with
  | .primitive _ => ([], [])
  | .composite body =>
      let localResults := body.wires.map (resolveWire definitions body chain)
      let nested := body.instances.map fun item =>
        match definition? definitions item.definition with
        | none => ([], [])
        | some child => expandWires definitions child (chain ++ [idSlug "inst:" item.id])
      ((localResults.map (·.errors)).join ++ (nested.map (·.1)).join,
       localResults.filterMap (·.wire) ++ (nested.map (·.2)).join)

private def wireTargetKey (wire : ExpandedWire) : String :=
  wire.modelWire.target.box ++ "|" ++ wire.modelWire.target.port

private def reservedWireIdentityErrors (wires : List ExpandedWire) : List LinkErrorV1 :=
  wires.filterMap fun wire =>
    let synthesized := "occ:#wire:to_" ++ wire.modelWire.target.box ++ "_" ++
      wire.modelWire.target.port
    if wire.wireOccurrence == synthesized then
      some (mkError .reservedRuntimeIdentity wire.declaration
        s!"wire id '{wire.declaration.raw}' is reserved because its linked occurrence collides with the direct_stable synthesized form")
    else none

private def multipleDriverErrors (wires : List ExpandedWire) : List LinkErrorV1 :=
  (wires.map wireTargetKey).eraseDups.filterMap fun key =>
    let drivers := sortBy (wires.filter fun wire => wireTargetKey wire == key) (·.wireOccurrence)
    match drivers with
    | first :: _ :: _ => some (mkError .multipleDrivers first.declaration
        s!"multiple wires drive '{first.modelWire.target.box}.{first.modelWire.target.port}'"
        (drivers.map (·.declaration)))
    | _ => none

private def expandedMailbox (wire : ExpandedWire) : Plan.MailboxIdentityV1 := {
  identity := "mbox:" ++ wire.wireOccurrence ++ "|" ++
    wire.sourceOccurrence ++ "." ++ wire.sourcePort.raw ++ "|" ++
    wire.targetOccurrence ++ "." ++ wire.targetPort.raw
  sourceBox := wire.modelWire.source.box
  sourcePort := wire.modelWire.source.port
  targetBox := wire.modelWire.target.box
  targetPort := wire.modelWire.target.port }

private def identityErrors
    (transitions : List Plan.TransitionIdentityV1) : List LinkErrorV1 :=
  let rec loop (seen : List (UInt32 × String)) : List Plan.TransitionIdentityV1 → List LinkErrorV1
    | [] => []
    | transition :: rest =>
        let reserved := if Hash.isReservedRuleWord transition.ruleWord then
          [mkError .reservedRuntimeIdentity (sid transition.identity)
            s!"rule word {transition.ruleWord.toNat} for '{transition.identity}' is reserved"]
        else []
        let collision := match seen.find? fun entry => entry.1 == transition.ruleWord with
          | none => []
          | some first => [mkError .identityCollision (sid transition.identity)
              s!"rule word {transition.ruleWord.toNat} collides for '{transition.identity}' and '{first.2}'; rename a stable id"
              [sid first.2]]
        reserved ++ collision ++ loop ((transition.ruleWord, transition.identity) :: seen) rest
  loop [] transitions

private def resolveSummaries
    (source : CompositionSourceV1) (leaves : List ExpandedLeaf) :
    List LinkErrorV1 × List IR.SummaryDecl :=
  let results := source.summaries.map fun summary =>
    let path := summary.instancePath.map (idSlug "inst:")
    let pathText := String.intercalate "/" path
    match leaves.find? fun leaf => leaf.instancePath == summary.instancePath with
    | none => Sum.inl (mkError .invalidSummary source.modelId
        s!"summary '{summary.name}' path '{pathText}' does not resolve to a leaf"
        summary.instancePath)
    | some leaf =>
        if leaf.modelBox.views.any fun view => view.name == summary.view then
          Sum.inr (IR.SummaryDecl.mk summary.name leaf.modelBox.name summary.view summary.reduce)
        else Sum.inl (mkError .invalidSummary source.modelId
          s!"summary '{summary.name}' view '{summary.view}' does not exist on '{leaf.modelBox.name}'"
          summary.instancePath)
  (results.filterMap fun result => match result with | .inl error => some error | .inr _ => none,
   results.filterMap fun result => match result with | .inl _ => none | .inr summary => some summary)

private def optionAll (predicate : α → Bool) : Option α → Bool
  | none => true
  | some value => predicate value

mutual
  private partial def expressionParametersValid (parameters : List String) : IR.Expr → Bool
    | .real _ | .int _ | .bool _ | .enum _ | .selfAttr _ | .enumIs _ _ => true
    | .param name => parameters.contains name
    | .add lhs rhs | .sub lhs rhs | .mul lhs rhs | .div lhs rhs
    | .eq lhs rhs | .ne lhs rhs | .lt lhs rhs | .le lhs rhs
    | .gt lhs rhs | .ge lhs rhs | .and lhs rhs | .or lhs rhs =>
        expressionParametersValid parameters lhs && expressionParametersValid parameters rhs
    | .not expression => expressionParametersValid parameters expression
    | .input _ aggregate => aggregateParametersValid parameters aggregate
    | .agg operation _ _ _ filter =>
        aggOpParametersValid parameters operation && expressionParametersValid parameters filter

  private partial def aggOpParametersValid (parameters : List String) : IR.AggOp → Bool
    | .count => true
    | .sum value => expressionParametersValid parameters value

  private partial def aggregateParametersValid
      (parameters : List String) : IR.Aggregate → Bool
    | .mk operation filter =>
        aggOpParametersValid parameters operation &&
        optionAll (expressionParametersValid parameters) filter
end

private def namesUnique (names : List String) : Bool :=
  names.eraseDups.length == names.length

private def slugNamesValidAndUnique (names : List String) : Bool :=
  names.all PlanExport.isSlug && namesUnique names

private def schemaValid (schema : List IR.Attr) : Bool :=
  namesUnique (schema.map (·.name))

private partial def normalizeRuntimeScientific (value : IR.Scientific) : IR.Scientific :=
  if value.coefficient != 0 && value.coefficient % 10 == 0 then
    normalizeRuntimeScientific ⟨value.coefficient / 10, value.exponent + 1⟩
  else value

private def decimalDigitCount (value : Nat) : Nat :=
  (toString value).length

private def scientificDecimalOrder (value : IR.Scientific) : Int :=
  value.exponent + Int.ofNat (decimalDigitCount value.coefficient.natAbs) - 1

private def scientificToJsonNumber (value : IR.Scientific) : Lean.JsonNumber :=
  if value.exponent < 0 then
    .mk value.coefficient (-value.exponent).toNat
  else
    .mk (value.coefficient * (10 : Int) ^ value.exponent.toNat) 0

private def scientificToFloat (value : IR.Scientific) : Float :=
  (scientificToJsonNumber value).toFloat

private structure PositiveRational where
  numerator : Nat
  denominator : Nat

private def floatRational (value : Float) : PositiveRational :=
  let (fraction, exponent) := value.abs.frExp
  let significand := (fraction.scaleB 53).toUInt64.toNat
  let shift := exponent - 53
  if shift < 0 then
    ⟨significand, 2 ^ (-shift).toNat⟩
  else
    ⟨significand * 2 ^ shift.toNat, 1⟩

private def decimalRational (coefficient : Nat) (exponent : Int) : PositiveRational :=
  if exponent < 0 then
    ⟨coefficient, 10 ^ (-exponent).toNat⟩
  else
    ⟨coefficient * 10 ^ exponent.toNat, 1⟩

private def scaledCoefficientRatio
    (target : PositiveRational) (exponent : Int) : PositiveRational :=
  if exponent < 0 then
    ⟨target.numerator * 10 ^ (-exponent).toNat, target.denominator⟩
  else
    ⟨target.numerator, target.denominator * 10 ^ exponent.toNat⟩

private def nearbyDecimalCoefficients (ratio : PositiveRational) : List Nat :=
  let floor := ratio.numerator / ratio.denominator
  [floor - 1, floor, floor + 1, floor + 2].eraseDups

private def rationalDistance
    (target candidate : PositiveRational) : Nat × Nat :=
  let left := candidate.numerator * target.denominator
  let right := target.numerator * candidate.denominator
  let numerator := if left < right then right - left else left - right
  (numerator, candidate.denominator * target.denominator)

private def rationalDistanceLess (left right : Nat × Nat) : Bool :=
  left.1 * right.2 < right.1 * left.2

private def canonicalCandidateBetter
    (target : PositiveRational) (candidate current : IR.Scientific) : Bool :=
  let candidateDistance := rationalDistance target
    (decimalRational candidate.coefficient.natAbs candidate.exponent)
  let currentDistance := rationalDistance target
    (decimalRational current.coefficient.natAbs current.exponent)
  if rationalDistanceLess candidateDistance currentDistance then true
  else if rationalDistanceLess currentDistance candidateDistance then false
  else
    let candidateEven := candidate.coefficient.natAbs % 2 == 0
    let currentEven := current.coefficient.natAbs % 2 == 0
    if candidateEven != currentEven then candidateEven
    else candidate.coefficient.natAbs < current.coefficient.natAbs

private def canonicalCandidatesForDigits
    (target : Float) (targetRational : PositiveRational) (negative : Bool)
    (sourceOrder : Int) (digits : Nat) : List IR.Scientific :=
  let orders := [sourceOrder - 1, sourceOrder, sourceOrder + 1]
  (orders.map fun order =>
    let exponent := order - Int.ofNat digits + 1
    let ratio := scaledCoefficientRatio targetRational exponent
    (nearbyDecimalCoefficients ratio).filterMap fun coefficient =>
      if coefficient == 0 || decimalDigitCount coefficient != digits then none
      else
        let signedCoefficient :=
          if negative then -(Int.ofNat coefficient) else Int.ofNat coefficient
        let candidate := normalizeRuntimeScientific ⟨signedCoefficient, exponent⟩
        if decimalDigitCount candidate.coefficient.natAbs == digits &&
            scientificToFloat candidate == target then some candidate
        else none).join.eraseDups

private def chooseCanonicalCandidate
    (target : PositiveRational) (candidates : List IR.Scientific) : Option IR.Scientific :=
  match candidates with
  | [] => none
  | first :: rest => some (rest.foldl (fun current candidate =>
      if canonicalCandidateBetter target candidate current then candidate else current) first)

/-- Recover serde/Ryu's shortest round-tripping decimal without routing through
    Lean's deliberately low-precision `Float.toString`. Binary64 has a
    shortest representation of at most 17 significant decimal digits. -/
private def canonicalScientificForFloat?
    (source : IR.Scientific) (target : Float) : Option IR.Scientific :=
  let normalized := normalizeRuntimeScientific source
  let targetRational := floatRational target
  let negative := normalized.coefficient < 0
  let sourceOrder := scientificDecimalOrder normalized
  let rec search (remaining digits : Nat) : Option IR.Scientific :=
    match remaining with
    | 0 => none
    | remaining + 1 =>
        let candidates := canonicalCandidatesForDigits
          target targetRational negative sourceOrder digits
        match chooseCanonicalCandidate targetRational candidates with
        | some candidate => some candidate
        | none => search remaining (digits + 1)
  search 17 1

/-- Whether an exact source decimal is already the finite, non-underflowing,
    shortest binary64 spelling that Rust's canonical JSON writer will emit. -/
private def scientificSupportedByRuntime (value : IR.Scientific) : Bool :=
  if value.coefficient == 0 then true
  else
    let normalized := normalizeRuntimeScientific value
    let decimalOrder := scientificDecimalOrder normalized
    if decimalOrder < -324 || decimalOrder > 308 then false
    else
      let converted := scientificToFloat normalized
      converted.isFinite && converted != 0.0 &&
        canonicalScientificForFloat? normalized converted == some normalized

private def parameterValid (parameter : IR.ParamDecl) : Bool :=
  match parameter.ty, parameter.default with
  | .real, .real _ => true
  | .int, .int _ => true
  | _, _ => false

private inductive ExprValueType where
  | real | int | bool | enum (variants : List String) | ref (table : String)
deriving BEq

private def ExprValueType.numeric : ExprValueType → Bool
  | .real | .int => true
  | _ => false

private def ExprValueType.orderable : ExprValueType → Bool
  | .real | .int | .enum _ => true
  | _ => false

private def attrValueType : IR.AttrType → ExprValueType
  | .real => .real
  | .int => .int
  | .enum variants => .enum variants
  | .ref table => .ref table

private def parameterValueType : IR.ParamType → ExprValueType
  | .real => .real
  | .int => .int

private def findAttrType (attrs : List IR.Attr) (name : String) : Option ExprValueType :=
  (attrs.find? fun attr => attr.name == name).map fun attr => attrValueType attr.ty

private def inputRowExpressionValid : IR.Expr → Bool
  | .input _ _ | .agg _ _ _ _ _ => false
  | .add lhs rhs | .sub lhs rhs | .mul lhs rhs | .div lhs rhs
  | .eq lhs rhs | .ne lhs rhs | .lt lhs rhs | .le lhs rhs
  | .gt lhs rhs | .ge lhs rhs | .and lhs rhs | .or lhs rhs =>
      inputRowExpressionValid lhs && inputRowExpressionValid rhs
  | .not expression => inputRowExpressionValid expression
  | .real _ | .int _ | .bool _ | .enum _ | .param _ | .selfAttr _ | .enumIs _ _ => true

mutual
  private partial def inferExpression
      (model : IR.Model) (modelBox : IR.Box) (rowAttrs : List IR.Attr)
      (expected : Option ExprValueType) : IR.Expr → Option ExprValueType
    | .real value => if scientificSupportedByRuntime value then some .real else none
    | .int _ => some .int
    | .bool _ => some .bool
    | .enum variant => match expected with
        | some (.enum variants) => if variants.contains variant then expected else none
        | _ => none
    | .param name =>
        (model.params.find? fun parameter => parameter.name == name).map fun parameter =>
          parameterValueType parameter.ty
    | .selfAttr name => findAttrType rowAttrs name
    | .add lhs rhs | .sub lhs rhs | .mul lhs rhs =>
        inferNumericBinary model modelBox rowAttrs false lhs rhs
    | .div lhs rhs => inferNumericBinary model modelBox rowAttrs true lhs rhs
    | .eq lhs rhs | .ne lhs rhs =>
        let pair := match lhs with
          | .enum _ =>
              match inferExpression model modelBox rowAttrs none rhs with
              | none => none
              | some rightType =>
                  (inferExpression model modelBox rowAttrs (some rightType) lhs).map fun leftType =>
                    (leftType, rightType)
          | _ =>
              match inferExpression model modelBox rowAttrs none lhs with
              | none => none
              | some leftType =>
                  (inferExpression model modelBox rowAttrs (some leftType) rhs).map fun rightType =>
                    (leftType, rightType)
        match pair with
        | none => none
        | some (leftType, rightType) =>
            if leftType == rightType || (leftType.numeric && rightType.numeric) then some .bool
            else none
    | .lt lhs rhs | .le lhs rhs | .gt lhs rhs | .ge lhs rhs =>
        match inferExpression model modelBox rowAttrs none lhs with
        | none => none
        | some leftType =>
            match inferExpression model modelBox rowAttrs (some leftType) rhs with
            | some rightType =>
                if leftType.numeric && rightType.numeric then some .bool else none
            | none => none
    | .and lhs rhs | .or lhs rhs =>
        if inferExpression model modelBox rowAttrs (some .bool) lhs == some .bool &&
            inferExpression model modelBox rowAttrs (some .bool) rhs == some .bool then some .bool
        else none
    | .not expression =>
        if inferExpression model modelBox rowAttrs (some .bool) expression == some .bool then some .bool
        else none
    | .enumIs attr variant =>
        match rowAttrs.find? fun declaration => declaration.name == attr with
        | some { ty := .enum variants, .. } => if variants.contains variant then some .bool else none
        | _ => none
    | .input port aggregate =>
        match modelBox.inputs.find? fun input => input.name == port with
        | none => none
        | some input =>
            match aggregate with
            | .mk operation filter =>
                let filterValid := match filter with
                  | none => true
                  | some expression => inputRowExpressionValid expression &&
                      inferExpression model modelBox input.schema (some .bool) expression == some .bool
                let operationValid := match operation with
                  | .count => true
                  | .sum value => inputRowExpressionValid value
                if filterValid && operationValid then
                  inferAggOp model modelBox input.schema operation
                else none
    | .agg operation table fkAttr selfFkAttr filter =>
        match modelBox.tables.find? fun candidate => candidate.name == table with
        | none => none
        | some target =>
            match findAttrType target.attrs fkAttr, findAttrType rowAttrs selfFkAttr with
            | some (.ref leftTable), some (.ref rightTable) =>
                if leftTable == rightTable &&
                    inferExpression model modelBox target.attrs (some .bool) filter == some .bool then
                  inferAggOp model modelBox target.attrs operation
                else none
            | _, _ => none

  private partial def inferAggOp
      (model : IR.Model) (modelBox : IR.Box) (rowAttrs : List IR.Attr) :
      IR.AggOp → Option ExprValueType
    | .count => some .int
    | .sum value =>
        match inferExpression model modelBox rowAttrs none value with
        | some valueType => if valueType.numeric then some valueType else none
        | none => none

  private partial def inferNumericBinary
      (model : IR.Model) (modelBox : IR.Box) (rowAttrs : List IR.Attr)
      (division : Bool) (lhs rhs : IR.Expr) : Option ExprValueType :=
    match inferExpression model modelBox rowAttrs none lhs,
        inferExpression model modelBox rowAttrs none rhs with
    | some leftType, some rightType =>
        if leftType.numeric && rightType.numeric then
          if division || leftType == .real || rightType == .real then some .real else some .int
        else none
    | _, _ => none
end

mutual
  private partial def expressionParameterNames : IR.Expr → List String
    | .real _ | .int _ | .bool _ | .enum _ | .selfAttr _ | .enumIs _ _ => []
    | .param name => [name]
    | .add lhs rhs | .sub lhs rhs | .mul lhs rhs | .div lhs rhs
    | .eq lhs rhs | .ne lhs rhs | .lt lhs rhs | .le lhs rhs
    | .gt lhs rhs | .ge lhs rhs | .and lhs rhs | .or lhs rhs =>
        expressionParameterNames lhs ++ expressionParameterNames rhs
    | .not expression => expressionParameterNames expression
    | .input _ aggregate => aggregateParameterNames aggregate
    | .agg operation _ _ _ filter =>
        aggOpParameterNames operation ++ expressionParameterNames filter

  private partial def aggOpParameterNames : IR.AggOp → List String
    | .count => []
    | .sum value => expressionParameterNames value

  private partial def aggregateParameterNames : IR.Aggregate → List String
    | .mk operation filter =>
        aggOpParameterNames operation ++ (filter.map expressionParameterNames).getD []
end

private def parametersWithWrongType
    (model : IR.Model) (expected : ExprValueType) (names : List String) : List String :=
  names.filter fun name =>
    match model.params.find? fun parameter => parameter.name == name with
    | none => false
    | some parameter => parameterValueType parameter.ty != expected

private def strictExpressionTypeMismatches
    (model : IR.Model) (modelBox : IR.Box) (rowAttrs : List IR.Attr)
    (expected : ExprValueType) (expression : IR.Expr) : List String :=
  match inferExpression model modelBox rowAttrs none expression with
  | some actual =>
      if actual == expected then []
      else parametersWithWrongType model expected (expressionParameterNames expression)
  | none => []

private partial def nestedExpressionTypeMismatches
    (model : IR.Model) (modelBox : IR.Box) (rowAttrs : List IR.Attr) : IR.Expr → List String
  | .real _ | .int _ | .bool _ | .enum _ | .param _ | .selfAttr _ | .enumIs _ _ => []
  | .add lhs rhs | .sub lhs rhs | .mul lhs rhs | .div lhs rhs
  | .eq lhs rhs | .ne lhs rhs | .lt lhs rhs | .le lhs rhs
  | .gt lhs rhs | .ge lhs rhs =>
      nestedExpressionTypeMismatches model modelBox rowAttrs lhs ++
      nestedExpressionTypeMismatches model modelBox rowAttrs rhs
  | .and lhs rhs | .or lhs rhs =>
      strictExpressionTypeMismatches model modelBox rowAttrs .bool lhs ++
      strictExpressionTypeMismatches model modelBox rowAttrs .bool rhs ++
      nestedExpressionTypeMismatches model modelBox rowAttrs lhs ++
      nestedExpressionTypeMismatches model modelBox rowAttrs rhs
  | .not expression =>
      strictExpressionTypeMismatches model modelBox rowAttrs .bool expression ++
      nestedExpressionTypeMismatches model modelBox rowAttrs expression
  | .input port (.mk operation filter) =>
      match modelBox.inputs.find? fun input => input.name == port with
      | none => []
      | some input =>
          (match operation with
            | .count => []
            | .sum value => nestedExpressionTypeMismatches model modelBox input.schema value) ++
          (match filter with
            | none => []
            | some expression =>
                strictExpressionTypeMismatches model modelBox input.schema .bool expression ++
                nestedExpressionTypeMismatches model modelBox input.schema expression)
  | .agg operation table _ _ filter =>
      match modelBox.tables.find? fun candidate => candidate.name == table with
      | none => []
      | some target =>
          (match operation with
            | .count => []
            | .sum value => nestedExpressionTypeMismatches model modelBox target.attrs value) ++
          strictExpressionTypeMismatches model modelBox target.attrs .bool filter ++
          nestedExpressionTypeMismatches model modelBox target.attrs filter

private def expressionTypeMismatches
    (model : IR.Model) (modelBox : IR.Box) (rowAttrs : List IR.Attr)
    (expected : ExprValueType) (expression : IR.Expr) : List String :=
  strictExpressionTypeMismatches model modelBox rowAttrs expected expression ++
  nestedExpressionTypeMismatches model modelBox rowAttrs expression

private def transitionParameterTypeMismatches
    (model : IR.Model) (modelBox : IR.Box) (transition : IR.Transition) : List String :=
  match modelBox.tables.find? fun table => table.name == transition.table with
  | none => []
  | some table =>
      expressionTypeMismatches model modelBox table.attrs .bool transition.guard ++
      expressionTypeMismatches model modelBox table.attrs .real transition.hazard ++
      (transition.effects.map fun effect => match effect with
        | .setAttr attr value => match findAttrType table.attrs attr with
          | none => []
          | some expected => expressionTypeMismatches model modelBox table.attrs expected value).join ++
      (transition.contests.map fun claim =>
        let resourceErrors := match inferExpression model modelBox table.attrs none claim.resource with
          | some (.ref _) => nestedExpressionTypeMismatches model modelBox table.attrs claim.resource
          | some _ => parametersWithWrongType model (.ref "")
              (expressionParameterNames claim.resource) ++
              nestedExpressionTypeMismatches model modelBox table.attrs claim.resource
          | none => nestedExpressionTypeMismatches model modelBox table.attrs claim.resource
        let orderingErrors := match claim.ordering with
          | .raceTime => []
          | .key expression => nestedExpressionTypeMismatches model modelBox table.attrs expression
        resourceErrors ++ orderingErrors).join

private def outputParameterTypeMismatches
    (model : IR.Model) (modelBox : IR.Box) (output : IR.OutputDecl) : List String :=
  match output.builder with
  | .perTable table fields =>
      match modelBox.tables.find? fun candidate => candidate.name == table with
      | none => []
      | some source => ((fields.zip output.schema).map fun (field, attr) =>
          (match field.filter with
            | none => []
            | some filter => expressionTypeMismatches model modelBox source.attrs .bool filter) ++
          match field.op with
          | .count => []
          | .sum value =>
              expressionTypeMismatches model modelBox source.attrs (attrValueType attr.ty) value).join

private def viewParameterTypeMismatches
    (model : IR.Model) (modelBox : IR.Box) (view : IR.ViewDecl) : List String :=
  match modelBox.tables.find? fun table => table.name == view.table with
  | none => []
  | some table =>
      (match view.filter with
        | none => []
        | some filter => expressionTypeMismatches model modelBox table.attrs .bool filter) ++
      (match view.value with
        | none => []
        | some value => nestedExpressionTypeMismatches model modelBox table.attrs value)

private def leafParameterTypeErrors
    (model : IR.Model) (leaf : ExpandedLeaf) : List LinkErrorV1 :=
  let names := ((leaf.modelBox.transitions.map
      (transitionParameterTypeMismatches model leaf.modelBox)).join ++
    (leaf.modelBox.outputs.map (outputParameterTypeMismatches model leaf.modelBox)).join ++
    (leaf.modelBox.views.map (viewParameterTypeMismatches model leaf.modelBox)).join).eraseDups
  names.filterMap fun parameterName =>
    match leaf.bindings.find? fun binding => binding.parameter == parameterName with
    | none => none
    | some binding => some (mkError .ambiguousParameterBinding leaf.ownerInstance
        s!"binding requirement '{binding.requirement}' to model parameter '{parameterName}' has a type incompatible with its use in '{leaf.definition.raw}'"
        [leaf.definition])

private def scientificLt (left right : IR.Scientific) : Bool :=
  if left.exponent == right.exponent then left.coefficient < right.coefficient
  else if left.exponent > right.exponent then
    left.coefficient * (10 : Int) ^ (left.exponent - right.exponent).toNat < right.coefficient
  else
    left.coefficient < right.coefficient * (10 : Int) ^ (right.exponent - left.exponent).toNat

private def parameterSemanticallyValid (parameter : IR.ParamDecl) : Bool :=
  parameterValid parameter &&
  (match parameter.default with
    | .real value => scientificSupportedByRuntime value
    | .int _ => true) &&
  match parameter.prior with
  | none => true
  | some prior =>
      parameter.ty == .real && prior.args.length == 2 &&
      prior.args.all scientificSupportedByRuntime && match prior.family with
      | .uniform => match prior.args with
          | [lower, upper] => scientificLt lower upper
          | _ => false
      | .normal | .logNormal => true

private def schemaSemanticallyValid (modelBox : IR.Box) (schema : List IR.Attr) : Bool :=
  schemaValid schema && schema.all fun attr => match attr.ty with
  | .enum variants => !variants.isEmpty && namesUnique variants
  | .ref table => modelBox.tables.any fun candidate => candidate.name == table
  | .real | .int => true

private def transitionSemanticallyValid
    (model : IR.Model) (modelBox : IR.Box) (transition : IR.Transition) : Bool :=
  match modelBox.tables.find? fun table => table.name == transition.table with
  | none => false
  | some table =>
      let resources := transition.contests.map (·.resource)
      inferExpression model modelBox table.attrs (some .bool) transition.guard == some .bool &&
      inferExpression model modelBox table.attrs (some .real) transition.hazard == some .real &&
      (match transition.hazard with | .real value => value.coefficient >= 0 | _ => true) &&
      transition.effects.all (fun effect => match effect with
        | .setAttr attr value => match findAttrType table.attrs attr with
          | none => false
          | some destination => inferExpression model modelBox table.attrs (some destination) value ==
              some destination) &&
      resources.eraseDups.length == resources.length &&
      transition.contests.all (fun claim =>
        (match inferExpression model modelBox table.attrs none claim.resource with
          | some (.ref _) => true
          | _ => false) && match claim.ordering with
          | .raceTime => true
          | .key expression => match inferExpression model modelBox table.attrs none expression with
              | some valueType => valueType.orderable
              | none => false) &&
      transition.effects.all fun effect => match effect with
        | .setAttr attr value => match findAttrType table.attrs attr with
          | some (.ref _) => transition.contests.any fun claim => claim.resource == value
          | _ => true

private def outputSemanticallyValid
    (model : IR.Model) (modelBox : IR.Box) (output : IR.OutputDecl) : Bool :=
  schemaSemanticallyValid modelBox output.schema && match output.builder with
  | .perTable table fields =>
      match modelBox.tables.find? fun candidate => candidate.name == table with
      | none => false
      | some source =>
          fields.length == output.schema.length &&
          (fields.zip output.schema).all fun (field, attr) =>
            field.name == attr.name &&
            optionAll (fun filter =>
              inferExpression model modelBox source.attrs (some .bool) filter == some .bool)
              field.filter &&
            inferAggOp model modelBox source.attrs field.op == some (attrValueType attr.ty)

private def viewSemanticallyValid
    (model : IR.Model) (modelBox : IR.Box) (view : IR.ViewDecl) : Bool :=
  match modelBox.tables.find? fun table => table.name == view.table with
  | none => false
  | some table =>
      optionAll (fun filter =>
        inferExpression model modelBox table.attrs (some .bool) filter == some .bool) view.filter &&
      match view.reduce, view.value with
      | .count, none => true
      | .count, some _ => false
      | _, none => false
      | _, some value => match inferExpression model modelBox table.attrs none value with
          | some valueType => valueType.numeric
          | none => false

private def modelBoxIrValid (model : IR.Model) (modelBox : IR.Box) : Bool :=
  let tableNames := modelBox.tables.map (·.name)
  let transitionNames := modelBox.transitions.map (·.name)
  let inputNames := modelBox.inputs.map (·.name)
  let outputNames := modelBox.outputs.map (·.name)
  let viewNames := modelBox.views.map (·.name)
  namesUnique tableNames &&
  slugNamesValidAndUnique transitionNames &&
  slugNamesValidAndUnique inputNames &&
  slugNamesValidAndUnique outputNames &&
  namesUnique viewNames &&
  modelBox.tables.all (fun table => schemaSemanticallyValid modelBox table.attrs) &&
  modelBox.inputs.all (fun input => schemaSemanticallyValid modelBox input.schema) &&
  modelBox.outputs.all (outputSemanticallyValid model modelBox) &&
  modelBox.transitions.all (transitionSemanticallyValid model modelBox) &&
  modelBox.views.all (viewSemanticallyValid model modelBox)

private def modelWireValid (model : IR.Model) (wire : IR.Wire) : Bool :=
  match model.boxes.find? fun modelBox => modelBox.name == wire.source.box,
        model.boxes.find? fun modelBox => modelBox.name == wire.target.box with
  | some sourceBox, some targetBox =>
      match sourceBox.outputs.find? fun output => output.name == wire.source.port,
            targetBox.inputs.find? fun input => input.name == wire.target.port with
      | some output, some input => output.schema == input.schema
      | _, _ => false
  | _, _ => false

private def wireEndpointKey (wire : IR.Wire) : String :=
  wire.source.box ++ "|" ++ wire.source.port ++ "|" ++
    wire.target.box ++ "|" ++ wire.target.port

private def mailboxEndpointKey (mailbox : Plan.MailboxIdentityV1) : String :=
  mailbox.sourceBox ++ "|" ++ mailbox.sourcePort ++ "|" ++
    mailbox.targetBox ++ "|" ++ mailbox.targetPort

private def singleDriverValid (wires : List IR.Wire) : Bool :=
  let targets := wires.map fun wire => wire.target.box ++ "|" ++ wire.target.port
  targets.eraseDups.length == targets.length

private def occurrenceChainValid (value : String) : Bool :=
  if !value.startsWith "occ:" then false
  else
    let chain := value.drop 4
    chain.isEmpty || (chain.splitOn "/").all PlanExport.isSlug

private def linkedMailboxIdentityValid (mailbox : Plan.MailboxIdentityV1) : Bool :=
  match mailbox.identity.splitOn "|" with
  | [head, source, target] =>
      if !head.startsWith "mbox:" then false
      else
        let wireOccurrence := head.drop 5
        match wireOccurrence.splitOn "#wire:" with
        | [ownerOccurrence, wireSlug] =>
            occurrenceChainValid ownerOccurrence && PlanExport.isSlug wireSlug &&
            source == "occ:" ++ mailbox.sourceBox ++ ".port:" ++ mailbox.sourcePort &&
            target == "occ:" ++ mailbox.targetBox ++ ".port:" ++ mailbox.targetPort
        | _ => false
  | _ => false

private def modelIrValid (model : IR.Model) : Bool :=
  let parameterNames := model.params.map (·.name)
  namesUnique parameterNames && model.params.all parameterSemanticallyValid &&
  scientificSupportedByRuntime model.dt && model.dt.coefficient > 0 &&
  PlanExport.isSlug model.name &&
  (model.boxes.map (·.name)).eraseDups.length == model.boxes.length &&
  model.boxes.all (fun modelBox =>
    (modelBox.name.splitOn "/").all PlanExport.isSlug &&
    modelBoxIrValid model modelBox) &&
  model.wires.all (modelWireValid model) && singleDriverValid model.wires &&
  namesUnique (model.summaries.map (·.name)) &&
  model.summaries.all (fun summary =>
    match model.boxes.find? fun modelBox => modelBox.name == summary.box with
    | none => false
    | some modelBox => modelBox.views.any fun view => view.name == summary.view)

private def allDistinctWords (transitions : List Plan.TransitionIdentityV1) : Bool :=
  let rec loop (seen : List UInt32) : List Plan.TransitionIdentityV1 → Bool
    | [] => true
    | transition :: rest =>
        !Hash.isReservedRuleWord transition.ruleWord &&
        !seen.contains transition.ruleWord && loop (transition.ruleWord :: seen) rest
  loop [] transitions

/-- Executable-plan invariants rechecked at the final linker boundary. -/
def planValidCheck (plan : Plan.ExecutablePlanV1) : Bool :=
  let expectedLeaves := sortBy (plan.model.boxes.map fun modelBox =>
    { box := modelBox.name, occurrence := "occ:" ++ modelBox.name : Plan.LeafIdentityV1 }) (·.box)
  let expectedTransitions := sortBy ((plan.model.boxes.map fun modelBox =>
    modelBox.transitions.map fun transition =>
      let identity := "occ:" ++ modelBox.name ++ "#" ++ transition.name
      { box := modelBox.name, name := transition.name, identity,
        ruleWord := Hash.ruleWord identity : Plan.TransitionIdentityV1 }).join) (·.identity)
  let expectedScheduler : List Plan.SchedulerDomainV1 := [{
    id := Plan.globalSchedulerDomain
    algorithm := Plan.tauLeapAlgorithm
    leaves := expectedLeaves.map (·.box) }]
  plan.schemaVersion == Plan.planSchema &&
  plan.identityScheme == Plan.stableIdentityScheme &&
  plan.origin == .linked &&
  plan.identity.modelId == "model:" ++ plan.model.name &&
  plan.identity.enabledFeatures.isEmpty &&
  plan.identity.leaves == expectedLeaves &&
  plan.identity.transitions == expectedTransitions &&
  plan.identity.schedulerDomains == expectedScheduler &&
  plan.identity.mailboxes == sortBy plan.identity.mailboxes (·.identity) &&
  namesUnique (plan.identity.mailboxes.map (·.identity)) &&
  plan.identity.mailboxes.all linkedMailboxIdentityValid &&
  sortBy (plan.identity.mailboxes.map mailboxEndpointKey) id ==
    sortBy (plan.model.wires.map wireEndpointKey) id &&
  plan.model.wires.map wireEndpointKey ==
    plan.identity.mailboxes.map mailboxEndpointKey &&
  modelIrValid plan.model &&
  plan.model.params == sortBy plan.model.params (·.name) &&
  plan.model.boxes == sortBy plan.model.boxes (·.name) &&
  (plan.model.boxes.all fun modelBox => canonicalBox modelBox == modelBox) &&
  plan.model.summaries == sortBy plan.model.summaries (·.name) &&
  allDistinctWords plan.identity.transitions &&
  match plan.linkedProvenance with
  | none => false
  | some provenance =>
      provenance.sourceHash.algorithm == Plan.hashAlgorithm &&
      provenance.sourceHash.domain == Plan.sourceArtifactDomain &&
      provenance.linker.semantics == Plan.linkerSemantics &&
      provenance.linker.sourceSchema == Plan.compositionSourceSchema &&
      provenance.linker.planSchema == Plan.planSchema &&
      provenance.linker.identityScheme == Plan.stableIdentityScheme &&
      provenance.linker.canonicalEncoding == Plan.canonicalEncoding &&
      provenance.linker.sourceMapSchema == Plan.sourceMapSchema &&
      provenance.sourceMap.schemaVersion == Plan.sourceMapSchema &&
      provenance.sourceMap.boundary.isEmpty && provenance.sourceMap.hidden.isEmpty &&
      provenance.sourceMap.leaves == sortBy provenance.sourceMap.leaves (·.occurrence) &&
      provenance.sourceMap.leaves.map (·.occurrence) == expectedLeaves.map (·.occurrence)

/-- Canonical product linker for composition source V1. -/
def linkV1 (source : CompositionSourceV1) (sourceCanonicalBytes : String) :
    Except (List LinkErrorV1) LinkResultV1 := do
  let firstErrors := envelopeErrors source ++ constructErrors source
  if !firstErrors.isEmpty then fail firstErrors
  let collectionErrors := duplicateDefinitionErrors source.definitions ++
    missingDefinitionErrors source
  if !collectionErrors.isEmpty then fail collectionErrors
  let recursionErrors := cycleErrors source
  if !recursionErrors.isEmpty then fail recursionErrors
  let root ← match definition? source.definitions source.rootDefinition with
    | some root => pure root
    | none => fail [mkError .missingDefinition source.rootDefinition
        s!"root definition '{source.rootDefinition.raw}' does not exist"]
  let rootBindings := root.parameterRequirements.map fun requirement =>
    { requirement, parameter := requirement : ParameterBinding }
  let rootRequirementErrors := root.parameterRequirements.filterMap fun requirement =>
    if source.parameters.any fun parameter => parameter.name == requirement then none
    else some (mkError .unboundParameter root.id
      s!"root requirement '{requirement}' has no same-named model parameter")
  let parameterErrors := rootRequirementErrors ++
    reachableBindingErrors source root rootBindings
  if !parameterErrors.isEmpty then fail parameterErrors
  let leaves := expandDefinition source.definitions root [] [] rootBindings
  let typeCheckModel : IR.Model := {
    name := idSlug "model:" source.modelId
    dt := source.outerDt
    params := source.parameters
    boxes := leaves.map (·.modelBox)
    wires := []
    summaries := [] }
  let bindingTypeErrors := (leaves.map (leafParameterTypeErrors typeCheckModel)).join
  if !bindingTypeErrors.isEmpty then fail bindingTypeErrors
  let (wireResolutionErrors, expandedWires) := expandWires source.definitions root []
  if !wireResolutionErrors.isEmpty then fail wireResolutionErrors
  let reservedWireErrors := reservedWireIdentityErrors expandedWires
  if !reservedWireErrors.isEmpty then fail reservedWireErrors
  let driverErrors := multipleDriverErrors expandedWires
  if !driverErrors.isEmpty then fail driverErrors
  let identityLeaves := sortBy (leaves.map fun leaf =>
    { box := leaf.modelBox.name, occurrence := leaf.occurrence : Plan.LeafIdentityV1 }) (·.box)
  let transitions := sortBy ((leaves.map fun leaf => leaf.modelBox.transitions.map fun transition =>
    let identity := leaf.occurrence ++ "#" ++ transition.name
    { box := leaf.modelBox.name
      name := transition.name
      identity
      ruleWord := Hash.ruleWord identity : Plan.TransitionIdentityV1 }).join) (·.identity)
  let wordErrors := identityErrors transitions
  if !wordErrors.isEmpty then fail wordErrors
  let (summaryErrors, summaries) := resolveSummaries source leaves
  if !summaryErrors.isEmpty then fail summaryErrors
  let orderedExpandedWires := sortBy expandedWires fun wire =>
    (expandedMailbox wire).identity
  let model : IR.Model := {
    name := idSlug "model:" source.modelId
    dt := source.outerDt
    params := sortBy source.parameters (·.name)
    boxes := sortBy (leaves.map (·.modelBox)) (·.name)
    wires := orderedExpandedWires.map (·.modelWire)
    summaries := sortBy summaries (·.name) }
  let mailboxes := orderedExpandedWires.map expandedMailbox
  let sourceDigest := Hash.hashRecord Plan.sourceArtifactDomain sourceCanonicalBytes.toUTF8
  let sourceMap : SourceMapV1 := {
    schemaVersion := Plan.sourceMapSchema
    leaves := sortBy (leaves.map fun leaf => {
      occurrence := leaf.occurrence
      definition := leaf.definition.raw
      instancePath := leaf.instancePath.map (·.raw)
      displayPath := leaf.modelBox.name }) (·.occurrence)
    boundary := []
    hidden := [] }
  let plan : Plan.ExecutablePlanV1 := {
    schemaVersion := Plan.planSchema
    identityScheme := Plan.stableIdentityScheme
    origin := .linked
    model
    identity := {
      modelId := source.modelId.raw
      enabledFeatures := []
      schedulerDomains := [{
        id := Plan.globalSchedulerDomain
        algorithm := Plan.tauLeapAlgorithm
        leaves := identityLeaves.map (·.box) }]
      leaves := identityLeaves
      transitions
      mailboxes }
    linkedProvenance := some {
      sourceHash := {
        algorithm := sourceDigest.algorithm
        domain := sourceDigest.domain
        digest := sourceDigest.digest }
      linker := {
        semantics := Plan.linkerSemantics
        sourceSchema := Plan.compositionSourceSchema
        planSchema := Plan.planSchema
        identityScheme := Plan.stableIdentityScheme
        canonicalEncoding := Plan.canonicalEncoding
        sourceMapSchema := Plan.sourceMapSchema }
      sourceMap } }
  if planValidCheck plan then
    pure {
      plan
      report := {
        warnings := if source.definitions.any fun definition =>
          match definition.body with
          | .primitive _ => false
          | .composite _ => !definition.parameterRequirements.isEmpty
        then ["composite parameter requirements are forwarded through the shared model-level V1 namespace"]
        else []
        statistics := {
          leaves := identityLeaves.length
          transitions := transitions.length
          mailboxes := mailboxes.length } } }
  else
    fail [mkError .unsupportedConstruct source.modelId
      "linked model failed the executable-plan V1 validity check"]

end Sembla.Composition
