import Sembla.DSL

/-!
# Step 9: advanced mathematical-surface detour

This standalone example does not continue the workplace-SIR state. It shows how
finite mathematical declarations expand at elaboration time to Sembla's ordinary
scalar parameters and transitions, and how table-scoped views and box-scoped
summaries compactly declare observations.
-/
namespace Sembla.Tutorial.Step09

open Sembla.IR Sembla.DSL

sembla_model RegionalMovementDetour
    (name := "tutorial_09_mathematical_surface")
    (dt := 1.0) where
  domain Area := {north, south}
  domain Level := 0 .. 1

  partition AgeBand projects population.Person.age_months where
    «00_04» := 0 ..< 60
    «05_plus» := 60 ..

  param baseline : ℝ := 0.01 ~ LogNormal (-4.605170185988091) 0.25
  param movement (origin : Area, band : AgeBand) : ℝ where
    [north, «00_04»] := 0.10 ~ Normal 0.10 0.01
    [north, «05_plus»] := 0.08 ~ LogNormal (-2.5257286443082556) 0.20
    [south, «00_04»] := 0.09
    [south, «05_plus»] := 0.07

  function Identity (level : Level) : Int where
    [0] := level
    [1] := level

  box population where
    system Resource (rows := 8)
    system Person (rows := 8) where
      status : {present, vacant}
      area : Area
      level : Level
      age_months : Int
      marker : Int
      resource : Resource

    state Present on Person where
      status := present
    state Vacant on Person where
      status := vacant
    state InArea (region : Area) on Person where
      area := region
    state AtLevel (selected : Level) on Person where
      level := selected
    state Eligible on Person where
      match marker ≥ 0

    relation relocate (origin : Area, destination : Area, band : AgeBand,
        selected : Level) on Person subject to origin ≠ destination where
      from Present, InArea(origin), AtLevel(selected), Eligible, AgeBand(band)
      hazard baseline * movement(origin, band)
      claim resource by race_time
      set marker := Identity(selected)
      become InArea(destination)
      become Vacant

    views Person where
      present_count := count where status = present
      present_cells := count
        where status = present
        by area, band(age_months, 60)

  summaries population where
    final_present := last present_count

private def ruleNames : List String :=
  RegionalMovementDetour.boxes.bind fun modelBox => modelBox.transitions.map (·.name)

#guard RegionalMovementDetour.params.map (·.name) ==
  ["baseline", "movement_north_00_04", "movement_north_05_plus",
   "movement_south_00_04", "movement_south_05_plus"]
#guard ruleNames ==
  ["relocate_north_south_00_04_0", "relocate_north_south_00_04_1",
   "relocate_north_south_05_plus_0", "relocate_north_south_05_plus_1",
   "relocate_south_north_00_04_0", "relocate_south_north_00_04_1",
   "relocate_south_north_05_plus_0", "relocate_south_north_05_plus_1"]
#guard (RegionalMovementDetour.boxes.bind (·.transitions)).head?.map (·.contests.length) == some 1
#guard (RegionalMovementDetour.boxes.bind (·.transitions)).head?.map (·.effects) == some
  [Effect.setAttr "marker" (Expr.int 0), Effect.setAttr "area" (Expr.enum "south"),
   Effect.setAttr "status" (Expr.enum "vacant")]

end Sembla.Tutorial.Step09
