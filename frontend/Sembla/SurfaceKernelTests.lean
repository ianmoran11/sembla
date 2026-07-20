import Sembla.Json
import Sembla.DSL

namespace Sembla.SurfaceKernelTests
open Sembla.IR Sembla.DSL Sembla.Widgets

/-- A compact contract fixture whose declarations deliberately use multiple
    entries in every ordered surface category.  Output builders are written in
    reverse schema order so this also pins the schema-ordered IR field rule. -/
private def orderingModel : Model := model% "surface_order_contract" step(0.5) where
  params [
    param firstRate : Real := 0.1,
    param secondRate : Real := 0.2]
  boxes [
    box alphaBox where
      systems [
        system Actor as "actor" rows(10) where [
          state phase : {Start, Middle, End},
          attr score : Real,
          attr visits : Int],
        system Group as "group" rows(4) where [
          attr rank : Int,
          attr weight : Real]]
      inputs [
        input inboundOne {first : Int, second : Real},
        input inboundTwo {first : Int, second : Real}]
      transitions [
        transition alphaMove on Actor where
          guard phase = Start
          hazard parameter firstRate
          set [phase := Middle, score := 1.5, visits := 1],
        transition alphaScore on Actor where
          guard phase = Middle
          hazard parameter secondRate
          set [score := 2.5, visits := 2]]
      outputs [
        output outgoingOne {first : Int, second : Real} from Actor fields [
          field second := sum (score),
          field first := count where phase = Start],
        output outgoingTwo {first : Int, second : Real} from Actor fields [
          field second := sum (score),
          field first := count where phase = Middle]]
      views [
        view alphaCount from Actor where phase = Start reduce count,
        view alphaTotal from Actor using score reduce sum],
    box betaBox where
      systems [
        system Controller as "controller" rows(2) where [
          state mode : {Closed, Open},
          attr level : Real,
          attr events : Int],
        system Archive as "archive" rows(3) where [
          attr rank : Int,
          attr weight : Real]]
      inputs [
        input receivedOne {first : Int, second : Real},
        input receivedTwo {first : Int, second : Real}]
      transitions [
        transition betaOpen on Controller where
          guard mode = Closed
          hazard parameter firstRate
          set [mode := Open, level := 3.5, events := 3],
        transition betaClose on Controller where
          guard mode = Open
          hazard parameter secondRate
          set [level := 4.5, events := 4]]
      outputs [
        output responseOne {first : Int, second : Real} from Controller fields [
          field second := sum (level),
          field first := count where mode = Closed],
        output responseTwo {first : Int, second : Real} from Controller fields [
          field second := sum (level),
          field first := count where mode = Open]]
      views [
        view betaCount from Controller where mode = Closed reduce count,
        view betaTotal from Controller using level reduce sum]]
  wires [
    wire alphaBox outgoingOne -> betaBox receivedOne,
    wire alphaBox outgoingTwo -> betaBox receivedTwo,
    wire betaBox responseOne -> alphaBox inboundOne,
    wire betaBox responseTwo -> alphaBox inboundTwo]
  summaries [
    summary alphaCountLast from alphaBox view alphaCount reduce last,
    summary alphaTotalMax from alphaBox view alphaTotal reduce max,
    summary betaCountSum from betaBox view betaCount reduce sum,
    summary betaTotalTick from betaBox view betaTotal reduce argmax_tick]

#guard orderingModel.params.map (·.name) == ["firstRate", "secondRate"]
#guard orderingModel.boxes.map (·.name) == ["alphaBox", "betaBox"]
#guard orderingModel.boxes.map (fun item => item.tables.map (·.name)) ==
  [["actor", "group"], ["controller", "archive"]]
#guard orderingModel.boxes.bind (fun item => item.transitions.map (·.name)) ==
  ["alphaMove", "alphaScore", "betaOpen", "betaClose"]
#guard orderingModel.boxes.map (fun item => item.inputs.map (·.name)) ==
  [["inboundOne", "inboundTwo"], ["receivedOne", "receivedTwo"]]
#guard orderingModel.boxes.map (fun item => item.outputs.map (·.name)) ==
  [["outgoingOne", "outgoingTwo"], ["responseOne", "responseTwo"]]
#guard orderingModel.boxes.map (fun item => item.views.map (·.name)) ==
  [["alphaCount", "alphaTotal"], ["betaCount", "betaTotal"]]
#guard orderingModel.wires.map (fun item =>
    (item.source.box, item.source.port, item.target.box, item.target.port)) ==
  [("alphaBox", "outgoingOne", "betaBox", "receivedOne"),
   ("alphaBox", "outgoingTwo", "betaBox", "receivedTwo"),
   ("betaBox", "responseOne", "alphaBox", "inboundOne"),
   ("betaBox", "responseTwo", "alphaBox", "inboundTwo")]
#guard orderingModel.summaries.map (·.name) ==
  ["alphaCountLast", "alphaTotalMax", "betaCountSum", "betaTotalTick"]

private def tableAt? (boxIndex tableIndex : Nat) : Option Table :=
  orderingModel.boxes[boxIndex]? >>= fun item => item.tables[tableIndex]?

private def inputAt? (boxIndex inputIndex : Nat) : Option PortDecl :=
  orderingModel.boxes[boxIndex]? >>= fun item => item.inputs[inputIndex]?

private def outputAt? (boxIndex outputIndex : Nat) : Option OutputDecl :=
  orderingModel.boxes[boxIndex]? >>= fun item => item.outputs[outputIndex]?

private def transitionAt? (boxIndex transitionIndex : Nat) : Option Transition :=
  orderingModel.boxes[boxIndex]? >>= fun item => item.transitions[transitionIndex]?

#guard (tableAt? 0 0).map (fun item => item.attrs.map (·.name)) ==
  some ["phase", "score", "visits"]
#guard (tableAt? 0 1).map (fun item => item.attrs.map (·.name)) ==
  some ["rank", "weight"]
#guard (tableAt? 1 0).map (fun item => item.attrs.map (·.name)) ==
  some ["mode", "level", "events"]
#guard (inputAt? 0 0).map (fun item => item.schema.map (·.name)) ==
  some ["first", "second"]
#guard (outputAt? 0 0).map (fun item => item.schema.map (·.name)) ==
  some ["first", "second"]

private def outputFieldNames (item : OutputDecl) : List String :=
  match item.builder with
  | .perTable _ declarations => declarations.map (·.name)

#guard (outputAt? 0 0).map outputFieldNames == some ["first", "second"]
#guard (outputAt? 1 0).map outputFieldNames == some ["first", "second"]

private def effectAttrs (item : Transition) : List String :=
  item.effects.map fun effect => match effect with
    | .setAttr name _ => name

#guard (transitionAt? 0 0).map effectAttrs == some ["phase", "score", "visits"]
#guard (transitionAt? 0 1).map effectAttrs == some ["score", "visits"]
#guard (transitionAt? 1 0).map effectAttrs == some ["mode", "level", "events"]
#guard (transitionAt? 1 1).map effectAttrs == some ["level", "events"]

-- Exact serialization pins every nested list and field order as one frozen value.
#guard toJson orderingModel ==
  "{\"name\":\"surface_order_contract\",\"dt\":0.5,\"params\":[{\"name\":\"firstRate\",\"ty\":\"real\",\"default\":{\"kind\":\"real\",\"value\":0.1},\"prior\":null},{\"name\":\"secondRate\",\"ty\":\"real\",\"default\":{\"kind\":\"real\",\"value\":0.2},\"prior\":null}],\"boxes\":[{\"name\":\"alphaBox\",\"tables\":[{\"name\":\"actor\",\"size_hint\":10,\"attrs\":[{\"name\":\"phase\",\"ty\":{\"kind\":\"enum\",\"variants\":[\"Start\",\"Middle\",\"End\"]}},{\"name\":\"score\",\"ty\":{\"kind\":\"real\"}},{\"name\":\"visits\",\"ty\":{\"kind\":\"int\"}}]},{\"name\":\"group\",\"size_hint\":4,\"attrs\":[{\"name\":\"rank\",\"ty\":{\"kind\":\"int\"}},{\"name\":\"weight\",\"ty\":{\"kind\":\"real\"}}]}],\"transitions\":[{\"name\":\"alphaMove\",\"table\":\"actor\",\"guard\":{\"kind\":\"enum_is\",\"attr\":\"phase\",\"variant\":\"Start\"},\"hazard\":{\"kind\":\"param\",\"name\":\"firstRate\"},\"effects\":[{\"kind\":\"set_attr\",\"attr\":\"phase\",\"value\":{\"kind\":\"enum\",\"variant\":\"Middle\"}},{\"kind\":\"set_attr\",\"attr\":\"score\",\"value\":{\"kind\":\"real\",\"value\":1.5}},{\"kind\":\"set_attr\",\"attr\":\"visits\",\"value\":{\"kind\":\"int\",\"value\":1}}],\"contests\":[]},{\"name\":\"alphaScore\",\"table\":\"actor\",\"guard\":{\"kind\":\"enum_is\",\"attr\":\"phase\",\"variant\":\"Middle\"},\"hazard\":{\"kind\":\"param\",\"name\":\"secondRate\"},\"effects\":[{\"kind\":\"set_attr\",\"attr\":\"score\",\"value\":{\"kind\":\"real\",\"value\":2.5}},{\"kind\":\"set_attr\",\"attr\":\"visits\",\"value\":{\"kind\":\"int\",\"value\":2}}],\"contests\":[]}],\"inputs\":[{\"name\":\"inboundOne\",\"schema\":[{\"name\":\"first\",\"ty\":{\"kind\":\"int\"}},{\"name\":\"second\",\"ty\":{\"kind\":\"real\"}}]},{\"name\":\"inboundTwo\",\"schema\":[{\"name\":\"first\",\"ty\":{\"kind\":\"int\"}},{\"name\":\"second\",\"ty\":{\"kind\":\"real\"}}]}],\"outputs\":[{\"name\":\"outgoingOne\",\"schema\":[{\"name\":\"first\",\"ty\":{\"kind\":\"int\"}},{\"name\":\"second\",\"ty\":{\"kind\":\"real\"}}],\"builder\":{\"kind\":\"per_table\",\"table\":\"actor\",\"fields\":[{\"name\":\"first\",\"op\":{\"kind\":\"count\"},\"filter\":{\"kind\":\"enum_is\",\"attr\":\"phase\",\"variant\":\"Start\"}},{\"name\":\"second\",\"op\":{\"kind\":\"sum\",\"value\":{\"kind\":\"self_attr\",\"name\":\"score\"}},\"filter\":null}]}},{\"name\":\"outgoingTwo\",\"schema\":[{\"name\":\"first\",\"ty\":{\"kind\":\"int\"}},{\"name\":\"second\",\"ty\":{\"kind\":\"real\"}}],\"builder\":{\"kind\":\"per_table\",\"table\":\"actor\",\"fields\":[{\"name\":\"first\",\"op\":{\"kind\":\"count\"},\"filter\":{\"kind\":\"enum_is\",\"attr\":\"phase\",\"variant\":\"Middle\"}},{\"name\":\"second\",\"op\":{\"kind\":\"sum\",\"value\":{\"kind\":\"self_attr\",\"name\":\"score\"}},\"filter\":null}]}}],\"views\":[{\"name\":\"alphaCount\",\"table\":\"actor\",\"filter\":{\"kind\":\"enum_is\",\"attr\":\"phase\",\"variant\":\"Start\"},\"value\":null,\"reduce\":\"count\"},{\"name\":\"alphaTotal\",\"table\":\"actor\",\"filter\":null,\"value\":{\"kind\":\"self_attr\",\"name\":\"score\"},\"reduce\":\"sum\"}]},{\"name\":\"betaBox\",\"tables\":[{\"name\":\"controller\",\"size_hint\":2,\"attrs\":[{\"name\":\"mode\",\"ty\":{\"kind\":\"enum\",\"variants\":[\"Closed\",\"Open\"]}},{\"name\":\"level\",\"ty\":{\"kind\":\"real\"}},{\"name\":\"events\",\"ty\":{\"kind\":\"int\"}}]},{\"name\":\"archive\",\"size_hint\":3,\"attrs\":[{\"name\":\"rank\",\"ty\":{\"kind\":\"int\"}},{\"name\":\"weight\",\"ty\":{\"kind\":\"real\"}}]}],\"transitions\":[{\"name\":\"betaOpen\",\"table\":\"controller\",\"guard\":{\"kind\":\"enum_is\",\"attr\":\"mode\",\"variant\":\"Closed\"},\"hazard\":{\"kind\":\"param\",\"name\":\"firstRate\"},\"effects\":[{\"kind\":\"set_attr\",\"attr\":\"mode\",\"value\":{\"kind\":\"enum\",\"variant\":\"Open\"}},{\"kind\":\"set_attr\",\"attr\":\"level\",\"value\":{\"kind\":\"real\",\"value\":3.5}},{\"kind\":\"set_attr\",\"attr\":\"events\",\"value\":{\"kind\":\"int\",\"value\":3}}],\"contests\":[]},{\"name\":\"betaClose\",\"table\":\"controller\",\"guard\":{\"kind\":\"enum_is\",\"attr\":\"mode\",\"variant\":\"Open\"},\"hazard\":{\"kind\":\"param\",\"name\":\"secondRate\"},\"effects\":[{\"kind\":\"set_attr\",\"attr\":\"level\",\"value\":{\"kind\":\"real\",\"value\":4.5}},{\"kind\":\"set_attr\",\"attr\":\"events\",\"value\":{\"kind\":\"int\",\"value\":4}}],\"contests\":[]}],\"inputs\":[{\"name\":\"receivedOne\",\"schema\":[{\"name\":\"first\",\"ty\":{\"kind\":\"int\"}},{\"name\":\"second\",\"ty\":{\"kind\":\"real\"}}]},{\"name\":\"receivedTwo\",\"schema\":[{\"name\":\"first\",\"ty\":{\"kind\":\"int\"}},{\"name\":\"second\",\"ty\":{\"kind\":\"real\"}}]}],\"outputs\":[{\"name\":\"responseOne\",\"schema\":[{\"name\":\"first\",\"ty\":{\"kind\":\"int\"}},{\"name\":\"second\",\"ty\":{\"kind\":\"real\"}}],\"builder\":{\"kind\":\"per_table\",\"table\":\"controller\",\"fields\":[{\"name\":\"first\",\"op\":{\"kind\":\"count\"},\"filter\":{\"kind\":\"enum_is\",\"attr\":\"mode\",\"variant\":\"Closed\"}},{\"name\":\"second\",\"op\":{\"kind\":\"sum\",\"value\":{\"kind\":\"self_attr\",\"name\":\"level\"}},\"filter\":null}]}},{\"name\":\"responseTwo\",\"schema\":[{\"name\":\"first\",\"ty\":{\"kind\":\"int\"}},{\"name\":\"second\",\"ty\":{\"kind\":\"real\"}}],\"builder\":{\"kind\":\"per_table\",\"table\":\"controller\",\"fields\":[{\"name\":\"first\",\"op\":{\"kind\":\"count\"},\"filter\":{\"kind\":\"enum_is\",\"attr\":\"mode\",\"variant\":\"Open\"}},{\"name\":\"second\",\"op\":{\"kind\":\"sum\",\"value\":{\"kind\":\"self_attr\",\"name\":\"level\"}},\"filter\":null}]}}],\"views\":[{\"name\":\"betaCount\",\"table\":\"controller\",\"filter\":{\"kind\":\"enum_is\",\"attr\":\"mode\",\"variant\":\"Closed\"},\"value\":null,\"reduce\":\"count\"},{\"name\":\"betaTotal\",\"table\":\"controller\",\"filter\":null,\"value\":{\"kind\":\"self_attr\",\"name\":\"level\"},\"reduce\":\"sum\"}]}],\"wires\":[{\"from\":{\"box\":\"alphaBox\",\"port\":\"outgoingOne\"},\"to\":{\"box\":\"betaBox\",\"port\":\"receivedOne\"}},{\"from\":{\"box\":\"alphaBox\",\"port\":\"outgoingTwo\"},\"to\":{\"box\":\"betaBox\",\"port\":\"receivedTwo\"}},{\"from\":{\"box\":\"betaBox\",\"port\":\"responseOne\"},\"to\":{\"box\":\"alphaBox\",\"port\":\"inboundOne\"}},{\"from\":{\"box\":\"betaBox\",\"port\":\"responseTwo\"},\"to\":{\"box\":\"alphaBox\",\"port\":\"inboundTwo\"}}],\"summaries\":[{\"name\":\"alphaCountLast\",\"box\":\"alphaBox\",\"view\":\"alphaCount\",\"reduce\":\"last\"},{\"name\":\"alphaTotalMax\",\"box\":\"alphaBox\",\"view\":\"alphaTotal\",\"reduce\":\"max\"},{\"name\":\"betaCountSum\",\"box\":\"betaBox\",\"view\":\"betaCount\",\"reduce\":\"sum\"},{\"name\":\"betaTotalTick\",\"box\":\"betaBox\",\"view\":\"betaTotal\",\"reduce\":\"argmax_tick\"}]}\n"

/-- Legacy and option-B spellings must feed the same collected kernel and emit
    exactly the same model, including prior and priorless parameter metadata. -/
private def legacyBinderModel : Model := model% "binder_contract" step(0.5) where
  params [
    param beta : Real := 0.8 prior LogNormal(-0.2231435513142097, 0.25),
    param gamma : Real := 0.1]
  boxes [box binders where
    systems [system Person as "person" rows(4) where [
      state health : {S, I, R},
      attr score : Real]]
    inputs []
    transitions [transition advance on Person where
      guard health = S
      hazard parameter beta
      set [health := I, score := 1.0]]
    outputs []]
  wires []

private def optionBBinderModel : Model := model% "binder_contract" step(0.5) where
  params [
    param β : ℝ := 0.8 ~ LogNormal (-0.2231435513142097) 0.25,
    param γ : ℝ := 0.1]
  boxes [box binders where
    systems [system Person (rows := 4) where [
      state health : {S, I, R},
      attr score : ℝ]]
    inputs []
    transitions [transition advance on Person where
      guard health = S
      hazard β
      set [health := I, score := 1.0]]
    outputs []]
  wires []

#guard optionBBinderModel.params.map (·.name) == ["beta", "gamma"]
#guard optionBBinderModel.params.map (·.prior.isSome) == [true, false]
#guard optionBBinderModel.params == legacyBinderModel.params
#guard optionBBinderModel == legacyBinderModel
#guard toJson optionBBinderModel == toJson legacyBinderModel

private def binderHazard? : Option Expr := do
  let item ← optionBBinderModel.boxes.head?
  let selected ← item.transitions.head?
  pure selected.hazard

#guard binderHazard? == some (.param "beta")

private def optionBPanel? : Option HazardPanelProps :=
  hazardPanelProps? optionBBinderModel "binders" "advance"

#guard (optionBPanel? >>= (·.params.head?)).map (·.name) == some "beta"
#guard (optionBPanel? >>= (·.params.head?)).map (·.defaultValue) == some 0.8
#guard (optionBPanel? >>= (·.params.head?) >>= (·.density)).map (·.family) ==
  some "LogNormal"
#guard hazardPanelProps? optionBBinderModel "binders" "advance" ==
  hazardPanelProps? legacyBinderModel "binders" "advance"

/-- Each mathematical alias is pinned directly to its existing IR node. -/
private def aliasModel : Model := model% "alias_contract" step(1.0) where
  params [param β : ℝ := 0.5]
  boxes [box aliases where
    systems [system PolicyController (rows := 2) where [
      state health : {S, I, R},
      attr score : ℝ]]
    inputs []
    transitions [transition compare on PolicyController where
      guard health ≠ I ∧ score ≤ 2.0
      hazard β · 1.0
      set [health := I, score := 1.0]]
    outputs []]
  wires []

private def aliasTransition? : Option Transition := do
  let item ← aliasModel.boxes.head?
  item.transitions.head?

private def isAliasGuard : Expr → Bool
  | .and (.ne (.selfAttr "health") (.enum "I"))
      (.le (.selfAttr "score") (.real _)) => true
  | _ => false

private def isAliasHazard : Expr → Bool
  | .mul (.param "beta") (.real _) => true
  | _ => false

#guard aliasModel.params.map (·.name) == ["beta"]
#guard aliasModel.boxes.map (fun item => item.tables.map (·.name)) ==
  [["policy_controller"]]
#guard aliasTransition?.map (isAliasGuard ·.guard) == some true
#guard aliasTransition?.map (isAliasHazard ·.hazard) == some true

end Sembla.SurfaceKernelTests
