//! Deterministic, snapshot-isolated synchronous box composition.

use std::cmp::Ordering;
use std::error::Error;
use std::fmt;

use sembla_ir::{
    AggOp, AttrType, ClaimOrdering, Effect, Expr, OutputBuilder, SummaryReduce, ValidatedModel,
    ViewReduce,
};

use crate::eval::{
    eval_column, eval_typed_ref_column, AggCache, EvalError, EvalTable, ParamEnv, ValueColumn,
};
use crate::rng::exp_f64;
use crate::state::{ColumnData, InputTable, Snapshot, StateError, StateStore};

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

/// One model-declaration-ordered summary value folded across a run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SummaryValue {
    pub name: String,
    pub value: ObservationValue,
}

/// Observable result of one committed tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickReport {
    pub tick: u32,
    /// View values in box order and then view declaration order.
    pub views: Vec<ViewValue>,
    /// Model-global rule counts, retained for single-box API compatibility.
    pub fired: Vec<(u32, usize)>,
    /// Counts grouped in box declaration order for composed-model reporting.
    pub fired_per_box: Vec<(String, Vec<(u32, usize)>)>,
    pub deferred_per_resource_table: Vec<(String, usize)>,
    /// PRD 0005 group-by accumulators built across all boxes for this tick.
    /// A cached aggregate contributes once regardless of querying row count.
    pub aggregate_builds: usize,
}

/// A structured saturation warning produced by [`run`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaturationWarning {
    pub tick: u32,
    pub table: String,
    pub deferred_count: usize,
    pub fired_count: usize,
}

/// Observable result of a multi-tick run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunReport {
    pub ticks: Vec<TickReport>,
    pub summaries: Vec<SummaryValue>,
    pub warnings: Vec<SaturationWarning>,
}

/// A deterministic tick execution failure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TickError {
    UnsupportedBoxCount {
        found: usize,
    },
    Evaluation(String),
    State(String),
    InvalidRuntimeType {
        context: String,
        found: String,
    },
    EntityIdOverflow {
        rule_id: u32,
        row: usize,
    },
    IncompatibleClaimOrdering {
        table: String,
        row: u32,
    },
    DoubleWrite {
        box_name: Box<str>,
        table: Box<str>,
        attr: Box<str>,
        row: usize,
        first_rule_id: u32,
        first_transition: Box<str>,
        second_rule_id: u32,
        second_transition: Box<str>,
    },
}

impl fmt::Display for TickError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedBoxCount { found } => write!(
                formatter,
                "tick executor requires exactly one box, found {found}"
            ),
            Self::Evaluation(message) => write!(formatter, "expression evaluation failed: {message}"),
            Self::State(message) => write!(formatter, "state operation failed: {message}"),
            Self::InvalidRuntimeType { context, found } => {
                write!(formatter, "{context} evaluated to {found}")
            }
            Self::EntityIdOverflow { rule_id, row } => write!(
                formatter,
                "rule {rule_id} row {row} cannot be represented as a u32 entity ID"
            ),
            Self::IncompatibleClaimOrdering { table, row } => write!(
                formatter,
                "resource '{table}' row {row} has incompatible claim ordering modes or key types"
            ),
            Self::DoubleWrite {
                box_name,
                table,
                attr,
                row,
                first_rule_id,
                first_transition,
                second_rule_id,
                second_transition,
            } => write!(
                formatter,
                "double write to {box_name}.{table}.{attr}[{row}] by transition '{first_transition}' (rule {first_rule_id}) and transition '{second_transition}' (rule {second_rule_id})"
            ),
        }
    }
}

impl Error for TickError {}

impl From<EvalError> for TickError {
    fn from(error: EvalError) -> Self {
        Self::Evaluation(error.to_string())
    }
}

impl From<StateError> for TickError {
    fn from(error: StateError) -> Self {
        Self::State(error.to_string())
    }
}

#[derive(Clone, Debug)]
struct Candidate {
    rule_id: u32,
    table_index: usize,
    entity_id: u32,
    row: usize,
    claims: Vec<CandidateClaim>,
}

#[derive(Clone, Debug)]
struct CandidateClaim {
    table_index: usize,
    resource_row: u32,
    ordering: OrderingValue,
}

#[derive(Clone, Debug)]
enum OrderingValue {
    RaceTime(f64),
    Real(f64),
    Int(i64),
    Enum {
        table_index: usize,
        attr_index: usize,
        value: u16,
    },
}

#[derive(Clone, Copy, Debug)]
struct ClaimInstance {
    candidate_index: usize,
    claim_index: usize,
}

#[derive(Clone, Debug)]
enum PendingValue {
    Real(f64),
    Int(i64),
    Enum(u16),
    Ref(u32),
}

#[derive(Clone, Debug)]
struct PendingWrite {
    box_index: usize,
    table_index: usize,
    attr_index: usize,
    row: usize,
    value: PendingValue,
    rule_id: u32,
    transition_name: String,
}

struct TickOutcome {
    report: TickReport,
    fired_per_resource_table: Vec<(String, usize)>,
}

struct BoxOutcome {
    pending: Vec<PendingWrite>,
    fired: Vec<(u32, usize)>,
    deferred: Vec<usize>,
    fired_per_resource_table: Vec<usize>,
    aggregate_builds: usize,
}

struct Resolution {
    fires: Vec<bool>,
    deferred: Vec<usize>,
    fired_per_resource_table: Vec<usize>,
}

/// Executes and commits one deterministic, snapshot-isolated tick.
pub fn run_tick(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
) -> Result<TickReport, TickError> {
    Ok(execute_tick(model, state, params, seed, tick)?.report)
}

/// Executes ticks `0..n_ticks` and records strict saturation warnings.
pub fn run(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    n_ticks: u32,
) -> Result<RunReport, TickError> {
    let mut ticks = Vec::with_capacity(n_ticks as usize);
    let mut warnings = Vec::new();
    for tick in 0..n_ticks {
        let outcome = execute_tick(model, state, params, seed, tick)?;
        for (table, deferred_count) in &outcome.report.deferred_per_resource_table {
            let fired_count = outcome
                .fired_per_resource_table
                .iter()
                .find(|(name, _)| name == table)
                .map_or(0, |(_, count)| *count);
            if exceeds_saturation_threshold(*deferred_count, fired_count) {
                let warning = SaturationWarning {
                    tick,
                    table: table.clone(),
                    deferred_count: *deferred_count,
                    fired_count,
                };
                eprintln!(
                    "warning: tick {} resource table '{}': {} deferred exceeds 10% of {} fired",
                    warning.tick, warning.table, warning.deferred_count, warning.fired_count
                );
                warnings.push(warning);
            }
        }
        ticks.push(outcome.report);
    }
    let summaries = summarize(model, &ticks)?;
    Ok(RunReport {
        ticks,
        summaries,
        warnings,
    })
}

fn exceeds_saturation_threshold(deferred: usize, fired: usize) -> bool {
    (deferred as u128) * 10 > fired as u128
}

fn execute_tick(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
) -> Result<TickOutcome, TickError> {
    let snapshot = state.snapshot();
    let mut box_outcomes = Vec::with_capacity(model.model().boxes.len());
    for box_index in 0..model.model().boxes.len() {
        box_outcomes.push(stage_box(model, box_index, &snapshot, params, seed, tick)?);
    }

    let pending: Vec<_> = box_outcomes
        .iter_mut()
        .flat_map(|outcome| std::mem::take(&mut outcome.pending))
        .collect();
    detect_double_writes(&pending, model)?;
    let apply_result = {
        let mut writes = state.write_buffer()?;
        pending.iter().try_for_each(|write| {
            let model_box = &model.model().boxes[write.box_index];
            let table = &model_box.tables[write.table_index];
            let attr = &table.attrs[write.attr_index];
            match &write.value {
                PendingValue::Real(value) => {
                    writes.set_real(&model_box.name, &table.name, &attr.name, write.row, *value)
                }
                PendingValue::Int(value) => {
                    writes.set_int(&model_box.name, &table.name, &attr.name, write.row, *value)
                }
                PendingValue::Enum(value) => {
                    writes.set_enum(&model_box.name, &table.name, &attr.name, write.row, *value)
                }
                PendingValue::Ref(value) => {
                    writes.set_ref(&model_box.name, &table.name, &attr.name, write.row, *value)
                }
            }
        })
    };
    if let Err(error) = apply_result {
        state.discard_writes();
        return Err(error.into());
    }
    // Moore-machine outputs observe the prospective new state, but output
    // construction is fallible. Build every delivered table before commit so
    // an overflow or evaluation error leaves both old state and old inputs
    // unchanged.
    let next_inputs = match state
        .prepared_snapshot()
        .map_err(TickError::from)
        .and_then(|prepared| build_next_inputs(model, &prepared, params))
    {
        Ok(inputs) => inputs,
        Err(error) => {
            state.discard_writes();
            return Err(error);
        }
    };
    if let Err(error) = state.commit() {
        state.discard_writes();
        return Err(error.into());
    }
    state.replace_inputs(next_inputs);
    // Observation is deliberately evaluated only after commit and receives an
    // immutable store. It cannot consume RNG coordinates, stage writes, or
    // influence conflict resolution or scheduling.
    let views = observe_views(model, state, params)?;

    let mut fired = model
        .transitions()
        .iter()
        .map(|transition| (transition.rule_id, 0))
        .collect::<Vec<_>>();
    let mut fired_per_box = Vec::with_capacity(box_outcomes.len());
    let mut deferred_per_resource_table = Vec::new();
    let mut fired_per_resource_table = Vec::new();
    let mut aggregate_builds = 0;
    let qualify = model.model().boxes.len() > 1;
    for (box_index, outcome) in box_outcomes.into_iter().enumerate() {
        let model_box = &model.model().boxes[box_index];
        for (rule_id, count) in &outcome.fired {
            fired[*rule_id as usize].1 = *count;
        }
        fired_per_box.push((model_box.name.clone(), outcome.fired));
        aggregate_builds += outcome.aggregate_builds;
        for (table_index, count) in outcome.deferred.into_iter().enumerate() {
            let name = report_table_name(model_box, table_index, qualify);
            if count != 0 {
                deferred_per_resource_table.push((name.clone(), count));
            }
            fired_per_resource_table.push((name, outcome.fired_per_resource_table[table_index]));
        }
    }

    Ok(TickOutcome {
        report: TickReport {
            tick,
            views,
            fired,
            fired_per_box,
            deferred_per_resource_table,
            aggregate_builds,
        },
        fired_per_resource_table,
    })
}

/// Evaluates declaration-ordered views from an already committed state.
///
/// Alternate execution backends use this observation-only entry point after
/// reconstructing a read-only host snapshot; it never schedules transitions,
/// consumes RNG coordinates, or mutates state.
pub fn observe_views(
    model: &ValidatedModel,
    state: &StateStore,
    params: &ParamEnv,
) -> Result<Vec<ViewValue>, TickError> {
    let snapshot = state.snapshot();
    let mut cache = AggCache::new(model, &snapshot, params);
    let mut observations = Vec::new();
    for model_box in &model.model().boxes {
        for view in &model_box.views {
            let table = EvalTable::new(model, &model_box.name, &view.table)?;
            let row_count = snapshot.row_count(&model_box.name, &view.table)?;
            let selected = match &view.filter {
                Some(filter) => match eval_column(filter, table, &snapshot, params, &mut cache)? {
                    ValueColumn::Bool(values) => values,
                    other => return Err(runtime_type("view filter", &other)),
                },
                None => vec![true; row_count],
            };
            let value = match view.reduce {
                ViewReduce::Count => ObservationValue::Int(
                    i64::try_from(selected.iter().filter(|selected| **selected).count()).map_err(
                        |_| {
                            TickError::Evaluation(format!(
                                "view '{}.{}' count exceeds i64",
                                model_box.name, view.name
                            ))
                        },
                    )?,
                ),
                ViewReduce::Sum | ViewReduce::Min | ViewReduce::Max => {
                    let expression = view
                        .value
                        .as_ref()
                        .expect("validated numeric view has a value");
                    let column = eval_column(expression, table, &snapshot, params, &mut cache)?;
                    reduce_view_column(&model_box.name, &view.name, view.reduce, column, &selected)?
                }
            };
            observations.push(ViewValue {
                box_name: model_box.name.clone(),
                name: view.name.clone(),
                value,
            });
        }
    }
    Ok(observations)
}

fn reduce_view_column(
    box_name: &str,
    view_name: &str,
    reduce: ViewReduce,
    column: ValueColumn,
    selected: &[bool],
) -> Result<ObservationValue, TickError> {
    match column {
        ValueColumn::Int(values) => {
            let mut result = match reduce {
                ViewReduce::Sum => 0_i64,
                ViewReduce::Min => i64::MAX,
                ViewReduce::Max => i64::MIN,
                ViewReduce::Count => unreachable!("count does not evaluate a value"),
            };
            for value in values
                .into_iter()
                .zip(selected)
                .filter_map(|(value, selected)| selected.then_some(value))
            {
                result = match reduce {
                    ViewReduce::Sum => result.checked_add(value).ok_or_else(|| {
                        TickError::Evaluation(format!(
                            "view '{box_name}.{view_name}' integer sum overflowed"
                        ))
                    })?,
                    ViewReduce::Min => result.min(value),
                    ViewReduce::Max => result.max(value),
                    ViewReduce::Count => unreachable!(),
                };
            }
            Ok(ObservationValue::Int(result))
        }
        ValueColumn::Real(values) => {
            let mut result = match reduce {
                ViewReduce::Sum => 0.0,
                ViewReduce::Min => f64::INFINITY,
                ViewReduce::Max => f64::NEG_INFINITY,
                ViewReduce::Count => unreachable!("count does not evaluate a value"),
            };
            for value in values
                .into_iter()
                .zip(selected)
                .filter_map(|(value, selected)| selected.then_some(value))
            {
                result = match reduce {
                    ViewReduce::Sum => result + value,
                    ViewReduce::Min if value.total_cmp(&result) == Ordering::Less => value,
                    ViewReduce::Max if value.total_cmp(&result) == Ordering::Greater => value,
                    ViewReduce::Min | ViewReduce::Max => result,
                    ViewReduce::Count => unreachable!(),
                };
            }
            Ok(ObservationValue::Real(result))
        }
        other => Err(runtime_type("view value", &other)),
    }
}

/// Folds model-declared summaries over tick view values in tick order.
pub fn summarize(
    model: &ValidatedModel,
    ticks: &[TickReport],
) -> Result<Vec<SummaryValue>, TickError> {
    let mut summaries = Vec::with_capacity(model.model().summaries.len());
    for declaration in &model.model().summaries {
        let values = ticks
            .iter()
            .map(|tick| {
                tick.views
                    .iter()
                    .find(|view| {
                        view.box_name == declaration.r#box && view.name == declaration.view
                    })
                    .map(|view| (tick.tick, view.value))
                    .ok_or_else(|| {
                        TickError::Evaluation(format!(
                            "summary '{}' could not find view '{}.{}' at tick {}",
                            declaration.name, declaration.r#box, declaration.view, tick.tick
                        ))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let value = fold_summary(&declaration.name, declaration.reduce, &values)?;
        summaries.push(SummaryValue {
            name: declaration.name.clone(),
            value,
        });
    }
    Ok(summaries)
}

fn fold_summary(
    name: &str,
    reduce: SummaryReduce,
    values: &[(u32, ObservationValue)],
) -> Result<ObservationValue, TickError> {
    let Some(&(first_tick, first_value)) = values.first() else {
        return Err(TickError::Evaluation(format!(
            "summary '{name}' cannot reduce an empty run"
        )));
    };
    match reduce {
        SummaryReduce::Last => Ok(values.last().expect("nonempty").1),
        SummaryReduce::ArgmaxTick => {
            let mut best_tick = first_tick;
            let mut best_value = first_value;
            for &(tick, value) in &values[1..] {
                if observation_cmp(value, best_value)? == Ordering::Greater {
                    best_tick = tick;
                    best_value = value;
                }
            }
            Ok(ObservationValue::Int(i64::from(best_tick)))
        }
        SummaryReduce::Sum => match first_value {
            ObservationValue::Int(_) => {
                let mut total = 0_i64;
                for &(_, value) in values {
                    let ObservationValue::Int(value) = value else {
                        return Err(summary_type_mismatch(name));
                    };
                    total = total.checked_add(value).ok_or_else(|| {
                        TickError::Evaluation(format!("summary '{name}' integer sum overflowed"))
                    })?;
                }
                Ok(ObservationValue::Int(total))
            }
            ObservationValue::Real(_) => {
                let mut total = 0.0;
                for &(_, value) in values {
                    let ObservationValue::Real(value) = value else {
                        return Err(summary_type_mismatch(name));
                    };
                    total += value;
                }
                Ok(ObservationValue::Real(total))
            }
        },
        SummaryReduce::Min | SummaryReduce::Max => {
            let mut result = first_value;
            for &(_, value) in &values[1..] {
                let ordering = observation_cmp(value, result)?;
                if (reduce == SummaryReduce::Min && ordering == Ordering::Less)
                    || (reduce == SummaryReduce::Max && ordering == Ordering::Greater)
                {
                    result = value;
                }
            }
            Ok(result)
        }
    }
}

fn observation_cmp(left: ObservationValue, right: ObservationValue) -> Result<Ordering, TickError> {
    match (left, right) {
        (ObservationValue::Int(left), ObservationValue::Int(right)) => Ok(left.cmp(&right)),
        (ObservationValue::Real(left), ObservationValue::Real(right)) => Ok(left.total_cmp(&right)),
        _ => Err(TickError::Evaluation(
            "observation values changed numeric type across ticks".to_owned(),
        )),
    }
}

fn summary_type_mismatch(name: &str) -> TickError {
    TickError::Evaluation(format!(
        "summary '{name}' source changed numeric type across ticks"
    ))
}

fn report_table_name(model_box: &sembla_ir::Box, table_index: usize, qualify: bool) -> String {
    if qualify {
        format!("{}.{}", model_box.name, model_box.tables[table_index].name)
    } else {
        model_box.tables[table_index].name.clone()
    }
}

fn stage_box(
    model: &ValidatedModel,
    box_index: usize,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
) -> Result<BoxOutcome, TickError> {
    let model_box = &model.model().boxes[box_index];
    let transitions: Vec<_> = model
        .transitions()
        .iter()
        .filter(|transition| transition.box_index == box_index)
        .collect();
    let mut cache = AggCache::new(model, snapshot, params);
    let mut candidates = Vec::new();
    for validated in &transitions {
        let transition = &model_box.transitions[validated.transition_index];
        let table_index = model_box
            .tables
            .iter()
            .position(|table| table.name == transition.table)
            .expect("validated transition table disappeared");
        let table = EvalTable::new(model, &model_box.name, &transition.table)?;
        let guards = match eval_column(&transition.guard, table, snapshot, params, &mut cache)? {
            ValueColumn::Bool(values) => values,
            other => return Err(runtime_type("transition guard", &other)),
        };
        let hazards = match eval_column(&transition.hazard, table, snapshot, params, &mut cache)? {
            ValueColumn::Real(values) => values,
            other => return Err(runtime_type("transition hazard", &other)),
        };
        let mut claim_columns = Vec::with_capacity(transition.contests.len());
        for claim in &transition.contests {
            let resources =
                eval_typed_ref_column(&claim.resource, table, snapshot, params, &mut cache)?;
            let resource_table_index = model_box
                .tables
                .iter()
                .position(|schema| schema.name == resources.target_table)
                .expect("validated Ref target table disappeared");
            let ordering = match &claim.ordering {
                ClaimOrdering::RaceTime => None,
                ClaimOrdering::Key { expr } => {
                    Some(eval_column(expr, table, snapshot, params, &mut cache)?)
                }
            };
            claim_columns.push((resource_table_index, resources.values, ordering, claim));
        }
        for (row, (guard, lambda)) in guards.into_iter().zip(hazards).enumerate() {
            if !guard || lambda.partial_cmp(&0.0) != Some(Ordering::Greater) {
                continue;
            }
            let entity_id = u32::try_from(row).map_err(|_| TickError::EntityIdOverflow {
                rule_id: validated.rule_id,
                row,
            })?;
            let race_time = exp_f64(seed, tick, validated.rule_id, entity_id, 0, lambda);
            if race_time.partial_cmp(&model.model().dt) != Some(Ordering::Less) {
                continue;
            }
            let mut claims = Vec::with_capacity(claim_columns.len());
            for (resource_table, resources, key_column, claim) in &claim_columns {
                let ordering = match (&claim.ordering, key_column) {
                    (ClaimOrdering::RaceTime, None) => OrderingValue::RaceTime(race_time),
                    (ClaimOrdering::Key { expr }, Some(column)) => {
                        key_at(column, expr, table_index, model_box, row)?
                    }
                    _ => unreachable!("claim ordering column construction is exhaustive"),
                };
                claims.push(CandidateClaim {
                    table_index: *resource_table,
                    resource_row: resources[row],
                    ordering,
                });
            }
            candidates.push(Candidate {
                rule_id: validated.rule_id,
                table_index,
                entity_id,
                row,
                claims,
            });
        }
    }
    let resolution = resolve_claims(&candidates, model_box.tables.len(), model_box)?;
    let mut pending = Vec::new();
    for validated in &transitions {
        let transition = &model_box.transitions[validated.transition_index];
        let winner_indices: Vec<usize> = candidates
            .iter()
            .enumerate()
            .filter(|(index, candidate)| {
                candidate.rule_id == validated.rule_id && resolution.fires[*index]
            })
            .map(|(index, _)| index)
            .collect();
        if winner_indices.is_empty() {
            continue;
        }
        let table = EvalTable::new(model, &model_box.name, &transition.table)?;
        let table_index = candidates[winner_indices[0]].table_index;
        let schema = &model_box.tables[table_index];
        let mut effect_columns = Vec::with_capacity(transition.effects.len());
        for effect in &transition.effects {
            let Effect::SetAttr { attr, value } = effect;
            let attr_index = schema
                .attrs
                .iter()
                .position(|declaration| declaration.name == *attr)
                .expect("validated effect attribute disappeared");
            let destination = &schema.attrs[attr_index];
            let value = match &destination.ty {
                AttrType::Ref { .. } => PendingColumn::Ref(
                    eval_typed_ref_column(value, table, snapshot, params, &mut cache)?.values,
                ),
                _ => PendingColumn::Value(eval_column(
                    value,
                    table.with_expected_attr(attr)?,
                    snapshot,
                    params,
                    &mut cache,
                )?),
            };
            effect_columns.push((attr_index, value));
        }
        for candidate_index in winner_indices {
            let candidate = &candidates[candidate_index];
            for (attr_index, values) in &effect_columns {
                pending.push(PendingWrite {
                    box_index,
                    table_index,
                    attr_index: *attr_index,
                    row: candidate.row,
                    value: values.at(candidate.row)?,
                    rule_id: candidate.rule_id,
                    transition_name: transition.name.clone(),
                });
            }
        }
    }
    let mut fired = transitions
        .iter()
        .map(|transition| (transition.rule_id, 0))
        .collect::<Vec<_>>();
    for (candidate, fire) in candidates.iter().zip(&resolution.fires) {
        if *fire {
            let entry = fired
                .iter_mut()
                .find(|(rule_id, _)| *rule_id == candidate.rule_id)
                .expect("candidate has validated transition");
            entry.1 += 1;
        }
    }
    Ok(BoxOutcome {
        pending,
        fired,
        deferred: resolution.deferred,
        fired_per_resource_table: resolution.fired_per_resource_table,
        aggregate_builds: cache.build_count(),
    })
}

fn build_next_inputs(
    model: &ValidatedModel,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
) -> Result<Vec<InputTable>, TickError> {
    let mut inputs = model
        .model()
        .boxes
        .iter()
        .flat_map(|model_box| {
            model_box
                .inputs
                .iter()
                .map(|input| InputTable::empty(&model_box.name, &input.name, &input.schema))
        })
        .collect::<Vec<_>>();
    for wire in &model.model().wires {
        let source_box = model
            .model()
            .boxes
            .iter()
            .find(|model_box| model_box.name == wire.from.r#box)
            .expect("validated wire source box disappeared");
        let output = source_box
            .outputs
            .iter()
            .find(|output| output.name == wire.from.port)
            .expect("validated wire source port disappeared");
        let built = build_output(model, snapshot, params, source_box, output)?;
        let destination = inputs
            .iter_mut()
            .find(|input| input.box_name == wire.to.r#box && input.port_name == wire.to.port)
            .expect("validated wire destination disappeared");
        destination.row_count = built.row_count;
        destination.columns = built.columns;
    }
    Ok(inputs)
}

fn build_output(
    model: &ValidatedModel,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    model_box: &sembla_ir::Box,
    output: &sembla_ir::OutputDecl,
) -> Result<InputTable, TickError> {
    let OutputBuilder::PerTable { table, fields } = &output.builder;
    let eval_table = EvalTable::new(model, &model_box.name, table)?;
    let rows = snapshot.row_count(&model_box.name, table)?;
    let mut cache = AggCache::new(model, snapshot, params);
    let mut columns = Vec::with_capacity(fields.len());
    for field in fields {
        let selected = match &field.filter {
            Some(filter) => match eval_column(filter, eval_table, snapshot, params, &mut cache)? {
                ValueColumn::Bool(values) => values,
                other => return Err(runtime_type("output filter", &other)),
            },
            None => vec![true; rows],
        };
        let column = match &field.op {
            AggOp::Count => {
                let count = selected.iter().filter(|value| **value).count();
                ColumnData::Int(vec![i64::try_from(count).map_err(|_| {
                    TickError::Evaluation("output count exceeds i64".to_owned())
                })?])
            }
            AggOp::Sum { value } => {
                match eval_column(value, eval_table, snapshot, params, &mut cache)? {
                    ValueColumn::Real(values) => ColumnData::Real(vec![values
                        .into_iter()
                        .zip(&selected)
                        .filter(|(_, selected)| **selected)
                        .map(|(value, _)| value)
                        .fold(0.0, |sum, value| sum + value)]),
                    ValueColumn::Int(values) => {
                        let mut sum = 0_i64;
                        for (row, (value, selected)) in
                            values.into_iter().zip(&selected).enumerate()
                        {
                            if *selected {
                                sum = sum.checked_add(value).ok_or_else(|| {
                                    TickError::Evaluation(format!(
                                        "output integer sum overflow at row {row}"
                                    ))
                                })?;
                            }
                        }
                        ColumnData::Int(vec![sum])
                    }
                    other => return Err(runtime_type("output Sum", &other)),
                }
            }
        };
        columns.push(column);
    }
    Ok(InputTable {
        box_name: model_box.name.clone(),
        port_name: output.name.clone(),
        schema: output.schema.clone(),
        row_count: 1,
        columns,
    })
}

fn runtime_type(context: &str, column: &ValueColumn) -> TickError {
    let found = match column {
        ValueColumn::Real(_) => "Real",
        ValueColumn::Int(_) => "Int",
        ValueColumn::Bool(_) => "Bool",
        ValueColumn::Enum(_) => "Enum",
    };
    TickError::InvalidRuntimeType {
        context: context.to_owned(),
        found: found.to_owned(),
    }
}

fn key_at(
    column: &ValueColumn,
    expr: &Expr,
    table_index: usize,
    model_box: &sembla_ir::Box,
    row: usize,
) -> Result<OrderingValue, TickError> {
    match column {
        ValueColumn::Real(values) => Ok(OrderingValue::Real(values[row])),
        ValueColumn::Int(values) => Ok(OrderingValue::Int(values[row])),
        ValueColumn::Enum(values) => {
            let Expr::SelfAttr { name } = expr else {
                return Err(TickError::Evaluation(
                    "Enum contest key has no source attribute identity".to_owned(),
                ));
            };
            let attr_index = model_box.tables[table_index]
                .attrs
                .iter()
                .position(|attr| attr.name == *name)
                .expect("validated key attribute disappeared");
            Ok(OrderingValue::Enum {
                table_index,
                attr_index,
                value: values[row],
            })
        }
        ValueColumn::Bool(_) => Err(runtime_type("contest key", column)),
    }
}

fn resolve_claims(
    candidates: &[Candidate],
    table_count: usize,
    model_box: &sembla_ir::Box,
) -> Result<Resolution, TickError> {
    let mut instances = Vec::new();
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        for claim_index in 0..candidate.claims.len() {
            instances.push(ClaimInstance {
                candidate_index,
                claim_index,
            });
        }
    }
    instances.sort_by(|lhs, rhs| {
        let lhs_candidate = &candidates[lhs.candidate_index];
        let rhs_candidate = &candidates[rhs.candidate_index];
        let lhs_claim = &lhs_candidate.claims[lhs.claim_index];
        let rhs_claim = &rhs_candidate.claims[rhs.claim_index];
        (
            lhs_claim.table_index,
            lhs_claim.resource_row,
            lhs_candidate.rule_id,
            lhs_candidate.entity_id,
        )
            .cmp(&(
                rhs_claim.table_index,
                rhs_claim.resource_row,
                rhs_candidate.rule_id,
                rhs_candidate.entity_id,
            ))
            .then(lhs.claim_index.cmp(&rhs.claim_index))
    });

    let mut won_all = vec![true; candidates.len()];
    let mut deferred_table = vec![vec![false; table_count]; candidates.len()];
    let mut start = 0;
    while start < instances.len() {
        let first = instances[start];
        let first_claim = &candidates[first.candidate_index].claims[first.claim_index];
        let mut end = start + 1;
        while end < instances.len() {
            let claim =
                &candidates[instances[end].candidate_index].claims[instances[end].claim_index];
            if (claim.table_index, claim.resource_row)
                != (first_claim.table_index, first_claim.resource_row)
            {
                break;
            }
            end += 1;
        }
        let mut winner = first;
        for instance in &instances[start + 1..end] {
            if compare_instances(*instance, winner, candidates, model_box)? == Ordering::Less {
                winner = *instance;
            }
        }
        let winner_candidate = winner.candidate_index;
        for instance in &instances[start..end] {
            if instance.candidate_index != winner_candidate {
                won_all[instance.candidate_index] = false;
                deferred_table[instance.candidate_index][first_claim.table_index] = true;
            }
        }
        start = end;
    }

    let mut deferred = vec![0; table_count];
    let mut fired_per_resource_table = vec![0; table_count];
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        for (table_index, lost) in deferred_table[candidate_index].iter().enumerate() {
            if *lost {
                deferred[table_index] += 1;
            }
        }
        if won_all[candidate_index] {
            let mut counted = vec![false; table_count];
            for claim in &candidate.claims {
                if !counted[claim.table_index] {
                    counted[claim.table_index] = true;
                    fired_per_resource_table[claim.table_index] += 1;
                }
            }
        }
    }
    Ok(Resolution {
        fires: won_all,
        deferred,
        fired_per_resource_table,
    })
}

fn compare_instances(
    lhs: ClaimInstance,
    rhs: ClaimInstance,
    candidates: &[Candidate],
    model_box: &sembla_ir::Box,
) -> Result<Ordering, TickError> {
    let lhs_candidate = &candidates[lhs.candidate_index];
    let rhs_candidate = &candidates[rhs.candidate_index];
    let lhs_claim = &lhs_candidate.claims[lhs.claim_index];
    let rhs_claim = &rhs_candidate.claims[rhs.claim_index];
    let key_order = match (&lhs_claim.ordering, &rhs_claim.ordering) {
        (OrderingValue::RaceTime(lhs), OrderingValue::RaceTime(rhs))
        | (OrderingValue::Real(lhs), OrderingValue::Real(rhs)) => lhs.total_cmp(rhs),
        (OrderingValue::Int(lhs), OrderingValue::Int(rhs)) => lhs.cmp(rhs),
        (
            OrderingValue::Enum {
                table_index: lhs_table,
                attr_index: lhs_attr,
                value: lhs,
            },
            OrderingValue::Enum {
                table_index: rhs_table,
                attr_index: rhs_attr,
                value: rhs,
            },
        ) if enum_domains_match(model_box, *lhs_table, *lhs_attr, *rhs_table, *rhs_attr) => {
            lhs.cmp(rhs)
        }
        _ => {
            return Err(TickError::IncompatibleClaimOrdering {
                table: model_box.tables[lhs_claim.table_index].name.clone(),
                row: lhs_claim.resource_row,
            })
        }
    };
    Ok(key_order.then_with(|| {
        (lhs_candidate.rule_id, lhs_candidate.entity_id)
            .cmp(&(rhs_candidate.rule_id, rhs_candidate.entity_id))
    }))
}

fn enum_domains_match(
    model_box: &sembla_ir::Box,
    lhs_table: usize,
    lhs_attr: usize,
    rhs_table: usize,
    rhs_attr: usize,
) -> bool {
    match (
        &model_box.tables[lhs_table].attrs[lhs_attr].ty,
        &model_box.tables[rhs_table].attrs[rhs_attr].ty,
    ) {
        (
            AttrType::Enum {
                variants: lhs_variants,
            },
            AttrType::Enum {
                variants: rhs_variants,
            },
        ) => lhs_variants == rhs_variants,
        _ => false,
    }
}

enum PendingColumn {
    Value(ValueColumn),
    Ref(Vec<u32>),
}

impl PendingColumn {
    fn at(&self, row: usize) -> Result<PendingValue, TickError> {
        match self {
            Self::Value(ValueColumn::Real(values)) => Ok(PendingValue::Real(values[row])),
            Self::Value(ValueColumn::Int(values)) => Ok(PendingValue::Int(values[row])),
            Self::Value(ValueColumn::Enum(values)) => Ok(PendingValue::Enum(values[row])),
            Self::Ref(values) => Ok(PendingValue::Ref(values[row])),
            Self::Value(ValueColumn::Bool(_)) => Err(TickError::InvalidRuntimeType {
                context: "effect value".to_owned(),
                found: "Bool".to_owned(),
            }),
        }
    }
}

fn detect_double_writes(pending: &[PendingWrite], model: &ValidatedModel) -> Result<(), TickError> {
    let mut order: Vec<usize> = (0..pending.len()).collect();
    order.sort_by_key(|index| {
        let write = &pending[*index];
        (
            write.box_index,
            write.table_index,
            write.attr_index,
            write.row,
        )
    });
    for pair in order.windows(2) {
        let first = &pending[pair[0]];
        let second = &pending[pair[1]];
        if (
            first.box_index,
            first.table_index,
            first.attr_index,
            first.row,
        ) == (
            second.box_index,
            second.table_index,
            second.attr_index,
            second.row,
        ) {
            let model_box = &model.model().boxes[first.box_index];
            return Err(TickError::DoubleWrite {
                box_name: model_box.name.clone().into_boxed_str(),
                table: model_box.tables[first.table_index]
                    .name
                    .clone()
                    .into_boxed_str(),
                attr: model_box.tables[first.table_index].attrs[first.attr_index]
                    .name
                    .clone()
                    .into_boxed_str(),
                row: first.row,
                first_rule_id: first.rule_id,
                first_transition: first.transition_name.clone().into_boxed_str(),
                second_rule_id: second.rule_id,
                second_transition: second.transition_name.clone().into_boxed_str(),
            });
        }
    }
    Ok(())
}
