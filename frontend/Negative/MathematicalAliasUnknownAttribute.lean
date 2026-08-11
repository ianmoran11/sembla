import Sembla.DSL

sembla_model AliasUnknownAttribute (dt := 1.0) where
  box population where
    system Person (rows := 1)
    state Active on Person where
      missing := 1
