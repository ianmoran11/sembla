import Lean.Data.Json
import Std
import Sembla.Plan

namespace Sembla.PlanJson

open Sembla

/-- The deliberately small value language used by the canonical plan writer. -/
inductive CJson where
  | null
  | str (s : String)
  | num (n : IR.Scientific)
  | int (n : Int)
  | bool (b : Bool)
  | arr (xs : Array CJson)
  | obj (fields : Array (String × CJson))
deriving Repr, BEq, Inhabited

private def hexDigit (n : Nat) : Char :=
  if n < 10 then Char.ofNat ('0'.toNat + n)
  else Char.ofNat ('a'.toNat + n - 10)

private def escapedChar (character : Char) : String :=
  let code := character.toNat
  if code == 0x22 then "\\\""
  else if code == 0x5c then "\\\\"
  else if code == 0x08 then "\\b"
  else if code == 0x09 then "\\t"
  else if code == 0x0a then "\\n"
  else if code == 0x0c then "\\f"
  else if code == 0x0d then "\\r"
  else if code < 0x20 then
    "\\u00" ++ String.singleton (hexDigit (code / 16)) ++
      String.singleton (hexDigit (code % 16))
  else
    String.singleton character

private def quote (value : String) : String :=
  "\"" ++ value.foldl (fun out character => out ++ escapedChar character) "" ++ "\""

private partial def normalizeScientific (coefficient exponent : Int) : Int × Int :=
  if coefficient != 0 && coefficient % 10 == 0 then
    normalizeScientific (coefficient / 10) (exponent + 1)
  else
    (coefficient, exponent)

/-- Render an exact decimal with serde_json/ryu's finite-f64 spelling policy. -/
def renderScientific (value : IR.Scientific) : String :=
  if value.coefficient == 0 then "0.0"
  else
    let (coefficient, exponent) := normalizeScientific value.coefficient value.exponent
    let negative := coefficient < 0
    let sign := if negative then "-" else ""
    let digits := toString coefficient.natAbs
    let digitCount := Int.ofNat digits.length
    let normalizedExponent := exponent + digitCount - 1
    if normalizedExponent < -5 || normalizedExponent >= 16 then
      let rest := digits.drop 1
      let mantissa :=
        if rest.isEmpty then digits.take 1
        else digits.take 1 ++ "." ++ rest
      let exponentSign := if normalizedExponent >= 0 then "+" else ""
      sign ++ mantissa ++ "e" ++ exponentSign ++ toString normalizedExponent
    else
      let decimalPosition := digitCount + exponent
      if decimalPosition <= 0 then
        sign ++ "0." ++ String.mk (List.replicate (-decimalPosition).toNat '0') ++ digits
      else if decimalPosition >= digitCount then
        sign ++ digits ++ String.mk (List.replicate (decimalPosition - digitCount).toNat '0') ++ ".0"
      else
        let position := decimalPosition.toNat
        sign ++ digits.take position ++ "." ++ digits.drop position

private partial def byteArrayLessAt (lhs rhs : ByteArray) (index : Nat) : Bool :=
  if index >= lhs.size then index < rhs.size
  else if index >= rhs.size then false
  else
    let left := (lhs.get! index).toNat
    let right := (rhs.get! index).toNat
    if left < right then true
    else if left > right then false
    else byteArrayLessAt lhs rhs (index + 1)

private def bytewiseLess (lhs rhs : String) : Bool :=
  byteArrayLessAt lhs.toUTF8 rhs.toUTF8 0

/-- Render compact canonical JSON, recursively sorting object keys by UTF-8 bytes. -/
partial def CJson.render : CJson → String
  | .null => "null"
  | .str value => quote value
  | .num value => renderScientific value
  | .int value => toString value
  | .bool value => if value then "true" else "false"
  | .arr values =>
      "[" ++ String.intercalate "," (values.toList.map CJson.render) ++ "]"
  | .obj fields =>
      let sorted := fields.qsort fun left right => bytewiseLess left.1 right.1
      let rendered := sorted.toList.map fun (name, value) => quote name ++ ":" ++ value.render
      "{" ++ String.intercalate "," rendered ++ "}"

private def arr (values : List CJson) : CJson := .arr values.toArray
private def obj (fields : List (String × CJson)) : CJson := .obj fields.toArray
private def opt (encode : α → CJson) : Option α → CJson
  | none => .null
  | some value => encode value
private def sortBy (items : List α) (key : α → String) : List α :=
  items.mergeSort fun left right => key left < key right

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
private def portJson (port : IR.PortDecl) : CJson := obj [
  ("name", .str port.name), ("schema", arr (port.schema.map attrJson))]
private def fieldJson (field : IR.OutputField) : CJson := obj [
  ("name", .str field.name), ("op", aggOpJson field.op),
  ("filter", opt exprJson field.filter)]
private def builderJson : IR.OutputBuilder → CJson
  | .perTable table fields => obj [
      ("kind", .str "per_table"), ("table", .str table),
      ("fields", arr (fields.map fieldJson))]
private def outputJson (output : IR.OutputDecl) : CJson := obj [
  ("name", .str output.name), ("schema", arr (output.schema.map attrJson)),
  ("builder", builderJson output.builder)]
private def viewReduceJson : IR.ViewReduce → CJson
  | .sum => .str "sum"
  | .count => .str "count"
  | .min => .str "min"
  | .max => .str "max"
private def viewJson (view : IR.ViewDecl) : CJson := obj [
  ("name", .str view.name), ("table", .str view.table),
  ("filter", opt exprJson view.filter), ("value", opt exprJson view.value),
  ("reduce", viewReduceJson view.reduce)]
private def boxJson (modelBox : IR.Box) : CJson := obj [
  ("name", .str modelBox.name),
  ("tables", arr ((sortBy modelBox.tables (·.name)).map tableJson)),
  ("transitions", arr ((sortBy modelBox.transitions (·.name)).map transitionJson)),
  ("inputs", arr ((sortBy modelBox.inputs (·.name)).map portJson)),
  ("outputs", arr ((sortBy modelBox.outputs (·.name)).map outputJson)),
  ("views", arr ((sortBy modelBox.views (·.name)).map viewJson))]
private def endpointJson (endpoint : IR.WireEndpoint) : CJson := obj [
  ("box", .str endpoint.box), ("port", .str endpoint.port)]
private def wireJson (wire : IR.Wire) : CJson := obj [
  ("from", endpointJson wire.source), ("to", endpointJson wire.target)]
private def summaryReduceJson : IR.SummaryReduce → CJson
  | .sum => .str "sum"
  | .min => .str "min"
  | .max => .str "max"
  | .last => .str "last"
  | .argmaxTick => .str "argmax_tick"
private def summaryJson (summary : IR.SummaryDecl) : CJson := obj [
  ("name", .str summary.name), ("box", .str summary.box),
  ("view", .str summary.view), ("reduce", summaryReduceJson summary.reduce)]

/-- The direct-stable mailbox identity used as the canonical wire sort key. -/
def directMailboxIdentity (wire : IR.Wire) : String :=
  "mbox:occ:#wire:to_" ++ wire.target.box ++ "_" ++ wire.target.port ++
    "|occ:" ++ wire.source.box ++ ".port:" ++ wire.source.port ++
    "|occ:" ++ wire.target.box ++ ".port:" ++ wire.target.port

/-- Encode a model with exactly the array ordering required by plan V1. -/
def modelToCJson (model : IR.Model) : CJson := obj [
  ("name", .str model.name), ("dt", .num model.dt),
  ("params", arr ((sortBy model.params (·.name)).map paramJson)),
  ("boxes", arr ((sortBy model.boxes (·.name)).map boxJson)),
  /- Wire ordering is origin-specific: direct-stable construction sorts by its
     synthesized mailbox identities, while the linker sorts by declared wire
     occurrences. Both constructors canonicalize before reaching this writer. -/
  ("wires", arr (model.wires.map wireJson)),
  ("summaries", arr ((sortBy model.summaries (·.name)).map summaryJson))]

private def originJson : Plan.PlanOrigin → CJson
  | .linked => .str "linked"
  | .directStable => .str "direct_stable"
private def hashRecordJson (record : Plan.HashRecordV1) : CJson := obj [
  ("algorithm", .str record.algorithm), ("domain", .str record.domain),
  ("digest", .str record.digest)]
private def linkerJson (linker : Plan.LinkerDescriptorV1) : CJson := obj [
  ("semantics", .str linker.semantics), ("source_schema", .str linker.sourceSchema),
  ("plan_schema", .str linker.planSchema), ("identity_scheme", .str linker.identityScheme),
  ("canonical_encoding", .str linker.canonicalEncoding),
  ("source_map_schema", .str linker.sourceMapSchema)]
private def sourceMapLeafJson (leaf : Composition.SourceMapLeafV1) : CJson := obj [
  ("occurrence", .str leaf.occurrence),
  ("definition", .str leaf.definition),
  ("instance_path", arr (leaf.instancePath.map CJson.str)),
  ("display_path", .str leaf.displayPath)]
private def sourceMapJson (sourceMap : Composition.SourceMapV1) : CJson := obj [
  ("schema_version", .str sourceMap.schemaVersion),
  ("leaves", arr ((sortBy sourceMap.leaves (·.occurrence)).map sourceMapLeafJson)),
  ("boundary", arr (sourceMap.boundary.map CJson.str)),
  ("hidden", arr (sourceMap.hidden.map CJson.str))]
private def provenanceJson (provenance : Plan.LinkedProvenanceV1) : CJson := obj [
  ("source_hash", hashRecordJson provenance.sourceHash),
  ("linker", linkerJson provenance.linker),
  ("source_map", sourceMapJson provenance.sourceMap)]
private def schedulerJson (domain : Plan.SchedulerDomainV1) : CJson := obj [
  ("id", .str domain.id), ("algorithm", .str domain.algorithm),
  ("leaves", arr (domain.leaves.map CJson.str))]
private def leafJson (leaf : Plan.LeafIdentityV1) : CJson := obj [
  ("box", .str leaf.box), ("occurrence", .str leaf.occurrence)]
private def transitionIdentityJson (transition : Plan.TransitionIdentityV1) : CJson := obj [
  ("box", .str transition.box), ("name", .str transition.name),
  ("identity", .str transition.identity),
  ("rule_word", .int (Int.ofNat transition.ruleWord.toNat))]
private def mailboxJson (mailbox : Plan.MailboxIdentityV1) : CJson := obj [
  ("identity", .str mailbox.identity), ("source_box", .str mailbox.sourceBox),
  ("source_port", .str mailbox.sourcePort), ("target_box", .str mailbox.targetBox),
  ("target_port", .str mailbox.targetPort)]
private def identityJson (identity : Plan.IdentityMapV1) : CJson := obj [
  ("model_id", .str identity.modelId),
  ("enabled_features", arr (identity.enabledFeatures.map CJson.str)),
  ("scheduler_domains", arr (identity.schedulerDomains.map schedulerJson)),
  ("leaves", arr ((sortBy identity.leaves (·.box)).map leafJson)),
  ("transitions", arr ((sortBy identity.transitions (·.identity)).map transitionIdentityJson)),
  ("mailboxes", arr ((sortBy identity.mailboxes (·.identity)).map mailboxJson))]

/-- Encode the complete executable envelope, omitting absent linked provenance. -/
def planToCJson (plan : Plan.ExecutablePlanV1) : CJson :=
  let required := [
    ("schema_version", .str plan.schemaVersion),
    ("identity_scheme", .str plan.identityScheme),
    ("origin", originJson plan.origin),
    ("model", modelToCJson plan.model),
    ("identity", identityJson plan.identity)]
  let fields := match plan.linkedProvenance with
    | none => required
    | some provenance => required ++ [("linked_provenance", provenanceJson provenance)]
  obj fields

/-- The exact payload Rust hashes under `sembla.plan-core/v1`. -/
def semanticPayloadToCJson (plan : Plan.ExecutablePlanV1) : CJson := obj [
  ("identity", identityJson plan.identity),
  ("identity_scheme", .str plan.identityScheme),
  ("model", modelToCJson plan.model),
  ("schema_version", .str plan.schemaVersion)]

/-- Canonical plan bytes as a String, deliberately without a trailing newline. -/
def renderPlan (plan : Plan.ExecutablePlanV1) : String :=
  (planToCJson plan).render

end Sembla.PlanJson
