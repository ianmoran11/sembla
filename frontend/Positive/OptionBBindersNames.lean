import Sembla.DSL

namespace Sembla.Positive.OptionBBindersNames
open Sembla.IR Sembla.DSL

/-- Exercises every frozen derivation mapping, an exact override, a forward Ref,
    and equal table runtime names in distinct box namespaces. -/
private def namesModel : Model := model% "option_b_names" step(1.0) where
  params [
    param β : ℝ := 0.1,
    param γ : ℝ := 0.1,
    param «λ» : ℝ := 0.1,
    param μ : ℝ := 0.1,
    param σ : ℝ := 0.1,
    param τ : ℝ := 0.1,
    param θ : ℝ := 0.1,
    param «λ_parent» : ℝ := 0.1,
    param PolicyController : ℝ := 0.1,
    param HTTPServer : ℝ := 0.1,
    param Person2D : ℝ := 0.1,
    param rate_v2 : ℝ := 0.1]
  boxes [
    box first where
      systems [
        system Person (rows := 2) where [ref employer : Employer],
        system PolicyController (rows := 1) where [],
        system HTTPServer (rows := 1) where [],
        system Person2D (rows := 1) where [],
        system internal_name2 (rows := 1) where [],
        system Employer (rows := 1) where [],
        system LegacyPerson (name := "Person") (rows := 4) where [],
        system «Legacy-Person» (name := "legacy_punctuation") (rows := 1) where [],
        system SharedSource (name := "shared") (rows := 1) where []]
      inputs []
      transitions []
      outputs [],
    box second where
      systems [system SharedReplica (name := "shared") (rows := 1) where []]
      inputs []
      transitions []
      outputs []]
  wires []

#guard namesModel.params.map (·.name) == [
  "beta", "gamma", "lambda", "mu", "sigma", "tau", "theta", "lambda_parent",
  "policy_controller", "http_server", "person2_d", "rate_v2"]

#guard namesModel.boxes.map (fun item => item.tables.map (·.name)) == [
  ["person", "policy_controller", "http_server", "person2_d", "internal_name2",
    "employer", "Person", "legacy_punctuation", "shared"],
  ["shared"]]

private def forwardRefType? : Option AttrType := do
  let firstBox ← namesModel.boxes.head?
  let person ← firstBox.tables.head?
  let employer ← person.attrs.head?
  pure employer.ty

#guard forwardRefType? == some (.ref "employer")

end Sembla.Positive.OptionBBindersNames
