import Sembla.Hash
import Sembla.PlanJson

namespace Sembla.PlanExport

open Sembla

private def isLowerAscii (character : Char) : Bool :=
  let code := character.toNat
  'a'.toNat <= code && code <= 'z'.toNat

private def isDigitAscii (character : Char) : Bool :=
  let code := character.toNat
  '0'.toNat <= code && code <= '9'.toNat

/-- The frozen ASCII slug grammar `[a-z][a-z0-9_]*`. -/
def isSlug (value : String) : Bool :=
  match value.toList with
  | [] => false
  | first :: rest =>
      isLowerAscii first && rest.all fun character =>
        isLowerAscii character || isDigitAscii character || character == '_'

private def validateSlugs (model : IR.Model) : Except String Unit := do
  if !isSlug model.name then
    throw s!"model name '{model.name}' is not a slug"
  for modelBox in model.boxes do
    if !isSlug modelBox.name then
      throw s!"box name '{modelBox.name}' is not a slug"
    for transition in modelBox.transitions do
      if !isSlug transition.name then
        throw s!"transition name '{transition.name}' in box '{modelBox.name}' is not a slug"
    for port in modelBox.inputs do
      if !isSlug port.name then
        throw s!"input port name '{port.name}' in box '{modelBox.name}' is not a slug"
    for port in modelBox.outputs do
      if !isSlug port.name then
        throw s!"output port name '{port.name}' in box '{modelBox.name}' is not a slug"

private def sortBy (items : List α) (key : α → String) : List α :=
  items.mergeSort fun left right => key left < key right

private def canonicalBox (modelBox : IR.Box) : IR.Box :=
  { modelBox with
    tables := sortBy modelBox.tables (·.name)
    transitions := sortBy modelBox.transitions (·.name)
    inputs := sortBy modelBox.inputs (·.name)
    outputs := sortBy modelBox.outputs (·.name)
    views := sortBy modelBox.views (·.name) }

private def canonicalModel (model : IR.Model) : IR.Model :=
  { model with
    params := sortBy model.params (·.name)
    boxes := sortBy (model.boxes.map canonicalBox) (·.name)
    wires := sortBy model.wires PlanJson.directMailboxIdentity
    summaries := sortBy model.summaries (·.name) }

private def occurrence (boxName : String) : String :=
  "occ:" ++ boxName

private def transitionIdentity (boxName transitionName : String) : String :=
  occurrence boxName ++ "#" ++ transitionName

/-- Validate reserved and duplicate plan words without reassigning either. -/
def validateRuleWords (transitions : List Plan.TransitionIdentityV1) : Except String Unit :=
  let rec loop (seen : List (UInt32 × String)) : List Plan.TransitionIdentityV1 → Except String Unit
    | [] => pure ()
    | transition :: rest => do
        if Hash.isReservedRuleWord transition.ruleWord then
          throw s!"rule word {transition.ruleWord.toNat} for '{transition.identity}' is reserved"
        match seen.find? fun entry => entry.1 == transition.ruleWord with
        | some first =>
            throw s!"duplicate rule word {transition.ruleWord.toNat} for '{transition.identity}'; first used by '{first.2}'"
        | none => loop ((transition.ruleWord, transition.identity) :: seen) rest
  loop [] transitions

/-- Testable direct-stable constructor; production uses `Sembla.Hash.ruleWord`. -/
def directStablePlanWithRuleWord
    (deriveRuleWord : String → UInt32) (inputModel : IR.Model) :
    Except String Plan.ExecutablePlanV1 := do
  validateSlugs inputModel
  let model := canonicalModel inputModel
  let leaves := sortBy (model.boxes.map fun modelBox =>
    { box := modelBox.name
      occurrence := occurrence modelBox.name : Plan.LeafIdentityV1 }) (·.box)
  let transitions := sortBy ((model.boxes.map fun modelBox =>
    modelBox.transitions.map fun transition =>
      let identity := transitionIdentity modelBox.name transition.name
      { box := modelBox.name
        name := transition.name
        identity
        ruleWord := deriveRuleWord identity : Plan.TransitionIdentityV1 }).join) (·.identity)
  validateRuleWords transitions
  let mailboxes := sortBy (model.wires.map fun wire =>
    { identity := PlanJson.directMailboxIdentity wire
      sourceBox := wire.source.box
      sourcePort := wire.source.port
      targetBox := wire.target.box
      targetPort := wire.target.port : Plan.MailboxIdentityV1 }) (·.identity)
  let schedulerLeaves := leaves.map (·.box)
  pure {
    schemaVersion := Plan.planSchema
    identityScheme := Plan.stableIdentityScheme
    origin := .directStable
    model
    identity := {
      modelId := "model:" ++ model.name
      enabledFeatures := []
      schedulerDomains := [{
        id := Plan.globalSchedulerDomain
        algorithm := Plan.tauLeapAlgorithm
        leaves := schedulerLeaves }]
      leaves
      transitions
      mailboxes }
    linkedProvenance := none }

/-- Build the frozen direct-stable V1 plan envelope for a registered model. -/
def directStablePlan (model : IR.Model) : Except String Plan.ExecutablePlanV1 :=
  directStablePlanWithRuleWord Hash.ruleWord model

end Sembla.PlanExport
