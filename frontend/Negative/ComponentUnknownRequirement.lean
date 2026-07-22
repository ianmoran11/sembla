import Sembla.Composition.SurfaceModels
open Sembla.Composition.SurfaceModels

sembla_component UnknownRequirement where
  instance population := Population (missing := beta)
