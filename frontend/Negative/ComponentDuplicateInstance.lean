import Sembla.Composition.SurfaceModels
open Sembla.Composition.SurfaceModels

sembla_component DuplicateInstance where
  instance population := Population
  instance population := Population
