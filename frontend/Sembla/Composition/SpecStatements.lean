import Sembla.Composition.Link
import Sembla.Composition.SpecObservation
import Sembla.Composition.SpecStatic

namespace Sembla.Composition

/-!
# Composition preservation statements

Proof status in V1:

* `linkV1_produces_valid_plan` is proved.
* `staticPreservationStatement` is stated-deferred as a general theorem shape;
  the complete linkable fixture corpus is checked executably in `SpecTests`.
* `preservationStatement` is stated-deferred because executable stochastic
  source and plan denotations are future work.

The opaque observation functions freeze the signatures of that future work
without pretending that source meaning is defined by linking. The V1
observation quotient includes every field of `CompositionObservation`, notably
the Philox draw coordinates. Hashes remain consequences of canonical artifacts,
not observations themselves. See DECISIONS.md §J10 and §J13.
-/

/-- Future independent behavioral source denotation. Its implementation is
    intentionally deferred beyond the static structural fragment. -/
opaque denoteSourceObs : CompositionSourceV1 → CompositionObservation

/-- Future behavioral denotation of a flat executable plan. Its implementation
    is intentionally deferred beyond the static structural fragment. -/
opaque denotePlanObs : Plan.ExecutablePlanV1 → CompositionObservation

/-- Full behavioral preservation: the frozen obligation, stated but deferred. -/
def preservationStatement : Prop :=
  ∀ (src : CompositionSourceV1) (bytes : String) (r : LinkResultV1),
    linkV1 src bytes = .ok r →
    denoteSourceObs src = denotePlanObs r.plan

/-- Static preservation: the V1-checkable core of behavioral preservation. -/
def staticPreservationStatement : Prop :=
  ∀ (src : CompositionSourceV1) (bytes : String) (r : LinkResultV1)
      (meaning : StaticMeaning),
    linkV1 src bytes = .ok r →
    denoteSourceStatic src = .ok meaning →
    denotePlanStatic r.plan = meaning

/-- Every successful linker result has passed the executable-plan V1 validity
    boundary. This is proved from the linker's explicit finalization gate. -/
theorem linkV1_produces_valid_plan
    (src : CompositionSourceV1) (bytes : String) (r : LinkResultV1)
    (h : linkV1 src bytes = .ok r) :
    planValidCheck r.plan = true :=
  linkV1ProducesValidPlan src bytes r h

end Sembla.Composition
