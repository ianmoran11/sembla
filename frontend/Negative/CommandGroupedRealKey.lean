import Sembla.DSL
open Sembla.DSL

sembla_model bad (dt := 1.0) where
  box world where
    system Person (rows := 1) where
      rate : ℝ
    grouped view cells := count Person by rate
