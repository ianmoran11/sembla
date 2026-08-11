import Sembla.Tutorial.Step01_Recovery
import Sembla.Tutorial.Step02_Parameters
import Sembla.Tutorial.Step03_Workplaces
import Sembla.Tutorial.Step04_Observations
import Sembla.Tutorial.Step05_PolicyFeedback
import Sembla.Tutorial.Step06_InspectAndExport
import Sembla.Tutorial.Step07_LumpingProof
import Sembla.Tutorial.Step08_IndexedFamilies
import Sembla.Tutorial.Step09_MathematicalSurface

/-!
# Progressive workplace-SIR tutorial

Read the modules in filename order:

1. `Step01_Recovery` — one state machine and one transition;
2. `Step02_Parameters` — parameters, priors, and infection;
3. `Step03_Workplaces` — references and grouped interaction;
4. `Step04_Observations` — views and summaries;
5. `Step05_PolicyFeedback` — boxes, ports, outputs, and wires;
6. `Step06_InspectAndExport` — widgets and canonical JSON; and
7. `Step07_LumpingProof` — the specification-level rewrite theorem;
8. `Step08_IndexedFamilies` — finite parameter tables and statically expanded transitions; and
9. `Step09_MathematicalSurface` — a standalone advanced detour through named
   domains, partitions, functions, aliases, and relations.

The first eight models are complete and independently buildable, so readers can
compare adjacent workplace-SIR steps directly. Step 9 is explicitly standalone
and does not pretend to extend that model state. Lean elaborates the DSL and renders structural
information; the Rust CLI performs whole-model validation and execution after
export.
-/
