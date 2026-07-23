import Sembla.DSL

open Sembla.DSL

sembla_model BadIntParamPrior (dt := 1.0) where
  param n : Int := 5 ~ LogNormal 0.0 1.0
  box values where
    system Row (rows := 1)
