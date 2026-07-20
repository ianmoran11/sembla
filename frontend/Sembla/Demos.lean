import Sembla.Demos.Modeling
import Sembla.Demos.DeepIR
import Sembla.Demos.CanonicalModels
import Sembla.Demos.Serialization
import Sembla.Demos.Widgets
import Sembla.Demos.Proofs

/-!
# Sembla Lean feature demonstrations

This root imports a curated, build-checked tutorial suite:

* `Sembla.Demos.Modeling` — the complete supported `model%` authoring surface;
* `Sembla.Demos.DeepIR` — lower-level IR data, expressions, priors, and contests;
* `Sembla.Demos.CanonicalModels` — all eight shipped scientific models;
* `Sembla.Demos.Serialization` — exact JSON and the Rust export boundary;
* `Sembla.Demos.Widgets` — pure props, infoview HTML, and all themes; and
* `Sembla.Demos.Proofs` — executable lumping fixtures and the proved rewrite.

The suite deliberately does not claim that Lean executes the Rust oracle,
CUDA backend, sweeps, or NPE calibration. Lean elaborates and type-checks the
DSL into ordinary IR data, renders structure widgets, and proves the
specification-level target 1a; whole-model runtime validation and execution
happen after export in Rust, and the other capabilities consume that validated
IR outside Lean. Compile-time error
examples live under `frontend/Negative/` and run via
`bash frontend/scripts/test-negative.sh`.
-/
