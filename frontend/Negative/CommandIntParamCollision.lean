import Sembla.DSL

open Sembla.DSL

sembla_model BadIntParamCollision (dt := 1.0) where
  param n : Int := 1
  param n : Int := 2
  box values where
    system Row (rows := 1)
