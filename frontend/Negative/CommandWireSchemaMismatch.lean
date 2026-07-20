import Sembla.DSL
open Sembla.DSL
sembla_model SchemaMismatch (dt := 1.0) where
  box a where
    system A (rows := 1) where
      x : Int
    output p from A where
      x : Int := count where x = 1
  box b where
    system B (rows := 1)
    input p where
      x : ℝ
  wire a p -> b p
