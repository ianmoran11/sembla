import Sembla.Demos.CanonicalModels
import Sembla.Demos.DeepIR
import Sembla.Demos.Modeling
import Sembla.Json

/-!
# Serialization and export-boundary feature tour

Every DSL model is an ordinary `Sembla.IR.Model`. `Sembla.IR.toJson` emits the
canonical, newline-terminated IR consumed by the Rust CLI. Exact decimal values
retain their represented digits, including scientific notation.

This module keeps output as values so normal builds stay quiet. In an editor,
try `#eval featureTourJson`; from `frontend/`, use
`lake exe sembla-export sir /tmp/sir.json` for the supported file-export path,
then validate with the Rust CLI.
-/

namespace Sembla.Demos.Serialization

open Sembla.Demos.CanonicalModels Sembla.Demos.DeepIR Sembla.Demos.Modeling

/-- Canonical JSON for the all-surface DSL tutorial model. -/
def featureTourJson : String := Sembla.IR.toJson featureTour

/-- All shipped canonical models paired with their serialized IR. -/
def canonicalJson : List (String × String) :=
  catalog.map fun (exportName, model) => (exportName, Sembla.IR.toJson model)

#guard featureTourJson.startsWith "{\"name\":\"lean_feature_tour\",\"dt\":0.25"
#guard featureTourJson.length > 100
#guard featureTourJson.endsWith "}\n"
#guard canonicalJson.length == 8
#guard canonicalJson.all (fun entry => !entry.2.isEmpty && entry.2.endsWith "}\n")
#guard numericJson.startsWith "{\"name\":\"direct_ir_numeric_gallery\",\"dt\":0.125"

end Sembla.Demos.Serialization
