import Sembla.Models

namespace Sembla.CanonicalModelsTests
open Sembla.IR Sembla.Models

private def canonicalModels : List Model := [
  sir,
  observations,
  sirPolicy,
  reversibleCtmc,
  radioactiveDecayChain,
  sisImportation,
  seirsWaning,
  noisyVoter
]

#guard canonicalModels.length == 8
#guard canonicalModels.map (·.name) == [
  "sir_workplace_frequency_dependent",
  "observations",
  "sir_workplace_policy_feedback",
  "reversible_two_state_ctmc",
  "radioactive_decay_chain",
  "sis_with_importation",
  "seirs_with_waning_immunity",
  "noisy_voter_mean_field"
]
#guard canonicalModels.map (fun model => model.params.map (·.name)) == [
  ["beta", "gamma"],
  [],
  ["beta", "gamma"],
  ["rate_ab", "rate_ba"],
  ["lambda_parent", "lambda_daughter"],
  ["import_rate", "beta", "gamma"],
  ["import_rate", "beta", "sigma", "gamma", "omega"],
  ["mutation_rate", "imitation_rate"]
]
#guard canonicalModels.map (fun model => model.boxes.map (·.name)) == [
  ["sir"],
  ["population"],
  ["population", "policy"],
  ["chain"],
  ["decay"],
  ["epidemic"],
  ["epidemic"],
  ["opinions"]
]
#guard canonicalModels.map (fun model =>
    model.boxes.map (fun modelBox => modelBox.tables.map (·.name))) == [
  [["person", "employer"]],
  [["Person"]],
  [["person", "employer"], ["controller"]],
  [["particle"]],
  [["atom"]],
  [["person", "community"]],
  [["person", "community"]],
  [["agent", "community"]]
]
#guard canonicalModels.map (fun model =>
    model.boxes.map (fun modelBox => modelBox.transitions.map (·.name))) == [
  [["infect", "recover"]],
  [[]],
  [["infect", "recover"], ["restrict", "reopen"]],
  [["move_ab", "move_ba"]],
  [["parent_decay", "daughter_decay"]],
  [["infect", "recover"]],
  [["expose", "progress", "recover", "wane"]],
  [["adopt_b", "adopt_a"]]
]

end Sembla.CanonicalModelsTests
