import Sembla.DSL

sembla_model IntFamilyRealDefault (dt := 1.0) where
  index age := 0 .. 0
  param tally[age] : Int where
    [0] := 1.5
