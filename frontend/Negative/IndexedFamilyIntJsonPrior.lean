import Sembla.DSL

sembla_model IntFamilyJsonPrior (dt := 1.0) where
  index age := 0 .. 1
  index sex := {male, female}
  param tally[age, sex] : Int from json "../Sembla/TestData/IndexedFamilies/beta.json" sha256
    "8236ce78ea61f379f10919456a06b231dac5d22daadffc74ea12b6e0b30a30b7"
