import Sembla.Frontend.Builders.Core

/-!
Pure, syntax-independent builders for transition expressions, effects, claims,
transition payloads, and source-ordinal overlays on accepted core shells.

All semantic resolution delegates to the PRD 0005/0006 declaration, term, and
whole-model checkers. Raw constructors retain their arguments exactly.
-/
namespace Sembla.Frontend.Builders

open Sembla
open Sembla.Semantics

/-! ## Exact raw constructors -/

namespace TransitionRaw

def real (value : IR.Scientific) : IR.Expr := .real value
def int (value : Int) : IR.Expr := .int value
def bool (value : Bool) : IR.Expr := .bool value
def enum (variant : String) : IR.Expr := .enum variant
def parameter (name : String) : IR.Expr := .param name
def selfAttribute (name : String) : IR.Expr := .selfAttr name
def add (lhs rhs : IR.Expr) : IR.Expr := .add lhs rhs
def sub (lhs rhs : IR.Expr) : IR.Expr := .sub lhs rhs
def mul (lhs rhs : IR.Expr) : IR.Expr := .mul lhs rhs
def div (lhs rhs : IR.Expr) : IR.Expr := .div lhs rhs
def eq (lhs rhs : IR.Expr) : IR.Expr := .eq lhs rhs
def ne (lhs rhs : IR.Expr) : IR.Expr := .ne lhs rhs
def lt (lhs rhs : IR.Expr) : IR.Expr := .lt lhs rhs
def le (lhs rhs : IR.Expr) : IR.Expr := .le lhs rhs
def gt (lhs rhs : IR.Expr) : IR.Expr := .gt lhs rhs
def ge (lhs rhs : IR.Expr) : IR.Expr := .ge lhs rhs
def and (lhs rhs : IR.Expr) : IR.Expr := .and lhs rhs
def or (lhs rhs : IR.Expr) : IR.Expr := .or lhs rhs
def not (value : IR.Expr) : IR.Expr := .not value
def enumIs (attributeName variant : String) : IR.Expr := .enumIs attributeName variant
def input (port : String) (aggregate : IR.Aggregate) : IR.Expr := .input port aggregate
def relatedAggregate (op : IR.AggOp) (table foreignKey selfKey : String)
    (filter : IR.Expr) : IR.Expr := .agg op table foreignKey selfKey filter

def count : IR.AggOp := .count
def sum (value : IR.Expr) : IR.AggOp := .sum value
def aggregate (op : IR.AggOp) (filter : Option IR.Expr := none) : IR.Aggregate :=
  .mk op filter

def setAttribute (destination : String) (value : IR.Expr) : IR.Effect :=
  .setAttr destination value

def raceClaim (resource : IR.Expr) : IR.ResourceClaim :=
  { resource := resource, ordering := .raceTime }

def keyClaim (resource key : IR.Expr) : IR.ResourceClaim :=
  { resource := resource, ordering := .key key }

def transition (name table : String) (guard hazard : IR.Expr)
    (effects : List IR.Effect) (claims : List IR.ResourceClaim) : IR.Transition :=
  { name := name, table := table, guard := guard, hazard := hazard
    effects := effects, contests := claims }

@[simp] theorem real_exact (value) : real value = .real value := rfl
@[simp] theorem int_exact (value) : int value = .int value := rfl
@[simp] theorem bool_exact (value) : bool value = .bool value := rfl
@[simp] theorem enum_exact (variant) : enum variant = .enum variant := rfl
@[simp] theorem parameter_exact (name) : parameter name = .param name := rfl
@[simp] theorem selfAttribute_exact (name) :
    selfAttribute name = .selfAttr name := rfl
@[simp] theorem add_exact (lhs rhs) : add lhs rhs = .add lhs rhs := rfl
@[simp] theorem sub_exact (lhs rhs) : sub lhs rhs = .sub lhs rhs := rfl
@[simp] theorem mul_exact (lhs rhs) : mul lhs rhs = .mul lhs rhs := rfl
@[simp] theorem div_exact (lhs rhs) : div lhs rhs = .div lhs rhs := rfl
@[simp] theorem eq_exact (lhs rhs) : eq lhs rhs = .eq lhs rhs := rfl
@[simp] theorem ne_exact (lhs rhs) : ne lhs rhs = .ne lhs rhs := rfl
@[simp] theorem lt_exact (lhs rhs) : lt lhs rhs = .lt lhs rhs := rfl
@[simp] theorem le_exact (lhs rhs) : le lhs rhs = .le lhs rhs := rfl
@[simp] theorem gt_exact (lhs rhs) : gt lhs rhs = .gt lhs rhs := rfl
@[simp] theorem ge_exact (lhs rhs) : ge lhs rhs = .ge lhs rhs := rfl
@[simp] theorem and_exact (lhs rhs) : and lhs rhs = .and lhs rhs := rfl
@[simp] theorem or_exact (lhs rhs) : or lhs rhs = .or lhs rhs := rfl
@[simp] theorem not_exact (value) : not value = .not value := rfl
@[simp] theorem enumIs_exact (attributeName variant) :
    enumIs attributeName variant = .enumIs attributeName variant := rfl
@[simp] theorem input_exact (port aggregate) :
    input port aggregate = .input port aggregate := rfl
@[simp] theorem relatedAggregate_exact (op table foreignKey selfKey filter) :
    relatedAggregate op table foreignKey selfKey filter =
      .agg op table foreignKey selfKey filter := rfl
@[simp] theorem count_exact : count = .count := rfl
@[simp] theorem sum_exact (value) : sum value = .sum value := rfl
@[simp] theorem aggregate_exact (op filter) :
    aggregate op filter = .mk op filter := rfl
@[simp] theorem setAttribute_exact (destination value) :
    setAttribute destination value = .setAttr destination value := rfl
@[simp] theorem raceClaim_exact (resource) :
    raceClaim resource = { resource := resource, ordering := .raceTime } := rfl
@[simp] theorem raceClaim_resource (resource) :
    (raceClaim resource).resource = resource := rfl
@[simp] theorem raceClaim_ordering (resource) :
    (raceClaim resource).ordering = .raceTime := rfl
@[simp] theorem keyClaim_exact (resource key) :
    keyClaim resource key = { resource := resource, ordering := .key key } := rfl
@[simp] theorem keyClaim_resource (resource key) :
    (keyClaim resource key).resource = resource := rfl
@[simp] theorem keyClaim_ordering (resource key) :
    (keyClaim resource key).ordering = .key key := rfl
@[simp] theorem transition_exact (name table guard hazard effects claims) :
    transition name table guard hazard effects claims =
      { name := name, table := table, guard := guard, hazard := hazard
        effects := effects, contests := claims } := rfl
@[simp] theorem transition_name (name table guard hazard effects claims) :
    (transition name table guard hazard effects claims).name = name := rfl
@[simp] theorem transition_table (name table guard hazard effects claims) :
    (transition name table guard hazard effects claims).table = table := rfl
@[simp] theorem transition_guard (name table guard hazard effects claims) :
    (transition name table guard hazard effects claims).guard = guard := rfl
@[simp] theorem transition_hazard (name table guard hazard effects claims) :
    (transition name table guard hazard effects claims).hazard = hazard := rfl
@[simp] theorem transition_effects (name table guard hazard effects claims) :
    (transition name table guard hazard effects claims).effects = effects := rfl
@[simp] theorem transition_claims (name table guard hazard effects claims) :
    (transition name table guard hazard effects claims).contests = claims := rfl

end TransitionRaw

/-! ## Intrinsically typed component constructors -/

namespace TransitionTyped

/-- Construct an assignment whose value is indexed by its exact destination
sort. -/
def effect (Γ : TermContext)
    (destination : AttributeId (Γ.model.schemaFor Γ.current))
    (value : Term Γ.model Γ.current Γ.inputs
      ((Γ.model.schemaFor Γ.current).attributeSort destination)) :
    Effect Γ.model Γ.current Γ.inputs :=
  .setAttr destination value

/-- Construct a current-surface race-time claim from an intrinsically resolved
reference resource. -/
def raceClaim (Γ : TermContext) (target : TableTarget Γ.model.catalog)
    (resource : Term Γ.model Γ.current Γ.inputs (.ref target)) :
    ResourceClaim Γ.model Γ.current Γ.inputs :=
  { resourceTarget := target
    resource := resource
    orderingDomain := .real
    orderingAvailability := .surfaceProduced
    ordering := .raceTime }

/-- Construct a raw-checkable key claim in any accepted Real, Int, or
owner-indexed Enum ordering domain. -/
def keyClaim (Γ : TermContext) (target : TableTarget Γ.model.catalog)
    (resource : Term Γ.model Γ.current Γ.inputs (.ref target))
    (domain : OrderingDomain Γ.model)
    (key : Term Γ.model Γ.current Γ.inputs domain.scalarSort) :
    ResourceClaim Γ.model Γ.current Γ.inputs :=
  { resourceTarget := target
    resource := resource
    orderingDomain := domain
    orderingAvailability := .rawCheckable
    ordering := .key domain key }

@[simp] theorem effect_erase_exact (Γ : TermContext)
    (destination : AttributeId (Γ.model.schemaFor Γ.current))
    (value : Term Γ.model Γ.current Γ.inputs
      ((Γ.model.schemaFor Γ.current).attributeSort destination)) :
    (effect Γ destination value).erase =
      .setAttr ((Γ.model.schemaFor Γ.current).attributeName destination) value.erase := rfl

@[simp] theorem raceClaim_erase_exact (Γ : TermContext)
    (target : TableTarget Γ.model.catalog)
    (resource : Term Γ.model Γ.current Γ.inputs (.ref target)) :
    (raceClaim Γ target resource).erase =
      { resource := resource.erase, ordering := .raceTime } := rfl

@[simp] theorem keyClaim_erase_exact (Γ : TermContext)
    (target : TableTarget Γ.model.catalog)
    (resource : Term Γ.model Γ.current Γ.inputs (.ref target))
    (domain : OrderingDomain Γ.model)
    (key : Term Γ.model Γ.current Γ.inputs domain.scalarSort) :
    (keyClaim Γ target resource domain key).erase =
      { resource := resource.erase, ordering := .key key.erase } := rfl

end TransitionTyped

/-! ## Stable builder failures -/

/-- Builder-owned source locations. Nested checker errors retain their own exact
checker paths; this path is used only for frontend-only restrictions. -/
inductive TransitionBuilderPathSegment where
  | box (index : Nat)
  | transition (index : Nat)
  | targetTable
  | guard
  | hazard
  | effect (index : Nat)
  | destination
  | effectValue
  | claim (index : Nat)
  | resource
  | orderingKey
  deriving Repr, BEq, DecidableEq

/-- Exact nested failures from the accepted builders/checkers plus the one
current-surface restriction owned by this slice. -/
inductive TransitionBuilderError where
  | core (error : CoreBuilderError)
  | declaration (error : CheckError)
  | term (error : TermCheckError)
  | modelCheck (error : ModelCheckError)
  | unsupportedSurfaceKeyOrdering (path : List TransitionBuilderPathSegment)
  deriving Repr, BEq

private def liftTermResult {α : Type} :
    Except TermCheckError α → Except TransitionBuilderError α
  | .ok value => .ok value
  | .error error => .error (.term error)

private def liftModelResult :
    Except ModelCheckError Checked.Model → Except TransitionBuilderError Checked.Model
  | .ok checked => .ok checked
  | .error (.declaration error) => .error (.declaration error)
  | .error (.model error) => .error (.modelCheck (.model error))

@[simp] private theorem liftTermResult_ok_iff {α : Type}
    (result : Except TermCheckError α) (value : α) :
    liftTermResult result = .ok value ↔ result = .ok value := by
  cases result <;> simp [liftTermResult]

@[simp] private theorem liftTermResult_error_iff {α : Type}
    (result : Except TermCheckError α) (error : TermCheckError) :
    liftTermResult result = .error (.term error) ↔ result = .error error := by
  cases result <;> simp [liftTermResult]

@[simp] private theorem liftModelResult_ok_iff
    (result : Except ModelCheckError Checked.Model) (checked : Checked.Model) :
    liftModelResult result = .ok checked ↔ result = .ok checked := by
  cases result with
  | ok value => simp [liftModelResult]
  | error error => cases error <;> simp [liftModelResult]

/-! ## Context-parametric checker adapters -/

/-- Synthesize any PRD 0006-valid raw expression in the authoritative context. -/
def buildSynthExpr (Γ : TermContext)
    (scope : RowScope Γ.model Γ.current Γ.inputs) (raw : IR.Expr)
    (path : List ModelCheckPathSegment := []) :
    Except TransitionBuilderError (CheckedExprResult Γ scope) :=
  liftTermResult (synthExpr Γ scope raw path)

/-- Check any raw expression against an authoritative expected sort/origin. -/
def buildExpectedExpr (Γ : TermContext)
    (scope : RowScope Γ.model Γ.current Γ.inputs) (raw : IR.Expr)
    (expected : ScalarSort Γ.model.catalog) (origin : SortOrigin Γ scope expected)
    (path : List ModelCheckPathSegment := []) :
    Except TransitionBuilderError
      (Expr Γ.model Γ.current Γ.inputs scope expected) :=
  liftTermResult (checkExpr Γ scope raw expected origin path)

/-- Check one exact raw assignment. Complete soundness is exposed through the
public complete-transition theorem because the component theorem is private in
PRD 0006. -/
def buildEffect (Γ : TermContext) (raw : IR.Effect)
    (path : List ModelCheckPathSegment := []) :
    Except TransitionBuilderError (Effect Γ.model Γ.current Γ.inputs) :=
  liftTermResult (checkEffect Γ raw path)

/-- Check one exact raw claim, including raw-only Real/Int/Enum key orderings. -/
def buildClaim (Γ : TermContext) (raw : IR.ResourceClaim)
    (path : List ModelCheckPathSegment := []) :
    Except TransitionBuilderError (ResourceClaim Γ.model Γ.current Γ.inputs) :=
  liftTermResult (checkClaim Γ raw path)

/-- Check one complete transition payload in the supplied authoritative context. -/
def buildTransition (Γ : TermContext) (raw : IR.Transition)
    (path : List ModelCheckPathSegment := []) :
    Except TransitionBuilderError (TransitionTerms Γ.model Γ.current Γ.inputs) :=
  liftTermResult (checkTransitionTerms Γ raw path)

/-- Successful expression synthesis yields the accepted independent typing
judgment and exact raw erasure. -/
theorem buildSynthExpr_sound {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs} {raw path result}
    (success : buildSynthExpr Γ scope raw path = .ok result) :
    ExprSynthesizes Γ scope raw result.sort result.expr ∧
      result.expr.erase = raw := by
  have checked := (liftTermResult_ok_iff (synthExpr Γ scope raw path) result).mp success
  exact ⟨synthExpr_sound checked, synthExpr_success_erase_exact checked⟩

/-- Successful expected-sort checking yields the accepted independent typing
judgment and exact raw erasure. -/
theorem buildExpectedExpr_sound {Γ : TermContext}
    {scope : RowScope Γ.model Γ.current Γ.inputs}
    {raw expected origin path result}
    (success : buildExpectedExpr Γ scope raw expected origin path = .ok result) :
    ExprChecks Γ scope expected raw result ∧ result.erase = raw := by
  have checked :=
    (liftTermResult_ok_iff (checkExpr Γ scope raw expected origin path) result).mp success
  exact ⟨checkExpr_sound checked, checkExpr_success_erase_exact checked⟩

/-- Successful complete transition checking proves every guard, hazard, effect,
claim, uniqueness, and Ref-write coverage rule and preserves all payload order. -/
theorem buildTransition_sound {Γ : TermContext} {raw path checked}
    (success : buildTransition Γ raw path = .ok checked) :
    TransitionWellTyped Γ raw checked ∧
      checked.eraseGuard = raw.guard ∧
      checked.eraseHazard = raw.hazard ∧
      checked.eraseEffects = raw.effects ∧
      checked.eraseClaims = raw.contests := by
  have checkedOk :=
    (liftTermResult_ok_iff (checkTransitionTerms Γ raw path) checked).mp success
  have typed := checkTransitionTerms_sound checkedOk
  exact ⟨typed, typed.eraseGuard_exact, typed.eraseHazard_exact,
    typed.eraseEffects_exact, typed.eraseClaims_exact⟩

/-- Every independently well-typed raw transition is reproduced exactly. -/
theorem buildTransition_complete {Γ : TermContext} {raw checked}
    (typed : TransitionWellTyped Γ raw checked)
    (path : List ModelCheckPathSegment := []) :
    buildTransition Γ raw path = .ok checked := by
  apply (liftTermResult_ok_iff (checkTransitionTerms Γ raw path) checked).mpr
  exact checkTransitionTerms_complete typed path

/-- Each exact PRD 0006 transition-checker error is retained unchanged. -/
theorem buildTransition_error_iff (Γ : TermContext) (raw : IR.Transition)
    (error : TermCheckError) (path : List ModelCheckPathSegment := []) :
    buildTransition Γ raw path = .error (.term error) ↔
      checkTransitionTerms Γ raw path = .error error :=
  liftTermResult_error_iff (checkTransitionTerms Γ raw path) error

/-- Transition-builder failure is exactly a PRD 0006 transition-checker failure. -/
theorem buildTransition_failure_iff (Γ : TermContext) (raw : IR.Transition)
    (path : List ModelCheckPathSegment := []) :
    (∃ error, buildTransition Γ raw path = .error (.term error)) ↔
      ∃ error, checkTransitionTerms Γ raw path = .error error := by
  constructor <;> rintro ⟨error, failed⟩
  · exact ⟨error, (buildTransition_error_iff Γ raw error path).mp failed⟩
  · exact ⟨error, (buildTransition_error_iff Γ raw error path).mpr failed⟩

/-! ## Current race-time-only surface adapter -/

private def firstKeyIndexAux : Nat → List IR.ResourceClaim → Option Nat
  | _, [] => none
  | index, claim :: claims =>
      match claim.ordering with
      | .raceTime => firstKeyIndexAux (index + 1) claims
      | .key _ => some index

/-- First key-ordered claim, if any, in exact source order. -/
private def firstKeyIndex (claims : List IR.ResourceClaim) : Option Nat :=
  firstKeyIndexAux 0 claims

private theorem firstKeyIndexAux_none_iff (claims : List IR.ResourceClaim)
    (index : Nat) :
    firstKeyIndexAux index claims = none ↔
      ∀ claim ∈ claims, claim.ordering = .raceTime := by
  induction claims generalizing index with
  | nil => simp [firstKeyIndexAux]
  | cons claim claims ih =>
      cases claim with
      | mk resource ordering =>
          cases ordering <;> simp [firstKeyIndexAux, ih]

/-- Absence of a key index means exactly that every source claim is race-time. -/
private theorem firstKeyIndex_none_iff (claims : List IR.ResourceClaim) :
    firstKeyIndex claims = none ↔
      ∀ claim ∈ claims, claim.ordering = .raceTime :=
  firstKeyIndexAux_none_iff claims 0

/-- Preserve the current surface's race-time-only claim restriction while the
raw/checker path above continues to admit every checker-valid key domain. -/
def buildSurfaceTransition (Γ : TermContext) (raw : IR.Transition)
    (boxIndex : Nat := 0) (transitionIndex : Nat := 0)
    (path : List ModelCheckPathSegment := []) :
    Except TransitionBuilderError (TransitionTerms Γ.model Γ.current Γ.inputs) :=
  match firstKeyIndex raw.contests with
  | some claimIndex => .error (.unsupportedSurfaceKeyOrdering
      [.box boxIndex, .transition transitionIndex, .claim claimIndex, .orderingKey])
  | none => buildTransition Γ raw path

@[simp] private theorem buildSurfaceTransition_key_rejected {Γ : TermContext}
    {raw : IR.Transition} {boxIndex transitionIndex claimIndex path}
    (found : firstKeyIndex raw.contests = some claimIndex) :
    buildSurfaceTransition Γ raw boxIndex transitionIndex path =
      .error (.unsupportedSurfaceKeyOrdering
        [.box boxIndex, .transition transitionIndex, .claim claimIndex, .orderingKey]) := by
  simp [buildSurfaceTransition, found]

/-- Surface rejection occurs exactly when at least one source claim is not
race-time. The returned path identifies a source claim ordinal without exposing
the private first-key search. -/
theorem buildSurfaceTransition_unsupported_iff (Γ : TermContext)
    (raw : IR.Transition) (boxIndex transitionIndex : Nat := 0)
    (path : List ModelCheckPathSegment := []) :
    (∃ claimIndex,
      buildSurfaceTransition Γ raw boxIndex transitionIndex path =
        .error (.unsupportedSurfaceKeyOrdering
          [.box boxIndex, .transition transitionIndex, .claim claimIndex, .orderingKey])) ↔
      ¬ ∀ claim ∈ raw.contests, claim.ordering = .raceTime := by
  cases found : firstKeyIndex raw.contests with
  | none =>
      have raceOnly := (firstKeyIndex_none_iff raw.contests).mp found
      constructor
      · rintro ⟨claimIndex, failed⟩
        cases checked : checkTransitionTerms Γ raw path <;>
          simp [buildSurfaceTransition, found, buildTransition, liftTermResult,
            checked] at failed
      · intro notRaceOnly
        exact (notRaceOnly raceOnly).elim
  | some claimIndex =>
      have notRaceOnly : ¬ ∀ claim ∈ raw.contests, claim.ordering = .raceTime := by
        intro raceOnly
        have noKey := (firstKeyIndex_none_iff raw.contests).mpr raceOnly
        simp [found] at noKey
      constructor
      · intro _
        exact notRaceOnly
      · intro _
        exact ⟨claimIndex, buildSurfaceTransition_key_rejected found⟩

/-- Surface failure is exactly either the frontend-only race-time restriction
or an authoritative transition-checker failure. The statement deliberately
does not prescribe precedence when both independent defects are present. -/
theorem buildSurfaceTransition_failure_iff (Γ : TermContext)
    (raw : IR.Transition) (boxIndex transitionIndex : Nat := 0)
    (path : List ModelCheckPathSegment := []) :
    (∃ error,
      buildSurfaceTransition Γ raw boxIndex transitionIndex path = .error error) ↔
      (¬ ∀ claim ∈ raw.contests, claim.ordering = .raceTime) ∨
      ∃ termError, checkTransitionTerms Γ raw path = .error termError := by
  cases found : firstKeyIndex raw.contests with
  | none =>
      have raceOnly := (firstKeyIndex_none_iff raw.contests).mp found
      cases checked : checkTransitionTerms Γ raw path with
      | ok checkedTerms =>
          constructor
          · rintro ⟨error, failed⟩
            simp [buildSurfaceTransition, found, buildTransition, liftTermResult,
              checked] at failed
          · rintro (notRaceOnly | ⟨termError, failed⟩)
            · exact (notRaceOnly raceOnly).elim
            · simp [checked] at failed
      | error termError =>
          constructor
          · intro _
            exact .inr ⟨termError, rfl⟩
          · intro _
            exact ⟨.term termError, by
              simp [buildSurfaceTransition, found, buildTransition, liftTermResult,
                checked]⟩
  | some claimIndex =>
      have notRaceOnly : ¬ ∀ claim ∈ raw.contests, claim.ordering = .raceTime := by
        intro raceOnly
        have noKey := (firstKeyIndex_none_iff raw.contests).mpr raceOnly
        simp [found] at noKey
      constructor
      · intro _
        exact .inl notRaceOnly
      · intro _
        exact ⟨.unsupportedSurfaceKeyOrdering
            [.box boxIndex, .transition transitionIndex, .claim claimIndex, .orderingKey],
          buildSurfaceTransition_key_rejected found⟩

/-- Every successful surface request contains only race-time claims. -/
theorem buildSurfaceTransition_success_race_only {Γ : TermContext}
    {raw : IR.Transition} {boxIndex transitionIndex path checked}
    (success : buildSurfaceTransition Γ raw boxIndex transitionIndex path = .ok checked) :
    ∀ claim ∈ raw.contests, claim.ordering = .raceTime := by
  cases found : firstKeyIndex raw.contests with
  | some claimIndex =>
      simp [buildSurfaceTransition, found] at success
  | none => exact (firstKeyIndex_none_iff raw.contests).mp found

/-- A race-time-only surface request delegates unchanged to complete checking. -/
theorem buildSurfaceTransition_race_only {Γ : TermContext}
    {raw : IR.Transition} {boxIndex transitionIndex path}
    (raceOnly : ∀ claim ∈ raw.contests, claim.ordering = .raceTime) :
    buildSurfaceTransition Γ raw boxIndex transitionIndex path =
      buildTransition Γ raw path := by
  have noKey := (firstKeyIndex_none_iff raw.contests).mpr raceOnly
  simp [buildSurfaceTransition, noKey]

/-! ## Source-ordinal transition overlays -/

/-- Exactly one transition list for each existing core box ordinal. The
dependent function cannot contain an extra list and cannot omit an ordinal. -/
structure TransitionOverlaySpec where
  core : CoreModelShell
  transitions : (ordinal : Fin core.boxes.length) → List IR.Transition

namespace TransitionOverlaySpec

/-- Exact raw box at one existing core source ordinal. -/
def rawBox (spec : TransitionOverlaySpec)
    (ordinal : Fin spec.core.boxes.length) : IR.Box :=
  { (spec.core.boxes.get ordinal).toRaw with
    transitions := spec.transitions ordinal }

/-- Source-ordered boxes generated from every and only core source ordinal. -/
def rawBoxes (spec : TransitionOverlaySpec) : List IR.Box :=
  List.ofFn spec.rawBox

/-- Exact transition-only raw model projection. -/
def toRaw (spec : TransitionOverlaySpec) : IR.Model :=
  { spec.core.toRaw with boxes := spec.rawBoxes }

@[simp] theorem rawBox_name (spec : TransitionOverlaySpec) (ordinal) :
    (spec.rawBox ordinal).name = (spec.core.boxes.get ordinal).name := rfl
@[simp] theorem rawBox_tables (spec : TransitionOverlaySpec) (ordinal) :
    (spec.rawBox ordinal).tables = (spec.core.boxes.get ordinal).tables := rfl
@[simp] theorem rawBox_transitions (spec : TransitionOverlaySpec) (ordinal) :
    (spec.rawBox ordinal).transitions = spec.transitions ordinal := rfl
@[simp] theorem rawBox_inputs (spec : TransitionOverlaySpec) (ordinal) :
    (spec.rawBox ordinal).inputs = [] := rfl
@[simp] theorem rawBox_outputs (spec : TransitionOverlaySpec) (ordinal) :
    (spec.rawBox ordinal).outputs = [] := rfl
@[simp] theorem rawBox_views (spec : TransitionOverlaySpec) (ordinal) :
    (spec.rawBox ordinal).views = [] := rfl
@[simp] theorem rawBox_groupedViews (spec : TransitionOverlaySpec) (ordinal) :
    (spec.rawBox ordinal).groupedViews = [] := rfl

@[simp] theorem rawBoxes_length (spec : TransitionOverlaySpec) :
    spec.rawBoxes.length = spec.core.boxes.length := by
  simp [rawBoxes]

/-- Reading generated boxes by the same source ordinal returns exactly the
attached box; this is the no-drop/no-reassignment theorem. -/
theorem rawBoxes_get (spec : TransitionOverlaySpec)
    (ordinal : Fin spec.core.boxes.length) :
    spec.rawBoxes.get ⟨ordinal.val, by
      rw [rawBoxes_length]
      exact ordinal.isLt⟩ = spec.rawBox ordinal := by
  simp [rawBoxes]

@[simp] theorem toRaw_name (spec : TransitionOverlaySpec) :
    spec.toRaw.name = spec.core.name := rfl
@[simp] theorem toRaw_dt (spec : TransitionOverlaySpec) :
    spec.toRaw.dt = spec.core.dt := rfl
@[simp] theorem toRaw_params (spec : TransitionOverlaySpec) :
    spec.toRaw.params = spec.core.params := rfl
@[simp] theorem toRaw_boxes (spec : TransitionOverlaySpec) :
    spec.toRaw.boxes = spec.rawBoxes := rfl
@[simp] theorem toRaw_wires (spec : TransitionOverlaySpec) :
    spec.toRaw.wires = [] := rfl
@[simp] theorem toRaw_summaries (spec : TransitionOverlaySpec) :
    spec.toRaw.summaries = [] := rfl

/-- Core box names remain in exact source order. -/
theorem toRaw_box_names_exact (spec : TransitionOverlaySpec) :
    spec.toRaw.boxes.map IR.Box.name = spec.core.boxNames := by
  apply List.ext_get
  · simp [CoreModelShell.boxNames]
  · intro index leftBound rightBound
    simp [rawBoxes, rawBox, CoreModelShell.boxNames]

/-- Core table/attribute declarations remain in exact box source order. -/
theorem toRaw_box_tables_exact (spec : TransitionOverlaySpec) :
    spec.toRaw.boxes.map IR.Box.tables =
      spec.core.boxes.map CoreBoxShell.tables := by
  apply List.ext_get
  · simp
  · intro index leftBound rightBound
    simp [rawBoxes, rawBox]

/-- Every later-owned per-box field remains empty in this slice. -/
theorem toRaw_slice_boundary (spec : TransitionOverlaySpec) :
    spec.toRaw.boxes.Forall (fun box =>
      box.inputs = [] ∧ box.outputs = [] ∧ box.views = [] ∧ box.groupedViews = []) ∧
      spec.toRaw.wires = [] ∧ spec.toRaw.summaries = [] := by
  constructor
  · rw [toRaw_boxes]
    apply List.forall_iff_forall_mem.mpr
    intro box member
    rw [rawBoxes] at member
    have ranged : ∃ ordinal, spec.rawBox ordinal = box := by
      simpa only [List.mem_ofFn, Set.mem_range] using member
    obtain ⟨ordinal, rfl⟩ := ranged
    exact ⟨rfl, rfl, rfl, rfl⟩
  · exact ⟨rfl, rfl⟩

end TransitionOverlaySpec

/-! ## Whole-model construction and correspondence -/

/-- Validate the accepted core first, then the exact transition overlay through
the authoritative whole-model checker. -/
def buildTransitionOverlay (spec : TransitionOverlaySpec) :
    Except TransitionBuilderError Checked.Model :=
  match buildModelShell spec.core with
  | .error error => .error (.core error)
  | .ok _ => liftModelResult (checkModel spec.toRaw)

/-- Each exact core failure is preserved without re-running core diagnostics. -/
theorem buildTransitionOverlay_core_error_iff (spec : TransitionOverlaySpec)
    (error : CoreBuilderError) :
    buildTransitionOverlay spec = .error (.core error) ↔
      buildModelShell spec.core = .error error := by
  cases coreResult : buildModelShell spec.core with
  | error coreError => simp [buildTransitionOverlay, coreResult]
  | ok coreRaw =>
      cases modelResult : checkModel spec.toRaw with
      | ok checked => simp [buildTransitionOverlay, coreResult, modelResult, liftModelResult]
      | error modelError =>
          cases modelError <;>
            simp [buildTransitionOverlay, coreResult, modelResult, liftModelResult]

/-- Each exact PRD 0005 declaration failure is preserved. -/
theorem buildTransitionOverlay_declaration_error_iff
    (spec : TransitionOverlaySpec) (error : CheckError) :
    buildTransitionOverlay spec = .error (.declaration error) ↔
      ∃ coreRaw, buildModelShell spec.core = .ok coreRaw ∧
        checkModel spec.toRaw = .error (.declaration error) := by
  cases coreResult : buildModelShell spec.core with
  | error coreError => simp [buildTransitionOverlay, coreResult]
  | ok coreRaw =>
      cases modelResult : checkModel spec.toRaw with
      | ok checked => simp [buildTransitionOverlay, coreResult, modelResult, liftModelResult]
      | error modelError =>
          cases modelError <;>
            simp [buildTransitionOverlay, coreResult, modelResult, liftModelResult]

/-- Each exact PRD 0006 model/term failure and its path is preserved. -/
theorem buildTransitionOverlay_model_check_error_iff
    (spec : TransitionOverlaySpec) (error : ModelTermError) :
    buildTransitionOverlay spec = .error (.modelCheck (.model error)) ↔
      ∃ coreRaw, buildModelShell spec.core = .ok coreRaw ∧
        checkModel spec.toRaw = .error (.model error) := by
  cases coreResult : buildModelShell spec.core with
  | error coreError => simp [buildTransitionOverlay, coreResult]
  | ok coreRaw =>
      cases modelResult : checkModel spec.toRaw with
      | ok checked => simp [buildTransitionOverlay, coreResult, modelResult, liftModelResult]
      | error modelError =>
          cases modelError <;>
            simp [buildTransitionOverlay, coreResult, modelResult, liftModelResult]

/-- Successful overlay construction is accepted by the whole-model checker and
its checked result reconstructs the exact overlay raw model. -/
theorem buildTransitionOverlay_sound {spec : TransitionOverlaySpec} {checked}
    (success : buildTransitionOverlay spec = .ok checked) :
    ModelWellFormed spec.toRaw ∧
      checkModel spec.toRaw = .ok checked ∧ checked.erase = spec.toRaw := by
  unfold buildTransitionOverlay at success
  cases coreResult : buildModelShell spec.core with
  | error error => simp [coreResult] at success
  | ok raw =>
      have checkedOk :=
        (liftModelResult_ok_iff (checkModel spec.toRaw) checked).mp (by
          simpa [coreResult] using success)
      exact ⟨(checkModel_sound checkedOk).1, checkedOk,
        (checkModel_sound checkedOk).2⟩

/-- Public whole-model acceptance and exact-erasure bridge for transition-rich
models. -/
theorem buildTransitionOverlay_model_acceptance_and_erasure
    {spec : TransitionOverlaySpec} {checked}
    (success : buildTransitionOverlay spec = .ok checked) :
    ∃ actual, checkModel spec.toRaw = .ok actual ∧ actual.erase = spec.toRaw := by
  exact ⟨checked, (buildTransitionOverlay_sound success).2.1,
    (buildTransitionOverlay_sound success).2.2⟩

/-- Every independently valid core-plus-transition candidate is reproduced by
the builder without structural change. -/
theorem buildTransitionOverlay_complete (spec : TransitionOverlaySpec)
    (coreAccepted : DeclarationsWellFormed spec.core.toRaw)
    (accepted : ModelWellFormed spec.toRaw) :
    ∃ checked, buildTransitionOverlay spec = .ok checked := by
  have coreOk := buildModelShell_complete coreAccepted
  obtain ⟨checked, checkedOk⟩ := checkModel_complete accepted
  refine ⟨checked, ?_⟩
  simp [buildTransitionOverlay, coreOk, checkedOk, liftModelResult]

/-- Builder failure is characterized solely by core-builder or whole-model
checker failure, never by a circular builder predicate. -/
theorem buildTransitionOverlay_failure_iff (spec : TransitionOverlaySpec) :
    (∃ error, buildTransitionOverlay spec = .error error) ↔
      (∃ coreError, buildModelShell spec.core = .error coreError) ∨
      (∃ coreRaw, buildModelShell spec.core = .ok coreRaw ∧
        ∃ modelError, checkModel spec.toRaw = .error modelError) := by
  cases coreResult : buildModelShell spec.core with
  | error coreError =>
      simp [buildTransitionOverlay, coreResult]
  | ok coreRaw =>
      cases modelResult : checkModel spec.toRaw with
      | ok checked => simp [buildTransitionOverlay, coreResult, modelResult, liftModelResult]
      | error modelError =>
          cases modelError <;>
            simp [buildTransitionOverlay, coreResult, modelResult, liftModelResult]

end Sembla.Frontend.Builders
