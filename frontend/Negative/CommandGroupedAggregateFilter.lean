import Sembla.DSL
open Sembla.DSL

sembla_model bad (dt := 1.0) where
  box world where
    system Area (rows := 1)
    system Person (rows := 1) where
      area : Area
      sex : {a, b}
    grouped view cells := count Person by sex where countBy area (sex = a) > 0
