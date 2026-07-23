import Sembla.Composition.SurfaceModels
import Sembla.Composition.Fixtures
import Sembla.WidgetDisplay

namespace Sembla.Composition.WidgetTests

open Lean ProofWidgets
open Sembla.Composition.Widget Sembla.WidgetDisplay
open Sembla.Composition.SurfaceModels

private def epidemicPolicyProps : CompositionDiagramProps :=
  diagramPropsOfDefinitionWithDefinitions
    Exposing.EpidemicPolicy__semblaDefinitions Exposing.EpidemicPolicy

#guard epidemicPolicyProps.title == "Epidemic policy"
#guard epidemicPolicyProps.nodes.map (·.instanceName) == #["Population", "Policy"]
#guard epidemicPolicyProps.nodes.map (·.definitionName) == #["Population", "Policy"]
#guard epidemicPolicyProps.nodes.map (·.kind) == #["primitive", "primitive"]
#guard epidemicPolicyProps.nodes[0]!.ports == #[
  ("Restriction modifier", "input"), ("Infection count", "output")]
#guard epidemicPolicyProps.nodes[1]!.ports == #[
  ("Infection count", "input"), ("Restriction modifier", "output")]
#guard epidemicPolicyProps.wires.map (fun edge =>
  (edge.source, edge.target, edge.delayTicks)) == #[
    ("Population.Infection count", "Policy.Infection count", 1),
    ("Policy.Restriction modifier", "Population.Restriction modifier", 1)]
#guard epidemicPolicyProps.exposures.map (fun exposure =>
  (exposure.inner, exposure.outer)) == #[
    ("Population.Infection count", "Infection count"),
    ("Policy.Restriction modifier", "Restriction modifier")]
#guard epidemicPolicyProps.hidden.isEmpty

private def twoRegionsProps : CompositionDiagramProps :=
  diagramPropsOfDefinitionWithDefinitions
    TwoRegions__semblaDefinitions TwoRegions

#guard twoRegionsProps.nodes.map (·.instanceName) == #["North", "South"]
#guard twoRegionsProps.nodes.map (·.definitionName) ==
  #["Epidemic policy", "Epidemic policy"]
#guard twoRegionsProps.nodes.map (·.kind) == #["composite", "composite"]
#guard twoRegionsProps.nodes.all fun node => node.ports == #[
  ("Infection count", "output"), ("Restriction modifier", "output")]
#guard twoRegionsProps.wires.isEmpty
#guard twoRegionsProps.exposures.isEmpty
#guard twoRegionsProps.hidden.isEmpty

private def regionalResponseProps : CompositionDiagramProps :=
  diagramPropsOfSource Sembla.Composition.Fixtures.regionalResponse

#guard regionalResponseProps.title == "Regional response"
#guard regionalResponseProps.nodes.map (fun node =>
  (node.instanceName, node.definitionName, node.kind)) ==
    #[("Epidemic", "Epidemic policy", "composite")]
#guard regionalResponseProps.wires.isEmpty
#guard regionalResponseProps.exposures.map (fun exposure =>
  (exposure.inner, exposure.outer)) ==
    #[("Epidemic.Infection count", "Regional infection count")]
#guard regionalResponseProps.hidden == #["Epidemic.Restriction modifier"]

private def populationProps : CompositionDiagramProps :=
  diagramPropsOfDefinition Population

#guard populationProps.title == "Population"
#guard populationProps.nodes.map (fun node =>
  (node.instanceName, node.definitionName, node.kind)) ==
    #[("Population", "Population", "primitive")]
#guard populationProps.nodes[0]!.ports == #[
  ("Restriction modifier", "input"), ("Infection count", "output")]
#guard populationProps.wires.isEmpty
#guard populationProps.exposures.isEmpty
#guard populationProps.hidden.isEmpty

-- JSON encoding is part of the pure props contract consumed by ProofWidgets.
private def propsJson := toJson epidemicPolicyProps
#guard (propsJson.getObjVal? "nodes").isOk
#guard (propsJson.getObjVal? "wires").isOk
#guard (propsJson.getObjVal? "exposures").isOk
#guard (propsJson.getObjVal? "hidden").isOk

private def stylesAreObjects : Nat → Html → Bool
  | 0, _ => false
  | _ + 1, .text _ => true
  | fuel + 1, .component _ _ _ children => children.all (stylesAreObjects fuel)
  | fuel + 1, .element _ attributes children =>
      attributes.all (fun (name, value) =>
        name != "style" || match value with
          | .obj _ => true
          | _ => false) &&
      children.all (stylesAreObjects fuel)

private def allHtmlTexts : Nat → Html → List String
  | 0, _ => []
  | _ + 1, .text value => [value]
  | fuel + 1, .component _ _ _ children => children.toList.bind (allHtmlTexts fuel)
  | fuel + 1, .element _ _ children => children.toList.bind (allHtmlTexts fuel)

private def allAttributes : Nat → Html → List (Array (String × Json))
  | 0, _ => []
  | _ + 1, .text _ => []
  | fuel + 1, .component _ _ _ children => children.toList.bind (allAttributes fuel)
  | fuel + 1, .element _ attributes children =>
      attributes :: children.toList.bind (allAttributes fuel)

private def attrJson? (attributes : Array (String × Json)) (name : String) : Option Json :=
  attributes.find? (·.1 == name) |>.map (·.2)

private def attrString? (attributes : Array (String × Json)) (name : String) : Option String :=
  match attrJson? attributes name with
  | some (.str value) => some value
  | _ => none

private def styleString? (attributes : Array (String × Json)) (name : String) : Option String :=
  match attrJson? attributes "style" with
  | some style => match style.getObjVal? name with
    | .ok (.str value) => some value
    | _ => none
  | none => none

private def attributesWith (name value : String) (html : Html) : List (Array (String × Json)) :=
  (allAttributes 30 html).filter fun attributes => attrString? attributes name == some value

private def rootTheme? (html : Html) : Option String :=
  (allAttributes 30 html).findSome? fun attributes =>
    attrString? attributes "data-sembla-theme"

private def academicHtml := compositionDiagramHtmlWithTheme .academic epidemicPolicyProps
private def editorHtml := compositionDiagramHtmlWithTheme .editor epidemicPolicyProps
private def notebookHtml := compositionDiagramHtmlWithTheme .notebook epidemicPolicyProps

#guard stylesAreObjects 30 (compositionDiagramHtml epidemicPolicyProps)
#guard rootTheme? academicHtml == some "academic"
#guard rootTheme? editorHtml == some "editor"
#guard rootTheme? notebookHtml == some "notebook"

private def diagramTexts := allHtmlTexts 30 academicHtml
private def wireAttributes := attributesWith "data-sembla-edge-kind" "wire" academicHtml
private def exposureAttributes := attributesWith "data-sembla-edge-kind" "exposure" academicHtml

#guard diagramTexts.contains "Population"
#guard diagramTexts.contains "Policy"
#guard (diagramTexts.filter (· == "1-tick delay")).length == 2
#guard (diagramTexts.filter (· == "zero-delay alias")).length == 2
#guard wireAttributes.length == 2
#guard exposureAttributes.length == 2
#guard wireAttributes.all fun attributes =>
  styleString? attributes "borderLeft" == some "3px solid var(--sembla-state-1)"
#guard exposureAttributes.all fun attributes =>
  styleString? attributes "borderLeft" == some "3px dashed var(--sembla-state-3)"

private def regionalHtml := compositionDiagramHtml regionalResponseProps
private def hiddenAttributes :=
  attributesWith "data-sembla-port-visibility" "hidden" regionalHtml

#guard (allHtmlTexts 30 regionalHtml).contains "Epidemic.Restriction modifier"
#guard hiddenAttributes.length == 1
#guard hiddenAttributes.all fun attributes =>
  styleString? attributes "textDecoration" == some "line-through"

end Sembla.Composition.WidgetTests
