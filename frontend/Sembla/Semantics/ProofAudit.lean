import Lean.Elab.Command
import Lean.Meta.Basic
import Lean.Util.CollectAxioms

/-!
A deterministic environment audit for the Lean IR formalization track.

`#audit_proofs` receives module/namespace roots. The repository proof-hygiene
script imports every covered source module before invoking it, so private
module declarations cannot escape the inventory through generated names.
-/
namespace Sembla.Semantics.ProofAudit

open Lean Elab Command

private def nameLess (left right : Name) : Bool :=
  left.toString < right.toString

private def underAnyRoot (roots : Array Name) (name : Name) : Bool :=
  roots.any fun root => root.isPrefixOf name

private def declarationModule? (env : Environment) (name : Name) : Option Name := do
  let moduleIndex ← env.getModuleIdxFor? name
  env.header.moduleNames[moduleIndex.toNat]?

private def isCovered (env : Environment) (roots : Array Name) (name : Name) : Bool :=
  underAnyRoot roots name ||
    (declarationModule? env name).any (underAnyRoot roots)

private def allowedAxiom (name : Name) : Bool :=
  name == ``propext || name == ``Classical.choice || name == ``Quot.sound

private def renderNames (names : Array Name) : String :=
  if names.isEmpty then
    "none"
  else
    String.intercalate ", " (names.toList.map Name.toString)

private def auditCoveredProofs (roots : Array Name) : CommandElabM Unit := do
  let env ← getEnv
  let mut declarations : Array (Name × ConstantInfo) := #[]
  for (name, info) in env.constants do
    if isCovered env roots name then
      declarations := declarations.push (name, info)
  declarations := declarations.qsort fun left right => nameLess left.1 right.1

  let mut theoremCount := 0
  let mut opaqueDataCount := 0
  let mut failures : Array String := #[]
  for (name, info) in declarations do
    match info with
    | .thmInfo _ =>
        theoremCount := theoremCount + 1
        let axioms := (← Lean.collectAxioms name).qsort nameLess
        logInfo m!"proof-audit theorem {name}: axioms [{renderNames axioms}]"
        let rejected := axioms.filter fun axiomName => !allowedAxiom axiomName
        unless rejected.isEmpty do
          failures := failures.push
            s!"theorem {name} depends on unapproved axioms: {renderNames rejected}"
    | .axiomInfo _ =>
        -- The compiler emits module-owned axiomInfo specializations for meta
        -- code. Source scanning bans explicit axiom syntax, while public
        -- declarations under a covered root are rejected here as a backstop.
        if underAnyRoot roots name then
          failures := failures.push s!"explicit axiom declaration in covered namespace: {name}"
    | .opaqueInfo value =>
        let proposition ← liftTermElabM do Lean.Meta.isProp value.type
        if proposition then
          failures := failures.push s!"opaque proposition in covered module: {name}"
        else
          opaqueDataCount := opaqueDataCount + 1
          logInfo m!"proof-audit opaque-data {name}"
    | _ => pure ()

  if failures.isEmpty then
    logInfo m!"proof-audit passed: {theoremCount} theorem/lemma declarations; {opaqueDataCount} opaque data declarations; roots [{renderNames roots}]"
  else
    throwError m!"proof audit failed:\n{String.intercalate "\n" failures.toList}"

/--
Enumerate every theorem/lemma in the given namespace or module roots and reject
transitive axioms other than `propext`, `Classical.choice`, and `Quot.sound`.
Covered explicit axioms and proposition-valued opaque declarations also fail.
-/
syntax (name := auditProofs) "#audit_proofs" ident+ : command

elab_rules : command
  | `(#audit_proofs $roots:ident*) =>
      auditCoveredProofs (roots.map Syntax.getId)

end Sembla.Semantics.ProofAudit
