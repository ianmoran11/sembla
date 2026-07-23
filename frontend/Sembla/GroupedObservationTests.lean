import Sembla.DSL
import Sembla.Json
import Sembla.PlanExport

namespace Sembla.GroupedObservationTests

open Sembla Sembla.IR Sembla.DSL

sembla_model groupedObservationModel
    (name := "grouped_observation") (dt := 1.0) where
  box world where
    system Area (rows := 12)
    system PersonSlot (rows := 5) where
      sex : {male, female}
      area : Area
      age_months : Int
      occupancy : {present, vacant}

    transition exit on PersonSlot where
      guard occupancy = present
      hazard 0.2
      set occupancy := vacant

    view population := count PersonSlot where occupancy = present
    grouped view population_cells :=
      count PersonSlot by sex, area, band age_months 60 where occupancy = present

private def listGroupedObservationModel : Model :=
  model% "grouped_observation" step(1.0) where
    params []
    boxes [box world where
      systems [
        system Area (rows := 12) where [],
        system PersonSlot (rows := 5) where [
          state sex : {male, female},
          ref area : Area,
          attr age_months : Int,
          state occupancy : {present, vacant}]]
      inputs []
      transitions [transition exit on PersonSlot where
        guard occupancy = present
        hazard 0.2
        set [occupancy := vacant]]
      outputs []
      views [
        view population from PersonSlot where occupancy = present reduce count,
        grouped view population_cells :=
          count PersonSlot by sex, area, band age_months 60 where occupancy = present]]
    wires []

private def expectedGroupedObservationModel : Model :=
  Model.mk "grouped_observation" 1.0 [] [
    Box.mk "world" [
      Table.mk "area" 12 [],
      Table.mk "person_slot" 5 [
        Attr.mk "sex" (.enum ["male", "female"]),
        Attr.mk "area" (.ref "area"),
        Attr.mk "age_months" .int,
        Attr.mk "occupancy" (.enum ["present", "vacant"])]] [
      Transition.mk "exit" "person_slot"
        (.enumIs "occupancy" "present") (.real 0.2)
        [.setAttr "occupancy" (.enum "vacant")] []]
      [] [] [
        ViewDecl.mk "population" "person_slot"
          (some (.enumIs "occupancy" "present")) none .count]
      [GroupedViewDecl.mk "population_cells" "person_slot"
        (some (.enumIs "occupancy" "present")) [
          GroupKey.mk "sex" none,
          GroupKey.mk "area" none,
          GroupKey.mk "age_months" (some 60)]]]
    [] []

#guard groupedObservationModel == expectedGroupedObservationModel
#guard listGroupedObservationModel == expectedGroupedObservationModel
#guard toJson groupedObservationModel == toJson expectedGroupedObservationModel

/-- Canonical legacy-model fixture consumed by Rust end-to-end tests. -/
def groupedObservationModelJson : String := toJson groupedObservationModel

/-- Direct-stable plan records the feature because its model has grouped views. -/
def groupedObservationPlanJson : String :=
  match PlanExport.directStablePlan groupedObservationModel with
  | .error message => message
  | .ok plan => PlanJson.planToCJson plan |>.render

private def groupedPlanFeatureMatches : Bool :=
  match PlanExport.directStablePlan groupedObservationModel with
  | .error _ => false
  | .ok plan => plan.identity.enabledFeatures == ["grouped-observations"]

#guard groupedPlanFeatureMatches

end Sembla.GroupedObservationTests
