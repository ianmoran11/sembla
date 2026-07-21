import Sembla.Composition.Json
import Sembla.Models

namespace Sembla.Composition.Fixtures

open Sembla

private def sid (raw : String) : StableId := ⟨raw⟩

private def emptyBox : IR.Box := {
  name := ""
  tables := []
  «transitions» := []
  «inputs» := []
  «outputs» := []
  «views» := [] }

private def infectionSchema : List IR.Attr := [
  { name := "infected", ty := .int }]

private def restrictionSchema : List IR.Attr := [
  { name := "restriction", ty := .real }]

private def pulseSchema : List IR.Attr := [
  { name := "value", ty := .int }]

mutual
  private partial def renameRestrictionExpr : IR.Expr → IR.Expr
    | .selfAttr name =>
        .selfAttr (if name == "modifier_offset" then "restriction" else name)
    | .add lhs rhs => .add (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .sub lhs rhs => .sub (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .mul lhs rhs => .mul (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .div lhs rhs => .div (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .eq lhs rhs => .eq (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .ne lhs rhs => .ne (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .lt lhs rhs => .lt (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .le lhs rhs => .le (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .gt lhs rhs => .gt (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .ge lhs rhs => .ge (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .and lhs rhs => .and (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .or lhs rhs => .or (renameRestrictionExpr lhs) (renameRestrictionExpr rhs)
    | .not expression => .not (renameRestrictionExpr expression)
    | .input port aggregate => .input port (renameRestrictionAggregate aggregate)
    | .agg operation table fk selfFk filter =>
        .agg (renameRestrictionAggOp operation) table fk selfFk (renameRestrictionExpr filter)
    | expression => expression

  private partial def renameRestrictionAggOp : IR.AggOp → IR.AggOp
    | .count => .count
    | .sum value => .sum (renameRestrictionExpr value)

  private partial def renameRestrictionAggregate : IR.Aggregate → IR.Aggregate
    | .mk operation filter =>
        .mk (renameRestrictionAggOp operation) (filter.map renameRestrictionExpr)
end

private def scalePopulation (modelBox : IR.Box) : IR.Box :=
  { modelBox with tables := modelBox.tables.map fun table =>
      if table.name == "person" then { table with sizeHint := 1000 }
      else if table.name == "employer" then { table with sizeHint := 50 }
      else table }

private def adaptPopulation (modelBox : IR.Box) : IR.Box :=
  let scaled := scalePopulation modelBox
  { scaled with
    «transitions» := scaled.transitions.map fun item =>
      { item with «hazard» := renameRestrictionExpr item.hazard }
    «inputs» := scaled.inputs.map fun port =>
      if port.name == "restriction_modifier" then { port with schema := restrictionSchema }
      else port }

private def adaptPolicy (modelBox : IR.Box) : IR.Box :=
  { modelBox with «outputs» := modelBox.outputs.map fun item =>
      if item.name == "restriction_modifier" then
        let builder := match item.builder with
          | .perTable table outputFields => .perTable table (outputFields.map fun itemField =>
              if itemField.name == "modifier_offset" then { itemField with name := "restriction" }
              else itemField)
        { item with schema := restrictionSchema, builder }
      else item }

/-- The post-adaptation population leaf used by linker twin tests. -/
def populationBox : IR.Box :=
  adaptPopulation ((Models.sirPolicy.boxes.get? 0).getD emptyBox)

/-- The post-adaptation policy leaf used by linker twin tests. -/
def policyBox : IR.Box :=
  adaptPolicy ((Models.sirPolicy.boxes.get? 1).getD emptyBox)

private def populationDefinition : ComponentDefinitionV1 := {
  id := sid "def:population"
  displayName := "Population"
  parameterRequirements := ["beta", "gamma"]
  ports := [
    { id := sid "port:restriction_modifier"
      displayName := "Restriction modifier"
      direction := .input
      schema := restrictionSchema },
    { id := sid "port:infection_count"
      displayName := "Infection count"
      direction := .output
      schema := infectionSchema }]
  body := .primitive {
    tables := populationBox.tables
    «transitions» := populationBox.transitions
    «inputs» := populationBox.inputs
    «outputs» := populationBox.outputs
    «views» := populationBox.views } }

private def policyDefinition : ComponentDefinitionV1 := {
  id := sid "def:policy"
  displayName := "Policy"
  parameterRequirements := []
  ports := [
    { id := sid "port:infection_count"
      displayName := "Infection count"
      direction := .input
      schema := infectionSchema },
    { id := sid "port:restriction_modifier"
      displayName := "Restriction modifier"
      direction := .output
      schema := restrictionSchema }]
  body := .primitive {
    tables := policyBox.tables
    «transitions» := policyBox.transitions
    «inputs» := policyBox.inputs
    «outputs» := policyBox.outputs
    «views» := policyBox.views } }

private def betaGammaBindings : List ParameterBinding := [
  { requirement := "beta", «parameter» := "beta" },
  { requirement := "gamma", «parameter» := "gamma" }]

private def populationInstance : InstanceDeclV1 := {
  id := sid "inst:population"
  displayName := "Population"
  definition := sid "def:population"
  parameterBindings := betaGammaBindings }

private def policyInstance : InstanceDeclV1 := {
  id := sid "inst:policy"
  displayName := "Policy"
  definition := sid "def:policy"
  parameterBindings := [] }

private def regionalInstance
    (id definition displayName : String) : InstanceDeclV1 := {
  id := sid id
  displayName
  definition := sid definition
  parameterBindings := betaGammaBindings }

private def emptyComposite (instances : List InstanceDeclV1) : CompositeBodyV1 := {
  instances
  «wires» := []
  exposures := []
  hiddenPorts := [] }

private def independentDefinition : ComponentDefinitionV1 := {
  id := sid "def:independent_epidemic_policy"
  displayName := "Independent epidemic and policy"
  parameterRequirements := ["beta", "gamma"]
  ports := []
  body := .composite (emptyComposite [populationInstance, policyInstance]) }

private def epidemicWires : List WireDeclV1 := [
  { id := sid "wire:count_to_policy"
    sourceInstance := sid "inst:population"
    sourcePort := sid "port:infection_count"
    targetInstance := sid "inst:policy"
    targetPort := sid "port:infection_count"
    delayTicks := 1 },
  { id := sid "wire:restriction_to_population"
    sourceInstance := sid "inst:policy"
    sourcePort := sid "port:restriction_modifier"
    targetInstance := sid "inst:population"
    targetPort := sid "port:restriction_modifier"
    delayTicks := 1 }]

private def epidemicDefinition : ComponentDefinitionV1 := {
  id := sid "def:epidemic_policy"
  displayName := "Epidemic policy"
  parameterRequirements := ["beta", "gamma"]
  ports := []
  body := .composite {
    instances := [populationInstance, policyInstance]
    «wires» := epidemicWires
    exposures := []
    hiddenPorts := [] } }

/-- The exposure-bearing family variant.  It intentionally keeps the same
    stable definition id while the plain epidemic-policy fixture above remains
    exposure-free for PRD 0008. -/
private def pingDefinition : ComponentDefinitionV1 := {
  id := sid "def:ping"
  displayName := "Ping"
  parameterRequirements := []
  ports := [{
    id := sid "port:pulse"
    displayName := "Pulse"
    direction := .output
    schema := pulseSchema }]
  body := .primitive {
    tables := [{
      name := "sender"
      sizeHint := 1
      attrs := [{ name := "phase", ty := .enum ["Idle", "Fire"] }] }]
    «transitions» := [
      { name := "arm"
        table := "sender"
        «guard» := .enumIs "phase" "Idle"
        «hazard» := .real 1e300
        effects := [.setAttr "phase" (.enum "Fire")]
        contests := [] },
      { name := "disarm"
        table := "sender"
        «guard» := .enumIs "phase" "Fire"
        «hazard» := .real 1e300
        effects := [.setAttr "phase" (.enum "Idle")]
        contests := [] }]
    «inputs» := []
    «outputs» := [{
      name := "pulse"
      schema := pulseSchema
      builder := .perTable "sender" [{
        name := "value"
        op := .count
        filter := some (.enumIs "phase" "Fire") }] }]
    «views» := [] } }

private def pongDefinition : ComponentDefinitionV1 := {
  id := sid "def:pong"
  displayName := "Pong"
  parameterRequirements := []
  ports := [{
    id := sid "port:pulse"
    displayName := "Pulse"
    direction := .input
    schema := pulseSchema }]
  body := .primitive {
    tables := [{
      name := "receiver"
      sizeHint := 1
      attrs := [{ name := "seen", ty := .enum ["No", "Yes"] }] }]
    «transitions» := [{
      name := "notice"
      table := "receiver"
      «guard» := .and
        (.gt (.input "pulse" (.mk (.sum (.selfAttr "value")) none)) (.int 0))
        (.enumIs "seen" "No")
      «hazard» := .real 1e300
      effects := [.setAttr "seen" (.enum "Yes")]
      contests := [] }]
    «inputs» := [{ name := "pulse", schema := pulseSchema }]
    «outputs» := []
    «views» := [{
      name := "seen_yes"
      table := "receiver"
      filter := some (.enumIs "seen" "Yes")
      value := none
      «reduce» := .count }] } }

private def pingPongDefinition : ComponentDefinitionV1 := {
  id := sid "def:ping_pong"
  displayName := "Ping pong"
  parameterRequirements := []
  ports := []
  body := .composite {
    instances := [
      { id := sid "inst:ping"
        displayName := "Ping"
        definition := sid "def:ping"
        parameterBindings := [] },
      { id := sid "inst:pong"
        displayName := "Pong"
        definition := sid "def:pong"
        parameterBindings := [] }]
    «wires» := [{
      id := sid "wire:pulse"
      sourceInstance := sid "inst:ping"
      sourcePort := sid "port:pulse"
      targetInstance := sid "inst:pong"
      targetPort := sid "port:pulse"
      delayTicks := 1 }]
    exposures := []
    hiddenPorts := [] } }

private def epidemicPolicyExposed : ComponentDefinitionV1 := {
  id := sid "def:epidemic_policy"
  displayName := "Epidemic policy"
  parameterRequirements := ["beta", "gamma"]
  ports := [
    { id := sid "port:infection_count"
      displayName := "Infection count"
      direction := .output
      schema := infectionSchema },
    { id := sid "port:restriction_modifier"
      displayName := "Restriction modifier"
      direction := .output
      schema := restrictionSchema }]
  body := .composite {
    instances := [populationInstance, policyInstance]
    «wires» := epidemicWires
    exposures := [
      { id := sid "expose:infection_count"
        innerInstance := sid "inst:population"
        innerPort := sid "port:infection_count"
        outerPort := sid "port:infection_count" },
      { id := sid "expose:restriction_modifier"
        innerInstance := sid "inst:policy"
        innerPort := sid "port:restriction_modifier"
        outerPort := sid "port:restriction_modifier" }]
    hiddenPorts := [] } }

private def mkSource
    (name displayName : String) (definitions : List ComponentDefinitionV1)
    (root : String) (sourceSummaries : List SourceSummaryV1 := []) : CompositionSourceV1 := {
  schemaVersion := Plan.compositionSourceSchema
  modelId := sid ("model:" ++ name)
  displayName
  outerDt := 0.25
  parameters := Models.sirPolicy.params
  definitions
  rootDefinition := sid root
  requiredFeatures := []
  «summaries» := sourceSummaries }

def soloPopulation : CompositionSourceV1 :=
  let root : ComponentDefinitionV1 := {
    id := sid "def:solo_population"
    displayName := "Solo population"
    parameterRequirements := ["beta", "gamma"]
    ports := []
    body := .composite (emptyComposite [populationInstance]) }
  mkSource "solo_population" "Solo population"
    [populationDefinition, root] "def:solo_population"

def independentEpidemicPolicy : CompositionSourceV1 :=
  mkSource "independent_epidemic_policy" "Independent epidemic and policy"
    [populationDefinition, policyDefinition, independentDefinition]
    "def:independent_epidemic_policy"

def twoIndependentRegions : CompositionSourceV1 :=
  let root : ComponentDefinitionV1 := {
    id := sid "def:two_independent_regions"
    displayName := "Two independent regions"
    parameterRequirements := ["beta", "gamma"]
    ports := []
    body := .composite (emptyComposite [
      regionalInstance "inst:north" "def:independent_epidemic_policy" "North",
      regionalInstance "inst:south" "def:independent_epidemic_policy" "South"]) }
  mkSource "two_independent_regions" "Two independent regions"
    [populationDefinition, policyDefinition, independentDefinition, root]
    "def:two_independent_regions"

def epidemicPolicy : CompositionSourceV1 :=
  mkSource "epidemic_policy" "Epidemic policy"
    [populationDefinition, policyDefinition, epidemicDefinition]
    "def:epidemic_policy"

def pingPong : CompositionSourceV1 :=
  mkSource "ping_pong" "Ping pong"
    [pingDefinition, pongDefinition, pingPongDefinition]
    "def:ping_pong"

def twoRegions : CompositionSourceV1 :=
  let root : ComponentDefinitionV1 := {
    id := sid "def:two_regions"
    displayName := "Two regions"
    parameterRequirements := ["beta", "gamma"]
    ports := []
    body := .composite (emptyComposite [
      regionalInstance "inst:north" "def:epidemic_policy" "North",
      regionalInstance "inst:south" "def:epidemic_policy" "South"]) }
  mkSource "two_regions" "Two regions"
    [populationDefinition, policyDefinition, epidemicPolicyExposed, root]
    "def:two_regions" [{
      name := "peak_i"
      «reduce» := .max
      instancePath := [sid "inst:north", sid "inst:population"]
      «view» := "I" }]

def regionalResponse : CompositionSourceV1 :=
  let root : ComponentDefinitionV1 := {
    id := sid "def:regional_response"
    displayName := "Regional response"
    parameterRequirements := ["beta", "gamma"]
    ports := [{
      id := sid "port:regional_infection_count"
      displayName := "Regional infection count"
      direction := .output
      schema := infectionSchema }]
    body := .composite {
      instances := [regionalInstance "inst:epidemic" "def:epidemic_policy" "Epidemic"]
      «wires» := []
      exposures := [{
        id := sid "expose:regional_infection_count"
        innerInstance := sid "inst:epidemic"
        innerPort := sid "port:infection_count"
        outerPort := sid "port:regional_infection_count" }]
      hiddenPorts := [{
        instance_ := sid "inst:epidemic"
        port := sid "port:restriction_modifier" }] } }
  mkSource "regional_response" "Regional response"
    [populationDefinition, policyDefinition, epidemicPolicyExposed, root]
    "def:regional_response"

/-- Export registry.  Names are the frozen fixture stems. -/
def corpus : List (String × CompositionSourceV1) := [
  ("solo_population", soloPopulation),
  ("independent_epidemic_policy", independentEpidemicPolicy),
  ("two_independent_regions", twoIndependentRegions),
  ("epidemic_policy", epidemicPolicy),
  ("ping_pong", pingPong),
  ("two_regions", twoRegions),
  ("regional_response", regionalResponse)]

def lookup (name : String) : Option CompositionSourceV1 :=
  (corpus.find? fun entry => entry.1 == name).map (·.2)

end Sembla.Composition.Fixtures
