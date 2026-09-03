use sembla_ir::{ValidatedModel, ViewReduce};

use crate::eval::{expr_is_gather_eligible, expr_is_gather_eligible_int, EvalTable};

/// A numeric observation scalar. Real equality is bitwise so report equality
/// remains an exact determinism check, including signed zero and NaN payloads.
#[derive(Clone, Copy, Debug)]
pub enum ObservationValue {
    Real(f64),
    Int(i64),
}

impl PartialEq for ObservationValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Real(left), Self::Real(right)) => left.to_bits() == right.to_bits(),
            (Self::Int(left), Self::Int(right)) => left == right,
            _ => false,
        }
    }
}

impl Eq for ObservationValue {}

/// One declaration-ordered view value from a committed post-tick state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ViewValue {
    pub box_name: String,
    pub name: String,
    pub value: ObservationValue,
}

/// Conservative IR-only eligibility for one declared observation view.
///
/// This is a backend-capability description derived from oracle semantics, not
/// a device implementation type. It deliberately names no hardware API.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceViewEligibility {
    pub box_name: String,
    pub name: String,
    pub eligible: bool,
    pub reason: &'static str,
}

/// Run-wide device-observation decision. State download may be skipped only
/// when this decision is eligible; one host-bound view forces the complete run
/// back to host observation. Backends consume this semantic decision; the
/// runtime neither selects nor depends on a backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceObservationEligibility {
    pub eligible: bool,
    pub reason: &'static str,
    pub views: Vec<DeviceViewEligibility>,
}

/// Decides device-observation eligibility from validated IR.
///
/// Eligibility means that a backend can preserve the CPU oracle's exact
/// observation contract without materializing host state. The expression check
/// deliberately reuses the evaluator's gather predicate, keeping this policy
/// backend-neutral and avoiding another expression whitelist.
pub fn device_observation_eligibility(model: &ValidatedModel) -> DeviceObservationEligibility {
    const ELIGIBLE: &str = "all scalar and grouped views are device-eligible";
    const FALLBACK: &str = "at least one view requires host observation";
    const NO_VIEWS: &str = "no declared views; legacy state reporting requires host state";
    const COUNT: &str = "count with a row-local filter";
    const INT_MIN_MAX: &str = "Int min/max with row-local filter and value";
    const GROUPED_COUNT: &str = "grouped count with a row-local filter and exactly boundable keys";
    const FILTER: &str = "filter is not a row-local infallible expression";
    const VALUE: &str =
        "value is not a row-local infallible Int expression; Real extrema retain host NaN ordering";
    const SUM: &str = "Sum preserves host order for Real and host overflow association for Int";

    let mut views = Vec::new();
    for model_box in &model.model().boxes {
        for view in &model_box.views {
            let table = EvalTable::new(model, &model_box.name, &view.table);
            let filter_eligible = table.is_ok_and(|table| {
                view.filter.as_ref().map_or(true, |filter| {
                    expr_is_gather_eligible(filter, table).unwrap_or(false)
                })
            });
            let (eligible, reason) = if !filter_eligible {
                (false, FILTER)
            } else {
                match view.reduce {
                    ViewReduce::Count => (true, COUNT),
                    ViewReduce::Sum => (false, SUM),
                    ViewReduce::Min | ViewReduce::Max => {
                        let value_eligible = EvalTable::new(model, &model_box.name, &view.table)
                            .ok()
                            .zip(view.value.as_ref())
                            .is_some_and(|(table, value)| {
                                expr_is_gather_eligible_int(value, table).unwrap_or(false)
                            });
                        if value_eligible {
                            (true, INT_MIN_MAX)
                        } else {
                            (false, VALUE)
                        }
                    }
                }
            };
            views.push(DeviceViewEligibility {
                box_name: model_box.name.clone(),
                name: view.name.clone(),
                eligible,
                reason,
            });
        }
        for view in &model_box.grouped_views {
            let filter_eligible =
                EvalTable::new(model, &model_box.name, &view.table).is_ok_and(|table| {
                    view.filter.as_ref().map_or(true, |filter| {
                        expr_is_gather_eligible(filter, table).unwrap_or(false)
                    })
                });
            views.push(DeviceViewEligibility {
                box_name: model_box.name.clone(),
                name: view.name.clone(),
                eligible: filter_eligible,
                reason: if filter_eligible {
                    GROUPED_COUNT
                } else {
                    FILTER
                },
            });
        }
    }
    let eligible = !views.is_empty() && views.iter().all(|view| view.eligible);
    DeviceObservationEligibility {
        eligible,
        reason: if eligible {
            ELIGIBLE
        } else if views.is_empty() {
            NO_VIEWS
        } else {
            FALLBACK
        },
        views,
    }
}

/// One non-empty grouped bucket from committed post-tick state.
/// Keys retain underlying numeric values so callers sort before rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroupedViewValue {
    pub box_name: String,
    pub name: String,
    pub keys: Vec<i128>,
    pub count: usize,
}

/// One model-declaration-ordered summary value folded across a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryValue {
    pub name: String,
    pub value: ObservationValue,
}
