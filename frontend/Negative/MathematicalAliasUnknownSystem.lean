import Sembla.DSL

sembla_model AliasUnknownSystem (dt := 1.0) where
  box population where
    state Active on Missing where
      match true
