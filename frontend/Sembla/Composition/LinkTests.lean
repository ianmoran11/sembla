import Sembla.Composition.Fixtures
import Sembla.Composition.Link

namespace Sembla.Composition.LinkTests

open Sembla
open Sembla.Composition.Fixtures

private def sid (raw : String) : StableId := ⟨raw⟩

private def linkErrors (source : CompositionSourceV1) : List LinkErrorV1 :=
  match linkV1 source (Json.render source) with
  | .ok _ => []
  | .error errors => errors

private def codes (source : CompositionSourceV1) : List LinkErrorCodeV1 :=
  (linkErrors source).map (·.code)

private def linkedPlan? (source : CompositionSourceV1) : Option Plan.ExecutablePlanV1 :=
  match linkV1 source (Json.render source) with
  | .error _ => none
  | .ok result => some result.plan

private def modifyRootComposite
    (source : CompositionSourceV1) (transform : CompositeBodyV1 → CompositeBodyV1) :
    CompositionSourceV1 :=
  { source with definitions := source.definitions.map fun definition =>
      if definition.id == source.rootDefinition then
        match definition.body with
        | .primitive _ => definition
        | .composite body => { definition with body := .composite (transform body) }
      else definition }

private def modifyRootInstances
    (source : CompositionSourceV1) (transform : InstanceDeclV1 → InstanceDeclV1) :
    CompositionSourceV1 :=
  modifyRootComposite source fun body =>
    { body with instances := body.instances.map transform }

private def duplicateDefinition : CompositionSourceV1 :=
  match soloPopulation.definitions with
  | [] => soloPopulation
  | definition :: _ =>
      { soloPopulation with definitions := soloPopulation.definitions ++ [definition] }

#guard codes duplicateDefinition == [.duplicateStableId]

private def missingDefinition : CompositionSourceV1 :=
  { soloPopulation with rootDefinition := sid "def:missing" }

#guard codes missingDefinition == [.missingDefinition]

private def missingReferencedDefinition : CompositionSourceV1 :=
  modifyRootInstances soloPopulation fun item =>
    if item.id.raw == "inst:population" then
      { item with definition := sid "def:missing" }
    else item

#guard codes missingReferencedDefinition == [.missingDefinition]

private def cycleDefinition (name target itemName : String) : ComponentDefinitionV1 := {
  id := sid ("def:" ++ name)
  displayName := name
  parameterRequirements := []
  ports := []
  body := .composite {
    instances := [{
      id := sid ("inst:" ++ itemName)
      displayName := itemName
      definition := sid ("def:" ++ target)
      parameterBindings := [] }]
    «wires» := []
    exposures := []
    hiddenPorts := [] } }

private def cyclicSource : CompositionSourceV1 :=
  { soloPopulation with
    definitions := [
      cycleDefinition "cycle_a" "cycle_b" "to_b",
      cycleDefinition "cycle_b" "cycle_a" "to_a"]
    rootDefinition := sid "def:cycle_a" }

#guard codes cyclicSource == [.recursiveDefinition]

private def twoCycleRoot (reverse : Bool) : ComponentDefinitionV1 :=
  let items : List InstanceDeclV1 := [{
      id := sid "inst:first_cycle"
      displayName := "First cycle"
      definition := sid "def:cycle_a"
      parameterBindings := [] }, {
      id := sid "inst:second_cycle"
      displayName := "Second cycle"
      definition := sid "def:cycle_c"
      parameterBindings := [] }]
  { id := sid "def:two_cycles"
    displayName := "Two cycles"
    parameterRequirements := []
    ports := []
    body := .composite {
      instances := if reverse then items.reverse else items
      «wires» := []
      exposures := []
      hiddenPorts := [] } }

private def twoCycleSource (reverse : Bool) : CompositionSourceV1 :=
  { soloPopulation with
    definitions := [
      twoCycleRoot reverse,
      cycleDefinition "cycle_a" "cycle_b" "to_b",
      cycleDefinition "cycle_b" "cycle_a" "to_a",
      cycleDefinition "cycle_c" "cycle_d" "to_d",
      cycleDefinition "cycle_d" "cycle_c" "to_c"]
    rootDefinition := sid "def:two_cycles" }

#guard codes (twoCycleSource false) == [.recursiveDefinition, .recursiveDefinition]
#guard linkErrors (twoCycleSource false) == linkErrors (twoCycleSource true)

private def unboundParameter : CompositionSourceV1 :=
  modifyRootInstances soloPopulation fun item =>
    if item.id.raw == "inst:population" then
      { item with parameterBindings := item.parameterBindings.filter (·.requirement != "gamma") }
    else item

#guard codes unboundParameter == [.unboundParameter]

private def duplicateBinding : CompositionSourceV1 :=
  modifyRootInstances soloPopulation fun item =>
    if item.id.raw == "inst:population" then
      { item with parameterBindings := item.parameterBindings ++ [{
          requirement := "beta", «parameter» := "beta" }] }
    else item

#guard codes duplicateBinding == [.ambiguousParameterBinding]

private def unknownBindingTarget : CompositionSourceV1 :=
  modifyRootInstances soloPopulation fun item =>
    if item.id.raw == "inst:population" then
      { item with parameterBindings := item.parameterBindings.map fun binding =>
          if binding.requirement == "gamma" then { binding with «parameter» := "missing_gamma" }
          else binding }
    else item

#guard codes unknownBindingTarget == [.unboundParameter]

private def unknownRequirementBinding : CompositionSourceV1 :=
  modifyRootInstances soloPopulation fun item =>
    if item.id.raw == "inst:population" then
      { item with parameterBindings := item.parameterBindings ++ [{
          requirement := "delta", «parameter» := "beta" }] }
    else item

#guard codes unknownRequirementBinding == [.ambiguousParameterBinding]

private def missingLeafSummary : CompositionSourceV1 :=
  { soloPopulation with «summaries» := [{
      name := "missing_leaf"
      «reduce» := .max
      instancePath := [sid "inst:missing"]
      «view» := "I" }] }

#guard codes missingLeafSummary == [.invalidSummary]

private def missingViewSummary : CompositionSourceV1 :=
  { soloPopulation with «summaries» := [{
      name := "missing_view"
      «reduce» := .max
      instancePath := [sid "inst:population"]
      «view» := "missing" }] }

#guard codes missingViewSummary == [.invalidSummary]

private def sourceWithWire : CompositionSourceV1 :=
  modifyRootComposite independentEpidemicPolicy fun body =>
    { body with «wires» := [{
        id := sid "wire:test"
        sourceInstance := sid "inst:population"
        sourcePort := sid "port:infection_count"
        targetInstance := sid "inst:policy"
        targetPort := sid "port:infection_count"
        delayTicks := 1 }] }

#guard codes sourceWithWire == [.unsupportedConstruct]

private def wireErrorNamesPrd0008 : Bool :=
  match linkV1 sourceWithWire (Json.render sourceWithWire) with
  | .ok _ => false
  | .error [error] => error.message ==
      "wire 'wire:test' is unsupported by product linking; PRD 0008 adds wires"
  | .error _ => false

#guard wireErrorNamesPrd0008

private def sourceWithExposure : CompositionSourceV1 :=
  modifyRootComposite independentEpidemicPolicy fun body =>
    { body with exposures := [{
        id := sid "expose:test"
        innerInstance := sid "inst:population"
        innerPort := sid "port:infection_count"
        outerPort := sid "port:test" }] }

#guard codes sourceWithExposure == [.unsupportedConstruct]

private def exposureErrorNamesPrd0009 : Bool :=
  match linkV1 sourceWithExposure (Json.render sourceWithExposure) with
  | .ok _ => false
  | .error [error] => error.message ==
      "exposure 'expose:test' is unsupported by product linking; PRD 0009 adds exposures"
  | .error _ => false

#guard exposureErrorNamesPrd0009

private def sourceWithHiddenPort : CompositionSourceV1 :=
  modifyRootComposite independentEpidemicPolicy fun body =>
    { body with hiddenPorts := [{
        instance_ := sid "inst:population"
        port := sid "port:infection_count" }] }

#guard codes sourceWithHiddenPort == [.unsupportedConstruct]

private def sortedIndependentErrors : CompositionSourceV1 :=
  { sourceWithWire with
    schemaVersion := "sembla.composition-source/v2"
    requiredFeatures := ["future_feature"] }

#guard codes sortedIndependentErrors == [
  .unknownVersion, .unsupportedFeature, .unsupportedConstruct]

private def primitiveRoot : CompositionSourceV1 :=
  { soloPopulation with rootDefinition := sid "def:population" }

#guard codes primitiveRoot == [.unsupportedConstruct]

private def modifyPopulationPrimitive
    (source : CompositionSourceV1) (transform : PrimitiveBodyV1 → PrimitiveBodyV1) :
    CompositionSourceV1 :=
  { source with definitions := source.definitions.map fun definition =>
      if definition.id.raw == "def:population" then
        match definition.body with
        | .composite _ => definition
        | .primitive body => { definition with body := .primitive (transform body) }
      else definition }

private def overflowingOuterDt : CompositionSourceV1 :=
  { soloPopulation with outerDt := ⟨1, 400⟩ }

private def underflowingOuterDt : CompositionSourceV1 :=
  { soloPopulation with outerDt := ⟨1, -400⟩ }

private def overflowingParameterDefault : CompositionSourceV1 :=
  { soloPopulation with parameters := soloPopulation.parameters.map fun paramDecl =>
      if paramDecl.name == "beta" then
        { paramDecl with default := .real ⟨1, 400⟩ }
      else paramDecl }

private def overflowingPriorArgument : CompositionSourceV1 :=
  { soloPopulation with parameters := soloPopulation.parameters.map fun paramDecl =>
      if paramDecl.name == "beta" then
        { paramDecl with «prior» := some { family := .normal, args := [⟨1, 400⟩, 1.0] } }
      else paramDecl }

private def overflowingExpressionLiteral : CompositionSourceV1 :=
  modifyPopulationPrimitive soloPopulation fun body =>
    { body with «transitions» := body.transitions.map fun item =>
        if item.name == "infect" then
          { item with «hazard» := .real ⟨1, 400⟩ }
        else item }

private def noncanonicalScientific : IR.Scientific :=
  ⟨10000000000000001, -17⟩

private def noncanonicalOuterDt : CompositionSourceV1 :=
  { soloPopulation with outerDt := noncanonicalScientific }

private def roundedIntegerOuterDt : CompositionSourceV1 :=
  { soloPopulation with outerDt := ⟨9007199254740993, 0⟩ }

private def noncanonicalParameterDefault : CompositionSourceV1 :=
  { soloPopulation with parameters := soloPopulation.parameters.map fun paramDecl =>
      if paramDecl.name == "beta" then
        { paramDecl with default := .real noncanonicalScientific }
      else paramDecl }

private def noncanonicalPriorArgument : CompositionSourceV1 :=
  { soloPopulation with parameters := soloPopulation.parameters.map fun paramDecl =>
      if paramDecl.name == "beta" then
        { paramDecl with «prior» := some {
            family := .normal, args := [noncanonicalScientific, 1.0] } }
      else paramDecl }

private def noncanonicalExpressionLiteral : CompositionSourceV1 :=
  modifyPopulationPrimitive soloPopulation fun body =>
    { body with «transitions» := body.transitions.map fun item =>
        if item.name == "infect" then
          { item with «hazard» := .real noncanonicalScientific }
        else item }

private def gammaAsInteger (source : CompositionSourceV1) : CompositionSourceV1 :=
  { source with parameters := source.parameters.map fun paramDecl =>
      if paramDecl.name == "gamma" then
        { paramDecl with ty := .int, default := .int 1, «prior» := none }
      else paramDecl }

private def mismatchedParameterType : CompositionSourceV1 :=
  gammaAsInteger soloPopulation

private def repeatedMismatchedParameterType : CompositionSourceV1 :=
  gammaAsInteger twoIndependentRegions

#guard codes overflowingOuterDt == [.unsupportedConstruct]
#guard codes underflowingOuterDt == [.unsupportedConstruct]
#guard codes overflowingParameterDefault == [.unsupportedConstruct]
#guard codes overflowingPriorArgument == [.unsupportedConstruct]
#guard codes overflowingExpressionLiteral == [.unsupportedConstruct]
#guard codes noncanonicalOuterDt == [.unsupportedConstruct]
#guard codes roundedIntegerOuterDt == [.unsupportedConstruct]
#guard codes noncanonicalParameterDefault == [.unsupportedConstruct]
#guard codes noncanonicalPriorArgument == [.unsupportedConstruct]
#guard codes noncanonicalExpressionLiteral == [.unsupportedConstruct]
#guard codes mismatchedParameterType == [.ambiguousParameterBinding]
#guard codes repeatedMismatchedParameterType == [
  .ambiguousParameterBinding, .ambiguousParameterBinding]
#guard linkErrors repeatedMismatchedParameterType == linkErrors {
  repeatedMismatchedParameterType with
  definitions := repeatedMismatchedParameterType.definitions.reverse }
#guard (linkedPlan? { soloPopulation with outerDt := ⟨1, 308⟩ }).isSome
#guard (linkedPlan? { soloPopulation with
  outerDt := ⟨17976931348623157, 292⟩ }).isSome
#guard (linkedPlan? { soloPopulation with outerDt := ⟨5, -324⟩ }).isSome

private def malformedTransition : CompositionSourceV1 :=
  { soloPopulation with definitions := soloPopulation.definitions.map fun definition =>
      if definition.id.raw == "def:population" then
        match definition.body with
        | .composite _ => definition
        | .primitive body => { definition with body := .primitive {
            body with «transitions» := body.transitions.map fun item =>
              if item.name == "infect" then { item with name := "bad-name" } else item } }
      else definition }

#guard codes malformedTransition == [.unsupportedConstruct]

private def unknownEffectAttribute : CompositionSourceV1 :=
  { soloPopulation with definitions := soloPopulation.definitions.map fun definition =>
      if definition.id.raw == "def:population" then
        match definition.body with
        | .composite _ => definition
        | .primitive body => { definition with body := .primitive {
            body with «transitions» := body.transitions.map fun item =>
              if item.name == "infect" then { item with effects := item.effects.map fun effect =>
                match effect with | .setAttr _ value => .setAttr "missing" value }
              else item } }
      else definition }

#guard codes unknownEffectAttribute == [.unsupportedConstruct]

private def deterministic : Bool :=
  match linkedPlan? independentEpidemicPolicy, linkedPlan? independentEpidemicPolicy with
  | some first, some second => PlanJson.renderPlan first == PlanJson.renderPlan second
  | _, _ => false

#guard deterministic

/-- The binding README makes semantic permutation laws byte-equality of plan
    cores. Full envelopes intentionally differ because source arrays preserve
    author order and provenance hashes those exact canonical source bytes. -/
private def definitionOrderInsensitive : Bool :=
  let reversed := { independentEpidemicPolicy with
    definitions := independentEpidemicPolicy.definitions.reverse }
  let originalBytes := Json.render independentEpidemicPolicy
  let reversedBytes := Json.render reversed
  match linkV1 independentEpidemicPolicy originalBytes, linkV1 reversed reversedBytes with
  | .ok first, .ok second =>
      let firstCore := (PlanJson.semanticPayloadToCJson first.plan).render
      let secondCore := (PlanJson.semanticPayloadToCJson second.plan).render
      match first.plan.linkedProvenance, second.plan.linkedProvenance with
      | some firstProvenance, some secondProvenance =>
          firstCore == secondCore && originalBytes != reversedBytes &&
          firstProvenance.sourceHash.digest ==
            (Hash.hashRecord Plan.sourceArtifactDomain originalBytes.toUTF8).digest &&
          secondProvenance.sourceHash.digest ==
            (Hash.hashRecord Plan.sourceArtifactDomain reversedBytes.toUTF8).digest &&
          firstProvenance.sourceHash.digest != secondProvenance.sourceHash.digest
      | _, _ => false
  | _, _ => false

#guard definitionOrderInsensitive

mutual
  private partial def expressionContainsParameter (expected : String) : IR.Expr → Bool
    | .real _ | .int _ | .bool _ | .enum _ | .selfAttr _ | .enumIs _ _ => false
    | .param name => name == expected
    | .add lhs rhs | .sub lhs rhs | .mul lhs rhs | .div lhs rhs
    | .eq lhs rhs | .ne lhs rhs | .lt lhs rhs | .le lhs rhs
    | .gt lhs rhs | .ge lhs rhs | .and lhs rhs | .or lhs rhs =>
        expressionContainsParameter expected lhs || expressionContainsParameter expected rhs
    | .not expression => expressionContainsParameter expected expression
    | .input _ aggregate => aggregateContainsParameter expected aggregate
    | .agg operation _ _ _ filter =>
        aggOpContainsParameter expected operation || expressionContainsParameter expected filter

  private partial def aggOpContainsParameter (expected : String) : IR.AggOp → Bool
    | .count => false
    | .sum value => expressionContainsParameter expected value

  private partial def aggregateContainsParameter (expected : String) : IR.Aggregate → Bool
    | .mk operation filter =>
        aggOpContainsParameter expected operation || match filter with
        | none => false
        | some expression => expressionContainsParameter expected expression
end

private def renamedNestedBindings : CompositionSourceV1 :=
  let regional := modifyRootInstances twoIndependentRegions fun item =>
    let scopePrefix := if item.id.raw == "inst:north" then "north_" else "south_"
    { item with parameterBindings := item.parameterBindings.map fun binding =>
        { binding with «parameter» := scopePrefix ++ binding.requirement } }
  let regionalParameters := (twoIndependentRegions.parameters.map fun paramDecl => [
    { paramDecl with name := "north_" ++ paramDecl.name },
    { paramDecl with name := "south_" ++ paramDecl.name }]).join
  { regional with parameters := regional.parameters ++ regionalParameters }

private def transitionUses
    (plan : Plan.ExecutablePlanV1) (boxName transitionName parameterName : String) : Bool :=
  match plan.model.boxes.find? fun modelBox => modelBox.name == boxName with
  | none => false
  | some modelBox =>
      match modelBox.transitions.find? fun item => item.name == transitionName with
      | none => false
      | some item => expressionContainsParameter parameterName item.hazard

private def nestedBindingsForward : Bool :=
  match linkedPlan? renamedNestedBindings with
  | none => false
  | some plan =>
      transitionUses plan "north/population" "infect" "north_beta" &&
      transitionUses plan "north/population" "recover" "north_gamma" &&
      transitionUses plan "south/population" "infect" "south_beta" &&
      transitionUses plan "south/population" "recover" "south_gamma"

#guard nestedBindingsForward

private def repeatedCompositeDistinct : Bool :=
  match linkedPlan? twoIndependentRegions with
  | none => false
  | some plan =>
      let occurrences := plan.identity.leaves.map (·.occurrence)
      let words := plan.identity.transitions.map (·.ruleWord)
      occurrences.length == 4 && occurrences.eraseDups.length == 4 &&
        words.length == 8 && words.eraseDups.length == 8

#guard repeatedCompositeDistinct

private def repeatedCompositeWordsPinned : Bool :=
  match linkedPlan? twoIndependentRegions with
  | none => false
  | some plan => (plan.identity.transitions.map (fun item =>
      (item.identity, item.ruleWord.toNat))) == [
        ("occ:north/policy#reopen", 1371785137),
        ("occ:north/policy#restrict", 1934331310),
        ("occ:north/population#infect", 1000729077),
        ("occ:north/population#recover", 3023508040),
        ("occ:south/policy#reopen", 3107531662),
        ("occ:south/policy#restrict", 1868310822),
        ("occ:south/population#infect", 3844621682),
        ("occ:south/population#recover", 4161204503)]

#guard repeatedCompositeWordsPinned

end Sembla.Composition.LinkTests
