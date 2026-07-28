//! Deterministic, snapshot-isolated synchronous box composition.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use sembla_ir::{
    AggOp, AttrType, ClaimOrdering, Effect, Expr, FeatureSet, OutputBuilder, SummaryReduce,
    ValidatedModel, ViewReduce, GROUPED_OBSERVATIONS_FEATURE,
};

use crate::eval::{
    eval_column, eval_gather, eval_typed_ref_column, eval_typed_ref_gather,
    expr_is_gather_eligible, expr_is_gather_eligible_int, prepare_row_expr,
    tick_tile_rows_for_live_set, tick_tiling_enabled, tick_worker_count, tiled_expr_footprint,
    AggCache, EvalError, EvalTable, ParamEnv, PreparedColumn, PreparedExpr, PreparedValue,
    TiledExprFootprint, ValueColumn,
};
use crate::rng::{exp_f64, exp_f64_from_uniform, uniform_f64};
use crate::state::{ColumnData, InputTable, ResolvedWriteColumn, Snapshot, StateError, StateStore};

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

/// Conservative IR-only eligibility for one declared observation view.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceViewEligibility {
    pub box_name: String,
    pub name: String,
    pub eligible: bool,
    pub reason: &'static str,
}

/// Run-wide device-observation decision. State download may be skipped only
/// when this decision is eligible; one host-bound view forces the complete run
/// back to host observation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceObservationEligibility {
    pub eligible: bool,
    pub reason: &'static str,
    pub views: Vec<DeviceViewEligibility>,
}

/// Decides device-observation eligibility from validated IR.
///
/// The expression check deliberately reuses the evaluator's gather predicate:
/// `Expr::Agg`, `Expr::Input`, and row-fallible checked integer arithmetic are
/// therefore rejected without maintaining another expression whitelist.
/// Integer `count`, `min`, and `max` are commutative monoids. Grouped views are
/// count-only and their validated Enum, Ref, and banded Int keys are exactly
/// boundable at runtime. Sums stay on the host because Real has a canonical
/// ascending-row order and reassociated Int can overflow differently. Real
/// min/max also stay on the host because the CPU's total ordering (including
/// NaNs) is not this integer reduction.
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct TiledPlanProfile {
    retained_root_bytes_per_row: usize,
    peak_bytes_per_row: usize,
    node_count: usize,
}

impl TiledPlanProfile {
    fn include(&mut self, footprint: TiledExprFootprint) {
        self.retained_root_bytes_per_row = self
            .retained_root_bytes_per_row
            .saturating_add(footprint.root_bytes_per_row);
        self.peak_bytes_per_row = self.peak_bytes_per_row.max(footprint.peak_bytes_per_row);
        self.node_count = self.node_count.saturating_add(footprint.node_count);
    }

    fn live_set_bytes_per_row(self) -> usize {
        self.retained_root_bytes_per_row
            .saturating_add(self.peak_bytes_per_row)
            .max(1)
    }
}

struct TransitionTilingCandidate {
    box_index: usize,
    transition_index: usize,
    table_index: usize,
    row_count: usize,
    profile: TiledPlanProfile,
}

struct PreparedTransition<'state> {
    box_index: usize,
    transition_index: usize,
    rule_id: u32,
    rule_word: u32,
    table_index: usize,
    row_count: usize,
    tile_rows: usize,
    guard: PreparedExpr<'state>,
    hazard: PreparedExpr<'state>,
    race_strategy: RacingClockStrategy,
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

fn transition_tiling_profile(
    model: &ValidatedModel,
    box_index: usize,
    transition_index: usize,
) -> Result<Option<TiledPlanProfile>, TickError> {
    let model_box = &model.model().boxes[box_index];
    let transition = &model_box.transitions[transition_index];
    let table = EvalTable::new(model, &model_box.name, &transition.table)?;
    let mut profile = TiledPlanProfile::default();
    for expression in [&transition.guard, &transition.hazard] {
        let Some(footprint) = tiled_expr_footprint(expression, table)? else {
            return Ok(None);
        };
        profile.include(footprint);
    }
    for claim in &transition.contests {
        let Some(footprint) = tiled_expr_footprint(&claim.resource, table)? else {
            return Ok(None);
        };
        profile.include(footprint);
        if let ClaimOrdering::Key { expr } = &claim.ordering {
            let Some(footprint) = tiled_expr_footprint(expr, table)? else {
                return Ok(None);
            };
            profile.include(footprint);
        }
    }
    Ok(Some(profile))
}

fn prepare_tiled_transition<'state>(
    model: &ValidatedModel,
    box_index: usize,
    transition_index: usize,
    tile_rows: usize,
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
    let race_strategy = if row_count != 0 && has_constant_hazard(&transition.hazard) {
        let PreparedColumn::Real(values) = hazard.tile(0, 1)? else {
            return Err(TickError::Evaluation(
                "transition hazard did not prepare as Real".to_owned(),
            ));
        };
        RacingClockStrategy::for_transition(
            RacingClockFilter::for_hazard(values[0], model.model().dt),
            transition,
        )
    } else {
        RacingClockStrategy::Canonical
    };
    Ok(Some(PreparedTransition {
        box_index,
        transition_index,
        rule_id: validated.rule_id,
        rule_word: validated.rule_word,
        table_index,
        row_count,
        tile_rows,
        guard,
        hazard,
        race_strategy,
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
                    let Some(firing) = candidate_race_time(
                        RacingClockCoordinates {
                            seed,
                            tick,
                            rule_id: plan.rule_id,
                            rule_word: plan.rule_word,
                            row,
                        },
                        lambda,
                        dt,
                        plan.race_strategy,
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
                            (PreparedClaimOrdering::RaceTime, None) => OrderingValue::RaceTime(
                                firing
                                    .race_time
                                    .expect("a contested transition must sample its race time"),
                            ),
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
                        entity_id: firing.entity_id,
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
    let mut candidates = Vec::new();
    for (box_index, model_box) in model.model().boxes.iter().enumerate() {
        for (transition_index, transition) in model_box.transitions.iter().enumerate() {
            let table_index = model_box
                .tables
                .iter()
                .position(|table| table.name == transition.table)
                .expect("validated transition table disappeared");
            let row_count = match snapshot.row_count(&model_box.name, &transition.table) {
                Ok(row_count) => row_count,
                Err(error) => {
                    results[box_index][transition_index] = Some(Err(error.into()));
                    continue;
                }
            };
            match transition_tiling_profile(model, box_index, transition_index) {
                Ok(Some(profile)) => candidates.push(TransitionTilingCandidate {
                    box_index,
                    transition_index,
                    table_index,
                    row_count,
                    profile,
                }),
                Ok(None) => {}
                Err(error) => results[box_index][transition_index] = Some(Err(error)),
            }
        }
    }

    let mut candidate_groups: Vec<(usize, usize, usize, Vec<usize>)> = Vec::new();
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        if let Some((_, _, _, indices)) =
            candidate_groups
                .iter_mut()
                .find(|(box_index, table_index, _, _)| {
                    (*box_index, *table_index) == (candidate.box_index, candidate.table_index)
                })
        {
            indices.push(candidate_index);
        } else {
            candidate_groups.push((
                candidate.box_index,
                candidate.table_index,
                candidate.row_count,
                vec![candidate_index],
            ));
        }
    }

    let mut plans = Vec::new();
    for (_, _, row_count, candidate_indices) in candidate_groups {
        let node_count = candidate_indices.iter().fold(0_usize, |total, index| {
            total.saturating_add(candidates[*index].profile.node_count)
        });
        let live_set_bytes_per_row = candidate_indices
            .iter()
            .map(|index| candidates[*index].profile.live_set_bytes_per_row())
            .max()
            .unwrap_or(1);
        let tile_rows = tick_tile_rows_for_live_set(live_set_bytes_per_row);
        if !tick_tiling_enabled(row_count, node_count) || row_count <= tile_rows {
            continue;
        }
        for candidate_index in candidate_indices {
            let candidate = &candidates[candidate_index];
            match prepare_tiled_transition(
                model,
                candidate.box_index,
                candidate.transition_index,
                tile_rows,
                snapshot,
                params,
            ) {
                Ok(Some(plan)) => plans.push(plan),
                Ok(None) => {}
                Err(error) => {
                    results[candidate.box_index][candidate.transition_index] = Some(Err(error));
                }
            }
        }
    }
    if plans.is_empty() {
        return results;
    }

    let mut groups: Vec<(usize, usize, usize, usize, Vec<usize>)> = Vec::new();
    for (plan_index, plan) in plans.iter().enumerate() {
        if let Some((_, _, _, _, indices)) =
            groups
                .iter_mut()
                .find(|(box_index, table_index, tile_rows, _, _)| {
                    (*box_index, *table_index, *tile_rows)
                        == (plan.box_index, plan.table_index, plan.tile_rows)
                })
        {
            indices.push(plan_index);
        } else {
            groups.push((
                plan.box_index,
                plan.table_index,
                plan.tile_rows,
                plan.row_count,
                vec![plan_index],
            ));
        }
    }
    let mut tasks = Vec::new();
    for (_, _, tile_rows, row_count, plan_indices) in groups {
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

    let mut destinations = Vec::new();
    let mut pending = Vec::new();
    for outcome in &mut box_outcomes {
        let destination_base = destinations.len();
        destinations.append(&mut outcome.destinations);
        pending.extend(
            std::mem::take(&mut outcome.pending)
                .into_iter()
                .map(|mut write| {
                    write.destination_index += destination_base;
                    write
                }),
        );
    }
    detect_double_writes(&pending, &destinations, model)?;
    let apply_result = {
        let mut writes = state.write_buffer()?;
        pending.iter().try_for_each(|write| {
            let destination = destinations[write.destination_index]
                .resolution
                .as_ref()
                .copied()
                .map_err(Clone::clone)?;
            match &write.value {
                PendingValue::Real(value) => {
                    writes.set_resolved_real(destination, write.row, *value)
                }
                PendingValue::Int(value) => writes.set_resolved_int(destination, write.row, *value),
                PendingValue::Enum(value) => {
                    writes.set_resolved_enum(destination, write.row, *value)
                }
                PendingValue::Ref(value) => writes.set_resolved_ref(destination, write.row, *value),
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

struct ViewTilingCandidate {
    ordinal: usize,
    box_index: usize,
    view_index: usize,
    table_index: usize,
    row_count: usize,
    profile: TiledPlanProfile,
}

struct PreparedView<'state> {
    ordinal: usize,
    box_index: usize,
    view_index: usize,
    table_index: usize,
    row_count: usize,
    tile_rows: usize,
    reduce: ViewReduce,
    filter: Option<PreparedExpr<'state>>,
    value: Option<PreparedExpr<'state>>,
}

struct ViewTileOutput<'state> {
    plan_index: usize,
    start: usize,
    count: Option<Result<usize, TickError>>,
    filter: Option<Result<PreparedColumn<'state>, TickError>>,
    value: Option<Result<PreparedColumn<'state>, TickError>>,
}

fn evaluate_view_tile_task<'state>(
    task: &TileTask,
    plans: &[PreparedView<'state>],
) -> Vec<ViewTileOutput<'state>> {
    task.plan_indices
        .iter()
        .map(|plan_index| {
            let plan = &plans[*plan_index];
            let filter = plan
                .filter
                .as_ref()
                .map(|filter| filter.tile(task.start, task.end).map_err(Into::into));
            if plan.reduce == ViewReduce::Count {
                let count = match filter {
                    Some(Ok(PreparedColumn::Bool(selected))) => {
                        Ok(selected.iter().filter(|value| **value).count())
                    }
                    Some(Ok(other)) => Err(prepared_runtime_type("view filter", &other)),
                    Some(Err(error)) => Err(error),
                    None => Ok(task.end - task.start),
                };
                ViewTileOutput {
                    plan_index: *plan_index,
                    start: task.start,
                    count: Some(count),
                    filter: None,
                    value: None,
                }
            } else {
                ViewTileOutput {
                    plan_index: *plan_index,
                    start: task.start,
                    count: None,
                    filter,
                    value: plan
                        .value
                        .as_ref()
                        .map(|value| value.tile(task.start, task.end).map_err(Into::into)),
                }
            }
        })
        .collect()
}

fn reduce_prepared_view_tiles(
    box_name: &str,
    view_name: &str,
    reduce: ViewReduce,
    columns: Vec<PreparedColumn<'_>>,
    filters: Vec<Option<Cow<'_, [bool]>>>,
) -> Result<ObservationValue, TickError> {
    match columns.first() {
        Some(PreparedColumn::Int(_)) => {
            let mut result = match reduce {
                ViewReduce::Sum => 0_i64,
                ViewReduce::Min => i64::MAX,
                ViewReduce::Max => i64::MIN,
                ViewReduce::Count => unreachable!("count does not evaluate a value"),
            };
            for (column, filter) in columns.into_iter().zip(filters) {
                let PreparedColumn::Int(values) = column else {
                    return Err(TickError::Evaluation(
                        "view value changed type between tiles".to_owned(),
                    ));
                };
                for (offset, value) in values.iter().copied().enumerate() {
                    if filter.as_ref().is_some_and(|selected| !selected[offset]) {
                        continue;
                    }
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
            }
            Ok(ObservationValue::Int(result))
        }
        Some(PreparedColumn::Real(_)) => {
            let mut result = match reduce {
                ViewReduce::Sum => 0.0,
                ViewReduce::Min => f64::INFINITY,
                ViewReduce::Max => f64::NEG_INFINITY,
                ViewReduce::Count => unreachable!("count does not evaluate a value"),
            };
            // This is the canonical Level A reduction order: fixed tiles are
            // consumed by ascending start row, and rows stay ascending inside
            // each tile. Workers evaluate row-local values only; they never
            // form floating-point partial sums.
            for (column, filter) in columns.into_iter().zip(filters) {
                let PreparedColumn::Real(values) = column else {
                    return Err(TickError::Evaluation(
                        "view value changed type between tiles".to_owned(),
                    ));
                };
                for (offset, value) in values.iter().copied().enumerate() {
                    if filter.as_ref().is_some_and(|selected| !selected[offset]) {
                        continue;
                    }
                    result = match reduce {
                        ViewReduce::Sum => result + value,
                        ViewReduce::Min if value.total_cmp(&result) == Ordering::Less => value,
                        ViewReduce::Max if value.total_cmp(&result) == Ordering::Greater => value,
                        ViewReduce::Min | ViewReduce::Max => result,
                        ViewReduce::Count => unreachable!(),
                    };
                }
            }
            Ok(ObservationValue::Real(result))
        }
        Some(_) => Err(TickError::InvalidRuntimeType {
            context: "view value".to_owned(),
            found: "non-numeric".to_owned(),
        }),
        None => unreachable!("tiled views always contain at least one row"),
    }
}

/// Prepares eligible committed-state views and opens one fixed-task parallel
/// region for observation. Count filters and row-local numeric expressions are
/// evaluated per tile. Numeric reductions happen only after every tile has
/// joined: integer operations combine in row order for identical overflow
/// behaviour, while `f64` values are accumulated one row at a time in canonical
/// ascending order. Aggregate/input-dependent or row-fallible expressions keep
/// the original whole-column path.
fn prepare_tiled_views(
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
    let mut candidates = Vec::new();
    let mut ordinal = 0;
    for (box_index, model_box) in model.model().boxes.iter().enumerate() {
        for (view_index, view) in model_box.views.iter().enumerate() {
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
            let table = match EvalTable::new(model, &model_box.name, &view.table) {
                Ok(table) => table,
                Err(error) => {
                    results[ordinal] = Some(Err(error.into()));
                    ordinal += 1;
                    continue;
                }
            };
            let mut profile = TiledPlanProfile::default();
            let filter_eligible = match &view.filter {
                Some(filter) => match tiled_expr_footprint(filter, table) {
                    Ok(Some(footprint)) => {
                        profile.include(footprint);
                        true
                    }
                    Ok(None) => false,
                    Err(error) => {
                        results[ordinal] = Some(Err(error.into()));
                        false
                    }
                },
                None => true,
            };
            let value_eligible = match view.reduce {
                ViewReduce::Count => true,
                ViewReduce::Sum | ViewReduce::Min | ViewReduce::Max => {
                    let expression = view
                        .value
                        .as_ref()
                        .expect("validated numeric view has a value");
                    match tiled_expr_footprint(expression, table) {
                        Ok(Some(footprint)) => {
                            profile.include(footprint);
                            true
                        }
                        Ok(None) => false,
                        Err(error) => {
                            results[ordinal] = Some(Err(error.into()));
                            false
                        }
                    }
                }
            };
            if filter_eligible && value_eligible {
                candidates.push(ViewTilingCandidate {
                    ordinal,
                    box_index,
                    view_index,
                    table_index,
                    row_count,
                    profile,
                });
            }
            ordinal += 1;
        }
    }

    let mut candidate_groups: Vec<(usize, usize, usize, Vec<usize>)> = Vec::new();
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        if let Some((_, _, _, indices)) =
            candidate_groups
                .iter_mut()
                .find(|(box_index, table_index, _, _)| {
                    (*box_index, *table_index) == (candidate.box_index, candidate.table_index)
                })
        {
            indices.push(candidate_index);
        } else {
            candidate_groups.push((
                candidate.box_index,
                candidate.table_index,
                candidate.row_count,
                vec![candidate_index],
            ));
        }
    }

    let mut plans = Vec::new();
    for (_, _, row_count, candidate_indices) in candidate_groups {
        // A Count plan reduces and drops its filter before the next plan.
        // Numeric filter/value roots stay in the task output until canonical
        // ordered reduction, so account for those retained roots while walking
        // plans in declaration order. Work always accumulates across plans.
        let mut node_count = 0_usize;
        let mut retained_numeric_roots = 0_usize;
        let mut live_set_bytes_per_row = 1_usize;
        for candidate_index in &candidate_indices {
            let candidate = &candidates[*candidate_index];
            node_count = node_count.saturating_add(candidate.profile.node_count);
            live_set_bytes_per_row = live_set_bytes_per_row.max(
                retained_numeric_roots.saturating_add(candidate.profile.live_set_bytes_per_row()),
            );
            if model.model().boxes[candidate.box_index].views[candidate.view_index].reduce
                != ViewReduce::Count
            {
                retained_numeric_roots = retained_numeric_roots
                    .saturating_add(candidate.profile.retained_root_bytes_per_row);
            }
        }
        let tile_rows = tick_tile_rows_for_live_set(live_set_bytes_per_row);
        if !tick_tiling_enabled(row_count, node_count) || row_count <= tile_rows {
            continue;
        }
        for candidate_index in candidate_indices {
            let candidate = &candidates[candidate_index];
            let model_box = &model.model().boxes[candidate.box_index];
            let view = &model_box.views[candidate.view_index];
            let table = match EvalTable::new(model, &model_box.name, &view.table) {
                Ok(table) => table,
                Err(error) => {
                    results[candidate.ordinal] = Some(Err(error.into()));
                    continue;
                }
            };
            let filter = match &view.filter {
                Some(filter) => match prepare_row_expr(filter, table, snapshot, params) {
                    Ok(Some(filter)) => Some(filter),
                    Ok(None) => continue,
                    Err(error) => {
                        results[candidate.ordinal] = Some(Err(error.into()));
                        continue;
                    }
                },
                None => None,
            };
            let value = match view.reduce {
                ViewReduce::Count => None,
                ViewReduce::Sum | ViewReduce::Min | ViewReduce::Max => {
                    let expression = view
                        .value
                        .as_ref()
                        .expect("validated numeric view has a value");
                    match prepare_row_expr(expression, table, snapshot, params) {
                        Ok(Some(value)) => Some(value),
                        Ok(None) => continue,
                        Err(error) => {
                            results[candidate.ordinal] = Some(Err(error.into()));
                            continue;
                        }
                    }
                }
            };
            plans.push(PreparedView {
                ordinal: candidate.ordinal,
                box_index: candidate.box_index,
                view_index: candidate.view_index,
                table_index: candidate.table_index,
                row_count: candidate.row_count,
                tile_rows,
                reduce: view.reduce,
                filter,
                value,
            });
        }
    }
    if plans.is_empty() {
        return results;
    }

    let mut groups: Vec<(usize, usize, usize, usize, Vec<usize>)> = Vec::new();
    for (plan_index, plan) in plans.iter().enumerate() {
        if let Some((_, _, _, _, indices)) =
            groups
                .iter_mut()
                .find(|(box_index, table_index, tile_rows, _, _)| {
                    (*box_index, *table_index, *tile_rows)
                        == (plan.box_index, plan.table_index, plan.tile_rows)
                })
        {
            indices.push(plan_index);
        } else {
            groups.push((
                plan.box_index,
                plan.table_index,
                plan.tile_rows,
                plan.row_count,
                vec![plan_index],
            ));
        }
    }
    let mut tasks = Vec::new();
    for (_, _, tile_rows, row_count, plan_indices) in groups {
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
            task_outputs[task_index] = Some(evaluate_view_tile_task(task, &plans));
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
                                    evaluate_view_tile_task(&tasks[task_index], plans),
                                )
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .flat_map(|handle| handle.join().expect("view tile worker panicked"))
                .collect::<Vec<_>>()
        });
        for (task_index, output) in worker_outputs {
            task_outputs[task_index] = Some(output);
        }
    }

    let mut plan_outputs = std::iter::repeat_with(Vec::new)
        .take(plans.len())
        .collect::<Vec<Vec<ViewTileOutput<'_>>>>();
    for outputs in task_outputs.into_iter().map(Option::unwrap) {
        for output in outputs {
            plan_outputs[output.plan_index].push(output);
        }
    }
    for (plan_index, plan) in plans.iter().enumerate() {
        let mut outputs = std::mem::take(&mut plan_outputs[plan_index]);
        outputs.sort_by_key(|output| output.start);
        let model_box = &model.model().boxes[plan.box_index];
        let view = &model_box.views[plan.view_index];
        if plan.reduce == ViewReduce::Count {
            let count = outputs
                .into_iter()
                .map(|output| output.count.expect("count tile must contain a partial"))
                .try_fold(0_usize, |total, count| count.map(|count| total + count))
                .and_then(|count| {
                    i64::try_from(count).map_err(|_| {
                        TickError::Evaluation(format!(
                            "view '{}.{}' count exceeds i64",
                            model_box.name, view.name
                        ))
                    })
                })
                .map(ObservationValue::Int);
            results[plan.ordinal] = Some(count);
            continue;
        }

        let mut filters = Vec::with_capacity(outputs.len());
        let mut values = Vec::with_capacity(outputs.len());
        let mut error = None;
        for output in outputs {
            let filter = match output.filter {
                Some(Ok(PreparedColumn::Bool(selected))) => Some(selected),
                Some(Ok(other)) => {
                    error = Some(prepared_runtime_type("view filter", &other));
                    None
                }
                Some(Err(found)) => {
                    error = Some(found);
                    None
                }
                None => None,
            };
            filters.push(filter);
            values.push(output.value);
            if error.is_some() {
                break;
            }
        }
        if let Some(error) = error {
            results[plan.ordinal] = Some(Err(error));
            continue;
        }

        let value = match plan.reduce {
            ViewReduce::Count => unreachable!("count views return above"),
            ViewReduce::Sum | ViewReduce::Min | ViewReduce::Max => {
                let mut columns = Vec::with_capacity(values.len());
                let mut value_error = None;
                for value in values {
                    match value.expect("numeric view tile must evaluate a value") {
                        Ok(column) => columns.push(column),
                        Err(error) => {
                            value_error = Some(error);
                            break;
                        }
                    }
                }
                if let Some(error) = value_error {
                    Err(error)
                } else {
                    reduce_prepared_view_tiles(
                        &model_box.name,
                        &view.name,
                        plan.reduce,
                        columns,
                        filters,
                    )
                }
            }
        };
        results[plan.ordinal] = Some(value);
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
    let mut tiled_values = prepare_tiled_views(model, &snapshot, params);
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
        let race_strategy = if has_constant_hazard(&transition.hazard) {
            RacingClockStrategy::for_transition(
                hazards
                    .first()
                    .and_then(|lambda| RacingClockFilter::for_hazard(*lambda, model.model().dt)),
                transition,
            )
        } else {
            RacingClockStrategy::Canonical
        };
        let mut push_candidate = |row: usize, firing: CandidateFiring| -> Result<(), TickError> {
            let mut claims = Vec::with_capacity(claim_columns.len());
            for (resource_table, resources, key_column, claim) in &claim_columns {
                let ordering = match (&claim.ordering, key_column) {
                    (ClaimOrdering::RaceTime, None) => OrderingValue::RaceTime(
                        firing
                            .race_time
                            .expect("a contested transition must sample its race time"),
                    ),
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
                entity_id: firing.entity_id,
                row,
                claims,
            });
            Ok(())
        };
        for (row, (guard, lambda)) in guards.into_iter().zip(hazards).enumerate() {
            if !guard || lambda.partial_cmp(&0.0) != Some(Ordering::Greater) {
                continue;
            }
            let Some(firing) = candidate_race_time(
                RacingClockCoordinates {
                    seed,
                    tick,
                    rule_id: validated.rule_id,
                    rule_word: validated.rule_word,
                    row,
                },
                lambda,
                model.model().dt,
                race_strategy,
            )?
            else {
                continue;
            };
            push_candidate(row, firing)?;
        }
    }
    let resolution = resolve_claims(&candidates, model_box.tables.len(), model_box)?;
    let mut destinations = Vec::new();
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
        let mut winner_rows = None;
        let mut effect_columns = Vec::with_capacity(transition.effects.len());
        for effect in &transition.effects {
            let Effect::SetAttr { attr, value } = effect;
            let attr_index = schema
                .attrs
                .iter()
                .position(|declaration| declaration.name == *attr)
                .expect("validated effect attribute disappeared");
            let destination = &schema.attrs[attr_index];
            let effect_table = match &destination.ty {
                AttrType::Ref { .. } => table,
                _ => table.with_expected_attr(attr)?,
            };
            let gather = expr_is_gather_eligible(value, effect_table)?;
            let rows = gather.then(|| {
                winner_rows.get_or_insert_with(|| {
                    let rows = winner_indices
                        .iter()
                        .map(|index| candidates[*index].row)
                        .collect::<Vec<_>>();
                    debug_assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
                    rows
                })
            });
            let (values, gathered) = match (&destination.ty, rows) {
                (AttrType::Ref { .. }, Some(rows)) => (
                    PendingColumn::Ref(
                        eval_typed_ref_gather(
                            value,
                            effect_table,
                            rows,
                            snapshot,
                            params,
                            &mut cache,
                        )?
                        .expect("gather eligibility and preparation must agree")
                        .values,
                    ),
                    true,
                ),
                (AttrType::Ref { .. }, None) => (
                    PendingColumn::Ref(
                        eval_typed_ref_column(value, effect_table, snapshot, params, &mut cache)?
                            .values,
                    ),
                    false,
                ),
                (_, Some(rows)) => (
                    PendingColumn::Value(
                        eval_gather(value, effect_table, rows, snapshot, params, &mut cache)?
                            .expect("gather eligibility and preparation must agree"),
                    ),
                    true,
                ),
                (_, None) => (
                    PendingColumn::Value(eval_column(
                        value,
                        effect_table,
                        snapshot,
                        params,
                        &mut cache,
                    )?),
                    false,
                ),
            };
            // Resolve once now, but publish a failure only at this effect's
            // first pending write. That retains later effect-evaluation,
            // DoubleWrite, and write-buffer error precedence from the original
            // per-write lookup path.
            let destination_index = destinations.len();
            destinations.push(PendingDestination {
                box_index,
                table_index,
                attr_index,
                resolution: snapshot.resolve_write_column(
                    &model_box.name,
                    &schema.name,
                    &destination.name,
                ),
            });
            effect_columns.push(EffectColumn {
                destination_index,
                values,
                gathered,
            });
        }
        for (winner_offset, candidate_index) in winner_indices.into_iter().enumerate() {
            let candidate = &candidates[candidate_index];
            for effect in &effect_columns {
                let value_index = if effect.gathered {
                    winner_offset
                } else {
                    candidate.row
                };
                pending.push(PendingWrite {
                    destination_index: effect.destination_index,
                    row: candidate.row,
                    value: effect.values.at(value_index)?,
                    rule_id: candidate.rule_id,
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
        destinations,
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

struct EffectColumn {
    destination_index: usize,
    values: PendingColumn,
    gathered: bool,
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

fn detect_double_writes(
    pending: &[PendingWrite],
    destinations: &[PendingDestination],
    model: &ValidatedModel,
) -> Result<(), TickError> {
    DOUBLE_WRITE_SCRATCH.with(|scratch| {
        let mut scratch = scratch.borrow_mut();
        scratch.prepare(pending, destinations);

        // The replaced stable sort and map both reported the lexicographically
        // first duplicated cell. The bitmap finds that cell without retaining
        // writer identity; the terminating error path recovers the first two
        // push-order writers with one linear scan.
        let mut collision = None;
        for write in pending {
            if scratch.mark(write) {
                let cell = write_cell(write, destinations);
                if collision.map_or(true, |reported| cell < reported) {
                    collision = Some(cell);
                }
            }
        }

        let Some(collision) = collision else {
            return Ok(());
        };
        let mut first_index = None;
        for (index, write) in pending.iter().enumerate() {
            if write_cell(write, destinations) != collision {
                continue;
            }
            let Some(first_index) = first_index else {
                first_index = Some(index);
                continue;
            };
            return double_write_error(&pending[first_index], write, destinations, model);
        }
        unreachable!("a bitmap collision must have at least two writers")
    })
}

fn write_cell(write: &PendingWrite, destinations: &[PendingDestination]) -> WriteCell {
    let destination = &destinations[write.destination_index];
    (
        destination.box_index,
        destination.table_index,
        destination.attr_index,
        write.row,
    )
}

fn double_write_error(
    first: &PendingWrite,
    second: &PendingWrite,
    destinations: &[PendingDestination],
    model: &ValidatedModel,
) -> Result<(), TickError> {
    let destination = &destinations[first.destination_index];
    let model_box = &model.model().boxes[destination.box_index];
    Err(TickError::DoubleWrite {
        box_name: model_box.name.clone().into_boxed_str(),
        table: model_box.tables[destination.table_index]
            .name
            .clone()
            .into_boxed_str(),
        attr: model_box.tables[destination.table_index].attrs[destination.attr_index]
            .name
            .clone()
            .into_boxed_str(),
        row: first.row,
        first_rule_id: first.rule_id,
        first_transition: transition_name(model, first.rule_id).into(),
        second_rule_id: second.rule_id,
        second_transition: transition_name(model, second.rule_id).into(),
    })
}

fn transition_name(model: &ValidatedModel, rule_id: u32) -> &str {
    let validated = model
        .transitions()
        .iter()
        .find(|transition| transition.rule_id == rule_id)
        .expect("pending write has a validated transition");
    &model.model().boxes[validated.box_index].transitions[validated.transition_index].name
}

#[cfg(test)]
mod double_write_bitmap_tests {
    use super::*;
    use crate::state::{ColumnData, ColumnInit, StateStore, TableInit};
    use sembla_ir::{validate, Attr, Box as ModelBox, Effect, Model, Table, Transition};

    #[test]
    fn scratch_reuses_storage_and_sizes_only_written_columns() {
        const ATTRS: usize = 128;
        const ROWS: usize = 130;

        DOUBLE_WRITE_SCRATCH.with(|scratch| {
            *scratch.borrow_mut() = DoubleWriteScratch::default();
        });
        let attrs = (0..ATTRS)
            .map(|index| Attr {
                name: format!("field_{index}"),
                ty: AttrType::Int,
            })
            .collect::<Vec<_>>();
        let model = validate(Model {
            name: "bitmap-scratch".to_owned(),
            dt: 1.0,
            params: Vec::new(),
            boxes: vec![ModelBox {
                name: "world".to_owned(),
                tables: vec![Table {
                    name: "Item".to_owned(),
                    size_hint: ROWS as u64,
                    attrs: attrs.clone(),
                }],
                transitions: vec![Transition {
                    name: "write-one-of-many".to_owned(),
                    table: "Item".to_owned(),
                    guard: Expr::Bool { value: true },
                    hazard: Expr::Real { value: 1.0e300 },
                    effects: vec![Effect::SetAttr {
                        attr: "field_0".to_owned(),
                        value: Expr::Int { value: 1 },
                    }],
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
        .unwrap();
        let columns = attrs
            .iter()
            .map(|attr| ColumnInit::new(&attr.name, ColumnData::Int(vec![0; ROWS])))
            .collect();
        let mut state =
            StateStore::new(&model, vec![TableInit::new("world", "Item", ROWS, columns)]).unwrap();
        let params = ParamEnv::defaults(&model);

        run_tick(&model, &mut state, &params, 7, 0).unwrap();
        let first = DOUBLE_WRITE_SCRATCH.with(|scratch| {
            let scratch = scratch.borrow();
            assert_eq!(scratch.destination_slots.len(), 1);
            assert_eq!(
                scratch.columns.len(),
                1,
                "127 unwritten columns need no bitmap"
            );
            assert_eq!(scratch.words.len(), ROWS.div_ceil(u64::BITS as usize));
            assert_eq!(scratch.touched_words.len(), scratch.words.len());
            (
                scratch.words.as_ptr(),
                scratch.words.capacity(),
                scratch.touched_words.as_ptr(),
                scratch.touched_words.capacity(),
            )
        });

        run_tick(&model, &mut state, &params, 7, 1).unwrap();
        DOUBLE_WRITE_SCRATCH.with(|scratch| {
            let scratch = scratch.borrow();
            assert_eq!(scratch.destination_slots.len(), 1);
            assert_eq!(scratch.columns.len(), 1);
            assert_eq!(scratch.words.len(), ROWS.div_ceil(u64::BITS as usize));
            assert_eq!(
                (
                    scratch.words.as_ptr(),
                    scratch.words.capacity(),
                    scratch.touched_words.as_ptr(),
                    scratch.touched_words.capacity(),
                ),
                first,
                "bitmap and touched-word storage must be reused across ticks"
            );
        });
    }
}

#[cfg(test)]
mod parallel_tests {
    use super::*;
    use crate::eval::with_test_tick_tiles;
    use crate::state::{ColumnInit, StateStore, TableInit};
    use sembla_ir::{
        parse_json, validate, Attr, Box as ModelBox, Model, ParamDecl, ParamType, ParamValue,
        ResourceClaim, Table, Transition, ViewDecl,
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
                        effects: vec![Effect::SetAttr {
                            attr: "x".to_owned(),
                            value: Expr::Add {
                                lhs: Box::new(Expr::SelfAttr {
                                    name: "x".to_owned(),
                                }),
                                rhs: Box::new(Expr::Real { value: 0.125 }),
                            },
                        }],
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
                views: vec![
                    ViewDecl {
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
                    },
                    ViewDecl {
                        name: "weighted_x".to_owned(),
                        table: "Person".to_owned(),
                        filter: Some(Expr::Gt {
                            lhs: Box::new(Expr::SelfAttr {
                                name: "x".to_owned(),
                            }),
                            rhs: Box::new(Expr::Real { value: 0.2 }),
                        }),
                        value: Some(Expr::Div {
                            lhs: Box::new(Expr::Add {
                                lhs: Box::new(Expr::SelfAttr {
                                    name: "x".to_owned(),
                                }),
                                rhs: Box::new(Expr::Real { value: 0.375 }),
                            }),
                            rhs: Box::new(Expr::Real { value: 1.25 }),
                        }),
                        reduce: ViewReduce::Sum,
                    },
                ],
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

    fn static_view_profiles(
        model: &ValidatedModel,
        box_index: usize,
        table_name: &str,
    ) -> Vec<TiledPlanProfile> {
        let model_box = &model.model().boxes[box_index];
        let table = EvalTable::new(model, &model_box.name, table_name).unwrap();
        model_box
            .views
            .iter()
            .filter(|view| view.table == table_name)
            .map(|view| {
                let mut profile = TiledPlanProfile::default();
                if let Some(filter) = &view.filter {
                    profile.include(tiled_expr_footprint(filter, table).unwrap().unwrap());
                }
                if let Some(value) = &view.value {
                    profile.include(tiled_expr_footprint(value, table).unwrap().unwrap());
                }
                profile
            })
            .collect()
    }

    #[test]
    fn benchmark_shapes_derive_hand_checked_tiles_and_work_decisions() {
        let (mixed_transition_model, _) = tiled_fixture(1);
        let mixed_profiles = mixed_transition_model.model().boxes[0]
            .transitions
            .iter()
            .enumerate()
            .map(|(index, _)| {
                transition_tiling_profile(&mixed_transition_model, 0, index)
                    .unwrap()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            mixed_profiles
                .iter()
                .map(|profile| profile.node_count)
                .sum::<usize>(),
            15
        );
        let mixed_live_set = mixed_profiles
            .iter()
            .map(|profile| profile.live_set_bytes_per_row())
            .max()
            .unwrap();
        assert_eq!(mixed_live_set, 37);
        assert_eq!(tick_tile_rows_for_live_set(mixed_live_set), 832);

        let demographic = validate(
            parse_json(include_str!(
                "../../../fixtures/demographic/benchmark/demographic_slots.no-grouped.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let transition_profiles = demographic.model().boxes[0]
            .transitions
            .iter()
            .enumerate()
            .map(|(index, _)| {
                transition_tiling_profile(&demographic, 0, index)
                    .unwrap()
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let transition_nodes = transition_profiles
            .iter()
            .map(|profile| profile.node_count)
            .sum::<usize>();
        let transition_live_set = transition_profiles
            .iter()
            .map(|profile| profile.live_set_bytes_per_row())
            .max()
            .unwrap();
        assert_eq!(transition_nodes, 65);
        assert_eq!(transition_live_set, 33);
        assert_eq!(tick_tile_rows_for_live_set(transition_live_set), 960);
        assert!(tick_tiling_enabled(1_000_000, transition_nodes));

        let demographic_views = static_view_profiles(&demographic, 0, "person_slot");
        let demographic_view_nodes = demographic_views
            .iter()
            .map(|profile| profile.node_count)
            .sum::<usize>();
        let demographic_view_live_set = demographic_views
            .iter()
            .map(|profile| profile.live_set_bytes_per_row())
            .max()
            .unwrap();
        assert_eq!(demographic_view_nodes, 67);
        assert_eq!(demographic_view_live_set, 20);
        assert_eq!(
            tick_tile_rows_for_live_set(demographic_view_live_set),
            1_600
        );
        assert!(tick_tiling_enabled(1_000_000, demographic_view_nodes));

        let canary = validate(
            parse_json(include_str!(
                "../../../fixtures/performance/many_views_tiling_canary.json"
            ))
            .unwrap(),
        )
        .unwrap();
        let canary_views = static_view_profiles(&canary, 0, "row");
        let canary_view_nodes = canary_views
            .iter()
            .map(|profile| profile.node_count)
            .sum::<usize>();
        let canary_view_live_set = canary_views
            .iter()
            .map(|profile| profile.live_set_bytes_per_row())
            .max()
            .unwrap();
        assert_eq!(canary_view_nodes, 680);
        assert_eq!(canary_view_live_set, 41);
        assert_eq!(tick_tile_rows_for_live_set(canary_view_live_set), 768);
        assert!(tick_tiling_enabled(262_144, canary_view_nodes));
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
    fn numeric_view_sum_and_effect_state_are_bit_identical_across_workers_and_tiles() {
        let row_count = 65_537;
        let evaluate = |workers, tile_rows, threshold| {
            let (model, mut state) = tiled_fixture(row_count);
            let report = with_test_tick_tiles(workers, tile_rows, threshold, || {
                let params = ParamEnv::defaults(&model);
                run_tick(&model, &mut state, &params, 0xC0FFEE, 7).unwrap()
            });
            let sum_bits = report
                .views
                .iter()
                .find(|view| view.name == "weighted_x")
                .and_then(|view| match view.value {
                    ObservationValue::Real(value) => Some(value.to_bits()),
                    ObservationValue::Int(_) => None,
                })
                .expect("fixture must report the Real numeric view");
            let snapshot = state.snapshot();
            let state_bits = (0..row_count)
                .map(|row| {
                    snapshot
                        .real("world", "Person", "x", row)
                        .unwrap()
                        .to_bits()
                })
                .collect::<Vec<_>>();
            (sum_bits, state_bits)
        };

        let fallback = evaluate(1, 1_024, row_count + 1);
        assert!(
            fallback
                .1
                .iter()
                .enumerate()
                .any(|(row, bits)| { *bits != ((row % 997) as f64 / 997.0).to_bits() }),
            "the fixture must execute at least one effect write"
        );
        for workers in [1, 2, 4] {
            for tile_rows in [257, 1_024, 4_093] {
                assert_eq!(
                    evaluate(workers, tile_rows, 0),
                    fallback,
                    "numeric view or effect state changed at {workers} workers and {tile_rows} rows/tile"
                );
            }
        }
    }

    #[test]
    fn tiled_real_view_reduction_keeps_canonical_row_order() {
        let source = include_str!("executor.rs");
        let reduction = source
            .split_once("// This is the canonical Level A reduction order")
            .expect("tiled Real view reduction must document its canonical order")
            .1
            .split_once("Ok(ObservationValue::Real(result))")
            .expect("tiled Real reduction must return its ordered result")
            .0;
        assert!(reduction.contains("for (column, filter) in columns.into_iter().zip(filters)"));
        assert!(reduction.contains("for (offset, value) in values.iter().copied().enumerate()"));
        assert!(reduction.contains("ViewReduce::Sum => result + value"));
        assert!(!reduction.contains("sum::<f64>"));
        assert!(!reduction.contains("par_iter"));
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
                    RacingClockStrategy::Guarded(filter),
                )?
                .map(|firing| {
                    (
                        firing.entity_id,
                        firing
                            .race_time
                            .expect("the guarded path samples a race time")
                            .to_bits(),
                    )
                });
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
            if let Some(firing) = candidate_race_time(
                RacingClockCoordinates {
                    seed,
                    tick,
                    rule_id,
                    rule_word,
                    row,
                },
                lambda,
                dt,
                RacingClockStrategy::Guarded(filter),
            )? {
                let candidate = (
                    firing
                        .race_time
                        .expect("the guarded path samples a race time"),
                    firing.entity_id,
                );
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

    #[test]
    fn degenerate_uncontested_transition_skips_clock_but_contested_transition_does_not() {
        let make_transition = |contests| Transition {
            name: "degenerate".to_owned(),
            table: "Person".to_owned(),
            guard: Expr::Bool { value: true },
            hazard: Expr::Real { value: 1e300 },
            effects: Vec::new(),
            contests,
        };
        let filter = RacingClockFilter::for_hazard(1e300, 1.0).unwrap();
        assert_eq!(filter.threshold, 0.0);

        let uncontested = make_transition(Vec::new());
        let uncontested_strategy = RacingClockStrategy::for_transition(Some(filter), &uncontested);
        assert_eq!(uncontested_strategy, RacingClockStrategy::AlwaysFires);
        let firing = candidate_race_time(
            RacingClockCoordinates {
                seed: 1,
                tick: 0,
                rule_id: 7,
                rule_word: 11,
                row: 0,
            },
            1e300,
            1.0,
            uncontested_strategy,
        )
        .unwrap()
        .expect("the exact-zero threshold must always fire");
        assert_eq!(firing.entity_id, 0);
        assert_eq!(firing.race_time, None, "the fast path must not draw");

        let contested = make_transition(vec![ResourceClaim {
            resource: Expr::SelfAttr {
                name: "resource".to_owned(),
            },
            ordering: ClaimOrdering::RaceTime,
        }]);
        let contested_strategy = RacingClockStrategy::for_transition(Some(filter), &contested);
        assert_eq!(
            contested_strategy,
            RacingClockStrategy::Guarded(filter),
            "a contested transition consumes the exact race time"
        );
        let contested_firing = candidate_race_time(
            RacingClockCoordinates {
                seed: 1,
                tick: 0,
                rule_id: 7,
                rule_word: 11,
                row: 0,
            },
            1e300,
            1.0,
            contested_strategy,
        )
        .unwrap()
        .expect("the degenerate contested transition must still fire");
        assert_eq!(
            contested_firing.race_time.unwrap().to_bits(),
            exp_f64(1, 0, 11, 0, 0, 1e300).to_bits(),
            "the contested path must retain the canonical sampled time"
        );

        let ordinary = make_transition(Vec::new());
        let ordinary_filter = RacingClockFilter::for_hazard(0.025, 1.0).unwrap();
        assert_eq!(
            RacingClockStrategy::for_transition(Some(ordinary_filter), &ordinary),
            RacingClockStrategy::Guarded(ordinary_filter),
            "an uncontested transition with a nonzero threshold is not provably certain"
        );
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn racing_clock_fast_paths_cannot_hide_entity_id_overflow() {
        let row = u32::MAX as usize + 1;
        for strategy in [
            RacingClockStrategy::Guarded(RacingClockFilter {
                threshold: 1.0,
                lo: 1.0,
            }),
            RacingClockStrategy::AlwaysFires,
        ] {
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
                strategy,
            )
            .expect_err("row conversion must precede every fast path");
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
            2,
            "one fixed-task region stages transitions and one observes committed views"
        );
        assert!(!production_eval.contains("std::thread::scope(|scope|"));
    }
}
