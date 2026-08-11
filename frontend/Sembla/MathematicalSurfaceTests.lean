import Sembla.Json
import Sembla.DSL

namespace Sembla.MathematicalSurfaceTests
open Sembla.IR Sembla.DSL

private def legacyCollectedPriorShape (declaration : SurfaceParam) :
    Option (Lean.TSyntax `term × Lean.TSyntax `term) :=
  declaration.prior

sembla_model Mathematical (name := "mathematical_surface") (dt := 1.0) where
  domain Area := {north, south}
  domain ExactAge := 0 .. 20

  partition AgeBand projects population.Person.age where
    «00_09» := 0 ..< 10
    «10_plus» := 10 ..

  param rate (area : Area) : ℝ where
    [north] := 0.1 ~ Normal 0.1 0.01
    [south] := 0.2 ~ LogNormal (-1.6094379124341003) 0.2

  param north_factor : ℝ := 1.0
  param south_factor : ℝ := 2.0

  function Factor (area : Area) : ℝ where
    [north] := north_factor
    [south] := south_factor

  box population where
    system Person (rows := 10) where
      place : Area
      destination : Area
      age : Int
      status : {present, vacant}

    state Present on Person where
      status := present

    state Vacant on Person where
      status := vacant

    state InArea (region : Area) on Person where
      place := region

    relation move (origin : Area, target : Area) on Person subject to origin ≠ target where
      from Present, InArea(origin)
      hazard rate(origin) * Factor(target)
      set destination := target
      become InArea(target)
      set status := vacant

private def transitionNames (model : Model) : List String :=
  model.boxes.bind fun modelBox => modelBox.transitions.map (·.name)

#guard Mathematical.params.map (·.name) ==
  ["rate_north", "rate_south", "north_factor", "south_factor"]
#guard transitionNames Mathematical == ["move_north_south", "move_south_north"]
#guard (Mathematical.boxes.bind (·.tables) |>.map (·.attrs)) ==
  [[ { name := "place", ty := .enum ["north", "south"] }
   , { name := "destination", ty := .enum ["north", "south"] }
   , { name := "age", ty := .int }
   , { name := "status", ty := .enum ["present", "vacant"] }
   ]]

private def firstMove? := (Mathematical.boxes.bind (·.transitions)).head?
#guard firstMove?.map (·.guard) == some
  (Expr.and (Expr.enumIs "status" "present") (Expr.enumIs "place" "north"))
#guard firstMove?.map (·.hazard) == some
  (Expr.mul (Expr.param "rate_north") (Expr.param "south_factor"))
#guard firstMove?.map (·.effects) == some
  [ Effect.setAttr "destination" (Expr.enum "south")
  , Effect.setAttr "place" (Expr.enum "south")
  , Effect.setAttr "status" (Expr.enum "vacant")
  ]

sembla_model FunctionFormalBindings (dt := 1.0) where
  domain Level := 0 .. 1

  function Identity (level : Level) : Int where
    [0] := level
    [1] := level

  box population where
    system Person (rows := 1) where
      value : Int

    state Any on Person where
      match true

    transition literal_call on Person where
      guard true
      hazard 0.1
      set value := Identity(1)

    relation bound_call (selected : Level) on Person where
      from Any
      hazard 0.1
      set value := Identity(selected)

private def formalBindingRules :=
  FunctionFormalBindings.boxes.bind (·.transitions)
#guard formalBindingRules.map (·.name) ==
  ["literal_call", "bound_call_0", "bound_call_1"]
#guard formalBindingRules.map (·.effects) ==
  [[Effect.setAttr "value" (Expr.int 1)],
   [Effect.setAttr "value" (Expr.int 0)],
   [Effect.setAttr "value" (Expr.int 1)]]

sembla_model ScalarPriorFamilies (dt := 1.0) where
  param legacy : ℝ := 0.1 ~ LogNormal (-2.302585092994046) 0.5
  param symmetric : ℝ := 0.1 ~ Normal 0.1 0.01

  box population where
    system Person (rows := 1) where
      value : ℝ

#guard ScalarPriorFamilies.params.map (·.prior) ==
  [some { family := .logNormal, args := [(-2.302585092994046), 0.5] },
   some { family := .normal, args := [0.1, 0.01] }]

sembla_model ScalarForms (dt := 1.0) where
  box population where
    system Person (rows := 1) where
      age : Int
      status : {present, vacant}

    transition truth on Person where
      guard true
      hazard 0.1
      set status := vacant

    transition negated on Person where
      guard ¬false
      hazard 0.1
      set status := vacant

    transition older on Person where
      guard age ≥ 1
      hazard 0.1
      set status := vacant

private def scalarRules := ScalarForms.boxes.bind (·.transitions)
#guard scalarRules.map (·.guard) ==
  [Expr.bool true, Expr.not (Expr.bool false),
    Expr.ge (Expr.selfAttr "age") (Expr.int 1)]

sembla_model PartitionRelations (dt := 1.0) where
  partition AgeBand projects population.Person.age where
    young := 0 ..< 10
    old := 10 ..

  box population where
    system Person (rows := 4) where
      age : Int
      status : {present, vacant}

    state Present on Person where
      status := present

    state Vacant on Person where
      status := vacant

    relation age_out (band : AgeBand) on Person where
      from Present, AgeBand(band)
      hazard 0.1
      become Vacant

    transition age_fallback[band : AgeBand] on Person where
      guard status = present ∧ age ∈ band
      hazard 0.1
      set status := vacant

private def partitionRules := PartitionRelations.boxes.bind (·.transitions)
#guard partitionRules.map (·.name) ==
  ["age_out_young", "age_out_old", "age_fallback_young", "age_fallback_old"]
private def boundedBand := Expr.and (Expr.ge (Expr.selfAttr "age") (Expr.int 0))
  (Expr.lt (Expr.selfAttr "age") (Expr.int 10))
#guard partitionRules[0]?.map (·.guard) == some
  (Expr.and (Expr.enumIs "status" "present") boundedBand)
#guard partitionRules[1]?.map (·.guard) == some
  (Expr.and (Expr.enumIs "status" "present")
    (Expr.ge (Expr.selfAttr "age") (Expr.int 10)))
#guard partitionRules[2]?.map (·.guard) == partitionRules[0]?.map (·.guard)
#guard partitionRules[3]?.map (·.guard) == partitionRules[1]?.map (·.guard)

sembla_model NumericPartitionComponents (dt := 1.0) where
  partition NumericBand projects population.Person.age where
    «00_04» := 0 ..< 5
    «100_plus» := 5 ..

  param mortality (band : NumericBand) : ℝ where
    [«00_04»] := 0.1
    [«100_plus»] := 0.2

  box population where
    system Person (rows := 2) where
      age : Int
      occupancy : {present, vacant}
      event : {none_, death}
      area : {nsw, vic}
      sex : {male, female}

    state Occupied on Person where
      occupancy := present
    state NoEvent on Person where
      event := none_
    state InNsw on Person where
      area := nsw
    state Male on Person where
      sex := male
    state Vacant on Person where
      occupancy := vacant

    relation die (band : NumericBand) on Person where
      from Occupied, NoEvent, InNsw, Male, NumericBand(band)
      hazard mortality(band)
      become Vacant

private def numericPartitionRules := NumericPartitionComponents.boxes.bind (·.transitions)
#guard NumericPartitionComponents.params.map (·.name) ==
  ["mortality_00_04", "mortality_100_plus"]
#guard numericPartitionRules.map (·.name) == ["die_00_04", "die_100_plus"]
private def numericBounded := Expr.and (Expr.ge (Expr.selfAttr "age") (Expr.int 0))
  (Expr.lt (Expr.selfAttr "age") (Expr.int 5))
#guard numericPartitionRules[0]?.map (·.guard) == some
  (Expr.and (Expr.enumIs "occupancy" "present")
    (Expr.and (Expr.enumIs "event" "none_")
      (Expr.and (Expr.enumIs "area" "nsw")
        (Expr.and (Expr.enumIs "sex" "male") numericBounded))))
#guard numericPartitionRules[1]?.map (·.guard) == some
  (Expr.and (Expr.enumIs "occupancy" "present")
    (Expr.and (Expr.enumIs "event" "none_")
      (Expr.and (Expr.enumIs "area" "nsw")
        (Expr.and (Expr.enumIs "sex" "male")
          (Expr.ge (Expr.selfAttr "age") (Expr.int 5))))))

sembla_model ClaimOrder (dt := 1.0) where
  domain UnitDomain := {only}

  box population where
    system Resource (rows := 2)
    system Person (rows := 2) where
      status : {present, vacant}
      first : Resource
      second : Resource

    state Present on Person where
      status := present

    relation consume (unit : UnitDomain) on Person where
      from Present
      hazard 0.1
      claim first by race_time
      claim second by race_time
      set status := vacant

private def claimRule? := (ClaimOrder.boxes.bind (·.transitions)).head?
#guard claimRule?.map (·.contests) == some
  [ { resource := Expr.selfAttr "first", ordering := .raceTime }
  , { resource := Expr.selfAttr "second", ordering := .raceTime }
  ]

sembla_model NamedArrow (name := "named_arrow_twin") (dt := 1.0) where
  domain Health := {S, I}
  domain age := {child, adult}
  domain sex := {male, female}

  param β (a : age, g : sex) : ℝ where
    [child, male] := 0.1
    [child, female] := 0.2
    [adult, male] := 0.3
    [adult, female] := 0.4

  box population where
    system Person (rows := 10) where
      health : Health
      age : age
      sex : sex

    state Susceptible on Person where
      health := S

    state Infectious on Person where
      health := I

    infect on Person : Susceptible[age, sex] →[β(age, sex)] Infectious

sembla_model DirectNamedDomains (name := "named_arrow_twin") (dt := 1.0) where
  domain Health := {S, I}
  domain age := {child, adult}
  domain sex := {male, female}

  param β[age, sex] : ℝ where
    [child, male] := 0.1
    [child, female] := 0.2
    [adult, male] := 0.3
    [adult, female] := 0.4

  box population where
    system Person (rows := 10) where
      health : Health
      age : age
      sex : sex

    infect[age, sex] on Person : health: S →[β[age, sex]] I

sembla_model InlineDomainTwin (name := "named_arrow_twin") (dt := 1.0) where
  index age := {child, adult}
  index sex := {male, female}

  param β[age, sex] : ℝ where
    [child, male] := 0.1
    [child, female] := 0.2
    [adult, male] := 0.3
    [adult, female] := 0.4

  box population where
    system Person (rows := 10) where
      health : {S, I}
      age : {child, adult}
      sex : {male, female}

    infect[age, sex] on Person : health: S →[β[age, sex]] I

#guard NamedArrow == DirectNamedDomains
#guard NamedArrow == InlineDomainTwin
#guard Sembla.IR.toJson NamedArrow == Sembla.IR.toJson DirectNamedDomains
#guard Sembla.IR.toJson NamedArrow == Sembla.IR.toJson InlineDomainTwin
#guard transitionNames NamedArrow ==
  ["infect_child_male", "infect_child_female", "infect_adult_male", "infect_adult_female"]

private def firstInfect? := (NamedArrow.boxes.bind (·.transitions)).head?
#guard firstInfect?.map (·.guard) == some
  (Expr.and
    (Expr.and (Expr.enumIs "health" "S") (Expr.enumIs "age" "child"))
    (Expr.enumIs "sex" "male"))

sembla_model RelationOrdering (dt := 1.0) where
  domain Area8 := {nsw, vic, qld, sa, wa, tas, nt, act}
  box population where
    system Person (rows := 1) where
      place : Area8
      status : {present, vacant}
    state Active on Person where
      status := present
    relation route (origin : Area8, target : Area8) on Person subject to origin ≠ target where
      from Active
      hazard 0.1
      set place := target

private def relationOrderingNames := transitionNames RelationOrdering
#guard relationOrderingNames.length == 56
#guard relationOrderingNames ==
  ["route_nsw_vic", "route_nsw_qld", "route_nsw_sa", "route_nsw_wa",
   "route_nsw_tas", "route_nsw_nt", "route_nsw_act", "route_vic_nsw",
   "route_vic_qld", "route_vic_sa", "route_vic_wa", "route_vic_tas",
   "route_vic_nt", "route_vic_act", "route_qld_nsw", "route_qld_vic",
   "route_qld_sa", "route_qld_wa", "route_qld_tas", "route_qld_nt",
   "route_qld_act", "route_sa_nsw", "route_sa_vic", "route_sa_qld",
   "route_sa_wa", "route_sa_tas", "route_sa_nt", "route_sa_act",
   "route_wa_nsw", "route_wa_vic", "route_wa_qld", "route_wa_sa",
   "route_wa_tas", "route_wa_nt", "route_wa_act", "route_tas_nsw",
   "route_tas_vic", "route_tas_qld", "route_tas_sa", "route_tas_wa",
   "route_tas_nt", "route_tas_act", "route_nt_nsw", "route_nt_vic",
   "route_nt_qld", "route_nt_sa", "route_nt_wa", "route_nt_tas",
   "route_nt_act", "route_act_nsw", "route_act_vic", "route_act_qld",
   "route_act_sa", "route_act_wa", "route_act_tas", "route_act_nt"]

/-- Contextual introducers do not reserve ordinary Lean identifiers. -/
private def domain := 1
private def partition := 2
private def function := 3
private def relation := 4
private def become := 5
private def state := 6
private def subject := 7
private def claim := 8
private def Normal := 9
private def LogNormal := 10
#guard domain + partition + function + relation + become + state + subject + claim +
  Normal + LogNormal == 55

end Sembla.MathematicalSurfaceTests
