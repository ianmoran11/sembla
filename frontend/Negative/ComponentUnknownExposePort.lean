import Sembla.Composition.SurfaceModels
open Sembla.Composition.SurfaceModels

sembla_component UnknownExposePort where
  instance population := Population
  expose bad : population.missing as missing
