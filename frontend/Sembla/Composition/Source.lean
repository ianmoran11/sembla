import Std
import Sembla.IR
import Sembla.Plan

namespace Sembla.Composition

open Sembla

structure StableId where
  raw : String
deriving Repr, BEq, Ord

inductive PortDirection where
  | input
  | output
deriving Repr, BEq

structure PortDeclV1 where
  id : StableId
  displayName : String
  direction : PortDirection
  /- The PRD prose requires the existing `(name, type)` port schema shape.
     In the current IR that shape is `IR.Attr`; `IR.OutputField` instead stores
     `(name, aggregation, filter)` and cannot equal an input/output schema. -/
  schema : List IR.Attr
deriving Repr, BEq

structure ParameterBinding where
  requirement : String
  parameter : String
deriving Repr, BEq

structure InstanceDeclV1 where
  id : StableId
  displayName : String
  definition : StableId
  parameterBindings : List ParameterBinding
deriving Repr, BEq

structure WireDeclV1 where
  id : StableId
  sourceInstance : StableId
  sourcePort : StableId
  targetInstance : StableId
  targetPort : StableId
  delayTicks : Nat
deriving Repr, BEq

structure ExposureDeclV1 where
  id : StableId
  innerInstance : StableId
  innerPort : StableId
  outerPort : StableId
deriving Repr, BEq

structure HiddenPortV1 where
  instance_ : StableId
  port : StableId
deriving Repr, BEq

structure PrimitiveBodyV1 where
  tables : List IR.Table
  transitions : List IR.Transition
  inputs : List IR.PortDecl
  outputs : List IR.OutputDecl
  views : List IR.ViewDecl
deriving Repr, BEq

structure CompositeBodyV1 where
  instances : List InstanceDeclV1
  wires : List WireDeclV1
  exposures : List ExposureDeclV1
  hiddenPorts : List HiddenPortV1
deriving Repr, BEq

inductive ComponentBodyV1 where
  | primitive (body : PrimitiveBodyV1)
  | composite (body : CompositeBodyV1)
deriving Repr, BEq

structure ComponentDefinitionV1 where
  id : StableId
  displayName : String
  parameterRequirements : List String
  ports : List PortDeclV1
  body : ComponentBodyV1
deriving Repr, BEq

structure SourceSummaryV1 where
  name : String
  reduce : IR.SummaryReduce
  instancePath : List StableId
  view : String
deriving Repr, BEq

structure CompositionSourceV1 where
  schemaVersion : String
  modelId : StableId
  displayName : String
  outerDt : IR.Scientific
  parameters : List IR.ParamDecl
  definitions : List ComponentDefinitionV1
  rootDefinition : StableId
  requiredFeatures : List String
  summaries : List SourceSummaryV1
deriving Repr, BEq

private def isSlug (value : String) : Bool :=
  match value.toList with
  | [] => false
  | first :: rest =>
      first >= 'a' && first <= 'z' && rest.all fun character =>
        (character >= 'a' && character <= 'z') ||
        (character >= '0' && character <= '9') || character == '_'

private def validateStableId
    (path expectedPrefix : String) (id : StableId) : Except String Unit := do
  if !id.raw.startsWith expectedPrefix then
    throw s!"{path}: expected '{expectedPrefix}' stable id; got '{id.raw}'"
  let payload := id.raw.drop expectedPrefix.length
  if !isSlug payload then
    throw s!"{path}: payload '{payload}' is not a slug"

private def ensureUnique
    (kind : String) (entries : List (String × String)) : Except String Unit :=
  let rec loop (seen : List String) : List (String × String) → Except String Unit
    | [] => pure ()
    | (path, value) :: rest => do
        if seen.contains value then
          throw s!"{path}: duplicate {kind} id '{value}'"
        loop (value :: seen) rest
  loop [] entries

private def indexedPaths
    (base field : String) (values : List α) (get : α → String) : List (String × String) :=
  let rec loop (index : Nat) : List α → List (String × String)
    | [] => []
    | value :: rest =>
        (s!"{base}.{field}[{index}].id", get value) :: loop (index + 1) rest
  loop 0 values

private def validatePrimitivePorts
    (path : String) (definition : ComponentDefinitionV1)
    (body : PrimitiveBodyV1) : Except String Unit := do
  let rec checkPorts (index : Nat) : List PortDeclV1 → Except String Unit
    | [] => pure ()
    | port :: rest => do
        let portPath := s!"{path}.ports[{index}]"
        let name := port.id.raw.drop "port:".length
        match port.direction with
        | .input =>
            match body.inputs.find? fun input => input.name == name with
            | none => throw s!"{portPath}: port '{port.id.raw}' has no matching primitive input"
            | some input =>
                if port.schema != input.schema then
                  throw s!"{portPath}.schema: does not match primitive input '{name}' schema"
        | .output =>
            match body.outputs.find? fun output => output.name == name with
            | none => throw s!"{portPath}: port '{port.id.raw}' has no matching primitive output"
            | some output =>
                if port.schema != output.schema then
                  throw s!"{portPath}.schema: does not match primitive output '{name}' schema"
        checkPorts (index + 1) rest
  checkPorts 0 definition.ports
  let bodyPortCount := body.inputs.length + body.outputs.length
  if definition.ports.length != bodyPortCount then
    throw s!"{path}.ports: expected exactly {bodyPortCount} primitive port declarations; got {definition.ports.length}"
  let rec checkInputs (index : Nat) : List IR.PortDecl → Except String Unit
    | [] => pure ()
    | input :: rest => do
        let id := "port:" ++ input.name
        unless definition.ports.any fun port =>
            port.id.raw == id && port.direction == .input do
          throw s!"{path}.inputs[{index}]: no matching declared input port '{id}'"
        checkInputs (index + 1) rest
  checkInputs 0 body.inputs
  let rec checkOutputs (index : Nat) : List IR.OutputDecl → Except String Unit
    | [] => pure ()
    | output :: rest => do
        let id := "port:" ++ output.name
        unless definition.ports.any fun port =>
            port.id.raw == id && port.direction == .output do
          throw s!"{path}.outputs[{index}]: no matching declared output port '{id}'"
        checkOutputs (index + 1) rest
  checkOutputs 0 body.outputs

private def ensureDeclaredInstance
    (path : String) (instances : List InstanceDeclV1) (id : StableId) : Except String Unit := do
  unless instances.any fun item => item.id == id do
    throw s!"{path}: undeclared instance '{id.raw}'"

private def validateCompositeReferences
    (path : String) (body : CompositeBodyV1) : Except String Unit := do
  let rec checkWires (index : Nat) : List WireDeclV1 → Except String Unit
    | [] => pure ()
    | wire :: rest => do
        let wirePath := s!"{path}.wires[{index}]"
        ensureDeclaredInstance (wirePath ++ ".source_instance") body.instances wire.sourceInstance
        ensureDeclaredInstance (wirePath ++ ".target_instance") body.instances wire.targetInstance
        checkWires (index + 1) rest
  checkWires 0 body.wires
  let rec checkExposures (index : Nat) : List ExposureDeclV1 → Except String Unit
    | [] => pure ()
    | exposure :: rest => do
        ensureDeclaredInstance
          (s!"{path}.exposures[{index}].inner_instance") body.instances exposure.innerInstance
        checkExposures (index + 1) rest
  checkExposures 0 body.exposures
  let rec checkHidden (index : Nat) : List HiddenPortV1 → Except String Unit
    | [] => pure ()
    | hidden :: rest => do
        ensureDeclaredInstance
          (s!"{path}.hidden_ports[{index}].instance") body.instances hidden.instance_
        checkHidden (index + 1) rest
  checkHidden 0 body.hiddenPorts

private def validateDefinition
    (index : Nat) (definition : ComponentDefinitionV1) : Except String Unit := do
  let path := s!"definitions[{index}]"
  validateStableId (path ++ ".id") "def:" definition.id
  ensureUnique "port" (indexedPaths path "ports" definition.ports (·.id.raw))
  let rec checkPorts (portIndex : Nat) : List PortDeclV1 → Except String Unit
    | [] => pure ()
    | port :: rest => do
        validateStableId (s!"{path}.ports[{portIndex}].id") "port:" port.id
        checkPorts (portIndex + 1) rest
  checkPorts 0 definition.ports
  match definition.body with
  | .primitive body =>
      validatePrimitivePorts path definition body
  | .composite body =>
      ensureUnique "instance" (indexedPaths path "instances" body.instances (·.id.raw))
      ensureUnique "wire" (indexedPaths path "wires" body.wires (·.id.raw))
      ensureUnique "exposure" (indexedPaths path "exposures" body.exposures (·.id.raw))
      let rec checkInstances (itemIndex : Nat) : List InstanceDeclV1 → Except String Unit
        | [] => pure ()
        | item :: rest => do
            let itemPath := s!"{path}.instances[{itemIndex}]"
            validateStableId (itemPath ++ ".id") "inst:" item.id
            validateStableId (itemPath ++ ".definition") "def:" item.definition
            checkInstances (itemIndex + 1) rest
      checkInstances 0 body.instances
      let rec checkWires (itemIndex : Nat) : List WireDeclV1 → Except String Unit
        | [] => pure ()
        | wire :: rest => do
            let itemPath := s!"{path}.wires[{itemIndex}]"
            validateStableId (itemPath ++ ".id") "wire:" wire.id
            validateStableId (itemPath ++ ".source_instance") "inst:" wire.sourceInstance
            validateStableId (itemPath ++ ".source_port") "port:" wire.sourcePort
            validateStableId (itemPath ++ ".target_instance") "inst:" wire.targetInstance
            validateStableId (itemPath ++ ".target_port") "port:" wire.targetPort
            if wire.delayTicks != 1 then
              throw s!"{itemPath}.delay_ticks: V1 requires exactly 1"
            checkWires (itemIndex + 1) rest
      checkWires 0 body.wires
      let rec checkExposures (itemIndex : Nat) : List ExposureDeclV1 → Except String Unit
        | [] => pure ()
        | exposure :: rest => do
            let itemPath := s!"{path}.exposures[{itemIndex}]"
            validateStableId (itemPath ++ ".id") "expose:" exposure.id
            validateStableId (itemPath ++ ".inner_instance") "inst:" exposure.innerInstance
            validateStableId (itemPath ++ ".inner_port") "port:" exposure.innerPort
            validateStableId (itemPath ++ ".outer_port") "port:" exposure.outerPort
            checkExposures (itemIndex + 1) rest
      checkExposures 0 body.exposures
      let rec checkHidden (itemIndex : Nat) : List HiddenPortV1 → Except String Unit
        | [] => pure ()
        | hidden :: rest => do
            let itemPath := s!"{path}.hidden_ports[{itemIndex}]"
            validateStableId (itemPath ++ ".instance") "inst:" hidden.instance_
            validateStableId (itemPath ++ ".port") "port:" hidden.port
            checkHidden (itemIndex + 1) rest
      checkHidden 0 body.hiddenPorts
      validateCompositeReferences path body

/-- Check the source-V1 rules that do not require resolving component definitions. -/
def wellFormed (source : CompositionSourceV1) : Except String Unit := do
  if source.schemaVersion != Plan.compositionSourceSchema then
    throw s!"schema_version: expected '{Plan.compositionSourceSchema}'; got '{source.schemaVersion}'"
  match source.requiredFeatures with
  | feature :: _ => throw s!"required_features[0]: unsupported feature '{feature}'"
  | [] => pure ()
  validateStableId "model_id" "model:" source.modelId
  validateStableId "root_definition" "def:" source.rootDefinition
  ensureUnique "definition"
    (indexedPaths "" "definitions" source.definitions (·.id.raw) |>.map fun entry =>
      (entry.1.drop 1, entry.2))
  let rec checkDefinitions (index : Nat) : List ComponentDefinitionV1 → Except String Unit
    | [] => pure ()
    | definition :: rest => do
        validateDefinition index definition
        checkDefinitions (index + 1) rest
  checkDefinitions 0 source.definitions
  let rec checkSummaries (index : Nat) : List SourceSummaryV1 → Except String Unit
    | [] => pure ()
    | summary :: rest => do
        let rec checkPath (pathIndex : Nat) : List StableId → Except String Unit
          | [] => pure ()
          | id :: tail => do
              validateStableId
                (s!"summaries[{index}].instance_path[{pathIndex}]") "inst:" id
              checkPath (pathIndex + 1) tail
        checkPath 0 summary.instancePath
        checkSummaries (index + 1) rest
  checkSummaries 0 source.summaries

end Sembla.Composition
