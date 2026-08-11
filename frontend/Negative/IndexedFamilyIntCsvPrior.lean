import Sembla.DSL

sembla_model IntFamilyCsvPrior (dt := 1.0) where
  index age := 0 .. 1
  index sex := {male, female}
  param tally[age, sex] : Int from csv "../Sembla/TestData/IndexedFamilies/beta.csv" sha256
    "613195059792a4054b01c98e083d375865c7cfaf14515285cdc90b6cf04ce3fc"
