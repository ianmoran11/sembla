import Sembla.Json
import Sembla.DSL

namespace Sembla.CommandFrontendTests
open Sembla.IR Sembla.DSL Sembla.Widgets

namespace Namespaced
sembla_model EmptyCommand (dt := 1.0) where

#guard EmptyCommand.name == "empty_command" && EmptyCommand.params.isEmpty &&
  EmptyCommand.boxes.isEmpty && EmptyCommand.wires.isEmpty && EmptyCommand.summaries.isEmpty

sembla_model CommandFeatureTour
    (name := "command_feature_tour")
    (dt := 0.25) where
  summary total_I := sum population.I
  param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25

  box population where
    view I := count Person where health = I
    input rich_input where
      baseline : ℝ
      infected : Int
      mode : {Open, Restricted}
      subject : Person
    system Person (rows := 1_000) where
      health : {S, I, R}
      risk : ℝ
      visits : Int
      employer : Employer
    output infection_count from Person where
      infected : Int := count where health = I
      total_risk : ℝ := sum (risk)
    infect on Person : health: S →[β · freq (health = I) over employer] I
    input policy_signal where
      modifier : ℝ
    system Employer (name := "employer") (rows := 50)
    transition adjust on Person where
      guard risk > γ
      hazard 0.1
      set risk := 0.4
      set visits := 1
    view risk_total := sum Person using risk
    view visits_min := min Person using visits
    view active_risk_max := max Person where health = I using risk

  wire policy restriction_modifier -> population policy_signal
  param γ : ℝ := 0.1

  box policy where
    output restriction_modifier from Controller where
      modifier : ℝ := sum (modifier)
    system Controller (rows := 1) where
      mode : {Open, Restricted}
      modifier : ℝ
    input infection_count where
      infected : Int
      total_risk : ℝ
    transition restrict on Controller where
      guard mode = Open ∧ inputSum infection_count field infected > 500
      hazard 1e300
      set mode := Restricted
      set modifier := 0.4

  summary minimum_I := min population.I
  wire population infection_count -> policy infection_count
  summary peak_I := max population.I
  summary final_I := last population.I
  summary peak_tick := argmaxₜ population.I

#check CommandFeatureTour
end Namespaced

private def legacyFeatureTour : Model := model% "command_feature_tour" step(0.25) where
  params [
    param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25),
    param gamma : Real := 0.1]
  boxes [
    box population where
      systems [
        system Person as "person" rows(1000) where [
          state health : {S, I, R},
          attr risk : Real,
          attr visits : Int,
          ref employer : Employer],
        system Employer as "employer" rows(50) where []]
      inputs [
        input rich_input {
          attr baseline : Real,
          attr infected : Int,
          state mode : {Open, Restricted},
          ref subject : Person},
        input policy_signal {attr modifier : Real}]
      transitions [
        transition infect on Person where guard health = S
          hazard parameter beta * (countBy employer (health = I) / sizeBy employer)
          set [health := I],
        transition adjust on Person where guard risk > parameter gamma hazard 0.1
          set [risk := 0.4, visits := 1]]
      outputs [
        output infection_count {attr infected : Int, attr total_risk : Real} from Person fields [
          field infected := count where health = I,
          field total_risk := sum (risk)]]
      views [
        view I from Person where health = I reduce count,
        view risk_total from Person using risk reduce sum,
        view visits_min from Person using visits reduce min,
        view active_risk_max from Person where health = I using risk reduce max],
    box policy where
      systems [
        system Controller as "controller" rows(1) where [
          state mode : {Open, Restricted},
          attr modifier : Real]]
      inputs [input infection_count {attr infected : Int, attr total_risk : Real}]
      transitions [
        transition restrict on Controller where
          guard mode = Open && inputSum infection_count field infected > 500
          hazard 1e300 set [mode := Restricted, modifier := 0.4]]
      outputs [
        output restriction_modifier {attr modifier : Real} from Controller fields [
          field modifier := sum (modifier)]]
      views []]
  wires [
    wire policy restriction_modifier -> population policy_signal,
    wire population infection_count -> policy infection_count]
  summaries [
    summary total_I from population view I reduce sum,
    summary minimum_I from population view I reduce min,
    summary peak_I from population view I reduce max,
    summary final_I from population view I reduce last,
    summary peak_tick from population view I reduce argmax_tick]

#guard Namespaced.CommandFeatureTour == legacyFeatureTour
#guard Namespaced.CommandFeatureTour.name == "command_feature_tour"
#guard Namespaced.CommandFeatureTour.params.map (·.name) == ["beta", "gamma"]

private def fullInputShape : Bool :=
  match Namespaced.CommandFeatureTour.boxes with
  | population :: _ => match population.inputs with
    | rich :: _ =>
        rich.schema.map (·.name) == ["baseline", "infected", "mode", "subject"] &&
        rich.schema.map (·.ty) == [
          AttrType.real, AttrType.int, AttrType.enum ["Open", "Restricted"], AttrType.ref "person"]
    | _ => false
  | _ => false

#guard fullInputShape
#guard Sembla.IR.toJson Namespaced.CommandFeatureTour == Sembla.IR.toJson legacyFeatureTour
#guard stateDiagramProps? Namespaced.CommandFeatureTour "population" "person" ==
  stateDiagramProps? legacyFeatureTour "population" "person"
#guard hazardPanelProps? Namespaced.CommandFeatureTour "population" "infect" ==
  hazardPanelProps? legacyFeatureTour "population" "infect"
#guard hazardPanelProps? Namespaced.CommandFeatureTour "population" "adjust" ==
  hazardPanelProps? legacyFeatureTour "population" "adjust"
#guard hazardPanelProps? Namespaced.CommandFeatureTour "policy" "restrict" ==
  hazardPanelProps? legacyFeatureTour "policy" "restrict"

namespace ScopedObservationSyntax

sembla_model LongForm
    (name := "scoped_observation_syntax")
    (dt := 1.0) where
  box population where
    system Row (rows := 4) where
      status : {active, inactive}
      age : Int
      score : ℝ

    view active := count Row where status = active
    view all_rows := count Row
    view total_score := sum Row using score
    view minimum_active_score := min Row where status = active using score
    view maximum_score := max Row using score
    grouped view cells :=
      count Row by status, band age 12 where status = active
    grouped view all_cells := count Row by status, band age 12

  summary final_active := last population.active
  summary total_score_over_time := sum population.total_score
  summary minimum_active_score := min population.minimum_active_score
  summary maximum_score := max population.maximum_score
  summary maximum_score_tick := argmaxₜ population.maximum_score

sembla_model ScopedForm
    (name := "scoped_observation_syntax")
    (dt := 1.0) where
  box population where
    -- The scoped block deliberately precedes its source to pin multi-pass lookup.
    views Row where
      active := count where status = active
      cells := count
        where status = active
        by status, band(age, 12)
      all_rows := count
      total_score := sum score
      minimum_active_score := min score where status = active
      maximum_score := max score
      all_cells := count by status, band(age, 12)

    system Row (rows := 4) where
      status : {active, inactive}
      age : Int
      score : ℝ

  summaries population where
    final_active := last active
    total_score_over_time := sum total_score
    minimum_active_score := min minimum_active_score
    maximum_score := max maximum_score
    maximum_score_tick := argmaxₜ maximum_score

#guard ScopedForm == LongForm
#guard Sembla.IR.toJson ScopedForm == Sembla.IR.toJson LongForm

end ScopedObservationSyntax

-- A separate deliberately interleaved model pins stable partitioning for every IR list.
sembla_model InterleavedOrder (dt := 1.0) where
  param zeta : ℝ := 0.2
  summary first_summary := min alpha.second_view
  box alpha where
    view second_view := max Row using score
    output outbound from Row where
      second_field : ℝ := sum (score)
      first_field : Int := count where status = A
    system Target (rows := 2)
    transition second_rule on Row where
      guard status = A
      hazard 0.2
      set score := 0.3
      set visits := 2
    input second_input where
      second_field : ℝ
      first_field : Int
    system Row (rows := 3) where
      second_attr : ℝ
      status : {Z, A}
      score : ℝ
      visits : Int
      target : Target
    view first_view := count Row where status = Z
    transition first_rule on Row where
      guard status = Z
      hazard alpha
      set score := 0.4
  param alpha : ℝ := 0.1
  box beta where
    system Sink (rows := 1)
    input inbound where
      second_field : ℝ
      first_field : Int
  summary second_summary := last alpha.first_view
  wire alpha outbound -> beta inbound

private def legacyInterleavedOrder : Model := model% "interleaved_order" step(1.0) where
  params [
    param zeta : Real := 0.2,
    param alpha : Real := 0.1]
  boxes [
    box alpha where
      systems [
        system Target as "target" rows(2) where [],
        system Row as "row" rows(3) where [
          attr second_attr : Real,
          state status : {Z, A},
          attr score : Real,
          attr visits : Int,
          ref target : Target]]
      inputs [input second_input {attr second_field : Real, attr first_field : Int}]
      transitions [
        transition second_rule on Row where guard status = A hazard 0.2
          set [score := 0.3, visits := 2],
        transition first_rule on Row where guard status = Z hazard parameter alpha
          set [score := 0.4]]
      outputs [
        output outbound {attr second_field : Real, attr first_field : Int} from Row fields [
          field second_field := sum (score),
          field first_field := count where status = A]]
      views [
        view second_view from Row using score reduce max,
        view first_view from Row where status = Z reduce count],
    box beta where
      systems [system Sink as "sink" rows(1) where []]
      inputs [input inbound {attr second_field : Real, attr first_field : Int}]
      transitions []
      outputs []
      views []]
  wires [wire alpha outbound -> beta inbound]
  summaries [
    summary first_summary from alpha view second_view reduce min,
    summary second_summary from alpha view first_view reduce last]

#guard InterleavedOrder == legacyInterleavedOrder
#guard Sembla.IR.toJson InterleavedOrder == Sembla.IR.toJson legacyInterleavedOrder
#guard stateDiagramProps? InterleavedOrder "alpha" "row" ==
  stateDiagramProps? legacyInterleavedOrder "alpha" "row"
#guard hazardPanelProps? InterleavedOrder "alpha" "second_rule" ==
  hazardPanelProps? legacyInterleavedOrder "alpha" "second_rule"

sembla_model ArrowForms (dt := 1.0) where
  param rate : ℝ := 0.1
  box arrows where
    system One (rows := 1) where
      mode : {X, Y}
    inferred : X →[rate] Y
    explicit_system on One : X →[rate] Y
    explicit_attribute : mode: X →[rate] Y
    fully_explicit on One : mode: X →[rate] Y

private def names (xs : List α) (f : α → String) : List String := xs.map f

#guard ArrowForms.boxes.head?.map (fun modelBox => names modelBox.transitions (·.name)) ==
  some ["inferred", "explicit_system", "explicit_attribute", "fully_explicit"]
#guard InterleavedOrder.name == "interleaved_order"
#guard names InterleavedOrder.params (·.name) == ["zeta", "alpha"]
#guard names InterleavedOrder.boxes (·.name) == ["alpha", "beta"]
#guard names InterleavedOrder.wires (fun w => w.source.box ++ "." ++ w.source.port) == ["alpha.outbound"]
#guard names InterleavedOrder.summaries (·.name) == ["first_summary", "second_summary"]

private def nestedInterleavedOrder : Bool :=
  match InterleavedOrder.boxes with
  | alphaBox :: _ =>
      names alphaBox.tables (·.name) == ["target", "row"] &&
      names alphaBox.transitions (·.name) == ["second_rule", "first_rule"] &&
      names alphaBox.inputs (·.name) == ["second_input"] &&
      names alphaBox.outputs (·.name) == ["outbound"] &&
      names alphaBox.views (·.name) == ["second_view", "first_view"] &&
      match alphaBox.tables, alphaBox.inputs, alphaBox.outputs, alphaBox.transitions with
      | _ :: row :: _, inputDecl :: _, outDecl :: _, ruleDecl :: _ =>
          names row.attrs (·.name) == ["second_attr", "status", "score", "visits", "target"] &&
          (match row.attrs with
          | _ :: enumAttr :: _ => match enumAttr.ty with
            | .enum variants => variants == ["Z", "A"]
            | _ => false
          | _ => false) &&
          names inputDecl.schema (·.name) == ["second_field", "first_field"] &&
          names outDecl.schema (·.name) == ["second_field", "first_field"] &&
          (match outDecl.builder with
          | .perTable _ builderFields =>
              names builderFields (·.name) == ["second_field", "first_field"]) &&
          names ruleDecl.effects (fun effect => match effect with
            | .setAttr attrName _ => attrName) == ["score", "visits"]
      | _, _, _, _ => false
  | _ => false

#guard nestedInterleavedOrder

namespace ClaimedRefExactRaw
open Sembla.Frontend.Builders

sembla_model Checked (name := "claimed_ref_exact_raw") (dt := 1.0) where
  box arena where
    system Resource (name := "runtime_resource") (rows := 1)
    system Writer (name := "runtime_writer") (rows := 1) where
      parent : Resource
      backup : Resource
      visits : Int
    transition rewrite on Writer where
      guard true
      hazard 1.0
      contest parent by race_time
      contest backup by race_time
      set visits := 1
      set parent := parent

private def exactRaw : Model :=
  Model.mk "claimed_ref_exact_raw" 1.0 []
    [Box.mk "arena"
      [ Table.mk "runtime_resource" 1 []
      , Table.mk "runtime_writer" 1
          [ Attr.mk "parent" (.ref "runtime_resource")
          , Attr.mk "backup" (.ref "runtime_resource")
          , Attr.mk "visits" .int ] ]
      [TransitionRaw.transition "rewrite" "runtime_writer" (.bool true) (.real 1.0)
        [ TransitionRaw.setAttribute "visits" (.int 1)
        , TransitionRaw.setAttribute "parent" (.selfAttr "parent") ]
        [ TransitionRaw.raceClaim (.selfAttr "parent")
        , TransitionRaw.raceClaim (.selfAttr "backup") ]]
      [] [] [] []]
    [] []

#guard Checked == exactRaw
#guard Sembla.IR.toJson Checked == Sembla.IR.toJson exactRaw

end ClaimedRefExactRaw

namespace AliasTransitionPlanningParity
open Sembla.Frontend.Builders

sembla_model Checked (name := "alias_transition_planning_parity") (dt := 1.0) where
  domain Region := {north, south}
  box population where
    system Person (name := "people") (rows := 2) where
      region : Region
      status : {susceptible, infected}
      visits : Int
      score : ℝ
    state Susceptible on Person where
      status := susceptible
      match visits ≥ 0
    state InRegion (selected : Region) on Person where
      region := selected
    state Moved (selected : Region) on Person where
      region := selected
      status := infected
      visits := 2
    transition idle on Person where
      guard true
      hazard 0.01
      set visits := 0
    relation move (origin : Region, target : Region) on Person subject to origin ≠ target where
      from Susceptible, InRegion(origin)
      hazard 0.25
      set score := 1.5
      become Moved(target)

private def exactRaw : Model :=
  Model.mk "alias_transition_planning_parity" 1.0 []
    [Box.mk "population"
      [Table.mk "people" 2
        [ Attr.mk "region" (.enum ["north", "south"])
        , Attr.mk "status" (.enum ["susceptible", "infected"])
        , Attr.mk "visits" .int
        , Attr.mk "score" .real ]]
      [ TransitionRaw.transition "idle" "people" (.bool true) (.real 0.01)
          [TransitionRaw.setAttribute "visits" (.int 0)] []
      , TransitionRaw.transition "move_north_south" "people"
          (.and (.enumIs "status" "susceptible")
            (.and (.ge (.selfAttr "visits") (.int 0))
              (.enumIs "region" "north")))
          (.real 0.25)
          [ TransitionRaw.setAttribute "score" (.real 1.5)
          , TransitionRaw.setAttribute "region" (.enum "south")
          , TransitionRaw.setAttribute "status" (.enum "infected")
          , TransitionRaw.setAttribute "visits" (.int 2) ] []
      , TransitionRaw.transition "move_south_north" "people"
          (.and (.enumIs "status" "susceptible")
            (.and (.ge (.selfAttr "visits") (.int 0))
              (.enumIs "region" "south")))
          (.real 0.25)
          [ TransitionRaw.setAttribute "score" (.real 1.5)
          , TransitionRaw.setAttribute "region" (.enum "north")
          , TransitionRaw.setAttribute "status" (.enum "infected")
          , TransitionRaw.setAttribute "visits" (.int 2) ] [] ]
      [] [] [] []]
    [] []

private def transitionList := Checked.boxes.bind (·.transitions)

#guard Checked == exactRaw
#guard Sembla.IR.toJson Checked == Sembla.IR.toJson exactRaw
#guard transitionList.map (·.name) == ["idle", "move_north_south", "move_south_north"]
#guard (transitionList.get? 1).map (·.guard) == some
  (.and (.enumIs "status" "susceptible")
    (.and (.ge (.selfAttr "visits") (.int 0)) (.enumIs "region" "north")))
#guard (transitionList.get? 1).map (·.effects) == some
  [ .setAttr "score" (.real 1.5)
  , .setAttr "region" (.enum "south")
  , .setAttr "status" (.enum "infected")
  , .setAttr "visits" (.int 2) ]

end AliasTransitionPlanningParity

namespace ZeroInstanceAliasPlanningParity
open Sembla.Frontend.Builders

sembla_model Checked (name := "zero_instance_alias_planning_parity") (dt := 1.0) where
  domain OnlyDomain := {only}
  box population where
    system Person (name := "people") (rows := 1) where
      status : {present, absent}
    state Present on Person where
      status := present
    transition keep on Person where
      guard true
      hazard 0.1
      set status := present
    relation impossible (left : OnlyDomain, right : OnlyDomain) on Person
        subject to left ≠ right where
      from Present
      hazard 0.5
      become Present

private def exactRaw : Model :=
  Model.mk "zero_instance_alias_planning_parity" 1.0 []
    [Box.mk "population"
      [Table.mk "people" 1 [Attr.mk "status" (.enum ["present", "absent"])]]
      [TransitionRaw.transition "keep" "people" (.bool true) (.real 0.1)
        [TransitionRaw.setAttribute "status" (.enum "present")] []]
      [] [] [] []]
    [] []

#guard Checked == exactRaw
#guard Sembla.IR.toJson Checked == Sembla.IR.toJson exactRaw
#guard Checked.boxes.head?.map (fun modelBox => modelBox.transitions.map (·.name)) ==
  some ["keep"]

end ZeroInstanceAliasPlanningParity

namespace UsedAliasCheckerProbes

/- The used source emits a valid Ref equality followed by an invalid predicate.
The unused validator would reject the Ref assignment first, so this Bool error
proves the emitted source reached the authoritative guard checker. -/
/--
error: guard has type Int; expected Bool
-/
#guard_msgs (error) in
sembla_model UsedSourceAliasError (dt := 1.0) where
  domain One := {only}
  box population where
    system Family (rows := 1)
    system Person (rows := 1) where
      parent : Family
      status : {susceptible, infected}
    state Broken on Person where
      parent := parent
      match 1
    state Infected on Person where
      status := infected
    relation apply (_x : One) on Person where
      from Broken
      hazard 0.1
      become Infected

/- The used destination emits a Ref write without a claim. The unused validator
would reject the Ref assignment immediately, while the authoritative checker
reports claim ownership after checking the emitted effect. -/
/--
error: writes to Ref attributes require resource claims, which are not supported by this DSL
-/
#guard_msgs (error) in
sembla_model UsedDestinationAliasError (dt := 1.0) where
  domain One := {only}
  box population where
    system Family (rows := 1)
    system Person (rows := 1) where
      parent : Family
      status : {susceptible, infected}
    state Susceptible on Person where
      status := susceptible
    state Broken on Person where
      parent := parent
    relation apply (_x : One) on Person where
      from Susceptible
      hazard 0.1
      become Broken

end UsedAliasCheckerProbes

namespace LogicalGuardDiagnosticParity

/--
error: left operand of ∧ must have type Bool
-/
#guard_msgs (error) in
sembla_model BadAndLeft (dt := 1.0) where
  box b where
    system Person (rows := 1) where value : ℝ
    transition bad on Person where
      guard value ∧ true
      hazard 0.1
      set value := value

/--
error: right operand of ∧ must have type Bool
-/
#guard_msgs (error) in
sembla_model BadAndRight (dt := 1.0) where
  box b where
    system Person (rows := 1) where value : ℝ
    transition bad on Person where
      guard true ∧ value
      hazard 0.1
      set value := value

/--
error: left operand of ∧ must have type Bool
-/
#guard_msgs (error) in
sembla_model BadNestedAnd (dt := 1.0) where
  box b where
    system Person (rows := 1) where value : ℝ
    transition bad on Person where
      guard true ∧ (value ∧ true)
      hazard 0.1
      set value := value

/--
error: operand of ¬ must have type Bool
-/
#guard_msgs (error) in
sembla_model BadNot (dt := 1.0) where
  box b where
    system Person (rows := 1) where value : ℝ
    transition bad on Person where
      guard ¬value
      hazard 0.1
      set value := value

/--
error: left operand of && must have type Bool
-/
#guard_msgs (error) in
sembla_model BadAsciiAnd (dt := 1.0) where
  box b where
    system Person (rows := 1) where value : ℝ
    transition bad on Person where
      guard value && true
      hazard 0.1
      set value := value

/--
error: guard has type Real; expected Bool
-/
#guard_msgs (error) in
sembla_model BadRootGuard (dt := 1.0) where
  box b where
    system Person (rows := 1) where value : ℝ
    transition bad on Person where
      guard value
      hazard 0.1
      set value := value

end LogicalGuardDiagnosticParity

end Sembla.CommandFrontendTests
