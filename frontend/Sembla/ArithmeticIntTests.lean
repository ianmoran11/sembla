import Sembla.Json
import Sembla.DSL

namespace Sembla.ArithmeticIntTests
open Sembla.IR Sembla.DSL

/- The exported end-to-end fixture: its Int parameter participates in the
   guard, while the effect reads `counter` from the old tick snapshot. -/
sembla_model incrementModel
    (name := "arithmetic_int_increment")
    (dt := 1.0) where
  param retirement_months : Int := 10
  param adjustment : ℝ := 0.5

  box increments where
    system Row (rows := 1) where
      counter : Int
      score : ℝ

    transition increment on Row where
      guard counter < retirement_months
      hazard 1e300
      set counter := counter + 1
      set score := score + adjustment

    view counter := max Row using counter

/-- The list frontend and command frontend must feed the same semantic kernel. -/
private def listIncrementModel : Model := model% "arithmetic_int_increment" step(1.0) where
  params [param retirement_months : Int := 10, param adjustment : ℝ := 0.5]
  boxes [box increments where
    systems [system Row (rows := 1) where [counter : Int, score : ℝ]]
    inputs []
    transitions [transition increment on Row where
      guard counter < retirement_months
      hazard 1e300
      set [counter := counter + 1, score := score + adjustment]]
    outputs []
    views [view counter from Row using counter reduce max]]
  wires []

#guard incrementModel == listIncrementModel
#guard toJson incrementModel == toJson listIncrementModel
#guard incrementModel.params ==
  [ ParamDecl.mk "retirement_months" ParamType.int (ParamValue.int 10) none
  , ParamDecl.mk "adjustment" ParamType.real (ParamValue.real 0.5) none
  ]

private def incrementTransition? : Option Transition := do
  let modelBox ← incrementModel.boxes.head?
  modelBox.transitions.head?

#guard incrementTransition?.map (·.guard) ==
  some (.lt (.selfAttr "counter") (.param "retirement_months"))
#guard incrementTransition?.map (·.effects) == some
  [ .setAttr "counter" (.add (.selfAttr "counter") (.int 1))
  , .setAttr "score" (.add (.selfAttr "score") (.param "adjustment"))
  ]

/- Parentheses are syntax-only: the old literal and a trivial expression emit
   byte-identical IR. -/
private def literalEffect : Model := model% "literal_effect_twin" step(1.0) where
  params []
  boxes [box values where
    systems [system Row (rows := 1) where [score : ℝ]]
    inputs []
    transitions [transition assign on Row where
      guard score = 0.0
      hazard 1e300
      set [score := 0.4]]
    outputs []]
  wires []

private def parenthesizedEffect : Model := model% "literal_effect_twin" step(1.0) where
  params []
  boxes [box values where
    systems [system Row (rows := 1) where [score : ℝ]]
    inputs []
    transitions [transition assign on Row where
      guard score = 0.0
      hazard 1e300
      set [score := (0.4)]]
    outputs []]
  wires []

#guard literalEffect == parenthesizedEffect
#guard toJson literalEffect == toJson parenthesizedEffect

sembla_model NegativeIntDefault
    (name := "negative_int_default")
    (dt := 1.0) where
  param offset : Int := -3
  box values where
    system Row (rows := 1)

#guard NegativeIntDefault.params ==
  [ParamDecl.mk "offset" ParamType.int (ParamValue.int (-3)) none]

/-- Canonical bytes used to generate the Rust integration-test fixture. -/
def incrementModelJson : String := toJson incrementModel

end Sembla.ArithmeticIntTests
