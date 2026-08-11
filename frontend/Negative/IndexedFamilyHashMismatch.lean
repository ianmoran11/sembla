import Sembla.DSL

sembla_model FamilyHashMismatch (dt := 1.0) where
  index age := 0 .. 1
  index sex := {male, female}
  param beta[age, sex] : ℝ from csv "../Sembla/TestData/IndexedFamilies/beta.csv" sha256
    "0000000000000000000000000000000000000000000000000000000000000000"
