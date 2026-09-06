//! Deterministic, snapshot-isolated synchronous box composition.

use std::cmp::Ordering;
use std::collections::HashMap;

use sembla_ir::{
    AggOp, AttrType, ClaimOrdering, Effect, Expr, FeatureSet, OutputBuilder, SummaryReduce,
    ValidatedModel, ViewReduce, GROUPED_OBSERVATIONS_FEATURE,
};

use crate::config::CpuExecutionConfig;
use crate::error::TickError;
use crate::eval::{
    eval_column, eval_gather, eval_typed_ref_column, eval_typed_ref_gather,
    expr_is_gather_eligible, prepare_row_expr, tick_tile_rows_for_live_set, tick_tiling_enabled,
    tick_worker_count, tiled_expr_footprint, AggCache, EvalTable, PreparedColumn, PreparedExpr,
    PreparedValue, TiledExprFootprint, ValueColumn,
};
use sembla_runtime::core::{
    ColumnData, GroupedViewValue, InputTable, ObservationValue, ParamEnv, Snapshot, StateError,
    StateStore, SummaryValue, ViewValue,
};
use sembla_runtime::engine::{exp_f64_from_uniform, ResolvedWriteColumn};
use sembla_runtime::rng::{exp_f64, uniform_f64};

mod staging;
mod tiling;

use staging::{detect_double_writes, stage_box};
use tiling::{prepare_tiled_candidates, prepare_tiled_views};
#[cfg(test)]
use tiling::{transition_tiling_profile, TiledPlanProfile};

/// Relative slack below the `exp(-lambda * dt)` boundary used only to reject
/// certain non-firers. Near the benchmark's thresholds, one binary64 ULP is at
/// most `2^-52` relative; `1e-12` is about 4,500 ULPs. That envelope dominates
/// the documented one-ULP platform `exp`/`ln` disagreement plus the handful of
/// rounding steps that form the threshold. Candidates inside the envelope are
/// still decided by the canonical platform-`ln` racing clock.
const RACING_CLOCK_FILTER_RELATIVE_MARGIN: f64 = 1e-12;

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

#[derive(Clone, Copy, Debug, PartialEq)]
struct RacingClockFilter {
    threshold: f64,
    lo: f64,
}

impl RacingClockFilter {
    fn for_hazard(lambda: f64, dt: f64) -> Option<Self> {
        if lambda.partial_cmp(&0.0) != Some(Ordering::Greater) {
            return None;
        }
        let threshold = (-(lambda * dt)).exp();
        let lo = threshold * (1.0 - RACING_CLOCK_FILTER_RELATIVE_MARGIN);
        (!lo.is_nan()).then_some(Self { threshold, lo })
    }

    fn admits(self, uniform: f64) -> bool {
        uniform.partial_cmp(&self.lo) != Some(Ordering::Less)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RacingClockStrategy {
    Canonical,
    Guarded(RacingClockFilter),
    AlwaysFires,
}

impl RacingClockStrategy {
    fn for_transition(
        filter: Option<RacingClockFilter>,
        transition: &sembla_ir::Transition,
    ) -> Self {
        match filter {
            // `Candidate` carries no sampled time. The only IR locations that
            // consume it are contests, and every contest is conservatively
            // treated as a consumer even when its ordering is key-based.
            Some(filter) if filter.threshold == 0.0 && transition.contests.is_empty() => {
                Self::AlwaysFires
            }
            Some(filter) => Self::Guarded(filter),
            None => Self::Canonical,
        }
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

#[derive(Clone, Copy, Debug, PartialEq)]
struct CandidateFiring {
    entity_id: u32,
    race_time: Option<f64>,
}

/// Converts the row before consulting the strategy so every enabled candidate
/// keeps the canonical `EntityIdOverflow` path. `AlwaysFires` therefore skips
/// only the draw and transform, never a diagnostic.
fn candidate_race_time(
    coordinates: RacingClockCoordinates,
    lambda: f64,
    dt: f64,
    strategy: RacingClockStrategy,
) -> Result<Option<CandidateFiring>, TickError> {
    let entity_id = u32::try_from(coordinates.row).map_err(|_| TickError::EntityIdOverflow {
        rule_id: coordinates.rule_id,
        row: coordinates.row,
    })?;
    let race_time = match strategy {
        RacingClockStrategy::AlwaysFires => {
            return Ok(Some(CandidateFiring {
                entity_id,
                race_time: None,
            }));
        }
        RacingClockStrategy::Guarded(filter) => {
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
        }
        RacingClockStrategy::Canonical => exp_f64(
            coordinates.seed,
            coordinates.tick,
            coordinates.rule_word,
            entity_id,
            0,
            lambda,
        ),
    };
    Ok(
        (race_time.partial_cmp(&dt) == Some(Ordering::Less)).then_some(CandidateFiring {
            entity_id,
            race_time: Some(race_time),
        }),
    )
}

#[derive(Clone, Debug)]
struct Candidate {
    /// Stable runtime identity used only for Philox and conflict tie-breaks.
    rule_word: u32,
    entity_id: u32,
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
struct PendingDestination {
    box_index: usize,
    table_index: usize,
    attr_index: usize,
    resolution: Result<ResolvedWriteColumn, StateError>,
}

#[derive(Clone, Debug)]
struct PendingWrite {
    destination_index: usize,
    row: usize,
    value: PendingValue,
    rule_id: u32,
}

type WriteCell = (usize, usize, usize, usize);
type WriteColumn = (usize, usize, usize);

#[derive(Clone, Copy, Debug)]
struct BitmapColumn {
    identity: WriteColumn,
    row_span: usize,
    word_start: usize,
}

#[derive(Default)]
struct DoubleWriteScratch {
    destination_slots: Vec<usize>,
    columns: Vec<BitmapColumn>,
    words: Vec<u64>,
    touched_words: Vec<usize>,
}

impl DoubleWriteScratch {
    fn prepare(&mut self, pending: &[PendingWrite], destinations: &[PendingDestination]) {
        for word_index in self.touched_words.drain(..) {
            self.words[word_index] = 0;
        }
        self.destination_slots.clear();
        self.columns.clear();
        self.destination_slots.reserve(destinations.len());
        self.columns.reserve(destinations.len());

        for destination in destinations {
            let identity = (
                destination.box_index,
                destination.table_index,
                destination.attr_index,
            );
            let slot = self
                .columns
                .iter()
                .position(|column| column.identity == identity)
                .unwrap_or_else(|| {
                    let slot = self.columns.len();
                    self.columns.push(BitmapColumn {
                        identity,
                        row_span: 0,
                        word_start: 0,
                    });
                    slot
                });
            self.destination_slots.push(slot);
        }

        for write in pending {
            let column = &mut self.columns[self.destination_slots[write.destination_index]];
            column.row_span = column.row_span.max(write.row + 1);
        }

        let mut word_count = 0;
        for column in &mut self.columns {
            column.word_start = word_count;
            word_count = word_count
                .checked_add(column.row_span.div_ceil(u64::BITS as usize))
                .expect("double-write bitmap size exceeds usize");
        }
        self.words.resize(word_count, 0);
        self.words.truncate(word_count);
        self.touched_words.reserve(word_count);
    }

    fn mark(&mut self, write: &PendingWrite) -> bool {
        let column = &self.columns[self.destination_slots[write.destination_index]];
        let word_index = column.word_start + write.row / u64::BITS as usize;
        let mask = 1_u64 << (write.row % u64::BITS as usize);
        let word = self.words[word_index];
        if word & mask != 0 {
            return true;
        }
        if word == 0 {
            self.touched_words.push(word_index);
        }
        self.words[word_index] = word | mask;
        false
    }
}

thread_local! {
    static DOUBLE_WRITE_SCRATCH: std::cell::RefCell<DoubleWriteScratch> =
        std::cell::RefCell::new(DoubleWriteScratch::default());
}

struct TickOutcome {
    report: TickReport,
    fired_per_resource_table: Vec<(String, usize)>,
}

struct BoxOutcome {
    destinations: Vec<PendingDestination>,
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

/// Reusable CPU execution policy with no process-global tuning state.
#[derive(Clone, Debug, Default)]
pub struct CpuExecutor {
    config: CpuExecutionConfig,
}

impl CpuExecutor {
    pub fn new(config: CpuExecutionConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &CpuExecutionConfig {
        &self.config
    }

    pub fn run_tick(
        &self,
        model: &ValidatedModel,
        state: &mut StateStore,
        params: &ParamEnv,
        seed: u64,
        tick: u32,
    ) -> Result<TickReport, TickError> {
        self.run_tick_with_features(model, state, params, seed, tick, &FeatureSet::new())
    }

    pub fn run_tick_with_features(
        &self,
        model: &ValidatedModel,
        state: &mut StateStore,
        params: &ParamEnv,
        seed: u64,
        tick: u32,
        enabled_features: &FeatureSet,
    ) -> Result<TickReport, TickError> {
        require_grouped_observations_feature(model, enabled_features)?;
        Ok(execute_tick(model, state, params, seed, tick, &self.config)?.report)
    }

    pub fn run_tick_with_features_timed(
        &self,
        model: &ValidatedModel,
        state: &mut StateStore,
        params: &ParamEnv,
        seed: u64,
        tick: u32,
        enabled_features: &FeatureSet,
    ) -> Result<TimedTickReport, TickError> {
        run_tick_timed_configured(
            model,
            state,
            params,
            seed,
            tick,
            enabled_features,
            &self.config,
        )
    }

    pub fn run(
        &self,
        model: &ValidatedModel,
        state: &mut StateStore,
        params: &ParamEnv,
        seed: u64,
        n_ticks: u32,
    ) -> Result<RunReport, TickError> {
        self.run_with_features(model, state, params, seed, n_ticks, &FeatureSet::new())
    }

    pub fn run_with_features(
        &self,
        model: &ValidatedModel,
        state: &mut StateStore,
        params: &ParamEnv,
        seed: u64,
        n_ticks: u32,
        enabled_features: &FeatureSet,
    ) -> Result<RunReport, TickError> {
        run_configured(
            model,
            state,
            params,
            seed,
            n_ticks,
            enabled_features,
            &self.config,
        )
    }

    pub fn observe_views(
        &self,
        model: &ValidatedModel,
        state: &StateStore,
        params: &ParamEnv,
    ) -> Result<Vec<ViewValue>, TickError> {
        observe_views_configured(model, state, params, &self.config)
    }
}

/// Executes and commits one deterministic, snapshot-isolated tick.
pub fn run_tick(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
) -> Result<TickReport, TickError> {
    CpuExecutor::default().run_tick(model, state, params, seed, tick)
}

pub fn run_tick_with_features(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
    enabled_features: &FeatureSet,
) -> Result<TickReport, TickError> {
    CpuExecutor::default().run_tick_with_features(
        model,
        state,
        params,
        seed,
        tick,
        enabled_features,
    )
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
    CpuExecutor::default().run_tick_with_features_timed(
        model,
        state,
        params,
        seed,
        tick,
        enabled_features,
    )
}

fn run_tick_timed_configured(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
    enabled_features: &FeatureSet,
    config: &CpuExecutionConfig,
) -> Result<TimedTickReport, TickError> {
    require_grouped_observations_feature(model, enabled_features)?;

    let started = std::time::Instant::now();
    let box_outcomes = execute_tick_state(model, state, params, seed, tick, config)?;
    let execute_tick = started.elapsed();

    let started = std::time::Instant::now();
    let (views, grouped_views) = observe_tick(model, state, params, config)?;
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
    CpuExecutor::default().run(model, state, params, seed, n_ticks)
}

pub fn run_with_features(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    n_ticks: u32,
    enabled_features: &FeatureSet,
) -> Result<RunReport, TickError> {
    CpuExecutor::default().run_with_features(model, state, params, seed, n_ticks, enabled_features)
}

fn run_configured(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    n_ticks: u32,
    enabled_features: &FeatureSet,
    config: &CpuExecutionConfig,
) -> Result<RunReport, TickError> {
    require_grouped_observations_feature(model, enabled_features)?;
    let mut ticks = Vec::with_capacity(n_ticks as usize);
    let mut warnings = Vec::new();
    for tick in 0..n_ticks {
        let outcome = execute_tick(model, state, params, seed, tick, config)?;
        for (table, deferred_count) in &outcome.report.deferred_per_resource_table {
            let fired_count = outcome
                .fired_per_resource_table
                .iter()
                .find(|(name, _)| name == table)
                .map_or(0, |(_, count)| *count);
            if exceeds_saturation_threshold(*deferred_count, fired_count) {
                warnings.push(SaturationWarning {
                    tick,
                    table: table.clone(),
                    deferred_count: *deferred_count,
                    fired_count,
                });
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
    config: &CpuExecutionConfig,
) -> Result<TickOutcome, TickError> {
    let box_outcomes = execute_tick_state(model, state, params, seed, tick, config)?;
    let (views, grouped_views) = observe_tick(model, state, params, config)?;
    Ok(finish_tick(model, tick, box_outcomes, views, grouped_views))
}

fn execute_tick_state(
    model: &ValidatedModel,
    state: &mut StateStore,
    params: &ParamEnv,
    seed: u64,
    tick: u32,
    config: &CpuExecutionConfig,
) -> Result<Vec<BoxOutcome>, TickError> {
    let snapshot = state.snapshot();
    let mut tiled_candidates =
        prepare_tiled_candidates(model, &snapshot, params, seed, tick, config);
    let mut box_outcomes = Vec::with_capacity(model.model().boxes.len());
    for (box_index, candidates) in tiled_candidates.iter_mut().enumerate() {
        box_outcomes.push(stage_box(
            model, box_index, &snapshot, params, seed, tick, candidates,
        )?);
    }

    // Destinations are box-local. Check every box before applying any writes;
    // declaration order also preserves the first duplicated cell diagnostic.
    for outcome in &box_outcomes {
        detect_double_writes(&outcome.pending, &outcome.destinations, model)?;
    }
    let apply_result = {
        let mut writes = state.write_buffer()?;
        box_outcomes.iter_mut().try_for_each(|outcome| {
            std::mem::take(&mut outcome.pending)
                .into_iter()
                .try_for_each(|write| {
                    let destination = outcome.destinations[write.destination_index]
                        .resolution
                        .as_ref()
                        .copied()
                        .map_err(Clone::clone)?;
                    match &write.value {
                        PendingValue::Real(value) => {
                            writes.set_resolved_real(destination, write.row, *value)
                        }
                        PendingValue::Int(value) => {
                            writes.set_resolved_int(destination, write.row, *value)
                        }
                        PendingValue::Enum(value) => {
                            writes.set_resolved_enum(destination, write.row, *value)
                        }
                        PendingValue::Ref(value) => {
                            writes.set_resolved_ref(destination, write.row, *value)
                        }
                    }
                })
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
    config: &CpuExecutionConfig,
) -> Result<(Vec<ViewValue>, Vec<GroupedViewValue>), TickError> {
    // Observation is deliberately evaluated only after commit and receives an
    // immutable store. It cannot consume RNG coordinates, stage writes, or
    // influence conflict resolution or scheduling.
    Ok((
        observe_views_configured(model, state, params, config)?,
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
    observe_views_configured(model, state, params, &CpuExecutionConfig::default())
}

fn observe_views_configured(
    model: &ValidatedModel,
    state: &StateStore,
    params: &ParamEnv,
    config: &CpuExecutionConfig,
) -> Result<Vec<ViewValue>, TickError> {
    let snapshot = state.snapshot();
    let mut tiled_values = prepare_tiled_views(model, &snapshot, params, config);
    let mut cache = AggCache::new(model, &snapshot, params);
    let mut observations = Vec::new();
    let mut view_ordinal = 0;
    for model_box in &model.model().boxes {
        for view in &model_box.views {
            let tiled_value = tiled_values[view_ordinal].take();
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

enum GroupedKeyColumn<'a> {
    Enum(&'a [u16]),
    Ref(&'a [u32]),
    IntBand(&'a [i64], u64),
}

impl GroupedKeyColumn<'_> {
    fn at(&self, row: usize) -> i64 {
        match self {
            Self::Enum(values) => i64::from(values[row]),
            Self::Ref(values) => i64::from(values[row]),
            Self::IntBand(values, width) => match i64::try_from(*width) {
                Ok(width) => values[row].div_euclid(width),
                // A positive width larger than i64::MAX spans every
                // nonnegative Int; all negative Ints belong to band -1.
                Err(_) => -i64::from(values[row] < 0),
            },
        }
    }
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
                    ValueColumn::Bool(values) => Some(values),
                    other => return Err(runtime_type("grouped view filter", &other)),
                },
                None => None,
            };
            let mut rows =
                (0..row_count).filter(|row| selected.as_ref().map_or(true, |mask| mask[*row]));
            let Some(first_row) = rows.next() else {
                continue;
            };
            // Resolve keys only when a row is selected, preserving diagnostics
            // for empty tables and filters that exclude every row.
            let columns = view
                .keys
                .iter()
                .map(|key| {
                    let attr = table_decl
                        .attrs
                        .iter()
                        .find(|attr| attr.name == key.attr)
                        .expect("validated grouped key disappeared");
                    let column =
                        snapshot.resolve_column(&model_box.name, &view.table, &key.attr)?;
                    Ok(match (&attr.ty, key.band_width) {
                        (AttrType::Enum { .. }, None) => {
                            GroupedKeyColumn::Enum(column.enum_values()?)
                        }
                        (AttrType::Ref { .. }, None) => GroupedKeyColumn::Ref(column.ref_values()?),
                        (AttrType::Int, Some(width)) => {
                            GroupedKeyColumn::IntBand(column.int_values()?, width)
                        }
                        _ => unreachable!("validated grouped key type disappeared"),
                    })
                })
                .collect::<Result<Vec<_>, StateError>>()?;
            let mut buckets: HashMap<Vec<i64>, usize> = HashMap::new();
            let mut tuple = Vec::with_capacity(columns.len());
            for row in std::iter::once(first_row).chain(rows) {
                tuple.clear();
                tuple.extend(columns.iter().map(|column| column.at(row)));
                if let Some(count) = buckets.get_mut(tuple.as_slice()) {
                    *count += 1;
                } else {
                    buckets.insert(tuple.clone(), 1);
                }
            }
            // Counts are exact integers; hash iteration never determines
            // publication order. Only the distinct keys need sorting.
            let mut buckets = buckets.into_iter().collect::<Vec<_>>();
            buckets.sort_unstable_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));
            observations.extend(buckets.into_iter().map(|(keys, count)| GroupedViewValue {
                box_name: model_box.name.clone(),
                name: view.name.clone(),
                keys: keys.into_iter().map(i128::from).collect(),
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
    for wire in model.wires() {
        let source_box = &model.model().boxes[wire.from_box_index];
        let output = &source_box.outputs[wire.output_index];
        let built = build_output(model, snapshot, params, source_box, output)?;
        let destination_index = model
            .global_input_index(wire.to_box_index, wire.input_index)
            .expect("validated wire destination disappeared");
        let destination = &mut inputs[destination_index];
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

fn prepared_runtime_type(context: &str, column: &PreparedColumn<'_>) -> TickError {
    let found = match column {
        PreparedColumn::Real(_) => "Real",
        PreparedColumn::Int(_) => "Int",
        PreparedColumn::Bool(_) => "Bool",
        PreparedColumn::Enum(_) => "Enum",
        PreparedColumn::Ref(_) => "Ref",
    };
    TickError::InvalidRuntimeType {
        context: context.to_owned(),
        found: found.to_owned(),
    }
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

#[cfg(test)]
#[path = "executor_double_write_tests.rs"]
mod double_write_bitmap_tests;

#[cfg(test)]
#[path = "executor_parallel_tests.rs"]
mod parallel_tests;
