import Sembla.DSL

open Sembla.DSL

sembla_model BadEffectIntType (dt := 1.0) where
  box values where
    system Row (rows := 1) where
      age_months : Int
    transition update on Row where
      guard age_months = age_months
      hazard 1e300
      set age_months := 0.5
