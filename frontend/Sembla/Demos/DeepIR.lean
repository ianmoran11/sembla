import Sembla.Json

/-!
# Deep-IR feature tour

The contextual DSL intentionally exposes the common modeling path. The pure
IR remains ordinary Lean data and also represents lower-level compiler/runtime
contracts such as all prior families, Boolean expression constructors,
resource contests, and deterministic claim orderings.

These values are construction examples, not a second surface language and not
a runtime execution inside Lean.
-/

namespace Sembla.Demos.DeepIR

open Sembla.IR

/-- Every parameter-prior family represented by the deep IR. -/
def priorGallery : List ParamDecl :=
  [ { name := "normal_rate"
      ty := .real
      default := .real 1.25
      prior := some { family := .normal, args := [0.0, 1.0] } }
  , { name := "lognormal_rate"
      ty := .real
      default := .real 0.8
      prior := some { family := .logNormal, args := [-0.2231435513142097, 0.25] } }
  , { name := "uniform_rate"
      ty := .real
      default := .real 0.5
      prior := some { family := .uniform, args := [0.0, 1.0] } }
  , { name := "integer_control"
      ty := .int
      default := .int 3
      prior := none }
  ]

/-- Attribute kinds available to systems, ports, and output schemas. -/
def attributeGallery : List Attr :=
  [ { name := "weight", ty := .real }
  , { name := "visits", ty := .int }
  , { name := "health", ty := .enum ["S", "I", "R"] }
  , { name := "employer", ty := .ref "employer" }
  , { name := "household", ty := .ref "household" }
  ]

/-- Representative scalar, Boolean, input, and relational-aggregate expressions. -/
def expressionGallery : List Expr :=
  [ .real 0.25
  , .int 7
  , .bool true
  , .enum "I"
  , .param "normal_rate"
  , .selfAttr "weight"
  , .add (.selfAttr "weight") (.real 1.0)
  , .sub (.selfAttr "weight") (.real 1.0)
  , .mul (.param "normal_rate") (.selfAttr "weight")
  , .div (.selfAttr "weight") (.real 2.0)
  , .eq (.selfAttr "visits") (.int 7)
  , .ne (.selfAttr "visits") (.int 0)
  , .lt (.selfAttr "weight") (.real 1.0)
  , .le (.selfAttr "weight") (.real 1.0)
  , .gt (.selfAttr "weight") (.real 0.0)
  , .ge (.selfAttr "weight") (.real 0.0)
  , .and (.bool true) (.not (.bool false))
  , .or (.bool false) (.enumIs "health" "I")
  , .input "signals" (.mk .count none)
  , .input "signals" (.mk (.sum (.selfAttr "weight"))
      (some (.gt (.selfAttr "weight") (.real 0.0))))
  , .agg .count "person" "employer" "employer" (.enumIs "health" "I")
  , .agg (.sum (.selfAttr "weight")) "person" "employer" "employer" (.bool true)
  ]

/-- A transition with both racing-clock and explicit-key resource contests. -/
def contestedMove : Transition :=
  { name := "move"
    table := "person"
    guard := .and (.enumIs "health" "I") (.gt (.selfAttr "weight") (.real 0.0))
    hazard := .param "normal_rate"
    effects := [.setAttr "health" (.enum "R")]
    contests :=
      [ { resource := .selfAttr "employer", ordering := .raceTime }
      , { resource := .selfAttr "household", ordering := .key (.selfAttr "visits") }
      ] }

/-- A minimal serializable model showing exact decimal and prior encoding. -/
def numericModel : Model :=
  { name := "direct_ir_numeric_gallery"
    dt := 0.125
    params := priorGallery
    boxes := []
    wires := []
    summaries := [] }

/-- Canonical JSON is the boundary consumed and validated by the Rust CLI. -/
def numericJson : String := Sembla.IR.toJson numericModel

#guard priorGallery.map (·.name) ==
  ["normal_rate", "lognormal_rate", "uniform_rate", "integer_control"]
#guard attributeGallery.length == 5
#guard expressionGallery.length == 22
#guard contestedMove.contests.map (·.ordering) == [ClaimOrdering.raceTime, .key (.selfAttr "visits")]
#guard numericJson.startsWith "{\"name\":\"direct_ir_numeric_gallery\",\"dt\":0.125"
#guard numericJson.endsWith "}\n"

end Sembla.Demos.DeepIR
