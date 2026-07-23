import Sembla.DSL
open Sembla.DSL

sembla_model bad (dt := 1.0) where
  box world where
    system Person (rows := 1) where
      sex : {a, b}
    grouped view cells := count Person by band sex 2
