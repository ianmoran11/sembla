import Sembla.DSL
open Sembla.DSL
sembla_model ReactionContest (dt := 1.0) where
  box world where
    system Resource (rows := 1)
    system Slot (rows := 1) where
      occupancy : {present, vacant}
      slot_resource : Resource
    exit on Slot : occupancy : present → [1.0] vacant contest slot_resource by race_time
