import Sembla.DSL

set_option sembla.maxFamilyExpansion 63 in
sembla_model RelationCap (dt := 1.0) where
  domain Area := {nsw, vic, qld, sa, wa, tas, nt, act}
  box population where
    system Person (rows := 1) where
      place : Area
      status : {present, vacant}
    state Active on Person where
      status := present
    relation route (origin : Area, target : Area) on Person subject to origin ≠ target where
      from Active
      hazard 0.1
      set place := target
