import Sembla.Models.AustralianPopulation.Surface

/-! Compatibility count projection over the declaratively authored transitions. -/

namespace Sembla.Models.AustralianPopulation

private def allRules :=
  Sembla.Models.australianPopulationDeclarative.boxes.bind (·.transitions)

def transitionCountsHold : Bool :=
  allRules.length == 418 &&
    (allRules.drop 2 |>.take 56).length == 56 &&
    (allRules.drop 58 |>.take 336).length == 336

end Sembla.Models.AustralianPopulation
