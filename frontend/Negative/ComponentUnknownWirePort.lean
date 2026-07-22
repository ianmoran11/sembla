import Sembla.Composition.SurfaceModels
open Sembla.Composition.SurfaceModels

sembla_component UnknownWirePort where
  instance population := Population
  instance policy := Policy
  wire bad : population.missing -> policy.infection_count
