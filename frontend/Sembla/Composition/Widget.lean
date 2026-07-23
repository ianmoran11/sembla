import Lean.Data.Json.FromToJson
import Sembla.Composition.Source

/-!
Pure presentation props for composition structure widgets.

Builders consume already-elaborated composition values, preserve authored list
order, and never parse or re-elaborate source text.
-/
namespace Sembla.Composition.Widget

open Lean

structure CompositionNodeProps where
  instanceName : String
  definitionName : String
  kind : String
  ports : Array (String × String)
deriving Repr, BEq, Inhabited, ToJson

structure CompositionWireProps where
  source : String
  target : String
  delayTicks : Nat
deriving Repr, BEq, Inhabited, ToJson

structure CompositionExposureProps where
  inner : String
  outer : String
deriving Repr, BEq, Inhabited, ToJson

structure CompositionDiagramProps where
  title : String
  nodes : Array CompositionNodeProps
  wires : Array CompositionWireProps
  exposures : Array CompositionExposureProps
  hidden : Array String
deriving Repr, BEq, Inhabited, ToJson

private def displayNameFromSlug (slug : String) : String :=
  match (slug.replace "_" " ").toList with
  | [] => ""
  | first :: rest => String.mk (first.toUpper :: rest)

private def stableDisplayName (stablePrefix : String) (id : StableId) : String :=
  let slug := if id.raw.startsWith stablePrefix then id.raw.drop stablePrefix.length else id.raw
  displayNameFromSlug slug

private def directionName : PortDirection → String
  | .input => "input"
  | .output => "output"

private def portProps (definition : ComponentDefinitionV1) : Array (String × String) :=
  definition.ports.map (fun port => (port.displayName, directionName port.direction)) |>.toArray

private def definitionKind (definition : ComponentDefinitionV1) : String :=
  match definition.body with
  | .primitive _ => "primitive"
  | .composite _ => "composite"

private def findDefinition? (definitions : List ComponentDefinitionV1)
    (id : StableId) : Option ComponentDefinitionV1 :=
  definitions.find? (·.id == id)

private def nodeProps (definitions : List ComponentDefinitionV1)
    (instance_ : InstanceDeclV1) : CompositionNodeProps :=
  match findDefinition? definitions instance_.definition with
  | some definition => {
      instanceName := instance_.displayName
      definitionName := definition.displayName
      kind := definitionKind definition
      ports := portProps definition }
  | none => {
      instanceName := instance_.displayName
      definitionName := stableDisplayName "def:" instance_.definition
      kind := "composite"
      ports := #[] }

private def findInstance? (body : CompositeBodyV1)
    (id : StableId) : Option InstanceDeclV1 :=
  body.instances.find? (·.id == id)

private def endpointLabel (definitions : List ComponentDefinitionV1)
    (body : CompositeBodyV1) (instanceId portId : StableId) : String :=
  let instanceName := (findInstance? body instanceId).map (·.displayName)
    |>.getD (stableDisplayName "inst:" instanceId)
  let portName : Option String := do
    let instance_ ← findInstance? body instanceId
    let definition ← findDefinition? definitions instance_.definition
    let port ← definition.ports.find? (·.id == portId)
    pure port.displayName
  instanceName ++ "." ++ portName.getD (stableDisplayName "port:" portId)

private def outerPortLabel (definition : ComponentDefinitionV1) (id : StableId) : String :=
  (definition.ports.find? (·.id == id)).map (·.displayName)
    |>.getD (stableDisplayName "port:" id)

/-- Build a definition diagram with a catalog that can resolve direct children.
    Only the authored level is shown; child composites are never expanded. -/
def diagramPropsOfDefinitionWithDefinitions (definitions : List ComponentDefinitionV1)
    (definition : ComponentDefinitionV1) : CompositionDiagramProps :=
  match definition.body with
  | .primitive _ => {
      title := definition.displayName
      nodes := #[{
        instanceName := definition.displayName
        definitionName := definition.displayName
        kind := "primitive"
        ports := portProps definition }]
      wires := #[]
      exposures := #[]
      hidden := #[] }
  | .composite body => {
      title := definition.displayName
      nodes := body.instances.map (nodeProps definitions) |>.toArray
      wires := body.wires.map (fun wire => {
        source := endpointLabel definitions body wire.sourceInstance wire.sourcePort
        target := endpointLabel definitions body wire.targetInstance wire.targetPort
        delayTicks := wire.delayTicks }) |>.toArray
      exposures := body.exposures.map (fun exposure => {
        inner := endpointLabel definitions body exposure.innerInstance exposure.innerPort
        outer := outerPortLabel definition exposure.outerPort }) |>.toArray
      hidden := body.hiddenPorts.map (fun hidden =>
        endpointLabel definitions body hidden.instance_ hidden.port) |>.toArray }

/-- Build a diagram from one definition. Primitive definitions are complete.
    A composite's child ids remain a deterministic fallback when no surrounding
    definition catalog is available; command/source attachment uses the cataloged
    builder above. -/
def diagramPropsOfDefinition (definition : ComponentDefinitionV1) : CompositionDiagramProps :=
  diagramPropsOfDefinitionWithDefinitions [definition] definition

/-- Build the root definition's one-level diagram from an elaborated source. -/
def diagramPropsOfSource (source : CompositionSourceV1) : CompositionDiagramProps :=
  match findDefinition? source.definitions source.rootDefinition with
  | some root =>
      { diagramPropsOfDefinitionWithDefinitions source.definitions root with
        title := source.displayName }
  | none => {
      title := source.displayName
      nodes := #[]
      wires := #[]
      exposures := #[]
      hidden := #[] }

end Sembla.Composition.Widget
