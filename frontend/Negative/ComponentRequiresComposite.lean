import Sembla.Composition.SurfaceModels
open Sembla.Composition.SurfaceModels

sembla_component RequiresComposite where
  requires beta
  instance population := Population
