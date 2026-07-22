import Sembla.Composition.Source

namespace Sembla.Composition

open Sembla

/-!
# Composition observation contract

This module freezes the V1 observation quotient required by the composition
architecture. Equality of `CompositionObservation` includes every field below:
leaf and mailbox state, external outputs, firing order, Philox draw coordinates,
and named scalar observations. In particular, draw coordinates are observable;
they are not quotiented away.

Canonical artifact hashes are consequences of these canonically represented
observations and their enclosing artifacts. Hashes are not themselves fields of
the observation contract. This follows the byte-level boundary recorded in
DECISIONS.md §J10.

The structures here define the eventual behavioral theorem's types. The full
source and plan observation functions are intentionally deferred in
`SpecStatements`; PRD 0010 proves only linker plan validity and executable-checks
the independent static fragment.
-/

/-- One canonical table. Columns and rows are in canonical artifact order, and
    every cell is its canonical textual artifact representation. -/
structure CanonicalTable where
  name : StableId
  columns : List String
  rows : List (List String)
deriving Repr, BEq

/-- The complete canonical table state at one leaf or mailbox observation point.
    Tables are ordered by stable table identity. -/
structure CanonicalTableState where
  tables : List CanonicalTable
deriving Repr, BEq

/-- The Philox counter tuple used by V1, in counter-word order. All four values
    are 32-bit words exactly as consumed by the runtime RNG. -/
structure DrawCoordinate where
  tick : UInt32
  ruleWord : UInt32
  entityId : UInt32
  drawIdx : UInt32
deriving Repr, BEq

/-- The complete V1 behavioral observation quotient. Every field participates
    in observational equivalence, including `drawCoordinates`. -/
structure CompositionObservation where
  leafState : StableId → Nat → CanonicalTableState
  mailboxState : StableId → Nat → CanonicalTableState
  externalOutputs : StableId → Nat → CanonicalTable
  fired : Nat → List StableId
  drawCoordinates : Nat → List DrawCoordinate
  observations : StableId → Nat → IR.Scientific

private instance : Inhabited CanonicalTable := ⟨{
  name := ⟨"observation:empty"⟩
  columns := []
  rows := [] }⟩

private instance : Inhabited CanonicalTableState := ⟨{ tables := [] }⟩

/-- Elaboration witness for the future opaque denotation signatures; it is not
    an implementation of either behavioral denotation. -/
instance : Inhabited CompositionObservation := ⟨{
  leafState := fun _ _ => default
  mailboxState := fun _ _ => default
  externalOutputs := fun _ _ => default
  fired := fun _ => []
  drawCoordinates := fun _ => []
  observations := fun _ _ => ⟨0, 0⟩ }⟩

end Sembla.Composition
