import Sembla.DSL

/-!
# Australian population microsimulation

`australianPopulationSchema` preserves the public schema-only structure used by
existing callers. `australianPopulationDeclarative` authors the complete model
with named domains, a projected age partition, pinned generated parameter
families, reusable state aliases, and relation expansion. Both retain the
canonical one-in-a-hundred row hints used by the initial-state fixture.
-/

namespace Sembla.Models

open Sembla.IR Sembla.DSL

sembla_model australianPopulationSchema
    (name := "australian_population")
    (dt := 1.0) where
  box demographic where
    system PersonSlot (rows := 352_460) where
      occupancy : {vacant, present}
      event : {none_, birth, death, overseas_arrival, overseas_departure,
        interstate_move}
      sex : {male, female}
      age_months : Int
      event_age_months : Int
      generation : Int
      entry_stream : {birth_slot, overseas_slot, retired_slot}
      entry_age_months : Int
      area : {nsw, vic, qld, sa, wa, tas, nt, act}
      prev_area : {none_, nsw, vic, qld, sa, wa, tas, nt, act}
      slot_resource : SlotResource
    system SlotResource (rows := 352_460)

    views PersonSlot where
      population := count where occupancy = present
      births_this_tick := count where event = birth
      deaths_this_tick := count where event = death
      overseas_arrivals_this_tick := count where event = overseas_arrival
      overseas_departures_this_tick := count where event = overseas_departure
      interstate_moves_this_tick := count where event = interstate_move
      locked_out := count where occupancy = present ∧ event ≠ none_
      invalid_age := count where occupancy = present ∧ age_months < 0
      vacant_birth_slots := count where
        occupancy = vacant ∧ entry_stream = birth_slot ∧ event = none_
      vacant_overseas_slots := count where
        occupancy = vacant ∧ entry_stream = overseas_slot ∧ event = none_
      retired_slots := count where
        occupancy = vacant ∧ entry_stream = retired_slot ∧ event = none_
      max_generation := max generation

      population_cells := count
        where occupancy = present
        by area, sex, band(age_months, 60)
      population_single_year_cells := count
        where occupancy = present
        by area, sex, band(age_months, 12)
      interstate_flows := count
        where event = interstate_move
        by prev_area, area
      interstate_age_sex_flows := count
        where event = interstate_move
        by prev_area, area, sex, band(event_age_months, 60)
      births_cells := count
        where event = birth
        by area, sex
      deaths_cells := count
        where event = death
        by area, sex
      deaths_state_age_cells := count
        where event = death
        by area, sex, band(event_age_months, 60)
      overseas_arrival_cells := count
        where event = overseas_arrival
        by area, sex
      overseas_departure_cells := count
        where event = overseas_departure
        by area, sex
      vacancy_cells := count
        where occupancy = vacant ∧ event = none_
        by entry_stream, area

  summaries demographic where
    final_population := last population
    births_total := sum births_this_tick
    deaths_total := sum deaths_this_tick
    overseas_arrivals_total := sum overseas_arrivals_this_tick
    overseas_departures_total := sum overseas_departures_this_tick
    interstate_moves_total := sum interstate_moves_this_tick
    minimum_vacant_birth_slots := min vacant_birth_slots
    minimum_vacant_overseas_slots := min vacant_overseas_slots
    maximum_invalid_age_count := max invalid_age
    locked_out_total := sum locked_out
    final_max_generation := last max_generation

sembla_model australianPopulationDeclarative
    (name := "australian_population")
    (dt := 1.0) where
  domain Occupancy := {vacant, present}
  domain Event := {none_, birth, death, overseas_arrival,
    overseas_departure, interstate_move}
  domain Sex := {male, female}
  domain EntryStream := {birth_slot, overseas_slot, retired_slot}
  domain Area := {nsw, vic, qld, sa, wa, tas, nt, act}
  domain PreviousArea := {none_, nsw, vic, qld, sa, wa, tas, nt, act}

  partition AgeBand projects demographic.PersonSlot.age_months where
    «00_04» := 0 ..< 60
    «05_09» := 60 ..< 120
    «10_14» := 120 ..< 180
    «15_19» := 180 ..< 240
    «20_24» := 240 ..< 300
    «25_29» := 300 ..< 360
    «30_34» := 360 ..< 420
    «35_39» := 420 ..< 480
    «40_44» := 480 ..< 540
    «45_49» := 540 ..< 600
    «50_54» := 600 ..< 660
    «55_59» := 660 ..< 720
    «60_64» := 720 ..< 780
    «65_69» := 780 ..< 840
    «70_74» := 840 ..< 900
    «75_79» := 900 ..< 960
    «80_84» := 960 ..< 1020
    «85_89» := 1020 ..< 1080
    «90_94» := 1080 ..< 1140
    «95_99» := 1140 ..< 1200
    «100_plus» := 1200 ..

  param interstate_base : ℝ := 0.0001 ~ LogNormal (-9.210340371976184) 0.5
  param push_vic : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param pull_vic : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param push_qld : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param pull_qld : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param push_sa : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param pull_sa : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param push_wa : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param pull_wa : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param push_tas : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param pull_tas : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param push_nt : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param pull_nt : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param push_act : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param pull_act : ℝ := 1.0 ~ LogNormal 0.0 0.25
  param peak_months : ℝ := 360.0 ~ LogNormal 5.886104031450156 0.25
  param k : ℝ := 1e-05 ~ LogNormal (-11.512925464970229) 0.5

  function Push (area : Area) : ℝ where
    [nsw] := 1.0
    [vic] := push_vic
    [qld] := push_qld
    [sa] := push_sa
    [wa] := push_wa
    [tas] := push_tas
    [nt] := push_nt
    [act] := push_act

  function Pull (area : Area) : ℝ where
    [nsw] := 1.0
    [vic] := pull_vic
    [qld] := pull_qld
    [sa] := pull_sa
    [wa] := pull_wa
    [tas] := pull_tas
    [nt] := pull_nt
    [act] := pull_act

  param birth_rate (area : Area) : ℝ
    from json "Data/birth_rate.json"
    sha256 "509af5170d0dee42e58788d743838f9a76a39d00bb7de15d3353919e96b5d887"
  param mortality (area : Area, band : AgeBand, sex : Sex) : ℝ
    from json "Data/mortality.json"
    sha256 "970aa4d8c630237588419b6e92883ad5c36398696ad6bf11272392cb8c996bd5"
  param overseas_arrival (area : Area) : ℝ
    from json "Data/overseas_arrival.json"
    sha256 "f485b037975ac52607eb5a032a2fc0366cbf25c11df017c797d364be78ae19f6"
  param emigration (area : Area) : ℝ
    from json "Data/emigration.json"
    sha256 "9a8712d54ff70cd0b21de618703f9fb04a0a02baa91f4a4c2e7ed14121191f1a"

  box demographic where
    system PersonSlot (rows := 352_460) where
      occupancy : Occupancy
      event : Event
      sex : Sex
      age_months : Int
      event_age_months : Int
      generation : Int
      entry_stream : EntryStream
      entry_age_months : Int
      area : Area
      prev_area : PreviousArea
      slot_resource : SlotResource
    system SlotResource (rows := 352_460)

    state Present on PersonSlot where
      occupancy := present
    state Vacant on PersonSlot where
      occupancy := vacant
    state NoEvent on PersonSlot where
      event := none_
    state InArea (region : Area) on PersonSlot where
      area := region
    state HasSex (gender : Sex) on PersonSlot where
      sex := gender
    state BirthEntry on PersonSlot where
      entry_stream := birth_slot
    state OverseasEntry on PersonSlot where
      entry_stream := overseas_slot

    transition age_monthly on PersonSlot where
      guard occupancy = present
      hazard 1e300
      set age_months := age_months + 1

    transition clear_event on PersonSlot where
      guard ¬(event = none_)
      hazard 1e300
      set event := none_
      set prev_area := none_

    relation move (origin : Area, destination : Area) on PersonSlot
        subject to origin ≠ destination where
      from Present, NoEvent, InArea(origin)
      hazard ((interstate_base * Push(origin)) * Pull(destination)) *
        (1.0 / (1.0 + k * ((age_months - peak_months) *
          (age_months - peak_months))))
      claim slot_resource by race_time
      set prev_area := origin
      become InArea(destination)
      set event := interstate_move
      set event_age_months := age_months

    relation die (region : Area, band : AgeBand, gender : Sex) on PersonSlot where
      from Present, NoEvent, InArea(region), HasSex(gender), AgeBand(band)
      hazard mortality(region, band, gender)
      claim slot_resource by race_time
      set event_age_months := age_months
      set entry_stream := retired_slot
      become Vacant
      set event := death

    relation birth (region : Area) on PersonSlot where
      from Vacant, NoEvent, BirthEntry, InArea(region)
      hazard birth_rate(region)
      become Present
      set event := birth
      set age_months := 0
      set event_age_months := 0
      set generation := generation + 1

    relation overseas_arrive (region : Area) on PersonSlot where
      from Vacant, NoEvent, OverseasEntry, InArea(region)
      hazard overseas_arrival(region)
      become Present
      set event := overseas_arrival
      set age_months := entry_age_months
      set event_age_months := entry_age_months
      set generation := generation + 1

    relation emigrate (region : Area) on PersonSlot where
      from Present, NoEvent, InArea(region)
      hazard emigration(region)
      claim slot_resource by race_time
      set event_age_months := age_months
      set entry_stream := retired_slot
      become Vacant
      set event := overseas_departure

    views PersonSlot where
      population := count where occupancy = present
      births_this_tick := count where event = birth
      deaths_this_tick := count where event = death
      overseas_arrivals_this_tick := count where event = overseas_arrival
      overseas_departures_this_tick := count where event = overseas_departure
      interstate_moves_this_tick := count where event = interstate_move
      locked_out := count where occupancy = present ∧ event ≠ none_
      invalid_age := count where occupancy = present ∧ age_months < 0
      vacant_birth_slots := count where
        occupancy = vacant ∧ entry_stream = birth_slot ∧ event = none_
      vacant_overseas_slots := count where
        occupancy = vacant ∧ entry_stream = overseas_slot ∧ event = none_
      retired_slots := count where
        occupancy = vacant ∧ entry_stream = retired_slot ∧ event = none_
      max_generation := max generation

      population_cells := count
        where occupancy = present
        by area, sex, band(age_months, 60)
      population_single_year_cells := count
        where occupancy = present
        by area, sex, band(age_months, 12)
      interstate_flows := count
        where event = interstate_move
        by prev_area, area
      interstate_age_sex_flows := count
        where event = interstate_move
        by prev_area, area, sex, band(event_age_months, 60)
      births_cells := count
        where event = birth
        by area, sex
      deaths_cells := count
        where event = death
        by area, sex
      deaths_state_age_cells := count
        where event = death
        by area, sex, band(event_age_months, 60)
      overseas_arrival_cells := count
        where event = overseas_arrival
        by area, sex
      overseas_departure_cells := count
        where event = overseas_departure
        by area, sex
      vacancy_cells := count
        where occupancy = vacant ∧ event = none_
        by entry_stream, area

  summaries demographic where
    final_population := last population
    births_total := sum births_this_tick
    deaths_total := sum deaths_this_tick
    overseas_arrivals_total := sum overseas_arrivals_this_tick
    overseas_departures_total := sum overseas_departures_this_tick
    interstate_moves_total := sum interstate_moves_this_tick
    minimum_vacant_birth_slots := min vacant_birth_slots
    minimum_vacant_overseas_slots := min vacant_overseas_slots
    maximum_invalid_age_count := max invalid_age
    locked_out_total := sum locked_out
    final_max_generation := last max_generation

end Sembla.Models
