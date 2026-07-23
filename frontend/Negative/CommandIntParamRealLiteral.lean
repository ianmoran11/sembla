import Sembla.DSL

open Sembla.DSL

sembla_model BadIntParamRealLiteral (dt := 1.0) where
  param n : Int := 0.5
  box values where
    system Row (rows := 1)
