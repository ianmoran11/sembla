import Sembla.Json
import Sembla.DSL

namespace Sembla.ReactionArrowTests
open Sembla.IR Sembla.DSL Sembla.Widgets

private def twinsMatch (arrow expanded : Model) (boxName tableName : String) : Bool :=
  arrow == expanded &&
    toJson arrow == toJson expanded &&
    stateDiagramProps? arrow boxName tableName ==
      stateDiagramProps? expanded boxName tableName

private def panelsMatch (arrow expanded : Model) (boxName transitionName : String) : Bool :=
  hazardPanelProps? arrow boxName transitionName ==
    hazardPanelProps? expanded boxName transitionName

/-- The four frozen spellings all lower to the same existing transition shape
    and preserve textual transition/effect order. -/
private def fourFormsArrow : Model := model% "reaction_four_forms" step(1.0) where
  params []
  boxes [box reactions where
    systems [system Person (rows := 4) where [state health : {S, I}]]
    inputs []
    transitions [
      inferred : S →[0.5] I,
      explicitSystem on Person : I →[0.5] S,
      explicitAttribute : health: S →[0.5] I,
      explicitBoth on Person : health: I →[0.5] S]
    outputs []]
  wires []

private def fourFormsExpanded : Model := model% "reaction_four_forms" step(1.0) where
  params []
  boxes [box reactions where
    systems [system Person (rows := 4) where [state health : {S, I}]]
    inputs []
    transitions [
      transition inferred on Person where
        guard health = S hazard 0.5 set [health := I],
      transition explicitSystem on Person where
        guard health = I hazard 0.5 set [health := S],
      transition explicitAttribute on Person where
        guard health = S hazard 0.5 set [health := I],
      transition explicitBoth on Person where
        guard health = I hazard 0.5 set [health := S]]
    outputs []]
  wires []

#guard twinsMatch fourFormsArrow fourFormsExpanded "reactions" "person"
#guard panelsMatch fourFormsArrow fourFormsExpanded "reactions" "inferred"
#guard panelsMatch fourFormsArrow fourFormsExpanded "reactions" "explicitSystem"
#guard panelsMatch fourFormsArrow fourFormsExpanded "reactions" "explicitAttribute"
#guard panelsMatch fourFormsArrow fourFormsExpanded "reactions" "explicitBoth"

private def exactFourTransitions : List Transition := [
  Transition.mk "inferred" "person"
    (Expr.enumIs "health" "S") (Expr.real 0.5)
    [Effect.setAttr "health" (Expr.enum "I")] [],
  Transition.mk "explicitSystem" "person"
    (Expr.enumIs "health" "I") (Expr.real 0.5)
    [Effect.setAttr "health" (Expr.enum "S")] [],
  Transition.mk "explicitAttribute" "person"
    (Expr.enumIs "health" "S") (Expr.real 0.5)
    [Effect.setAttr "health" (Expr.enum "I")] [],
  Transition.mk "explicitBoth" "person"
    (Expr.enumIs "health" "I") (Expr.real 0.5)
    [Effect.setAttr "health" (Expr.enum "S")] []]

private def fourTransitions? : Option (List Transition) :=
  fourFormsArrow.boxes.head? |>.map (·.transitions)

#guard fourTransitions? == some exactFourTransitions

private def selfLoopArrow : Model := model% "reaction_self_loop" step(1.0) where
  params []
  boxes [box reactions where
    systems [system Person (rows := 4) where [state health : {S, I}]]
    inputs []
    transitions [stay : S →[0.5] S]
    outputs []]
  wires []

private def selfLoopExpanded : Model := model% "reaction_self_loop" step(1.0) where
  params []
  boxes [box reactions where
    systems [system Person (rows := 4) where [state health : {S, I}]]
    inputs []
    transitions [transition stay on Person where
      guard health = S hazard 0.5 set [health := S]]
    outputs []]
  wires []

#guard twinsMatch selfLoopArrow selfLoopExpanded "reactions" "person"
#guard panelsMatch selfLoopArrow selfLoopExpanded "reactions" "stay"

private def greekHazardArrow : Model := model% "reaction_greek_hazard" step(1.0) where
  params [param β : ℝ := 0.5]
  boxes [box reactions where
    systems [system Person (rows := 4) where [state health : {S, I}]]
    inputs []
    transitions [infect : S →[β] I]
    outputs []]
  wires []

private def greekHazardExpanded : Model := model% "reaction_greek_hazard" step(1.0) where
  params [param beta : Real := 0.5]
  boxes [box reactions where
    systems [system Person (rows := 4) where [state health : {S, I}]]
    inputs []
    transitions [transition infect on Person where
      guard health = S hazard parameter beta set [health := I]]
    outputs []]
  wires []

#guard twinsMatch greekHazardArrow greekHazardExpanded "reactions" "person"
#guard panelsMatch greekHazardArrow greekHazardExpanded "reactions" "infect"

private def aggregateHazardArrow : Model := model% "reaction_aggregate_hazard" step(1.0) where
  params []
  boxes [box reactions where
    systems [
      system Person (rows := 4) where [state health : {S, I}, ref employer : Employer],
      system Employer (rows := 2) where []]
    inputs []
    transitions [infect on Person : S →[countBy employer (health = I) / sizeBy employer] I]
    outputs []]
  wires []

private def aggregateHazardExpanded : Model := model% "reaction_aggregate_hazard" step(1.0) where
  params []
  boxes [box reactions where
    systems [
      system Person (rows := 4) where [state health : {S, I}, ref employer : Employer],
      system Employer (rows := 2) where []]
    inputs []
    transitions [transition infect on Person where
      guard health = S
      hazard countBy employer (health = I) / sizeBy employer
      set [health := I]]
    outputs []]
  wires []

#guard twinsMatch aggregateHazardArrow aggregateHazardExpanded "reactions" "person"
#guard panelsMatch aggregateHazardArrow aggregateHazardExpanded "reactions" "infect"

/-- Candidate inference examines every collected system: the first system has
    the endpoints split across different state columns, while the sole compatible
    system is deliberately declared second. -/
private def collectedCandidateArrow : Model := model% "reaction_collected_candidate" step(1.0) where
  params []
  boxes [box reactions where
    systems [
      system Split (rows := 1) where [
        state first : {S, X},
        state second : {Y, I}],
      system Person (rows := 4) where [state health : {S, I}]]
    inputs []
    transitions [
      infect : S →[0.5] I,
      labeled : health: S →[0.5] I]
    outputs []]
  wires []

private def collectedCandidateExpanded : Model := model% "reaction_collected_candidate" step(1.0) where
  params []
  boxes [box reactions where
    systems [
      system Split (rows := 1) where [
        state first : {S, X},
        state second : {Y, I}],
      system Person (rows := 4) where [state health : {S, I}]]
    inputs []
    transitions [
      transition infect on Person where
        guard health = S hazard 0.5 set [health := I],
      transition labeled on Person where
        guard health = S hazard 0.5 set [health := I]]
    outputs []]
  wires []

#guard twinsMatch collectedCandidateArrow collectedCandidateExpanded "reactions" "person"
#guard panelsMatch collectedCandidateArrow collectedCandidateExpanded "reactions" "infect"
#guard panelsMatch collectedCandidateArrow collectedCandidateExpanded "reactions" "labeled"

/-- Explicit system and state labels jointly resolve both inference ambiguities. -/
private def disambiguatedArrow : Model := model% "reaction_disambiguated" step(1.0) where
  params []
  boxes [box reactions where
    systems [
      system Animal (rows := 2) where [state health : {S, I}],
      system Person (rows := 4) where [
        state health : {S, I},
        state mode : {Open, Closed}]]
    inputs []
    transitions [infect on Person : health: S →[0.5] I]
    outputs []]
  wires []

private def disambiguatedExpanded : Model := model% "reaction_disambiguated" step(1.0) where
  params []
  boxes [box reactions where
    systems [
      system Animal (rows := 2) where [state health : {S, I}],
      system Person (rows := 4) where [
        state health : {S, I},
        state mode : {Open, Closed}]]
    inputs []
    transitions [transition infect on Person where
      guard health = S hazard 0.5 set [health := I]]
    outputs []]
  wires []

#guard twinsMatch disambiguatedArrow disambiguatedExpanded "reactions" "person"
#guard panelsMatch disambiguatedArrow disambiguatedExpanded "reactions" "infect"

end Sembla.ReactionArrowTests
