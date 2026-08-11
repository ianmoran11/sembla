import Sembla.DSL

sembla_model AliasUnknownDomain (dt := 1.0) where
  box population where
    system Person (rows := 1)
    state Active (x : Missing) on Person where
      match true
