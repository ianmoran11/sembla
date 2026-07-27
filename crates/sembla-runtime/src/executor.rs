//! Deterministic, snapshot-isolated synchronous box composition.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sembla_ir::{
    AggOp, AttrType, ClaimOrdering, Effect, Expr, FeatureSet, OutputBuilder, SummaryReduce,
    ValidatedModel, ViewReduce, GROUPED_OBSERVATIONS_FEATURE,
};

use crate::eval::{
    eval_column, eval_typed_ref_column, prepare_row_expr, tick_tile_rows, tick_tiling_enabled,
    tick_worker_count, AggCache, EvalError, EvalTable, ParamEnv, PreparedColumn, PreparedExpr,
    PreparedValue, ValueColumn,
};
use crate::rng::{exp_f64, exp_f64_from_uniform, uniform_f64};
use crate::state::{ColumnData, InputTable, Snapshot, StateError, StateStore};

/// Relative slack below the `exp(-lambda * dt)` boundary used only to reject
/// certain non-firers. Near the benchmark's thresholds, one binary64 ULP is at
/// most `2^-52` relative; `1e-12` is about 4,500 ULPs. That envelope dominates
/// the documented one-ULP platform `exp`/`ln` disagreement plus the handful of
/// rounding steps that form the threshold. Candidates inside the envelope are
/// still decided by the canonical platform-`ln` racing clock.
const RACING_CLOCK_FILTER_RELATIVE_MARGIN: f64 = 1e-12;

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

/// Observable result of one committed tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TickReport {
    pub tick: u32,
    /// View values in box order and then view declaration order.
    pub views: Vec<ViewValue>,
    /// Non-empty grouped buckets, sorted by view declaration then numeric key tuple.
    pub grouped_views: Vec<GroupedViewValue>,
    /// Model-global rule counts, retained for single-box API compatibility.
    pub fired: Vec<(u32, usize)>,
    /// Counts grouped in box declaration order for composed-model reporting.
    pub fired_per_box: Vec<(String, Vec<(u32, usize)>)>,
    pub deferred_per_resource_table: Vec<(String, usize)>,
    /// PRD 0005 group-by accumulators built across all boxes for this tick.
    /// A cached aggregate contributes once regardless of querying row count.
    pub aggregate_builds: usize,
}

/// Per-phase wall durations for one instrumented CPU tick.
#[derive(Clone, Copy, Debug)]
pub struct TickPhaseDurations {
    pub execute_tick: std::time::Duration,
    pub observe_views: std::time::Duration,
    pub report: std::time::Duration,
}

/// One CPU tick report paired with its per-phase instrumentation.
#[derive(Clone, Debug)]
pub struct TimedTickReport {
    pub report: TickReport,
    pub phases: TickPhaseDurations,
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

#[derive(Clone, Copy, Debug)]
struct RacingClockFilter {
    lo: f64,
}

impl RacingClockFilter {
    fn for_hazard(lambda: f64, dt: f64) -> Option<Self> {
        if lambda.partial_cmp(&0.0) != Some(Ordering::Greater) {
            return None;
        }
        let threshold = (-(lambda * dt)).exp();
        let lo = threshold * (1.0 - RACING_CLOCK_FILTER_RELATIVE_MARGIN);
        (!lo.is_nan()).then_some(Self { lo })
    }

    fn admits(self, uniform: f64) -> bool {
        uniform.partial_cmp(&self.lo) != Some(Ordering::Less)
    }
}

fn has_constant_hazard(expr: &Expr) -> bool {
    matches!(expr, Expr::Real { .. } | Expr::Param { .. })
}

#[derive(Clone, Copy, Debug)]
struct RacingClockCoordinates {
    seed: u64,
    tick: u32,
    rule_id: u32,
    rule_word: u32,
    row: usize,
}

/// Converts the row before consulting the filter so an enabled candidate keeps
/// the canonical `EntityIdOverflow` path even when its draw would be rejected.
fn candidate_race_time(
    coordinates: RacingClockCoordinates,
    lambda: f64,
    dt: f64,
    filter: Option<RacingClockFilter>,
) -> Result<Option<(u32, f64)>, TickError> {
    let entity_id = u32::try_from(coordinates.row).map_err(|_| TickError::EntityIdOverflow {
        rule_id: coordinates.rule_id,
        row: coordinates.row,
    })?;
    let race_time = if let Some(filter) = filter {
        let uniform = uniform_f64(
            coordinates.seed,
            coordinates.tick,
            coordinates.rule_word,
            entity_id,
            0,
        );
        if !filter.admits(uniform) {
            return Ok(None);
        }
        exp_f64_from_uniform(uniform, lambda)
    } else {
        exp_f64(
            coordinates.seed,
            coordinates.tick,
            coordinates.rule_word,
            entity_id,
            0,
            lambda,
        )
    };
    Ok((race_time.partial_cmp(&dt) == Some(Ordering::Less)).then_some((entity_id, race_time)))
}

#[derive(Clone, Debug)]
struct Candidate {
    /// Dense ordinal retained for transition lookup, reports, and diagnostics.
    rule_id: u32,
    /// Stable runtime identity used only for Philox and conflict tie-breaks.
    rule_word: u32,
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

struct PreparedTransition<'state> {
    box_index: usize,
    transition_index: usize,
    rule_id: u32,
    rule_word: u32,
    table_index: usize,
    row_count: usize,
    guard: PreparedExpr<'state>,
    hazard: PreparedExpr<'state>,
    race_filter: Option<RacingClockFilter>,
    claims: Vec<PreparedClaim<'state>>,
}

struct PreparedClaim<'state> {
    resource_table_index: usize,
    resource: PreparedExpr<'state>,
    ordering: PreparedClaimOrdering<'state>,
}

enum PreparedClaimOrdering<'state> {
    RaceTime,
    Key {
        expr: PreparedExpr<'state>,
        enum_identity: Option<(usize, usize)>,
    },
}

struct TileTask {
    plan_indices: Vec<usize>,
    start: usize,
    end: usize,
}

struct TilePlanOutput {
    plan_index: usize,
    candidates: Vec<Candidate>,
    error: Option<TickError>,
}

/// Executes and commits one deterministic, snapshot-isolated tick.
pub fn run_tick(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
) -> Result<TickReport, TickError> {
    run_tick_with_features(model, state, params, seed, tick, &FeatureSet::new())
}

pub fn run_tick_with_features(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
    enabled_features: &FeatureSet,
) -> Result<TickReport, TickError> {
    require_grouped_observations_feature(model, enabled_features)?;
    Ok(execute_tick(model, state, params, seed, tick)?.report)
}

/// Executes one CPU tick while measuring only the named per-tick phase
/// boundaries. The ordinary run path calls [`run_tick_with_features`] and
/// allocates no timer state.
pub fn run_tick_with_features_timed(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
    enabled_features: &FeatureSet,
) -> Result<TimedTickReport, TickError> {
    require_grouped_observations_feature(model, enabled_features)?;

    let started = std::time::Instant::now();
    let box_outcomes = execute_tick_state(model, state, params, seed, tick)?;
    let execute_tick = started.elapsed();

    let started = std::time::Instant::now();
    let (views, grouped_views) = observe_tick(model, state, params)?;
    let observe_views = started.elapsed();

    let started = std::time::Instant::now();
    let outcome = finish_tick(model, tick, box_outcomes, views, grouped_views);
    let report = started.elapsed();

    Ok(TimedTickReport {
        report: outcome.report,
        phases: TickPhaseDurations {
            execute_tick,
            observe_views,
            report,
        },
    })
}

/// Executes ticks `0..n_ticks` and records strict saturation warnings.
pub fn run(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    n_ticks: u32,
) -> Result<RunReport, TickError> {
    run_with_features(model, state, params, seed, n_ticks, &FeatureSet::new())
}

pub fn run_with_features(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    n_ticks: u32,
    enabled_features: &FeatureSet,
) -> Result<RunReport, TickError> {
    require_grouped_observations_feature(model, enabled_features)?;
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

fn require_grouped_observations_feature(
    model: &ValidatedModel,
    enabled_features: &FeatureSet,
) -> Result<(), TickError> {
    if model
        .model()
        .boxes
        .iter()
        .any(|model_box| !model_box.grouped_views.is_empty())
        && !enabled_features.contains(GROUPED_OBSERVATIONS_FEATURE)
    {
        return Err(TickError::Evaluation(format!(
            "grouped_views require enabled feature '{GROUPED_OBSERVATIONS_FEATURE}'"
        )));
    }
    Ok(())
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
    let box_outcomes = execute_tick_state(model, state, params, seed, tick)?;
    let (views, grouped_views) = observe_tick(model, state, params)?;
    Ok(finish_tick(model, tick, box_outcomes, views, grouped_views))
}

fn prepare_tiled_transition<'state>(
    model: &ValidatedModel,
    box_index: usize,
    transition_index: usize,
    snapshot: &'state Snapshot<'_>,
    params: &ParamEnv,
) -> Result<Option<PreparedTransition<'state>>, TickError> {
    let model_box = &model.model().boxes[box_index];
    let transition = &model_box.transitions[transition_index];
    let validated = model
        .transitions()
        .iter()
        .find(|candidate| {
            candidate.box_index == box_index && candidate.transition_index == transition_index
        })
        .expect("validated transition disappeared");
    let table_index = model_box
        .tables
        .iter()
        .position(|table| table.name == transition.table)
        .expect("validated transition table disappeared");
    let row_count = snapshot.row_count(&model_box.name, &transition.table)?;
    if !tick_tiling_enabled(row_count) {
        return Ok(None);
    }
    let table = EvalTable::new(model, &model_box.name, &transition.table)?;
    let Some(guard) = prepare_row_expr(&transition.guard, table, snapshot, params)? else {
        return Ok(None);
    };
    let Some(hazard) = prepare_row_expr(&transition.hazard, table, snapshot, params)? else {
        return Ok(None);
    };
    let mut claims = Vec::with_capacity(transition.contests.len());
    for claim in &transition.contests {
        let Some(resource) = prepare_row_expr(&claim.resource, table, snapshot, params)? else {
            return Ok(None);
        };
        let target_table = resource.ref_target().ok_or_else(|| {
            TickError::Evaluation("contest resource did not prepare as Ref".to_owned())
        })?;
        let resource_table_index = model_box
            .tables
            .iter()
            .position(|schema| schema.name == target_table)
            .expect("validated Ref target table disappeared");
        let ordering = match &claim.ordering {
            ClaimOrdering::RaceTime => PreparedClaimOrdering::RaceTime,
            ClaimOrdering::Key { expr } => {
                let Some(prepared) = prepare_row_expr(expr, table, snapshot, params)? else {
                    return Ok(None);
                };
                let enum_identity = if let Expr::SelfAttr { name } = expr {
                    model_box.tables[table_index]
                        .attrs
                        .iter()
                        .position(|attr| attr.name == *name)
                        .filter(|attr_index| {
                            matches!(
                                model_box.tables[table_index].attrs[*attr_index].ty,
                                AttrType::Enum { .. }
                            )
                        })
                        .map(|attr_index| (table_index, attr_index))
                } else {
                    None
                };
                PreparedClaimOrdering::Key {
                    expr: prepared,
                    enum_identity,
                }
            }
        };
        claims.push(PreparedClaim {
            resource_table_index,
            resource,
            ordering,
        });
    }
    // Direct literals and parameters are the deliberately narrow row-invariant
    // fragment. Compute their filter only after all eager preparation succeeds,
    // preserving declaration-ordered expression errors.
    let race_filter = if row_count != 0 && has_constant_hazard(&transition.hazard) {
        let PreparedColumn::Real(values) = hazard.tile(0, 1)? else {
            return Err(TickError::Evaluation(
                "transition hazard did not prepare as Real".to_owned(),
            ));
        };
        RacingClockFilter::for_hazard(values[0], model.model().dt)
    } else {
        None
    };
    Ok(Some(PreparedTransition {
        box_index,
        transition_index,
        rule_id: validated.rule_id,
        rule_word: validated.rule_word,
        table_index,
        row_count,
        guard,
        hazard,
        race_filter,
        claims,
    }))
}

fn prepared_ordering_value(
    value: PreparedValue,
    enum_identity: Option<(usize, usize)>,
) -> Result<OrderingValue, TickError> {
    match value {
        PreparedValue::Real(value) => Ok(OrderingValue::Real(value)),
        PreparedValue::Int(value) => Ok(OrderingValue::Int(value)),
        PreparedValue::Enum(value) => {
            let (table_index, attr_index) = enum_identity.ok_or_else(|| {
                TickError::Evaluation(
                    "Enum contest key has no source attribute identity".to_owned(),
                )
            })?;
            Ok(OrderingValue::Enum {
                table_index,
                attr_index,
                value,
            })
        }
        PreparedValue::Bool | PreparedValue::Ref => Err(TickError::InvalidRuntimeType {
            context: "contest key".to_owned(),
            found: match value {
                PreparedValue::Bool => "Bool",
                PreparedValue::Ref => "Ref",
                _ => unreachable!(),
            }
            .to_owned(),
        }),
    }
}

fn evaluate_tile_task(
    task: &TileTask,
    plans: &[PreparedTransition<'_>],
    seed: u64,
    tick: u32,
    dt: f64,
) -> Vec<TilePlanOutput> {
    task.plan_indices
        .iter()
        .map(|plan_index| {
            let plan = &plans[*plan_index];
            let evaluated = (|| -> Result<Vec<Candidate>, TickError> {
                // Dispatch each expression node once per tile, not once per
                // row. Every root is still evaluated eagerly before racing.
                let guard = plan.guard.tile(task.start, task.end)?;
                let hazard = plan.hazard.tile(task.start, task.end)?;
                let mut claim_columns = Vec::with_capacity(plan.claims.len());
                for claim in &plan.claims {
                    let resource = claim.resource.tile(task.start, task.end)?;
                    let ordering = match &claim.ordering {
                        PreparedClaimOrdering::RaceTime => None,
                        PreparedClaimOrdering::Key { expr, .. } => {
                            Some(expr.tile(task.start, task.end)?)
                        }
                    };
                    claim_columns.push((resource, ordering));
                }
                let PreparedColumn::Bool(guards) = guard else {
                    return Err(TickError::Evaluation(
                        "transition guard did not prepare as Bool".to_owned(),
                    ));
                };
                let PreparedColumn::Real(hazards) = hazard else {
                    return Err(TickError::Evaluation(
                        "transition hazard did not prepare as Real".to_owned(),
                    ));
                };
                let mut candidates = Vec::new();
                for (offset, (guard, lambda)) in guards
                    .iter()
                    .copied()
                    .zip(hazards.iter().copied())
                    .enumerate()
                {
                    if !guard || lambda.partial_cmp(&0.0) != Some(Ordering::Greater) {
                        continue;
                    }
                    let row = task.start + offset;
                    let Some((entity_id, race_time)) = candidate_race_time(
                        RacingClockCoordinates {
                            seed,
                            tick,
                            rule_id: plan.rule_id,
                            rule_word: plan.rule_word,
                            row,
                        },
                        lambda,
                        dt,
                        plan.race_filter,
                    )?
                    else {
                        continue;
                    };
                    let mut claims = Vec::with_capacity(plan.claims.len());
                    for (claim, (resource, ordering)) in plan.claims.iter().zip(&claim_columns) {
                        let PreparedColumn::Ref(resources) = resource else {
                            return Err(TickError::Evaluation(
                                "contest resource did not prepare as Ref".to_owned(),
                            ));
                        };
                        let ordering = match (&claim.ordering, ordering) {
                            (PreparedClaimOrdering::RaceTime, None) => {
                                OrderingValue::RaceTime(race_time)
                            }
                            (PreparedClaimOrdering::Key { enum_identity, .. }, Some(values)) => {
                                let value = match values {
                                    PreparedColumn::Real(values) => {
                                        PreparedValue::Real(values[offset])
                                    }
                                    PreparedColumn::Int(values) => {
                                        PreparedValue::Int(values[offset])
                                    }
                                    PreparedColumn::Enum(values) => {
                                        PreparedValue::Enum(values[offset])
                                    }
                                    PreparedColumn::Bool(_) => PreparedValue::Bool,
                                    PreparedColumn::Ref(_) => PreparedValue::Ref,
                                };
                                prepared_ordering_value(value, *enum_identity)?
                            }
                            _ => unreachable!("prepared claim ordering is exhaustive"),
                        };
                        claims.push(CandidateClaim {
                            table_index: claim.resource_table_index,
                            resource_row: resources[offset],
                            ordering,
                        });
                    }
                    candidates.push(Candidate {
                        rule_id: plan.rule_id,
                        rule_word: plan.rule_word,
                        table_index: plan.table_index,
                        entity_id,
                        row,
                        claims,
                    });
                }
                Ok(candidates)
            })();
            match evaluated {
                Ok(candidates) => TilePlanOutput {
                    plan_index: *plan_index,
                    candidates,
                    error: None,
                },
                Err(error) => TilePlanOutput {
                    plan_index: *plan_index,
                    candidates: Vec::new(),
                    error: Some(error),
                },
            }
        })
        .collect()
}

type TiledCandidateResults = Vec<Vec<Option<Result<Vec<Candidate>, TickError>>>>;

/// Prepares every eligible transition, then opens at most one parallel region
/// for the tick. Fixed task boundaries are `(table, tile_start, tile_end)` and
/// depend only on stable model indices, row count, and tile size. Worker count
/// changes only which worker receives a complete fixed task.
fn prepare_tiled_candidates(
    model: &ValidatedModel,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
) -> TiledCandidateResults {
    let mut results = model
        .model()
        .boxes
        .iter()
        .map(|model_box| {
            std::iter::repeat_with(|| None)
                .take(model_box.transitions.len())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut plans = Vec::new();
    for (box_index, model_box) in model.model().boxes.iter().enumerate() {
        for transition_index in 0..model_box.transitions.len() {
            match prepare_tiled_transition(model, box_index, transition_index, snapshot, params) {
                Ok(Some(plan)) => plans.push(plan),
                Ok(None) => {}
                Err(error) => results[box_index][transition_index] = Some(Err(error)),
            }
        }
    }
    if plans.is_empty() {
        return results;
    }

    let mut groups: Vec<(usize, usize, usize, Vec<usize>)> = Vec::new();
    for (plan_index, plan) in plans.iter().enumerate() {
        if let Some((_, _, _, indices)) =
            groups.iter_mut().find(|(box_index, table_index, _, _)| {
                (*box_index, *table_index) == (plan.box_index, plan.table_index)
            })
        {
            indices.push(plan_index);
        } else {
            groups.push((
                plan.box_index,
                plan.table_index,
                plan.row_count,
                vec![plan_index],
            ));
        }
    }
    let tile_rows = tick_tile_rows();
    let mut tasks = Vec::new();
    for (_, _, row_count, plan_indices) in groups {
        for start in (0..row_count).step_by(tile_rows) {
            tasks.push(TileTask {
                plan_indices: plan_indices.clone(),
                start,
                end: (start + tile_rows).min(row_count),
            });
        }
    }

    let worker_count = tick_worker_count().max(1).min(tasks.len().max(1));
    let mut task_outputs = std::iter::repeat_with(|| None)
        .take(tasks.len())
        .collect::<Vec<_>>();
    if worker_count == 1 {
        for (task_index, task) in tasks.iter().enumerate() {
            task_outputs[task_index] = Some(evaluate_tile_task(
                task,
                &plans,
                seed,
                tick,
                model.model().dt,
            ));
        }
    } else {
        let mut worker_tasks = std::iter::repeat_with(Vec::new)
            .take(worker_count)
            .collect::<Vec<Vec<usize>>>();
        for task_index in 0..tasks.len() {
            worker_tasks[task_index % worker_count].push(task_index);
        }
        let worker_outputs = std::thread::scope(|scope| {
            let handles = worker_tasks
                .into_iter()
                .map(|task_indices| {
                    let tasks = &tasks;
                    let plans = &plans;
                    scope.spawn(move || {
                        task_indices
                            .into_iter()
                            .map(|task_index| {
                                (
                                    task_index,
                                    evaluate_tile_task(
                                        &tasks[task_index],
                                        plans,
                                        seed,
                                        tick,
                                        model.model().dt,
                                    ),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().expect("tile worker panicked"))
                .collect::<Vec<_>>()
        });
        for (task_index, output) in worker_outputs {
            task_outputs[task_index] = Some(output);
        }
    }

    for outputs in task_outputs.into_iter().map(Option::unwrap) {
        for output in outputs {
            let plan = &plans[output.plan_index];
            let slot = &mut results[plan.box_index][plan.transition_index];
            if let Some(error) = output.error {
                if slot.is_none() {
                    *slot = Some(Err(error));
                }
            } else if !matches!(slot, Some(Err(_))) {
                match slot {
                    Some(Ok(candidates)) => candidates.extend(output.candidates),
                    None => *slot = Some(Ok(output.candidates)),
                    Some(Err(_)) => unreachable!(),
                }
            }
        }
    }
    results
}

fn execute_tick_state(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
) -> Result<Vec<BoxOutcome>, TickError> {
    let snapshot = state.snapshot();
    let mut tiled_candidates = prepare_tiled_candidates(model, &snapshot, params, seed, tick);
    let mut box_outcomes = Vec::with_capacity(model.model().boxes.len());
    for (box_index, candidates) in tiled_candidates.iter_mut().enumerate() {
        box_outcomes.push(stage_box(
            model, box_index, &snapshot, params, seed, tick, candidates,
        )?);
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
    Ok(box_outcomes)
}

fn observe_tick(
    model: &ValidatedModel,
    state: &StateStore,
    params: &ParamEnv,
) -> Result<(Vec<ViewValue>, Vec<GroupedViewValue>), TickError> {
    // Observation is deliberately evaluated only after commit and receives an
    // immutable store. It cannot consume RNG coordinates, stage writes, or
    // influence conflict resolution or scheduling.
    Ok((
        observe_views(model, state, params)?,
        observe_grouped_views(model, state, params)?,
    ))
}

fn finish_tick(
    model: &ValidatedModel,
    tick: u32,
    box_outcomes: Vec<BoxOutcome>,
    views: Vec<ViewValue>,
    grouped_views: Vec<GroupedViewValue>,
) -> TickOutcome {
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

    TickOutcome {
        report: TickReport {
            tick,
            views,
            grouped_views,
            fired,
            fired_per_box,
            deferred_per_resource_table,
            aggregate_builds,
        },
        fired_per_resource_table,
    }
}

struct PreparedCountView<'state> {
    ordinal: usize,
    box_index: usize,
    view_index: usize,
    table_index: usize,
    row_count: usize,
    filter: Option<PreparedExpr<'state>>,
}

/// Tiles observation-only count filters on the calling thread. This is a second
/// row-wise phase but not a second parallel region: committed views cannot share
/// the tick-start snapshot used by transition staging. Numeric views, including
/// every `f64` reduction, retain the original column-wise ascending-row path.
fn prepare_tiled_count_views(
    model: &ValidatedModel,
    snapshot: &Snapshot<'_>,
    params: &ParamEnv,
) -> Vec<Option<Result<ObservationValue, TickError>>> {
    let view_count = model
        .model()
        .boxes
        .iter()
        .map(|model_box| model_box.views.len())
        .sum();
    let mut results = std::iter::repeat_with(|| None)
        .take(view_count)
        .collect::<Vec<_>>();
    let mut plans = Vec::new();
    let mut ordinal = 0;
    for (box_index, model_box) in model.model().boxes.iter().enumerate() {
        for (view_index, view) in model_box.views.iter().enumerate() {
            if view.reduce != ViewReduce::Count {
                ordinal += 1;
                continue;
            }
            let table_index = model_box
                .tables
                .iter()
                .position(|table| table.name == view.table)
                .expect("validated view table disappeared");
            let row_count = match snapshot.row_count(&model_box.name, &view.table) {
                Ok(row_count) => row_count,
                Err(error) => {
                    results[ordinal] = Some(Err(error.into()));
                    ordinal += 1;
                    continue;
                }
            };
            if !tick_tiling_enabled(row_count) {
                ordinal += 1;
                continue;
            }
            let table = match EvalTable::new(model, &model_box.name, &view.table) {
                Ok(table) => table,
                Err(error) => {
                    results[ordinal] = Some(Err(error.into()));
                    ordinal += 1;
                    continue;
                }
            };
            let filter = match &view.filter {
                Some(filter) => match prepare_row_expr(filter, table, snapshot, params) {
                    Ok(Some(filter)) => Some(filter),
                    Ok(None) => {
                        ordinal += 1;
                        continue;
                    }
                    Err(error) => {
                        results[ordinal] = Some(Err(error.into()));
                        ordinal += 1;
                        continue;
                    }
                },
                None => None,
            };
            plans.push(PreparedCountView {
                ordinal,
                box_index,
                view_index,
                table_index,
                row_count,
                filter,
            });
            ordinal += 1;
        }
    }

    let mut groups: Vec<(usize, usize, usize, Vec<usize>)> = Vec::new();
    for (plan_index, plan) in plans.iter().enumerate() {
        if let Some((_, _, _, indices)) =
            groups.iter_mut().find(|(box_index, table_index, _, _)| {
                (*box_index, *table_index) == (plan.box_index, plan.table_index)
            })
        {
            indices.push(plan_index);
        } else {
            groups.push((
                plan.box_index,
                plan.table_index,
                plan.row_count,
                vec![plan_index],
            ));
        }
    }
    let mut counts = vec![0_usize; plans.len()];
    let tile_rows = tick_tile_rows();
    for (_, _, row_count, plan_indices) in groups {
        for start in (0..row_count).step_by(tile_rows) {
            let end = (start + tile_rows).min(row_count);
            for plan_index in &plan_indices {
                let plan = &plans[*plan_index];
                if matches!(results[plan.ordinal], Some(Err(_))) {
                    continue;
                }
                let selected = match &plan.filter {
                    Some(filter) => match filter.tile(start, end) {
                        Ok(PreparedColumn::Bool(selected)) => selected,
                        Ok(_) => {
                            results[plan.ordinal] = Some(Err(TickError::Evaluation(
                                "view filter did not prepare as Bool".to_owned(),
                            )));
                            continue;
                        }
                        Err(error) => {
                            results[plan.ordinal] = Some(Err(error.into()));
                            continue;
                        }
                    },
                    None => vec![true; end - start].into(),
                };
                counts[*plan_index] += selected.iter().filter(|value| **value).count();
            }
        }
    }
    for (plan_index, plan) in plans.iter().enumerate() {
        if matches!(results[plan.ordinal], Some(Err(_))) {
            continue;
        }
        let model_box = &model.model().boxes[plan.box_index];
        let view = &model_box.views[plan.view_index];
        let count = i64::try_from(counts[plan_index]).map_err(|_| {
            TickError::Evaluation(format!(
                "view '{}.{}' count exceeds i64",
                model_box.name, view.name
            ))
        });
        results[plan.ordinal] = Some(count.map(ObservationValue::Int));
    }
    results
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
    let mut tiled_counts = prepare_tiled_count_views(model, &snapshot, params);
    let mut cache = AggCache::new(model, &snapshot, params);
    let mut observations = Vec::new();
    let mut view_ordinal = 0;
    for model_box in &model.model().boxes {
        for view in &model_box.views {
            let tiled_value = tiled_counts[view_ordinal].take();
            view_ordinal += 1;
            let value = if let Some(value) = tiled_value {
                value?
            } else {
                let table = EvalTable::new(model, &model_box.name, &view.table)?;
                let row_count = snapshot.row_count(&model_box.name, &view.table)?;
                let selected = match &view.filter {
                    Some(filter) => {
                        match eval_column(filter, table, &snapshot, params, &mut cache)? {
                            ValueColumn::Bool(values) => values,
                            other => return Err(runtime_type("view filter", &other)),
                        }
                    }
                    None => vec![true; row_count],
                };
                match view.reduce {
                    ViewReduce::Count => ObservationValue::Int(
                        i64::try_from(selected.iter().filter(|selected| **selected).count())
                            .map_err(|_| {
                                TickError::Evaluation(format!(
                                    "view '{}.{}' count exceeds i64",
                                    model_box.name, view.name
                                ))
                            })?,
                    ),
                    ViewReduce::Sum | ViewReduce::Min | ViewReduce::Max => {
                        let expression = view
                            .value
                            .as_ref()
                            .expect("validated numeric view has a value");
                        let column = eval_column(expression, table, &snapshot, params, &mut cache)?;
                        reduce_view_column(
                            &model_box.name,
                            &view.name,
                            view.reduce,
                            column,
                            &selected,
                        )?
                    }
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

/// Evaluates grouped count views from committed state without execution feedback.
pub fn observe_grouped_views(
    model: &ValidatedModel,
    state: &StateStore,
    params: &ParamEnv,
) -> Result<Vec<GroupedViewValue>, TickError> {
    let snapshot = state.snapshot();
    let mut cache = AggCache::new(model, &snapshot, params);
    let mut observations = Vec::new();
    for model_box in &model.model().boxes {
        for view in &model_box.grouped_views {
            let table_decl = model_box
                .tables
                .iter()
                .find(|table| table.name == view.table)
                .expect("validated grouped view table disappeared");
            let table = EvalTable::new(model, &model_box.name, &view.table)?;
            let row_count = snapshot.row_count(&model_box.name, &view.table)?;
            let selected = match &view.filter {
                Some(filter) => match eval_column(filter, table, &snapshot, params, &mut cache)? {
                    ValueColumn::Bool(values) => values,
                    other => return Err(runtime_type("grouped view filter", &other)),
                },
                None => vec![true; row_count],
            };
            let mut buckets: BTreeMap<Vec<i128>, usize> = BTreeMap::new();
            for (row, selected) in selected.into_iter().enumerate() {
                if !selected {
                    continue;
                }
                let mut tuple = Vec::with_capacity(view.keys.len());
                for key in &view.keys {
                    let attr = table_decl
                        .attrs
                        .iter()
                        .find(|attr| attr.name == key.attr)
                        .expect("validated grouped key disappeared");
                    let value = match (&attr.ty, key.band_width) {
                        (AttrType::Enum { .. }, None) => i128::from(snapshot.enum_index(
                            &model_box.name,
                            &view.table,
                            &key.attr,
                            row,
                        )?),
                        (AttrType::Ref { .. }, None) => i128::from(snapshot.reference(
                            &model_box.name,
                            &view.table,
                            &key.attr,
                            row,
                        )?),
                        (AttrType::Int, Some(width)) => i128::from(snapshot.int(
                            &model_box.name,
                            &view.table,
                            &key.attr,
                            row,
                        )?)
                        .div_euclid(i128::from(width)),
                        _ => unreachable!("validated grouped key type disappeared"),
                    };
                    tuple.push(value);
                }
                *buckets.entry(tuple).or_default() += 1;
            }
            observations.extend(buckets.into_iter().map(|(keys, count)| GroupedViewValue {
                box_name: model_box.name.clone(),
                name: view.name.clone(),
                keys,
                count,
            }));
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
    tiled_candidates: &mut [Option<Result<Vec<Candidate>, TickError>>],
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
        if let Some(result) = tiled_candidates[validated.transition_index].take() {
            candidates.extend(result?);
            continue;
        }
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
        let race_filter = if has_constant_hazard(&transition.hazard) {
            hazards
                .first()
                .and_then(|lambda| RacingClockFilter::for_hazard(*lambda, model.model().dt))
        } else {
            None
        };
        let mut push_candidate =
            |row: usize, entity_id: u32, race_time: f64| -> Result<(), TickError> {
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
                    rule_word: validated.rule_word,
                    table_index,
                    entity_id,
                    row,
                    claims,
                });
                Ok(())
            };
        for (row, (guard, lambda)) in guards.into_iter().zip(hazards).enumerate() {
            if !guard || lambda.partial_cmp(&0.0) != Some(Ordering::Greater) {
                continue;
            }
            let Some((entity_id, race_time)) = candidate_race_time(
                RacingClockCoordinates {
                    seed,
                    tick,
                    rule_id: validated.rule_id,
                    rule_word: validated.rule_word,
                    row,
                },
                lambda,
                model.model().dt,
                race_filter,
            )?
            else {
                continue;
            };
            push_candidate(row, entity_id, race_time)?;
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
            lhs_candidate.rule_word,
            lhs_candidate.entity_id,
        )
            .cmp(&(
                rhs_claim.table_index,
                rhs_claim.resource_row,
                rhs_candidate.rule_word,
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
        (lhs_candidate.rule_word, lhs_candidate.entity_id)
            .cmp(&(rhs_candidate.rule_word, rhs_candidate.entity_id))
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

#[cfg(test)]
mod parallel_tests {
    use super::*;
    use crate::eval::with_test_tick_tiles;
    use crate::state::{ColumnInit, StateStore, TableInit};
    use sembla_ir::{
        validate, Attr, Box as ModelBox, Model, ParamDecl, ParamType, ParamValue, ResourceClaim,
        Table, Transition, ViewDecl,
    };

    fn tiled_fixture(row_count: usize) -> (ValidatedModel, StateStore) {
        let model = validate(Model {
            name: "tile-determinism".to_owned(),
            dt: 5.0,
            params: Vec::new(),
            boxes: vec![ModelBox {
                name: "world".to_owned(),
                tables: vec![
                    Table {
                        name: "Resource".to_owned(),
                        size_hint: 1_024,
                        attrs: Vec::new(),
                    },
                    Table {
                        name: "KeyResource".to_owned(),
                        size_hint: 1_024,
                        attrs: Vec::new(),
                    },
                    Table {
                        name: "Person".to_owned(),
                        size_hint: row_count as u64,
                        attrs: vec![
                            Attr {
                                name: "x".to_owned(),
                                ty: AttrType::Real,
                            },
                            Attr {
                                name: "resource".to_owned(),
                                ty: AttrType::Ref {
                                    table: "Resource".to_owned(),
                                },
                            },
                            Attr {
                                name: "key_resource".to_owned(),
                                ty: AttrType::Ref {
                                    table: "KeyResource".to_owned(),
                                },
                            },
                        ],
                    },
                ],
                transitions: vec![
                    Transition {
                        name: "race".to_owned(),
                        table: "Person".to_owned(),
                        guard: Expr::Gt {
                            lhs: Box::new(Expr::SelfAttr {
                                name: "x".to_owned(),
                            }),
                            rhs: Box::new(Expr::Real { value: -0.5 }),
                        },
                        hazard: Expr::Div {
                            lhs: Box::new(Expr::Add {
                                lhs: Box::new(Expr::Mul {
                                    lhs: Box::new(Expr::SelfAttr {
                                        name: "x".to_owned(),
                                    }),
                                    rhs: Box::new(Expr::Real { value: 0.000_001 }),
                                }),
                                rhs: Box::new(Expr::Real { value: 0.001 }),
                            }),
                            rhs: Box::new(Expr::Real { value: 3.0 }),
                        },
                        effects: Vec::new(),
                        contests: vec![ResourceClaim {
                            resource: Expr::SelfAttr {
                                name: "resource".to_owned(),
                            },
                            ordering: ClaimOrdering::RaceTime,
                        }],
                    },
                    Transition {
                        name: "key".to_owned(),
                        table: "Person".to_owned(),
                        guard: Expr::Bool { value: true },
                        hazard: Expr::Real { value: 0.001 },
                        effects: Vec::new(),
                        contests: vec![ResourceClaim {
                            resource: Expr::SelfAttr {
                                name: "key_resource".to_owned(),
                            },
                            ordering: ClaimOrdering::Key {
                                expr: Expr::SelfAttr {
                                    name: "x".to_owned(),
                                },
                            },
                        }],
                    },
                ],
                inputs: Vec::new(),
                outputs: Vec::new(),
                views: vec![ViewDecl {
                    name: "positive_x".to_owned(),
                    table: "Person".to_owned(),
                    filter: Some(Expr::Gt {
                        lhs: Box::new(Expr::Div {
                            lhs: Box::new(Expr::Add {
                                lhs: Box::new(Expr::SelfAttr {
                                    name: "x".to_owned(),
                                }),
                                rhs: Box::new(Expr::Real { value: 0.25 }),
                            }),
                            rhs: Box::new(Expr::Real { value: 3.0 }),
                        }),
                        rhs: Box::new(Expr::Real { value: 0.1 }),
                    }),
                    value: None,
                    reduce: ViewReduce::Count,
                }],
                grouped_views: Vec::new(),
            }],
            wires: Vec::new(),
            summaries: Vec::new(),
        })
        .unwrap();
        let x = (0..row_count)
            .map(|row| (row % 997) as f64 / 997.0)
            .collect();
        let resources = (0..row_count).map(|row| (row % 1_024) as u32).collect();
        let key_resources = (0..row_count)
            .map(|row| ((row * 17) % 1_024) as u32)
            .collect();
        let state = StateStore::new(
            &model,
            vec![
                TableInit::new("world", "Resource", 1_024, Vec::new()),
                TableInit::new("world", "KeyResource", 1_024, Vec::new()),
                TableInit::new(
                    "world",
                    "Person",
                    row_count,
                    vec![
                        ColumnInit::new("x", ColumnData::Real(x)),
                        ColumnInit::new("resource", ColumnData::Ref(resources)),
                        ColumnInit::new("key_resource", ColumnData::Ref(key_resources)),
                    ],
                ),
            ],
        )
        .unwrap();
        (model, state)
    }

    fn parameter_type_fixture(
        row_count: usize,
        parameter_type: ParamType,
        default: ParamValue,
    ) -> ValidatedModel {
        validate(Model {
            name: "tile-parameter-type".to_owned(),
            dt: 1.0,
            params: vec![ParamDecl {
                name: "rate".to_owned(),
                ty: parameter_type,
                default,
                prior: None,
            }],
            boxes: vec![ModelBox {
                name: "world".to_owned(),
                tables: vec![Table {
                    name: "Person".to_owned(),
                    size_hint: row_count as u64,
                    attrs: Vec::new(),
                }],
                transitions: vec![Transition {
                    name: "parameter-hazard".to_owned(),
                    table: "Person".to_owned(),
                    guard: Expr::Bool { value: true },
                    hazard: Expr::Add {
                        lhs: Box::new(Expr::Param {
                            name: "rate".to_owned(),
                        }),
                        rhs: Box::new(Expr::Real { value: 0.0 }),
                    },
                    effects: Vec::new(),
                    contests: Vec::new(),
                }],
                inputs: Vec::new(),
                outputs: Vec::new(),
                views: Vec::new(),
                grouped_views: Vec::new(),
            }],
            wires: Vec::new(),
            summaries: Vec::new(),
        })
        .unwrap()
    }

    fn parameter_type_state(model: &ValidatedModel, row_count: usize) -> StateStore {
        StateStore::new(
            model,
            vec![TableInit::new("world", "Person", row_count, Vec::new())],
        )
        .unwrap()
    }

    fn tiled_race_fingerprint(
        model: &ValidatedModel,
        state: &StateStore,
        workers: usize,
        tile_rows: usize,
    ) -> Vec<(u32, u32, usize, u32, usize, u32, u64)> {
        with_test_tick_tiles(workers, tile_rows, 0, || {
            let params = ParamEnv::defaults(model);
            let snapshot = state.snapshot();
            let mut results = prepare_tiled_candidates(model, &snapshot, &params, 0xC0FFEE, 7);
            results[0][0]
                .take()
                .expect("transition should clear the tiling threshold")
                .unwrap()
                .into_iter()
                .map(|candidate| {
                    let claim = &candidate.claims[0];
                    let OrderingValue::RaceTime(time) = claim.ordering else {
                        panic!("test claim must retain its race time");
                    };
                    (
                        candidate.rule_id,
                        candidate.rule_word,
                        candidate.row,
                        candidate.entity_id,
                        claim.table_index,
                        claim.resource_row,
                        time.to_bits(),
                    )
                })
                .collect()
        })
    }

    fn tiled_key_fingerprint(
        model: &ValidatedModel,
        state: &StateStore,
        workers: usize,
        tile_rows: usize,
    ) -> Vec<(usize, u32, u64)> {
        with_test_tick_tiles(workers, tile_rows, 0, || {
            let params = ParamEnv::defaults(model);
            let snapshot = state.snapshot();
            let mut results = prepare_tiled_candidates(model, &snapshot, &params, 0xC0FFEE, 7);
            results[0][1]
                .take()
                .expect("key transition should clear the tiling threshold")
                .unwrap()
                .into_iter()
                .map(|candidate| {
                    let claim = &candidate.claims[0];
                    let OrderingValue::Real(key) = claim.ordering else {
                        panic!("test claim must retain its Real key");
                    };
                    (candidate.row, claim.resource_row, key.to_bits())
                })
                .collect()
        })
    }

    #[test]
    fn real_chain_racing_clock_and_key_are_bit_identical_across_workers_and_tiles() {
        let (model, state) = tiled_fixture(65_537);
        let serial_races = tiled_race_fingerprint(&model, &state, 1, 257);
        let serial_keys = tiled_key_fingerprint(&model, &state, 1, 257);
        assert!(!serial_races.is_empty());
        assert!(!serial_keys.is_empty());
        for workers in [1, 2, 4] {
            for tile_rows in [257, 1_024, 4_093] {
                assert_eq!(
                    tiled_race_fingerprint(&model, &state, workers, tile_rows),
                    serial_races,
                    "racing clock changed at {workers} workers and {tile_rows} rows/tile"
                );
                assert_eq!(
                    tiled_key_fingerprint(&model, &state, workers, tile_rows),
                    serial_keys,
                    "claim key changed at {workers} workers and {tile_rows} rows/tile"
                );
            }
        }
    }

    #[test]
    fn tick_report_is_identical_across_workers_and_tiles() {
        let evaluate = |workers, tile_rows| {
            let (model, mut state) = tiled_fixture(65_537);
            with_test_tick_tiles(workers, tile_rows, 0, || {
                let params = ParamEnv::defaults(&model);
                run_tick(&model, &mut state, &params, 0xC0FFEE, 7).unwrap()
            })
        };
        let serial = evaluate(1, 257);
        for workers in [1, 2, 4] {
            for tile_rows in [257, 1_024, 4_093] {
                assert_eq!(
                    evaluate(workers, tile_rows),
                    serial,
                    "tick output changed at {workers} workers and {tile_rows} rows/tile"
                );
            }
        }
    }

    #[test]
    fn wrong_typed_parameter_environment_matches_fallback_across_workers_and_tiles() {
        let row_count = 65_537;
        let model = parameter_type_fixture(
            row_count,
            ParamType::Real,
            ParamValue::Real { value: 0.001 },
        );
        let environment_model =
            parameter_type_fixture(row_count, ParamType::Int, ParamValue::Int { value: 1 });
        let params = ParamEnv::defaults(&environment_model);
        let evaluate = |workers, tile_rows, threshold| {
            let mut state = parameter_type_state(&model, row_count);
            with_test_tick_tiles(workers, tile_rows, threshold, || {
                run_tick(&model, &mut state, &params, 0xC0FFEE, 7)
                    .expect_err("the mismatched parameter environment must be rejected")
                    .to_string()
            })
        };

        let fallback_error = evaluate(1, 1_024, row_count + 1);
        assert!(
            fallback_error.contains("parameter environment value for 'rate' has the wrong type")
        );
        for workers in [1, 2, 4] {
            for tile_rows in [257, 1_024, 4_093] {
                assert_eq!(
                    evaluate(workers, tile_rows, 0),
                    fallback_error,
                    "parameter error changed at {workers} workers and {tile_rows} rows/tile"
                );
            }
        }
    }

    #[test]
    fn racing_clock_filter_rejects_below_lo_and_admits_boundary_envelope() {
        let filter = RacingClockFilter::for_hazard(0.025, 1.0).unwrap();
        let below = f64::from_bits(filter.lo.to_bits() - 1);
        let above = f64::from_bits(filter.lo.to_bits() + 1);

        assert!(!filter.admits(below));
        assert!(filter.admits(filter.lo));
        assert!(filter.admits(above));
    }

    #[test]
    fn guarded_racing_clock_preserves_firing_set_bits_and_contested_winner() -> Result<(), TickError>
    {
        let seed = 0xC0FFEE;
        let tick = 7;
        let rule_id = 19;
        let rule_word = 29;
        let dt = 1.0;
        let mut rejected = 0;
        let mut admitted = 0;

        for lambda in [
            0.001, 0.002, 0.0025, 0.003, 0.012, 0.018, 0.020, 0.025, 1e300,
        ] {
            let filter = RacingClockFilter::for_hazard(lambda, dt).unwrap();
            for row in 0..100_000 {
                let entity_id = row as u32;
                let uniform = uniform_f64(seed, tick, rule_word, entity_id, 0);
                if filter.admits(uniform) {
                    admitted += 1;
                } else {
                    rejected += 1;
                }
                let oracle = exp_f64(seed, tick, rule_word, entity_id, 0, lambda);
                let expected = (oracle.partial_cmp(&dt) == Some(Ordering::Less))
                    .then_some((entity_id, oracle.to_bits()));
                let actual = candidate_race_time(
                    RacingClockCoordinates {
                        seed,
                        tick,
                        rule_id,
                        rule_word,
                        row,
                    },
                    lambda,
                    dt,
                    Some(filter),
                )?
                .map(|(entity_id, race_time)| (entity_id, race_time.to_bits()));
                assert_eq!(actual, expected, "firing set changed for hazard {lambda}");
            }
        }
        assert!(rejected > 0, "the sweep must exercise the fast reject path");
        assert!(
            admitted > 0,
            "the sweep must exercise the canonical ln path"
        );

        // A contested transition consumes exact race-time bits for its argmin.
        // Group rows onto 128 resources and prove every winner is unchanged.
        let lambda = 0.025;
        let filter = RacingClockFilter::for_hazard(lambda, dt).unwrap();
        let mut oracle_winners: Vec<Option<(f64, u32)>> = vec![None; 128];
        let mut guarded_winners: Vec<Option<(f64, u32)>> = vec![None; 128];
        for row in 0..100_000 {
            let entity_id = row as u32;
            let resource = row % 128;
            let oracle = exp_f64(seed, tick, rule_word, entity_id, 0, lambda);
            if oracle.partial_cmp(&dt) == Some(Ordering::Less) {
                let candidate = (oracle, entity_id);
                if oracle_winners[resource].map_or(true, |winner| {
                    candidate
                        .0
                        .total_cmp(&winner.0)
                        .then(candidate.1.cmp(&winner.1))
                        == Ordering::Less
                }) {
                    oracle_winners[resource] = Some(candidate);
                }
            }
            if let Some((entity_id, race_time)) = candidate_race_time(
                RacingClockCoordinates {
                    seed,
                    tick,
                    rule_id,
                    rule_word,
                    row,
                },
                lambda,
                dt,
                Some(filter),
            )? {
                let candidate = (race_time, entity_id);
                if guarded_winners[resource].map_or(true, |winner| {
                    candidate
                        .0
                        .total_cmp(&winner.0)
                        .then(candidate.1.cmp(&winner.1))
                        == Ordering::Less
                }) {
                    guarded_winners[resource] = Some(candidate);
                }
            }
        }
        assert_eq!(
            guarded_winners
                .iter()
                .map(|winner| winner.map(|(time, entity)| (time.to_bits(), entity)))
                .collect::<Vec<_>>(),
            oracle_winners
                .iter()
                .map(|winner| winner.map(|(time, entity)| (time.to_bits(), entity)))
                .collect::<Vec<_>>()
        );

        Ok::<(), TickError>(())
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn racing_clock_filter_cannot_hide_entity_id_overflow() {
        let row = u32::MAX as usize + 1;
        let error = candidate_race_time(
            RacingClockCoordinates {
                seed: 1,
                tick: 0,
                rule_id: 7,
                rule_word: 11,
                row,
            },
            0.001,
            1.0,
            Some(RacingClockFilter { lo: 1.0 }),
        )
        .expect_err("row conversion must precede a filter that rejects every draw");
        match error {
            TickError::EntityIdOverflow {
                rule_id,
                row: error_row,
            } => {
                assert_eq!(rule_id, 7);
                assert_eq!(error_row, row);
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn threshold_falls_back_and_only_tick_orchestration_can_spawn() {
        let row_count = 4_097;
        let (model, state) = tiled_fixture(row_count);
        with_test_tick_tiles(4, 257, row_count + 1, || {
            let params = ParamEnv::defaults(&model);
            let snapshot = state.snapshot();
            let results = prepare_tiled_candidates(&model, &snapshot, &params, 1, 0);
            assert!(results[0][0].is_none());
        });

        let production_executor = include_str!("executor.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        let production_eval = include_str!("eval.rs")
            .split_once("#[cfg(test)]")
            .unwrap()
            .0;
        assert_eq!(
            production_executor
                .matches("std::thread::scope(|scope|")
                .count(),
            1
        );
        assert!(!production_eval.contains("std::thread::scope(|scope|"));
    }
}
