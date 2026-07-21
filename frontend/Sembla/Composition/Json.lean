import Lean.Data.Json
import Std
import Sembla.Composition.Source
import Sembla.PlanJson

namespace Sembla.Composition.Json

open Sembla
open Sembla.PlanJson

private def arr (values : List CJson) : CJson := .arr values.toArray
private def obj (fields : List (String × CJson)) : CJson := .obj fields.toArray
private def opt (encode : α → CJson) : Option α → CJson
  | none => .null
  | some value => encode value

/- Primitive IR encoding deliberately mirrors `Sembla.PlanJson` while preserving
   every source list in author order.  The PRD-0005 helper definitions are
   private, so the source module repeats their frozen shapes. -/
private def paramTypeJson : IR.ParamType → CJson
  | .real => .str "real"
  | .int => .str "int"
private def paramValueJson : IR.ParamValue → CJson
  | .real value => obj [("kind", .str "real"), ("value", .num value)]
  | .int value => obj [("kind", .str "int"), ("value", .int value)]
private def priorFamilyJson : IR.PriorFamily → CJson
  | .normal => .str "normal"
  | .logNormal => .str "log_normal"
  | .uniform => .str "uniform"
private def priorJson (prior : IR.Prior) : CJson := obj [
  ("family", priorFamilyJson prior.family),
  ("args", arr (prior.args.map CJson.num))]
private def paramJson (param : IR.ParamDecl) : CJson := obj [
  ("name", .str param.name), ("ty", paramTypeJson param.ty),
  ("default", paramValueJson param.default), ("prior", opt priorJson param.prior)]

private def attrTypeJson : IR.AttrType → CJson
  | .real => obj [("kind", .str "real")]
  | .int => obj [("kind", .str "int")]
  | .enum variants => obj [("kind", .str "enum"), ("variants", arr (variants.map CJson.str))]
  | .ref table => obj [("kind", .str "ref"), ("table", .str table)]
private def attrJson (attr : IR.Attr) : CJson := obj [
  ("name", .str attr.name), ("ty", attrTypeJson attr.ty)]
private def tableJson (table : IR.Table) : CJson := obj [
  ("name", .str table.name), ("size_hint", .int (Int.ofNat table.sizeHint)),
  ("attrs", arr (table.attrs.map attrJson))]

mutual
  private partial def exprJson : IR.Expr → CJson
    | .real value => taggedValue "real" (.num value)
    | .int value => taggedValue "int" (.int value)
    | .bool value => taggedValue "bool" (.bool value)
    | .enum variant => obj [("kind", .str "enum"), ("variant", .str variant)]
    | .param name => obj [("kind", .str "param"), ("name", .str name)]
    | .selfAttr name => obj [("kind", .str "self_attr"), ("name", .str name)]
    | .add lhs rhs => binary "add" lhs rhs
    | .sub lhs rhs => binary "sub" lhs rhs
    | .mul lhs rhs => binary "mul" lhs rhs
    | .div lhs rhs => binary "div" lhs rhs
    | .eq lhs rhs => binary "eq" lhs rhs
    | .ne lhs rhs => binary "ne" lhs rhs
    | .lt lhs rhs => binary "lt" lhs rhs
    | .le lhs rhs => binary "le" lhs rhs
    | .gt lhs rhs => binary "gt" lhs rhs
    | .ge lhs rhs => binary "ge" lhs rhs
    | .and lhs rhs => binary "and" lhs rhs
    | .or lhs rhs => binary "or" lhs rhs
    | .not expression => obj [("kind", .str "not"), ("expr", exprJson expression)]
    | .enumIs attr variant => obj [
        ("kind", .str "enum_is"), ("attr", .str attr), ("variant", .str variant)]
    | .input port aggregate => obj [
        ("kind", .str "input"), ("port", .str port), ("agg", aggregateJson aggregate)]
    | .agg operation table fk selfFk filter => obj [
        ("kind", .str "agg"), ("op", aggOpJson operation), ("table", .str table),
        ("on", obj [("fk_attr", .str fk), ("self_fk_attr", .str selfFk)]),
        ("filter", exprJson filter)]

  private partial def aggOpJson : IR.AggOp → CJson
    | .count => obj [("kind", .str "count")]
    | .sum value => obj [("kind", .str "sum"), ("value", exprJson value)]

  private partial def aggregateJson : IR.Aggregate → CJson
    | .mk operation filter => obj [
        ("op", aggOpJson operation), ("filter", opt exprJson filter)]

  private partial def binary (kind : String) (lhs rhs : IR.Expr) : CJson := obj [
    ("kind", .str kind), ("lhs", exprJson lhs), ("rhs", exprJson rhs)]

  private partial def taggedValue (kind : String) (value : CJson) : CJson := obj [
    ("kind", .str kind), ("value", value)]
end

private def effectJson : IR.Effect → CJson
  | .setAttr attr value => obj [
      ("kind", .str "set_attr"), ("attr", .str attr), ("value", exprJson value)]
private def orderingJson : IR.ClaimOrdering → CJson
  | .raceTime => obj [("kind", .str "race_time")]
  | .key expression => obj [("kind", .str "key"), ("expr", exprJson expression)]
private def claimJson (claim : IR.ResourceClaim) : CJson := obj [
  ("resource", exprJson claim.resource), ("ordering", orderingJson claim.ordering)]
private def transitionJson (transition : IR.Transition) : CJson := obj [
  ("name", .str transition.name), ("table", .str transition.table),
  ("guard", exprJson transition.guard), ("hazard", exprJson transition.hazard),
  ("effects", arr (transition.effects.map effectJson)),
  ("contests", arr (transition.contests.map claimJson))]
private def inputJson (port : IR.PortDecl) : CJson := obj [
  ("name", .str port.name), ("schema", arr (port.schema.map attrJson))]
private def outputFieldJson (field : IR.OutputField) : CJson := obj [
  ("name", .str field.name), ("op", aggOpJson field.op),
  ("filter", opt exprJson field.filter)]
private def outputBuilderJson : IR.OutputBuilder → CJson
  | .perTable table fields => obj [
      ("kind", .str "per_table"), ("table", .str table),
      ("fields", arr (fields.map outputFieldJson))]
private def outputJson (output : IR.OutputDecl) : CJson := obj [
  ("name", .str output.name), ("schema", arr (output.schema.map attrJson)),
  ("builder", outputBuilderJson output.builder)]
private def viewReduceJson : IR.ViewReduce → CJson
  | .sum => .str "sum"
  | .count => .str "count"
  | .min => .str "min"
  | .max => .str "max"
private def viewJson (view : IR.ViewDecl) : CJson := obj [
  ("name", .str view.name), ("table", .str view.table),
  ("filter", opt exprJson view.filter), ("value", opt exprJson view.value),
  ("reduce", viewReduceJson view.reduce)]
private def summaryReduceJson : IR.SummaryReduce → CJson
  | .sum => .str "sum"
  | .min => .str "min"
  | .max => .str "max"
  | .last => .str "last"
  | .argmaxTick => .str "argmax_tick"

private def stableIdJson (id : StableId) : CJson := .str id.raw
private def directionJson : PortDirection → CJson
  | .input => .str "input"
  | .output => .str "output"
private def declaredPortJson (port : PortDeclV1) : CJson := obj [
  ("id", stableIdJson port.id), ("display_name", .str port.displayName),
  ("direction", directionJson port.direction), ("schema", arr (port.schema.map attrJson))]
private def bindingJson (binding : ParameterBinding) : CJson := obj [
  ("requirement", .str binding.requirement), ("parameter", .str binding.parameter)]
private def instanceJson (item : InstanceDeclV1) : CJson := obj [
  ("id", stableIdJson item.id), ("display_name", .str item.displayName),
  ("definition", stableIdJson item.definition),
  ("parameter_bindings", arr (item.parameterBindings.map bindingJson))]
private def wireJson (wire : WireDeclV1) : CJson := obj [
  ("id", stableIdJson wire.id), ("source_instance", stableIdJson wire.sourceInstance),
  ("source_port", stableIdJson wire.sourcePort),
  ("target_instance", stableIdJson wire.targetInstance),
  ("target_port", stableIdJson wire.targetPort),
  ("delay_ticks", .int (Int.ofNat wire.delayTicks))]
private def exposureJson (exposure : ExposureDeclV1) : CJson := obj [
  ("id", stableIdJson exposure.id),
  ("inner_instance", stableIdJson exposure.innerInstance),
  ("inner_port", stableIdJson exposure.innerPort),
  ("outer_port", stableIdJson exposure.outerPort)]
private def hiddenPortJson (hidden : HiddenPortV1) : CJson := obj [
  ("instance", stableIdJson hidden.instance_), ("port", stableIdJson hidden.port)]

private def definitionJson (definition : ComponentDefinitionV1) : CJson :=
  let common := [
    ("id", stableIdJson definition.id),
    ("display_name", .str definition.displayName),
    ("parameter_requirements", arr (definition.parameterRequirements.map CJson.str)),
    ("ports", arr (definition.ports.map declaredPortJson))]
  match definition.body with
  | .primitive body => obj (common ++ [
      ("kind", .str "primitive"),
      ("tables", arr (body.tables.map tableJson)),
      ("transitions", arr (body.transitions.map transitionJson)),
      ("inputs", arr (body.inputs.map inputJson)),
      ("outputs", arr (body.outputs.map outputJson)),
      ("views", arr (body.views.map viewJson))])
  | .composite body => obj (common ++ [
      ("kind", .str "composite"),
      ("instances", arr (body.instances.map instanceJson)),
      ("wires", arr (body.wires.map wireJson)),
      ("exposures", arr (body.exposures.map exposureJson)),
      ("hidden_ports", arr (body.hiddenPorts.map hiddenPortJson))])

private def sourceSummaryJson (summary : SourceSummaryV1) : CJson := obj [
  ("name", .str summary.name), ("reduce", summaryReduceJson summary.reduce),
  ("instance_path", arr (summary.instancePath.map stableIdJson)),
  ("view", .str summary.view)]

/-- Encode source V1 without reordering any author-owned array. -/
def encode (source : CompositionSourceV1) : CJson := obj [
  ("schema_version", .str source.schemaVersion),
  ("model_id", stableIdJson source.modelId),
  ("display_name", .str source.displayName),
  ("outer_dt", .num source.outerDt),
  ("parameters", arr (source.parameters.map paramJson)),
  ("definitions", arr (source.definitions.map definitionJson)),
  ("root_definition", stableIdJson source.rootDefinition),
  ("required_features", arr (source.requiredFeatures.map CJson.str)),
  ("summaries", arr (source.summaries.map sourceSummaryJson))]

def render (source : CompositionSourceV1) : String := (encode source).render

/- Explicit, path-aware decoding. -/
private abbrev JsonObject := Lean.RBNode String (fun _ => Lean.Json)

private def childPath (path field : String) : String :=
  if path == "$" then field else path ++ "." ++ field

private def expectObject (path : String) : Lean.Json → Except String JsonObject
  | .obj fields => pure fields
  | _ => throw s!"{path}: expected object"

private def checkFields
    (path : String) (fields : JsonObject) (allowed : List String) : Except String Unit := do
  fields.foldM (fun _ key _ =>
    if allowed.contains key then pure ()
    else throw s!"unknown field '{key}' at {path}") ()

private def requiredField
    (path name : String) (fields : JsonObject) : Except String Lean.Json :=
  match fields.find compare name with
  | some value => pure value
  | none => throw s!"{childPath path name}: missing required field"

private def expectString (path : String) : Lean.Json → Except String String
  | .str value => pure value
  | _ => throw s!"{path}: expected string"
private def expectBool (path : String) : Lean.Json → Except String Bool
  | .bool value => pure value
  | _ => throw s!"{path}: expected boolean"
private def expectScientific (path : String) : Lean.Json → Except String IR.Scientific
  | .num value => pure ⟨value.mantissa, -(Int.ofNat value.exponent)⟩
  | _ => throw s!"{path}: expected number"
private def expectInt (path : String) (value : Lean.Json) : Except String Int :=
  value.getInt?.mapError fun _ => s!"{path}: expected integer"
private def expectNat (path : String) (value : Lean.Json) : Except String Nat :=
  value.getNat?.mapError fun _ => s!"{path}: expected natural number"

private def decodeList
    (path : String) (decode : String → Lean.Json → Except String α) :
    Lean.Json → Except String (List α)
  | .arr values =>
      let rec loop (index : Nat) : List Lean.Json → Except String (List α)
        | [] => pure []
        | value :: rest => do
            let head ← decode s!"{path}[{index}]" value
            let tail ← loop (index + 1) rest
            pure (head :: tail)
      loop 0 values.toList
  | _ => throw s!"{path}: expected array"

private def decodeStringList (path : String) : Lean.Json → Except String (List String) :=
  decodeList path expectString
private def decodeStableId (path : String) (value : Lean.Json) : Except String StableId :=
  return ⟨← expectString path value⟩
private def decodeOptional
    (path : String) (decode : String → Lean.Json → Except String α) :
    Lean.Json → Except String (Option α)
  | .null => pure none
  | value => return some (← decode path value)

private def decodeParamType (path : String) (value : Lean.Json) : Except String IR.ParamType := do
  match ← expectString path value with
  | "real" => pure .real
  | "int" => pure .int
  | other => throw s!"{path}: unknown parameter type '{other}'"

private def decodePriorFamily (path : String) (value : Lean.Json) : Except String IR.PriorFamily := do
  match ← expectString path value with
  | "normal" => pure .normal
  | "log_normal" => pure .logNormal
  | "uniform" => pure .uniform
  | other => throw s!"{path}: unknown prior family '{other}'"

private def decodePrior (path : String) (value : Lean.Json) : Except String IR.Prior := do
  let fields ← expectObject path value
  checkFields path fields ["family", "args"]
  pure {
    family := ← decodePriorFamily (childPath path "family") (← requiredField path "family" fields)
    args := ← decodeList (childPath path "args") expectScientific
      (← requiredField path "args" fields) }

private def decodeParamValue
    (path : String) (value : Lean.Json) : Except String IR.ParamValue := do
  let fields ← expectObject path value
  checkFields path fields ["kind", "value"]
  let kind ← expectString (childPath path "kind") (← requiredField path "kind" fields)
  let raw ← requiredField path "value" fields
  match kind with
  | "real" => return .real (← expectScientific (childPath path "value") raw)
  | "int" => return .int (← expectInt (childPath path "value") raw)
  | other => throw s!"{childPath path "kind"}: unknown parameter value kind '{other}'"

private def decodeParam (path : String) (value : Lean.Json) : Except String IR.ParamDecl := do
  let fields ← expectObject path value
  checkFields path fields ["name", "ty", "default", "prior"]
  pure {
    name := ← expectString (childPath path "name") (← requiredField path "name" fields)
    ty := ← decodeParamType (childPath path "ty") (← requiredField path "ty" fields)
    default := ← decodeParamValue (childPath path "default") (← requiredField path "default" fields)
    prior := ← decodeOptional (childPath path "prior") decodePrior
      (← requiredField path "prior" fields) }

private def decodeAttrType (path : String) (value : Lean.Json) : Except String IR.AttrType := do
  let fields ← expectObject path value
  let kind ← expectString (childPath path "kind") (← requiredField path "kind" fields)
  match kind with
  | "real" =>
      checkFields path fields ["kind"]
      pure .real
  | "int" =>
      checkFields path fields ["kind"]
      pure .int
  | "enum" =>
      checkFields path fields ["kind", "variants"]
      return .enum (← decodeStringList (childPath path "variants")
        (← requiredField path "variants" fields))
  | "ref" =>
      checkFields path fields ["kind", "table"]
      return .ref (← expectString (childPath path "table")
        (← requiredField path "table" fields))
  | other => throw s!"{childPath path "kind"}: unknown attribute type kind '{other}'"

private def decodeAttr (path : String) (value : Lean.Json) : Except String IR.Attr := do
  let fields ← expectObject path value
  checkFields path fields ["name", "ty"]
  pure {
    name := ← expectString (childPath path "name") (← requiredField path "name" fields)
    ty := ← decodeAttrType (childPath path "ty") (← requiredField path "ty" fields) }

private def decodeTable (path : String) (value : Lean.Json) : Except String IR.Table := do
  let fields ← expectObject path value
  checkFields path fields ["name", "size_hint", "attrs"]
  pure {
    name := ← expectString (childPath path "name") (← requiredField path "name" fields)
    sizeHint := ← expectNat (childPath path "size_hint") (← requiredField path "size_hint" fields)
    attrs := ← decodeList (childPath path "attrs") decodeAttr (← requiredField path "attrs" fields) }

mutual
  private partial def decodeExpr (path : String) (value : Lean.Json) : Except String IR.Expr := do
    let fields ← expectObject path value
    let kind ← expectString (childPath path "kind") (← requiredField path "kind" fields)
    match kind with
    | "real" =>
        checkFields path fields ["kind", "value"]
        return .real (← expectScientific (childPath path "value") (← requiredField path "value" fields))
    | "int" =>
        checkFields path fields ["kind", "value"]
        return .int (← expectInt (childPath path "value") (← requiredField path "value" fields))
    | "bool" =>
        checkFields path fields ["kind", "value"]
        return .bool (← expectBool (childPath path "value") (← requiredField path "value" fields))
    | "enum" =>
        checkFields path fields ["kind", "variant"]
        return .enum (← expectString (childPath path "variant") (← requiredField path "variant" fields))
    | "param" =>
        checkFields path fields ["kind", "name"]
        return .param (← expectString (childPath path "name") (← requiredField path "name" fields))
    | "self_attr" =>
        checkFields path fields ["kind", "name"]
        return .selfAttr (← expectString (childPath path "name") (← requiredField path "name" fields))
    | "add" => decodeBinary path fields .add
    | "sub" => decodeBinary path fields .sub
    | "mul" => decodeBinary path fields .mul
    | "div" => decodeBinary path fields .div
    | "eq" => decodeBinary path fields .eq
    | "ne" => decodeBinary path fields .ne
    | "lt" => decodeBinary path fields .lt
    | "le" => decodeBinary path fields .le
    | "gt" => decodeBinary path fields .gt
    | "ge" => decodeBinary path fields .ge
    | "and" => decodeBinary path fields .and
    | "or" => decodeBinary path fields .or
    | "not" =>
        checkFields path fields ["kind", "expr"]
        return .not (← decodeExpr (childPath path "expr") (← requiredField path "expr" fields))
    | "enum_is" =>
        checkFields path fields ["kind", "attr", "variant"]
        return .enumIs
          (← expectString (childPath path "attr") (← requiredField path "attr" fields))
          (← expectString (childPath path "variant") (← requiredField path "variant" fields))
    | "input" =>
        checkFields path fields ["kind", "port", "agg"]
        return .input
          (← expectString (childPath path "port") (← requiredField path "port" fields))
          (← decodeAggregate (childPath path "agg") (← requiredField path "agg" fields))
    | "agg" =>
        checkFields path fields ["kind", "op", "table", "on", "filter"]
        let onPath := childPath path "on"
        let onFields ← expectObject onPath (← requiredField path "on" fields)
        checkFields onPath onFields ["fk_attr", "self_fk_attr"]
        return .agg
          (← decodeAggOp (childPath path "op") (← requiredField path "op" fields))
          (← expectString (childPath path "table") (← requiredField path "table" fields))
          (← expectString (childPath onPath "fk_attr") (← requiredField onPath "fk_attr" onFields))
          (← expectString (childPath onPath "self_fk_attr") (← requiredField onPath "self_fk_attr" onFields))
          (← decodeExpr (childPath path "filter") (← requiredField path "filter" fields))
    | other => throw s!"{childPath path "kind"}: unknown expression kind '{other}'"

  private partial def decodeBinary
      (path : String) (fields : JsonObject) (make : IR.Expr → IR.Expr → IR.Expr) :
      Except String IR.Expr := do
    checkFields path fields ["kind", "lhs", "rhs"]
    return make
      (← decodeExpr (childPath path "lhs") (← requiredField path "lhs" fields))
      (← decodeExpr (childPath path "rhs") (← requiredField path "rhs" fields))

  private partial def decodeAggOp
      (path : String) (value : Lean.Json) : Except String IR.AggOp := do
    let fields ← expectObject path value
    let kind ← expectString (childPath path "kind") (← requiredField path "kind" fields)
    match kind with
    | "count" =>
        checkFields path fields ["kind"]
        pure .count
    | "sum" =>
        checkFields path fields ["kind", "value"]
        return .sum (← decodeExpr (childPath path "value") (← requiredField path "value" fields))
    | other => throw s!"{childPath path "kind"}: unknown aggregate operation kind '{other}'"

  private partial def decodeAggregate
      (path : String) (value : Lean.Json) : Except String IR.Aggregate := do
    let fields ← expectObject path value
    checkFields path fields ["op", "filter"]
    return .mk
      (← decodeAggOp (childPath path "op") (← requiredField path "op" fields))
      (← decodeOptional (childPath path "filter") decodeExpr (← requiredField path "filter" fields))
end

private def decodeEffect (path : String) (value : Lean.Json) : Except String IR.Effect := do
  let fields ← expectObject path value
  checkFields path fields ["kind", "attr", "value"]
  let kind ← expectString (childPath path "kind") (← requiredField path "kind" fields)
  if kind != "set_attr" then
    throw s!"{childPath path "kind"}: unknown effect kind '{kind}'"
  return .setAttr
    (← expectString (childPath path "attr") (← requiredField path "attr" fields))
    (← decodeExpr (childPath path "value") (← requiredField path "value" fields))

private def decodeOrdering
    (path : String) (value : Lean.Json) : Except String IR.ClaimOrdering := do
  let fields ← expectObject path value
  let kind ← expectString (childPath path "kind") (← requiredField path "kind" fields)
  match kind with
  | "race_time" =>
      checkFields path fields ["kind"]
      pure .raceTime
  | "key" =>
      checkFields path fields ["kind", "expr"]
      return .key (← decodeExpr (childPath path "expr") (← requiredField path "expr" fields))
  | other => throw s!"{childPath path "kind"}: unknown claim ordering kind '{other}'"

private def decodeClaim
    (path : String) (value : Lean.Json) : Except String IR.ResourceClaim := do
  let fields ← expectObject path value
  checkFields path fields ["resource", "ordering"]
  pure {
    resource := ← decodeExpr (childPath path "resource") (← requiredField path "resource" fields)
    ordering := ← decodeOrdering (childPath path "ordering") (← requiredField path "ordering" fields) }

private def decodeTransition
    (path : String) (value : Lean.Json) : Except String IR.Transition := do
  let fields ← expectObject path value
  checkFields path fields ["name", "table", "guard", "hazard", "effects", "contests"]
  pure {
    name := ← expectString (childPath path "name") (← requiredField path "name" fields)
    table := ← expectString (childPath path "table") (← requiredField path "table" fields)
    guard := ← decodeExpr (childPath path "guard") (← requiredField path "guard" fields)
    hazard := ← decodeExpr (childPath path "hazard") (← requiredField path "hazard" fields)
    effects := ← decodeList (childPath path "effects") decodeEffect (← requiredField path "effects" fields)
    contests := ← decodeList (childPath path "contests") decodeClaim (← requiredField path "contests" fields) }

private def decodeInput (path : String) (value : Lean.Json) : Except String IR.PortDecl := do
  let fields ← expectObject path value
  checkFields path fields ["name", "schema"]
  pure {
    name := ← expectString (childPath path "name") (← requiredField path "name" fields)
    schema := ← decodeList (childPath path "schema") decodeAttr (← requiredField path "schema" fields) }

private def decodeOutputField
    (path : String) (value : Lean.Json) : Except String IR.OutputField := do
  let fields ← expectObject path value
  checkFields path fields ["name", "op", "filter"]
  pure {
    name := ← expectString (childPath path "name") (← requiredField path "name" fields)
    op := ← decodeAggOp (childPath path "op") (← requiredField path "op" fields)
    filter := ← decodeOptional (childPath path "filter") decodeExpr (← requiredField path "filter" fields) }

private def decodeOutputBuilder
    (path : String) (value : Lean.Json) : Except String IR.OutputBuilder := do
  let fields ← expectObject path value
  checkFields path fields ["kind", "table", "fields"]
  let kind ← expectString (childPath path "kind") (← requiredField path "kind" fields)
  if kind != "per_table" then
    throw s!"{childPath path "kind"}: unknown output builder kind '{kind}'"
  return .perTable
    (← expectString (childPath path "table") (← requiredField path "table" fields))
    (← decodeList (childPath path "fields") decodeOutputField (← requiredField path "fields" fields))

private def decodeOutput
    (path : String) (value : Lean.Json) : Except String IR.OutputDecl := do
  let fields ← expectObject path value
  checkFields path fields ["name", "schema", "builder"]
  pure {
    name := ← expectString (childPath path "name") (← requiredField path "name" fields)
    schema := ← decodeList (childPath path "schema") decodeAttr (← requiredField path "schema" fields)
    builder := ← decodeOutputBuilder (childPath path "builder") (← requiredField path "builder" fields) }

private def decodeViewReduce (path : String) (value : Lean.Json) : Except String IR.ViewReduce := do
  match ← expectString path value with
  | "sum" => pure .sum
  | "count" => pure .count
  | "min" => pure .min
  | "max" => pure .max
  | other => throw s!"{path}: unknown view reduction '{other}'"

private def decodeView (path : String) (value : Lean.Json) : Except String IR.ViewDecl := do
  let fields ← expectObject path value
  checkFields path fields ["name", "table", "filter", "value", "reduce"]
  pure {
    name := ← expectString (childPath path "name") (← requiredField path "name" fields)
    table := ← expectString (childPath path "table") (← requiredField path "table" fields)
    filter := ← decodeOptional (childPath path "filter") decodeExpr (← requiredField path "filter" fields)
    value := ← decodeOptional (childPath path "value") decodeExpr (← requiredField path "value" fields)
    reduce := ← decodeViewReduce (childPath path "reduce") (← requiredField path "reduce" fields) }

private def decodeDirection
    (path : String) (value : Lean.Json) : Except String PortDirection := do
  match ← expectString path value with
  | "input" => pure .input
  | "output" => pure .output
  | other => throw s!"{path}: unknown port direction '{other}'"

private def decodeDeclaredPort
    (path : String) (value : Lean.Json) : Except String PortDeclV1 := do
  let fields ← expectObject path value
  checkFields path fields ["id", "display_name", "direction", "schema"]
  pure {
    id := ← decodeStableId (childPath path "id") (← requiredField path "id" fields)
    displayName := ← expectString (childPath path "display_name") (← requiredField path "display_name" fields)
    direction := ← decodeDirection (childPath path "direction") (← requiredField path "direction" fields)
    schema := ← decodeList (childPath path "schema") decodeAttr (← requiredField path "schema" fields) }

private def decodeBinding
    (path : String) (value : Lean.Json) : Except String ParameterBinding := do
  let fields ← expectObject path value
  checkFields path fields ["requirement", "parameter"]
  pure {
    requirement := ← expectString (childPath path "requirement")
      (← requiredField path "requirement" fields)
    parameter := ← expectString (childPath path "parameter")
      (← requiredField path "parameter" fields) }

private def decodeInstance
    (path : String) (value : Lean.Json) : Except String InstanceDeclV1 := do
  let fields ← expectObject path value
  checkFields path fields ["id", "display_name", "definition", "parameter_bindings"]
  pure {
    id := ← decodeStableId (childPath path "id") (← requiredField path "id" fields)
    displayName := ← expectString (childPath path "display_name") (← requiredField path "display_name" fields)
    definition := ← decodeStableId (childPath path "definition") (← requiredField path "definition" fields)
    parameterBindings := ← decodeList (childPath path "parameter_bindings") decodeBinding
      (← requiredField path "parameter_bindings" fields) }

private def decodeWire (path : String) (value : Lean.Json) : Except String WireDeclV1 := do
  let fields ← expectObject path value
  checkFields path fields [
    "id", "source_instance", "source_port", "target_instance", "target_port", "delay_ticks"]
  pure {
    id := ← decodeStableId (childPath path "id") (← requiredField path "id" fields)
    sourceInstance := ← decodeStableId (childPath path "source_instance")
      (← requiredField path "source_instance" fields)
    sourcePort := ← decodeStableId (childPath path "source_port")
      (← requiredField path "source_port" fields)
    targetInstance := ← decodeStableId (childPath path "target_instance")
      (← requiredField path "target_instance" fields)
    targetPort := ← decodeStableId (childPath path "target_port")
      (← requiredField path "target_port" fields)
    delayTicks := ← expectNat (childPath path "delay_ticks") (← requiredField path "delay_ticks" fields) }

private def decodeExposure
    (path : String) (value : Lean.Json) : Except String ExposureDeclV1 := do
  let fields ← expectObject path value
  checkFields path fields ["id", "inner_instance", "inner_port", "outer_port"]
  pure {
    id := ← decodeStableId (childPath path "id") (← requiredField path "id" fields)
    innerInstance := ← decodeStableId (childPath path "inner_instance")
      (← requiredField path "inner_instance" fields)
    innerPort := ← decodeStableId (childPath path "inner_port")
      (← requiredField path "inner_port" fields)
    outerPort := ← decodeStableId (childPath path "outer_port")
      (← requiredField path "outer_port" fields) }

private def decodeHiddenPort
    (path : String) (value : Lean.Json) : Except String HiddenPortV1 := do
  let fields ← expectObject path value
  checkFields path fields ["instance", "port"]
  pure {
    instance_ := ← decodeStableId (childPath path "instance") (← requiredField path "instance" fields)
    port := ← decodeStableId (childPath path "port") (← requiredField path "port" fields) }

private def decodeDefinition
    (path : String) (value : Lean.Json) : Except String ComponentDefinitionV1 := do
  let fields ← expectObject path value
  let kind ← expectString (childPath path "kind") (← requiredField path "kind" fields)
  let id ← decodeStableId (childPath path "id") (← requiredField path "id" fields)
  let displayName ← expectString (childPath path "display_name")
    (← requiredField path "display_name" fields)
  let requirements ← decodeStringList (childPath path "parameter_requirements")
    (← requiredField path "parameter_requirements" fields)
  let ports ← decodeList (childPath path "ports") decodeDeclaredPort
    (← requiredField path "ports" fields)
  match kind with
  | "primitive" =>
      checkFields path fields [
        "id", "display_name", "parameter_requirements", "ports", "kind",
        "tables", "transitions", "inputs", "outputs", "views"]
      pure {
        id, displayName, parameterRequirements := requirements, ports
        body := .primitive {
          tables := ← decodeList (childPath path "tables") decodeTable
            (← requiredField path "tables" fields)
          transitions := ← decodeList (childPath path "transitions") decodeTransition
            (← requiredField path "transitions" fields)
          inputs := ← decodeList (childPath path "inputs") decodeInput
            (← requiredField path "inputs" fields)
          outputs := ← decodeList (childPath path "outputs") decodeOutput
            (← requiredField path "outputs" fields)
          views := ← decodeList (childPath path "views") decodeView
            (← requiredField path "views" fields) } }
  | "composite" =>
      checkFields path fields [
        "id", "display_name", "parameter_requirements", "ports", "kind",
        "instances", "wires", "exposures", "hidden_ports"]
      pure {
        id, displayName, parameterRequirements := requirements, ports
        body := .composite {
          instances := ← decodeList (childPath path "instances") decodeInstance
            (← requiredField path "instances" fields)
          wires := ← decodeList (childPath path "wires") decodeWire
            (← requiredField path "wires" fields)
          exposures := ← decodeList (childPath path "exposures") decodeExposure
            (← requiredField path "exposures" fields)
          hiddenPorts := ← decodeList (childPath path "hidden_ports") decodeHiddenPort
            (← requiredField path "hidden_ports" fields) } }
  | other => throw s!"{childPath path "kind"}: unknown component kind '{other}'"

private def decodeSummaryReduce
    (path : String) (value : Lean.Json) : Except String IR.SummaryReduce := do
  match ← expectString path value with
  | "sum" => pure .sum
  | "min" => pure .min
  | "max" => pure .max
  | "last" => pure .last
  | "argmax_tick" => pure .argmaxTick
  | other => throw s!"{path}: unknown summary reduction '{other}'"

private def decodeSummary
    (path : String) (value : Lean.Json) : Except String SourceSummaryV1 := do
  let fields ← expectObject path value
  checkFields path fields ["name", "reduce", "instance_path", "view"]
  pure {
    name := ← expectString (childPath path "name") (← requiredField path "name" fields)
    reduce := ← decodeSummaryReduce (childPath path "reduce") (← requiredField path "reduce" fields)
    instancePath := ← decodeList (childPath path "instance_path") decodeStableId
      (← requiredField path "instance_path" fields)
    view := ← expectString (childPath path "view") (← requiredField path "view" fields) }

private def decodeSource (value : Lean.Json) : Except String CompositionSourceV1 := do
  let path := "$"
  let fields ← expectObject path value
  checkFields path fields [
    "schema_version", "model_id", "display_name", "outer_dt", "parameters",
    "definitions", "root_definition", "required_features", "summaries"]
  let schemaVersion ← expectString "schema_version" (← requiredField path "schema_version" fields)
  if schemaVersion != Plan.compositionSourceSchema then
    throw s!"unknown schema_version '{schemaVersion}'; supported: {Plan.compositionSourceSchema}"
  let requiredFeaturesValue ← requiredField path "required_features" fields
  let requiredFeatures : List String ←
    match requiredFeaturesValue with
    | .arr values =>
        match values.toList with
        | [] => pure []
        | first :: _ => do
            let feature ← expectString "required_features[0]" first
            throw s!"required_features[0]: unsupported feature '{feature}'"
    | _ => throw "required_features: expected array"
  pure {
    schemaVersion
    modelId := ← decodeStableId "model_id" (← requiredField path "model_id" fields)
    displayName := ← expectString "display_name" (← requiredField path "display_name" fields)
    outerDt := ← expectScientific "outer_dt" (← requiredField path "outer_dt" fields)
    parameters := ← decodeList "parameters" decodeParam (← requiredField path "parameters" fields)
    definitions := ← decodeList "definitions" decodeDefinition
      (← requiredField path "definitions" fields)
    rootDefinition := ← decodeStableId "root_definition"
      (← requiredField path "root_definition" fields)
    requiredFeatures
    summaries := ← decodeList "summaries" decodeSummary (← requiredField path "summaries" fields) }

/-- Parse permissive JSON formatting, reject unknown fields, then apply local V1 validation. -/
def parse (bytes : String) : Except String CompositionSourceV1 := do
  let json ← Lean.Json.parse bytes |>.mapError fun message => "json: " ++ message
  let source ← decodeSource json
  wellFormed source
  pure source

end Sembla.Composition.Json
