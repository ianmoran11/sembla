import Sembla.DSL
open Sembla.IR Sembla.DSL

def nonBooleanAndAlias : Model := model% "negative" step(1.0) where
  params []
  boxes [box demo where
    systems [system Person (rows := 1) where [attr left : ℝ, attr right : ℝ]]
    inputs []
    transitions [transition bad on Person where
      guard left ∧ right
      hazard 0.1
      set [left := 1.0]]
    outputs []]
  wires []
