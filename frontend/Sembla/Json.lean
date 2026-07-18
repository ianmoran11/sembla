import Lean.Data.Json
import Sembla.IR

namespace Sembla.IR

private def quote (value : String) : String := (Lean.Json.str value).compress
private def array (values : List String) : String := "[" ++ String.intercalate "," values ++ "]"
private def object (fields : List (String × String)) : String :=
  "{" ++ String.intercalate "," (fields.map fun (name, value) => quote name ++ ":" ++ value) ++ "}"
private def opt (f : α → String) : Option α → String
  | none => "null"
  | some value => f value
/-- Serialize an exact decimal using the same plain/scientific spelling
    thresholds as serde_json's finite-f64 formatter. This keeps Lean exports
    byte-compatible with Rust-canonical IR without losing source digits. -/
private def scientific (value : Scientific) : String :=
  if value.coefficient == 0 then "0"
  else
    let negative := value.coefficient < 0
    let sign := if negative then "-" else ""
    let digits := toString value.coefficient.natAbs
    let digitCount := Int.ofNat digits.length
    let normalizedExponent := value.exponent + digitCount - 1
    if normalizedExponent < -5 || normalizedExponent >= 16 then
      let rest := digits.drop 1
      let mantissa :=
        if rest.isEmpty then digits.take 1
        else digits.take 1 ++ "." ++ rest
      let exponentSign := if normalizedExponent >= 0 then "+" else ""
      sign ++ mantissa ++ "e" ++ exponentSign ++ toString normalizedExponent
    else
      let decimalPosition := digitCount + value.exponent
      if decimalPosition <= 0 then
        sign ++ "0." ++ String.mk (List.replicate (-decimalPosition).toNat '0') ++ digits
      else if decimalPosition >= digitCount then
        sign ++ digits ++ String.mk (List.replicate (decimalPosition - digitCount).toNat '0')
      else
        let position := decimalPosition.toNat
        sign ++ digits.take position ++ "." ++ digits.drop position

private def paramTypeJson : ParamType → String
  | .real => quote "real"
  | .int => quote "int"
private def paramValueJson : ParamValue → String
  | .real value => object [("kind", quote "real"), ("value", scientific value)]
  | .int value => object [("kind", quote "int"), ("value", toString value)]
private def priorFamilyJson : PriorFamily → String
  | .normal => quote "normal"
  | .logNormal => quote "log_normal"
  | .uniform => quote "uniform"
private def priorJson (prior : Prior) : String := object [
  ("family", priorFamilyJson prior.family),
  ("args", array (prior.args.map scientific))]
private def paramJson (param : ParamDecl) : String := object [
  ("name", quote param.name), ("ty", paramTypeJson param.ty),
  ("default", paramValueJson param.default), ("prior", opt priorJson param.prior)]

private def attrTypeJson : AttrType → String
  | .real => object [("kind", quote "real")]
  | .int => object [("kind", quote "int")]
  | .enum variants => object [("kind", quote "enum"), ("variants", array (variants.map quote))]
  | .ref table => object [("kind", quote "ref"), ("table", quote table)]
private def attrJson (attr : Attr) : String := object [("name", quote attr.name), ("ty", attrTypeJson attr.ty)]
private def tableJson (table : Table) : String := object [
  ("name", quote table.name), ("size_hint", toString table.sizeHint),
  ("attrs", array (table.attrs.map attrJson))]

mutual
  private partial def exprJson : Expr → String
    | .real value => taggedValue "real" (scientific value)
    | .int value => taggedValue "int" (toString value)
    | .bool value => taggedValue "bool" (if value then "true" else "false")
    | .enum variant => object [("kind", quote "enum"), ("variant", quote variant)]
    | .param name => object [("kind", quote "param"), ("name", quote name)]
    | .selfAttr name => object [("kind", quote "self_attr"), ("name", quote name)]
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
    | .not expr => object [("kind", quote "not"), ("expr", exprJson expr)]
    | .enumIs attr variant => object [
        ("kind", quote "enum_is"), ("attr", quote attr), ("variant", quote variant)]
    | .input port agg => object [
        ("kind", quote "input"), ("port", quote port), ("agg", aggregateJson agg)]
    | .agg op table fk selfFk filter => object [
        ("kind", quote "agg"), ("op", aggOpJson op), ("table", quote table),
        ("on", object [("fk_attr", quote fk), ("self_fk_attr", quote selfFk)]),
        ("filter", exprJson filter)]

  private partial def aggOpJson : AggOp → String
    | .count => object [("kind", quote "count")]
    | .sum value => object [("kind", quote "sum"), ("value", exprJson value)]

  private partial def aggregateJson : Aggregate → String
    | .mk op filter => object [("op", aggOpJson op), ("filter", opt exprJson filter)]

  private partial def binary (kind : String) (lhs rhs : Expr) : String := object [
    ("kind", quote kind), ("lhs", exprJson lhs), ("rhs", exprJson rhs)]

  private partial def taggedValue (kind value : String) : String := object [
    ("kind", quote kind), ("value", value)]
end

private def effectJson : Effect → String
  | .setAttr attr value => object [("kind", quote "set_attr"), ("attr", quote attr), ("value", exprJson value)]
private def orderingJson : ClaimOrdering → String
  | .raceTime => object [("kind", quote "race_time")]
  | .key expr => object [("kind", quote "key"), ("expr", exprJson expr)]
private def claimJson (claim : ResourceClaim) : String := object [
  ("resource", exprJson claim.resource), ("ordering", orderingJson claim.ordering)]
private def transitionJson (transition : Transition) : String := object [
  ("name", quote transition.name), ("table", quote transition.table),
  ("guard", exprJson transition.guard), ("hazard", exprJson transition.hazard),
  ("effects", array (transition.effects.map effectJson)),
  ("contests", array (transition.contests.map claimJson))]
private def portJson (port : PortDecl) : String := object [
  ("name", quote port.name), ("schema", array (port.schema.map attrJson))]
private def fieldJson (field : OutputField) : String := object [
  ("name", quote field.name), ("op", aggOpJson field.op), ("filter", opt exprJson field.filter)]
private def builderJson : OutputBuilder → String
  | .perTable table fields => object [
      ("kind", quote "per_table"), ("table", quote table), ("fields", array (fields.map fieldJson))]
private def outputJson (output : OutputDecl) : String := object [
  ("name", quote output.name), ("schema", array (output.schema.map attrJson)),
  ("builder", builderJson output.builder)]
/-- PRD 0002 keeps the existing Lean surface views-free while emitting the
    additive observation fields required by Rust-canonical IR. PRD 0004 adds
    declarations to the Lean IR and replaces these empty arrays. -/
private def boxJson (box : Box) : String := object [
  ("name", quote box.name), ("tables", array (box.tables.map tableJson)),
  ("transitions", array (box.transitions.map transitionJson)),
  ("inputs", array (box.inputs.map portJson)), ("outputs", array (box.outputs.map outputJson)),
  ("views", array [])]
private def endpointJson (endpoint : WireEndpoint) : String := object [
  ("box", quote endpoint.box), ("port", quote endpoint.port)]
private def wireJson (wire : Wire) : String := object [
  ("from", endpointJson wire.source), ("to", endpointJson wire.target)]

def toJson (model : Model) : String := object [
  ("name", quote model.name), ("dt", scientific model.dt),
  ("params", array (model.params.map paramJson)), ("boxes", array (model.boxes.map boxJson)),
  ("wires", array (model.wires.map wireJson)), ("summaries", array [])] ++ "\n"

end Sembla.IR
