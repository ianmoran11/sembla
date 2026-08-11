import Sembla.Hash
import Sembla.Json
import Sembla.Models.AustralianPopulation
import Sembla.Models.AustralianPopulation.Parameters
import Sembla.Models.AustralianPopulation.Transitions
import Sembla.Semantics.CheckModel

/-!
Direct source-integrity, ordering, and public-assembly wiring gates for the
Australian population model. These equalities verify that compatibility exports
project the declarative authority; they are not independent old/new parity
comparisons. Frozen model and plan bytes are checked separately by the exact
`cmp` gates in `scripts/check-parity.sh`.

The four `include_str` declarations make table-only edits visible even when a
previous `Surface.olean` exists; their hashes must agree with the pins authored
in `Surface.lean` before this module can elaborate successfully.
-/

namespace Sembla.Models.AustralianPopulation.Validation

open Sembla.IR

private def birthTable : String := include_str "Data/birth_rate.json"
private def mortalityTable : String := include_str "Data/mortality.json"
private def arrivalTable : String := include_str "Data/overseas_arrival.json"
private def emigrationTable : String := include_str "Data/emigration.json"

#guard Sembla.Hash.sha256HexOfString birthTable ==
  "509af5170d0dee42e58788d743838f9a76a39d00bb7de15d3353919e96b5d887"
#guard Sembla.Hash.sha256HexOfString mortalityTable ==
  "970aa4d8c630237588419b6e92883ad5c36398696ad6bf11272392cb8c996bd5"
#guard Sembla.Hash.sha256HexOfString arrivalTable ==
  "f485b037975ac52607eb5a032a2fc0366cbf25c11df017c797d364be78ae19f6"
#guard Sembla.Hash.sha256HexOfString emigrationTable ==
  "9a8712d54ff70cd0b21de618703f9fb04a0a02baa91f4a4c2e7ed14121191f1a"

private def declarativeModel : Model :=
  Sembla.Models.australianPopulationDeclarative
private def publicAssembly : Model := Sembla.Models.australianPopulation

private def withoutTransitions (modelBox : Box) : Box :=
  { modelBox with «transitions» := [] }

private def schemaProjection : Model :=
  { declarativeModel with
    «params» := []
    «boxes» := declarativeModel.boxes.map withoutTransitions }

private def allTransitions (candidate : Model) : List Transition :=
  candidate.boxes.bind (·.transitions)

private def firstDifference? [BEq α] (left right : List α) : Option Nat :=
  let rec loop (position : Nat) : List α → List α → Option Nat
    | [], [] => none
    | leftHead :: leftTail, rightHead :: rightTail =>
        if leftHead == rightHead then loop (position + 1) leftTail rightTail
        else some position
    | _, _ => some position
  loop 0 left right

private def areas : List String :=
  ["nsw", "vic", "qld", "sa", "wa", "tas", "nt", "act"]
private def bands : List String :=
  ["00_04", "05_09", "10_14", "15_19", "20_24", "25_29", "30_34",
   "35_39", "40_44", "45_49", "50_54", "55_59", "60_64", "65_69",
   "70_74", "75_79", "80_84", "85_89", "90_94", "95_99", "100_plus"]
private def sexes : List String := ["male", "female"]

private def expectedParameterNames : List String :=
  ["interstate_base", "push_vic", "pull_vic", "push_qld", "pull_qld",
   "push_sa", "pull_sa", "push_wa", "pull_wa", "push_tas", "pull_tas",
   "push_nt", "pull_nt", "push_act", "pull_act", "peak_months", "k"] ++
  areas.map ("birth_rate_" ++ ·) ++
  (areas.bind fun area => bands.bind fun band =>
    sexes.map fun sex => "mortality_" ++ area ++ "_" ++ band ++ "_" ++ sex) ++
  areas.map ("overseas_arrival_" ++ ·) ++
  areas.map ("emigration_" ++ ·)

private def expectedTransitionNames : List String :=
  ["age_monthly", "clear_event"] ++
  (areas.bind fun origin => (areas.filter (· != origin)).map fun destination =>
    "move_" ++ origin ++ "_" ++ destination) ++
  (areas.bind fun area => bands.bind fun band => sexes.map fun sex =>
    "die_" ++ area ++ "_" ++ band ++ "_" ++ sex) ++
  areas.map ("birth_" ++ ·) ++
  areas.map ("overseas_arrive_" ++ ·) ++
  areas.map ("emigrate_" ++ ·)

private def allRules := allTransitions declarativeModel
private def normalPriorCount : Nat :=
  declarativeModel.params.countP fun entry =>
    match entry.prior with
    | some value => value.family == .normal
    | none => false

#guard schemaProjection == Sembla.Models.australianPopulationSchema
-- Compatibility projections and the public export must wire to the authority.
#guard firstDifference? declarativeModel.params
  Sembla.Models.AustralianPopulation.parameters == none
#guard firstDifference? allRules (allTransitions publicAssembly) == none
#guard declarativeModel == publicAssembly
#guard Sembla.IR.toJson declarativeModel == Sembla.IR.toJson publicAssembly
#guard declarativeModel.params.map (·.name) == expectedParameterNames
#guard allRules.map (·.name) == expectedTransitionNames
#guard declarativeModel.params.length == 377
#guard allRules.length == 418
#guard (allRules.drop 2 |>.take 56).length == 56
#guard (allRules.drop 58 |>.take 336).length == 336
#guard normalPriorCount == 7
#guard !(declarativeModel.params.map (·.name)).contains "push_nsw"
#guard !(declarativeModel.params.map (·.name)).contains "pull_nsw"
#guard Sembla.Models.AustralianPopulation.transitionCountsHold
#guard (Sembla.Semantics.checkModel declarativeModel).isOk

end Sembla.Models.AustralianPopulation.Validation
