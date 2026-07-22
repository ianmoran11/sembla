import Sembla.Composition.SurfaceModels
open Sembla.Composition.SurfaceModels

sembla_component DuplicateWireLabel where
  instance population := Population
  instance policy := Policy
  wire duplicate : population.infection_count -> policy.infection_count
  wire duplicate : policy.restriction_modifier -> population.restriction_modifier
