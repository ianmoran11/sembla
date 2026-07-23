import Sembla.DSL

open Sembla.DSL

sembla_model BadUnknownEffectIdentifier (dt := 1.0) where
  box values where
    system Row (rows := 1) where
      counter : Int
    transition update on Row where
      guard counter = counter
      hazard 1e300
      set counter := missing + 1
