import Sembla.Composition.SurfaceModels
open Sembla.Composition.SurfaceModels

sembla_component UnknownHidePort where
  instance population := Population
  hide population.missing
