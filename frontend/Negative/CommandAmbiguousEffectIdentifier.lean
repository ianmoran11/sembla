import Sembla.DSL

open Sembla.DSL

sembla_model BadAmbiguousEffectIdentifier (dt := 1.0) where
  param counter : Int := 1
  box values where
    system Row (rows := 1) where
      counter : Int
      result : Int
    transition update on Row where
      guard result = result
      hazard 1e300
      set result := counter + 1
