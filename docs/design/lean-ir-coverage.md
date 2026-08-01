# Lean raw IR and plan coverage

Status: **PRD 0002 raw inventory**. This document classifies the current
serialization-friendly declarations; it does not claim checked or behavioral
semantics.

## Exact boundary and exclusions

The inventory boundary is exactly the public structures, their fields and the
inductive constructors declared in:

- [`Sembla.IR`](../../frontend/Sembla/IR.lean);
- [`Sembla.Composition.Source`](../../frontend/Sembla/Composition/Source.lean);
- [`Sembla.Composition.SourceMap`](../../frontend/Sembla/Composition/SourceMap.lean);
- [`Sembla.Plan`](../../frontend/Sembla/Plan.lean).

There are **55 inductive constructors and 163 structure fields (218 items)**.
Every item below has **PRD 0002** as its raw inventory owner. The separate
meaning/invariant owner is shown in the tables. `Sembla.Plan` version constants
are governing fixture evidence for version fields, not additional inventory
items.

Excluded are `PlanJson.CJson`, canonical encoders and bytes, hash
implementations, linker algorithms/errors and bundle artifact containers.

## Classification notation

The two classification axes are independent:

- foundational role: `S` semantic, `T` structural, `O` observational, `P`
  provenance-only;
- frontend/checking status: `SP` surface-produced, `RO` raw-only accepted,
  `CR` contextually rejected for some raw values, `DC` deferred composition
  input.

An entry such as `dt (S/CR/0005 → 0010,0016)` means role `S`, status `CR`,
meaning/invariant owner PRD 0005 and theorem dependencies PRDs 0010 and 0016.
Dependencies may be prerequisites or later consumers; the arrow does not imply
numeric run order. `FC` means the explicitly deferred future composition
formalization track. It is an owner, not a theorem claimed by the present track.

All classifier links below refer to exhaustive functions in
[`Sembla.Semantics.Raw`][raw-classifiers]. Inductive functions pattern-match
every constructor. Structure functions use every positional `mk` argument, so
constructor arity changes fail compilation. Every function is exercised by
[`constructorCoverage` or `structureCoverage`][raw-fixtures]; the combined
`coverageFixture` checks item count, unique names and raw owner `0002`.

## `Sembla.IR` inductive constructors

| Declaration | Constructors and classification | Guard / fixture |
| --- | --- | --- |
| `ParamType` | `real`, `int` (S/SP/0003 → 0005,0006,0007) | `classifyParamTypeConstructor`; `constructorCoverage` |
| `ParamValue` | `real`, `int` (S/SP/0003 → 0005,0007) | `classifyParamValueConstructor`; `parameterFixtures` |
| `PriorFamily` | `normal`, `logNormal`, `uniform` (T/SP/0005 → 0007) | `classifyPriorFamilyConstructor`; `parameterFixtures` |
| `AttrType` | `real`, `int`, `enum` (S/SP/0003 → 0005,0006); `ref` (S/SP/0003 → 0005,0006,0011) | `classifyAttrTypeConstructor`; `attributeFixtures` |
| `Expr` | `real`, `int`, `bool`, `enum`, `param`, `add`, `sub`, `mul`, `div`, `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `and`, `or`, `not` (S/SP/0004 → 0006,0010); `selfAttr`, `enumIs` (S/SP/0004 → 0006,0010,0011); `input`, `agg` (S/SP/0004 → 0006,0012) | `classifyExprConstructor`; `expressionVariants` |
| `AggOp` | `count`, `sum` (S/SP/0004 → 0006,0012) | `classifyAggOpConstructor`; `aggregateVariants` |
| `Aggregate` | `mk` (S/SP/0004 → 0006,0012) | `classifyAggregateConstructor`; `aggregateVariants` |
| `Effect` | `setAttr` (S/SP/0004 → 0006,0016) | `classifyEffectConstructor`; `transitionFixture` |
| `ClaimOrdering` | `raceTime` (S/SP/0004 → 0006,0015); `key` (S/RO/0004 → 0006,0015) | `classifyClaimOrderingConstructor`; `claimOrderingVariants` |
| `OutputBuilder` | `perTable` (O/SP/0012 → 0005,0006,0009) | `classifyOutputBuilderConstructor`; `outputFixture` |
| `ViewReduce` | `sum`, `count` (O/SP/0013 → 0005,0006); `min`, `max` (O/SP/0013 → 0005,0006,0017) | `classifyViewReduceConstructor`; `viewFixtures` |
| `SummaryReduce` | `sum`, `min`, `max`, `last`, `argmaxTick` (O/SP/0013 → 0017) | `classifySummaryReduceConstructor`; `summaryFixtures` |

## `Sembla.IR` structure fields

| Declaration | Every field: role/status/meaning owner → theorem dependencies | Guard / fixture |
| --- | --- | --- |
| `Scientific` | `coefficient`, `exponent` (S/SP/0003 → 0010,0020) | `classifyScientificFields`; `rawModelFixture` |
| `Prior` | `family` (S/SP/0005 → 0007); `args` (S/CR/0005 → 0007) | `classifyPriorFields`; `parameterFixtures` |
| `ParamDecl` | `name` (T/SP/0005 → 0007); `ty` (S/SP/0003 → 0005,0007); `default`, `prior` (S/CR/0005 → 0007) | `classifyParamDeclFields`; `parameterFixtures` |
| `Attr` | `name` (T/SP/0005 → 0007); `ty` (S/SP/0003 → 0005,0007) | `classifyAttrFields`; `attributeFixtures` |
| `Table` | `name` (T/SP/0005 → 0007); `sizeHint` (T/SP/0003 → 0007,0011); `attrs` (S/SP/0003 → 0005,0007,0011) | `classifyTableFields`; `rawModelFixture` |
| `ResourceClaim` | `resource`, `ordering` (S/SP/0004 → 0006,0015) | `classifyResourceClaimFields`; `transitionFixture` |
| `Transition` | `name` (T/SP/0005 → 0006,0008,0014); `table` (T/CR/0005 → 0006,0008,0014); `guard` (S/SP/0004 → 0006,0008,0014); `hazard` (S/CR/0004 → 0006,0008,0014); `effects` (S/SP/0004 → 0006,0008,0016); `contests` (S/SP/0015 → 0006,0008) | `classifyTransitionFields`; `transitionFixture` |
| `PortDecl` | `name` (T/SP/0005 → 0009,0012); `schema` (S/SP/0005 → 0009,0012) | `classifyPortDeclFields`; `inputPortFixture` |
| `OutputField` | `name` (T/SP/0012 → 0005,0006,0009); `op` (O/SP/0012 → 0005,0006,0009); `filter` (O/CR/0012 → 0005,0006,0009) | `classifyOutputFieldFields`; `outputFields` |
| `OutputDecl` | `name` (T/SP/0012 → 0005,0006,0009); `schema` (S/SP/0012 → 0005,0006,0009); `builder` (O/SP/0012 → 0005,0006,0009) | `classifyOutputDeclFields`; `outputFixture` |
| `ViewDecl` | `name` (T/SP/0013 → 0005,0006,0009); `table` (T/CR/0013 → 0005,0006,0009); `filter`, `value` (O/CR/0013 → 0005,0006,0009); `reduce` (O/SP/0013 → 0005,0006,0009) | `classifyViewDeclFields`; `viewFixtures` |
| `GroupKey` | `attr` (T/CR/0013 → 0005,0006,0009,0018); `bandWidth` (O/CR/0013 → 0005,0006,0009,0018) | `classifyGroupKeyFields`; `groupKeys` |
| `GroupedViewDecl` | `name` (T/SP/0013 → 0005,0006,0009,0018); `table` (T/CR/0013 → 0005,0006,0009,0018); `filter` (O/CR/0013 → 0005,0006,0009,0018); `keys` (O/SP/0013 → 0005,0006,0009,0018) | `classifyGroupedViewDeclFields`; `groupedViewFixture` |
| `Box` | `name` (T/SP/0005 → 0007,0019); `tables` (S/SP/0005 → 0003,0006,0007); `transitions` (S/SP/0006 → 0008,0014,0015,0016); `inputs` (S/SP/0012 → 0005,0006,0009); `outputs` (O/SP/0012 → 0005,0006,0009); `views` (O/SP/0013 → 0005,0006,0009); `groupedViews` (O/SP/0013 → 0005,0006,0009,0018) | `classifyBoxFields`; `boxFixture` |
| `WireEndpoint` | `box`, `port` (T/RO/0019 → 0020) | `classifyWireEndpointFields`; `modelWire` |
| `Wire` | `source`, `target` (T/RO/0019 → 0020) | `classifyWireFields`; `modelWire` |
| `SummaryDecl` | `name` (T/SP/0013 → 0005,0006,0009); `box`, `view` (T/CR/0013 → 0005,0006); `reduce` (O/SP/0013 → 0017) | `classifySummaryDeclFields`; `summaryFixtures` |
| `Model` | `name` (T/SP/0005 → 0007,0019); `dt` (S/CR/0005 → 0010,0016); `params` (S/SP/0005 → 0003,0007); `boxes` (S/SP/0006 → 0019); `wires` (T/RO/0019 → 0020); `summaries` (O/SP/0013 → 0017) | `classifyModelFields`; `rawModelFixture` |

## Composition-source inventory

Every item in this section has status `DC` and meaning/invariant owner `FC`.
That classification records raw input and a future obligation only; it does not
invent a current-track composition checker or denotation.

| Declaration | Constructors or fields by foundational role | Guard / fixture |
| --- | --- | --- |
| `StableId` | `raw` (T) | `classifyStableIdFields`; `compositionSourceFixture` |
| `PortDirection` | `input`, `output` (T) | `classifyPortDirectionConstructor`; `compositionInputPort`, `compositionOutputPort` |
| `PortDeclV1` | `id`, `direction` (T); `displayName` (P); `schema` (S) | `classifyCompositionPortDeclFields`; composition port fixtures |
| `ParameterBinding` | `requirement`, `parameter` (T) | `classifyParameterBindingFields`; `bindingFixture` |
| `InstanceDeclV1` | `id`, `definition`, `parameterBindings` (T); `displayName` (P) | `classifyInstanceDeclFields`; `instanceFixture` |
| `WireDeclV1` | `id`, `sourceInstance`, `sourcePort`, `targetInstance`, `targetPort` (T); `delayTicks` (S) | `classifyWireDeclFields`; `compositionWireFixture` |
| `ExposureDeclV1` | `id`, `innerInstance`, `innerPort`, `outerPort` (T) | `classifyExposureDeclFields`; `exposureFixture` |
| `HiddenPortV1` | `instance_`, `port` (T) | `classifyHiddenPortFields`; `hiddenPortFixture` |
| `PrimitiveBodyV1` | `tables`, `transitions`, `inputs` (S); `outputs`, `views` (O) | `classifyPrimitiveBodyFields`; `primitiveBodyFixture` |
| `CompositeBodyV1` | `instances`, `wires`, `exposures`, `hiddenPorts` (T) | `classifyCompositeBodyFields`; `compositeBodyFixture` |
| `ComponentBodyV1` | `primitive`, `composite` (T) | `classifyComponentBodyConstructor`; `constructorCoverage` |
| `ComponentDefinitionV1` | `id`, `parameterRequirements`, `ports` (T); `displayName` (P); `body` (S) | `classifyComponentDefinitionFields`; primitive/composite definitions |
| `SourceSummaryV1` | `name`, `instancePath` (T); `reduce`, `view` (O) | `classifySourceSummaryFields`; `sourceSummaryFixture` |
| `CompositionSourceV1` | `schemaVersion`, `modelId`, `definitions`, `rootDefinition`, `requiredFeatures` (T); `displayName` (P); `outerDt`, `parameters` (S); `summaries` (O) | `classifyCompositionSourceFields`; `compositionSourceFixture` |

## Source-map provenance inventory

All source-map fields are `P/DC/FC`.

| Declaration | Every field | Guard / fixture |
| --- | --- | --- |
| `SourceMapLeafV1` | `occurrence`, `definition`, `instancePath`, `displayPath` | `classifySourceMapLeafFields`; `sourceMapFixture.leaves` |
| `SourceMapBoundaryV1` | `outer`, `leaf`, `port`, `path` | `classifySourceMapBoundaryFields`; `sourceMapFixture.boundary` |
| `SourceMapHiddenV1` | `instance_`, `port` | `classifySourceMapHiddenFields`; `sourceMapFixture.hidden` |
| `SourceMapV1` | `schemaVersion`, `leaves`, `boundary`, `hidden` | `classifySourceMapFields`; `sourceMapFixture` |

## Plan inventory

| Declaration | Constructors or fields: role/status/meaning owner → theorem dependencies | Guard / fixture |
| --- | --- | --- |
| `PlanOrigin` | `linked`, `directStable` (T/RO/0019 → 0020) | `classifyPlanOriginConstructor`; linked/direct plan fixtures |
| `HashRecordV1` | `algorithm`, `domain`, `digest` (P/RO/0020) | `classifyHashRecordFields`; `hashFixture` |
| `LinkerDescriptorV1` | `semantics`, `sourceSchema`, `planSchema`, `identityScheme`, `canonicalEncoding`, `sourceMapSchema` (P/RO/0020) | `classifyLinkerDescriptorFields`; `linkerFixture` |
| `LinkedProvenanceV1` | `sourceHash`, `linker`, `sourceMap` (P/RO/0020 → 0019) | `classifyLinkedProvenanceFields`; `linkedProvenanceFixture` |
| `SchedulerDomainV1` | `id`, `algorithm`, `leaves` (T/RO/0020 → 0019) | `classifySchedulerDomainFields`; `schedulerFixture` |
| `LeafIdentityV1` | `box`, `occurrence` (T/RO/0020 → 0019) | `classifyLeafIdentityFields`; `leafIdentityFixture` |
| `TransitionIdentityV1` | `box`, `name`, `identity`, `ruleWord` (T/RO/0020 → 0019) | `classifyTransitionIdentityFields`; `transitionIdentityFixture` |
| `MailboxIdentityV1` | `identity`, `sourceBox`, `sourcePort`, `targetBox`, `targetPort` (T/RO/0020 → 0019) | `classifyMailboxIdentityFields`; `mailboxIdentityFixture` |
| `IdentityMapV1` | `modelId`, `enabledFeatures`, `schedulerDomains`, `leaves`, `transitions`, `mailboxes` (T/RO/0020 → 0019) | `classifyIdentityMapFields`; `identityFixture` |
| `ExecutablePlanV1` | `schemaVersion`, `identityScheme`, `origin` (T/RO/0019 → 0020); `model` (S/RO/0019 → 0006); `identity` (T/RO/0020 → 0019); `linkedProvenance` (P/RO/0020 → 0019) | `classifyExecutablePlanFields`; linked/direct plan fixtures |

## Fixture matrix

[`Sembla.Semantics.RawTests`][raw-fixtures] contains only raw data and computed
`#guard` checks:

- `parameterFixtures`, `attributeFixtures`, `expressionVariants`,
  `aggregateVariants`, `viewFixtures`, `summaryFixtures` and
  `rawModelFixture` cover every IR inductive variant, all prior families,
  nested aggregates, grouped views and version-independent raw model fields;
- `transitionFixture` contains two resource claims and
  `ClaimOrdering.key` raw syntax;
- `compositionSourceFixture` includes both port directions, both component-body
  constructors, populated parameter bindings, wires, exposures, hidden ports,
  source summaries and source-schema/`outerDt` fields;
- `sourceMapFixture` populates leaves, boundary aliases and hidden-port sections;
- `linkedPlanFixture` and `directPlanFixture` cover both origins, `some`/`none`
  linked provenance, identity sections, enabled features and governing version
  strings;
- `constructorCoverage`, `structureCoverage` and `coverageFixture` guard all
  218 item classifications and reject duplicate item names.

These fixtures do not call canonical encoders, hashing implementations or the
linker, and therefore do not extend the assurance boundary.

## Negative mutation evidence

The PRD 0002 implementation test temporarily added
`IR.ParamType.coverageSentinel`; building `Sembla.Semantics.Raw` failed at the
`classifyParamTypeConstructor` match with `missing cases:
IR.ParamType.coverageSentinel`. It then temporarily added
`IR.Attr.coverageSentinel`; the build failed at the positional
`classifyAttrFields` match because `IR.Attr.mk` had gained a third explicit
field. Both mutations were restored immediately. The final acceptance run
verifies zero diff and the original Git blob IDs for all four boundary modules.
This is mutation evidence for inventory exhaustiveness, not a persistent schema
change.

[raw-classifiers]: ../../frontend/Sembla/Semantics/Raw.lean
[raw-fixtures]: ../../frontend/Sembla/Semantics/RawTests.lean
