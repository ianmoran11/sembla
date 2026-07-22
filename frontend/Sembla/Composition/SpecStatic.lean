import Sembla.Composition.Errors
import Sembla.Hash

namespace Sembla.Composition

open Sembla

/-!
# Independent static composition denotations

`denoteSourceStatic` deliberately re-derives expansion, transitive boundary
resolution, wire/mailbox identities, and transition identities by structural
recursion over `CompositionSourceV1`. It does not call `linkV1` and does not
share the linker's expansion helpers. The duplication is the independent check:
a future refactor must not deduplicate these derivations.

`denotePlanStatic` has a different shape: it only reads the flat executable
plan. Their decidable equality makes the V1 structural fragment executable.
This is the statically checkable part of the preservation obligation; full
behavioral preservation remains stated-deferred. Canonical sorting and stable
identity comparison follow DECISIONS.md §J10.
-/

/-- Static objects whose preservation can be checked without an executable
    stochastic denotation. Every collection is returned in stable order. -/
structure StaticMeaning where
  leaves : List (String × StableId)
  transitions : List (String × UInt32)
  mailboxes : List String
  boundary : List (String × String)
deriving Repr, BEq

private def staticError
    (code : LinkErrorCodeV1) (primary : StableId) (message : String)
    (related : List StableId := []) : LinkErrorV1 :=
  { code, message, primary, related }

private def staticSortBy (items : List α) (key : α → String) : List α :=
  items.mergeSort fun left right => key left < key right

private def staticIdSlug (kindPrefix : String) (id : StableId) : String :=
  id.raw.drop kindPrefix.length

private def staticDefinition?
    (definitions : List ComponentDefinitionV1) (id : StableId) :
    Option ComponentDefinitionV1 :=
  definitions.find? fun definition => definition.id == id

private def staticEnvelopeErrors (source : CompositionSourceV1) : List LinkErrorV1 :=
  let versionErrors :=
    if source.schemaVersion == Plan.compositionSourceSchema then []
    else [staticError .unknownVersion source.modelId
      s!"unknown schema version '{source.schemaVersion}'; supported: {Plan.compositionSourceSchema}"]
  let featureErrors := source.requiredFeatures.map fun feature =>
    staticError .unsupportedFeature source.modelId s!"unsupported required feature '{feature}'"
  versionErrors ++ featureErrors

private def staticConstructErrors (source : CompositionSourceV1) : List LinkErrorV1 :=
  match staticDefinition? source.definitions source.rootDefinition with
  | some { body := .primitive _, .. } =>
      [staticError .unsupportedConstruct source.rootDefinition
        "primitive root definitions are unsupported in V1 because executable leaf paths must be nonempty"]
  | _ => []

private def staticDuplicateDefinitionErrors
    (definitions : List ComponentDefinitionV1) : List LinkErrorV1 :=
  let rec loop (seen : List StableId) : List ComponentDefinitionV1 → List LinkErrorV1
    | [] => []
    | definition :: rest =>
        let errors := if seen.contains definition.id then
          [staticError .duplicateStableId definition.id
            s!"duplicate definition id '{definition.id.raw}'" [definition.id]]
        else []
        errors ++ loop (definition.id :: seen) rest
  loop [] definitions

private def staticMissingDefinitionErrors
    (source : CompositionSourceV1) : List LinkErrorV1 :=
  let rootErrors :=
    if (staticDefinition? source.definitions source.rootDefinition).isSome then []
    else [staticError .missingDefinition source.rootDefinition
      s!"root definition '{source.rootDefinition.raw}' does not exist"]
  let instanceErrors := (source.definitions.map fun owner =>
    match owner.body with
    | .primitive _ => []
    | .composite body => body.instances.filterMap fun item =>
        if (staticDefinition? source.definitions item.definition).isSome then none
        else some (staticError .missingDefinition item.id
          s!"instance '{item.id.raw}' references missing definition '{item.definition.raw}'"
          [item.definition, owner.id])).join
  rootErrors ++ instanceErrors

private def staticSuffixFrom (id : StableId) : List StableId → List StableId
  | [] => []
  | head :: rest => if head == id then head :: rest else staticSuffixFrom id rest

private partial def staticCollectCycles
    (definitions : List ComponentDefinitionV1) (current : StableId)
    (stack : List StableId) : List (List StableId) :=
  if stack.contains current then
    [staticSuffixFrom current stack ++ [current]]
  else
    match staticDefinition? definitions current with
    | none => []
    | some definition =>
        match definition.body with
        | .primitive _ => []
        | .composite body => (body.instances.map fun item =>
            staticCollectCycles definitions item.definition (stack ++ [current])).join

private def staticRotateTo (first : StableId) (items : List StableId) : List StableId :=
  let rec loop (before : List StableId) : List StableId → List StableId
    | [] => items
    | head :: rest =>
        if head == first then head :: rest ++ before.reverse
        else loop (head :: before) rest
  loop [] items

private def staticNormalizeCycle (cycle : List StableId) : List StableId :=
  let core := cycle.take (cycle.length - 1)
  match staticSortBy core (·.raw) with
  | [] => []
  | first :: _ =>
      let rotated := staticRotateTo first core
      rotated ++ [first]

private def staticCycleErrors (source : CompositionSourceV1) : List LinkErrorV1 :=
  let candidates := (staticCollectCycles source.definitions source.rootDefinition []).map fun cycle =>
    let normalized := staticNormalizeCycle cycle
    match normalized with
    | [] => staticError .recursiveDefinition source.rootDefinition "recursive definition cycle"
    | primary :: _ => staticError .recursiveDefinition primary
        ("recursive definition cycle: " ++ String.intercalate " -> " (normalized.map (·.raw)))
        normalized
  candidates.foldl (fun unique candidate =>
    if unique.any fun error =>
        error.primary == candidate.primary && error.message == candidate.message then unique
    else candidate :: unique) []

private structure StaticLeaf where
  boxName : String
  occurrence : StableId
  transitions : List IR.Transition

private partial def expandStaticLeaves
    (definitions : List ComponentDefinitionV1) (definition : ComponentDefinitionV1)
    (chain : List String) : List StaticLeaf :=
  match definition.body with
  | .primitive body =>
      let boxName := if chain.isEmpty then staticIdSlug "def:" definition.id
        else String.intercalate "/" chain
      [{ boxName
         occurrence := ⟨"occ:" ++ boxName⟩
         transitions := body.transitions }]
  | .composite body => (body.instances.map fun item =>
      match staticDefinition? definitions item.definition with
      | none => []
      | some child => expandStaticLeaves definitions child
          (chain ++ [staticIdSlug "inst:" item.id])).join

private structure StaticResolvedBoundary where
  direction : PortDirection
  schema : List IR.Attr
  leafPath : List StableId
  leafPort : StableId

private instance : Inhabited StaticResolvedBoundary := ⟨{
  direction := .input
  schema := []
  leafPath := []
  leafPort := ⟨"port:invalid"⟩ }⟩

private instance : Inhabited LinkErrorV1 :=
  ⟨staticError .missingPort ⟨"port:invalid"⟩ "static boundary resolution failed"⟩

private partial def staticDescendantHasPort
    (definitions : List ComponentDefinitionV1) (definition : ComponentDefinitionV1)
    (portId : StableId) : Bool :=
  definition.ports.any (·.id == portId) || match definition.body with
  | .primitive _ => false
  | .composite body =>
      body.exposures.any (·.outerPort == portId) || body.instances.any fun item =>
        match staticDefinition? definitions item.definition with
        | none => false
        | some child => staticDescendantHasPort definitions child portId

mutual
  private partial def resolveStaticDefinitionBoundary
      (definitions : List ComponentDefinitionV1) (primary : StableId) (context : String)
      (definition : ComponentDefinitionV1) (portId : StableId) :
      Except LinkErrorV1 StaticResolvedBoundary := do
    match definition.body with
    | .primitive _ =>
        let port ← match definition.ports.find? fun candidate => candidate.id == portId with
          | some port => pure port
          | none => throw (staticError .missingPort primary
              s!"{context} references missing port '{portId.raw}' on primitive '{definition.id.raw}'"
              [definition.id, portId])
        pure {
          direction := port.direction
          schema := port.schema
          leafPath := []
          leafPort := port.id }
    | .composite body =>
        let exposure ← match body.exposures.find? fun candidate => candidate.outerPort == portId with
          | some exposure => pure exposure
          | none =>
              if staticDescendantHasPort definitions definition portId then
                throw (staticError .inaccessibleDescendantPort primary
                  s!"{context} references descendant port '{portId.raw}', but composite '{definition.id.raw}' failed to expose it"
                  [definition.id, portId])
              else
                throw (staticError .missingPort primary
                  s!"{context} references missing boundary port '{portId.raw}' on composite '{definition.id.raw}'"
                  [definition.id, portId])
        resolveStaticDirectChildBoundary definitions primary context body
          exposure.innerInstance exposure.innerPort

  private partial def resolveStaticDirectChildBoundary
      (definitions : List ComponentDefinitionV1) (primary : StableId) (context : String)
      (owner : CompositeBodyV1) (childId portId : StableId) :
      Except LinkErrorV1 StaticResolvedBoundary := do
    let child ← match owner.instances.find? fun item => item.id == childId with
      | some child => pure child
      | none => throw (staticError .missingPort primary
          s!"{context} references missing direct child '{childId.raw}'" [childId, portId])
    let childDefinition ← match staticDefinition? definitions child.definition with
      | some childDefinition => pure childDefinition
      | none => throw (staticError .missingPort primary
          s!"{context} cannot resolve child definition '{child.definition.raw}'"
          [child.id, child.definition, portId])
    let resolved ← resolveStaticDefinitionBoundary definitions primary context
      childDefinition portId
    pure { resolved with leafPath := child.id :: resolved.leafPath }
end

private structure StaticWireEndpoint where
  instance_ : InstanceDeclV1
  definition : ComponentDefinitionV1
  direction : PortDirection
  schema : List IR.Attr
  boxName : String
  occurrence : String
  port : StableId

private def resolveStaticWireEndpoint
    (definitions : List ComponentDefinitionV1) (owner : CompositeBodyV1)
    (ownerChain : List String) (wire : WireDeclV1) (childId portId : StableId) :
    Except LinkErrorV1 StaticWireEndpoint := do
  let child ← match owner.instances.find? fun item => item.id == childId with
    | some child => pure child
    | none => throw (staticError .missingPort wire.id
        s!"wire '{wire.id.raw}' references missing direct child '{childId.raw}'" [childId])
  let childDefinition ← match staticDefinition? definitions child.definition with
    | some childDefinition => pure childDefinition
    | none => throw (staticError .missingPort wire.id
        s!"wire '{wire.id.raw}' cannot resolve child definition '{child.definition.raw}'"
        [child.id, child.definition])
  let resolved ← resolveStaticDirectChildBoundary definitions wire.id
    s!"wire '{wire.id.raw}'" owner childId portId
  let chain := ownerChain ++ resolved.leafPath.map (staticIdSlug "inst:")
  let boxName := String.intercalate "/" chain
  pure {
    instance_ := child
    definition := childDefinition
    direction := resolved.direction
    schema := resolved.schema
    boxName
    occurrence := "occ:" ++ boxName
    port := resolved.leafPort }

private def resolveStaticWire
    (definitions : List ComponentDefinitionV1) (owner : CompositeBodyV1)
    (ownerChain : List String) (wire : WireDeclV1) : Except LinkErrorV1 String := do
  let source ← resolveStaticWireEndpoint definitions owner ownerChain wire
    wire.sourceInstance wire.sourcePort
  let target ← resolveStaticWireEndpoint definitions owner ownerChain wire
    wire.targetInstance wire.targetPort
  if source.direction != .output then
    throw (staticError .directionMismatch wire.id
      s!"wire '{wire.id.raw}' source port '{wire.sourcePort.raw}' is not an output"
      [source.instance_.id, source.definition.id, source.port])
  if target.direction != .input then
    throw (staticError .directionMismatch wire.id
      s!"wire '{wire.id.raw}' target port '{wire.targetPort.raw}' is not an input"
      [target.instance_.id, target.definition.id, target.port])
  if source.schema != target.schema then
    throw (staticError .schemaMismatch wire.id
      s!"wire '{wire.id.raw}' has incompatible source and target schemas"
      [source.port, target.port])
  let ownerOccurrence := "occ:" ++ String.intercalate "/" ownerChain
  let wireOccurrence := ownerOccurrence ++ "#wire:" ++ staticIdSlug "wire:" wire.id
  pure ("mbox:" ++ wireOccurrence ++ "|" ++ source.occurrence ++ "." ++
    source.port.raw ++ "|" ++ target.occurrence ++ "." ++ target.port.raw)

private partial def expandStaticMailboxes
    (definitions : List ComponentDefinitionV1) (definition : ComponentDefinitionV1)
    (chain : List String) : Except LinkErrorV1 (List String) := do
  match definition.body with
  | .primitive _ => pure []
  | .composite body =>
      let localMailboxes ← body.wires.mapM fun wire =>
        resolveStaticWire definitions body chain wire
      let nested ← body.instances.mapM fun item =>
        match staticDefinition? definitions item.definition with
        | none => pure []
        | some child => expandStaticMailboxes definitions child
            (chain ++ [staticIdSlug "inst:" item.id])
      pure (localMailboxes ++ nested.join)

private def denoteStaticRootBoundary
    (definitions : List ComponentDefinitionV1) (root : ComponentDefinitionV1) :
    Except LinkErrorV1 (List (String × String)) := do
  match root.body with
  | .primitive _ => pure []
  | .composite body =>
      body.exposures.mapM fun exposure => do
        let resolved ← resolveStaticDefinitionBoundary definitions exposure.id
          s!"root exposure '{exposure.id.raw}'" root exposure.outerPort
        let leaf := "occ:" ++ String.intercalate "/"
          (resolved.leafPath.map (staticIdSlug "inst:"))
        pure (exposure.outerPort.raw, leaf ++ "." ++ resolved.leafPort.raw)

/-- Independently interpret the static source hierarchy. This function is
    intentionally structurally recursive over source definitions and instances. -/
def denoteSourceStatic (source : CompositionSourceV1) :
    Except (List LinkErrorV1) StaticMeaning := do
  let firstErrors := staticEnvelopeErrors source ++ staticConstructErrors source
  if !firstErrors.isEmpty then .error (sortLinkErrors firstErrors)
  let collectionErrors := staticDuplicateDefinitionErrors source.definitions ++
    staticMissingDefinitionErrors source
  if !collectionErrors.isEmpty then .error (sortLinkErrors collectionErrors)
  let recursionErrors := staticCycleErrors source
  if !recursionErrors.isEmpty then .error (sortLinkErrors recursionErrors)
  let root ← match staticDefinition? source.definitions source.rootDefinition with
    | some root => pure root
    | none => .error (sortLinkErrors [staticError .missingDefinition source.rootDefinition
        s!"root definition '{source.rootDefinition.raw}' does not exist"])
  let staticLeaves := expandStaticLeaves source.definitions root []
  let mailboxes ← match expandStaticMailboxes source.definitions root [] with
    | .ok mailboxes => pure mailboxes
    | .error error => .error [error]
  let boundary ← match denoteStaticRootBoundary source.definitions root with
    | .ok boundary => pure boundary
    | .error error => .error [error]
  pure {
    leaves := staticSortBy (staticLeaves.map fun leaf =>
      (leaf.boxName, leaf.occurrence)) (·.1)
    transitions := staticSortBy ((staticLeaves.map fun leaf =>
      leaf.transitions.map fun transition =>
        let identity := leaf.occurrence.raw ++ "#" ++ transition.name
        (identity, Hash.ruleWord identity)).join) (·.1)
    mailboxes := staticSortBy mailboxes id
    boundary := staticSortBy boundary (·.1) }

/-- Read the same static object set from an already-flat executable plan. -/
def denotePlanStatic (plan : Plan.ExecutablePlanV1) : StaticMeaning := {
  leaves := staticSortBy (plan.identity.leaves.map fun leaf =>
    (leaf.box, ⟨leaf.occurrence⟩)) (·.1)
  transitions := staticSortBy (plan.identity.transitions.map fun transition =>
    (transition.identity, transition.ruleWord)) (·.1)
  mailboxes := staticSortBy (plan.identity.mailboxes.map (·.identity)) id
  boundary := staticSortBy (match plan.linkedProvenance with
    | none => []
    | some provenance => provenance.sourceMap.boundary.map fun boundary =>
        (boundary.outer, boundary.leaf ++ ".port:" ++ boundary.port)) (·.1) }

end Sembla.Composition
