import Sembla.DSL

sembla_model FamilyMalformedHashMismatch (dt := 1.0) where
  index age := 0 .. 0
  param beta[age] : ℝ from csv "../Sembla/TestData/IndexedFamilies/malformed.csv" sha256
    "0000000000000000000000000000000000000000000000000000000000000000"
