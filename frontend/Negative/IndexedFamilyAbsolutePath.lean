import Sembla.DSL

sembla_model FamilyAbsolutePath (dt := 1.0) where
  index age := 0 .. 0
  param beta[age] : ℝ from csv "/tmp/beta.csv" sha256
    "0000000000000000000000000000000000000000000000000000000000000000"
