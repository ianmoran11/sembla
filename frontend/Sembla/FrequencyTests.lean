import Sembla.Json
import Sembla.DSL

namespace Sembla.FrequencyTests
open Sembla.IR Sembla.DSL Sembla.Widgets

private def twinsMatch (surface legacy : Model) (boxName tableName : String) : Bool :=
  surface == legacy &&
    Sembla.IR.toJson surface == Sembla.IR.toJson legacy &&
    stateDiagramProps? surface boxName tableName ==
      stateDiagramProps? legacy boxName tableName

private def panelsMatch (surface legacy : Model) (boxName transitionName : String) : Bool :=
  hazardPanelProps? surface boxName transitionName ==
    hazardPanelProps? legacy boxName transitionName

private def keyedFrequency (table key : String) (predicate : Expr) : Expr :=
  Expr.div
    (Expr.agg AggOp.count table key key predicate)
    (Expr.agg AggOp.count table key key (Expr.bool true))

/-- A complete SIR-shaped twin pins the primary spelling, inferred reaction use,
    declaration order, exact JSON, and the existing expanded widget spelling. -/
private def frequencySir : Model := model% "frequency_sir" step(0.5) where
  params [
    param β : ℝ := 0.8,
    param γ : ℝ := 0.1]
  boxes [box sir where
    systems [
      system Person (rows := 1000) where [
        state health : {S, I, R},
        ref employer : Employer],
      system Employer (rows := 50) where []]
    inputs []
    transitions [
      infect : S →[β · freq (health = I) over employer] I,
      recover on Person : health: I →[γ] R]
    outputs []
    views [
      view S from Person where health = S reduce count,
      view I from Person where health = I reduce count,
      view R from Person where health = R reduce count]]
  wires []

private def legacySir : Model := model% "frequency_sir" step(0.5) where
  params [
    param beta : Real := 0.8,
    param gamma : Real := 0.1]
  boxes [box sir where
    systems [
      system Person as "person" rows(1000) where [
        state health : {S, I, R},
        ref employer : Employer],
      system Employer as "employer" rows(50) where []]
    inputs []
    transitions [
      transition infect on Person where
        guard health = S
        hazard parameter beta * (countBy employer (health = I) / sizeBy employer)
        set [health := I],
      transition recover on Person where
        guard health = I hazard parameter gamma set [health := R]]
    outputs []
    views [
      view S from Person where health = S reduce count,
      view I from Person where health = I reduce count,
      view R from Person where health = R reduce count]]
  wires []

#guard twinsMatch frequencySir legacySir "sir" "person"
#guard panelsMatch frequencySir legacySir "sir" "infect"
#guard panelsMatch frequencySir legacySir "sir" "recover"

private def sirInfectHazard? : Option Expr := do
  let modelBox ← frequencySir.boxes.head?
  let rule ← modelBox.transitions.head?
  pure rule.hazard

private def expectedPersonFrequency : Expr :=
  keyedFrequency "person" "employer" (Expr.enumIs "health" "I")

#guard sirInfectHazard? == some (Expr.mul (Expr.param "beta") expectedPersonFrequency)

private def sirInfectPanel? : Option HazardPanelProps :=
  hazardPanelProps? frequencySir "sir" "infect"

#guard sirInfectPanel?.map (·.hazard) == some
  "(beta * (countBy person.employer = self.employer where health = I / countBy person.employer = self.employer where true))"

/-- Derived table and key names are used exactly in both keyed aggregate nodes. -/
private def derivedFrequency : Model := model% "frequency_derived_names" step(1.0) where
  params []
  boxes [box derived where
    systems [
      system PolicyController (rows := 4) where [
        state health : {S, I},
        ref employer_group : EmployerGroup],
      system EmployerGroup (rows := 2) where []]
    inputs []
    transitions [infect : S →[freq (health = I) over employer_group] I]
    outputs []]
  wires []

private def legacyDerivedFrequency : Model := model% "frequency_derived_names" step(1.0) where
  params []
  boxes [box derived where
    systems [
      system PolicyController as "policy_controller" rows(4) where [
        state health : {S, I},
        ref employer_group : EmployerGroup],
      system EmployerGroup as "employer_group" rows(2) where []]
    inputs []
    transitions [transition infect on PolicyController where
      guard health = S
      hazard countBy employer_group (health = I) / sizeBy employer_group
      set [health := I]]
    outputs []]
  wires []

#guard twinsMatch derivedFrequency legacyDerivedFrequency "derived" "policy_controller"
#guard panelsMatch derivedFrequency legacyDerivedFrequency "derived" "infect"

private def derivedHazard? : Option Expr := do
  let modelBox ← derivedFrequency.boxes.head?
  let rule ← modelBox.transitions.head?
  pure rule.hazard

#guard derivedHazard? == some
  (keyedFrequency "policy_controller" "employer_group" (Expr.enumIs "health" "I"))

/-- The explicit-system arrow, explicit exported table-name override, mathematical
    conjunction, and model parameter all remain inside the accepted row-local fragment. -/
private def parameterPredicateFrequency : Model := model% "frequency_parameter_predicate" step(1.0) where
  params [param β : ℝ := 0.25]
  boxes [box overridden where
    systems [
      system LegacyPerson (name := "Person") (rows := 4) where [
        state health : {S, I},
        attr risk : ℝ,
        ref employer : Employer],
      system Employer (rows := 2) where []]
    inputs []
    transitions [
      infect on LegacyPerson : health: S →[freq (health = I ∧ risk > β) over employer] I]
    outputs []]
  wires []

private def legacyParameterPredicateFrequency : Model := model% "frequency_parameter_predicate" step(1.0) where
  params [param beta : Real := 0.25]
  boxes [box overridden where
    systems [
      system LegacyPerson as "Person" rows(4) where [
        state health : {S, I},
        attr risk : Real,
        ref employer : Employer],
      system Employer as "employer" rows(2) where []]
    inputs []
    transitions [transition infect on LegacyPerson where
      guard health = S
      hazard countBy employer (health = I ∧ risk > parameter beta) / sizeBy employer
      set [health := I]]
    outputs []]
  wires []

#guard twinsMatch parameterPredicateFrequency legacyParameterPredicateFrequency
  "overridden" "Person"
#guard panelsMatch parameterPredicateFrequency legacyParameterPredicateFrequency
  "overridden" "infect"

private def parameterPredicateHazard? : Option Expr := do
  let modelBox ← parameterPredicateFrequency.boxes.head?
  let rule ← modelBox.transitions.head?
  pure rule.hazard

private def expectedParameterPredicate : Expr :=
  Expr.and (Expr.enumIs "health" "I")
    (Expr.gt (Expr.selfAttr "risk") (Expr.param "beta"))

#guard parameterPredicateHazard? == some
  (keyedFrequency "Person" "employer" expectedParameterPredicate)

end Sembla.FrequencyTests
