import Sembla.Models.AustralianPopulation.Surface

/-!
Compatibility projection for callers of `AustralianPopulation.parameters`.
The authored declarations and pinned generated tables in `Surface.lean` are the
production authority.
-/

namespace Sembla.Models.AustralianPopulation

open Sembla.IR

def parameters : List ParamDecl := Sembla.Models.australianPopulationDeclarative.params

end Sembla.Models.AustralianPopulation
