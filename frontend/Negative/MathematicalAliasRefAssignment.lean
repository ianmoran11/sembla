import Sembla.DSL

sembla_model AliasRefAssignment (dt := 1.0) where
  box population where
    system Resource (rows := 1)
    system Person (rows := 1) where
      resource : Resource
    state Active on Person where
      resource := 1
