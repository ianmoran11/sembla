import Sembla.Frontend.Builders.Core

/-!
Executable builder fixtures for the complete declaration-only core fragment.
Every negative fixture contains one defect and checks a stable category/path.
-/
namespace Sembla.Frontend.Builders.CoreTests

open Sembla
open Sembla.Frontend.Builders
open Sembla.Semantics

private instance {ε α : Type} [BEq ε] [BEq α] : BEq (Except ε α) where
  beq left right :=
    match left, right with
    | .ok left, .ok right => left == right
    | .error left, .error right => left == right
    | _, _ => false

private def sci (coefficient exponent : Int) : IR.Scientific :=
  ⟨coefficient, exponent⟩

private def attr (name : String) (ty : IR.AttrType) : IR.Attr :=
  { name := name, ty := ty }

private def table (name : String) (sizeHint : Nat := 1)
    (attrs : List IR.Attr := []) : IR.Table :=
  { name := name, sizeHint := sizeHint, attrs := attrs }

private def realParam (name : String) (default : IR.Scientific)
    (prior : Option IR.Prior := none) : IR.ParamDecl :=
  { name := name, ty := .real, default := .real default, prior := prior }

private def intParam (name : String) (default : Int) : IR.ParamDecl :=
  { name := name, ty := .int, default := .int default, prior := none }

private def uniformPrior : IR.Prior :=
  { family := .uniform, args := [sci 100 (-2), sci 250 (-2)] }

private def normalPrior : IR.Prior :=
  { family := .normal, args := [sci (-30) (-1), sci 450 (-2)] }

private def logNormalPrior : IR.Prior :=
  { family := .logNormal, args := [sci (-2231435513142097) (-16), sci 25 (-2)] }

private def tableA : IR.Table :=
  table "A" 9
    [ attr "rate" .real
    , attr "count" .int
    , attr "status" (.enum ["zeta", "alpha"])
    , attr "forward" (.ref "B")
    , attr "self" (.ref "A") ]

private def tableB : IR.Table :=
  table "B" 0
    [ attr "backward" (.ref "A")
    , attr "self" (.ref "B") ]

private def positiveBox : CoreBoxShell :=
  { name := "network", tables := [tableA, tableB] }

private def emptyTableBox : CoreBoxShell :=
  { name := "empty-table", tables := [table "Zero" 0 []] }

private def zeroTableBox : CoreBoxShell :=
  { name := "zero-table", tables := [] }

private def positiveShell : CoreModelShell :=
  { name := "core-fixture"
    dt := sci 250 (-3)
    params :=
      [ realParam "plain-real" (sci 1200 (-3))
      , realParam "uniform" (sci 100 (-2)) (some uniformPrior)
      , realParam "normal" (sci (-300) (-2)) (some normalPrior)
      , realParam "log-normal" (sci 800 (-3)) (some logNormalPrior)
      , intParam "plain-int" (-7) ]
    boxes := [positiveBox, emptyTableBox, zeroTableBox] }

private def positiveRaw : IR.Model := positiveShell.toRaw

/-! Direct prior and parameter construction, including all accepted families. -/
#guard buildPrior .uniform [sci 100 (-2), sci 250 (-2)] == .ok uniformPrior
#guard buildPrior .normal [sci (-30) (-1), sci 450 (-2)] == .ok normalPrior
#guard buildPrior .logNormal
  [sci (-2231435513142097) (-16), sci 25 (-2)] == .ok logNormalPrior
#guard buildParameter "plain-real" .real (.real (sci 1200 (-3))) ==
  .ok (realParam "plain-real" (sci 1200 (-3)))
#guard buildParameter "uniform" .real (.real (sci 100 (-2))) (some uniformPrior) ==
  .ok (realParam "uniform" (sci 100 (-2)) (some uniformPrior))
#guard buildParameter "normal" .real (.real (sci (-300) (-2))) (some normalPrior) ==
  .ok (realParam "normal" (sci (-300) (-2)) (some normalPrior))
#guard buildParameter "log-normal" .real (.real (sci 800 (-3)))
  (some logNormalPrior) ==
  .ok (realParam "log-normal" (sci 800 (-3)) (some logNormalPrior))
#guard buildParameter "plain-int" .int (.int (-7)) == .ok (intParam "plain-int" (-7))
#guard buildAttribute ["A", "B"] "rate" .real == .ok (attr "rate" .real)
#guard buildAttribute ["A", "B"] "count" .int == .ok (attr "count" .int)
#guard buildAttribute ["A", "B"] "status" (.enum ["zeta", "alpha"]) ==
  .ok (attr "status" (.enum ["zeta", "alpha"]))
#guard buildAttribute ["A", "B"] "target" (.ref "B") ==
  .ok (attr "target" (.ref "B"))

/-! Complete-catalog table and shell construction preserves every raw field. -/
#guard buildTable ["A", "B"] tableA.name tableA.sizeHint tableA.attrs == .ok tableA
#guard buildTable ["A", "B"] tableB.name tableB.sizeHint tableB.attrs == .ok tableB
#guard buildBoxShell positiveBox == .ok positiveBox.toRaw
#guard buildBoxShell emptyTableBox == .ok emptyTableBox.toRaw
#guard buildBoxShell zeroTableBox == .ok zeroTableBox.toRaw
#guard buildModelShell positiveShell == .ok positiveRaw

private def emptyShell : CoreModelShell :=
  { name := "empty", dt := sci 100 (-3), params := [], boxes := [] }

#guard buildModelShell emptyShell == .ok emptyShell.toRaw
#guard positiveRaw.dt == sci 250 (-3)
#guard positiveRaw.params.map IR.ParamDecl.name ==
  ["plain-real", "uniform", "normal", "log-normal", "plain-int"]
#guard positiveRaw.boxes.map IR.Box.name == ["network", "empty-table", "zero-table"]
private def positiveFirstTablesPreserved : Bool :=
  match positiveRaw.boxes with
  | first :: _ => first.tables == [tableA, tableB]
  | [] => false

#guard positiveFirstTablesPreserved
#guard tableA.attrs.map IR.Attr.name == ["rate", "count", "status", "forward", "self"]
#guard tableB.sizeHint == 0
#guard positiveRaw.wires.isEmpty && positiveRaw.summaries.isEmpty
#guard positiveRaw.boxes.all fun box =>
  box.transitions.isEmpty && box.inputs.isEmpty && box.outputs.isEmpty &&
    box.views.isEmpty && box.groupedViews.isEmpty

private def bridgeShell : CoreModelShell :=
  { name := "theorem-bridge"
    dt := sci 1 (-1)
    params := [intParam "count" 2]
    boxes := [emptyTableBox, zeroTableBox] }

private theorem bridgeWellFormed : DeclarationsWellFormed bridgeShell.toRaw := by decide

private theorem bridgeBuilt : buildModelShell bridgeShell = .ok bridgeShell.toRaw :=
  buildModelShell_complete bridgeWellFormed

example : ∃ ctx, checkDeclarations bridgeShell.toRaw = .ok ctx :=
  buildModelShell_declaration_acceptance bridgeBuilt

example : ∃ checked, checkModel bridgeShell.toRaw = .ok checked ∧
    checked.erase = bridgeShell.toRaw :=
  buildModelShell_model_acceptance_and_erasure bridgeBuilt

#guard (checkDeclarations positiveRaw).isOk
#guard (checkModel positiveRaw).isOk

private def positiveCheckedErasesExactly : Bool :=
  match checkModel positiveRaw with
  | .ok checked => checked.erase == positiveRaw
  | .error _ => false

#guard positiveCheckedErasesExactly

/-! One-defect structured rejection fixtures. -/
private def hasError {α : Type} (result : Except CoreBuilderError α)
    (category : CoreBuilderErrorCategory)
    (path : List CoreBuilderPathSegment) : Bool :=
  match result with
  | .ok _ => false
  | .error issue => issue.category == category && issue.path == path

private def withDt (dt : IR.Scientific) : CoreModelShell :=
  { positiveShell with dt := dt }

#guard hasError (buildModelShell (withDt (sci 0 (-99))))
  .nonpositiveDt [.modelMetadata, .dt]
#guard hasError (buildModelShell (withDt (sci (-1) 40)))
  .nonpositiveDt [.modelMetadata, .dt]

private def duplicateParameterShell : CoreModelShell :=
  { positiveShell with
    params := [realParam "same" (sci 1 0), intParam "same" 1] }

#guard hasError (buildModelShell duplicateParameterShell)
  .duplicateParameterName [.parameter 1, .name]

private def duplicateBoxShell : CoreModelShell :=
  { positiveShell with boxes := [{ name := "same", tables := [] },
    { name := "same", tables := [] }] }

#guard hasError (buildModelShell duplicateBoxShell)
  .duplicateBoxName [.box 1, .name]

private def duplicateTableBox : CoreBoxShell :=
  { name := "duplicate-tables", tables := [table "T", table "T"] }

#guard hasError (buildBoxShell duplicateTableBox)
  .duplicateTableName [.table 1, .name]

#guard hasError (buildTable ["T"] "T" 1 [attr "same" .real, attr "same" .int])
  .duplicateAttributeName [.table 0, .attribute 1, .name]

#guard hasError (buildParameter "bad-real" .real (.int 1))
  .parameterDefaultTypeMismatch [.parameter 0, .defaultValue]
#guard hasError (buildParameter "bad-int" .int (.real (sci 1 0)))
  .parameterDefaultTypeMismatch [.parameter 0, .defaultValue]
#guard hasError (buildParameter "int-prior" .int (.int 1) (some normalPrior))
  .integerPrior [.parameter 0, .prior]

#guard hasError (buildPrior .normal [sci 1 0])
  .invalidPriorArity [.prior]
#guard hasError (buildPrior .logNormal [sci 1 0, sci 2 0, sci 3 0])
  .invalidPriorArity [.prior]
#guard hasError (buildPrior .uniform [sci 10 (-1), sci 1 0])
  .unorderedUniformBounds [.prior, .priorArgument 1]
#guard hasError (buildPrior .uniform [sci 2 0, sci 1 0])
  .unorderedUniformBounds [.prior, .priorArgument 1]

#guard hasError (buildTable ["T"] "T" 1 [attr "kind" (.enum [])])
  .emptyEnum [.table 0, .attribute 0, .enumVariant 0]
#guard hasError (buildTable ["T"] "T" 1
  [attr "kind" (.enum ["first", "second", "first"])])
  .duplicateEnumVariant [.table 0, .attribute 0, .enumVariant 2]
#guard hasError (buildTable ["T"] "T" 1 [attr "missing" (.ref "Absent")])
  .unresolvedTableReference [.table 0, .attribute 0, .tableReference]

/-! Model-level paths retain enclosing source indices. -/
private def badNestedBox : CoreBoxShell :=
  { name := "bad", tables := [table "T" 1 [attr "target" (.ref "Missing")]] }

private def badNestedShell : CoreModelShell :=
  { name := "bad-nested", dt := sci 1 0, params := [], boxes := [zeroTableBox, badNestedBox] }

#guard hasError (buildModelShell badNestedShell)
  .unresolvedTableReference [.box 1, .table 0, .attribute 0, .tableReference]

end Sembla.Frontend.Builders.CoreTests
