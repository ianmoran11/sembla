import Lean.Elab.Command
import Sembla.DSL
import Sembla.Composition.Source

namespace Sembla.Composition.Surface

open Lean Elab Term Command
open Sembla Sembla.IR Sembla.DSL

/-!
# Composition command surface

Primitive component bodies are collected with
`Sembla.DSL.parseCommandBoxWithPorts` and elaborated only by
`Sembla.DSL.elaborateSurfaceModelNoWidgets`. This module adds
composition structure around that shared kernel; it intentionally contains no
second expression checker or primitive IR builder. Each authored component also
gets a conventionally named ordinary Lean constant containing its transitive
definition list. This is dependency metadata, not a registry or environment
extension; composition reuse and resolution still begin from the referenced
component constant.
-/

private def identText (stx : TSyntax `ident) : String := stx.getId.getString!

private partial def findAtomToken? (stx : Syntax) (value : String) : Option Syntax :=
  if stx.isAtom && stx.getAtomVal == value then some stx
  else stx.getArgs.toList.findSome? fun child => findAtomToken? child value

private def displayNameFromSlug (slug : String) : String :=
  match (slug.replace "_" " ").toList with
  | [] => ""
  | first :: rest => String.mk (first.toUpper :: rest)

private def componentDefinitionsName (name : Name) : Name :=
  name.appendAfter "__semblaDefinitions"

private def renameFrom (names : List (String × String)) (name : String) : String :=
  match names.find? (·.1 == name) with
  | some (_, runtimeName) => runtimeName
  | none => name

mutual
  private partial def renameInputExpr (names : List (String × String)) : IR.Expr → IR.Expr
    | .add lhs rhs => .add (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .sub lhs rhs => .sub (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .mul lhs rhs => .mul (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .div lhs rhs => .div (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .eq lhs rhs => .eq (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .ne lhs rhs => .ne (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .lt lhs rhs => .lt (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .le lhs rhs => .le (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .gt lhs rhs => .gt (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .ge lhs rhs => .ge (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .and lhs rhs => .and (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .or lhs rhs => .or (renameInputExpr names lhs) (renameInputExpr names rhs)
    | .not expression => .not (renameInputExpr names expression)
    | .input port aggregate =>
        .input (renameFrom names port) (renameInputAggregate names aggregate)
    | .agg operation table fk selfFk filter =>
        .agg (renameInputAggOp names operation) table fk selfFk
          (renameInputExpr names filter)
    | expression => expression

  private partial def renameInputAggOp (names : List (String × String)) : AggOp → AggOp
    | .count => .count
    | .sum value => .sum (renameInputExpr names value)

  private partial def renameInputAggregate
      (names : List (String × String)) : Aggregate → Aggregate
    | .mk operation filter =>
        .mk (renameInputAggOp names operation) (filter.map (renameInputExpr names))
end

private def normalizePrimitiveBox
    (inputNames outputNames : List (String × String)) (modelBox : Box) : Box :=
  let renameEffect : Effect → Effect
    | .setAttr attrName value => .setAttr attrName (renameInputExpr inputNames value)
  let renameOrdering : ClaimOrdering → ClaimOrdering
    | .raceTime => .raceTime
    | .key expression => .key (renameInputExpr inputNames expression)
  let renameClaim (claim : ResourceClaim) : ResourceClaim := {
    resource := renameInputExpr inputNames claim.resource
    ordering := renameOrdering claim.ordering }
  let renameField (outputField : OutputField) : OutputField := {
    outputField with
    op := renameInputAggOp inputNames outputField.op
    filter := outputField.filter.map (renameInputExpr inputNames) }
  { modelBox with
    «transitions» := modelBox.transitions.map fun transitionDecl => {
      transitionDecl with
      «guard» := renameInputExpr inputNames transitionDecl.guard
      «hazard» := renameInputExpr inputNames transitionDecl.hazard
      effects := transitionDecl.effects.map renameEffect
      contests := transitionDecl.contests.map renameClaim }
    «inputs» := modelBox.inputs.map fun inputDecl =>
      { inputDecl with name := renameFrom inputNames inputDecl.name }
    «outputs» := modelBox.outputs.map fun outputDecl =>
      let builder := match outputDecl.builder with
        | .perTable table outputFields => .perTable table (outputFields.map renameField)
      { outputDecl with name := renameFrom outputNames outputDecl.name, builder }
    «views» := modelBox.views.map fun viewDecl => {
      viewDecl with
      filter := viewDecl.filter.map (renameInputExpr inputNames)
      value := viewDecl.value.map (renameInputExpr inputNames) } }

private def emptyBox : Box := {
  name := ""
  tables := []
  «transitions» := []
  «inputs» := []
  «outputs» := []
  «views» := [] }

private def primitiveComponentFromModel
    (id displayName : String) (requirements : List String)
    (inputNames outputNames : List (String × String))
    (portOrder : List (Bool × String)) (model : Model) : ComponentDefinitionV1 :=
  let modelBox := normalizePrimitiveBox inputNames outputNames
    ((model.boxes.get? 0).getD emptyBox)
  let ports : List PortDeclV1 := portOrder.filterMap fun (isOutput, name) =>
    if isOutput then
      (modelBox.outputs.find? (·.name == name)).map fun outputDecl => {
        id := StableId.mk ("port:" ++ outputDecl.name)
        displayName := displayNameFromSlug outputDecl.name
        direction := PortDirection.output
        schema := outputDecl.schema }
    else
      (modelBox.inputs.find? (·.name == name)).map fun inputDecl => {
        id := StableId.mk ("port:" ++ inputDecl.name)
        displayName := displayNameFromSlug inputDecl.name
        direction := PortDirection.input
        schema := inputDecl.schema }
  {
    id := ⟨"def:" ++ id⟩
    displayName
    parameterRequirements := requirements
    ports
    body := .primitive {
      tables := modelBox.tables
      «transitions» := modelBox.transitions
      «inputs» := modelBox.inputs
      «outputs» := modelBox.outputs
      «views» := modelBox.views } }

structure AuthoredInstance where
  id : String
  displayName : String
  component : ComponentDefinitionV1
  parameterBindings : List ParameterBinding

structure AuthoredWire where
  id : String
  sourceInstance : String
  sourcePort : String
  targetInstance : String
  targetPort : String

structure AuthoredExposure where
  id : String
  innerInstance : String
  innerPort : String
  outerPort : String

structure AuthoredHiddenPort where
  instance_ : String
  port : String

private def authoredInstance? (instances : List AuthoredInstance)
    (id : String) : Option AuthoredInstance :=
  instances.find? (·.id == id)

private def authoredPort? (instance_ : AuthoredInstance)
    (port : String) : Option PortDeclV1 :=
  instance_.component.ports.find? (·.id.raw == "port:" ++ port)

private def compositeComponent
    (id displayName : String) (instances : List AuthoredInstance)
    (wireSpecs : List AuthoredWire) (exposures : List AuthoredExposure)
    (hiddenPorts : List AuthoredHiddenPort) : ComponentDefinitionV1 :=
  let requirements := instances.foldl (fun found instance_ =>
    instance_.parameterBindings.foldl (fun found binding =>
      if found.contains binding.parameter then found else found ++ [binding.parameter]) found) []
  let ports : List PortDeclV1 := exposures.filterMap fun exposure => do
    let instance_ ← authoredInstance? instances exposure.innerInstance
    let port ← authoredPort? instance_ exposure.innerPort
    pure {
      id := ⟨"port:" ++ exposure.outerPort⟩
      displayName := displayNameFromSlug exposure.outerPort
      direction := port.direction
      schema := port.schema }
  {
    id := ⟨"def:" ++ id⟩
    displayName
    parameterRequirements := requirements
    ports
    body := .composite {
      instances := instances.map fun instance_ => {
        id := ⟨"inst:" ++ instance_.id⟩
        displayName := instance_.displayName
        definition := instance_.component.id
        parameterBindings := instance_.parameterBindings }
      «wires» := wireSpecs.map fun wireSpec => {
        id := ⟨"wire:" ++ wireSpec.id⟩
        sourceInstance := ⟨"inst:" ++ wireSpec.sourceInstance⟩
        sourcePort := ⟨"port:" ++ wireSpec.sourcePort⟩
        targetInstance := ⟨"inst:" ++ wireSpec.targetInstance⟩
        targetPort := ⟨"port:" ++ wireSpec.targetPort⟩
        delayTicks := 1 }
      exposures := exposures.map fun exposure => {
        id := ⟨"expose:" ++ exposure.id⟩
        innerInstance := ⟨"inst:" ++ exposure.innerInstance⟩
        innerPort := ⟨"port:" ++ exposure.innerPort⟩
        outerPort := ⟨"port:" ++ exposure.outerPort⟩ }
      hiddenPorts := hiddenPorts.map fun hidden => {
        instance_ := ⟨"inst:" ++ hidden.instance_⟩
        port := ⟨"port:" ++ hidden.port⟩ } } }

private def mergeDefinitionLists
    (groups : List (List ComponentDefinitionV1))
    (root : ComponentDefinitionV1) : List ComponentDefinitionV1 :=
  (groups.join ++ [root]).foldl (fun found definition =>
    if found.any (·.id == definition.id) then found else found ++ [definition]) []

private def sourceDisplayName (declarationRuntimeName : String) : String :=
  let withoutModel := if declarationRuntimeName.endsWith "_model" then
    declarationRuntimeName.dropRight "_model".length
  else declarationRuntimeName
  displayNameFromSlug withoutModel

private def compositionSourceFromModel
    (modelSlug declarationRuntimeName : String) (model : Model)
    (definitions : List ComponentDefinitionV1) (root : ComponentDefinitionV1)
    (sourceSummaries : List SourceSummaryV1) : CompositionSourceV1 := {
  schemaVersion := Plan.compositionSourceSchema
  modelId := ⟨"model:" ++ modelSlug⟩
  displayName := sourceDisplayName declarationRuntimeName
  outerDt := model.dt
  parameters := model.params
  definitions
  rootDefinition := root.id
  requiredFeatures := []
  «summaries» := sourceSummaries }

private unsafe def evalComponentUnsafe (expr : Lean.Expr) : TermElabM ComponentDefinitionV1 :=
  Meta.evalExpr ComponentDefinitionV1 (mkConst ``ComponentDefinitionV1) expr

@[implemented_by evalComponentUnsafe]
private opaque evalComponent (expr : Lean.Expr) : TermElabM ComponentDefinitionV1

private structure ComponentReference where
  token : TSyntax `ident
  expr : Lean.Expr
  value : ComponentDefinitionV1
  definitionsName? : Option Name

private def elaborateComponentReference
    (token : TSyntax `ident) : TermElabM ComponentReference := do
  let expression ← elabTerm token (some (mkConst ``ComponentDefinitionV1))
  if expression.hasSyntheticSorry then throwAbortCommand
  synthesizeSyntheticMVarsNoPostponing
  let expression ← instantiateMVars expression
  let name ← match expression.getAppFn.constName? with
    | some name => pure name
    | none => throwErrorAt token
        "component reference must be a named ComponentDefinitionV1 constant"
  let value ← evalComponent expression
  let dependencies := componentDefinitionsName name
  let environment ← getEnv
  pure {
    token
    expr := expression
    value
    definitionsName? := if environment.contains dependencies then some dependencies else none }

private def tokenForComponent
    (endpoint : TSyntax `ident) (component : Name) (offset : Nat) : TSyntax `ident :=
  let base := Lean.mkIdentFrom endpoint component
  match endpoint.raw.getHeadInfo with
  | .original leading position trailing _ =>
      let start := String.Pos.mk (position.byteIdx + offset)
      let finish := String.Pos.mk (start.byteIdx + component.getString!.utf8ByteSize)
      let leadingText := if offset == 0 then leading else Substring.mk leading.str start start
      ⟨base.raw.setInfo (.original leadingText start trailing finish)⟩
  | _ => base

private def dottedComponents (endpoint : TSyntax `ident) : List (TSyntax `ident) :=
  let components := endpoint.getId.components
  let rec loop (offset : Nat) : List Name → List (TSyntax `ident)
    | [] => []
    | component :: rest =>
        tokenForComponent endpoint component offset ::
          loop (offset + component.getString!.utf8ByteSize + 1) rest
  loop 0 components

private def endpointPair (endpoint : TSyntax `ident) : TermElabM (TSyntax `ident × TSyntax `ident) := do
  match dottedComponents endpoint with
  | [instance_, port] => pure (instance_, port)
  | _ => throwErrorAt endpoint "component port must have the form 'instance.port'"

private def ensureUniqueNames
    (kind : String) (entries : List (String × Syntax)) : TermElabM Unit := do
  let mut seen : List String := []
  for (name, token) in entries do
    if seen.contains name then throwErrorAt token "duplicate {kind} '{name}'"
    seen := name :: seen

private def exactSlug (value : String) : Bool :=
  match value.toList with
  | [] => false
  | first :: rest =>
      first >= 'a' && first <= 'z' && rest.all fun character =>
        (character >= 'a' && character <= 'z') ||
        (character >= '0' && character <= '9') || character == '_'

private def addGeneratedDefinitionsList
    (name : Name) (value : Lean.Expr) : TermElabM Unit := do
  let listType := mkApp (mkConst ``List [0]) (mkConst ``ComponentDefinitionV1)
  let declaration : Declaration := .defnDecl {
    name
    levelParams := []
    type := listType
    value
    hints := .regular 0
    safety := .safe }
  Term.ensureNoUnassignedMVars declaration
  addAndCompile declaration

/- Command grammar. -/
declare_syntax_cat semblaComponentBinding
syntax ident ":=" ident : semblaComponentBinding

declare_syntax_cat semblaComponentReference
syntax ident : semblaComponentReference
syntax ident "(" semblaComponentBinding,* ")" : semblaComponentReference

declare_syntax_cat semblaComponentInstance
syntax "instance" ident ":=" semblaComponentReference : semblaComponentInstance

declare_syntax_cat semblaComponentItem
syntax "requires" ident : semblaComponentItem
syntax semblaCommandBoxItem : semblaComponentItem
syntax semblaComponentInstance : semblaComponentItem
syntax "wire" ident ":" ident "->" ident : semblaComponentItem
syntax "wire" ident "->" ident : semblaComponentItem
syntax "expose" ident ":" ident "as" ident : semblaComponentItem
syntax "hide" ident : semblaComponentItem
syntax semblaParam : semblaComponentItem
syntax "dt" ":=" term : semblaComponentItem

syntax (name := semblaComponentCommand) "sembla_component" ident "where"
  manyIndent(ppLine semblaComponentItem) : command
syntax (name := semblaNamedComponentCommand) "sembla_component" ident
  "(" ident ":=" str ")" "where" manyIndent(ppLine semblaComponentItem) : command
syntax (name := semblaComponentDtCommand) "sembla_component" ident
  "(" "dt" ":=" term ")" "where" manyIndent(ppLine semblaComponentItem) : command

declare_syntax_cat semblaCompositionReduce
syntax "sum" : semblaCompositionReduce
syntax "min" : semblaCompositionReduce
syntax "max" : semblaCompositionReduce
syntax "last" : semblaCompositionReduce
syntax "argmax_tick" : semblaCompositionReduce

declare_syntax_cat semblaCompositionItem
syntax semblaParam : semblaCompositionItem
syntax "root" ident : semblaCompositionItem
syntax "summary" ident ":=" semblaCompositionReduce term : semblaCompositionItem

syntax (name := semblaCompositionCommand) "sembla_composition" ident
  "(" ident ":=" str ")" "(" "dt" ":=" term ")" "where"
  manyIndent(ppLine semblaCompositionItem) : command
syntax (name := semblaCompositionMissingNameCommand) "sembla_composition" ident
  "(" "dt" ":=" term ")" "where" manyIndent(ppLine semblaCompositionItem) : command

private structure ParsedBinding where
  requirement : String
  parameterName : String
  requirementToken : Syntax

private def parseComponentReference
    (stx : TSyntax `semblaComponentReference) : TermElabM
      (TSyntax `ident × List ParsedBinding) := do
  let parseBinding (binding : TSyntax `semblaComponentBinding) := do
    match binding with
    | `(semblaComponentBinding| $requirement:ident := $targetParameter:ident) =>
        pure {
          requirement := ← deriveRuntimeNameAt requirement
          parameterName := ← deriveRuntimeNameAt targetParameter
          requirementToken := requirement.raw }
    | _ => throwUnsupportedSyntax
  match stx with
  | `(semblaComponentReference| $component:ident) => pure (component, [])
  | `(semblaComponentReference| $component:ident ($bindings:semblaComponentBinding,*)) =>
      pure (component, ← bindings.getElems.toList.mapM parseBinding)
  | _ => throwUnsupportedSyntax

private structure ParsedInstance where
  runtimeName : String
  token : Syntax
  reference : ComponentReference
  bindings : List ParameterBinding

private def finishBindings (reference : ComponentReference)
    (bindings : List ParsedBinding) : TermElabM (List ParameterBinding) := do
  ensureUniqueNames "parameter binding"
    (bindings.map fun binding => (binding.requirement, binding.requirementToken))
  for binding in bindings do
    unless reference.value.parameterRequirements.contains binding.requirement do
      throwErrorAt binding.requirementToken
        "component '{reference.value.id.raw}' has no requirement '{binding.requirement}'"
  pure (reference.value.parameterRequirements.map fun requirement =>
    match bindings.find? (·.requirement == requirement) with
    | some binding => { requirement := requirement, «parameter» := binding.parameterName }
    | none => { requirement := requirement, «parameter» := requirement })

private def componentItemIsPrimitive (item : TSyntax `semblaComponentItem) : Bool :=
  match item with
  | `(semblaComponentItem| $bodyItem:semblaCommandBoxItem) => !bodyItem.raw.isMissing
  | _ => false

private def componentItemIsComposite (item : TSyntax `semblaComponentItem) : Bool :=
  if componentItemIsPrimitive item then false
  else match item with
  | `(semblaComponentItem| requires $_name:ident) => false
  | `(semblaComponentItem| $_paramDecl:semblaParam) => false
  | `(semblaComponentItem| dt := $_value:term) => false
  | _ => true

private def defineComponent (declaration : TSyntax `ident)
    (displayOverride : Option (TSyntax `str))
    (items : List (TSyntax `semblaComponentItem)) : CommandElabM Unit := do
  let currentNamespace ← getCurrNamespace
  let declarationName := currentNamespace ++ declaration.getId
  checkNotAlreadyDeclared declarationName
  Command.runTermElabM fun _ => Term.withDeclName declarationName do
    let runtimeName ← deriveRuntimeNameAt declaration
    let displayName := displayOverride.map (·.getString) |>.getD
      (displayNameFromSlug runtimeName)
    let hasPrimitive := items.any componentItemIsPrimitive
    let hasComposite := items.any componentItemIsComposite
    if hasPrimitive && hasComposite then
      match items.find? componentItemIsComposite with
      | some offending => throwErrorAt offending
          "primitive and composite component declarations cannot be mixed"
      | none => throwErrorAt declaration
          "primitive and composite component declarations cannot be mixed"
    if !hasPrimitive && !hasComposite then
      throwErrorAt declaration "sembla_component requires a primitive body or an instance"

    let (value, dependencyTerms) ← if hasPrimitive then do
      let mut requirements : List (String × String × Syntax) := []
      let mut boxItems : Array (TSyntax `semblaCommandBoxItem) := #[]
      for item in items do
        match item with
        | `(semblaComponentItem| requires $name:ident) =>
            requirements := requirements ++ [
              (identText name, ← deriveRuntimeNameAt name, name.raw)]
        | `(semblaComponentItem| $bodyItem:semblaCommandBoxItem) =>
            boxItems := boxItems.push bodyItem
        | `(semblaComponentItem| $paramDecl:semblaParam) =>
            throwErrorAt paramDecl "param declarations are not allowed on components"
        | `(semblaComponentItem| dt := $value:term) =>
            throwErrorAt ((findAtomToken? item.raw "dt").getD value.raw)
              "dt is not allowed on a component"
        | _ => throwUnsupportedSyntax
      ensureUniqueNames "parameter requirement"
        (requirements.map fun requirement => (requirement.2.1, requirement.2.2))
      let synthetic ← `(semblaCommandBox| box $declaration where $boxItems*)
      let collectedBox ← parseCommandBoxWithPorts synthetic
      let parsedBox := { collectedBox.surfaceBox with name := runtimeName }
      let inputNames ← parsedBox.inputs.mapM fun inputDecl => do
        pure (inputDecl.name, ← deriveRuntimeNameAt ⟨inputDecl.token⟩)
      let outputNames ← parsedBox.outputs.mapM fun outputDecl => do
        pure (outputDecl.name, ← deriveRuntimeNameAt ⟨outputDecl.token⟩)
      ensureUniqueNames "input port runtime name"
        (inputNames.zip parsedBox.inputs |>.map fun (entry, inputDecl) => (entry.2, inputDecl.token))
      ensureUniqueNames "output port runtime name"
        (outputNames.zip parsedBox.outputs |>.map fun (entry, outputDecl) => (entry.2, outputDecl.token))
      let portOrder ← collectedBox.ports.mapM fun
        | .input inputDecl => do
            pure (false, ← deriveRuntimeNameAt ⟨inputDecl.token⟩)
        | .output outputDecl => do
            pure (true, ← deriveRuntimeNameAt ⟨outputDecl.token⟩)
      let dummyDefault ← `(term| 0.0)
      let requirementParams : List SurfaceParam := requirements.map fun requirement => {
        sourceName := requirement.1
        name := requirement.2.1
        token := requirement.2.2
        «default» := dummyDefault
        «prior» := none }
      let dummyStep ← `(term| 1.0)
      let surface : SurfaceModel := {
        declarationName := identText declaration
        declarationToken := declaration.raw
        runtimeName := some (runtimeName, declaration.raw)
        «dt» := dummyStep
        «params» := requirementParams
        «boxes» := [parsedBox]
        «wires» := []
        «summaries» := [] }
      let modelExpression ← elaborateSurfaceModelNoWidgets surface fun result =>
        elabTerm result (some (mkConst ``Model))
      let requirementsExpression := Lean.toExpr (requirements.map (·.2.1))
      let inputsExpression := Lean.toExpr inputNames
      let outputsExpression := Lean.toExpr outputNames
      let portOrderExpression := Lean.toExpr portOrder
      let application ← Meta.mkAppM ``primitiveComponentFromModel #[
        Lean.toExpr runtimeName, Lean.toExpr displayName, requirementsExpression,
        inputsExpression, outputsExpression, portOrderExpression, modelExpression]
      pure (application, #[])
    else do
      let mut dependencyTerms : Array (TSyntax `term) := #[]
      for item in items do
        match item with
        | `(semblaComponentItem| requires $name:ident) =>
            throwErrorAt ((findAtomToken? item.raw "requires").getD name.raw)
              "requires declarations are not allowed on composite components"
        | _ => pure ()

      let mut parsedInstances : List ParsedInstance := []
      for item in items do
        match item with
        | `(semblaComponentItem| $instanceDecl:semblaComponentInstance) =>
            match instanceDecl.raw.getArgs with
            | #[_, nameSyntax, _, componentSyntax] =>
                let name : TSyntax `ident := ⟨nameSyntax⟩
                let component : TSyntax `semblaComponentReference := ⟨componentSyntax⟩
                let runtime ← deriveRuntimeNameAt name
                let (componentToken, explicitBindings) ← parseComponentReference component
                let reference ← elaborateComponentReference componentToken
                let bindings ← finishBindings reference explicitBindings
                parsedInstances := parsedInstances ++ [{
                  runtimeName := runtime
                  token := name.raw
                  reference
                  bindings }]
            | _ => throwUnsupportedSyntax
        | _ => pure ()
      let resolvedInstances := parsedInstances
      ensureUniqueNames "instance"
        (resolvedInstances.map fun instance_ => (instance_.runtimeName, instance_.token))
      let lookupInstance (token : TSyntax `ident) := do
        let runtime ← deriveRuntimeNameAt token
        match resolvedInstances.find? (·.runtimeName == runtime) with
        | some instance_ => pure instance_
        | none => throwErrorAt token "unknown component instance '{runtime}'"
      let lookupPort (instance_ : ParsedInstance) (token : TSyntax `ident) := do
        let runtime ← deriveRuntimeNameAt token
        match instance_.reference.value.ports.find? (·.id.raw == "port:" ++ runtime) with
        | some port => pure (runtime, port)
        | none => throwErrorAt token
            "unknown port '{instance_.runtimeName}.{runtime}'"

      let mut instanceTerms : Array (TSyntax `term) := #[]
      for instance_ in resolvedInstances do
        let component : TSyntax `term := ⟨instance_.reference.token.raw⟩
        let mut bindingTerms : Array (TSyntax `term) := #[]
        for binding in instance_.bindings do
          bindingTerms := bindingTerms.push (← `(ParameterBinding.mk
            $(Lean.quote binding.requirement) $(Lean.quote binding.parameter)))
        instanceTerms := instanceTerms.push (← `(AuthoredInstance.mk
          $(Lean.quote instance_.runtimeName)
          $(Lean.quote (displayNameFromSlug instance_.runtimeName))
          $component [$bindingTerms,*]))
        let dependencyTerm ← match instance_.reference.definitionsName? with
          | some name => pure (⟨mkIdent name⟩ : TSyntax `term)
          | none => `(term| [$component])
        dependencyTerms := dependencyTerms.push dependencyTerm

      let mut wireNames : List (String × Syntax) := []
      let mut wireTerms : Array (TSyntax `term) := #[]
      let mut exposureNames : List (String × Syntax) := []
      let mut outerPortNames : List (String × Syntax) := []
      let mut exposureTerms : Array (TSyntax `term) := #[]
      let mut hiddenTerms : Array (TSyntax `term) := #[]
      for item in items do
        match item with
        | `(semblaComponentItem| wire $label:ident : $source:ident -> $target:ident) =>
            let wireName ← deriveRuntimeNameAt label
            wireNames := wireNames ++ [(wireName, label.raw)]
            let (sourceInstanceToken, sourcePortToken) ← endpointPair source
            let (targetInstanceToken, targetPortToken) ← endpointPair target
            let sourceInstance ← lookupInstance sourceInstanceToken
            let targetInstance ← lookupInstance targetInstanceToken
            let (sourcePort, sourceDecl) ← lookupPort sourceInstance sourcePortToken
            let (targetPort, targetDecl) ← lookupPort targetInstance targetPortToken
            unless sourceDecl.direction == .output do
              throwErrorAt sourcePortToken
                "wire source port '{sourceInstance.runtimeName}.{sourcePort}' is not an output"
            unless targetDecl.direction == .input do
              throwErrorAt targetPortToken
                "wire target port '{targetInstance.runtimeName}.{targetPort}' is not an input"
            wireTerms := wireTerms.push (← `(AuthoredWire.mk
              $(Lean.quote wireName) $(Lean.quote sourceInstance.runtimeName)
              $(Lean.quote sourcePort) $(Lean.quote targetInstance.runtimeName)
              $(Lean.quote targetPort)))
        | `(semblaComponentItem| wire $source:ident -> $target:ident) =>
            if target.raw.isMissing then throwUnsupportedSyntax
            throwErrorAt ((findAtomToken? item.raw "wire").getD source.raw)
              "wire declarations require an explicit label before ':'"
        | `(semblaComponentItem| expose $label:ident : $inner:ident as $outer:ident) =>
            let exposureName ← deriveRuntimeNameAt label
            let outerName ← deriveRuntimeNameAt outer
            exposureNames := exposureNames ++ [(exposureName, label.raw)]
            outerPortNames := outerPortNames ++ [(outerName, outer.raw)]
            let (instanceToken, portToken) ← endpointPair inner
            let instance_ ← lookupInstance instanceToken
            let (portName, _) ← lookupPort instance_ portToken
            exposureTerms := exposureTerms.push (← `(AuthoredExposure.mk
              $(Lean.quote exposureName) $(Lean.quote instance_.runtimeName)
              $(Lean.quote portName) $(Lean.quote outerName)))
        | `(semblaComponentItem| hide $inner:ident) =>
            let (instanceToken, portToken) ← endpointPair inner
            let instance_ ← lookupInstance instanceToken
            let (portName, _) ← lookupPort instance_ portToken
            hiddenTerms := hiddenTerms.push (← `(AuthoredHiddenPort.mk
              $(Lean.quote instance_.runtimeName) $(Lean.quote portName)))
        | `(semblaComponentItem| $paramDecl:semblaParam) =>
            throwErrorAt paramDecl "param declarations are not allowed on composite components"
        | `(semblaComponentItem| dt := $value:term) =>
            throwErrorAt ((findAtomToken? item.raw "dt").getD value.raw)
              "dt is not allowed on a component"
        | _ => pure ()
      ensureUniqueNames "wire label" wireNames
      ensureUniqueNames "exposure label" exposureNames
      ensureUniqueNames "outer port" outerPortNames
      let instanceExpressions ← instanceTerms.toList.mapM fun (term : TSyntax `term) =>
        elabTerm term (some (mkConst ``AuthoredInstance))
      let wireExpressions ← wireTerms.toList.mapM fun (term : TSyntax `term) =>
        elabTerm term (some (mkConst ``AuthoredWire))
      let exposureExpressions ← exposureTerms.toList.mapM fun (term : TSyntax `term) =>
        elabTerm term (some (mkConst ``AuthoredExposure))
      let hiddenExpressions ← hiddenTerms.toList.mapM fun (term : TSyntax `term) =>
        elabTerm term (some (mkConst ``AuthoredHiddenPort))
      let instanceList ← Meta.mkListLit (mkConst ``AuthoredInstance) instanceExpressions
      let wireList ← Meta.mkListLit (mkConst ``AuthoredWire) wireExpressions
      let exposureList ← Meta.mkListLit (mkConst ``AuthoredExposure) exposureExpressions
      let hiddenList ← Meta.mkListLit (mkConst ``AuthoredHiddenPort) hiddenExpressions
      let compositeValue ← Meta.mkAppM ``compositeComponent #[
        Lean.toExpr runtimeName, Lean.toExpr displayName, instanceList, wireList,
        exposureList, hiddenList]
      pure (compositeValue, dependencyTerms)

    let value ← instantiateMVars value
    let componentDeclaration : Declaration := .defnDecl {
      name := declarationName
      levelParams := []
      type := mkConst ``ComponentDefinitionV1
      value
      hints := .regular 0
      safety := .safe }
    Term.ensureNoUnassignedMVars componentDeclaration
    addAndCompile componentDeclaration
    Term.addTermInfo' declaration (mkConst declarationName) (isBinder := true)

    let dependencyGroups ← `(term| [$dependencyTerms,*])
    let componentIdentifier : TSyntax `term := ⟨mkIdent declarationName⟩
    let definitionsTerm ← `(term| mergeDefinitionLists $dependencyGroups $componentIdentifier)
    let definitionsValue ← elabTerm definitionsTerm
      (some (mkApp (mkConst ``List [0]) (mkConst ``ComponentDefinitionV1)))
    addGeneratedDefinitionsList (componentDefinitionsName declarationName)
      (← instantiateMVars definitionsValue)

@[command_elab semblaComponentCommand] private def elabSemblaComponent : CommandElab := fun stx => do
  match stx with
  | `(command| sembla_component $declaration:ident where $items:semblaComponentItem*) =>
      defineComponent declaration none items.toList
  | _ => throwUnsupportedSyntax

@[command_elab semblaNamedComponentCommand] private def elabNamedSemblaComponent : CommandElab := fun stx => do
  match stx with
  | `(command| sembla_component $declaration:ident
        ($keyword:ident := $displayName:str) where $items:semblaComponentItem*) =>
      unless identText keyword == "display" do
        throwErrorAt keyword "expected 'display' component metadata"
      defineComponent declaration (some displayName) items.toList
  | _ => throwUnsupportedSyntax

@[command_elab semblaComponentDtCommand] private def elabComponentDt : CommandElab := fun stx => do
  match stx with
  | `(command| sembla_component $_declaration:ident (dt := $value:term) where
        $_items:semblaComponentItem*) =>
      throwErrorAt ((findAtomToken? stx "dt").getD value.raw)
        "dt is not allowed on a component"
  | _ => throwUnsupportedSyntax

private def parseCompositionReduce
    (stx : TSyntax `semblaCompositionReduce) : TermElabM (TSyntax `term) := do
  match stx with
  | `(semblaCompositionReduce| sum) => `(term| SummaryReduce.sum)
  | `(semblaCompositionReduce| min) => `(term| SummaryReduce.min)
  | `(semblaCompositionReduce| max) => `(term| SummaryReduce.max)
  | `(semblaCompositionReduce| last) => `(term| SummaryReduce.last)
  | `(semblaCompositionReduce| argmax_tick) => `(term| SummaryReduce.argmaxTick)
  | _ => throwUnsupportedSyntax

private def defineComposition (declaration : TSyntax `ident) (modelName : TSyntax `str)
    (stepWidth : TSyntax `term) (items : List (TSyntax `semblaCompositionItem)) :
    CommandElabM Unit := do
  let currentNamespace ← getCurrNamespace
  let declarationName := currentNamespace ++ declaration.getId
  checkNotAlreadyDeclared declarationName
  Command.runTermElabM fun _ => Term.withDeclName declarationName do
    unless exactSlug modelName.getString do
      throwErrorAt modelName "composition name must be an exact lowercase slug"
    let declarationRuntimeName ← deriveRuntimeNameAt declaration
    let mut parameters : List SurfaceParam := []
    let mut rootToken : Option (TSyntax `ident) := none
    let mut summaryItems : List
      (TSyntax `ident × TSyntax `semblaCompositionReduce × TSyntax `term) := []
    for item in items do
      match item with
      | `(semblaCompositionItem| $paramDecl:semblaParam) =>
          parameters := parameters ++ [← parseSurfaceParam paramDecl]
      | `(semblaCompositionItem| root $component:ident) =>
          if rootToken.isSome then throwErrorAt component "duplicate root declaration"
          rootToken := some component
      | item =>
          match item.raw.getArgs with
          | #[summaryKeyword, nameSyntax, _, reduceSyntax, endpointSyntax] =>
              unless summaryKeyword.isAtom && summaryKeyword.getAtomVal == "summary" do
                throwUnsupportedSyntax
              let name : TSyntax `ident := ⟨nameSyntax⟩
              let reducer : TSyntax `semblaCompositionReduce := ⟨reduceSyntax⟩
              let endpoint : TSyntax `term := ⟨endpointSyntax⟩
              summaryItems := summaryItems ++ [(name, reducer, endpoint)]
          | _ => throwUnsupportedSyntax
    let rootComponentToken ← rootToken.getDM
      (throwErrorAt declaration "sembla_composition requires exactly one root")
    ensureUniqueNames "summary" (summaryItems.map fun item =>
      (identText item.1, item.1.raw))
    let rootRef ← elaborateComponentReference rootComponentToken
    let stepSurface : SurfaceModel := {
      declarationName := identText declaration
      declarationToken := declaration.raw
      runtimeName := some (modelName.getString, modelName.raw)
      «dt» := stepWidth
      «params» := parameters
      «boxes» := []
      «wires» := []
      «summaries» := [] }
    let modelExpression ← elaborateSurfaceModelNoWidgets stepSurface fun result =>
      elabTerm result (some (mkConst ``Model))

    let mut summaryTerms : Array (TSyntax `term) := #[]
    for (name, reducer, endpoint) in summaryItems do
      let path ← match endpoint with
        | `(term| $path:ident) => pure path
        | _ => throwErrorAt endpoint
            "summary source must contain only identifier path segments"
      let components := dottedComponents path
      if components.length < 2 then
        throwErrorAt endpoint
          "summary source must have the form 'instance[.instance...].view'"
      let instanceTokens := components.dropLast
      let viewToken := components.getLast!
      let mut instanceTerms : Array (TSyntax `term) := #[]
      for instanceToken in instanceTokens do
        let runtime ← deriveRuntimeNameAt instanceToken
        instanceTerms := instanceTerms.push (← `(⟨$(Lean.quote ("inst:" ++ runtime))⟩))
      let reduceTerm ← parseCompositionReduce reducer
      let summaryName ← deriveRuntimeNameAt name
      summaryTerms := summaryTerms.push (← `(SourceSummaryV1.mk
        $(Lean.quote summaryName) $reduceTerm [$instanceTerms,*]
        $(Lean.quote (identText viewToken))))

    let definitionsExpression ← match rootRef.definitionsName? with
      | some name => pure (Lean.mkConst name)
      | none => Meta.mkListLit (Lean.mkConst ``ComponentDefinitionV1) [rootRef.expr]
    let summariesTerm ← `(term| [$summaryTerms,*])
    let summariesExpression ← elabTerm summariesTerm
      (some (mkApp (mkConst ``List [0]) (mkConst ``SourceSummaryV1)))
    let value ← Meta.mkAppM ``compositionSourceFromModel #[
      Lean.toExpr modelName.getString, Lean.toExpr declarationRuntimeName,
      modelExpression, definitionsExpression, rootRef.expr, summariesExpression]
    let value ← instantiateMVars value
    let sourceDeclaration : Declaration := .defnDecl {
      name := declarationName
      levelParams := []
      type := mkConst ``CompositionSourceV1
      value
      hints := .regular 0
      safety := .safe }
    Term.ensureNoUnassignedMVars sourceDeclaration
    addAndCompile sourceDeclaration
    Term.addTermInfo' declaration (mkConst declarationName) (isBinder := true)

@[command_elab semblaCompositionCommand] private def elabSemblaComposition : CommandElab := fun stx => do
  match stx with
  | `(command| sembla_composition $declaration:ident
        ($keyword:ident := $modelName:str) (dt := $width:term) where
        $items:semblaCompositionItem*) =>
      unless identText keyword == "name" do
        throwErrorAt keyword "expected 'name' composition metadata"
      defineComposition declaration modelName width items.toList
  | _ => throwUnsupportedSyntax

@[command_elab semblaCompositionMissingNameCommand] private def elabMissingCompositionName :
    CommandElab := fun stx => do
  match stx with
  | `(command| sembla_composition $declaration:ident (dt := $_step:term) where
        $_items:semblaCompositionItem*) =>
      throwErrorAt declaration "sembla_composition requires '(name := <exact slug>)'"
  | _ => throwUnsupportedSyntax

end Sembla.Composition.Surface
