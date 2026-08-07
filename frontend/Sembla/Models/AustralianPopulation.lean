import Sembla.DSL
import Sembla.Frontend.Builders.Transition
import Sembla.Json
import Sembla.PlanExport
import Sembla.Semantics.CheckModel

/-!
# Australian population microsimulation

The surface command declares the frozen schema and observations.  Movement and
mortality transitions are ordinary Lean data generated over the eight areas and
21 mortality bands, then appended to that checked surface model. This avoids 392
hand-copied transitions without adding syntax or extending the IR.

Rows in the canonical model are the one-in-a-hundred initial pool. The Python
state builder emits validation-safe companions with scale-specific
`person_slot` and `slot_resource` size hints and no feature-gated grouped views;
parameters, transitions and scalar observations remain canonical.
-/

namespace Sembla.Models

open Sembla.IR Sembla.DSL
open Sembla.Frontend.Builders

private def canonicalRows : Nat := 352460

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

    view population := count PersonSlot where occupancy = present
    view births_this_tick := count PersonSlot where event = birth
    view deaths_this_tick := count PersonSlot where event = death
    view overseas_arrivals_this_tick := count PersonSlot where
      event = overseas_arrival
    view overseas_departures_this_tick := count PersonSlot where
      event = overseas_departure
    view interstate_moves_this_tick := count PersonSlot where
      event = interstate_move
    view locked_out := count PersonSlot where
      occupancy = present ∧ event ≠ none_
    view invalid_age := count PersonSlot where
      occupancy = present ∧ age_months < 0
    view vacant_birth_slots := count PersonSlot where
      occupancy = vacant ∧ entry_stream = birth_slot ∧ event = none_
    view vacant_overseas_slots := count PersonSlot where
      occupancy = vacant ∧ entry_stream = overseas_slot ∧ event = none_
    view retired_slots := count PersonSlot where
      occupancy = vacant ∧ entry_stream = retired_slot ∧ event = none_
    view max_generation := max PersonSlot using generation

    grouped view population_cells :=
      count PersonSlot by area, sex, band age_months 60 where occupancy = present
    grouped view population_single_year_cells :=
      count PersonSlot by area, sex, band age_months 12 where occupancy = present
    grouped view interstate_flows :=
      count PersonSlot by prev_area, area where event = interstate_move
    grouped view interstate_age_sex_flows :=
      count PersonSlot by prev_area, area, sex, band event_age_months 60 where
        event = interstate_move
    grouped view births_cells :=
      count PersonSlot by area, sex where event = birth
    grouped view deaths_cells :=
      count PersonSlot by area, sex where event = death
    grouped view deaths_state_age_cells :=
      count PersonSlot by area, sex, band event_age_months 60 where event = death
    grouped view overseas_arrival_cells :=
      count PersonSlot by area, sex where event = overseas_arrival
    grouped view overseas_departure_cells :=
      count PersonSlot by area, sex where event = overseas_departure
    grouped view vacancy_cells :=
      count PersonSlot by entry_stream, area where
        occupancy = vacant ∧ event = none_

  summary final_population := last demographic.population
  summary births_total := sum demographic.births_this_tick
  summary deaths_total := sum demographic.deaths_this_tick
  summary overseas_arrivals_total := sum demographic.overseas_arrivals_this_tick
  summary overseas_departures_total := sum demographic.overseas_departures_this_tick
  summary interstate_moves_total := sum demographic.interstate_moves_this_tick
  summary minimum_vacant_birth_slots := min demographic.vacant_birth_slots
  summary minimum_vacant_overseas_slots := min demographic.vacant_overseas_slots
  summary maximum_invalid_age_count := max demographic.invalid_age
  summary locked_out_total := sum demographic.locked_out
  summary final_max_generation := last demographic.max_generation

private def areas : List String :=
  ["nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act"]

private def sexes : List String := ["male", "female"]

private structure MortalityBand where
  name : String
  lowerBound : ℤ
  upperBound : Option ℤ

private def mortalityBands : List MortalityBand :=
  [ { name := "00_04", lowerBound := 0, upperBound := some 60 }
  , { name := "05_09", lowerBound := 60, upperBound := some 120 }
  , { name := "10_14", lowerBound := 120, upperBound := some 180 }
  , { name := "15_19", lowerBound := 180, upperBound := some 240 }
  , { name := "20_24", lowerBound := 240, upperBound := some 300 }
  , { name := "25_29", lowerBound := 300, upperBound := some 360 }
  , { name := "30_34", lowerBound := 360, upperBound := some 420 }
  , { name := "35_39", lowerBound := 420, upperBound := some 480 }
  , { name := "40_44", lowerBound := 480, upperBound := some 540 }
  , { name := "45_49", lowerBound := 540, upperBound := some 600 }
  , { name := "50_54", lowerBound := 600, upperBound := some 660 }
  , { name := "55_59", lowerBound := 660, upperBound := some 720 }
  , { name := "60_64", lowerBound := 720, upperBound := some 780 }
  , { name := "65_69", lowerBound := 780, upperBound := some 840 }
  , { name := "70_74", lowerBound := 840, upperBound := some 900 }
  , { name := "75_79", lowerBound := 900, upperBound := some 960 }
  , { name := "80_84", lowerBound := 960, upperBound := some 1020 }
  , { name := "85_89", lowerBound := 1020, upperBound := some 1080 }
  , { name := "90_94", lowerBound := 1080, upperBound := some 1140 }
  , { name := "95_99", lowerBound := 1140, upperBound := some 1200 }
  , { name := "100_plus", lowerBound := 1200, upperBound := none }
  ]

private def logNormalParam (parameterName : String)
    (initialValue location spread : Scientific) : ParamDecl :=
  { name := parameterName
    ty := .real
    default := .real initialValue
    «prior» := some { family := .logNormal, args := [location, spread] } }

private def normalParam (parameterName : String)
    (initialValue location spread : Scientific) : ParamDecl :=
  { name := parameterName
    ty := .real
    default := .real initialValue
    «prior» := some { family := .normal, args := [location, spread] } }

/-- Positive fixed direct rates use median-centred LogNormal priors. Published
    zero mortality rates remain exact zeros and use a centred Normal prior with
    spread equal to half the ABS 0.1-per-1,000 rounding unit, divided by 12,000. -/
private def directLogNormalParam (parameterName : String)
    (initialValue location : Scientific) : ParamDecl :=
  logNormalParam parameterName initialValue location 0.5

private def directZeroMortalityParam (parameterName : String) : ParamDecl :=
  normalParam parameterName 0.0 0.0 0.000004166666666666667

set_option maxHeartbeats 800000 in
private def directParams : List ParamDecl :=
  [ directLogNormalParam "birth_rate_nsw" 0.005408332032700994 (-5.219814545546841)
  , directLogNormalParam "birth_rate_vic" 0.004831060415177164 (-5.3326892877589875)
  , directLogNormalParam "birth_rate_qld" 0.005432512989448913 (-5.215353454773702)
  , directLogNormalParam "birth_rate_sa" 0.005367819428515974 (-5.227333518381529)
  , directLogNormalParam "birth_rate_wa" 0.004906839844493987 (-5.317125160578194)
  , directLogNormalParam "birth_rate_tas" 0.00567856927731018 (-5.17105596577564)
  , directLogNormalParam "birth_rate_nt" 0.005298028488692229 (-5.240420510892615)
  , directLogNormalParam "birth_rate_act" 0.004936365572114512 (-5.311125932699634)
  , directLogNormalParam "mortality_nsw_00_04_male" 9.166666666666667e-05 (-9.297351748965813)
  , directLogNormalParam "mortality_nsw_00_04_female" 6.666666666666667e-05 (-9.615805480084347)
  , directLogNormalParam "mortality_nsw_05_09_male" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_nsw_05_09_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_nsw_10_14_male" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_nsw_10_14_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_nsw_15_19_male" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_nsw_15_19_female" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_nsw_20_24_male" 5e-05 (-9.903487552536127)
  , directLogNormalParam "mortality_nsw_20_24_female" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_nsw_25_29_male" 5e-05 (-9.903487552536127)
  , directLogNormalParam "mortality_nsw_25_29_female" 2.5e-05 (-10.596634733096073)
  , directLogNormalParam "mortality_nsw_30_34_male" 6.666666666666667e-05 (-9.615805480084347)
  , directLogNormalParam "mortality_nsw_30_34_female" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_nsw_35_39_male" 8.333333333333333e-05 (-9.392661928770137)
  , directLogNormalParam "mortality_nsw_35_39_female" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_nsw_40_44_male" 0.000125 (-8.987196820661973)
  , directLogNormalParam "mortality_nsw_40_44_female" 7.5e-05 (-9.498022444427964)
  , directLogNormalParam "mortality_nsw_45_49_male" 0.00019166666666666667 (-8.559752805835034)
  , directLogNormalParam "mortality_nsw_45_49_female" 0.00010833333333333333 (-9.130297664302647)
  , directLogNormalParam "mortality_nsw_50_54_male" 0.0002916666666666667 (-8.139898960274769)
  , directLogNormalParam "mortality_nsw_50_54_female" 0.000175 (-8.65072458404076)
  , directLogNormalParam "mortality_nsw_55_59_male" 0.00045 (-7.706262975199909)
  , directLogNormalParam "mortality_nsw_55_59_female" 0.00025833333333333334 (-8.261259817279036)
  , directLogNormalParam "mortality_nsw_60_64_male" 0.000675 (-7.300797867091744)
  , directLogNormalParam "mortality_nsw_60_64_female" 0.0004166666666666667 (-7.783224016336037)
  , directLogNormalParam "mortality_nsw_65_69_male" 0.0010916666666666666 (-6.8200496985630314)
  , directLogNormalParam "mortality_nsw_65_69_female" 0.0006666666666666666 (-7.313220387090301)
  , directLogNormalParam "mortality_nsw_70_74_male" 0.0017916666666666667 (-6.32460899363652)
  , directLogNormalParam "mortality_nsw_70_74_female" 0.0011 (-6.812445099177812)
  , directLogNormalParam "mortality_nsw_75_79_male" 0.003108333333333333 (-5.773668602120368)
  , directLogNormalParam "mortality_nsw_75_79_female" 0.0019416666666666666 (-6.2442085681984825)
  , directLogNormalParam "mortality_nsw_80_84_male" 0.005591666666666667 (-5.1864778847925015)
  , directLogNormalParam "mortality_nsw_80_84_female" 0.0038 (-5.572754212249797)
  , directLogNormalParam "mortality_nsw_85_89_male" 0.009883333333333333 (-4.616905442206512)
  , directLogNormalParam "mortality_nsw_85_89_female" 0.007216666666666666 (-4.9313621132017476)
  , directLogNormalParam "mortality_nsw_90_94_male" 0.017158333333333334 (-4.0652713147363935)
  , directLogNormalParam "mortality_nsw_90_94_female" 0.013791666666666667 (-4.28369073395302)
  , directLogNormalParam "mortality_nsw_95_99_male" 0.026 (-3.649658740960655)
  , directLogNormalParam "mortality_nsw_95_99_female" 0.023125 (-3.766840995583648)
  , directLogNormalParam "mortality_nsw_100_plus_male" 0.027941666666666667 (-3.5776362752979547)
  , directLogNormalParam "mortality_nsw_100_plus_female" 0.03455833333333334 (-3.3651065615178015)
  , directLogNormalParam "mortality_vic_00_04_male" 7.5e-05 (-9.498022444427964)
  , directLogNormalParam "mortality_vic_00_04_female" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_vic_05_09_male" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_vic_05_09_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_vic_10_14_male" 8.333333333333334e-06 (-11.695247021764184)
  , directZeroMortalityParam "mortality_vic_10_14_female"
  , directLogNormalParam "mortality_vic_15_19_male" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_vic_15_19_female" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_vic_20_24_male" 5e-05 (-9.903487552536127)
  , directLogNormalParam "mortality_vic_20_24_female" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_vic_25_29_male" 5e-05 (-9.903487552536127)
  , directLogNormalParam "mortality_vic_25_29_female" 2.5e-05 (-10.596634733096073)
  , directLogNormalParam "mortality_vic_30_34_male" 6.666666666666667e-05 (-9.615805480084347)
  , directLogNormalParam "mortality_vic_30_34_female" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_vic_35_39_male" 8.333333333333333e-05 (-9.392661928770137)
  , directLogNormalParam "mortality_vic_35_39_female" 5e-05 (-9.903487552536127)
  , directLogNormalParam "mortality_vic_40_44_male" 0.00011666666666666667 (-9.056189692148925)
  , directLogNormalParam "mortality_vic_40_44_female" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_vic_45_49_male" 0.00018333333333333334 (-8.604204568405867)
  , directLogNormalParam "mortality_vic_45_49_female" 0.00010833333333333333 (-9.130297664302647)
  , directLogNormalParam "mortality_vic_50_54_male" 0.00025833333333333334 (-8.261259817279036)
  , directLogNormalParam "mortality_vic_50_54_female" 0.00015833333333333332 (-8.750808042597743)
  , directLogNormalParam "mortality_vic_55_59_male" 0.0004166666666666667 (-7.783224016336037)
  , directLogNormalParam "mortality_vic_55_59_female" 0.00025 (-8.294049640102028)
  , directLogNormalParam "mortality_vic_60_64_male" 0.0006333333333333333 (-7.3645136814778525)
  , directLogNormalParam "mortality_vic_60_64_female" 0.00038333333333333334 (-7.866605625275088)
  , directLogNormalParam "mortality_vic_65_69_male" 0.0009833333333333332 (-6.924562397298518)
  , directLogNormalParam "mortality_vic_65_69_female" 0.0005583333333333333 (-7.490554402373217)
  , directLogNormalParam "mortality_vic_70_74_male" 0.0017833333333333334 (-6.3292710067423315)
  , directLogNormalParam "mortality_vic_70_74_female" 0.001075 (-6.835434617402511)
  , directLogNormalParam "mortality_vic_75_79_male" 0.0030583333333333335 (-5.789885173709613)
  , directLogNormalParam "mortality_vic_75_79_female" 0.0019083333333333333 (-6.261525018209944)
  , directLogNormalParam "mortality_vic_80_84_male" 0.005658333333333333 (-5.174625894205487)
  , directLogNormalParam "mortality_vic_80_84_female" 0.0037416666666666666 (-5.588224134021929)
  , directLogNormalParam "mortality_vic_85_89_male" 0.010075 (-4.59769817114939)
  , directLogNormalParam "mortality_vic_85_89_female" 0.007691666666666667 (-4.867617787261331)
  , directLogNormalParam "mortality_vic_90_94_male" 0.01725 (-4.059943135504768)
  , directLogNormalParam "mortality_vic_90_94_female" 0.014475 (-4.235332255523078)
  , directLogNormalParam "mortality_vic_95_99_male" 0.028858333333333333 (-3.545356477361761)
  , directLogNormalParam "mortality_vic_95_99_female" 0.024741666666666665 (-3.6992665470004233)
  , directLogNormalParam "mortality_vic_100_plus_male" 0.04045833333333333 (-3.207482641038758)
  , directLogNormalParam "mortality_vic_100_plus_female" 0.03598333333333333 (-3.3246994106894303)
  , directLogNormalParam "mortality_qld_00_04_male" 0.000125 (-8.987196820661973)
  , directLogNormalParam "mortality_qld_00_04_female" 9.166666666666667e-05 (-9.297351748965813)
  , directLogNormalParam "mortality_qld_05_09_male" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_qld_05_09_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_qld_10_14_male" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_qld_10_14_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_qld_15_19_male" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_qld_15_19_female" 2.5e-05 (-10.596634733096073)
  , directLogNormalParam "mortality_qld_20_24_male" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_qld_20_24_female" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_qld_25_29_male" 6.666666666666667e-05 (-9.615805480084347)
  , directLogNormalParam "mortality_qld_25_29_female" 2.5e-05 (-10.596634733096073)
  , directLogNormalParam "mortality_qld_30_34_male" 0.00010833333333333333 (-9.130297664302647)
  , directLogNormalParam "mortality_qld_30_34_female" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_qld_35_39_male" 0.00011666666666666667 (-9.056189692148925)
  , directLogNormalParam "mortality_qld_35_39_female" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_qld_40_44_male" 0.00014166666666666668 (-8.862033677707966)
  , directLogNormalParam "mortality_qld_40_44_female" 9.166666666666667e-05 (-9.297351748965813)
  , directLogNormalParam "mortality_qld_45_49_male" 0.00021666666666666666 (-8.437150483742702)
  , directLogNormalParam "mortality_qld_45_49_female" 0.000125 (-8.987196820661973)
  , directLogNormalParam "mortality_qld_50_54_male" 0.0003 (-8.111728083308073)
  , directLogNormalParam "mortality_qld_50_54_female" 0.00018333333333333334 (-8.604204568405867)
  , directLogNormalParam "mortality_qld_55_59_male" 0.00044166666666666665 (-7.724955108212061)
  , directLogNormalParam "mortality_qld_55_59_female" 0.0002666666666666667 (-8.229511118964457)
  , directLogNormalParam "mortality_qld_60_64_male" 0.00065 (-7.338538195074591)
  , directLogNormalParam "mortality_qld_60_64_female" 0.0004166666666666667 (-7.783224016336037)
  , directLogNormalParam "mortality_qld_65_69_male" 0.0011833333333333333 (-6.739419964162923)
  , directLogNormalParam "mortality_qld_65_69_female" 0.0006583333333333334 (-7.325799169297161)
  , directLogNormalParam "mortality_qld_70_74_male" 0.0018666666666666666 (-6.283600969909143)
  , directLogNormalParam "mortality_qld_70_74_female" 0.0012166666666666667 (-6.711640400055846)
  , directLogNormalParam "mortality_qld_75_79_male" 0.0032333333333333333 (-5.73424168214091)
  , directLogNormalParam "mortality_qld_75_79_female" 0.0019333333333333333 (-6.248509650097873)
  , directLogNormalParam "mortality_qld_80_84_male" 0.0059 (-5.132802928070463)
  , directLogNormalParam "mortality_qld_80_84_female" 0.003941666666666666 (-5.53615163327225)
  , directLogNormalParam "mortality_qld_85_89_male" 0.010291666666666666 (-4.576420772702106)
  , directLogNormalParam "mortality_qld_85_89_female" 0.007875 (-4.84406209427044)
  , directLogNormalParam "mortality_qld_90_94_male" 0.017791666666666667 (-4.029025096101458)
  , directLogNormalParam "mortality_qld_90_94_female" 0.014516666666666667 (-4.232457864351735)
  , directLogNormalParam "mortality_qld_95_99_male" 0.028558333333333335 (-3.555806499889574)
  , directLogNormalParam "mortality_qld_95_99_female" 0.02245 (-3.7964646647938736)
  , directLogNormalParam "mortality_qld_100_plus_male" 0.039775 (-3.2245167047582863)
  , directLogNormalParam "mortality_qld_100_plus_female" 0.0359 (-3.3270179834879037)
  , directLogNormalParam "mortality_sa_00_04_male" 9.166666666666667e-05 (-9.297351748965813)
  , directLogNormalParam "mortality_sa_00_04_female" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_sa_05_09_male" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_sa_05_09_female" 8.333333333333334e-06 (-11.695247021764184)
  , directZeroMortalityParam "mortality_sa_10_14_male"
  , directLogNormalParam "mortality_sa_10_14_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_sa_15_19_male" 5e-05 (-9.903487552536127)
  , directLogNormalParam "mortality_sa_15_19_female" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_sa_20_24_male" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_sa_20_24_female" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_sa_25_29_male" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_sa_25_29_female" 2.5e-05 (-10.596634733096073)
  , directLogNormalParam "mortality_sa_30_34_male" 6.666666666666667e-05 (-9.615805480084347)
  , directLogNormalParam "mortality_sa_30_34_female" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_sa_35_39_male" 0.000125 (-8.987196820661973)
  , directLogNormalParam "mortality_sa_35_39_female" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_sa_40_44_male" 0.00014166666666666668 (-8.862033677707966)
  , directLogNormalParam "mortality_sa_40_44_female" 0.00010833333333333333 (-9.130297664302647)
  , directLogNormalParam "mortality_sa_45_49_male" 0.00023333333333333333 (-8.36304251158898)
  , directLogNormalParam "mortality_sa_45_49_female" 0.00015833333333333332 (-8.750808042597743)
  , directLogNormalParam "mortality_sa_50_54_male" 0.0003 (-8.111728083308073)
  , directLogNormalParam "mortality_sa_50_54_female" 0.00020833333333333335 (-8.476371196895983)
  , directLogNormalParam "mortality_sa_55_59_male" 0.000475 (-7.652195753929633)
  , directLogNormalParam "mortality_sa_55_59_female" 0.0002666666666666667 (-8.229511118964457)
  , directLogNormalParam "mortality_sa_60_64_male" 0.0006916666666666667 (-7.276406413967585)
  , directLogNormalParam "mortality_sa_60_64_female" 0.00040833333333333336 (-7.803426723653557)
  , directLogNormalParam "mortality_sa_65_69_male" 0.00115 (-6.767993336606978)
  , directLogNormalParam "mortality_sa_65_69_female" 0.0006 (-7.418580902748127)
  , directLogNormalParam "mortality_sa_70_74_male" 0.001825 (-6.306175291947683)
  , directLogNormalParam "mortality_sa_70_74_female" 0.0011666666666666668 (-6.753604599154879)
  , directLogNormalParam "mortality_sa_75_79_male" 0.003075 (-5.784450377723656)
  , directLogNormalParam "mortality_sa_75_79_female" 0.002025 (-6.202185578423634)
  , directLogNormalParam "mortality_sa_80_84_male" 0.005816666666666667 (-5.14702791900181)
  , directLogNormalParam "mortality_sa_80_84_female" 0.004066666666666666 (-5.504931615911036)
  , directLogNormalParam "mortality_sa_85_89_male" 0.010616666666666667 (-4.545330185632074)
  , directLogNormalParam "mortality_sa_85_89_female" 0.0076 (-4.879607031689852)
  , directLogNormalParam "mortality_sa_90_94_male" 0.018508333333333335 (-3.9895341978697556)
  , directLogNormalParam "mortality_sa_90_94_female" 0.014841666666666666 (-4.210316738474521)
  , directLogNormalParam "mortality_sa_95_99_male" 0.025875 (-3.6544780273966038)
  , directLogNormalParam "mortality_sa_95_99_female" 0.0239 (-3.733876820044672)
  , directLogNormalParam "mortality_sa_100_plus_male" 0.018866666666666667 (-3.9703585824411096)
  , directLogNormalParam "mortality_sa_100_plus_female" 0.04373333333333333 (-3.1296446911402582)
  , directLogNormalParam "mortality_wa_00_04_male" 0.0001 (-9.210340371976184)
  , directLogNormalParam "mortality_wa_00_04_female" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_wa_05_09_male" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_wa_05_09_female" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_wa_10_14_male" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_wa_10_14_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_wa_15_19_male" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_wa_15_19_female" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_wa_20_24_male" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_wa_20_24_female" 2.5e-05 (-10.596634733096073)
  , directLogNormalParam "mortality_wa_25_29_male" 7.5e-05 (-9.498022444427964)
  , directLogNormalParam "mortality_wa_25_29_female" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_wa_30_34_male" 0.0001 (-9.210340371976184)
  , directLogNormalParam "mortality_wa_30_34_female" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_wa_35_39_male" 0.00010833333333333333 (-9.130297664302647)
  , directLogNormalParam "mortality_wa_35_39_female" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_wa_40_44_male" 0.00015 (-8.804875263868018)
  , directLogNormalParam "mortality_wa_40_44_female" 6.666666666666667e-05 (-9.615805480084347)
  , directLogNormalParam "mortality_wa_45_49_male" 0.00018333333333333334 (-8.604204568405867)
  , directLogNormalParam "mortality_wa_45_49_female" 0.00011666666666666667 (-9.056189692148925)
  , directLogNormalParam "mortality_wa_50_54_male" 0.00028333333333333335 (-8.168886497148021)
  , directLogNormalParam "mortality_wa_50_54_female" 0.00016666666666666666 (-8.699514748210191)
  , directLogNormalParam "mortality_wa_55_59_male" 0.00040833333333333336 (-7.803426723653557)
  , directLogNormalParam "mortality_wa_55_59_female" 0.0002666666666666667 (-8.229511118964457)
  , directLogNormalParam "mortality_wa_60_64_male" 0.0006 (-7.418580902748127)
  , directLogNormalParam "mortality_wa_60_64_female" 0.00035 (-7.957577403480815)
  , directLogNormalParam "mortality_wa_65_69_male" 0.0010583333333333334 (-6.851059935305591)
  , directLogNormalParam "mortality_wa_65_69_female" 0.0005666666666666667 (-7.4757393165880766)
  , directLogNormalParam "mortality_wa_70_74_male" 0.0017833333333333334 (-6.3292710067423315)
  , directLogNormalParam "mortality_wa_70_74_female" 0.0010833333333333333 (-6.8277125713086)
  , directLogNormalParam "mortality_wa_75_79_male" 0.003075 (-5.784450377723656)
  , directLogNormalParam "mortality_wa_75_79_female" 0.00165 (-6.406979991069647)
  , directLogNormalParam "mortality_wa_80_84_male" 0.005683333333333334 (-5.170217363920721)
  , directLogNormalParam "mortality_wa_80_84_female" 0.003491666666666667 (-5.657376101842045)
  , directLogNormalParam "mortality_wa_85_89_male" 0.010241666666666666 (-4.581290912198148)
  , directLogNormalParam "mortality_wa_85_89_female" 0.007083333333333333 (-4.950010672279821)
  , directLogNormalParam "mortality_wa_90_94_male" 0.017358333333333333 (-4.053682580503211)
  , directLogNormalParam "mortality_wa_90_94_female" 0.013083333333333334 (-4.336416123421829)
  , directLogNormalParam "mortality_wa_95_99_male" 0.024391666666666666 (-3.713513735072297)
  , directLogNormalParam "mortality_wa_95_99_female" 0.025058333333333332 (-3.686548838775654)
  , directLogNormalParam "mortality_wa_100_plus_male" 0.04901666666666667 (-3.0155949026502427)
  , directLogNormalParam "mortality_wa_100_plus_female" 0.034725 (-3.3602953903417294)
  , directLogNormalParam "mortality_tas_00_04_male" 0.0001 (-9.210340371976184)
  , directLogNormalParam "mortality_tas_00_04_female" 6.666666666666667e-05 (-9.615805480084347)
  , directLogNormalParam "mortality_tas_05_09_male" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_tas_05_09_female" 2.5e-05 (-10.596634733096073)
  , directLogNormalParam "mortality_tas_10_14_male" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_tas_10_14_female" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_tas_15_19_male" 6.666666666666667e-05 (-9.615805480084347)
  , directLogNormalParam "mortality_tas_15_19_female" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_tas_20_24_male" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_tas_20_24_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_tas_25_29_male" 7.5e-05 (-9.498022444427964)
  , directLogNormalParam "mortality_tas_25_29_female" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_tas_30_34_male" 7.5e-05 (-9.498022444427964)
  , directLogNormalParam "mortality_tas_30_34_female" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_tas_35_39_male" 0.00016666666666666666 (-8.699514748210191)
  , directLogNormalParam "mortality_tas_35_39_female" 5e-05 (-9.903487552536127)
  , directLogNormalParam "mortality_tas_40_44_male" 0.00015833333333333332 (-8.750808042597743)
  , directLogNormalParam "mortality_tas_40_44_female" 8.333333333333333e-05 (-9.392661928770137)
  , directLogNormalParam "mortality_tas_45_49_male" 0.0002666666666666667 (-8.229511118964457)
  , directLogNormalParam "mortality_tas_45_49_female" 0.000125 (-8.987196820661973)
  , directLogNormalParam "mortality_tas_50_54_male" 0.0003 (-8.111728083308073)
  , directLogNormalParam "mortality_tas_50_54_female" 0.00025 (-8.294049640102028)
  , directLogNormalParam "mortality_tas_55_59_male" 0.00045 (-7.706262975199909)
  , directLogNormalParam "mortality_tas_55_59_female" 0.00035 (-7.957577403480815)
  , directLogNormalParam "mortality_tas_60_64_male" 0.0007833333333333334 (-7.151952239494179)
  , directLogNormalParam "mortality_tas_60_64_female" 0.00040833333333333336 (-7.803426723653557)
  , directLogNormalParam "mortality_tas_65_69_male" 0.0012416666666666667 (-6.691300715818724)
  , directLogNormalParam "mortality_tas_65_69_female" 0.0008666666666666666 (-7.05085612262281)
  , directLogNormalParam "mortality_tas_70_74_male" 0.0019166666666666666 (-6.257167712840988)
  , directLogNormalParam "mortality_tas_70_74_female" 0.0013916666666666667 (-6.577253209347428)
  , directLogNormalParam "mortality_tas_75_79_male" 0.003525 (-5.647874842717905)
  , directLogNormalParam "mortality_tas_75_79_female" 0.0025083333333333333 (-5.988136757015307)
  , directLogNormalParam "mortality_tas_80_84_male" 0.0066 (-5.0206856299497575)
  , directLogNormalParam "mortality_tas_80_84_female" 0.0046 (-5.381698975487088)
  , directLogNormalParam "mortality_tas_85_89_male" 0.01115 (-4.496315781076009)
  , directLogNormalParam "mortality_tas_85_89_female" 0.008575 (-4.758904285930133)
  , directLogNormalParam "mortality_tas_90_94_male" 0.019 (-3.9633162998156966)
  , directLogNormalParam "mortality_tas_90_94_female" 0.016991666666666665 (-4.075032251189728)
  , directLogNormalParam "mortality_tas_95_99_male" 0.025925 (-3.652547524866546)
  , directLogNormalParam "mortality_tas_95_99_female" 0.025625 (-3.664186841523565)
  , directLogNormalParam "mortality_tas_100_plus_male" 0.024508333333333333 (-3.708742083210188)
  , directLogNormalParam "mortality_tas_100_plus_female" 0.03551666666666667 (-3.3377532091056206)
  , directLogNormalParam "mortality_nt_00_04_male" 0.000175 (-8.65072458404076)
  , directLogNormalParam "mortality_nt_00_04_female" 0.00011666666666666667 (-9.056189692148925)
  , directLogNormalParam "mortality_nt_05_09_male" 8.333333333333334e-06 (-11.695247021764184)
  , directZeroMortalityParam "mortality_nt_05_09_female"
  , directLogNormalParam "mortality_nt_10_14_male" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_nt_10_14_female" 2.5e-05 (-10.596634733096073)
  , directLogNormalParam "mortality_nt_15_19_male" 9.166666666666667e-05 (-9.297351748965813)
  , directLogNormalParam "mortality_nt_15_19_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_nt_20_24_male" 0.00021666666666666666 (-8.437150483742702)
  , directLogNormalParam "mortality_nt_20_24_female" 5e-05 (-9.903487552536127)
  , directLogNormalParam "mortality_nt_25_29_male" 0.00015833333333333332 (-8.750808042597743)
  , directLogNormalParam "mortality_nt_25_29_female" 8.333333333333333e-05 (-9.392661928770137)
  , directLogNormalParam "mortality_nt_30_34_male" 0.0002 (-8.517193191416238)
  , directLogNormalParam "mortality_nt_30_34_female" 6.666666666666667e-05 (-9.615805480084347)
  , directLogNormalParam "mortality_nt_35_39_male" 0.000225 (-8.399410155759854)
  , directLogNormalParam "mortality_nt_35_39_female" 0.00011666666666666667 (-9.056189692148925)
  , directLogNormalParam "mortality_nt_40_44_male" 0.0002916666666666667 (-8.139898960274769)
  , directLogNormalParam "mortality_nt_40_44_female" 0.00019166666666666667 (-8.559752805835034)
  , directLogNormalParam "mortality_nt_45_49_male" 0.000375 (-7.888584531993863)
  , directLogNormalParam "mortality_nt_45_49_female" 0.000225 (-8.399410155759854)
  , directLogNormalParam "mortality_nt_50_54_male" 0.000575 (-7.461140517166924)
  , directLogNormalParam "mortality_nt_50_54_female" 0.00038333333333333334 (-7.866605625275088)
  , directLogNormalParam "mortality_nt_55_59_male" 0.000725 (-7.229338903109599)
  , directLogNormalParam "mortality_nt_55_59_female" 0.000375 (-7.888584531993863)
  , directLogNormalParam "mortality_nt_60_64_male" 0.0011833333333333333 (-6.739419964162923)
  , directLogNormalParam "mortality_nt_60_64_female" 0.0006 (-7.418580902748127)
  , directLogNormalParam "mortality_nt_65_69_male" 0.0016 (-6.437751649736401)
  , directLogNormalParam "mortality_nt_65_69_female" 0.0008583333333333333 (-7.060518033534548)
  , directLogNormalParam "mortality_nt_70_74_male" 0.0027916666666666667 (-5.881116489939116)
  , directLogNormalParam "mortality_nt_70_74_female" 0.001875 (-6.2791466195597625)
  , directLogNormalParam "mortality_nt_75_79_male" 0.0036416666666666667 (-5.615313826668593)
  , directLogNormalParam "mortality_nt_75_79_female" 0.002825 (-5.869246914383733)
  , directLogNormalParam "mortality_nt_80_84_male" 0.0072 (-4.933674252960127)
  , directLogNormalParam "mortality_nt_80_84_female" 0.004125 (-5.490689259195492)
  , directLogNormalParam "mortality_nt_85_89_male" 0.013183333333333333 (-4.328801873436584)
  , directLogNormalParam "mortality_nt_85_89_female" 0.009325 (-4.675056313452258)
  , directLogNormalParam "mortality_nt_90_94_male" 0.013258333333333334 (-4.323128993426396)
  , directLogNormalParam "mortality_nt_90_94_female" 0.0148 (-4.213128098212068)
  , directLogNormalParam "mortality_nt_95_99_male" 0.020833333333333332 (-3.871201010907891)
  , directLogNormalParam "mortality_nt_95_99_female" 0.041666666666666664 (-3.1780538303479458)
  , directZeroMortalityParam "mortality_nt_100_plus_male"
  , directLogNormalParam "mortality_nt_100_plus_female" 0.036158333333333334 (-3.3198478359658323)
  , directLogNormalParam "mortality_act_00_04_male" 9.166666666666667e-05 (-9.297351748965813)
  , directLogNormalParam "mortality_act_00_04_female" 5.833333333333333e-05 (-9.74933687270887)
  , directZeroMortalityParam "mortality_act_05_09_male"
  , directZeroMortalityParam "mortality_act_05_09_female"
  , directLogNormalParam "mortality_act_10_14_male" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_act_10_14_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_act_15_19_male" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_act_15_19_female" 1.6666666666666667e-05 (-11.002099841204238)
  , directLogNormalParam "mortality_act_20_24_male" 5e-05 (-9.903487552536127)
  , directZeroMortalityParam "mortality_act_20_24_female"
  , directLogNormalParam "mortality_act_25_29_male" 4.1666666666666665e-05 (-10.085809109330082)
  , directLogNormalParam "mortality_act_25_29_female" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_act_30_34_male" 5e-05 (-9.903487552536127)
  , directLogNormalParam "mortality_act_30_34_female" 8.333333333333334e-06 (-11.695247021764184)
  , directLogNormalParam "mortality_act_35_39_male" 0.00014166666666666668 (-8.862033677707966)
  , directLogNormalParam "mortality_act_35_39_female" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_act_40_44_male" 0.00014166666666666668 (-8.862033677707966)
  , directLogNormalParam "mortality_act_40_44_female" 3.3333333333333335e-05 (-10.308952660644293)
  , directLogNormalParam "mortality_act_45_49_male" 0.000125 (-8.987196820661973)
  , directLogNormalParam "mortality_act_45_49_female" 5.833333333333333e-05 (-9.74933687270887)
  , directLogNormalParam "mortality_act_50_54_male" 0.00025833333333333334 (-8.261259817279036)
  , directLogNormalParam "mortality_act_50_54_female" 0.00013333333333333334 (-8.922658299524402)
  , directLogNormalParam "mortality_act_55_59_male" 0.00035833333333333333 (-7.934046906070621)
  , directLogNormalParam "mortality_act_55_59_female" 0.00015833333333333332 (-8.750808042597743)
  , directLogNormalParam "mortality_act_60_64_male" 0.0005 (-7.600902459542082)
  , directLogNormalParam "mortality_act_60_64_female" 0.00036666666666666667 (-7.911057387845922)
  , directLogNormalParam "mortality_act_65_69_male" 0.0009 (-7.013115794639964)
  , directLogNormalParam "mortality_act_65_69_female" 0.0006333333333333333 (-7.3645136814778525)
  , directLogNormalParam "mortality_act_70_74_male" 0.001625 (-6.422247463200436)
  , directLogNormalParam "mortality_act_70_74_female" 0.00105 (-6.858965114812705)
  , directLogNormalParam "mortality_act_75_79_male" 0.0030583333333333335 (-5.789885173709613)
  , directLogNormalParam "mortality_act_75_79_female" 0.001958333333333333 (-6.235661507620025)
  , directLogNormalParam "mortality_act_80_84_male" 0.005 (-5.298317366548036)
  , directLogNormalParam "mortality_act_80_84_female" 0.0033083333333333333 (-5.711310741076993)
  , directLogNormalParam "mortality_act_85_89_male" 0.01105 (-4.505324851018376)
  , directLogNormalParam "mortality_act_85_89_female" 0.007475 (-4.896191159705387)
  , directLogNormalParam "mortality_act_90_94_male" 0.020708333333333332 (-3.877219083233454)
  , directLogNormalParam "mortality_act_90_94_female" 0.014116666666666666 (-4.260399146552183)
  , directLogNormalParam "mortality_act_95_99_male" 0.03141666666666667 (-3.4604167413221263)
  , directLogNormalParam "mortality_act_95_99_female" 0.023683333333333334 (-3.742983713107137)
  , directLogNormalParam "mortality_act_100_plus_male" 0.08333333333333333 (-2.4849066497880004)
  , directLogNormalParam "mortality_act_100_plus_female" 0.03 (-3.506557897319982)
  , directLogNormalParam "overseas_arrival_nsw" 0.004329474279931048 (-5.442309157749786)
  , directLogNormalParam "overseas_arrival_vic" 0.003905378001428884 (-5.545400701033658)
  , directLogNormalParam "overseas_arrival_qld" 0.005054307436016764 (-5.287514441629423)
  , directLogNormalParam "overseas_arrival_sa" 0.004260119663611874 (-5.458458029045641)
  , directLogNormalParam "overseas_arrival_wa" 0.005905439962089735 (-5.131881325403855)
  , directLogNormalParam "overseas_arrival_tas" 0.0037327699347266038 (-5.590604711078398)
  , directLogNormalParam "overseas_arrival_nt" 0.004421489047180942 (-5.421278751124245)
  , directLogNormalParam "overseas_arrival_act" 0.003977552789773975 (-5.5270885257413545)
  , directLogNormalParam "emigration_nsw" 0.0010532884154231099 (-6.855838184546345)
  , directLogNormalParam "emigration_vic" 0.0009196863416369703 (-6.991477879055351)
  , directLogNormalParam "emigration_qld" 0.0009387605726916252 (-6.970950092428312)
  , directLogNormalParam "emigration_sa" 0.0005627855663066887 (-7.482611879274145)
  , directLogNormalParam "emigration_wa" 0.0010367353531120613 (-6.871678586632722)
  , directLogNormalParam "emigration_tas" 0.00040614696886621455 (-7.808795471584624)
  , directLogNormalParam "emigration_nt" 0.0013455016000951643 (-6.610988398578278)
  , directLogNormalParam "emigration_act" 0.0012761471964382132 (-6.663909743002529)
  ]


private def generatedParams : List ParamDecl :=
  [logNormalParam "interstate_base" 0.0001 (-9.210340371976184) 0.5] ++
  List.join ((areas.filter fun region => region != "nsw").map fun region =>
    [ logNormalParam ("push_" ++ region) 1.0 0.0 0.25
    , logNormalParam ("pull_" ++ region) 1.0 0.0 0.25 ]) ++
  [ logNormalParam "peak_months" 360.0 5.886104031450156 0.25
  , logNormalParam "k" 0.00001 (-11.512925464970229) 0.5 ] ++
  directParams

private def allOf : List Expr → Expr
  | [] => TransitionRaw.bool true
  | [condition] => condition
  | condition :: rest => TransitionRaw.and condition (allOf rest)

private def present : Expr := TransitionRaw.enumIs "occupancy" "present"
private def vacant : Expr := TransitionRaw.enumIs "occupancy" "vacant"
private def noEvent : Expr := TransitionRaw.enumIs "event" "none_"
private def age : Expr := TransitionRaw.selfAttribute "age_months"
private def slotClaim : ResourceClaim :=
  TransitionRaw.raceClaim (TransitionRaw.selfAttribute "slot_resource")

private def fixedSpatialFactor (stem region : String) : Expr :=
  if region == "nsw" then TransitionRaw.real 1.0
  else TransitionRaw.parameter (stem ++ region)

private def ageProfile : Expr :=
  let difference := TransitionRaw.sub age (TransitionRaw.parameter "peak_months")
  let square := TransitionRaw.mul difference difference
  TransitionRaw.div (TransitionRaw.real 1.0)
    (TransitionRaw.add (TransitionRaw.real 1.0)
      (TransitionRaw.mul (TransitionRaw.parameter "k") square))

private def moveHazard (origin destination : String) : Expr :=
  TransitionRaw.mul
    (TransitionRaw.mul
      (TransitionRaw.mul
        (TransitionRaw.parameter "interstate_base")
        (fixedSpatialFactor "push_" origin))
      (fixedSpatialFactor "pull_" destination))
    ageProfile

private def moveTransition (origin destination : String) : Transition :=
  TransitionRaw.transition
    ("move_" ++ origin ++ "_" ++ destination)
    "person_slot"
    (allOf [present, noEvent, TransitionRaw.enumIs "area" origin])
    (moveHazard origin destination)
    [ TransitionRaw.setAttribute "prev_area" (TransitionRaw.enum origin)
    , TransitionRaw.setAttribute "area" (TransitionRaw.enum destination)
    , TransitionRaw.setAttribute "event" (TransitionRaw.enum "interstate_move")
    , TransitionRaw.setAttribute "event_age_months" age
    ]
    [slotClaim]

private def moveTransitions : List Transition :=
  List.join (areas.map fun origin =>
    (areas.filter fun destination => destination != origin).map fun destination =>
      moveTransition origin destination)

private def ageGuard (ageBand : MortalityBand) : Expr :=
  let minimumAge := TransitionRaw.ge age (TransitionRaw.int ageBand.lowerBound)
  match ageBand.upperBound with
  | none => minimumAge
  | some maximumAge => TransitionRaw.and minimumAge
      (TransitionRaw.lt age (TransitionRaw.int maximumAge))

private def deathTransition (region : String) (ageBand : MortalityBand)
    (sexValue : String) : Transition :=
  TransitionRaw.transition
    ("die_" ++ region ++ "_" ++ ageBand.name ++ "_" ++ sexValue)
    "person_slot"
    (allOf [present, noEvent, TransitionRaw.enumIs "area" region,
      TransitionRaw.enumIs "sex" sexValue, ageGuard ageBand])
    (TransitionRaw.parameter
      ("mortality_" ++ region ++ "_" ++ ageBand.name ++ "_" ++ sexValue))
    [ TransitionRaw.setAttribute "event_age_months" age
    , TransitionRaw.setAttribute "entry_stream" (TransitionRaw.enum "retired_slot")
    , TransitionRaw.setAttribute "occupancy" (TransitionRaw.enum "vacant")
    , TransitionRaw.setAttribute "event" (TransitionRaw.enum "death")
    ]
    [slotClaim]

private def deathTransitions : List Transition :=
  List.join (areas.map fun region =>
    List.join (mortalityBands.map fun ageBand =>
      sexes.map (deathTransition region ageBand)))

private def birthTransition (region : String) : Transition :=
  TransitionRaw.transition
    ("birth_" ++ region)
    "person_slot"
    (allOf [vacant, noEvent, TransitionRaw.enumIs "entry_stream" "birth_slot",
      TransitionRaw.enumIs "area" region])
    (TransitionRaw.parameter ("birth_rate_" ++ region))
    [ TransitionRaw.setAttribute "occupancy" (TransitionRaw.enum "present")
    , TransitionRaw.setAttribute "event" (TransitionRaw.enum "birth")
    , TransitionRaw.setAttribute "age_months" (TransitionRaw.int 0)
    , TransitionRaw.setAttribute "event_age_months" (TransitionRaw.int 0)
    , TransitionRaw.setAttribute "generation"
        (TransitionRaw.add (TransitionRaw.selfAttribute "generation")
          (TransitionRaw.int 1))
    ]
    []

private def arrivalTransition (region : String) : Transition :=
  let entryAge := TransitionRaw.selfAttribute "entry_age_months"
  TransitionRaw.transition
    ("overseas_arrive_" ++ region)
    "person_slot"
    (allOf [vacant, noEvent,
      TransitionRaw.enumIs "entry_stream" "overseas_slot",
      TransitionRaw.enumIs "area" region])
    (TransitionRaw.parameter ("overseas_arrival_" ++ region))
    [ TransitionRaw.setAttribute "occupancy" (TransitionRaw.enum "present")
    , TransitionRaw.setAttribute "event" (TransitionRaw.enum "overseas_arrival")
    , TransitionRaw.setAttribute "age_months" entryAge
    , TransitionRaw.setAttribute "event_age_months" entryAge
    , TransitionRaw.setAttribute "generation"
        (TransitionRaw.add (TransitionRaw.selfAttribute "generation")
          (TransitionRaw.int 1))
    ]
    []

private def emigrationTransition (region : String) : Transition :=
  TransitionRaw.transition
    ("emigrate_" ++ region)
    "person_slot"
    (allOf [present, noEvent, TransitionRaw.enumIs "area" region])
    (TransitionRaw.parameter ("emigration_" ++ region))
    [ TransitionRaw.setAttribute "event_age_months" age
    , TransitionRaw.setAttribute "entry_stream" (TransitionRaw.enum "retired_slot")
    , TransitionRaw.setAttribute "occupancy" (TransitionRaw.enum "vacant")
    , TransitionRaw.setAttribute "event" (TransitionRaw.enum "overseas_departure")
    ]
    [slotClaim]

private def ageMonthly : Transition :=
  TransitionRaw.transition "age_monthly" "person_slot" present
    (TransitionRaw.real 1e300)
    [TransitionRaw.setAttribute "age_months"
      (TransitionRaw.add age (TransitionRaw.int 1))]
    []

private def clearEvent : Transition :=
  TransitionRaw.transition "clear_event" "person_slot"
    (TransitionRaw.not noEvent)
    (TransitionRaw.real 1e300)
    [ TransitionRaw.setAttribute "event" (TransitionRaw.enum "none_")
    , TransitionRaw.setAttribute "prev_area" (TransitionRaw.enum "none_")
    ]
    []

private def generatedTransitions : List Transition :=
  [ageMonthly, clearEvent] ++ moveTransitions ++ deathTransitions ++
  areas.map birthTransition ++ areas.map arrivalTransition ++
  areas.map emigrationTransition

private def attachGeneratedTransitions (modelBox : Box) : Box :=
  if modelBox.name == "demographic" then
    { modelBox with «transitions» := modelBox.transitions ++ generatedTransitions }
  else modelBox

/-- Canonical one-in-a-hundred model used by the initial-state fixture. -/
def australianPopulation : Model :=
  { australianPopulationSchema with
    «params» := generatedParams
    «boxes» := australianPopulationSchema.boxes.map attachGeneratedTransitions }

private def canonicalRowCountsHold : Bool :=
  match australianPopulation.boxes with
  | [modelBox] => modelBox.tables.map (fun modelTable => modelTable.sizeHint) ==
      [canonicalRows, canonicalRows]
  | _ => false

#guard australianPopulation.name == "australian_population"
#guard australianPopulation.boxes.length == 1
#guard canonicalRowCountsHold
#guard generatedParams.length == 377
#guard moveTransitions.length == 56
#guard deathTransitions.length == 336
#guard generatedTransitions.length == 418
#guard (Sembla.Semantics.checkModel australianPopulation).isOk

/-- Canonical legacy-model bytes consumed by the Python state builder. -/
def australianPopulationJson : String := toJson australianPopulation

/-- Direct-stable plan bytes for parity and downstream run fixtures. -/
def australianPopulationPlanJson : String :=
  match PlanExport.directStablePlan australianPopulation with
  | .error message => message
  | .ok plan => PlanJson.planToCJson plan |>.render

end Sembla.Models
