import Sembla.Semantics.Raw

/-!
Computed raw fixtures for the complete PRD 0002 inventory. These are deliberately
serialization-layer values: they exercise constructor and field coverage without
asserting that every raw value passes a later contextual checker.
-/
namespace Sembla.Semantics.RawTests

open Sembla
open Sembla.Semantics.Raw

private def scientific (coefficient exponent : Int) : IR.Scientific :=
  ⟨coefficient, exponent⟩

private def sid (raw : String) : Composition.StableId := ⟨raw⟩

private def truth : IR.Expr := .bool true
private def one : IR.Expr := .int 1
private def two : IR.Expr := .real (scientific 2 0)

/-- Every raw parameter/prior family, including the prior-free integer case. -/
def parameterFixtures : List IR.ParamDecl :=
  [ { name := "normal_rate"
      ty := .real
      default := .real (scientific 1 (-1))
      prior := some { family := .normal, args := [scientific 0 0, scientific 1 0] } }
  , { name := "lognormal_rate"
      ty := .real
      default := .real (scientific 2 (-1))
      prior := some { family := .logNormal, args := [scientific 0 0, scientific 5 (-1)] } }
  , { name := "uniform_rate"
      ty := .real
      default := .real (scientific 3 (-1))
      prior := some { family := .uniform, args := [scientific 0 0, scientific 1 0] } }
  , { name := "seed"
      ty := .int
      default := .int 7
      prior := none } ]

def attributeFixtures : List IR.Attr :=
  [ { name := "mass", ty := .real }
  , { name := "age", ty := .int }
  , { name := "status", ty := .enum ["susceptible", "infected"] }
  , { name := "region", ty := .ref "regions" } ]

/-- Every expression constructor. The final two entries contain nested aggregate
    syntax, including an aggregate inside an input aggregate's sum expression. -/
def expressionVariants : List IR.Expr :=
  [ .real (scientific 1 (-2))
  , .int 2
  , .bool true
  , .enum "infected"
  , .param "normal_rate"
  , .selfAttr "age"
  , .add one two
  , .sub two one
  , .mul one two
  , .div two one
  , .eq one two
  , .ne one two
  , .lt one two
  , .le one two
  , .gt two one
  , .ge two one
  , .and truth (.not (.bool false))
  , .or truth (.bool false)
  , .not truth
  , .enumIs "status" "infected"
  , .input "incoming"
      (.mk (.sum (.agg .count "people" "region" "region" truth)) (some truth))
  , .agg (.sum (.selfAttr "age")) "people" "region" "region" truth ]

def aggregateVariants : List IR.Aggregate :=
  [ .mk .count none
  , .mk (.sum (.selfAttr "age")) (some (.enumIs "status" "infected")) ]

def claimOrderingVariants : List IR.ClaimOrdering :=
  [.raceTime, .key (.selfAttr "age")]

private def resourceClaims : List IR.ResourceClaim :=
  [ { resource := .selfAttr "region", ordering := .raceTime }
  , { resource := .selfAttr "age", ordering := .key (.selfAttr "age") } ]

private def transitionFixture : IR.Transition :=
  { name := "infect"
    table := "people"
    guard := .enumIs "status" "susceptible"
    hazard := .mul (.param "normal_rate")
      (.agg .count "people" "region" "region" (.enumIs "status" "infected"))
    effects := [.setAttr "status" (.enum "infected")]
    contests := resourceClaims }

private def inputPortFixture : IR.PortDecl :=
  { name := "incoming", schema := attributeFixtures }

private def outputFields : List IR.OutputField :=
  [ { name := "population", op := .count, filter := none }
  , { name := "total_age", op := .sum (.selfAttr "age"),
      filter := some (.enumIs "status" "infected") } ]

private def outputFixture : IR.OutputDecl :=
  { name := "counts"
    schema := [{ name := "population", ty := .int }, { name := "total_age", ty := .int }]
    builder := .perTable "people" outputFields }

private def viewFixtures : List IR.ViewDecl :=
  [ { name := "sum_age", table := "people", filter := none,
      value := some (.selfAttr "age"), reduce := .sum }
  , { name := "count_people", table := "people", filter := some truth,
      value := none, reduce := .count }
  , { name := "min_age", table := "people", filter := none,
      value := some (.selfAttr "age"), reduce := .min }
  , { name := "max_age", table := "people", filter := none,
      value := some (.selfAttr "age"), reduce := .max } ]

private def groupKeys : List IR.GroupKey :=
  [ { attr := "status", bandWidth := none }
  , { attr := "age", bandWidth := some 10 } ]

private def groupedViewFixture : IR.GroupedViewDecl :=
  { name := "people_by_status_and_age"
    table := "people"
    filter := some truth
    keys := groupKeys }

private def peopleTable : IR.Table :=
  { name := "people", sizeHint := 64, attrs := attributeFixtures }

private def regionsTable : IR.Table :=
  { name := "regions", sizeHint := 4, attrs := [{ name := "weight", ty := .real }] }

private def boxFixture : IR.Box :=
  { name := "population"
    tables := [peopleTable, regionsTable]
    transitions := [transitionFixture]
    inputs := [inputPortFixture]
    outputs := [outputFixture]
    views := viewFixtures
    groupedViews := [groupedViewFixture] }

private def modelWire : IR.Wire :=
  { source := { box := "population", port := "counts" }
    target := { box := "population", port := "incoming" } }

private def summaryFixtures : List IR.SummaryDecl :=
  [ { name := "sum_age", box := "population", view := "sum_age", reduce := .sum }
  , { name := "min_age", box := "population", view := "min_age", reduce := .min }
  , { name := "max_age", box := "population", view := "max_age", reduce := .max }
  , { name := "last_count", box := "population", view := "count_people", reduce := .last }
  , { name := "peak_tick", box := "population", view := "count_people", reduce := .argmaxTick } ]

def rawModelFixture : IR.Model :=
  { name := "raw_coverage"
    dt := scientific 1 (-1)
    params := parameterFixtures
    boxes := [boxFixture]
    wires := [modelWire]
    summaries := summaryFixtures }

private def compositionInputPort : Composition.PortDeclV1 :=
  { id := sid "port:incoming"
    displayName := "Incoming"
    direction := .input
    schema := attributeFixtures }

private def compositionOutputPort : Composition.PortDeclV1 :=
  { id := sid "port:counts"
    displayName := "Counts"
    direction := .output
    schema := outputFixture.schema }

private def bindingFixture : Composition.ParameterBinding :=
  { requirement := "rate", parameter := "normal_rate" }

private def instanceFixture : Composition.InstanceDeclV1 :=
  { id := sid "instance:population"
    displayName := "Population instance"
    definition := sid "def:primitive"
    parameterBindings := [bindingFixture] }

private def compositionWireFixture : Composition.WireDeclV1 :=
  { id := sid "wire:feedback"
    sourceInstance := sid "instance:population"
    sourcePort := sid "port:counts"
    targetInstance := sid "instance:population"
    targetPort := sid "port:incoming"
    delayTicks := 1 }

private def exposureFixture : Composition.ExposureDeclV1 :=
  { id := sid "exposure:counts"
    innerInstance := sid "instance:population"
    innerPort := sid "port:counts"
    outerPort := sid "port:counts" }

private def hiddenPortFixture : Composition.HiddenPortV1 :=
  { instance_ := sid "instance:population", port := sid "port:incoming" }

private def primitiveBodyFixture : Composition.PrimitiveBodyV1 :=
  { tables := boxFixture.tables
    transitions := boxFixture.transitions
    inputs := boxFixture.inputs
    outputs := boxFixture.outputs
    views := boxFixture.views }

private def compositeBodyFixture : Composition.CompositeBodyV1 :=
  { instances := [instanceFixture]
    wires := [compositionWireFixture]
    exposures := [exposureFixture]
    hiddenPorts := [hiddenPortFixture] }

private def primitiveDefinition : Composition.ComponentDefinitionV1 :=
  { id := sid "def:primitive"
    displayName := "Primitive"
    parameterRequirements := ["rate"]
    ports := [compositionInputPort, compositionOutputPort]
    body := .primitive primitiveBodyFixture }

private def compositeDefinition : Composition.ComponentDefinitionV1 :=
  { id := sid "def:root"
    displayName := "Root composite"
    parameterRequirements := []
    ports := [compositionOutputPort]
    body := .composite compositeBodyFixture }

private def sourceSummaryFixture : Composition.SourceSummaryV1 :=
  { name := "population_peak"
    reduce := .argmaxTick
    instancePath := [sid "instance:population"]
    view := "count_people" }

def compositionSourceFixture : Composition.CompositionSourceV1 :=
  { schemaVersion := Plan.compositionSourceSchema
    modelId := sid "model:raw-coverage"
    displayName := "Raw coverage source"
    outerDt := scientific 1 (-1)
    parameters := parameterFixtures
    definitions := [primitiveDefinition, compositeDefinition]
    rootDefinition := sid "def:root"
    requiredFeatures := []
    summaries := [sourceSummaryFixture] }

private def sourceMapLeaf : Composition.SourceMapLeafV1 :=
  { occurrence := "leaf:population"
    definition := "def:primitive"
    instancePath := ["instance:population"]
    displayPath := "Root composite / Population instance" }

private def sourceMapBoundary : Composition.SourceMapBoundaryV1 :=
  { outer := "port:counts"
    leaf := "leaf:population"
    port := "port:counts"
    path := ["instance:population"] }

private def sourceMapHidden : Composition.SourceMapHiddenV1 :=
  { instance_ := "instance:population", port := "port:incoming" }

def sourceMapFixture : Composition.SourceMapV1 :=
  { schemaVersion := Plan.sourceMapSchema
    leaves := [sourceMapLeaf]
    boundary := [sourceMapBoundary]
    hidden := [sourceMapHidden] }

private def hashFixture : Plan.HashRecordV1 :=
  { algorithm := Plan.hashAlgorithm
    domain := Plan.sourceArtifactDomain
    digest := "0000000000000000000000000000000000000000000000000000000000000000" }

private def linkerFixture : Plan.LinkerDescriptorV1 :=
  { semantics := Plan.linkerSemantics
    sourceSchema := Plan.compositionSourceSchema
    planSchema := Plan.planSchema
    identityScheme := Plan.stableIdentityScheme
    canonicalEncoding := Plan.canonicalEncoding
    sourceMapSchema := Plan.sourceMapSchema }

private def linkedProvenanceFixture : Plan.LinkedProvenanceV1 :=
  { sourceHash := hashFixture, linker := linkerFixture, sourceMap := sourceMapFixture }

private def schedulerFixture : Plan.SchedulerDomainV1 :=
  { id := Plan.globalSchedulerDomain
    algorithm := Plan.tauLeapAlgorithm
    leaves := ["leaf:population"] }

private def leafIdentityFixture : Plan.LeafIdentityV1 :=
  { box := "population", occurrence := "leaf:population" }

private def transitionIdentityFixture : Plan.TransitionIdentityV1 :=
  { box := "population"
    name := "infect"
    identity := "transition:population/infect"
    ruleWord := 23 }

private def mailboxIdentityFixture : Plan.MailboxIdentityV1 :=
  { identity := "mailbox:feedback"
    sourceBox := "population"
    sourcePort := "counts"
    targetBox := "population"
    targetPort := "incoming" }

private def identityFixture : Plan.IdentityMapV1 :=
  { modelId := "model:raw-coverage"
    enabledFeatures := ["feature:stable-identities"]
    schedulerDomains := [schedulerFixture]
    leaves := [leafIdentityFixture]
    transitions := [transitionIdentityFixture]
    mailboxes := [mailboxIdentityFixture] }

def linkedPlanFixture : Plan.ExecutablePlanV1 :=
  { schemaVersion := Plan.planSchema
    identityScheme := Plan.stableIdentityScheme
    origin := .linked
    model := rawModelFixture
    identity := identityFixture
    linkedProvenance := some linkedProvenanceFixture }

def directPlanFixture : Plan.ExecutablePlanV1 :=
  { schemaVersion := Plan.planSchema
    identityScheme := Plan.stableIdentityScheme
    origin := .directStable
    model := rawModelFixture
    identity := identityFixture
    linkedProvenance := none }

/-- Every inductive constructor in the four-module raw boundary. -/
def constructorCoverage : List ItemCoverage :=
  [IR.ParamType.real, .int].map classifyParamTypeConstructor ++
  [IR.ParamValue.real (scientific 1 0), .int 1].map classifyParamValueConstructor ++
  [IR.PriorFamily.normal, .logNormal, .uniform].map classifyPriorFamilyConstructor ++
  [IR.AttrType.real, .int, .enum ["x"], .ref "people"].map classifyAttrTypeConstructor ++
  expressionVariants.map classifyExprConstructor ++
  [IR.AggOp.count, .sum one].map classifyAggOpConstructor ++
  [IR.Aggregate.mk .count none].map classifyAggregateConstructor ++
  [IR.Effect.setAttr "age" one].map classifyEffectConstructor ++
  claimOrderingVariants.map classifyClaimOrderingConstructor ++
  [IR.OutputBuilder.perTable "people" outputFields].map classifyOutputBuilderConstructor ++
  [IR.ViewReduce.sum, .count, .min, .max].map classifyViewReduceConstructor ++
  [IR.SummaryReduce.sum, .min, .max, .last, .argmaxTick].map classifySummaryReduceConstructor ++
  [Composition.PortDirection.input, .output].map classifyPortDirectionConstructor ++
  [Composition.ComponentBodyV1.primitive primitiveBodyFixture,
    .composite compositeBodyFixture].map classifyComponentBodyConstructor ++
  [Plan.PlanOrigin.linked, .directStable].map classifyPlanOriginConstructor

/-- One positional structure classifier invocation for every covered structure. -/
def structureCoverage : List ItemCoverage :=
  classifyScientificFields (scientific 1 (-1)) ++
  classifyPriorFields { family := .normal, args := [scientific 0 0, scientific 1 0] } ++
  classifyParamDeclFields
    { name := "normal_rate", ty := .real, default := .real (scientific 1 (-1)),
      prior := some { family := .normal, args := [scientific 0 0, scientific 1 0] } } ++
  classifyAttrFields { name := "mass", ty := .real } ++
  classifyTableFields peopleTable ++
  classifyResourceClaimFields
    { resource := .selfAttr "region", ordering := .raceTime } ++
  classifyTransitionFields transitionFixture ++
  classifyPortDeclFields inputPortFixture ++
  classifyOutputFieldFields { name := "population", op := .count, filter := none } ++
  classifyOutputDeclFields outputFixture ++
  classifyViewDeclFields
    { name := "sum_age", table := "people", filter := none,
      value := some (.selfAttr "age"), reduce := .sum } ++
  classifyGroupKeyFields { attr := "status", bandWidth := none } ++
  classifyGroupedViewDeclFields groupedViewFixture ++
  classifyBoxFields boxFixture ++
  classifyWireEndpointFields modelWire.source ++
  classifyWireFields modelWire ++
  classifySummaryDeclFields
    { name := "sum_age", box := "population", view := "sum_age", reduce := .sum } ++
  classifyModelFields rawModelFixture ++
  classifyStableIdFields (sid "id") ++
  classifyCompositionPortDeclFields compositionInputPort ++
  classifyParameterBindingFields bindingFixture ++
  classifyInstanceDeclFields instanceFixture ++
  classifyWireDeclFields compositionWireFixture ++
  classifyExposureDeclFields exposureFixture ++
  classifyHiddenPortFields hiddenPortFixture ++
  classifyPrimitiveBodyFields primitiveBodyFixture ++
  classifyCompositeBodyFields compositeBodyFixture ++
  classifyComponentDefinitionFields compositeDefinition ++
  classifySourceSummaryFields sourceSummaryFixture ++
  classifyCompositionSourceFields compositionSourceFixture ++
  classifySourceMapLeafFields sourceMapLeaf ++
  classifySourceMapBoundaryFields sourceMapBoundary ++
  classifySourceMapHiddenFields sourceMapHidden ++
  classifySourceMapFields sourceMapFixture ++
  classifyHashRecordFields hashFixture ++
  classifyLinkerDescriptorFields linkerFixture ++
  classifyLinkedProvenanceFields linkedProvenanceFixture ++
  classifySchedulerDomainFields schedulerFixture ++
  classifyLeafIdentityFields leafIdentityFixture ++
  classifyTransitionIdentityFields transitionIdentityFixture ++
  classifyMailboxIdentityFields mailboxIdentityFixture ++
  classifyIdentityMapFields identityFixture ++
  classifyExecutablePlanFields linkedPlanFixture

def coverageFixture : List ItemCoverage := constructorCoverage ++ structureCoverage

#guard parameterFixtures.length == 4
#guard expressionVariants.length == 22
#guard aggregateVariants.length == 2
#guard transitionFixture.contests.length == 2
#guard claimOrderingVariants.length == 2
#guard groupedViewFixture.keys.length == 2
#guard viewFixtures.length == 4
#guard summaryFixtures.length == 5
#guard compositeBodyFixture.instances.length == 1
#guard compositeBodyFixture.wires.length == 1
#guard compositeBodyFixture.exposures.length == 1
#guard compositeBodyFixture.hiddenPorts.length == 1
#guard compositionSourceFixture.definitions.length == 2
#guard compositionSourceFixture.summaries.length == 1
#guard sourceMapFixture.leaves.length == 1
#guard sourceMapFixture.boundary.length == 1
#guard sourceMapFixture.hidden.length == 1
#guard linkedPlanFixture.origin == .linked && linkedPlanFixture.linkedProvenance.isSome
#guard directPlanFixture.origin == .directStable && directPlanFixture.linkedProvenance.isNone
#guard linkedPlanFixture.schemaVersion == Plan.planSchema
#guard compositionSourceFixture.schemaVersion == Plan.compositionSourceSchema
#guard sourceMapFixture.schemaVersion == Plan.sourceMapSchema
#guard constructorCoverage.length == 55
#guard structureCoverage.length == 163
#guard coverageFixture.length == 218
#guard coverageFixture.all (fun item => item.rawInventoryOwner == 2)
#guard (coverageFixture.map (fun item => item.item)).eraseDups.length == coverageFixture.length

end Sembla.Semantics.RawTests
