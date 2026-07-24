import Sembla.DSL
import Sembla.Json
import Sembla.PlanExport

/-!
# Aggregate birth/death demographic slots

`demographicSlots` is the first test-scale demographic model assembled from the
surface DSL. A logical person is identified by `(slot, generation)`: activating
a vacant slot increments `generation`, so reuse denotes a new person.

DECISIONS §K10 deliberately gives every new entrant a one-tick exit lockout.
Its birth or migration marker is visible for exactly the committed entry tick;
on the next tick `clear_event` removes it, while every exit guard reads the old
marker and therefore remains ineligible. The golden run measures this cost as
`locked_out_total` instead of hiding it. Internal arrivals are independent
activations under §K9, so their national residual is reported, never repaired.
-/

namespace Sembla.Models

open Sembla.IR Sembla.DSL

sembla_model demographicSlots
    (name := "demographic_slots")
    (dt := 1.0) where
  param birth_rate : ℝ := 0.025 ~ LogNormal (-3.6888794541139363) 0.2
  param mortality_young : ℝ := 0.001 ~ LogNormal (-6.907755278982137) 0.2
  param mortality_adult : ℝ := 0.003 ~ LogNormal (-5.809142990314028) 0.2
  param mortality_old : ℝ := 0.012 ~ LogNormal (-4.422848629194137) 0.2
  param overseas_arrival_rate : ℝ := 0.02 ~ LogNormal (-3.912023005428146) 0.2
  param emigration_rate : ℝ := 0.002 ~ LogNormal (-6.214608098422191) 0.2
  param internal_departure_rate : ℝ := 0.0025 ~ LogNormal (-5.991464547107982) 0.2
  param internal_arrival_rate : ℝ := 0.018 ~ LogNormal (-4.017383521085972) 0.2

  box demographic where
    -- Direct-stable plans canonicalize table order, so declaration order is
    -- canonical too; one state artifact therefore loads through model or plan.
    system Area (rows := 4) where
      area_key : Int
    system PersonSlot (rows := 5_000) where
      occupancy : {vacant, present}
      event : {none_, birth, death, overseas_arrival, overseas_departure,
        internal_arrival, internal_departure}
      sex : {male, female}
      age_months : Int
      event_age_months : Int
      generation : Int
      entry_stream : {birth_slot, overseas_slot, internal_slot}
      entry_age_months : Int
      area : Area
      slot_resource : SlotResource
    -- Empty tables are supported; this table supplies one exclusive resource
    -- row per person slot without inventing placeholder state.
    system SlotResource (rows := 5_000)

    transition age_monthly on PersonSlot where
      guard occupancy = present
      hazard 1e300
      set age_months := age_months + 1

    transition clear_event on PersonSlot where
      guard event ≠ none_
      hazard 1e300
      set event := none_

    transition birth_activate on PersonSlot where
      guard occupancy = vacant ∧ event = none_ ∧ entry_stream = birth_slot
      hazard birth_rate
      set occupancy := present
      set event := birth
      set age_months := 0
      set event_age_months := 0
      set generation := generation + 1

    transition overseas_arrive on PersonSlot where
      guard occupancy = vacant ∧ event = none_ ∧ entry_stream = overseas_slot
      hazard overseas_arrival_rate
      set occupancy := present
      set event := overseas_arrival
      set age_months := entry_age_months
      set event_age_months := entry_age_months
      set generation := generation + 1

    transition internal_arrive on PersonSlot where
      guard occupancy = vacant ∧ event = none_ ∧ entry_stream = internal_slot
      hazard internal_arrival_rate
      set occupancy := present
      set event := internal_arrival
      set age_months := entry_age_months
      set event_age_months := entry_age_months
      set generation := generation + 1

    transition die_young on PersonSlot where
      guard occupancy = present ∧ event = none_ ∧ age_months < 240
      hazard mortality_young
      contest slot_resource by race_time
      set event_age_months := age_months
      set occupancy := vacant
      set event := death

    transition die_adult on PersonSlot where
      guard occupancy = present ∧ event = none_ ∧ age_months > 239 ∧ age_months < 780
      hazard mortality_adult
      contest slot_resource by race_time
      set event_age_months := age_months
      set occupancy := vacant
      set event := death

    transition die_old on PersonSlot where
      guard occupancy = present ∧ event = none_ ∧ age_months > 779
      hazard mortality_old
      contest slot_resource by race_time
      set event_age_months := age_months
      set occupancy := vacant
      set event := death

    transition emigrate on PersonSlot where
      guard occupancy = present ∧ event = none_
      hazard emigration_rate
      contest slot_resource by race_time
      set event_age_months := age_months
      set occupancy := vacant
      set event := overseas_departure

    transition internal_depart on PersonSlot where
      guard occupancy = present ∧ event = none_
      hazard internal_departure_rate
      contest slot_resource by race_time
      set event_age_months := age_months
      set occupancy := vacant
      set event := internal_departure

    view population := count PersonSlot where occupancy = present
    view invalid_age := count PersonSlot where occupancy = present ∧ age_months < 0
    view males := count PersonSlot where occupancy = present ∧ sex = male
    view females := count PersonSlot where occupancy = present ∧ sex = female
    view age_00_04 := count PersonSlot where
      occupancy = present ∧ age_months < 60
    view age_20_24 := count PersonSlot where
      occupancy = present ∧ age_months > 239 ∧ age_months < 300
    view age_65_69 := count PersonSlot where
      occupancy = present ∧ age_months > 779 ∧ age_months < 840
    view age_85_plus := count PersonSlot where
      occupancy = present ∧ age_months > 1019
    view births_this_tick := count PersonSlot where event = birth
    view deaths_this_tick := count PersonSlot where event = death
    view overseas_arrivals_this_tick := count PersonSlot where event = overseas_arrival
    view overseas_departures_this_tick := count PersonSlot where event = overseas_departure
    view internal_arrivals_this_tick := count PersonSlot where event = internal_arrival
    view internal_departures_this_tick := count PersonSlot where event = internal_departure
    view locked_out := count PersonSlot where occupancy = present ∧ event ≠ none_
    view vacant_birth_slots := count PersonSlot where
      occupancy = vacant ∧ entry_stream = birth_slot ∧ event = none_
    view vacant_overseas_slots := count PersonSlot where
      occupancy = vacant ∧ entry_stream = overseas_slot ∧ event = none_
    view vacant_internal_slots := count PersonSlot where
      occupancy = vacant ∧ entry_stream = internal_slot ∧ event = none_
    view max_generation := max PersonSlot using generation

    grouped view population_cells :=
      count PersonSlot by sex, area, band age_months 60 where occupancy = present
    grouped view deaths_cells :=
      count PersonSlot by sex, band event_age_months 60 where event = death
    grouped view vacancy_cells :=
      count PersonSlot by entry_stream, area where occupancy = vacant ∧ event = none_

  summary final_population := last demographic.population
  summary maximum_invalid_age_count := max demographic.invalid_age
  summary births_total := sum demographic.births_this_tick
  summary deaths_total := sum demographic.deaths_this_tick
  summary overseas_arrivals_total := sum demographic.overseas_arrivals_this_tick
  summary overseas_departures_total := sum demographic.overseas_departures_this_tick
  summary internal_arrivals_total := sum demographic.internal_arrivals_this_tick
  summary internal_departures_total := sum demographic.internal_departures_this_tick
  summary minimum_vacant_birth_slots := min demographic.vacant_birth_slots
  summary final_max_generation := last demographic.max_generation
  summary locked_out_total := sum demographic.locked_out

/-- Canonical legacy-model bytes used by Rust parity and demographic fixtures. -/
def demographicSlotsJson : String := toJson demographicSlots

/-- Direct-stable feature-bearing plan used by the next demographic PRD. -/
def demographicSlotsPlanJson : String :=
  match PlanExport.directStablePlan demographicSlots with
  | .error message => message
  | .ok plan => PlanJson.planToCJson plan |>.render

end Sembla.Models
