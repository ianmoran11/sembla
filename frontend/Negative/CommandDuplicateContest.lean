import Sembla.DSL
open Sembla.DSL
sembla_model DuplicateContest (dt := 1.0) where
  box world where
    system Resource (rows := 1)
    system Slot (rows := 1) where
      occupancy : {present, vacant}
      slot_resource : Resource
    transition exit on Slot where
      guard occupancy = present
      hazard 1.0
      contest slot_resource by race_time
      contest slot_resource by race_time
      set occupancy := vacant
