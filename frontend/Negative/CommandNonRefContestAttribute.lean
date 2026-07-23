import Sembla.DSL
open Sembla.DSL
sembla_model NonRefContest (dt := 1.0) where
  box world where
    system Slot (rows := 1) where
      occupancy : {present, vacant}
      age_months : Int
    transition exit on Slot where
      guard occupancy = present
      hazard 1.0
      contest age_months by race_time
      set occupancy := vacant
