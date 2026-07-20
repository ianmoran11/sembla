import Sembla.Models

/-!
# Canonical scientific model catalog

Sembla ships eight Lean-authored models. They cover a workplace SIR model,
all observation reductions, two-box feedback, reversible CTMC dynamics,
a radioactive decay chain, importation, waning immunity, and a noisy voter
system. This module makes the catalog discoverable and checks its exported
names and selected structural contracts.

From `frontend/`, export any entry with `lake exe sembla-export <name> <path>`.
Execution remains a Rust/CUDA-runtime responsibility after export.
-/

namespace Sembla.Demos.CanonicalModels

open Sembla.IR Sembla.Models

/-- Friendly export name paired with its elaborated model value. -/
def catalog : List (String × Model) :=
  [ ("sir", sir)
  , ("observations", observations)
  , ("sir_policy", sirPolicy)
  , ("reversible_ctmc", reversibleCtmc)
  , ("radioactive_decay_chain", radioactiveDecayChain)
  , ("sis_importation", sisImportation)
  , ("seirs_waning", seirsWaning)
  , ("noisy_voter", noisyVoter)
  ]

#guard catalog.map (·.1) ==
  [ "sir"
  , "observations"
  , "sir_policy"
  , "reversible_ctmc"
  , "radioactive_decay_chain"
  , "sis_importation"
  , "seirs_waning"
  , "noisy_voter"
  ]

#guard catalog.map (·.2.name) ==
  [ "sir_workplace_frequency_dependent"
  , "observations"
  , "sir_workplace_policy_feedback"
  , "reversible_two_state_ctmc"
  , "radioactive_decay_chain"
  , "sis_with_importation"
  , "seirs_with_waning_immunity"
  , "noisy_voter_mean_field"
  ]

#guard observations.boxes.map (fun modelBox => modelBox.views.map (·.reduce)) ==
  [[ViewReduce.sum, .count, .min, .max]]
#guard sirPolicy.boxes.map (·.name) == ["population", "policy"]
#guard sirPolicy.wires.length == 2
#guard sir.summaries.map (·.reduce) == [SummaryReduce.max, .argmaxTick]

end Sembla.Demos.CanonicalModels
