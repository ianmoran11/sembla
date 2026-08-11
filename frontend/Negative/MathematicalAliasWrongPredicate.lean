import Sembla.DSL

sembla_model AliasWrongPredicate (dt := 1.0) where
  box population where
    system Person (rows := 1) where
      age : Int
    state Active on Person where
      match age
