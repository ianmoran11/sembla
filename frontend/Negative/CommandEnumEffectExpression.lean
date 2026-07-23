import Sembla.DSL

open Sembla.DSL

sembla_model BadEnumEffectExpression (dt := 1.0) where
  box values where
    system Row (rows := 1) where
      mode : {Open, Closed}
    transition update on Row where
      guard mode = Open
      hazard 1e300
      set mode := mode + 1
