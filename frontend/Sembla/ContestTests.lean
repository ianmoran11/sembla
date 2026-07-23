import Sembla.Json
import Sembla.DSL

namespace Sembla.ContestTests
open Sembla.IR Sembla.DSL

/- Command-layout contest syntax lowers through the shared surface kernel. -/
sembla_model contestTwin
    (name := "contest_twin")
    (dt := 1.0) where
  box Arena where
    system SlotResource (rows := 1)
    system Slot (rows := 1) where
      occupancy : {present, vacant}
      slot_resource : SlotResource
      backup_resource : SlotResource

    transition exit on Slot where
      guard occupancy = present
      hazard 1.0
      contest slot_resource by race_time
      contest backup_resource by race_time
      set occupancy := vacant

private def listContestTwin : Model := model% "contest_twin" step(1.0) where
  params []
  boxes [box Arena where
    systems [
      system SlotResource (rows := 1) where [],
      system Slot (rows := 1) where [
        state occupancy : {present, vacant},
        ref slot_resource : SlotResource,
        ref backup_resource : SlotResource]]
    inputs []
    transitions [transition exit on Slot where
      guard occupancy = present
      hazard 1.0
      contest slot_resource by race_time
      contest backup_resource by race_time
      set [occupancy := vacant]]
    outputs []]
  wires []

private def expectedContestTwin : Model :=
  Model.mk "contest_twin" 1.0 []
    [Box.mk "Arena"
      [ Table.mk "slot_resource" 1 []
      , Table.mk "slot" 1
          [ Attr.mk "occupancy" (.enum ["present", "vacant"])
          , Attr.mk "slot_resource" (.ref "slot_resource")
          , Attr.mk "backup_resource" (.ref "slot_resource") ] ]
      [Transition.mk "exit" "slot"
        (.enumIs "occupancy" "present")
        (.real 1.0)
        [.setAttr "occupancy" (.enum "vacant")]
        [ ResourceClaim.mk (.selfAttr "slot_resource") .raceTime
        , ResourceClaim.mk (.selfAttr "backup_resource") .raceTime ]]
      [] [] [] []]
    [] []

#guard contestTwin == expectedContestTwin
#guard listContestTwin == expectedContestTwin
#guard toJson contestTwin == toJson expectedContestTwin

/- End-to-end fixture: every row has its own Ref resource, and both exits race
   for that resource before writing the same snapshot-isolated attributes. -/
sembla_model competingExitsModel
    (name := "contest_competing_exits")
    (dt := 1.0) where
  box World where
    system SlotResource (rows := 100)
    system Slot (rows := 100) where
      occupancy : {present, vacant}
      cause : {none, a, b}
      slot_resource : SlotResource

    transition exit_a on Slot where
      guard occupancy = present
      hazard 1e300
      contest slot_resource by race_time
      set occupancy := vacant
      set cause := a

    transition exit_b on Slot where
      guard occupancy = present
      hazard 8e299
      contest slot_resource by race_time
      set occupancy := vacant
      set cause := b

    view cause_a := count Slot where cause = a
    view cause_b := count Slot where cause = b
    view present := count Slot where occupancy = present

/-- Canonical bytes used by the Rust runtime integration test. -/
def competingExitsModelJson : String := toJson competingExitsModel

end Sembla.ContestTests
