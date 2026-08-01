import Sembla.IR
import Sembla.Composition.Source
import Sembla.Composition.SourceMap
import Sembla.Plan

/-!
Machine-checked inventory of the serialization-friendly V1 declarations.

The definitions here classify existing declarations; they do not redefine the
raw contract or assign behavioral meaning to unchecked syntax. Structure
classifiers use positional `mk` patterns deliberately: adding a field changes
the constructor arity and makes this module fail to compile. Inductive
classifiers are exhaustive, so adding a constructor has the same effect.
-/
namespace Sembla.Semantics.Raw

abbrev RawModel := IR.Model
abbrev RawCompositionSource := Composition.CompositionSourceV1
abbrev RawSourceMap := Composition.SourceMapV1
abbrev RawExecutablePlan := Plan.ExecutablePlanV1

inductive FoundationalRole where
  | semantic
  | structural
  | observational
  | provenanceOnly
deriving Repr, BEq

inductive FrontendStatus where
  | surfaceProduced
  | rawOnlyAccepted
  | contextuallyRejected
  | deferredCompositionInput
deriving Repr, BEq

inductive MeaningOwner where
  | prd0003
  | prd0004
  | prd0005
  | prd0006
  | prd0012
  | prd0013
  | prd0015
  | prd0016
  | prd0019
  | prd0020
  | futureComposition
deriving Repr, BEq

structure ItemCoverage where
  item : String
  rawInventoryOwner : Nat
  role : FoundationalRole
  frontendStatus : FrontendStatus
  meaningOwner : MeaningOwner
  theoremDependencies : List Nat
deriving Repr, BEq

private def covered (item : String) (role : FoundationalRole)
    (frontendStatus : FrontendStatus) (meaningOwner : MeaningOwner)
    (theoremDependencies : List Nat := []) : ItemCoverage :=
  { item
    rawInventoryOwner := 2
    role
    frontendStatus
    meaningOwner
    theoremDependencies }

/-! ## `Sembla.IR` inductive-constructor classifiers -/

def classifyParamTypeConstructor : IR.ParamType → ItemCoverage
  | .real => covered "IR.ParamType.real" .semantic .surfaceProduced .prd0003 [5, 6, 7]
  | .int => covered "IR.ParamType.int" .semantic .surfaceProduced .prd0003 [5, 6, 7]

def classifyParamValueConstructor : IR.ParamValue → ItemCoverage
  | .real _ => covered "IR.ParamValue.real" .semantic .surfaceProduced .prd0003 [5, 7]
  | .int _ => covered "IR.ParamValue.int" .semantic .surfaceProduced .prd0003 [5, 7]

def classifyPriorFamilyConstructor : IR.PriorFamily → ItemCoverage
  | .normal => covered "IR.PriorFamily.normal" .structural .surfaceProduced .prd0005 [7]
  | .logNormal => covered "IR.PriorFamily.logNormal" .structural .surfaceProduced .prd0005 [7]
  | .uniform => covered "IR.PriorFamily.uniform" .structural .surfaceProduced .prd0005 [7]

def classifyAttrTypeConstructor : IR.AttrType → ItemCoverage
  | .real => covered "IR.AttrType.real" .semantic .surfaceProduced .prd0003 [5, 6]
  | .int => covered "IR.AttrType.int" .semantic .surfaceProduced .prd0003 [5, 6]
  | .enum _ => covered "IR.AttrType.enum" .semantic .surfaceProduced .prd0003 [5, 6]
  | .ref _ => covered "IR.AttrType.ref" .semantic .surfaceProduced .prd0003 [5, 6, 11]

def classifyExprConstructor : IR.Expr → ItemCoverage
  | .real _ => covered "IR.Expr.real" .semantic .surfaceProduced .prd0004 [6, 10]
  | .int _ => covered "IR.Expr.int" .semantic .surfaceProduced .prd0004 [6, 10]
  | .bool _ => covered "IR.Expr.bool" .semantic .surfaceProduced .prd0004 [6, 10]
  | .enum _ => covered "IR.Expr.enum" .semantic .surfaceProduced .prd0004 [6, 10]
  | .param _ => covered "IR.Expr.param" .semantic .surfaceProduced .prd0004 [6, 10]
  | .selfAttr _ => covered "IR.Expr.selfAttr" .semantic .surfaceProduced .prd0004 [6, 10, 11]
  | .add _ _ => covered "IR.Expr.add" .semantic .surfaceProduced .prd0004 [6, 10]
  | .sub _ _ => covered "IR.Expr.sub" .semantic .surfaceProduced .prd0004 [6, 10]
  | .mul _ _ => covered "IR.Expr.mul" .semantic .surfaceProduced .prd0004 [6, 10]
  | .div _ _ => covered "IR.Expr.div" .semantic .surfaceProduced .prd0004 [6, 10]
  | .eq _ _ => covered "IR.Expr.eq" .semantic .surfaceProduced .prd0004 [6, 10]
  | .ne _ _ => covered "IR.Expr.ne" .semantic .surfaceProduced .prd0004 [6, 10]
  | .lt _ _ => covered "IR.Expr.lt" .semantic .surfaceProduced .prd0004 [6, 10]
  | .le _ _ => covered "IR.Expr.le" .semantic .surfaceProduced .prd0004 [6, 10]
  | .gt _ _ => covered "IR.Expr.gt" .semantic .surfaceProduced .prd0004 [6, 10]
  | .ge _ _ => covered "IR.Expr.ge" .semantic .surfaceProduced .prd0004 [6, 10]
  | .and _ _ => covered "IR.Expr.and" .semantic .surfaceProduced .prd0004 [6, 10]
  | .or _ _ => covered "IR.Expr.or" .semantic .surfaceProduced .prd0004 [6, 10]
  | .not _ => covered "IR.Expr.not" .semantic .surfaceProduced .prd0004 [6, 10]
  | .enumIs _ _ => covered "IR.Expr.enumIs" .semantic .surfaceProduced .prd0004 [6, 10, 11]
  | .input _ _ => covered "IR.Expr.input" .semantic .surfaceProduced .prd0004 [6, 12]
  | .agg _ _ _ _ _ => covered "IR.Expr.agg" .semantic .surfaceProduced .prd0004 [6, 12]

def classifyAggOpConstructor : IR.AggOp → ItemCoverage
  | .count => covered "IR.AggOp.count" .semantic .surfaceProduced .prd0004 [6, 12]
  | .sum _ => covered "IR.AggOp.sum" .semantic .surfaceProduced .prd0004 [6, 12]

def classifyAggregateConstructor : IR.Aggregate → ItemCoverage
  | .mk _ _ => covered "IR.Aggregate.mk" .semantic .surfaceProduced .prd0004 [6, 12]

def classifyEffectConstructor : IR.Effect → ItemCoverage
  | .setAttr _ _ => covered "IR.Effect.setAttr" .semantic .surfaceProduced .prd0004 [6, 16]

def classifyClaimOrderingConstructor : IR.ClaimOrdering → ItemCoverage
  | .raceTime => covered "IR.ClaimOrdering.raceTime" .semantic .surfaceProduced .prd0004 [6, 15]
  | .key _ => covered "IR.ClaimOrdering.key" .semantic .rawOnlyAccepted .prd0004 [6, 15]

def classifyOutputBuilderConstructor : IR.OutputBuilder → ItemCoverage
  | .perTable _ _ => covered "IR.OutputBuilder.perTable" .observational .surfaceProduced .prd0012 [5, 6, 9]

def classifyViewReduceConstructor : IR.ViewReduce → ItemCoverage
  | .sum => covered "IR.ViewReduce.sum" .observational .surfaceProduced .prd0013 [5, 6]
  | .count => covered "IR.ViewReduce.count" .observational .surfaceProduced .prd0013 [5, 6]
  | .min => covered "IR.ViewReduce.min" .observational .surfaceProduced .prd0013 [5, 6, 17]
  | .max => covered "IR.ViewReduce.max" .observational .surfaceProduced .prd0013 [5, 6, 17]

def classifySummaryReduceConstructor : IR.SummaryReduce → ItemCoverage
  | .sum => covered "IR.SummaryReduce.sum" .observational .surfaceProduced .prd0013 [17]
  | .min => covered "IR.SummaryReduce.min" .observational .surfaceProduced .prd0013 [17]
  | .max => covered "IR.SummaryReduce.max" .observational .surfaceProduced .prd0013 [17]
  | .last => covered "IR.SummaryReduce.last" .observational .surfaceProduced .prd0013 [17]
  | .argmaxTick => covered "IR.SummaryReduce.argmaxTick" .observational .surfaceProduced .prd0013 [17]

/-! ## `Sembla.IR` structure-field classifiers and arity guards -/

def classifyScientificFields : IR.Scientific → List ItemCoverage
  | .mk _ _ =>
      [ covered "IR.Scientific.coefficient" .semantic .surfaceProduced .prd0003 [10, 20]
      , covered "IR.Scientific.exponent" .semantic .surfaceProduced .prd0003 [10, 20] ]

def classifyPriorFields : IR.Prior → List ItemCoverage
  | .mk _ _ =>
      [ covered "IR.Prior.family" .semantic .surfaceProduced .prd0005 [7]
      , covered "IR.Prior.args" .semantic .contextuallyRejected .prd0005 [7] ]

def classifyParamDeclFields : IR.ParamDecl → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "IR.ParamDecl.name" .structural .surfaceProduced .prd0005 [7]
      , covered "IR.ParamDecl.ty" .semantic .surfaceProduced .prd0003 [5, 7]
      , covered "IR.ParamDecl.default" .semantic .contextuallyRejected .prd0005 [7]
      , covered "IR.ParamDecl.prior" .semantic .contextuallyRejected .prd0005 [7] ]

def classifyAttrFields : IR.Attr → List ItemCoverage
  | .mk _ _ =>
      [ covered "IR.Attr.name" .structural .surfaceProduced .prd0005 [7]
      , covered "IR.Attr.ty" .semantic .surfaceProduced .prd0003 [5, 7] ]

def classifyTableFields : IR.Table → List ItemCoverage
  | .mk _ _ _ =>
      [ covered "IR.Table.name" .structural .surfaceProduced .prd0005 [7]
      , covered "IR.Table.sizeHint" .structural .surfaceProduced .prd0003 [7, 11]
      , covered "IR.Table.attrs" .semantic .surfaceProduced .prd0003 [5, 7, 11] ]

def classifyResourceClaimFields : IR.ResourceClaim → List ItemCoverage
  | .mk _ _ =>
      [ covered "IR.ResourceClaim.resource" .semantic .surfaceProduced .prd0004 [6, 15]
      , covered "IR.ResourceClaim.ordering" .semantic .surfaceProduced .prd0004 [6, 15] ]

def classifyTransitionFields : IR.Transition → List ItemCoverage
  | .mk _ _ _ _ _ _ =>
      [ covered "IR.Transition.name" .structural .surfaceProduced .prd0005 [6, 8, 14]
      , covered "IR.Transition.table" .structural .contextuallyRejected .prd0005 [6, 8, 14]
      , covered "IR.Transition.guard" .semantic .surfaceProduced .prd0004 [6, 8, 14]
      , covered "IR.Transition.hazard" .semantic .contextuallyRejected .prd0004 [6, 8, 14]
      , covered "IR.Transition.effects" .semantic .surfaceProduced .prd0004 [6, 8, 16]
      , covered "IR.Transition.contests" .semantic .surfaceProduced .prd0015 [6, 8] ]

def classifyPortDeclFields : IR.PortDecl → List ItemCoverage
  | .mk _ _ =>
      [ covered "IR.PortDecl.name" .structural .surfaceProduced .prd0005 [9, 12]
      , covered "IR.PortDecl.schema" .semantic .surfaceProduced .prd0005 [9, 12] ]

def classifyOutputFieldFields : IR.OutputField → List ItemCoverage
  | .mk _ _ _ =>
      [ covered "IR.OutputField.name" .structural .surfaceProduced .prd0012 [5, 6, 9]
      , covered "IR.OutputField.op" .observational .surfaceProduced .prd0012 [5, 6, 9]
      , covered "IR.OutputField.filter" .observational .contextuallyRejected .prd0012 [5, 6, 9] ]

def classifyOutputDeclFields : IR.OutputDecl → List ItemCoverage
  | .mk _ _ _ =>
      [ covered "IR.OutputDecl.name" .structural .surfaceProduced .prd0012 [5, 6, 9]
      , covered "IR.OutputDecl.schema" .semantic .surfaceProduced .prd0012 [5, 6, 9]
      , covered "IR.OutputDecl.builder" .observational .surfaceProduced .prd0012 [5, 6, 9] ]

def classifyViewDeclFields : IR.ViewDecl → List ItemCoverage
  | .mk _ _ _ _ _ =>
      [ covered "IR.ViewDecl.name" .structural .surfaceProduced .prd0013 [5, 6, 9]
      , covered "IR.ViewDecl.table" .structural .contextuallyRejected .prd0013 [5, 6, 9]
      , covered "IR.ViewDecl.filter" .observational .contextuallyRejected .prd0013 [5, 6, 9]
      , covered "IR.ViewDecl.value" .observational .contextuallyRejected .prd0013 [5, 6, 9]
      , covered "IR.ViewDecl.reduce" .observational .surfaceProduced .prd0013 [5, 6, 9] ]

def classifyGroupKeyFields : IR.GroupKey → List ItemCoverage
  | .mk _ _ =>
      [ covered "IR.GroupKey.attr" .structural .contextuallyRejected .prd0013 [5, 6, 9, 18]
      , covered "IR.GroupKey.bandWidth" .observational .contextuallyRejected .prd0013 [5, 6, 9, 18] ]

def classifyGroupedViewDeclFields : IR.GroupedViewDecl → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "IR.GroupedViewDecl.name" .structural .surfaceProduced .prd0013 [5, 6, 9, 18]
      , covered "IR.GroupedViewDecl.table" .structural .contextuallyRejected .prd0013 [5, 6, 9, 18]
      , covered "IR.GroupedViewDecl.filter" .observational .contextuallyRejected .prd0013 [5, 6, 9, 18]
      , covered "IR.GroupedViewDecl.keys" .observational .surfaceProduced .prd0013 [5, 6, 9, 18] ]

def classifyBoxFields : IR.Box → List ItemCoverage
  | .mk _ _ _ _ _ _ _ =>
      [ covered "IR.Box.name" .structural .surfaceProduced .prd0005 [7, 19]
      , covered "IR.Box.tables" .semantic .surfaceProduced .prd0005 [3, 6, 7]
      , covered "IR.Box.transitions" .semantic .surfaceProduced .prd0006 [8, 14, 15, 16]
      , covered "IR.Box.inputs" .semantic .surfaceProduced .prd0012 [5, 6, 9]
      , covered "IR.Box.outputs" .observational .surfaceProduced .prd0012 [5, 6, 9]
      , covered "IR.Box.views" .observational .surfaceProduced .prd0013 [5, 6, 9]
      , covered "IR.Box.groupedViews" .observational .surfaceProduced .prd0013 [5, 6, 9, 18] ]

def classifyWireEndpointFields : IR.WireEndpoint → List ItemCoverage
  | .mk _ _ =>
      [ covered "IR.WireEndpoint.box" .structural .rawOnlyAccepted .prd0019 [20]
      , covered "IR.WireEndpoint.port" .structural .rawOnlyAccepted .prd0019 [20] ]

def classifyWireFields : IR.Wire → List ItemCoverage
  | .mk _ _ =>
      [ covered "IR.Wire.source" .structural .rawOnlyAccepted .prd0019 [20]
      , covered "IR.Wire.target" .structural .rawOnlyAccepted .prd0019 [20] ]

def classifySummaryDeclFields : IR.SummaryDecl → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "IR.SummaryDecl.name" .structural .surfaceProduced .prd0013 [5, 6, 9]
      , covered "IR.SummaryDecl.box" .structural .contextuallyRejected .prd0013 [5, 6]
      , covered "IR.SummaryDecl.view" .structural .contextuallyRejected .prd0013 [5, 6]
      , covered "IR.SummaryDecl.reduce" .observational .surfaceProduced .prd0013 [17] ]

def classifyModelFields : IR.Model → List ItemCoverage
  | .mk _ _ _ _ _ _ =>
      [ covered "IR.Model.name" .structural .surfaceProduced .prd0005 [7, 19]
      , covered "IR.Model.dt" .semantic .contextuallyRejected .prd0005 [10, 16]
      , covered "IR.Model.params" .semantic .surfaceProduced .prd0005 [3, 7]
      , covered "IR.Model.boxes" .semantic .surfaceProduced .prd0006 [19]
      , covered "IR.Model.wires" .structural .rawOnlyAccepted .prd0019 [20]
      , covered "IR.Model.summaries" .observational .surfaceProduced .prd0013 [17] ]

/-! ## Composition-source classifiers

These rows classify raw deferred input only. Their behavioral meaning and
checking owner is the future composition formalization track.
-/

def classifyPortDirectionConstructor : Composition.PortDirection → ItemCoverage
  | .input => covered "Composition.PortDirection.input" .structural .deferredCompositionInput .futureComposition
  | .output => covered "Composition.PortDirection.output" .structural .deferredCompositionInput .futureComposition

def classifyComponentBodyConstructor : Composition.ComponentBodyV1 → ItemCoverage
  | .primitive _ => covered "Composition.ComponentBodyV1.primitive" .structural .deferredCompositionInput .futureComposition
  | .composite _ => covered "Composition.ComponentBodyV1.composite" .structural .deferredCompositionInput .futureComposition

def classifyStableIdFields : Composition.StableId → List ItemCoverage
  | .mk _ =>
      [covered "Composition.StableId.raw" .structural .deferredCompositionInput .futureComposition]

def classifyCompositionPortDeclFields : Composition.PortDeclV1 → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "Composition.PortDeclV1.id" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.PortDeclV1.displayName" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.PortDeclV1.direction" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.PortDeclV1.schema" .semantic .deferredCompositionInput .futureComposition ]

def classifyParameterBindingFields : Composition.ParameterBinding → List ItemCoverage
  | .mk _ _ =>
      [ covered "Composition.ParameterBinding.requirement" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.ParameterBinding.parameter" .structural .deferredCompositionInput .futureComposition ]

def classifyInstanceDeclFields : Composition.InstanceDeclV1 → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "Composition.InstanceDeclV1.id" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.InstanceDeclV1.displayName" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.InstanceDeclV1.definition" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.InstanceDeclV1.parameterBindings" .structural .deferredCompositionInput .futureComposition ]

def classifyWireDeclFields : Composition.WireDeclV1 → List ItemCoverage
  | .mk _ _ _ _ _ _ =>
      [ covered "Composition.WireDeclV1.id" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.WireDeclV1.sourceInstance" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.WireDeclV1.sourcePort" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.WireDeclV1.targetInstance" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.WireDeclV1.targetPort" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.WireDeclV1.delayTicks" .semantic .deferredCompositionInput .futureComposition ]

def classifyExposureDeclFields : Composition.ExposureDeclV1 → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "Composition.ExposureDeclV1.id" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.ExposureDeclV1.innerInstance" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.ExposureDeclV1.innerPort" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.ExposureDeclV1.outerPort" .structural .deferredCompositionInput .futureComposition ]

def classifyHiddenPortFields : Composition.HiddenPortV1 → List ItemCoverage
  | .mk _ _ =>
      [ covered "Composition.HiddenPortV1.instance_" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.HiddenPortV1.port" .structural .deferredCompositionInput .futureComposition ]

def classifyPrimitiveBodyFields : Composition.PrimitiveBodyV1 → List ItemCoverage
  | .mk _ _ _ _ _ =>
      [ covered "Composition.PrimitiveBodyV1.tables" .semantic .deferredCompositionInput .futureComposition
      , covered "Composition.PrimitiveBodyV1.transitions" .semantic .deferredCompositionInput .futureComposition
      , covered "Composition.PrimitiveBodyV1.inputs" .semantic .deferredCompositionInput .futureComposition
      , covered "Composition.PrimitiveBodyV1.outputs" .observational .deferredCompositionInput .futureComposition
      , covered "Composition.PrimitiveBodyV1.views" .observational .deferredCompositionInput .futureComposition ]

def classifyCompositeBodyFields : Composition.CompositeBodyV1 → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "Composition.CompositeBodyV1.instances" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.CompositeBodyV1.wires" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.CompositeBodyV1.exposures" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.CompositeBodyV1.hiddenPorts" .structural .deferredCompositionInput .futureComposition ]

def classifyComponentDefinitionFields : Composition.ComponentDefinitionV1 → List ItemCoverage
  | .mk _ _ _ _ _ =>
      [ covered "Composition.ComponentDefinitionV1.id" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.ComponentDefinitionV1.displayName" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.ComponentDefinitionV1.parameterRequirements" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.ComponentDefinitionV1.ports" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.ComponentDefinitionV1.body" .semantic .deferredCompositionInput .futureComposition ]

def classifySourceSummaryFields : Composition.SourceSummaryV1 → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "Composition.SourceSummaryV1.name" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.SourceSummaryV1.reduce" .observational .deferredCompositionInput .futureComposition
      , covered "Composition.SourceSummaryV1.instancePath" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.SourceSummaryV1.view" .observational .deferredCompositionInput .futureComposition ]

def classifyCompositionSourceFields : Composition.CompositionSourceV1 → List ItemCoverage
  | .mk _ _ _ _ _ _ _ _ _ =>
      [ covered "Composition.CompositionSourceV1.schemaVersion" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.CompositionSourceV1.modelId" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.CompositionSourceV1.displayName" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.CompositionSourceV1.outerDt" .semantic .deferredCompositionInput .futureComposition
      , covered "Composition.CompositionSourceV1.parameters" .semantic .deferredCompositionInput .futureComposition
      , covered "Composition.CompositionSourceV1.definitions" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.CompositionSourceV1.rootDefinition" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.CompositionSourceV1.requiredFeatures" .structural .deferredCompositionInput .futureComposition
      , covered "Composition.CompositionSourceV1.summaries" .observational .deferredCompositionInput .futureComposition ]

/-! ## Source-map provenance classifiers -/

def classifySourceMapLeafFields : Composition.SourceMapLeafV1 → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "Composition.SourceMapLeafV1.occurrence" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.SourceMapLeafV1.definition" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.SourceMapLeafV1.instancePath" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.SourceMapLeafV1.displayPath" .provenanceOnly .deferredCompositionInput .futureComposition ]

def classifySourceMapBoundaryFields : Composition.SourceMapBoundaryV1 → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "Composition.SourceMapBoundaryV1.outer" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.SourceMapBoundaryV1.leaf" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.SourceMapBoundaryV1.port" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.SourceMapBoundaryV1.path" .provenanceOnly .deferredCompositionInput .futureComposition ]

def classifySourceMapHiddenFields : Composition.SourceMapHiddenV1 → List ItemCoverage
  | .mk _ _ =>
      [ covered "Composition.SourceMapHiddenV1.instance_" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.SourceMapHiddenV1.port" .provenanceOnly .deferredCompositionInput .futureComposition ]

def classifySourceMapFields : Composition.SourceMapV1 → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "Composition.SourceMapV1.schemaVersion" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.SourceMapV1.leaves" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.SourceMapV1.boundary" .provenanceOnly .deferredCompositionInput .futureComposition
      , covered "Composition.SourceMapV1.hidden" .provenanceOnly .deferredCompositionInput .futureComposition ]

/-! ## Plan classifiers -/

def classifyPlanOriginConstructor : Plan.PlanOrigin → ItemCoverage
  | .linked => covered "Plan.PlanOrigin.linked" .structural .rawOnlyAccepted .prd0019 [20]
  | .directStable => covered "Plan.PlanOrigin.directStable" .structural .rawOnlyAccepted .prd0019 [20]

def classifyHashRecordFields : Plan.HashRecordV1 → List ItemCoverage
  | .mk _ _ _ =>
      [ covered "Plan.HashRecordV1.algorithm" .provenanceOnly .rawOnlyAccepted .prd0020
      , covered "Plan.HashRecordV1.domain" .provenanceOnly .rawOnlyAccepted .prd0020
      , covered "Plan.HashRecordV1.digest" .provenanceOnly .rawOnlyAccepted .prd0020 ]

def classifyLinkerDescriptorFields : Plan.LinkerDescriptorV1 → List ItemCoverage
  | .mk _ _ _ _ _ _ =>
      [ covered "Plan.LinkerDescriptorV1.semantics" .provenanceOnly .rawOnlyAccepted .prd0020
      , covered "Plan.LinkerDescriptorV1.sourceSchema" .provenanceOnly .rawOnlyAccepted .prd0020
      , covered "Plan.LinkerDescriptorV1.planSchema" .provenanceOnly .rawOnlyAccepted .prd0020
      , covered "Plan.LinkerDescriptorV1.identityScheme" .provenanceOnly .rawOnlyAccepted .prd0020
      , covered "Plan.LinkerDescriptorV1.canonicalEncoding" .provenanceOnly .rawOnlyAccepted .prd0020
      , covered "Plan.LinkerDescriptorV1.sourceMapSchema" .provenanceOnly .rawOnlyAccepted .prd0020 ]

def classifyLinkedProvenanceFields : Plan.LinkedProvenanceV1 → List ItemCoverage
  | .mk _ _ _ =>
      [ covered "Plan.LinkedProvenanceV1.sourceHash" .provenanceOnly .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.LinkedProvenanceV1.linker" .provenanceOnly .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.LinkedProvenanceV1.sourceMap" .provenanceOnly .rawOnlyAccepted .prd0020 [19] ]

def classifySchedulerDomainFields : Plan.SchedulerDomainV1 → List ItemCoverage
  | .mk _ _ _ =>
      [ covered "Plan.SchedulerDomainV1.id" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.SchedulerDomainV1.algorithm" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.SchedulerDomainV1.leaves" .structural .rawOnlyAccepted .prd0020 [19] ]

def classifyLeafIdentityFields : Plan.LeafIdentityV1 → List ItemCoverage
  | .mk _ _ =>
      [ covered "Plan.LeafIdentityV1.box" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.LeafIdentityV1.occurrence" .structural .rawOnlyAccepted .prd0020 [19] ]

def classifyTransitionIdentityFields : Plan.TransitionIdentityV1 → List ItemCoverage
  | .mk _ _ _ _ =>
      [ covered "Plan.TransitionIdentityV1.box" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.TransitionIdentityV1.name" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.TransitionIdentityV1.identity" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.TransitionIdentityV1.ruleWord" .structural .rawOnlyAccepted .prd0020 [19] ]

def classifyMailboxIdentityFields : Plan.MailboxIdentityV1 → List ItemCoverage
  | .mk _ _ _ _ _ =>
      [ covered "Plan.MailboxIdentityV1.identity" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.MailboxIdentityV1.sourceBox" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.MailboxIdentityV1.sourcePort" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.MailboxIdentityV1.targetBox" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.MailboxIdentityV1.targetPort" .structural .rawOnlyAccepted .prd0020 [19] ]

def classifyIdentityMapFields : Plan.IdentityMapV1 → List ItemCoverage
  | .mk _ _ _ _ _ _ =>
      [ covered "Plan.IdentityMapV1.modelId" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.IdentityMapV1.enabledFeatures" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.IdentityMapV1.schedulerDomains" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.IdentityMapV1.leaves" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.IdentityMapV1.transitions" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.IdentityMapV1.mailboxes" .structural .rawOnlyAccepted .prd0020 [19] ]

def classifyExecutablePlanFields : Plan.ExecutablePlanV1 → List ItemCoverage
  | .mk _ _ _ _ _ _ =>
      [ covered "Plan.ExecutablePlanV1.schemaVersion" .structural .rawOnlyAccepted .prd0019 [20]
      , covered "Plan.ExecutablePlanV1.identityScheme" .structural .rawOnlyAccepted .prd0019 [20]
      , covered "Plan.ExecutablePlanV1.origin" .structural .rawOnlyAccepted .prd0019 [20]
      , covered "Plan.ExecutablePlanV1.model" .semantic .rawOnlyAccepted .prd0019 [6]
      , covered "Plan.ExecutablePlanV1.identity" .structural .rawOnlyAccepted .prd0020 [19]
      , covered "Plan.ExecutablePlanV1.linkedProvenance" .provenanceOnly .rawOnlyAccepted .prd0020 [19] ]

end Sembla.Semantics.Raw
