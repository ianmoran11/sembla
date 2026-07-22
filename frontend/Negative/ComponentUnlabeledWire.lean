import Sembla.Composition.SurfaceModels
open Sembla.Composition.SurfaceModels

sembla_component UnlabeledWire where
  instance population := Population
  instance policy := Policy
  wire population.infection_count -> policy.infection_count
