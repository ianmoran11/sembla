import Sembla.DSL

sembla_model IntFamilyPrior (dt := 1.0) where
  index age := 0 .. 0
  param tally[age] : Int where
    [0] := 1 ~ LogNormal 0.0 1.0
