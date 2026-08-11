import Sembla.DSL
open Sembla.DSL

sembla_model bad (dt := 1.0) where
  box world where
    system Person (rows := 1) where
      sex : {a, b}
    views Person where
      cells := count by band(sex, 2)
