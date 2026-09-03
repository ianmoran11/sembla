use sembla_ir::{Attr, AttrType, Expr, ParamType, ValidatedModel, ViewReduce};

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NumericKind {
    Real,
    Int,
}

fn row_numeric_kind(
    expr: &Expr,
    model: &ValidatedModel,
    row_attrs: &[Attr],
) -> Option<NumericKind> {
    match expr {
        Expr::Real { .. } => Some(NumericKind::Real),
        Expr::Int { .. } => Some(NumericKind::Int),
        Expr::Param { name } => model
            .model()
            .params
            .iter()
            .find(|param| param.name == *name)
            .map(|param| match param.ty {
                ParamType::Real => NumericKind::Real,
                ParamType::Int => NumericKind::Int,
            }),
        Expr::SelfAttr { name } => {
            row_attrs
                .iter()
                .find(|attr| attr.name == *name)
                .and_then(|attr| match attr.ty {
                    AttrType::Real => Some(NumericKind::Real),
                    AttrType::Int => Some(NumericKind::Int),
                    AttrType::Enum { .. } | AttrType::Ref { .. } => None,
                })
        }
        Expr::Add { lhs, rhs } | Expr::Sub { lhs, rhs } | Expr::Mul { lhs, rhs } => {
            let lhs = row_numeric_kind(lhs, model, row_attrs)?;
            let rhs = row_numeric_kind(rhs, model, row_attrs)?;
            Some(if lhs == NumericKind::Real || rhs == NumericKind::Real {
                NumericKind::Real
            } else {
                NumericKind::Int
            })
        }
        Expr::Div { lhs, rhs } => {
            row_numeric_kind(lhs, model, row_attrs)?;
            row_numeric_kind(rhs, model, row_attrs)?;
            Some(NumericKind::Real)
        }
        Expr::Bool { .. }
        | Expr::Enum { .. }
        | Expr::Eq { .. }
        | Expr::Ne { .. }
        | Expr::Lt { .. }
        | Expr::Le { .. }
        | Expr::Gt { .. }
        | Expr::Ge { .. }
        | Expr::And { .. }
        | Expr::Or { .. }
        | Expr::Not { .. }
        | Expr::EnumIs { .. }
        | Expr::Input { .. }
        | Expr::Agg { .. } => None,
    }
}

fn row_expr_is_infallible(expr: &Expr, model: &ValidatedModel, row_attrs: &[Attr]) -> bool {
    match expr {
        Expr::Real { .. }
        | Expr::Int { .. }
        | Expr::Bool { .. }
        | Expr::Enum { .. }
        | Expr::Param { .. }
        | Expr::SelfAttr { .. }
        | Expr::EnumIs { .. } => true,
        Expr::Add { lhs, rhs } | Expr::Sub { lhs, rhs } | Expr::Mul { lhs, rhs } => {
            row_numeric_kind(expr, model, row_attrs) == Some(NumericKind::Real)
                && row_expr_is_infallible(lhs, model, row_attrs)
                && row_expr_is_infallible(rhs, model, row_attrs)
        }
        Expr::Div { lhs, rhs }
        | Expr::Eq { lhs, rhs }
        | Expr::Ne { lhs, rhs }
        | Expr::Lt { lhs, rhs }
        | Expr::Le { lhs, rhs }
        | Expr::Gt { lhs, rhs }
        | Expr::Ge { lhs, rhs }
        | Expr::And { lhs, rhs }
        | Expr::Or { lhs, rhs } => {
            row_expr_is_infallible(lhs, model, row_attrs)
                && row_expr_is_infallible(rhs, model, row_attrs)
        }
        Expr::Not { expr } => row_expr_is_infallible(expr, model, row_attrs),
        Expr::Input { .. } | Expr::Agg { .. } => false,
    }
}

/// Decides device-observation eligibility from validated IR.
///
/// Eligibility means that a backend can preserve the CPU oracle's exact
/// observation contract without materializing host state. The expression check
/// is defined here over validated IR so accelerator capability policy cannot
/// depend on the CPU implementation crate.
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
            let table_attrs = model_box
                .tables
                .iter()
                .find(|table| table.name == view.table)
                .map(|table| table.attrs.as_slice());
            let filter_eligible = table_attrs.is_some_and(|attrs| {
                view.filter
                    .as_ref()
                    .map_or(true, |filter| row_expr_is_infallible(filter, model, attrs))
            });
            let (eligible, reason) = if !filter_eligible {
                (false, FILTER)
            } else {
                match view.reduce {
                    ViewReduce::Count => (true, COUNT),
                    ViewReduce::Sum => (false, SUM),
                    ViewReduce::Min | ViewReduce::Max => {
                        let value_eligible =
                            table_attrs
                                .zip(view.value.as_ref())
                                .is_some_and(|(attrs, value)| {
                                    row_numeric_kind(value, model, attrs) == Some(NumericKind::Int)
                                        && row_expr_is_infallible(value, model, attrs)
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
            let filter_eligible = model_box
                .tables
                .iter()
                .find(|table| table.name == view.table)
                .is_some_and(|table| {
                    view.filter.as_ref().map_or(true, |filter| {
                        row_expr_is_infallible(filter, model, &table.attrs)
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
