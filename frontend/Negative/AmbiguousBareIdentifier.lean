import Sembla.DSL
open Sembla.IR Sembla.DSL

def ambiguousBareIdentifier : Model := model% "negative" step(1.0) where
  params [param score : ℝ := 0.1]
  boxes [box demo where
    systems [system Person (rows := 1) where [attr score : ℝ]]
    inputs []
    transitions [transition bad on Person where
      guard score ≤ 1.0
      hazard 0.1
      set [score := 1.0]]
    outputs []]
  wires []
