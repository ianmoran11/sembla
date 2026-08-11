import Sembla.DSL

sembla_model CsvV1Normal (dt := 1.0) where
  domain group := {a, b}
  param mixed[group] : ℝ from csv "../Sembla/TestData/IndexedFamilies/mixed-priors-v2.csv" sha256
    "63fd9f51d41dd99abaa79872c04e8a5470de998e82552d1cd1646247759444de"
